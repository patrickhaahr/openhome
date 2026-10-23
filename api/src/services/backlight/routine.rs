//! Daily Copenhagen wall-clock policy, independent of Bluetooth and elapsed timers.
use super::{Brightness, Rgb, Selection};
use chrono::{DateTime, Duration, NaiveTime, Timelike, Utc};
use chrono_tz::Europe::Copenhagen;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BedtimeSettings {
    pub enabled: bool,
    #[serde(with = "clock_time")]
    pub start: NaiveTime,
    #[serde(with = "clock_time")]
    pub end: NaiveTime,
    pub brightness: Brightness,
}
/// Partial routine update; omitted settings retain their durable values.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BedtimeUpdate {
    #[serde(default)]
    pub enabled: crate::services::patch::Patch<bool>,
}
#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct BacklightOverride {
    pub selection: Selection,
    pub expires_at: Option<DateTime<Utc>>,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlSource {
    Automatic,
    Manual,
}
/// A narrow wall-clock input shared by API commands and the background routine.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
pub struct Evaluation {
    pub selection: Selection,
    pub next_transition: Option<DateTime<Utc>>,
}
/// Includes Start, excludes End. Search actual UTC minutes so local clock changes
/// cannot create an elapsed-24-hour drift or an imaginary transition.
#[must_use]
pub fn evaluate(settings: &BedtimeSettings, now: DateTime<Utc>) -> Evaluation {
    let inside = |instant: DateTime<Utc>| {
        let time = instant.with_timezone(&Copenhagen).time();
        if settings.start < settings.end {
            time >= settings.start && time < settings.end
        } else {
            time >= settings.start || time < settings.end
        }
    };
    let active = settings.enabled && inside(now);
    let next_transition = if settings.enabled {
        let minute = now
            .with_second(0)
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or(now);
        (1..=48 * 60)
            .map(|offset| minute + Duration::minutes(offset))
            .find(|instant| inside(*instant) != active)
    } else {
        None
    };
    Evaluation {
        selection: if active {
            Selection::Color {
                rgb: Rgb {
                    red: 255,
                    green: 0,
                    blue: 0,
                },
                brightness: settings.brightness,
            }
        } else {
            Selection::TvSync { brightness: None }
        },
        next_transition,
    }
}
mod clock_time {
    use chrono::NaiveTime;
    use serde::Deserialize;
    use serde::{Deserializer, Serializer};
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "Serde requires a borrowed field serializer"
    )]
    pub fn serialize<S: Serializer>(time: &NaiveTime, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&time.format("%H:%M").to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NaiveTime, D::Error> {
        let value = String::deserialize(deserializer)?;
        NaiveTime::parse_from_str(&value, "%H:%M").map_err(serde::de::Error::custom)
    }
}
