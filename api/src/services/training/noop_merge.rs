//! Ports of NOOP's source-precedence rules, so the API selects the same values NOOP shows.
//!
//! - Daily metrics (`mergeDaily`): imported values win field by field and computed values fill
//!   imported nulls. The sleep block (total, efficiency, stages, disturbances) is taken whole from
//!   the computed row, nulls included, when the user edited that night or when the import is a bare
//!   sleep total that a scored computed night can replace.
//! - Sleep sessions (`mergeSleepRichness`): per wake day, all imported sessions are kept unless the
//!   computed sessions carry richer staging, in which case all computed sessions are kept.
//! - Workouts (`WorkoutEditing.dedupCrossSource`): all namespaces' rows form one list. A detected
//!   bout that shadows a real session is dropped, and two rows of the same sport that overlap by
//!   more than half of the shorter one collapse to the richer row. Distinct sessions stay distinct.
//! - Journal Entries: one entry per question and day. The journal's own namespace wins, then the
//!   strap's imported and computed namespaces, then any other namespace alphabetically.
//!
//! Everything here is pure; callers pass rows for a single wake day or Training Day.

use super::noop_source::{COMPUTED_DEVICE_ID, IMPORTED_DEVICE_ID};

mod daily;
mod sleep;
mod workouts;

pub use daily::{DailyRow, ResolvedDaily, Sourced, merge_daily};
pub use sleep::{SessionRow, StageMinutes, merge_sleep_sessions, stage_minutes, stage_segments};
pub use workouts::{WorkoutOrigin, WorkoutRow, merge_workouts};

/// The NOOP device namespace a value was read from.
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

/// NOOP's namespace for answers logged in its journal.
pub const JOURNAL_DEVICE_ID: &str = "noop-journal";

/// Orders the namespaces answering one journal question: the least key wins.
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
