//! Walking loops from the shared map origin through the household's self-hosted openrouteservice.

use std::{sync::Arc, time::Duration};

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use url::Url;

use crate::services::map::MapOrigin;

/// Shortest and longest loop the planner requests.
const MIN_TARGET_METERS: u32 = 1_000;
const MAX_TARGET_METERS: u32 = 25_000;

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

/// Activities the planner can route. Running arrives with its own profile choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Activity {
    Walking,
}

/// Untrusted loop request body; parse it into a [`LoopRequest`].
#[derive(Deserialize)]
pub struct LoopRequestBody {
    activity: String,
    target_meters: f64,
}

/// A validated loop request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopRequest {
    activity: Activity,
    target_meters: u32,
}

/// Why a loop request was rejected before contacting the provider.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum LoopRequestError {
    #[error("Unsupported activity; only walking loops are available")]
    UnsupportedActivity,
    #[error("Target distance must be a whole number of metres between 1000 and 25000")]
    InvalidTarget,
}

impl LoopRequest {
    /// Parse the activity and a whole-metre target within the supported range.
    pub fn parse(body: &LoopRequestBody) -> Result<Self, LoopRequestError> {
        let activity = match body.activity.as_str() {
            "walking" => Activity::Walking,
            _ => return Err(LoopRequestError::UnsupportedActivity),
        };
        let target = body.target_meters;
        if !target.is_finite()
            || target.fract() != 0.0
            || !(f64::from(MIN_TARGET_METERS)..=f64::from(MAX_TARGET_METERS)).contains(&target)
        {
            return Err(LoopRequestError::InvalidTarget);
        }
        #[expect(
            clippy::cast_sign_loss,
            reason = "the target is a whole number within the positive supported range"
        )]
        let target_meters = target as u32;
        Ok(Self {
            activity,
            target_meters,
        })
    }
}

/// Generation outcome. Expected failures are values without upstream messages or coordinates.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum LoopRead {
    Generated { route: WalkingLoop },
    Unavailable { reason: LoopUnavailable },
}

/// Expected reasons no loop is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoopUnavailable {
    /// The shared map origin is not configured.
    MissingOrigin,
    /// No openrouteservice instance is configured.
    MissingProvider,
    /// The instance was unreachable, failed, refused the request or answered invalidly.
    ProviderUnavailable,
    /// No walkable loop could be found from the origin.
    NoRoute,
    /// The returned loop starts too far from the origin, does not close or misses the target badly.
    UnsuitableRoute,
}

/// A measured loop. Distances come from the returned geometry, so they describe what is drawn.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WalkingLoop {
    pub activity: Activity,
    pub target_meters: u32,
    pub distance_meters: f64,
    pub deviation_meters: f64,
    pub deviation_percent: f64,
    pub tolerance_percent: f64,
    pub within_tolerance: bool,
    pub geometry: LineString,
}

/// Line geometry, serialized as a `LineString` in `[longitude, latitude]` order.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LineString {
    #[serde(rename = "type")]
    kind: &'static str,
    pub coordinates: Vec<[f64; 2]>,
}

#[derive(Clone)]
struct Provider {
    client: Client,
    endpoint: ProviderEndpoint,
    // One request at a time keeps repeated taps from piling work onto the household's instance.
    gate: Arc<Mutex<()>>,
}

/// Plans loops from the shared map origin. Clones share one provider gate.
#[derive(Clone)]
pub struct LoopPlanner {
    origin: Option<MapOrigin>,
    provider: Option<Provider>,
}

impl LoopPlanner {
    /// The endpoint is supplied by bootstrap, allowing a controlled provider at the HTTP seam.
    pub fn new(
        origin: Option<MapOrigin>,
        endpoint: Option<ProviderEndpoint>,
    ) -> anyhow::Result<Self> {
        let provider = match endpoint {
            Some(endpoint) => Some(Provider {
                client: Client::builder()
                    .timeout(Duration::from_secs(15))
                    .redirect(reqwest::redirect::Policy::none())
                    .build()?,
                endpoint,
                gate: Arc::new(Mutex::new(())),
            }),
            None => None,
        };
        Ok(Self { origin, provider })
    }

