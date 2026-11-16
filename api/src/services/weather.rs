//! Private Open-Meteo integration and bounded, shared forecast cache.

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Client;
use serde::Serialize;
use tokio::sync::Mutex;
use url::Url;

mod provider;

use provider::ProviderForecast;

const TTL: chrono::Duration = chrono::Duration::minutes(15);
const RETRY: chrono::Duration = chrono::Duration::minutes(1);
const MAX_STALE: chrono::Duration = chrono::Duration::hours(6);

/// Validated private forecast coordinates; never serialized or logged.
#[derive(Clone)]
pub struct WeatherLocation {
    latitude: f64,
    longitude: f64,
}

impl WeatherLocation {
    /// Both coordinates must be supplied together, with finite geographic values.
    pub fn parse(latitude: Option<&str>, longitude: Option<&str>) -> anyhow::Result<Option<Self>> {
        match (latitude, longitude) {
            (None, None) => Ok(None),
            (Some(lat), Some(lon)) => {
                let latitude: f64 = lat
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Invalid WEATHER_LATITUDE"))?;
                let longitude: f64 = lon
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Invalid WEATHER_LONGITUDE"))?;
                anyhow::ensure!(
                    latitude.is_finite() && (-90.0..=90.0).contains(&latitude),
                    "Invalid WEATHER_LATITUDE"
                );
                anyhow::ensure!(
                    longitude.is_finite() && (-180.0..=180.0).contains(&longitude),
                    "Invalid WEATHER_LONGITUDE"
                );
                Ok(Some(Self {
                    latitude,
                    longitude,
                }))
            }
            _ => anyhow::bail!("Set both WEATHER_LATITUDE and WEATHER_LONGITUDE"),
        }
    }
}

/// Sanitized weather read. Stale data retains its original fetch and validity times.
#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum WeatherRead {
    Unavailable { reason: WeatherUnavailable },
    Fresh { forecast: Forecast },
    Stale { forecast: Forecast },
}

/// Expected failures, without upstream messages or private location details.
#[derive(Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WeatherUnavailable {
    MissingConfiguration,
    ProviderUnavailable,
}

/// Client contract: Celsius, metres per second, millimetres and Copenhagen calendar dates.
#[derive(Clone, Serialize)]
pub struct Forecast {
    pub fetched_at: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    pub timezone: &'static str,
    pub current: Option<CurrentConditions>,
    pub hourly: Vec<ForecastHour>,
    pub days: Vec<ForecastDay>,
}

/// Conditions at the start of an hour; precipitation covers that following hour.
#[derive(Clone, Serialize)]
pub struct ForecastHour {
    pub starts_at: DateTime<Utc>,
    pub temperature_c: Option<f64>,
    pub wind_ms: Option<f64>,
    pub precipitation_mm: Option<f64>,
    pub precipitation_probability_percent: Option<f64>,
}

impl Forecast {
    // Select on every read so stale cached data never starts with an elapsed hour.
    fn upcoming(&self, now: DateTime<Utc>) -> Self {
        let start = now
            - chrono::Duration::seconds(now.timestamp().rem_euclid(3600))
            - chrono::Duration::nanoseconds(i64::from(now.timestamp_subsec_nanos()));
        let hourly = (0..24)
            .map(|index| {
                let starts_at = start + chrono::Duration::hours(index);
                self.hourly
                    .iter()
                    .find(|hour| hour.starts_at == starts_at)
                    .cloned()
                    .unwrap_or(ForecastHour {
                        starts_at,
                        temperature_c: None,
                        wind_ms: None,
                        precipitation_mm: None,
                        precipitation_probability_percent: None,
                    })
            })
            .collect();
        Self {
            fetched_at: self.fetched_at,
            valid_until: self.valid_until,
            timezone: self.timezone,
            current: self.current.clone(),
            hourly,
            days: self.days.clone(),
        }
    }
}

/// Modelled current conditions. Null is absence, never zero or clear skies.
#[derive(Clone, Serialize)]
pub struct CurrentConditions {
    pub observed_at: DateTime<Utc>,
    pub temperature_c: Option<f64>,
    pub feels_like_c: Option<f64>,
    pub weather_code: Option<u8>,
    pub wind_ms: Option<f64>,
}

