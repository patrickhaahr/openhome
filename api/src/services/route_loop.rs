//! Walking and running loops from the shared map origin through the household's self-hosted
//! openrouteservice.

use std::{
    collections::HashMap,
    ops::RangeInclusive,
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use url::Url;
use uuid::Uuid;

use crate::services::map::MapOrigin;

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

/// Activities the planner can route. Both use the pedestrian `foot-walking` graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Activity {
    Walking,
    Running,
}

impl Activity {
    /// Ways the loop avoids. Runners also avoid steps, which break a running pace.
    fn avoided_features(self) -> &'static [&'static str] {
        match self {
            Self::Walking => &["ferries"],
            Self::Running => &["ferries", "steps"],
        }
    }
}

/// Untrusted loop request body; parse it into a [`LoopRequest`].
#[derive(Deserialize)]
pub struct LoopRequestBody {
    activity: String,
    target: TargetBody,
    step_length_cm: f64,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum TargetBody {
    Distance { meters: f64 },
    Steps { steps: f64 },
}

/// The requested effort: a distance, or steps converted with the request's step length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LoopTarget {
    Distance { meters: u32 },
    Steps { steps: u32, meters: u32 },
}

impl LoopTarget {
    /// The distance the planner aims for.
    #[must_use]
    pub fn meters(self) -> u32 {
        match self {
            Self::Distance { meters } | Self::Steps { meters, .. } => meters,
        }
    }
}

/// A validated loop request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopRequest {
    activity: Activity,
    target: LoopTarget,
    step_length_cm: u32,
}

/// Why a loop request was rejected before contacting the provider.
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
    /// Parse the activity, step length and a distance or step target within supported ranges.
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

/// A whole number within `range`, or `None`.
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

/// Generation outcome. Expected failures are values without upstream messages or coordinates.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum LoopRead {
    Generated { plan: LoopPlan },
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
    /// Every returned loop started too far from the origin, did not close or missed the target
    /// badly.
    UnsuitableRoute,
}

/// How a search for alternatives ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchEnd {
    /// Enough distinct loops matched the target.
    Complete,
    /// Every attempt was spent before enough distinct loops matched.
    Exhausted,
    /// The provider failed or the time ran out before the search finished.
    Interrupted,
}

/// Distinct measured loops for one request, matching loops first and closest first.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LoopPlan {
    pub activity: Activity,
    pub target: LoopTarget,
    pub step_length_cm: u32,
    pub tolerance_percent: f64,
    /// How many matching alternatives the search looked for.
    pub sought: usize,
    pub search: SearchEnd,
    pub loops: Vec<GeneratedLoop>,
}

/// A measured loop. Distances come from the returned geometry, so they describe what is drawn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratedLoop {
    /// Immutable identity assigned by Axum before offering the loop for selection.
    pub id: Uuid,
    pub distance_meters: f64,
    pub deviation_meters: f64,
    pub deviation_percent: f64,
    pub within_tolerance: bool,
    /// The measured distance divided by the request's step length: an estimate.
    pub estimated_steps: u32,
    pub geometry: LineString,
}

/// Line geometry, serialized as a `LineString` in `[longitude, latitude]` order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineString {
    #[serde(rename = "type")]
    kind: String,
    pub coordinates: Vec<[f64; 2]>,
}

/// A suitable loop measured from its geometry, before it is described against a request.
#[derive(Clone, Debug, PartialEq)]
struct MeasuredLoop {
    distance: f64,
    deviation_percent: f64,
    coordinates: Vec<[f64; 2]>,
}

impl MeasuredLoop {
    /// Decided on the one-decimal percentage the dashboard shows, so the label and the verdict
    /// always agree.
    fn matches(&self) -> bool {
        rounded_percent(self.deviation_percent).abs() <= TOLERANCE_PERCENT
    }
}

/// The loops a search kept and how it ended.
#[derive(Clone, Debug, PartialEq)]
struct Found {
    loops: Vec<MeasuredLoop>,
    end: SearchEnd,
}

type CacheKey = (Activity, u32);

#[derive(Default)]
struct Cache(HashMap<CacheKey, (Instant, Found)>);

impl Cache {
    fn get(&mut self, key: CacheKey) -> Option<Found> {
        self.0.retain(|_, (stored, _)| stored.elapsed() < CACHE_TTL);
        self.0.get(&key).map(|(_, found)| found.clone())
    }

    fn put(&mut self, key: CacheKey, found: Found) {
        if self.0.len() >= CACHE_CAPACITY
            && let Some(oldest) = self
                .0
                .iter()
                .min_by_key(|(_, (stored, _))| *stored)
                .map(|(key, _)| *key)
        {
            self.0.remove(&oldest);
        }
        self.0.insert(key, (Instant::now(), found));
    }
}

#[derive(Clone)]
struct Provider {
    client: Client,
    endpoint: ProviderEndpoint,
    // One search at a time keeps repeated taps from piling work onto the household's instance.
    gate: Arc<Mutex<()>>,
    cache: Arc<std::sync::Mutex<Cache>>,
}

