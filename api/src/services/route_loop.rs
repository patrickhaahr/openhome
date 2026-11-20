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

    #[test]
    fn converts_step_targets_with_the_requested_step_length() {
        assert_eq!(
            LoopRequest::parse(&body("walking", steps(10_000.0), 75.0)),
            Ok(LoopRequest {
                activity: Activity::Walking,
                target: LoopTarget::Steps {
                    steps: 10_000,
                    meters: 7500
                },
                step_length_cm: 75,
            })
        );
        assert_eq!(
            LoopRequest::parse(&body("walking", steps(3333.0), 77.0)).map(|r| r.target.meters()),
            Ok(2566)
        );
        for count in [1000.0, 40_000.0, 10_000.5, 0.0, f64::NAN] {
            assert_eq!(
                LoopRequest::parse(&body("walking", steps(count), 75.0)),
                Err(LoopRequestError::InvalidSteps)
            );
        }
        for length in [39.0, 151.0, 75.5, f64::NAN] {
            assert_eq!(
                LoopRequest::parse(&body("walking", steps(10_000.0), length)),
                Err(LoopRequestError::InvalidStepLength)
            );
        }
    }

    #[test]
    fn measures_great_circle_distances() {
        let distance = haversine([12.5692, 55.6869], [12.5692, 55.6969]);
        assert!((distance - 1112.0).abs() < 1.0, "{distance}");
    }

    fn square(east: f64, side: f64) -> MeasuredLoop {
        let [lon, lat]: [f64; 2] = [12.5692, 55.6869];
        let east = east * side / lat.to_radians().cos();
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
    fn tells_loops_on_other_ways_from_retraced_ones() {
        let east = square(1.0, 0.01);
        assert!(!distinct(&east, &east));
        let mut nudged = east.clone();
        for point in &mut nudged.coordinates {
            point[1] += 0.00018;
        }
        assert!(!distinct(&nudged, &east));
        assert!(distinct(&square(-1.0, 0.01), &east));
        let small = square(1.0, 0.004);
        assert!(distinct(&east, &small));
    }

    #[test]
    fn calibrates_later_requests_by_earlier_overshoot() {
        assert_eq!(calibrated_length(5000.0, &[]), 5000);
        assert_eq!(calibrated_length(5000.0, &[1.25]), 4000);
        assert_eq!(calibrated_length(5000.0, &[3.0]), 3000);
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
