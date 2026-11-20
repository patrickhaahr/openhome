use crate::error::{AppError, Result};
use std::{sync::atomic::Ordering, time::Duration};

use super::delivery::RetryClock;
use super::routine::{BedtimeUpdate, Policy, Target};
use super::types::{Recovery, Selection, Status, TransportError};
use super::{BacklightService, PROBE_INTERVAL, delivery};

impl BacklightService {
    pub async fn select(&self, selection: Selection) -> Result<Status> {
        let submission = self.submission.lock().await;
        let target = self.policy().await.select(selection, self.clock.now());
        self.prepare_profile(selection).await?;
        let revision = self.persist(&target).await?;
        drop(submission);
        self.finish_dispatch(selection, revision).await
    }
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
    pub(super) async fn persist(&self, target: &Target) -> Result<u64> {
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