/// Plans loops from the shared map origin. Clones share one provider gate and cache.
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
                    .timeout(PROVIDER_TIMEOUT)
                    .redirect(reqwest::redirect::Policy::none())
                    .build()?,
                endpoint,
                gate: Arc::new(Mutex::new(())),
                cache: Arc::default(),
            }),
            None => None,
        };
        Ok(Self { origin, provider })
    }

    /// Seek distinct loops for the request, or explain why none is shown.
    pub async fn generate(&self, request: LoopRequest) -> LoopRead {
        let Some(origin) = self.origin else {
            return LoopRead::Unavailable {
                reason: LoopUnavailable::MissingOrigin,
            };
        };
        let Some(provider) = &self.provider else {
            return LoopRead::Unavailable {
                reason: LoopUnavailable::MissingProvider,
            };
        };
        match provider.search(origin.coordinates(), request).await {
            Ok(found) => LoopRead::Generated {
                plan: describe(request, found),
            },
            Err(reason) => LoopRead::Unavailable { reason },
        }
    }
}

/// Describe the kept loops against the request that asked for them.
fn describe(request: LoopRequest, found: Found) -> LoopPlan {
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

/// Identity covers both geometry and the requested effort, including stride. Cached geometry
/// can serve different targets without letting their selection snapshots overwrite each other.
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

fn rounded_percent(percent: f64) -> f64 {
    (percent * 10.0).round() / 10.0
}

/// What one round trip contributed to a search.
enum Attempt {
    Kept,
    Duplicate,
    Unsuitable,
    NoRoute,
}

impl Provider {
    fn cached(&self, key: CacheKey) -> Option<Found> {
        self.cache.lock().ok()?.get(key)
    }

    /// Request round trips with successive seeds until enough distinct loops match the target,
    /// the attempts or time run out, or the provider fails.
    async fn search(
        &self,
        origin: [f64; 2],
        request: LoopRequest,
    ) -> Result<Found, LoopUnavailable> {
        let key = (request.activity, request.target.meters());
        if let Some(found) = self.cached(key) {
            return Ok(found);
        }
        let started = Instant::now();
        let Ok(_turn) = tokio::time::timeout(SEARCH_BUDGET, self.gate.lock()).await else {
            tracing::warn!("Route provider busy with another search");
            return Err(LoopUnavailable::ProviderUnavailable);
        };
        // A request queued behind an identical one reuses its answer.
        if let Some(found) = self.cached(key) {
            return Ok(found);
        }

        let target = f64::from(request.target.meters());
        let mut kept: Vec<MeasuredLoop> = Vec::new();
        let mut overshoot: Vec<f64> = Vec::new();
        // The outcome reported if nothing is kept.
        let mut nothing_kept = LoopUnavailable::NoRoute;
        let mut end = SearchEnd::Exhausted;
        for seed in 1..=MAX_ATTEMPTS {
            if started.elapsed() >= SEARCH_BUDGET {
                tracing::warn!(seed, "Route search ran out of time");
                nothing_kept = LoopUnavailable::ProviderUnavailable;
                end = SearchEnd::Interrupted;
                break;
            }
            let length = calibrated_length(target, &overshoot);
            let attempt = match self
                .round_trip(origin, request.activity, length, seed)
                .await
            {
                Ok(trip) => {
                    let Some((coordinates, distance)) = trip.geometry() else {
                        tracing::warn!("Route provider sent an invalid loop");
                        nothing_kept = LoopUnavailable::ProviderUnavailable;
                        end = SearchEnd::Interrupted;
                        break;
                    };
                    overshoot.push(distance / f64::from(length));
                    match suitable(origin, target, coordinates, distance) {
                        Some(candidate) => keep(&mut kept, candidate),
                        None => Attempt::Unsuitable,
                    }
                }
                // Before any loop, no walkable way near the origin means no seed can start one;
                // afterwards it only concerns this seed's generated waypoints.
                Err(Refusal::NoRoute { origin_unroutable })
                    if origin_unroutable && kept.is_empty() =>
                {
                    break;
                }
                Err(Refusal::NoRoute { .. }) => Attempt::NoRoute,
                Err(Refusal::Unavailable) => {
                    nothing_kept = LoopUnavailable::ProviderUnavailable;
                    end = SearchEnd::Interrupted;
                    break;
                }
            };
            match attempt {
                Attempt::Unsuitable => {
                    tracing::warn!(seed, "Rejected a loop from the route provider");
                    nothing_kept = LoopUnavailable::UnsuitableRoute;
                }
                Attempt::Kept
                    if kept.iter().filter(|loop_| loop_.matches()).count() >= SOUGHT_LOOPS =>
                {
                    end = SearchEnd::Complete;
                    break;
                }
                Attempt::Kept | Attempt::Duplicate | Attempt::NoRoute => {}
            }
        }

        if kept.is_empty() {
            return Err(nothing_kept);
        }
        kept.sort_by(|a, b| {
            b.matches().cmp(&a.matches()).then(
                a.deviation_percent
                    .abs()
                    .total_cmp(&b.deviation_percent.abs()),
            )
        });
        kept.truncate(SOUGHT_LOOPS);
        let found = Found { loops: kept, end };
        // An interrupted search is retried next time rather than remembered.
        if end != SearchEnd::Interrupted
            && let Ok(mut cache) = self.cache.lock()
        {
            cache.put(key, found.clone());
        }
        Ok(found)
    }

    async fn round_trip(
        &self,
        origin: [f64; 2],
        activity: Activity,
        length: u32,
        seed: u32,
    ) -> Result<ProviderTrip, Refusal> {
        // The profile already prefers footways and paths. Green and quiet weightings would need
        // external green and noise datasets that a standard self-hosted build does not have.
        let body = json!({
            "coordinates": [origin],
            "preference": "recommended",
            "instructions": false,
            "elevation": false,
            "options": {
                "round_trip": { "length": length, "points": 4, "seed": seed },
                "avoid_features": activity.avoided_features()
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
                Refusal::Unavailable
            })?;
        let status = response.status();
        if status == StatusCode::OK {
            return response.json::<ProviderTrip>().await.map_err(|_| {
                tracing::warn!("Route provider sent an unreadable loop");
                Refusal::Unavailable
            });
        }
        let code = response
            .json::<ProviderError>()
            .await
            .ok()
            .map(|body| body.error.code);
        match code {
            // 2009: no route between the generated waypoints; 2010: no walkable way near one.
            Some(code @ (2009 | 2010)) if status.is_client_error() => Err(Refusal::NoRoute {
                origin_unroutable: code == 2010,
            }),
            _ => {
                tracing::warn!(
                    status = status.as_u16(),
                    code,
                    "Route provider refused a loop request"
                );
                Err(Refusal::Unavailable)
            }
        }
    }
}

/// Keep `candidate` when it mostly follows other ways than every kept loop. A matching loop also
/// replaces near-misses it retraces, so a near-miss never crowds out a match on the same streets.
fn keep(kept: &mut Vec<MeasuredLoop>, candidate: MeasuredLoop) -> Attempt {
    let retraced = |loop_: &MeasuredLoop| !distinct(&candidate, loop_);
    if !kept.iter().any(retraced) {
        kept.push(candidate);
        return Attempt::Kept;
    }
    if candidate.matches()
        && kept
            .iter()
            .filter(|loop_| retraced(loop_))
            .all(|loop_| !loop_.matches())
    {
        kept.retain(|loop_| !retraced(loop_));
        kept.push(candidate);
        return Attempt::Kept;
    }
    Attempt::Duplicate
}

/// Why the provider returned no loop for one attempt.
enum Refusal {
    NoRoute { origin_unroutable: bool },
    Unavailable,
}

/// The length to request next: the target, corrected by how much earlier loops overshot.
fn calibrated_length(target: f64, overshoot: &[f64]) -> u32 {
    let ratio = if overshoot.is_empty() {
        1.0
    } else {
        overshoot.iter().sum::<f64>() / overshoot.len() as f64
    };
    let share = (1.0 / ratio).clamp(*CALIBRATION.start(), *CALIBRATION.end());
    #[expect(
        clippy::cast_sign_loss,
        reason = "a positive target scaled by a positive share"
    )]
    let length = (target * share).round() as u32;
    length
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
    /// The loop's coordinates and its length measured from them, or `None` when the response
    /// is not one plausible line.
    fn geometry(self) -> Option<(Vec<[f64; 2]>, f64)> {
        let [feature] = <[ProviderFeature; 1]>::try_from(self.features).ok()?;
        if feature.geometry.kind != "LineString"
            || !(2..=MAX_POINTS).contains(&feature.geometry.coordinates.len())
        {
            return None;
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
                    Some([*longitude, *latitude])
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let distance = coordinates
            .windows(2)
            .map(|pair| haversine(pair[0], pair[1]))
            .sum::<f64>();
        let summary = feature.properties.summary.distance;
        let agrees = summary.is_finite()
            && distance > 0.0
            && (summary - distance).abs() <= distance * SUMMARY_AGREEMENT_PERCENT / 100.0;
        agrees.then_some((coordinates, distance))
    }
}

/// The loop when it starts at the origin, closes and is near enough to the target to show.
fn suitable(
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

/// Whether `candidate` mostly follows ways other than `kept`'s. The comparison is one-sided:
/// a short loop inside a longer kept one adds nothing, while a longer loop around it does.
fn distinct(candidate: &MeasuredLoop, kept: &MeasuredLoop) -> bool {
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

/// Points every [`SAMPLE_SPACING_METERS`] along a projected line, starting at its first point.
fn samples(line: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut samples = line.first().copied().into_iter().collect::<Vec<_>>();
    // Distance travelled since the last sample; always below the spacing.
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

/// Distance from `point` to the segment between `from` and `to`, in projected metres.
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
