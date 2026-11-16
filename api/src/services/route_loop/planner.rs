use std::{collections::HashMap, sync::Arc, time::Instant};

use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::services::map::MapOrigin;

use super::measured::{Found, MeasuredLoop, distinct, haversine, suitable};
use super::plan::{LoopRead, LoopUnavailable, SearchEnd, describe};
use super::request::{Activity, LoopRequest};
use super::{
    CACHE_CAPACITY, CACHE_TTL, CALIBRATION, MAX_ATTEMPTS, MAX_POINTS, PROVIDER_TIMEOUT,
    ProviderEndpoint, SEARCH_BUDGET, SOUGHT_LOOPS, SUMMARY_AGREEMENT_PERCENT,
};

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

/// What one round trip contributed to a search.
enum Attempt {
    Kept,
    Duplicate,
    Unsuitable,
    NoRoute,
}

/// Accumulation survives individual refusals; termination is decided by the search loop.
struct SearchProgress {
    kept: Vec<MeasuredLoop>,
    overshoot: Vec<f64>,
    empty_reason: LoopUnavailable,
}

impl SearchProgress {
    fn new() -> Self {
        Self {
            kept: Vec::new(),
            overshoot: Vec::new(),
            empty_reason: LoopUnavailable::NoRoute,
        }
    }

    fn has_enough_matches(&self) -> bool {
        self.kept.iter().filter(|loop_| loop_.matches()).count() >= SOUGHT_LOOPS
    }

    fn finish(mut self, end: SearchEnd) -> Result<Found, LoopUnavailable> {
        if self.kept.is_empty() {
            return Err(if end == SearchEnd::Interrupted {
                LoopUnavailable::ProviderUnavailable
            } else {
                self.empty_reason
            });
        }
        self.kept.sort_by(|a, b| {
            b.matches().cmp(&a.matches()).then(
                a.deviation_percent
                    .abs()
                    .total_cmp(&b.deviation_percent.abs()),
            )
        });
        self.kept.truncate(SOUGHT_LOOPS);
        Ok(Found {
            loops: self.kept,
            end,
        })
    }
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
        let mut progress = SearchProgress::new();
        let end = 'search: {
            for seed in 1..=MAX_ATTEMPTS {
                if started.elapsed() >= SEARCH_BUDGET {
                    tracing::warn!(seed, "Route search ran out of time");
                    break 'search SearchEnd::Interrupted;
                }
                let length = calibrated_length(target, &progress.overshoot);
                let attempt = match self
                    .round_trip(origin, request.activity, length, seed)
                    .await
                {
                    Ok(trip) => {
                        let Some((coordinates, distance)) = trip.geometry() else {
                            tracing::warn!("Route provider sent an invalid loop");
                            break 'search SearchEnd::Interrupted;
                        };
                        progress.overshoot.push(distance / f64::from(length));
                        match suitable(origin, target, coordinates, distance) {
                            Some(candidate) => keep(&mut progress.kept, candidate),
                            None => Attempt::Unsuitable,
                        }
                    }
                    // Before any loop, no walkable way near the origin means no seed can start
                    // one; afterwards it only concerns this seed's generated waypoints.
                    Err(Refusal::NoRoute { origin_unroutable })
                        if origin_unroutable && progress.kept.is_empty() =>
                    {
                        // Preserve a preceding unsuitable verdict, if any.
                        return Err(progress.empty_reason);
                    }
                    Err(Refusal::NoRoute { .. }) => Attempt::NoRoute,
                    Err(Refusal::Unavailable) => {
                        break 'search SearchEnd::Interrupted;
                    }
                };
                match attempt {
                    Attempt::Unsuitable => {
                        tracing::warn!(seed, "Rejected a loop from the route provider");
                        progress.empty_reason = LoopUnavailable::UnsuitableRoute;
                    }
                    Attempt::Kept if progress.has_enough_matches() => {
                        break 'search SearchEnd::Complete;
                    }
                    Attempt::Kept | Attempt::Duplicate | Attempt::NoRoute => {}
                }
            }
            SearchEnd::Exhausted
        };
        let found = progress.finish(end)?;
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
pub(super) fn calibrated_length(target: f64, overshoot: &[f64]) -> u32 {
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
