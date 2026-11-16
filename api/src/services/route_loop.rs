//! Walking and running loops from the shared map origin through the household's self-hosted
//! openrouteservice.

use std::{ops::RangeInclusive, time::Duration};

use url::Url;

mod measured;
mod plan;
mod planner;
mod request;

pub use plan::{GeneratedLoop, LineString, LoopPlan, LoopRead, LoopUnavailable, SearchEnd};
pub use planner::LoopPlanner;
pub use request::{Activity, LoopRequest, LoopRequestBody, LoopRequestError, LoopTarget};

/// Shortest and longest loop the planner requests.
const TARGET_METERS: RangeInclusive<u32> = 1_000..=25_000;

/// Step targets are whole steps; the converted distance must still fit [`TARGET_METERS`].
const TARGET_STEPS: RangeInclusive<u32> = 1..=100_000;

/// Step lengths from a short walking step to a long running stride.
const STEP_LENGTH_CM: RangeInclusive<u32> = 40..=150;

/// A loop within this share of the target matches it; anything else is labeled as a deviation.
const TOLERANCE_PERCENT: f64 = 10.0;

/// Beyond this share the provider did not plan the requested effort, so the loop is unsuitable.
const UNSUITABLE_PERCENT: f64 = 50.0;

/// The loop must start this close to the origin; openrouteservice snaps to walkable ways
/// within 350 m.
const MAX_START_OFFSET_METERS: f64 = 400.0;

/// The geometry must end where it started to count as a loop.
const MAX_CLOSURE_METERS: f64 = 50.0;

/// The provider's own summary must agree with the returned geometry within this share.
const SUMMARY_AGREEMENT_PERCENT: f64 = 10.0;

const MAX_POINTS: usize = 20_000;

/// Matching alternatives a search looks for before it stops.
const SOUGHT_LOOPS: usize = 3;

/// Round trips one search may request, each with its own seed.
const MAX_ATTEMPTS: u32 = 6;

/// A search, including its wait behind another search, starts no new round trip after this
/// long, so the dashboard is answered promptly.
const SEARCH_BUDGET: Duration = Duration::from_secs(20);

const PROVIDER_TIMEOUT: Duration = Duration::from_secs(10);

/// Later attempts ask for a length corrected by how far earlier loops overshot, within these
/// shares of the target.
const CALIBRATION: RangeInclusive<f64> = 0.6..=1.4;

/// Two loops share a way where they pass within this distance of each other.
const SHARED_WAY_METERS: f64 = 60.0;

/// A loop is a distinct alternative only when at most this share of it follows a kept loop.
const MAX_SHARED_SHARE: f64 = 0.7;

/// Spacing of the points compared between loops.
const SAMPLE_SPACING_METERS: f64 = 100.0;

/// Completed searches are reused for this long; the same seeds plan the same loops.
const CACHE_TTL: Duration = Duration::from_mins(30);

const CACHE_CAPACITY: usize = 32;

const EARTH_RADIUS_METERS: f64 = 6_371_008.8;

/// Path of the foot-walking directions service below an instance's base URL.
const DIRECTIONS_PATH: &str = "/v2/directions/foot-walking/geojson";

/// Foot-walking directions endpoint of the household's own openrouteservice instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderEndpoint(Url);

impl ProviderEndpoint {
    /// Parse an instance base URL such as `http://openrouteservice:8082/ors`. Only plain HTTP(S)
    /// without credentials, query or fragment is accepted; errors never echo the input.
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
        // 3333 steps of 77 cm is 2566.41 m.
        assert_eq!(
            LoopRequest::parse(&body("walking", steps(3333.0), 77.0)).map(|r| r.target.meters()),
            Ok(2566)
        );
        // 1000 steps of 75 cm is only 750 m; 40 000 steps of 75 cm is 30 km.
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
        // One hundredth of a degree of latitude is about 1112 m everywhere.
        let distance = haversine([12.5692, 55.6869], [12.5692, 55.6969]);
        assert!((distance - 1112.0).abs() < 1.0, "{distance}");
    }

    /// A closed square from the origin, `side` degrees of latitude north and the same distance
    /// east or west, sampled densely.
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
        // The same square nudged 20 m north still follows the same ways.
        let mut nudged = east.clone();
        for point in &mut nudged.coordinates {
            point[1] += 0.00018;
        }
        assert!(!distinct(&nudged, &east));
        // A square to the west shares only the northbound street from the origin.
        assert!(distinct(&square(-1.0, 0.01), &east));
        // A larger loop around a small one in its corner adds new ways.
        let small = square(1.0, 0.004);
        assert!(distinct(&east, &small));
    }

    #[test]
    fn calibrates_later_requests_by_earlier_overshoot() {
        assert_eq!(calibrated_length(5000.0, &[]), 5000);
        assert_eq!(calibrated_length(5000.0, &[1.25]), 4000);
        // A wildly long provider is not chased below 60% of the target.
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
