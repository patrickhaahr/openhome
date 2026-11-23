use std::{ops::RangeInclusive, time::Duration};

use url::Url;

mod measured;
mod plan;
mod planner;
mod request;

pub use plan::{GeneratedLoop, LineString, LoopPlan, LoopRead, LoopUnavailable, SearchEnd};
pub use planner::LoopPlanner;
pub use request::{Activity, LoopRequest, LoopRequestBody, LoopRequestError, LoopTarget};

const TARGET_METERS: RangeInclusive<u32> = 1_000..=25_000;

const TARGET_STEPS: RangeInclusive<u32> = 1..=100_000;

const STEP_LENGTH_CM: RangeInclusive<u32> = 40..=150;

const TOLERANCE_PERCENT: f64 = 10.0;

const UNSUITABLE_PERCENT: f64 = 50.0;

const MAX_START_OFFSET_METERS: f64 = 400.0;

const MAX_CLOSURE_METERS: f64 = 50.0;

const SUMMARY_AGREEMENT_PERCENT: f64 = 10.0;

const MAX_POINTS: usize = 20_000;

const SOUGHT_LOOPS: usize = 3;

const MAX_ATTEMPTS: u32 = 6;

const SEARCH_BUDGET: Duration = Duration::from_secs(20);

const PROVIDER_TIMEOUT: Duration = Duration::from_secs(10);

const CALIBRATION: RangeInclusive<f64> = 0.6..=1.4;

const SHARED_WAY_METERS: f64 = 60.0;

const MAX_SHARED_SHARE: f64 = 0.7;

const SAMPLE_SPACING_METERS: f64 = 100.0;

const CACHE_TTL: Duration = Duration::from_mins(30);

const CACHE_CAPACITY: usize = 32;

const EARTH_RADIUS_METERS: f64 = 6_371_008.8;

const DIRECTIONS_PATH: &str = "/v2/directions/foot-walking/geojson";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderEndpoint(Url);

impl ProviderEndpoint {
    pub fn parse(base: Option<&str>) -> anyhow::Result<Option<Self>> {
        let Some(base) = base.map(str::trim).filter(|base| !base.is_empty()) else {
            return Ok(None);
        };
        let mut url =
            Url::parse(base).map_err(|_| anyhow::anyhow!("Invalid OPENROUTESERVICE_URL"))?;
        anyhow::ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "Invalid OPENROUTESERVICE_URL"
        );
        let path = format!("{}{DIRECTIONS_PATH}", url.path().trim_end_matches('/'));
        url.set_path(&path);
        Ok(Some(Self(url)))
    }
}

#[cfg(test)]
mod tests {
    use super::measured::{MeasuredLoop, distinct, haversine};
    use super::planner::calibrated_length;
    use super::request::TargetBody;
    use super::*;

    fn body(activity: &str, target: TargetBody, step_length_cm: f64) -> LoopRequestBody {
        LoopRequestBody {
            activity: activity.to_string(),
            target,
            step_length_cm,
        }
    }

    fn distance(meters: f64) -> TargetBody {
        TargetBody::Distance { meters }
    }

    fn steps(steps: f64) -> TargetBody {
        TargetBody::Steps { steps }
    }

    #[test]
    fn parses_activities_and_whole_metre_distances_within_range() {
        assert_eq!(
            LoopRequest::parse(&body("running", distance(10_000.0), 100.0)),
            Ok(LoopRequest {
                activity: Activity::Running,
                target: LoopTarget::Distance { meters: 10_000 },
                step_length_cm: 100,
            })
        );
        assert_eq!(
            LoopRequest::parse(&body("cycling", distance(5000.0), 75.0)),
            Err(LoopRequestError::UnsupportedActivity)
        );
        for meters in [999.0, 25_001.0, 5000.5, f64::NAN, f64::INFINITY, -5000.0] {
            assert_eq!(
                LoopRequest::parse(&body("walking", distance(meters), 75.0)),
                Err(LoopRequestError::InvalidDistance)
            );
        }
    }

    const CM_PER_METER: u32 = 100;
    const WALKING_STEP_CM: u32 = 75;

