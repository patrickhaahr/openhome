//! Micronutrient evaluation (`CONTEXT.md`, Recommended Intake, Upper Limit): each vitamin and
//! mineral the day supplied, as a percentage of its NNR 2023 Recommended Intake for the Profile's
//! sex and age band, and checked against its EFSA Upper Limits.
//!
//! - The age band comes from the Profile's birth date and the day evaluated, so a birthday moves
//!   later days to the next band. Without the Profile's sex or birth date, or below 18 years, the
//!   panel is unavailable with the reason; nothing is guessed.
//! - Reaching or passing a Recommended Intake is good and is never flagged.
//! - An Upper Limit is checked against the share of intake it applies to ([`Share`]): food and
//!   supplements, or only the supplement share or one chemical form. A day above it is a
//!   **caution**. A **cut down** applies when the trailing 7-day average is above it: the mean
//!   over the days with at least one Meal Log among that day and the 6 before it, and only when
//!   at least [`MIN_CUT_DOWN_DAYS`] of them know the amount, so a day or two above the limit stay
//!   a caution rather than a habit. Days without Meal Logs are left out rather than counted as
//!   zero, as they are for the Estimated Expenditure, so an untracked day never dilutes a
//!   habitual excess. A logged day without
//!   Supplement Doses of the nutrient adds zero to a supplement-only share; a logged day whose
//!   food value is unknown is left out of a share that includes food, because unknown is never
//!   zero. Cut down outranks caution. Nutrients without an Upper Limit carry no status at all.
//! - EPA+DHA adds food and supplements against EFSA's adequate intake of 250 mg. Compounds
//!   without a database parameter (creatine) have no reference; the Nutrition Day lists them as
//!   amounts taken.
//!
//! [`Diary`] holds every logged day's intake of the panel's nutrients and evaluates any of its
//! days, so a range is loaded once and each day is evaluated from it: as the Nutrition Day's
//! panel ([`Diary::evaluate`]), or unrounded for the Nutrition Summary to add up
//! ([`Diary::assess`]).

use self::references::{Measure, REFERENCES};

mod days;
mod diary;
mod status;
pub use days::Person;
pub(super) use days::{DayAssessment, NutrientDay};
use days::{DayEaten, DayIntake, LimitDay};
pub use diary::Diary;
pub(super) use diary::unavailable_on;
pub use status::{
    Citation, LimitStatus, Micronutrients, NutrientStatus, Panel, SOURCES, TrailingWindow,
    Unavailable, UpperLimitStatus,
};

pub mod references;

/// Days the trailing average spans, ending with the day evaluated.
pub const TRAILING_DAYS: u64 = 7;
/// The fewest logged days with a known amount in the trailing window whose average above an
/// Upper Limit says cut down; fewer days above it are only a caution.
pub const MIN_CUT_DOWN_DAYS: usize = 3;
const EPA: &str = "epa";
const DHA: &str = "dha";
/// Preformed vitamin A in food.
const RETINOL: &str = "retinol";

/// Every Portion value the panel reads: its parameters, EPA and DHA, and food retinol.
/// Supplement Doses always carry every amount, chemical forms included.
pub(super) fn value_keys() -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = REFERENCES
        .iter()
        .filter_map(|reference| match reference.measure {
            Measure::Parameter(key) => Some(key),
            Measure::EpaDha => None,
        })
        .chain([EPA, DHA, RETINOL])
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}
