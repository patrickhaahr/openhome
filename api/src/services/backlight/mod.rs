//! Local Backlight control. Private packets and identifiers never enter the HTTP contract.
mod bluetooth;
mod protocol;
use crate::error::{AppError, Result};
pub use bluetooth::Bluetooth;
use chrono::{DateTime, Utc};
use protocol::{TvSyncProfile, frame};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc, time::Duration};
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
    Off,
    TvSync,
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
    // All capture/read/write exchanges share this lock, including persistence before writes.
    worker: Arc<Mutex<()>>,
    transport: Option<Arc<dyn GattTransport>>,
    backup: Option<PathBuf>,
    profile: Arc<Mutex<Option<TvSyncProfile>>>,
}
impl BacklightService {
    pub async fn new(
        db: SqlitePool,
        transport: Option<Arc<dyn GattTransport>>,
        backup: Option<PathBuf>,
    ) -> Result<Self> {
        let row = sqlx::query!("SELECT tv_sync_profile, intended FROM backlight WHERE id = 1")
            .fetch_one(&db)
            .await
            .map_err(anyhow::Error::from)?;
        let profile = row
            .tv_sync_profile
            .and_then(|json| serde_json::from_str::<TvSyncProfile>(&json).ok())
            .and_then(TvSyncProfile::validate);
        let intended = serde_json::from_str(&row.intended).map_err(anyhow::Error::from)?;
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
            })),
            worker: Arc::new(Mutex::new(())),
            transport,
            backup,
            profile: Arc::new(Mutex::new(profile)),
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
        let _worker = self.worker.lock().await;
        // Capture before leaving TV Sync. Off still works if setup cannot be completed.
        if self.profile.lock().await.is_none() {
            let captured = self.capture().await;
            if selection != Selection::Off && captured.is_err() {
                return Err(AppError::Conflict(
                    "Save a valid TV Sync profile first".into(),
                ));
            }
        }
        let writes = match selection {
            Selection::Off => vec![frame(&[0x33, 1, 0])],
            Selection::TvSync => self
                .profile
                .lock()
                .await
                .as_ref()
                .ok_or_else(|| AppError::Conflict("Save a valid TV Sync profile first".into()))?
                .writes(),
        };
        let intended = serde_json::to_string(&selection).map_err(anyhow::Error::from)?;
        sqlx::query!("UPDATE backlight SET intended = ? WHERE id = 1", intended)
            .execute(&self.db)
            .await
            .map_err(anyhow::Error::from)?;
        {
            let mut state = self.state.lock().await;
            state.intended = selection;
            state.command = "pending";
        }
        if let Ok(result) = self.exchange(&writes).await {
            self.record_observation(&result).await;
            let mut state = self.state.lock().await;
            if result.writes_accepted {
                state.acknowledged = Some(selection);
                state.command = "acknowledged";
            } else {
                state.command = "failed";
            }
        } else {
            let mut state = self.state.lock().await;
            state.command = "failed";
            state.reachability = if self.transport.is_some() {
                "unreachable"
            } else {
                "unconfigured"
            };
        }
        Ok(self.status().await)
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
