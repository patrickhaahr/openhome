use chrono::{DateTime, FixedOffset, Utc};
use chrono_tz::Europe::Copenhagen;
use serde::Deserialize;

use super::{CurrentConditions, Forecast, ForecastDay, ForecastHour, TTL};

#[derive(Deserialize)]
pub(super) struct ProviderForecast {
    timezone: String,
    utc_offset_seconds: i32,
    current_units: Option<CurrentUnits>,
    current: Option<ProviderCurrent>,
    hourly_units: Option<HourlyUnits>,
    hourly: Option<ProviderHours>,
    daily_units: DailyUnits,
    daily: ProviderDays,
}

#[derive(Deserialize)]
struct HourlyUnits {
    temperature_2m: String,
    wind_speed_10m: String,
    precipitation: String,
    precipitation_probability: String,
}

#[derive(Deserialize)]
struct ProviderHours {
    time: Vec<i64>,
    #[serde(default)]
    temperature_2m: Vec<Option<f64>>,
    #[serde(default)]
    wind_speed_10m: Vec<Option<f64>>,
    #[serde(default)]
    precipitation: Vec<Option<f64>>,
    #[serde(default)]
    precipitation_probability: Vec<Option<f64>>,
}

impl ProviderHours {
    fn normalize(self, units: &HourlyUnits) -> Result<Vec<ForecastHour>, ()> {
        if units.temperature_2m != "°C"
            || units.wind_speed_10m != "m/s"
            || units.precipitation != "mm"
            || units.precipitation_probability != "%"
            || self.time.len() > 200
            || self.time.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(());
        }
        self.time
            .iter()
            .enumerate()
            .map(|(index, timestamp)| {
                if timestamp.rem_euclid(3600) != 0 {
                    return Err(());
                }
                // Open-Meteo precipitation is stamped at the END of its interval.
                // Never shift by array index across a missing hour or a DST transition.
                let next = timestamp
                    .checked_add(3600)
                    .and_then(|end| self.time.binary_search(&end).ok());
                Ok(ForecastHour {
                    starts_at: DateTime::from_timestamp(*timestamp, 0).ok_or(())?,
                    temperature_c: self.temperature_2m.get(index).copied().flatten(),
                    wind_ms: self
                        .wind_speed_10m
                        .get(index)
                        .copied()
                        .flatten()
                        .filter(|v| *v >= 0.0),
                    precipitation_mm: next
                        .and_then(|i| self.precipitation.get(i))
                        .copied()
                        .flatten()
                        .filter(|v| *v >= 0.0),
                    precipitation_probability_percent: next
                        .and_then(|i| self.precipitation_probability.get(i))
                        .copied()
                        .flatten()
                        .filter(|v| (0.0..=100.0).contains(v)),
                })
            })
            .collect()
    }
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
    pub(super) fn normalize(self, now: DateTime<Utc>) -> Result<Forecast, ()> {
        let units = &self.daily_units;
        if self.timezone != "Europe/Copenhagen"
            || !matches!(self.utc_offset_seconds, 3600 | 7200)
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
        // Daily Unix timestamps retain the provider's fixed request offset across DST.
        // Copenhagen's offset is still used for the current date and hourly labels.
        let daily_offset = FixedOffset::east_opt(self.utc_offset_seconds).ok_or(())?;
        let hourly = match (self.hourly, self.hourly_units) {
            (Some(hours), Some(units)) => hours.normalize(&units)?,
            (None, _) => Vec::new(),
            _ => return Err(()),
        };
        let days = self
            .daily
            .time
            .iter()
            .enumerate()
            .map(|(index, timestamp)| {
                let date = DateTime::from_timestamp(*timestamp, 0)
                    .ok_or(())?
                    .with_timezone(&daily_offset)
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
            hourly,
            days,
        })
    }
}
