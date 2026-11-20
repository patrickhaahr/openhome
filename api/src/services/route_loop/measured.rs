use super::plan::{SearchEnd, rounded_percent};
use super::{
    EARTH_RADIUS_METERS, MAX_CLOSURE_METERS, MAX_SHARED_SHARE, MAX_START_OFFSET_METERS,
    SAMPLE_SPACING_METERS, SHARED_WAY_METERS, TOLERANCE_PERCENT, UNSUITABLE_PERCENT,
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct MeasuredLoop {
    pub(super) distance: f64,
    pub(super) deviation_percent: f64,
    pub(super) coordinates: Vec<[f64; 2]>,
}

impl MeasuredLoop {
    pub(super) fn matches(&self) -> bool {
        rounded_percent(self.deviation_percent).abs() <= TOLERANCE_PERCENT
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Found {
    pub(super) loops: Vec<MeasuredLoop>,
    pub(super) end: SearchEnd,
}

pub(super) fn suitable(
    origin: [f64; 2],
    target: f64,
    coordinates: Vec<[f64; 2]>,
    distance: f64,
) -> Option<MeasuredLoop> {
    let (&start, &end) = (coordinates.first()?, coordinates.last()?);
    let deviation_percent = (distance - target) / target * 100.0;
    let suitable = haversine(origin, start) <= MAX_START_OFFSET_METERS
        && haversine(start, end) <= MAX_CLOSURE_METERS
        && deviation_percent.abs() <= UNSUITABLE_PERCENT;
    suitable.then_some(MeasuredLoop {
        distance,
        deviation_percent,
        coordinates,
    })
}

pub(super) fn distinct(candidate: &MeasuredLoop, kept: &MeasuredLoop) -> bool {
    let reference = candidate.coordinates[0][1].to_radians().cos();
    let project = |[longitude, latitude]: [f64; 2]| {
        [
            longitude.to_radians() * reference * EARTH_RADIUS_METERS,
            latitude.to_radians() * EARTH_RADIUS_METERS,
        ]
    };
    let samples = samples(
        &candidate
            .coordinates
            .iter()
            .copied()
            .map(project)
            .collect::<Vec<_>>(),
    );
    let kept = kept
        .coordinates
        .iter()
        .copied()
        .map(project)
        .collect::<Vec<_>>();
    let shared = samples
        .iter()
        .filter(|&&point| {
            kept.windows(2)
                .any(|pair| segment_distance(point, pair[0], pair[1]) <= SHARED_WAY_METERS)
        })
        .count();
    (shared as f64) <= samples.len() as f64 * MAX_SHARED_SHARE
}

fn samples(line: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut samples = line.first().copied().into_iter().collect::<Vec<_>>();
    let mut carried = 0.0;
    for pair in line.windows(2) {
        let [from, to] = [pair[0], pair[1]];
        let length = planar_distance(from, to);
        let mut along = SAMPLE_SPACING_METERS - carried;
        while along <= length {
            let t = along / length;
            samples.push([
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
            ]);
            along += SAMPLE_SPACING_METERS;
        }
        carried = length - (along - SAMPLE_SPACING_METERS);
    }
    samples
}

fn planar_distance(from: [f64; 2], to: [f64; 2]) -> f64 {
    (to[0] - from[0]).hypot(to[1] - from[1])
}

fn segment_distance(point: [f64; 2], from: [f64; 2], to: [f64; 2]) -> f64 {
    let [dx, dy] = [to[0] - from[0], to[1] - from[1]];
    let length_squared = dx.mul_add(dx, dy * dy);
    let t = if length_squared == 0.0 {
        0.0
    } else {
        (((point[0] - from[0]) * dx + (point[1] - from[1]) * dy) / length_squared).clamp(0.0, 1.0)
    };
    planar_distance(point, [from[0] + dx * t, from[1] + dy * t])
}

pub(super) fn haversine(from: [f64; 2], to: [f64; 2]) -> f64 {
    let [from_lon, from_lat] = from.map(f64::to_radians);
    let [to_lon, to_lat] = to.map(f64::to_radians);
    let half_chord = ((to_lat - from_lat) / 2.0).sin().powi(2)
        + from_lat.cos() * to_lat.cos() * ((to_lon - from_lon) / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_METERS * half_chord.sqrt().asin()
}
