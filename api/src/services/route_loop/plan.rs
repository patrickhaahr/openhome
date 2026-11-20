use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::measured::Found;
use super::request::{Activity, LoopRequest, LoopTarget};
use super::{SOUGHT_LOOPS, TOLERANCE_PERCENT};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum LoopRead {
    Generated { plan: LoopPlan },
    Unavailable { reason: LoopUnavailable },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoopUnavailable {
    MissingOrigin,
    MissingProvider,
    ProviderUnavailable,
    NoRoute,
    UnsuitableRoute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchEnd {
    Complete,
    Exhausted,
    Interrupted,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LoopPlan {
    pub activity: Activity,
    pub target: LoopTarget,
    pub step_length_cm: u32,
    pub tolerance_percent: f64,
    pub sought: usize,
    pub search: SearchEnd,
    pub loops: Vec<GeneratedLoop>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratedLoop {
    pub id: Uuid,
    pub distance_meters: f64,
    pub deviation_meters: f64,
    pub deviation_percent: f64,
    pub within_tolerance: bool,
    pub estimated_steps: u32,
    pub geometry: LineString,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineString {
    #[serde(rename = "type")]
    kind: String,
    pub coordinates: Vec<[f64; 2]>,
}

pub(super) fn describe(request: LoopRequest, found: Found) -> LoopPlan {
    let target = f64::from(request.target.meters());
    let step_length = f64::from(request.step_length_cm) / 100.0;
    let loops = found
        .loops
        .into_iter()
        .map(|measured| {
            let deviation = measured.distance - target;
            #[expect(
                clippy::cast_sign_loss,
                reason = "a measured loop is a positive distance of at most a few hundred \
                          thousand steps"
            )]
            let estimated_steps = (measured.distance / step_length).round() as u32;
            GeneratedLoop {
                id: candidate_id(request, &measured.coordinates),
                distance_meters: measured.distance.round(),
                deviation_meters: deviation.round(),
                deviation_percent: rounded_percent(measured.deviation_percent),
                within_tolerance: measured.matches(),
                estimated_steps,
                geometry: LineString {
                    kind: "LineString".to_string(),
                    coordinates: measured.coordinates,
                },
            }
        })
        .collect();
    LoopPlan {
        activity: request.activity,
        target: request.target,
        step_length_cm: request.step_length_cm,
        tolerance_percent: TOLERANCE_PERCENT,
        sought: SOUGHT_LOOPS,
        search: found.end,
        loops,
    }
}

fn candidate_id(request: LoopRequest, coordinates: &[[f64; 2]]) -> Uuid {
    let mut hash = Sha256::new();
    hash.update(b"openhome-loop-v1");
    hash.update([match request.activity {
        Activity::Walking => 0,
        Activity::Running => 1,
    }]);
    match request.target {
        LoopTarget::Distance { meters } => {
            hash.update([0]);
            hash.update(meters.to_le_bytes());
        }
        LoopTarget::Steps { steps, meters } => {
            hash.update([1]);
            hash.update(steps.to_le_bytes());
            hash.update(meters.to_le_bytes());
        }
    }
    hash.update(request.step_length_cm.to_le_bytes());
    for coordinate in coordinates {
        for value in coordinate {
            hash.update(value.to_le_bytes());
        }
    }
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&hash.finalize()[..16]);
    uuid::Builder::from_random_bytes(bytes).into_uuid()
}

pub(super) fn rounded_percent(percent: f64) -> f64 {
    (percent * 10.0).round() / 10.0
}