    /// Generate one loop through the provider, or explain why none is shown.
    pub async fn generate(&self, request: LoopRequest) -> LoopRead {
        let Some(origin) = self.origin else {
            return unavailable(LoopUnavailable::MissingOrigin);
        };
        let Some(provider) = &self.provider else {
            return unavailable(LoopUnavailable::MissingProvider);
        };
        match provider.round_trip(origin.coordinates(), request).await {
            Ok(trip) => match trip.measure(origin.coordinates(), request) {
                Ok(route) => LoopRead::Generated { route },
                Err(reason) => {
                    tracing::warn!(?reason, "Rejected a loop from the route provider");
                    unavailable(reason)
                }
            },
            Err(reason) => unavailable(reason),
        }
    }
}

fn unavailable(reason: LoopUnavailable) -> LoopRead {
    LoopRead::Unavailable { reason }
}

impl Provider {
    async fn round_trip(
        &self,
        origin: [f64; 2],
        request: LoopRequest,
    ) -> Result<ProviderTrip, LoopUnavailable> {
        let _turn = self.gate.lock().await;
        // The profile already prefers footways and paths. Green and quiet weightings would need
        // external green and noise datasets that a standard self-hosted build does not have.
        let body = json!({
            "coordinates": [origin],
            "preference": "recommended",
            "instructions": false,
            "elevation": false,
            "options": {
                "round_trip": { "length": request.target_meters, "points": 4, "seed": 1 },
                "avoid_features": ["ferries"]
            }
        });
        // Logs carry status, provider code and error kind only: never the origin-bearing body.
        let response = self
            .client
            .post(self.endpoint.0.clone())
            .header(reqwest::header::ACCEPT, "application/geo+json")
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                tracing::warn!(
                    timeout = error.is_timeout(),
                    connect = error.is_connect(),
                    "Route provider unreachable"
                );
                LoopUnavailable::ProviderUnavailable
            })?;
        let status = response.status();
        if status == StatusCode::OK {
            return response.json::<ProviderTrip>().await.map_err(|_| {
                tracing::warn!("Route provider sent an unreadable loop");
                LoopUnavailable::ProviderUnavailable
            });
        }
        let code = response
            .json::<ProviderError>()
            .await
            .ok()
            .map(|body| body.error.code);
        match code {
            // 2009: no route between locations; 2010: no walkable way near the origin.
            Some(2009 | 2010) if status.is_client_error() => Err(LoopUnavailable::NoRoute),
            _ => {
                tracing::warn!(
                    status = status.as_u16(),
                    code,
                    "Route provider refused a loop request"
                );
                Err(LoopUnavailable::ProviderUnavailable)
            }
        }
    }
}

#[derive(Deserialize)]
struct ProviderError {
    error: ProviderErrorBody,
}

#[derive(Deserialize)]
struct ProviderErrorBody {
    code: u32,
}

#[derive(Deserialize)]
struct ProviderTrip {
    features: Vec<ProviderFeature>,
}

#[derive(Deserialize)]
struct ProviderFeature {
    geometry: ProviderGeometry,
    properties: ProviderProperties,
}

#[derive(Deserialize)]
struct ProviderGeometry {
    #[serde(rename = "type")]
    kind: String,
    coordinates: Vec<Vec<f64>>,
}

#[derive(Deserialize)]
struct ProviderProperties {
    summary: ProviderSummary,
}

#[derive(Deserialize)]
struct ProviderSummary {
    distance: f64,
}