/// One forecast day, with explicitly named units and independently missing values.
#[derive(Clone, Serialize)]
pub struct ForecastDay {
    pub date: NaiveDate,
    pub weather_code: Option<u8>,
    pub high_c: Option<f64>,
    pub low_c: Option<f64>,
    pub precipitation_mm: Option<f64>,
    pub wind_ms: Option<f64>,
}

#[derive(Default)]
struct Cache {
    forecast: Option<Forecast>,
    refresh: RefreshSchedule,
}

// Admission is independent of forecast freshness; a failed refresh retains the data.
#[derive(Default)]
enum RefreshSchedule {
    #[default]
    Unattempted,
    Scheduled {
        at: DateTime<Utc>,
    },
    Backoff {
        until: DateTime<Utc>,
    },
}

impl RefreshSchedule {
    fn is_due(&self, now: DateTime<Utc>) -> bool {
        match self {
            Self::Unattempted => true,
            Self::Scheduled { at } => now >= *at,
            Self::Backoff { until } => now >= *until,
        }
    }
}

impl Cache {
    fn finish_refresh(&mut self, result: Result<Forecast, ()>, now: DateTime<Utc>) {
        match result {
            Ok(forecast) => {
                self.refresh = RefreshSchedule::Scheduled {
                    at: forecast.valid_until,
                };
                self.forecast = Some(forecast);
            }
            Err(()) => {
                // Raw request errors include the coordinate-bearing URL. Do not log them.
                self.refresh = RefreshSchedule::Backoff { until: now + RETRY };
            }
        }
    }

    fn read(&self, now: DateTime<Utc>) -> WeatherRead {
        match &self.forecast {
            Some(forecast) if now < forecast.valid_until => WeatherRead::Fresh {
                forecast: forecast.upcoming(now),
            },
            Some(forecast) if now < forecast.fetched_at + MAX_STALE => WeatherRead::Stale {
                forecast: forecast.upcoming(now),
            },
            _ => WeatherRead::Unavailable {
                reason: WeatherUnavailable::ProviderUnavailable,
            },
        }
    }
}

/// A clone shares one cache and serializes refreshes across all clients.
#[derive(Clone)]
pub struct WeatherService {
    client: Client,
    endpoint: Url,
    location: WeatherLocation,
    cache: Arc<Mutex<Cache>>,
}

impl WeatherService {
    /// The endpoint is supplied by bootstrap, allowing a controlled provider at the HTTP seam.
    pub fn new(location: WeatherLocation, endpoint: Url) -> anyhow::Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            endpoint,
            location,
            cache: Arc::new(Mutex::new(Cache::default())),
        })
    }

    /// Read cached weather or perform one bounded refresh; expected failures stay values.
    pub async fn read(&self) -> WeatherRead {
        self.read_at(Utc::now()).await
    }

    async fn read_at(&self, now: DateTime<Utc>) -> WeatherRead {
        // The guard owns the in-flight refresh: queued readers recheck admission after it
        // completes. Cancellation drops the guard without changing the prior schedule/data.
        let mut cache = self.cache.lock().await;
        if cache.refresh.is_due(now) {
            let result = self.fetch(now).await;
            cache.finish_refresh(result, now);
        }
        cache.read(now)
    }

    async fn fetch(&self, now: DateTime<Utc>) -> Result<Forecast, ()> {
        let mut endpoint = self.endpoint.clone();
        endpoint.query_pairs_mut().extend_pairs([
            ("latitude", self.location.latitude.to_string()),
            ("longitude", self.location.longitude.to_string()),
            ("current", "temperature_2m,apparent_temperature,weather_code,wind_speed_10m".to_string()),
            ("hourly", "temperature_2m,wind_speed_10m,precipitation,precipitation_probability".to_string()),
            ("daily", "weather_code,temperature_2m_max,temperature_2m_min,precipitation_sum,wind_speed_10m_max".to_string()),
            ("timezone", "Europe/Copenhagen".to_string()),
            ("temperature_unit", "celsius".to_string()),
            ("wind_speed_unit", "ms".to_string()),
            ("precipitation_unit", "mm".to_string()),
            ("timeformat", "unixtime".to_string()),
            ("forecast_days", "7".to_string()),
        ]);
        let response = self
            .client
            .get(endpoint)
            .send()
            .await
            .map_err(|_| ())?
            .error_for_status()
            .map_err(|_| ())?;
        let provider: ProviderForecast = response.json().await.map_err(|_| ())?;
        provider.normalize(now)
    }
}

#[cfg(test)]
mod tests;