    fn parse_walking_steps(
        count: f64,
        step_length_cm: f64,
    ) -> Result<LoopRequest, LoopRequestError> {
        LoopRequest::parse(&body("walking", steps(count), step_length_cm))
    }

    #[test]
    fn converts_ten_thousand_steps_of_the_requested_step_length_to_meters() {
        const STEPS: u32 = 10_000;
        assert_eq!(
            parse_walking_steps(f64::from(STEPS), f64::from(WALKING_STEP_CM)),
            Ok(LoopRequest {
                activity: Activity::Walking,
                target: LoopTarget::Steps {
                    steps: STEPS,
                    meters: STEPS * WALKING_STEP_CM / CM_PER_METER
                },
                step_length_cm: WALKING_STEP_CM,
            })
        );
    }

    #[test]
    fn rounds_converted_step_distances_to_whole_meters() {
        const STEPS: u32 = 3333;
        const STEP_LENGTH_CM: u32 = 77;
        assert_eq!(
            parse_walking_steps(f64::from(STEPS), f64::from(STEP_LENGTH_CM))
                .map(|request| request.target.meters()),
            Ok(STEPS * STEP_LENGTH_CM / CM_PER_METER)
        );
    }

    #[test]
    fn rejects_steps_that_convert_outside_the_target_distance() {
        const STEPS_BELOW_MINIMUM_DISTANCE: f64 = 1000.0;
        const STEPS_ABOVE_MAXIMUM_DISTANCE: f64 = 40_000.0;
        for count in [STEPS_BELOW_MINIMUM_DISTANCE, STEPS_ABOVE_MAXIMUM_DISTANCE] {
            assert_eq!(
                parse_walking_steps(count, f64::from(WALKING_STEP_CM)),
                Err(LoopRequestError::InvalidSteps)
            );
        }
    }

    #[test]
    fn rejects_fractional_empty_and_non_finite_step_counts() {
        for count in [10_000.5, 0.0, f64::NAN] {
            assert_eq!(
                parse_walking_steps(count, f64::from(WALKING_STEP_CM)),
                Err(LoopRequestError::InvalidSteps)
            );
        }
    }

    #[test]
    fn rejects_step_lengths_outside_the_range_and_fractional_ones() {
        let just_below = f64::from(*STEP_LENGTH_CM.start()) - 1.0;
        let just_above = f64::from(*STEP_LENGTH_CM.end()) + 1.0;
        for length in [just_below, just_above, 75.5, f64::NAN] {
            assert_eq!(
                parse_walking_steps(10_000.0, length),
                Err(LoopRequestError::InvalidStepLength)
            );
        }
    }

    const ORIGIN: [f64; 2] = [12.5692, 55.6869];
    const BLOCK_SIDE_DEGREES: f64 = 0.01;
    const SMALL_BLOCK_SIDE_DEGREES: f64 = 0.004;
    const METERS_PER_DEGREE_LATITUDE: f64 = 111_195.0;
    const DISTANCE_TOLERANCE_METERS: f64 = 1.0;
    const NUDGE_METERS: f64 = SHARED_WAY_METERS / 3.0;
    const EASTWARD: f64 = 1.0;
    const WESTWARD: f64 = -1.0;

    #[test]
    fn measures_the_distance_of_a_block_of_latitude_on_the_earth_sphere() {
        let north = [ORIGIN[0], ORIGIN[1] + BLOCK_SIDE_DEGREES];
        let distance = haversine(ORIGIN, north);
        let expected = BLOCK_SIDE_DEGREES * METERS_PER_DEGREE_LATITUDE;
        assert!(
            (distance - expected).abs() < DISTANCE_TOLERANCE_METERS,
            "{distance}"
        );
    }

