mod bluetooth;
mod delivery;
mod device;
mod effects;
mod protocol;
pub mod routine;
mod selection;
mod types;
use crate::error::Result;
pub use bluetooth::Bluetooth;
use chrono::Utc;
pub use effects::EffectId;
pub use protocol::InactivitySettings;
use protocol::TvSyncProfile;
use routine::{BacklightOverride, BedtimeSettings, Clock, Policy, SystemClock};
use sqlx::SqlitePool;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};
use tokio::sync::Mutex;
pub use types::{
    Brightness, Command, CurrentOperation, Exchange, GattTransport, Observation, ObservedMode,
    Packet, ProfileSource, Reachability, Recovery, Rgb, Selection, Setup, Status, TransportError,
    TransportFuture,
};

const PROBE_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct BacklightService {
    db: SqlitePool,
    state: Arc<Mutex<Status>>,
    worker: Arc<Mutex<()>>,
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
}
