//! Private Open-Meteo integration and bounded, shared forecast cache.

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Europe::Copenhagen;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use url::Url;

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
    pub days: Vec<ForecastDay>,
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
    next_attempt: Option<DateTime<Utc>>,
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
        let mut cache = self.cache.lock().await;
        if cache.next_attempt.is_none_or(|next| now >= next) {
            match self.fetch(now).await {
                Ok(forecast) => {
                    cache.next_attempt = Some(forecast.valid_until);
                    cache.forecast = Some(forecast);
                }
                Err(()) => {
                    // Raw request errors include the coordinate-bearing URL. Do not log them.
                    cache.next_attempt = Some(now + RETRY);
                }
            }
        }
        match &cache.forecast {
            Some(forecast) if now < forecast.valid_until => WeatherRead::Fresh {
                forecast: forecast.clone(),
            },
            Some(forecast) if now < forecast.fetched_at + MAX_STALE => WeatherRead::Stale {
                forecast: forecast.clone(),
            },
            _ => WeatherRead::Unavailable {
                reason: WeatherUnavailable::ProviderUnavailable,
            },
        }
    }

    async fn fetch(&self, now: DateTime<Utc>) -> Result<Forecast, ()> {
        let mut endpoint = self.endpoint.clone();
        endpoint.query_pairs_mut().extend_pairs([
            ("latitude", self.location.latitude.to_string()),
            ("longitude", self.location.longitude.to_string()),
            ("current", "temperature_2m,apparent_temperature,weather_code,wind_speed_10m".to_string()),
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

#[derive(Deserialize)]
struct ProviderForecast {
    timezone: String,
    current_units: Option<CurrentUnits>,
    current: Option<ProviderCurrent>,
    daily_units: DailyUnits,
    daily: ProviderDays,
}

#[derive(Deserialize)]
struct CurrentUnits {
    temperature_2m: String,
    apparent_temperature: String,
    wind_speed_10m: String,
}

#[derive(Deserialize)]
struct DailyUnits {
    temperature_2m_max: String,
    temperature_2m_min: String,
    precipitation_sum: String,
    wind_speed_10m_max: String,
}

#[derive(Deserialize)]
struct ProviderCurrent {
    time: i64,
    temperature_2m: Option<f64>,
    apparent_temperature: Option<f64>,
    weather_code: Option<u8>,
    wind_speed_10m: Option<f64>,
}

#[derive(Deserialize)]
struct ProviderDays {
    time: Vec<i64>,
    #[serde(default)]
    weather_code: Vec<Option<u8>>,
    #[serde(default)]
    temperature_2m_max: Vec<Option<f64>>,
    #[serde(default)]
    temperature_2m_min: Vec<Option<f64>>,
    #[serde(default)]
    precipitation_sum: Vec<Option<f64>>,
    #[serde(default)]
    wind_speed_10m_max: Vec<Option<f64>>,
}

impl ProviderForecast {
    fn normalize(self, now: DateTime<Utc>) -> Result<Forecast, ()> {
        let units = &self.daily_units;
        if self.timezone != "Europe/Copenhagen"
            || units.temperature_2m_max != "°C"
            || units.temperature_2m_min != "°C"
            || units.precipitation_sum != "mm"
            || units.wind_speed_10m_max != "m/s"
            || self.daily.time.len() != 7
        {
            return Err(());
        }
        let current = match (self.current, self.current_units) {
            (Some(value), Some(units))
                if units.temperature_2m == "°C"
                    && units.apparent_temperature == "°C"
                    && units.wind_speed_10m == "m/s" =>
            {
                Some(CurrentConditions {
                    observed_at: DateTime::from_timestamp(value.time, 0).ok_or(())?,
                    temperature_c: value.temperature_2m,
                    feels_like_c: value.apparent_temperature,
                    weather_code: value.weather_code,
                    wind_ms: value.wind_speed_10m.filter(|v| *v >= 0.0),
                })
            }
            (None, _) => None,
            _ => return Err(()),
        };
        let first_day = now.with_timezone(&Copenhagen).date_naive();
        let days = self
            .daily
            .time
            .iter()
            .enumerate()
            .map(|(index, timestamp)| {
                let date = DateTime::from_timestamp(*timestamp, 0)
                    .ok_or(())?
                    .with_timezone(&Copenhagen)
                    .date_naive();
                if Some(date) != first_day.checked_add_days(chrono::Days::new(index as u64)) {
                    return Err(());
                }
                Ok(ForecastDay {
                    date,
                    weather_code: self.daily.weather_code.get(index).copied().flatten(),
                    high_c: self.daily.temperature_2m_max.get(index).copied().flatten(),
                    low_c: self.daily.temperature_2m_min.get(index).copied().flatten(),
                    precipitation_mm: self
                        .daily
                        .precipitation_sum
                        .get(index)
                        .copied()
                        .flatten()
                        .filter(|v| *v >= 0.0),
                    wind_ms: self
                        .daily
                        .wind_speed_10m_max
                        .get(index)
                        .copied()
                        .flatten()
                        .filter(|v| *v >= 0.0),
                })
            })
            .collect::<Result<Vec<_>, ()>>()?;
        Ok(Forecast {
            fetched_at: now,
            valid_until: now + TTL,
            timezone: "Europe/Copenhagen",
            current,
            days,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::{Value, json};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, query_param},
    };

    fn fixture(now: DateTime<Utc>) -> Value {
        let mut body: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/weather-copenhagen.json"))
                .unwrap();
        let first = now.with_timezone(&Copenhagen).date_naive();
        body["daily"]["time"] = json!(
            (0..7)
                .map(|index| {
                    let day = first.checked_add_days(chrono::Days::new(index)).unwrap();
                    Copenhagen
                        .from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap())
                        .single()
                        .unwrap()
                        .timestamp()
                })
                .collect::<Vec<_>>()
        );
        body["current"]["time"] = json!(now.timestamp());
        body
    }

    fn service(server: &MockServer) -> WeatherService {
        WeatherService::new(
            WeatherLocation::parse(Some("55.67"), Some("12.57"))
                .unwrap()
                .unwrap(),
            Url::parse(&server.uri()).unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn normalizes_danish_calendar_days_across_dst_and_preserves_missing_values() {
        for instant in ["2026-03-28T23:30:00Z", "2026-10-24T22:30:00Z"] {
            let now: DateTime<Utc> = instant.parse().unwrap();
            let server = MockServer::start().await;
            let mut body = fixture(now);
            body["current"]["temperature_2m"] = Value::Null;
            body["daily"]["precipitation_sum"] = json!([null, 0.2]);
            Mock::given(method("GET"))
                .and(query_param("timezone", "Europe/Copenhagen"))
                .and(query_param("wind_speed_unit", "ms"))
                .and(query_param("timeformat", "unixtime"))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
                .expect(1)
                .mount(&server)
                .await;
            let response = serde_json::to_value(service(&server).read_at(now).await).unwrap();
            assert_eq!(response["state"], "fresh");
            assert_eq!(
                response["forecast"]["days"][0]["date"],
                if instant.starts_with("2026-03") {
                    "2026-03-29"
                } else {
                    "2026-10-25"
                }
            );
            assert_eq!(
                response["forecast"]["current"]["temperature_c"],
                Value::Null
            );
            assert_eq!(response["forecast"]["current"]["wind_ms"], 3.3);
            assert_eq!(
                response["forecast"]["days"][0]["precipitation_mm"],
                Value::Null
            );
            assert_eq!(response["forecast"]["days"][1]["precipitation_mm"], 0.2);
            assert_eq!(
                response["forecast"]["days"][2]["precipitation_mm"],
                Value::Null
            );
            assert_eq!(
                response["forecast"]["days"][6]["date"],
                if instant.starts_with("2026-03") {
                    "2026-04-04"
                } else {
                    "2026-10-31"
                }
            );
            assert!(response.to_string().find("latitude").is_none());
        }
    }

    #[tokio::test]
    async fn shares_refreshes_expires_at_ttl_backs_off_and_recovers() {
        let now: DateTime<Utc> = "2026-10-11T10:00:00Z".parse().unwrap();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture(now)))
            .expect(1)
            .mount(&server)
            .await;
        let service = service(&server);
        let clone = service.clone();
        let (a, b) = tokio::join!(service.read_at(now), clone.read_at(now));
        assert!(matches!(a, WeatherRead::Fresh { .. }));
        assert!(matches!(b, WeatherRead::Fresh { .. }));
        assert!(matches!(
            service
                .read_at(now + TTL - chrono::Duration::seconds(1))
                .await,
            WeatherRead::Fresh { .. }
        ));
        server.verify().await;
        server.reset().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503).set_body_string("synthetic-private-error"))
            .expect(1)
            .mount(&server)
            .await;
        let stale = serde_json::to_value(service.read_at(now + TTL).await).unwrap();
        assert_eq!(stale["state"], "stale");
        assert_eq!(stale["forecast"]["fetched_at"], "2026-10-11T10:00:00Z");
        assert_eq!(stale["forecast"]["valid_until"], "2026-10-11T10:15:00Z");
        assert!(matches!(
            service
                .read_at(now + TTL + chrono::Duration::seconds(59))
                .await,
            WeatherRead::Stale { .. }
        ));
        server.verify().await;
        server.reset().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture(now)))
            .expect(1)
            .mount(&server)
            .await;
        let fresh = serde_json::to_value(service.read_at(now + TTL + RETRY).await).unwrap();
        assert_eq!(fresh["state"], "fresh");
        assert_eq!(fresh["forecast"]["fetched_at"], "2026-10-11T10:16:00Z");
    }

    #[tokio::test]
    async fn stops_serving_old_forecasts_after_six_hours() {
        let now: DateTime<Utc> = "2026-10-11T10:00:00Z".parse().unwrap();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fixture(now)))
            .mount(&server)
            .await;
        let service = service(&server);
        service.read_at(now).await;
        server.reset().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        assert!(matches!(
            service
                .read_at(now + MAX_STALE - chrono::Duration::seconds(1))
                .await,
            WeatherRead::Stale { .. }
        ));
        let response = serde_json::to_value(service.read_at(now + MAX_STALE).await).unwrap();
        assert_eq!(
            response,
            json!({ "state": "unavailable", "reason": "provider-unavailable" })
        );
    }

    #[tokio::test]
    async fn rejects_wrong_units_timezone_and_incomplete_days_without_inventing_weather() {
        let now: DateTime<Utc> = "2026-10-11T10:00:00Z".parse().unwrap();
        for (field, value) in [
            ("timezone", json!("UTC")),
            ("daily_units", json!({})),
            ("daily", json!({ "time": [] })),
            (
                "current_units",
                json!({ "temperature_2m": "°F", "apparent_temperature": "°F", "wind_speed_10m": "mph" }),
            ),
        ] {
            let server = MockServer::start().await;
            let mut body = fixture(now);
            body[field] = value;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
                .expect(1)
                .mount(&server)
                .await;
            let service = service(&server);
            assert!(matches!(
                service.read_at(now).await,
                WeatherRead::Unavailable { .. }
            ));
            assert!(matches!(
                service.read_at(now + chrono::Duration::seconds(59)).await,
                WeatherRead::Unavailable { .. }
            ));
        }
    }

    #[test]
    fn validates_private_location_without_echoing_input() {
        assert!(WeatherLocation::parse(None, None).unwrap().is_none());
        for (lat, lon) in [
            (Some("secret-invalid"), Some("12")),
            (Some("NaN"), Some("12")),
            (Some("91"), Some("12")),
            (Some("55"), Some("181")),
            (Some("55"), None),
        ] {
            let error = WeatherLocation::parse(lat, lon).err().unwrap().to_string();
            assert!(!error.contains("secret-invalid"));
        }
    }
}