    fn square(direction: f64, side: f64) -> MeasuredLoop {
        let [lon, lat] = ORIGIN;
        let east = direction * side / lat.to_radians().cos();
        let corners = [
            [lon, lat],
            [lon, lat + side],
            [lon + east, lat + side],
            [lon + east, lat],
            [lon, lat],
        ];
        let coordinates = corners
            .windows(2)
            .flat_map(|pair| {
                (0..20).map(move |step| {
                    let t = f64::from(step) / 20.0;
                    [
                        pair[0][0] + (pair[1][0] - pair[0][0]) * t,
                        pair[0][1] + (pair[1][1] - pair[0][1]) * t,
                    ]
                })
            })
            .chain([[lon, lat]])
            .collect();
        MeasuredLoop {
            distance: 0.0,
            deviation_percent: 0.0,
            coordinates,
        }
    }

    #[test]
    fn a_loop_is_not_distinct_from_itself() {
        let east = square(EASTWARD, BLOCK_SIDE_DEGREES);
        assert!(!distinct(&east, &east));
    }

    #[test]
    fn a_loop_nudged_within_the_shared_way_distance_follows_the_same_ways() {
        let east = square(EASTWARD, BLOCK_SIDE_DEGREES);
        let mut nudged = east.clone();
        for point in &mut nudged.coordinates {
            point[1] += NUDGE_METERS / METERS_PER_DEGREE_LATITUDE;
        }
        assert!(!distinct(&nudged, &east));
    }

    #[test]
    fn a_loop_to_the_west_shares_only_the_northbound_street_from_the_origin() {
        let east = square(EASTWARD, BLOCK_SIDE_DEGREES);
        assert!(distinct(&square(WESTWARD, BLOCK_SIDE_DEGREES), &east));
    }

    #[test]
    fn a_larger_loop_around_a_small_one_in_its_corner_adds_new_ways() {
        let east = square(EASTWARD, BLOCK_SIDE_DEGREES);
        let small = square(EASTWARD, SMALL_BLOCK_SIDE_DEGREES);
        assert!(distinct(&east, &small));
    }

    const CALIBRATION_TARGET_METERS: f64 = 5000.0;
    const ROUNDING_TOLERANCE_METERS: f64 = 0.5;

    fn assert_requested_length(overshoot: &[f64], expected: f64) {
        let requested = f64::from(calibrated_length(CALIBRATION_TARGET_METERS, overshoot));
        assert!(
            (requested - expected).abs() <= ROUNDING_TOLERANCE_METERS,
            "{requested} != {expected}"
        );
    }

    #[test]
    fn requests_the_target_length_when_no_loop_overshot_yet() {
        assert_requested_length(&[], CALIBRATION_TARGET_METERS);
    }

    #[test]
    fn a_loop_a_quarter_too_long_asks_for_four_fifths_of_the_target() {
        const A_QUARTER_TOO_LONG: f64 = 1.25;
        assert_requested_length(
            &[A_QUARTER_TOO_LONG],
            CALIBRATION_TARGET_METERS / A_QUARTER_TOO_LONG,
        );
    }

    #[test]
    fn a_wildly_long_loop_is_not_chased_below_the_calibration_floor() {
        const WILDLY_TOO_LONG: f64 = 3.0;
        assert_requested_length(
            &[WILDLY_TOO_LONG],
            CALIBRATION_TARGET_METERS * CALIBRATION.start(),
        );
    }

    #[test]
    fn derives_the_directions_endpoint_from_a_plain_instance_url() {
        for base in [
            "http://openrouteservice:8082/ors",
            "http://openrouteservice:8082/ors/",
        ] {
            assert_eq!(
                ProviderEndpoint::parse(Some(base))
                    .unwrap()
                    .unwrap()
                    .0
                    .as_str(),
                "http://openrouteservice:8082/ors/v2/directions/foot-walking/geojson"
            );
        }
        assert_eq!(ProviderEndpoint::parse(Some("  ")).unwrap(), None);
        assert_eq!(ProviderEndpoint::parse(None).unwrap(), None);
        for base in [
            "synthetic-secret",
            "ftp://synthetic-secret/ors",
            "http://user:synthetic-secret@host/ors",
            "http://host/ors?key=synthetic-secret",
            "http://host/ors#synthetic-secret",
        ] {
            let error = ProviderEndpoint::parse(Some(base)).unwrap_err().to_string();
            assert!(!error.contains("synthetic-secret"), "{error}");
        }
    }
}
