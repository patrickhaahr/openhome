//! Walking loops from the shared map origin through openrouteservice's free round-trip routing.

use std::{fmt, sync::Arc, time::Duration};

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

/// Server-held openrouteservice key; never serialized, logged or debug-printed.
#[derive(Clone)]
pub struct ProviderKey(String);

impl ProviderKey {
    /// A blank key is absent configuration, not a credential.
    #[must_use]
    pub fn parse(key: Option<&str>) -> Option<Self> {
        key.map(str::trim)
            .filter(|key| !key.is_empty())
            .map(|key| Self(key.to_string()))
    }
}

impl fmt::Debug for ProviderKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProviderKey([redacted])")
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
    /// No openrouteservice key is configured.
    MissingProvider,
    /// The provider rejected the configured key.
    ProviderRejected,
    /// The free per-minute or daily quota is used up.
    QuotaExceeded,
    /// The provider was unreachable, failed or answered with an invalid response.
    ProviderUnavailable,
    /// No walkable loop could be found from the origin.
    NoRoute,
    /// The returned loop does not start at the origin, does not close or misses the target badly.
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
    endpoint: Url,
    key: ProviderKey,
    // One provider request at a time keeps repeated taps inside the free per-minute quota.
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
        key: Option<ProviderKey>,
        endpoint: Url,
    ) -> anyhow::Result<Self> {
        let provider = match key {
            Some(key) => Some(Provider {
                client: Client::builder()
                    .timeout(Duration::from_secs(15))
                    .redirect(reqwest::redirect::Policy::none())
                    .build()?,
                endpoint,
                key,
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
                Err(reason) => unavailable(reason),
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
        let body = json!({
            "coordinates": [origin],
            "preference": "recommended",
            "instructions": false,
            "elevation": false,
            "options": {
                "round_trip": { "length": request.target_meters, "points": 4, "seed": 1 },
                "avoid_features": ["ferries"],
                // Prefer green areas and quiet ways, where the provider has that data.
                "profile_params": { "weightings": {
                    "green": { "factor": 1.0 },
                    "quiet": { "factor": 0.5 }
                } }
            }
        });
        // Raw request errors can include the origin-bearing body. Never log them.
        let response = self
            .client
            .post(self.endpoint.clone())
            .header(reqwest::header::AUTHORIZATION, &self.key.0)
            .header(reqwest::header::ACCEPT, "application/geo+json")
            .json(&body)
            .send()
            .await
            .map_err(|_| LoopUnavailable::ProviderUnavailable)?;
        match response.status() {
            StatusCode::OK => response
                .json::<ProviderTrip>()
                .await
                .map_err(|_| LoopUnavailable::ProviderUnavailable),
            // 429 is the per-minute limit. The gateway also answers a spent daily quota with 403,
            // naming the quota in its body; any other 403 rejects the key.
            StatusCode::TOO_MANY_REQUESTS => Err(LoopUnavailable::QuotaExceeded),
            StatusCode::FORBIDDEN => match response.text().await {
                Ok(text) if text.to_ascii_lowercase().contains("quota") => {
                    Err(LoopUnavailable::QuotaExceeded)
                }
                _ => Err(LoopUnavailable::ProviderRejected),
            },
            StatusCode::UNAUTHORIZED => Err(LoopUnavailable::ProviderRejected),
            status if status.is_client_error() => {
                // 2009: no route between locations; 2010: no walkable way near the origin.
                let code = response
                    .json::<ProviderError>()
                    .await
                    .ok()
                    .map(|body| body.error.code);
                match code {
                    Some(2009 | 2010) => Err(LoopUnavailable::NoRoute),
                    _ => Err(LoopUnavailable::ProviderUnavailable),
                }
            }
            _ => Err(LoopUnavailable::ProviderUnavailable),
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
    fn redacts_the_provider_key() {
        let key = ProviderKey::parse(Some(" synthetic-secret ")).unwrap();
        assert!(!format!("{key:?}").contains("synthetic-secret"));
        assert!(ProviderKey::parse(Some("  ")).is_none());
    }
}
