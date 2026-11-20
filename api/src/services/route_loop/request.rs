use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use super::{STEP_LENGTH_CM, TARGET_METERS, TARGET_STEPS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Activity {
    Walking,
    Running,
}

impl Activity {
    pub(super) fn avoided_features(self) -> &'static [&'static str] {
        match self {
            Self::Walking => &["ferries"],
            Self::Running => &["ferries", "steps"],
        }
    }
}

#[derive(Deserialize)]
pub struct LoopRequestBody {
    pub(super) activity: String,
    pub(super) target: TargetBody,
    pub(super) step_length_cm: f64,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(super) enum TargetBody {
    Distance { meters: f64 },
    Steps { steps: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LoopTarget {
    Distance { meters: u32 },
    Steps { steps: u32, meters: u32 },
}

impl LoopTarget {
    #[must_use]
    pub fn meters(self) -> u32 {
        match self {
            Self::Distance { meters } | Self::Steps { meters, .. } => meters,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopRequest {
    pub(super) activity: Activity,
    pub(super) target: LoopTarget,
    pub(super) step_length_cm: u32,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum LoopRequestError {
    #[error("Unsupported activity; choose walking or running")]
    UnsupportedActivity,
    #[error("Step length must be a whole number of centimetres between 40 and 150")]
    InvalidStepLength,
    #[error("Target distance must be between 1 km and 25 km, in whole metres")]
    InvalidDistance,
    #[error("Step target must be a whole number of steps covering between 1 km and 25 km")]
    InvalidSteps,
}

impl LoopRequest {
    pub fn parse(body: &LoopRequestBody) -> Result<Self, LoopRequestError> {
        let activity = match body.activity.as_str() {
            "walking" => Activity::Walking,
            "running" => Activity::Running,
            _ => return Err(LoopRequestError::UnsupportedActivity),
        };
        let step_length_cm = whole(body.step_length_cm, &STEP_LENGTH_CM)
            .ok_or(LoopRequestError::InvalidStepLength)?;
        let target = match body.target {
            TargetBody::Distance { meters } => LoopTarget::Distance {
                meters: whole(meters, &TARGET_METERS).ok_or(LoopRequestError::InvalidDistance)?,
            },
            TargetBody::Steps { steps } => {
                let steps = whole(steps, &TARGET_STEPS).ok_or(LoopRequestError::InvalidSteps)?;
                let meters = (u64::from(steps) * u64::from(step_length_cm) + 50) / 100;
                let meters = u32::try_from(meters)
                    .ok()
                    .filter(|meters| TARGET_METERS.contains(meters))
                    .ok_or(LoopRequestError::InvalidSteps)?;
                LoopTarget::Steps { steps, meters }
            }
        };
        Ok(Self {
            activity,
            target,
            step_length_cm,
        })
    }
}

fn whole(value: f64, range: &RangeInclusive<u32>) -> Option<u32> {
    let within = value.is_finite()
        && value.fract() == 0.0
        && (f64::from(*range.start())..=f64::from(*range.end())).contains(&value);
    #[expect(
        clippy::cast_sign_loss,
        reason = "only used when the value is a whole number within a non-negative u32 range"
    )]
    within.then_some(value as u32)
}