impl ProviderTrip {
    /// Validate the provider's loop and measure it from its geometry.
    fn measure(
        self,
        origin: [f64; 2],
        request: LoopRequest,
    ) -> Result<WalkingLoop, LoopUnavailable> {
        let [feature] = <[ProviderFeature; 1]>::try_from(self.features)
            .map_err(|_| LoopUnavailable::ProviderUnavailable)?;
        if feature.geometry.kind != "LineString"
            || !(2..=MAX_POINTS).contains(&feature.geometry.coordinates.len())
        {
            return Err(LoopUnavailable::ProviderUnavailable);
        }
        let coordinates = feature
            .geometry
            .coordinates
            .iter()
            .map(|point| match point.as_slice() {
                // Elevation, when present, is a third value and is not displayed.
                [longitude, latitude, ..]
                    if longitude.is_finite()
                        && latitude.is_finite()
                        && (-180.0..=180.0).contains(longitude)
                        && (-90.0..=90.0).contains(latitude) =>
                {
                    Ok([*longitude, *latitude])
                }
                _ => Err(LoopUnavailable::ProviderUnavailable),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let distance = coordinates
            .windows(2)
            .map(|pair| haversine(pair[0], pair[1]))
            .sum::<f64>();
        let summary = feature.properties.summary.distance;
        if !summary.is_finite()
            || distance <= 0.0
            || (summary - distance).abs() > distance * SUMMARY_AGREEMENT_PERCENT / 100.0
        {
            return Err(LoopUnavailable::ProviderUnavailable);
        }
        let (Some(&start), Some(&end)) = (coordinates.first(), coordinates.last()) else {
            return Err(LoopUnavailable::ProviderUnavailable);
        };
        let target = f64::from(request.target_meters);
        let deviation = distance - target;
        let deviation_percent = deviation / target * 100.0;
        if haversine(origin, start) > MAX_START_OFFSET_METERS
            || haversine(start, end) > MAX_CLOSURE_METERS
            || deviation_percent.abs() > UNSUITABLE_PERCENT
        {
            return Err(LoopUnavailable::UnsuitableRoute);
        }
        Ok(WalkingLoop {
            activity: request.activity,
            target_meters: request.target_meters,
            distance_meters: distance.round(),
            deviation_meters: deviation.round(),
            deviation_percent: (deviation_percent * 10.0).round() / 10.0,
            tolerance_percent: TOLERANCE_PERCENT,
            within_tolerance: deviation_percent.abs() <= TOLERANCE_PERCENT,
            geometry: LineString {
                kind: "LineString",
                coordinates,
            },
        })
    }
}

/// Great-circle distance in metres between two `[longitude, latitude]` points.
fn haversine(from: [f64; 2], to: [f64; 2]) -> f64 {
    let [from_lon, from_lat] = from.map(f64::to_radians);
    let [to_lon, to_lat] = to.map(f64::to_radians);
    let half_chord = ((to_lat - from_lat) / 2.0).sin().powi(2)
        + from_lat.cos() * to_lat.cos() * ((to_lon - from_lon) / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_METERS * half_chord.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(activity: &str, target_meters: f64) -> LoopRequestBody {
        LoopRequestBody {
            activity: activity.to_string(),
            target_meters,
        }
    }

    #[test]
    fn parses_walking_targets_in_whole_metres_within_range() {
        assert_eq!(
            LoopRequest::parse(&body("walking", 5000.0)),
            Ok(LoopRequest {
                activity: Activity::Walking,
                target_meters: 5000
            })
        );
        assert_eq!(
            LoopRequest::parse(&body("running", 5000.0)),
            Err(LoopRequestError::UnsupportedActivity)
        );
        for target in [999.0, 25_001.0, 5000.5, f64::NAN, f64::INFINITY, -5000.0] {
            assert_eq!(
                LoopRequest::parse(&body("walking", target)),
                Err(LoopRequestError::InvalidTarget)
            );
        }
    }

    #[test]
    fn measures_great_circle_distances() {
        // One hundredth of a degree of latitude is about 1112 m everywhere.
        let distance = haversine([12.5692, 55.6869], [12.5692, 55.6969]);
        assert!((distance - 1112.0).abs() < 1.0, "{distance}");
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
