use super::noop_source::{COMPUTED_DEVICE_ID, IMPORTED_DEVICE_ID};

mod daily;
mod sleep;
mod workouts;

pub use daily::{DailyRow, ResolvedDaily, Sourced, merge_daily};
pub use sleep::{SessionRow, StageMinutes, merge_sleep_sessions, stage_minutes, stage_segments};
pub use workouts::{WorkoutOrigin, WorkoutRow, merge_workouts};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Namespace {
    Imported,
    Computed,
}

impl Namespace {
    pub fn from_device_id(device_id: &str) -> Option<Self> {
        match device_id {
            IMPORTED_DEVICE_ID => Some(Self::Imported),
            COMPUTED_DEVICE_ID => Some(Self::Computed),
            _ => None,
        }
    }

    pub const fn device_id(self) -> &'static str {
        match self {
            Self::Imported => IMPORTED_DEVICE_ID,
            Self::Computed => COMPUTED_DEVICE_ID,
        }
    }
}

pub const JOURNAL_DEVICE_ID: &str = "noop-journal";

pub fn journal_precedence(device_id: &str) -> (u8, &str) {
    let rank = match device_id {
        JOURNAL_DEVICE_ID => 0,
        IMPORTED_DEVICE_ID => 1,
        COMPUTED_DEVICE_ID => 2,
        _ => 3,
    };
    (rank, device_id)
}

#[cfg(test)]
mod tests;
