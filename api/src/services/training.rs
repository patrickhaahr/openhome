//! Training read model: bounded, read-only views that combine the training log (`app.db`) with the
//! NOOP push mirror (`noop.db`) for API clients such as the Hermes MCP adapter.
//!
//! This module owns every interpretation rule so clients never reimplement them:
//! - days are Europe/Copenhagen calendar days and timestamps are ISO 8601 with an offset
//!   ([`calendar`]);
//! - NOOP values are resolved across the strap's imported and on-device computed namespaces with
//!   NOOP's own precedence rules, and every selected value names the namespace it came from;
//!   workouts also include the active installation's other imported sources;
//! - freshness and coverage are derived from the receiver's push bookkeeping so a missing sync is
//!   never presented as a recorded absence.
//!
//! Training-log queries against `app.db` use the compile-time checked macros; NOOP queries use
//! runtime-checked `sqlx::query` because the compile-time macros only verify
//! against `app.db`.

pub mod calendar;
pub mod exercise_history;
mod noop_merge;
mod noop_source;
pub mod recovery;
pub mod sleep_recent;
pub mod workouts;

pub use calendar::CalendarDay;
pub use exercise_history::{ExerciseHistory, exercise_history};
pub use recovery::{RecoveryDay, recovery_day};
pub use sleep_recent::{RecentSleepNights, recent_sleep_nights};
pub use workouts::{WorkoutsOnDay, workouts_on_day};
