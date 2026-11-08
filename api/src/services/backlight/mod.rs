//! Local Backlight control. Private packets and identifiers never enter the HTTP contract.
mod bluetooth;
mod delivery;
mod effects;
mod protocol;
pub mod routine;
use crate::error::{AppError, Result};
pub use bluetooth::Bluetooth;
use chrono::{DateTime, Utc};
use delivery::RetryClock;
pub use effects::EffectId;
pub use protocol::InactivitySettings;
use protocol::TvSyncProfile;
use routine::{
    BacklightOverride, BedtimeSettings, BedtimeUpdate, Clock, ControlSource, Policy, SystemClock,
    Target,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::Mutex;

// H605C drops an idle link after about twenty seconds. Frequent power queries
// keep it alive without reasserting the intended lighting selection.
const PROBE_INTERVAL: Duration = Duration::from_secs(2);

pub type Packet = [u8; 20];
pub struct Exchange {
    /// A newly established link requires current-target recovery, even if reads succeed.
    pub reconnected: bool,
    pub writes_accepted: bool,
    pub replies: Vec<Packet>,
}
#[derive(Debug)]
pub enum TransportError {
    Unavailable,
    Superseded,
}
/// Checked after connecting and before each write, including every scene-upload frame.
pub type CurrentOperation = dyn Fn() -> bool + Send + Sync;
pub type TransportFuture<'a> =
    Pin<Box<dyn Future<Output = std::result::Result<Exchange, TransportError>> + Send + 'a>>;
/// One complete GATT operation: ordered writes followed by correlated status queries.
pub trait GattTransport: Send + Sync {
    /// Verify a private backup belongs to the configured controller.
    fn matches_device(&self, identity: &str) -> bool;
    /// Check `current` after connection setup and before every write.
    /// Keep the operation lock until completion/cancellation; never interleave scene uploads.
    fn exchange<'a>(
        &'a self,
        writes: &'a [Packet],
        queries: &'a [Vec<u8>],
        current: &'a CurrentOperation,
    ) -> TransportFuture<'a>;
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Selection {
    Off {},
    TvSync {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        brightness: Option<Brightness>,
    },
    Color {
        rgb: Rgb,
        brightness: Brightness,
    },
    Effect {
        effect: EffectId,
        brightness: Brightness,
    },
}
/// Controller brightness command value (0–100); not photometric calibration.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(try_from = "u8", into = "u8")]
pub struct Brightness(u8);
impl TryFrom<u8> for Brightness {
    type Error = &'static str;
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        if value <= 100 {
            Ok(Self(value))
        } else {
            Err("Brightness must be between 0 and 100")
        }
    }
}
impl From<Brightness> for u8 {
    fn from(value: Brightness) -> Self {
        value.0
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}
#[derive(Clone, Serialize)]
pub struct Status {
    pub effects: &'static [effects::Effect],
    pub setup: Setup,
    pub reachability: Reachability,
    pub intended: Selection,
    pub acknowledged: Option<Selection>,
    pub observed: Option<Observation>,
    pub stale: bool,
    pub command: Command,
    pub recovery: Recovery,
    pub bedtime: BedtimeSettings,
    pub control_source: ControlSource,
    pub override_expires_at: Option<DateTime<Utc>>,
    pub next_transition: Option<DateTime<Utc>>,
}
/// Whether a validated private TV Sync profile is available.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Setup {
    Incomplete,
    Ready,
}
/// Device reachability, independent of a command acknowledgment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reachability {
    Unconfigured,
    Unknown,
    Reachable,
    Unreachable,
}
/// Outcome of the most recent intended-target dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Command {
    Idle,
    Pending,
    Acknowledged,
    Failed,
}
/// Only the mode categories supported by controller readback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservedMode {
    TvSync,
    Color,
    Other,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Recovery {
    Idle,
    Pending,
    Retrying,
    Recovered,
    ProfileRequired,
}
#[derive(Clone, Serialize)]
pub struct Observation {
    pub power: bool,
    pub brightness: u8,
    pub mode: ObservedMode,
    pub observed_at: DateTime<Utc>,
}
#[derive(Deserialize)]
#[serde(tag = "source", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProfileSource {
    Capture,
    Backup,
}
#[derive(Clone)]
pub struct BacklightService {
    db: SqlitePool,
    state: Arc<Mutex<Status>>,
    // All capture/read/write exchanges share this lock for the complete operation.
    worker: Arc<Mutex<()>>,
    // Persistence follows submission order independently of slow device operations.
    submission: Arc<Mutex<()>>,
    revision: Arc<AtomicU64>,
    transport: Option<Arc<dyn GattTransport>>,
    backup: Option<PathBuf>,
    profile: Arc<Mutex<Option<TvSyncProfile>>>,
    clock: Arc<dyn Clock>,
}
impl BacklightService {
    pub async fn new(
        db: SqlitePool,
        transport: Option<Arc<dyn GattTransport>>,
        backup: Option<PathBuf>,
    ) -> Result<Self> {
        Self::with_clock(db, transport, backup, Arc::new(SystemClock)).await
    }
    pub async fn with_clock(
        db: SqlitePool,
        transport: Option<Arc<dyn GattTransport>>,
        backup: Option<PathBuf>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self> {
        let row = sqlx::query!(
            "SELECT tv_sync_profile, intended, bedtime, manual_override FROM backlight WHERE id = 1"
        )
        .fetch_one(&db)
        .await
        .map_err(anyhow::Error::from)?;
        let profile = row
            .tv_sync_profile
            .and_then(|json| serde_json::from_str::<TvSyncProfile>(&json).ok())
            .and_then(TvSyncProfile::validate);
        let bedtime: BedtimeSettings =
            serde_json::from_str(&row.bedtime).map_err(anyhow::Error::from)?;
        let manual_override: Option<BacklightOverride> = row
            .manual_override
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(anyhow::Error::from)?;
        // Startup applies current wall time; expired overrides are dropped, not replayed.
        let target = Policy::restored(bedtime, manual_override).resolve(clock.now());
        let service = Self {
            db,
            state: Arc::new(Mutex::new(Status {
                effects: effects::CATALOG,
                setup: if profile.is_some() {
                    Setup::Ready
                } else {
                    Setup::Incomplete
                },
                reachability: if transport.is_some() {
                    Reachability::Unknown
                } else {
                    Reachability::Unconfigured
                },
                intended: target.selection(),
                acknowledged: None,
                observed: None,
                stale: true,
                command: Command::Idle,
                recovery: Recovery::Pending,
                bedtime: target.bedtime().clone(),
                control_source: target.control_source(),
                override_expires_at: target.override_expires_at(),
                next_transition: target.next_transition(),
            })),
            worker: Arc::new(Mutex::new(())),
            submission: Arc::new(Mutex::new(())),
            revision: Arc::new(AtomicU64::new(0)),
            transport,
            backup,
            profile: Arc::new(Mutex::new(profile)),
            clock,
        };
        service.persist(&target).await?;
        Ok(service)
    }
    pub async fn status(&self) -> Status {
        let mut status = self.state.lock().await.clone();
        status.stale = status.reachability != Reachability::Reachable
            || status
                .observed
                .as_ref()
                .is_none_or(|o| Utc::now() - o.observed_at > chrono::Duration::seconds(30));
        status
    }
    /// Read or set the controller's native same-tone hold without changing lighting intent.
    pub async fn inactivity(&self, seconds: Option<u16>) -> Result<InactivitySettings> {
        if seconds.is_some_and(|value| !(120..=1800).contains(&value)) {
            return Err(AppError::Validation(
                "Inactivity hold must be between 120 and 1800 seconds".into(),
            ));
        }
        let _worker = self.worker.lock().await;
        let current = self.read_inactivity(&[]).await?;
        let Some(seconds) = seconds else {
            return Ok(current);
        };
        let desired = current.hold_same_tone(seconds);
        let observed = self.read_inactivity(&[desired.write()]).await?;
        if observed != desired {
            return Err(AppError::ServiceUnavailable(
                "Backlight inactivity setting was not confirmed".into(),
            ));
        }
        Ok(observed)
    }
    async fn read_inactivity(&self, writes: &[Packet]) -> Result<InactivitySettings> {
        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| AppError::ServiceUnavailable("Backlight is not configured".into()))?;
        let queries = vec![
            vec![0xaa, 1],
            vec![0xaa, 4],
            vec![0xaa, 5, 1],
            vec![0xaa, 0xa9, 0x0a],
        ];
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            transport.exchange(writes, &queries, &|| true),
        )
        .await
        .unwrap_or(Err(TransportError::Unavailable))
        .map_err(|_| AppError::ServiceUnavailable("Backlight could not be reached".into()))?;
        self.record_observation(&result).await;
        if !writes.is_empty() && !result.writes_accepted {
            return Err(AppError::ServiceUnavailable(
                "Backlight inactivity setting was not accepted".into(),
            ));
        }
        InactivitySettings::capture(&result.replies).ok_or_else(|| {
            AppError::ServiceUnavailable("Backlight inactivity settings are unavailable".into())
        })
    }
    async fn exchange(
        &self,
        writes: &[Packet],
        current: &CurrentOperation,
    ) -> std::result::Result<Exchange, TransportError> {
        let Some(transport) = &self.transport else {
            return Err(TransportError::Unavailable);
        };
        let queries = vec![vec![0xaa, 1], vec![0xaa, 4], vec![0xaa, 5, 1]];
        tokio::time::timeout(
            Duration::from_secs(30),
            transport.exchange(writes, &queries, current),
        )
        .await
        .unwrap_or(Err(TransportError::Unavailable))
    }
    async fn store_profile(&self, profile: TvSyncProfile) -> Result<()> {
        let json = serde_json::to_string(&profile).map_err(anyhow::Error::from)?;
        sqlx::query!(
            "UPDATE backlight SET tv_sync_profile = ? WHERE id = 1",
            json
        )
        .execute(&self.db)
        .await
        .map_err(anyhow::Error::from)?;
        *self.profile.lock().await = Some(profile);
        self.state.lock().await.profile_stored();
        Ok(())
    }
    async fn capture(&self) -> Result<()> {
        let result = self
            .exchange(&[], &|| true)
            .await
            .map_err(|_| AppError::ServiceUnavailable("Backlight could not be reached".into()))?;
        self.record_observation(&result).await;
        let profile = TvSyncProfile::capture(&result.replies).ok_or_else(|| {
            AppError::Conflict("TV Sync must be active to capture its profile".into())
        })?;
        self.store_profile(profile).await
    }
    pub async fn save_profile(&self, source: ProfileSource) -> Result<Status> {
        let _worker = self.worker.lock().await;
        match source {
            ProfileSource::Capture => self.capture().await?,
            ProfileSource::Backup => {
                let path = self
                    .backup
                    .as_ref()
                    .ok_or_else(|| AppError::Conflict("No private backup is configured".into()))?;
                let bytes = tokio::fs::read(path)
                    .await
                    .map_err(|_| AppError::Conflict("Private backup could not be read".into()))?;
                let backup: Backup = serde_json::from_slice(&bytes)
                    .map_err(|_| AppError::Validation("Private backup is invalid".into()))?;
                if !self
                    .transport
                    .as_ref()
                    .is_some_and(|device| device.matches_device(&backup.bluetooth_address))
                {
                    return Err(AppError::Validation(
                        "Private backup belongs to a different controller".into(),
                    ));
                }
                let profile = backup.profile().ok_or_else(|| {
                    AppError::Validation(
                        "Private backup does not contain a valid TV Sync profile".into(),
                    )
                })?;
                self.store_profile(profile).await?;
            }
        }
        Ok(self.status().await)
    }
    async fn record_observation(&self, result: &Exchange) {
        let observed = protocol::observation(&result.replies);
        self.state
            .lock()
            .await
            .observation_recorded(observed, result.reconnected);
    }
    pub async fn refresh(&self) -> Status {
        let _worker = self.worker.lock().await;
        if let Ok(result) = self.exchange(&[], &|| true).await {
            self.record_observation(&result).await;
        } else {
            self.state
                .lock()
                .await
                .refresh_failed(self.transport.is_some());
        }
        self.status().await
    }
    pub async fn select(&self, selection: Selection) -> Result<Status> {
        let submission = self.submission.lock().await;
        let target = self.policy().await.select(selection, self.clock.now());
        self.prepare_profile(selection).await?;
        let revision = self.persist(&target).await?;
        drop(submission);
        self.finish_dispatch(selection, revision).await
    }
    /// Current policy, rebuilt from the live status.
    async fn policy(&self) -> Policy {
        let state = self.state.lock().await;
        Policy::new(
            state.bedtime.clone(),
            state.control_source,
            state.intended,
            state.override_expires_at,
        )
    }
    async fn prepare_profile(&self, selection: Selection) -> Result<()> {
        // Capture before leaving TV Sync. Off still works if setup cannot be completed.
        if self.profile.lock().await.is_none() {
            let _worker = self.worker.lock().await;
            let captured = self.capture().await;
            if selection != (Selection::Off {}) && captured.is_err() {
                return Err(AppError::Conflict(
                    "Save a valid TV Sync profile first".into(),
                ));
            }
        }
        Ok(())
    }
    // Called with the submission lock: target, settings and override commit together.
    async fn persist(&self, target: &Target) -> Result<u64> {
        let selection = target.selection();
        let manual_override = target.manual_override();
        let intended = serde_json::to_string(&selection).map_err(anyhow::Error::from)?;
        let settings = serde_json::to_string(target.bedtime()).map_err(anyhow::Error::from)?;
        let override_json = manual_override
            .map(|value| serde_json::to_string(&value))
            .transpose()
            .map_err(anyhow::Error::from)?;
        sqlx::query!(
            "UPDATE backlight SET intended = ?, bedtime = ?, manual_override = ? WHERE id = 1",
            intended,
            settings,
            override_json
        )
        .execute(&self.db)
        .await
        .map_err(anyhow::Error::from)?;
        let mut state = self.state.lock().await;
        state.intended = selection;
        state.bedtime = target.bedtime().clone();
        state.control_source = target.control_source();
        state.override_expires_at = target.override_expires_at();
        state.next_transition = target.next_transition();
        state.target_accepted();
        Ok(self.revision.fetch_add(1, Ordering::SeqCst) + 1)
    }
    async fn finish_dispatch(&self, selection: Selection, revision: u64) -> Result<Status> {
        // Keep accepted durable work alive even if its requesting client disconnects.
        let service = self.clone();
        tokio::spawn(async move { service.dispatch(selection, revision).await })
            .await
            .map_err(anyhow::Error::from)??;
        Ok(self.status().await)
    }
    pub async fn automatic(&self) -> Result<Status> {
        let submission = self.submission.lock().await;
        let target = self.policy().await.automatic(self.clock.now());
        let selection = target.selection();
        self.prepare_profile(selection).await?;
        let revision = self.persist(&target).await?;
        drop(submission);
        self.finish_dispatch(selection, revision).await
    }
    pub async fn update_bedtime(&self, update: BedtimeUpdate) -> Result<Status> {
        if !update.supplied() {
            return Ok(self.status().await);
        }
        let submission = self.submission.lock().await;
        let target = self.policy().await.edit(update, self.clock.now())?;
        let selection = target.selection();
        self.prepare_profile(selection).await?;
        let revision = self.persist(&target).await?;
        drop(submission);
        self.finish_dispatch(selection, revision).await
    }
    /// Reevaluate current wall time without replaying historical transitions.
    /// Healthy observations do not reassert targets changed by external controllers.
    pub async fn run_routine(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut retry = RetryClock::new();
        let mut probe_at = tokio::time::Instant::now() + PROBE_INTERVAL;
        loop {
            interval.tick().await;
            if tokio::time::Instant::now() >= probe_at {
                if !self.state.lock().await.recovering() {
                    self.refresh().await;
                }
                probe_at = tokio::time::Instant::now() + PROBE_INTERVAL;
            }
            let submission = self.submission.lock().await;
            let status = self.state.lock().await.clone();
            let transition_due = status
                .next_transition
                .is_some_and(|next| self.clock.now() >= next);
            if !transition_due && !status.recovering() {
                continue;
            }
            if !retry.ready() {
                continue;
            }
            let target = Policy::new(
                status.bedtime,
                status.control_source,
                status.intended,
                status.override_expires_at,
            )
            .resolve(self.clock.now());
            let selection = target.selection();
            if let Ok(revision) = self.persist(&target).await {
                // Policy still advances when setup prevents dispatch, including override expiry.
                if selection != (Selection::Off {}) && self.profile.lock().await.is_none() {
                    self.state.lock().await.profile_required();
                    continue;
                }
                drop(submission);
                if self.finish_dispatch(selection, revision).await.is_err() {
                    tracing::warn!("Backlight recovery dispatch failed");
                }
            } else {
                self.state.lock().await.persistence_failed();
                tracing::warn!("Backlight routine could not save its target");
            }
            let retrying = self.state.lock().await.recovery == Recovery::Retrying;
            retry.attempted(retrying);
        }
    }
    async fn dispatch(&self, selection: Selection, revision: u64) -> Result<()> {
        let _worker = self.worker.lock().await;
        if self.revision.load(Ordering::SeqCst) != revision {
            return Ok(());
        }
        // A whole exchange ahead of us may have crossed the window boundary.
        // Leave the due cursor intact so the routine saves and applies only now's target.
        if self
            .state
            .lock()
            .await
            .next_transition
            .is_some_and(|next| self.clock.now() >= next)
        {
            return Ok(());
        }
        let writes = delivery::writes(selection, self.profile.lock().await.as_ref())?;
        let deadline = self.state.lock().await.next_transition;
        let revision_counter = self.revision.clone();
        let clock = self.clock.clone();
        let current = move || {
            revision_counter.load(Ordering::SeqCst) == revision
                && deadline.is_none_or(|next| clock.now() < next)
        };
        let result = self.exchange(&writes, &current).await;
        if matches!(result, Err(TransportError::Superseded)) {
            return Ok(());
        }
        if let Ok(result) = result {
            self.record_observation(&result).await;
            self.state.lock().await.dispatched(
                selection,
                result.writes_accepted,
                self.revision.load(Ordering::SeqCst) == revision,
            );
        } else {
            let mut state = self.state.lock().await;
            state.dispatch_failed(self.revision.load(Ordering::SeqCst) == revision);
            state.exchange_failed(self.transport.is_some());
        }
        Ok(())
    }
}
#[derive(Deserialize)]
struct Backup {
    schema_version: u8,
    bluetooth_address: String,
    responses: std::collections::HashMap<String, Option<String>>,
}
impl Backup {
    fn profile(&self) -> Option<TvSyncProfile> {
        if self.schema_version != 1 {
            return None;
        }
        let mut replies = Vec::new();
        for name in ["video_mode", "brightness"] {
            let hex = self.responses.get(name)?.as_ref()?;
            let bytes: Vec<u8> = hex
                .split_whitespace()
                .map(|b| u8::from_str_radix(b, 16))
                .collect::<std::result::Result<_, _>>()
                .ok()?;
            replies.push(Packet::try_from(bytes).ok()?);
        }
        TvSyncProfile::capture(&replies)
    }
}
