use crate::error::{AppError, Result};
use serde::Deserialize;
use std::time::Duration;

use super::protocol::{InactivitySettings, TvSyncProfile};
use super::types::{CurrentOperation, Exchange, Packet, ProfileSource, Status, TransportError};
use super::{BacklightService, protocol};

impl BacklightService {
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
    pub(super) async fn exchange(
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
    pub(super) async fn capture(&self) -> Result<()> {
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
    pub(super) async fn record_observation(&self, result: &Exchange) {
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
