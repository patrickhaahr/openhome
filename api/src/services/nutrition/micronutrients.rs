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

pub const TRAILING_DAYS: u64 = 7;
pub const MIN_CUT_DOWN_DAYS: usize = 3;
const EPA: &str = "epa";
const DHA: &str = "dha";
const RETINOL: &str = "retinol";

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
