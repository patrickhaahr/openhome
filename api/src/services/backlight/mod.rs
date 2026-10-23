//! Local Backlight control. Private packets and identifiers never enter the HTTP contract.
mod bluetooth;
mod protocol;
pub mod routine;
use crate::error::{AppError, Result};
pub use bluetooth::Bluetooth;
use chrono::{DateTime, Utc};
use protocol::{TvSyncProfile, frame};
use routine::{
    BacklightOverride, BedtimeSettings, BedtimeUpdate, Clock, ControlSource, SystemClock, evaluate,
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

pub type Packet = [u8; 20];
pub struct Exchange {
    pub writes_accepted: bool,
    pub replies: Vec<Packet>,
}
#[derive(Debug)]
pub struct TransportError;
pub type TransportFuture<'a> =
    Pin<Box<dyn Future<Output = std::result::Result<Exchange, TransportError>> + Send + 'a>>;
/// One complete GATT operation: ordered writes followed by correlated status queries.
pub trait GattTransport: Send + Sync {
    /// Verify a private backup belongs to the configured controller.
    fn matches_device(&self, identity: &str) -> bool;
    fn exchange<'a>(&'a self, writes: &'a [Packet], queries: &'a [Vec<u8>]) -> TransportFuture<'a>;
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
    pub setup: &'static str,
    pub reachability: &'static str,
    pub intended: Selection,
    pub acknowledged: Option<Selection>,
    pub observed: Option<Observation>,
    pub stale: bool,
    pub command: &'static str,
    pub bedtime: BedtimeSettings,
    pub control_source: ControlSource,
    pub override_expires_at: Option<DateTime<Utc>>,
    pub next_transition: Option<DateTime<Utc>>,
}
#[derive(Clone, Serialize)]
pub struct Observation {
    pub power: bool,
    pub brightness: u8,
    pub mode: &'static str,
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
        let intended = serde_json::from_str(&row.intended).map_err(anyhow::Error::from)?;
        let bedtime: BedtimeSettings =
            serde_json::from_str(&row.bedtime).map_err(anyhow::Error::from)?;
        let manual_override: Option<BacklightOverride> = row
            .manual_override
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map_err(anyhow::Error::from)?;
        let next_transition = evaluate(&bedtime, clock.now()).next_transition;
        Ok(Self {
            db,
            state: Arc::new(Mutex::new(Status {
                setup: if profile.is_some() {
                    "ready"
                } else {
                    "incomplete"
                },
                reachability: if transport.is_some() {
                    "unknown"
                } else {
                    "unconfigured"
                },
                intended,
                acknowledged: None,
                observed: None,
                stale: true,
                command: "idle",
                bedtime,
                control_source: if manual_override.is_some() {
                    ControlSource::Manual
                } else {
                    ControlSource::Automatic
                },
                override_expires_at: manual_override.and_then(|o| o.expires_at),
                next_transition,
            })),
            worker: Arc::new(Mutex::new(())),
            submission: Arc::new(Mutex::new(())),
            revision: Arc::new(AtomicU64::new(0)),
            transport,
            backup,
            profile: Arc::new(Mutex::new(profile)),
            clock,
        })
    }
    pub async fn status(&self) -> Status {
        let mut status = self.state.lock().await.clone();
        status.stale = status.reachability != "reachable"
            || status
                .observed
                .as_ref()
                .is_none_or(|o| Utc::now() - o.observed_at > chrono::Duration::seconds(30));
        status
    }
    async fn exchange(&self, writes: &[Packet]) -> std::result::Result<Exchange, TransportError> {
        let Some(transport) = &self.transport else {
            return Err(TransportError);
        };
        let queries = vec![vec![0xaa, 1], vec![0xaa, 4], vec![0xaa, 5, 1]];
        tokio::time::timeout(
            Duration::from_secs(30),
            transport.exchange(writes, &queries),
        )
        .await
        .unwrap_or(Err(TransportError))
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
        self.state.lock().await.setup = "ready";
        Ok(())
    }
    async fn capture(&self) -> Result<()> {
        let result = self
            .exchange(&[])
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
        let mut state = self.state.lock().await;
        state.reachability = if observed.is_some() {
            "reachable"
        } else {
            "unreachable"
        };
        if let Some(observed) = observed {
            state.observed = Some(observed);
        }
    }
    pub async fn refresh(&self) -> Status {
        let _worker = self.worker.lock().await;
        match self.exchange(&[]).await {
            Ok(result) => self.record_observation(&result).await,
            Err(_) => {
                self.state.lock().await.reachability = if self.transport.is_some() {
                    "unreachable"
                } else {
                    "unconfigured"
                }
            }
        }
        self.status().await
    }
    pub async fn select(&self, selection: Selection) -> Result<Status> {
        let submission = self.submission.lock().await;
        let bedtime = self.state.lock().await.bedtime.clone();
        let evaluation = evaluate(&bedtime, self.clock.now());
        let manual_override = Some(BacklightOverride {
            selection,
            expires_at: evaluation.next_transition,
        });
        let revision = self
            .persist(
                selection,
                bedtime,
                manual_override,
                evaluation.next_transition,
            )
            .await?;
        drop(submission);
        self.finish_dispatch(selection, revision).await
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
    async fn persist(
        &self,
        selection: Selection,
        bedtime: BedtimeSettings,
        manual_override: Option<BacklightOverride>,
        next_transition: Option<DateTime<Utc>>,
    ) -> Result<u64> {
        self.prepare_profile(selection).await?;
        let intended = serde_json::to_string(&selection).map_err(anyhow::Error::from)?;
        let settings = serde_json::to_string(&bedtime).map_err(anyhow::Error::from)?;
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
        state.bedtime = bedtime;
        state.control_source = if manual_override.is_some() {
            ControlSource::Manual
        } else {
            ControlSource::Automatic
        };
        state.override_expires_at = manual_override.and_then(|o| o.expires_at);
        state.next_transition = next_transition;
        state.command = "pending";
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
        let bedtime = self.state.lock().await.bedtime.clone();
        let evaluation = evaluate(&bedtime, self.clock.now());
        let selection = evaluation.selection;
        let revision = self
            .persist(selection, bedtime, None, evaluation.next_transition)
            .await?;
        drop(submission);
        self.finish_dispatch(selection, revision).await
    }
    pub async fn update_bedtime(&self, update: BedtimeUpdate) -> Result<Status> {
        use crate::services::patch::Patch;
        let enabled = match update.enabled {
            Patch::Missing => return Ok(self.status().await),
            Patch::Value(Some(enabled)) => enabled,
            Patch::Value(None) => {
                return Err(AppError::Validation("Enabled cannot be null".into()));
            }
        };
        let submission = self.submission.lock().await;
        let mut bedtime = self.state.lock().await.bedtime.clone();
        bedtime.enabled = enabled;
        let evaluation = evaluate(&bedtime, self.clock.now());
        let selection = evaluation.selection;
        let revision = self
            .persist(selection, bedtime, None, evaluation.next_transition)
            .await?;
        drop(submission);
        self.finish_dispatch(selection, revision).await
    }
    /// Reevaluate current wall time without replaying historical transitions.
    /// The runtime cursor makes uninterrupted operation leave external changes alone.
    pub async fn run_routine(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let submission = self.submission.lock().await;
            let status = self.state.lock().await.clone();
            if status
                .next_transition
                .is_none_or(|next| self.clock.now() < next)
            {
                continue;
            }
            let evaluation = evaluate(&status.bedtime, self.clock.now());
            let selection = evaluation.selection;
            if let Ok(revision) = self
                .persist(selection, status.bedtime, None, evaluation.next_transition)
                .await
            {
                drop(submission);
                if self.finish_dispatch(selection, revision).await.is_err() {
                    tracing::warn!("Backlight routine dispatch failed");
                }
            } else {
                // Recovery/retry is owned by the device worker; do not loop writes.
                self.state.lock().await.next_transition = evaluation.next_transition;
                tracing::warn!("Backlight routine could not save its target");
            }
        }
    }
    async fn dispatch(&self, selection: Selection, revision: u64) -> Result<()> {
        let _worker = self.worker.lock().await;
        if self.revision.load(Ordering::SeqCst) != revision {
            return Ok(());
        }
        let writes = match selection {
            Selection::Off {} => vec![frame(&[0x33, 1, 0])],
            Selection::TvSync { brightness } => self
                .profile
                .lock()
                .await
                .as_ref()
                .ok_or_else(|| AppError::Conflict("Save a valid TV Sync profile first".into()))?
                .writes(brightness.map(u8::from)),
            Selection::Color { rgb, brightness } => vec![
                frame(&[0x33, 1, 1]),
                // H605C requires the RGBIC whole-strip variant; generic RGB can ACK without applying.
                frame(&[
                    0x33, 5, 0x15, 1, rgb.red, rgb.green, rgb.blue, 0, 0, 0, 0, 0, 0xff, 0x7f,
                ]),
                frame(&[0x33, 4, brightness.into()]),
            ],
        };
        if let Ok(result) = self.exchange(&writes).await {
            self.record_observation(&result).await;
            let mut state = self.state.lock().await;
            if result.writes_accepted {
                state.acknowledged = Some(selection);
            }
            if self.revision.load(Ordering::SeqCst) == revision {
                state.command = if result.writes_accepted {
                    "acknowledged"
                } else {
                    "failed"
                };
            }
        } else {
            let mut state = self.state.lock().await;
            if self.revision.load(Ordering::SeqCst) == revision {
                state.command = "failed";
            }
            state.reachability = if self.transport.is_some() {
                "unreachable"
            } else {
                "unconfigured"
            };
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
