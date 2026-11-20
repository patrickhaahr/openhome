use super::{
    Command, Observation, Packet, Reachability, Recovery, Selection, Setup, Status,
    protocol::{TvSyncProfile, frame},
};
use crate::error::{AppError, Result};
use std::time::Duration;
use tokio::time::Instant;

impl Status {
    pub(super) fn target_accepted(&mut self) {
        self.command = Command::Pending;
        self.recovery = Recovery::Pending;
    }
    pub(super) fn observation_recorded(
        &mut self,
        observation: Option<Observation>,
        reconnected: bool,
    ) {
        if observation.is_none() {
            self.recovery = Recovery::Retrying;
        } else if reconnected {
            self.recovery = Recovery::Pending;
        }
        self.reachability = if observation.is_some() {
            Reachability::Reachable
        } else {
            Reachability::Unreachable
        };
        if let Some(observation) = observation {
            self.observed = Some(observation);
        }
    }
    pub(super) fn exchange_failed(&mut self, configured: bool) {
        self.reachability = if configured {
            Reachability::Unreachable
        } else {
            Reachability::Unconfigured
        };
    }
    pub(super) fn refresh_failed(&mut self, configured: bool) {
        self.recovery = Recovery::Retrying;
        self.exchange_failed(configured);
    }
    pub(super) fn dispatched(
        &mut self,
        selection: Selection,
        writes_accepted: bool,
        current: bool,
    ) {
        if writes_accepted {
            self.acknowledged = Some(selection);
        }
        if !current {
            return;
        }
        self.command = if writes_accepted {
            Command::Acknowledged
        } else {
            Command::Failed
        };
        if writes_accepted && self.reachability == Reachability::Reachable {
            if self.recovery != Recovery::Idle {
                self.recovery = Recovery::Recovered;
            }
        } else {
            self.recovery = Recovery::Retrying;
        }
    }
    pub(super) fn dispatch_failed(&mut self, current: bool) {
        if current {
            self.command = Command::Failed;
            self.recovery = Recovery::Retrying;
        }
    }
    pub(super) fn profile_required(&mut self) {
        self.recovery = Recovery::ProfileRequired;
        self.command = Command::Failed;
    }
    pub(super) fn profile_stored(&mut self) {
        self.setup = Setup::Ready;
        if self.recovery == Recovery::ProfileRequired {
            self.recovery = Recovery::Pending;
        }
    }
    pub(super) fn persistence_failed(&mut self) {
        self.recovery = Recovery::Retrying;
        self.command = Command::Failed;
    }
    pub(super) fn recovering(&self) -> bool {
        matches!(self.recovery, Recovery::Pending | Recovery::Retrying)
    }
}

pub(super) struct RetryClock {
    retry_at: Instant,
    delay: Duration,
}
impl RetryClock {
    const INITIAL: Duration = Duration::from_secs(1);
    const CAP: Duration = Duration::from_secs(30);

    pub(super) fn new() -> Self {
        Self {
            retry_at: Instant::now(),
            delay: Self::INITIAL,
        }
    }
    pub(super) fn ready(&self) -> bool {
        Instant::now() >= self.retry_at
    }
    pub(super) fn attempted(&mut self, retrying: bool) {
        if retrying {
            self.retry_at = Instant::now() + self.delay;
            self.delay = (self.delay * 2).min(Self::CAP);
        } else {
            self.retry_at = Instant::now();
            self.delay = Self::INITIAL;
        }
    }
}

pub(super) fn writes(selection: Selection, profile: Option<&TvSyncProfile>) -> Result<Vec<Packet>> {
    Ok(match selection {
        Selection::Effect { effect, brightness } => effect.writes(brightness.into()),
        Selection::Off {} => vec![frame(&[0x33, 1, 0])],
        Selection::TvSync { brightness } => profile
            .ok_or_else(|| AppError::Conflict("Save a valid TV Sync profile first".into()))?
            .writes(brightness.map(u8::from)),
        Selection::Color { rgb, brightness } => vec![
            frame(&[0x33, 1, 1]),
            frame(&[
                0x33, 5, 0x15, 1, rgb.red, rgb.green, rgb.blue, 0, 0, 0, 0, 0, 0xff, 0x7f,
            ]),
            frame(&[0x33, 4, brightness.into()]),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn retry_gaps_double_up_to_the_cap_and_reset_without_recovery() {
        let mut clock = RetryClock::new();
        assert!(clock.ready(), "the first attempt never waits");
        for gap in [
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4),
            Duration::from_secs(8),
            Duration::from_secs(16),
            Duration::from_secs(30),
            Duration::from_secs(30),
        ] {
            clock.attempted(true);
            assert!(
                !clock.ready(),
                "a retrying attempt schedules its doubled gap"
            );
            tokio::time::advance(gap.checked_sub(Duration::from_millis(1)).unwrap()).await;
            assert!(!clock.ready());
            tokio::time::advance(Duration::from_millis(1)).await;
            assert!(clock.ready(), "after exactly its gap the retry runs");
        }
        clock.attempted(false);
        assert!(clock.ready(), "a non-retrying attempt runs immediately");
        clock.attempted(true);
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(clock.ready(), "the doubled gap resets to one second");
    }
}
