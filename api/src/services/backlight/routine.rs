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
    #[serde(default)]
    pub start: crate::services::patch::Patch<String>,
    #[serde(default)]
    pub end: crate::services::patch::Patch<String>,
    #[serde(default)]
    pub brightness: crate::services::patch::Patch<Brightness>,
}
impl BedtimeUpdate {
    pub(super) fn supplied(&self) -> bool {
        self.enabled.supplied()
            || self.start.supplied()
            || self.end.supplied()
            || self.brightness.supplied()
    }
    pub(super) fn apply(self, settings: &mut BedtimeSettings) -> crate::error::Result<()> {
        use crate::{error::AppError, services::patch::Patch};
        fn field<T>(patch: Patch<T>, current: T) -> crate::error::Result<T> {
            match patch {
                Patch::Missing => Ok(current),
                Patch::Value(Some(value)) => Ok(value),
                Patch::Value(None) => Err(AppError::Validation(
                    "Bedtime settings cannot be null".into(),
                )),
            }
        }
        fn time(patch: Patch<String>, current: NaiveTime) -> crate::error::Result<NaiveTime> {
            let value = field(patch, current.format("%H:%M").to_string())?;
            let parsed = NaiveTime::parse_from_str(&value, "%H:%M").map_err(|_| {
                AppError::Validation("Use a time from 00:00 to 23:59 in HH:MM format".into())
            })?;
            if value != parsed.format("%H:%M").to_string() {
                return Err(AppError::Validation(
                    "Use HH:MM format for bedtime times".into(),
                ));
            }
            Ok(parsed)
        }
        settings.enabled = field(self.enabled, settings.enabled)?;
        settings.start = time(self.start, settings.start)?;
        settings.end = time(self.end, settings.end)?;
        settings.brightness = field(self.brightness, settings.brightness)?;
        if settings.start == settings.end {
            return Err(AppError::Validation(
                "Start and End must be different".into(),
            ));
        }
        Ok(())
    }
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
/// Who currently decides the intended selection. A manual selection always carries
/// its own data; Automatic carries none, so the two cannot disagree.
#[derive(Clone, Copy)]
enum Control {
    Automatic,
    Manual(BacklightOverride),
}
/// The intended selection and where it comes from. A manual decision's selection lives only
/// in its override and an automatic one's only in the routine, so the two cannot disagree.
#[derive(Clone, Copy)]
enum Decision {
    Automatic { selection: Selection },
    Manual(BacklightOverride),
}
/// The complete policy decision: intended selection, its source and the next due
/// transition. Independent of device delivery; a failed write never changes it. Only
/// `Policy` transitions construct one, and its fields are read through accessors, so a
/// selection and override that disagree cannot be persisted.
pub struct Target {
    bedtime: BedtimeSettings,
    decision: Decision,
    next_transition: Option<DateTime<Utc>>,
}
impl Target {
    fn new(
        bedtime: BedtimeSettings,
        manual_override: Option<BacklightOverride>,
        routine_selection: Selection,
        next_transition: Option<DateTime<Utc>>,
    ) -> Self {
        let decision = match manual_override {
            Some(value) => Decision::Manual(value),
            None => Decision::Automatic {
                selection: routine_selection,
            },
        };
        Self {
            bedtime,
            decision,
            next_transition,
        }
    }
    #[must_use]
    pub fn bedtime(&self) -> &BedtimeSettings {
        &self.bedtime
    }
    #[must_use]
    pub fn selection(&self) -> Selection {
        match self.decision {
            Decision::Automatic { selection } => selection,
            Decision::Manual(value) => value.selection,
        }
    }
    /// The manual override to persist; `None` while Automatic.
    #[must_use]
    pub fn manual_override(&self) -> Option<BacklightOverride> {
        match self.decision {
            Decision::Automatic { .. } => None,
            Decision::Manual(value) => Some(value),
        }
    }
    #[must_use]
    pub fn next_transition(&self) -> Option<DateTime<Utc>> {
        self.next_transition
    }
    #[must_use]
    pub fn control_source(&self) -> ControlSource {
        match self.decision {
            Decision::Automatic { .. } => ControlSource::Automatic,
            Decision::Manual(_) => ControlSource::Manual,
        }
    }
    #[must_use]
    pub fn override_expires_at(&self) -> Option<DateTime<Utc>> {
        self.manual_override().and_then(|value| value.expires_at)
    }
}
/// Owns every guarded policy transition: select, Automatic, Bedtime edit and
/// time-driven resolution (also used at startup). Each consumes the current policy
/// and yields the next `Target`; the clock is always the current wall time, so
/// missed transitions are never replayed.
pub struct Policy {
    bedtime: BedtimeSettings,
    control: Control,
}
impl Policy {
    /// Rebuild from persisted or live state. `override_selection` and expiry apply only
    /// when the source is Manual.
    #[must_use]
    pub fn new(
        bedtime: BedtimeSettings,
        source: ControlSource,
        override_selection: Selection,
        override_expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        let control = match source {
            ControlSource::Automatic => Control::Automatic,
            ControlSource::Manual => Control::Manual(BacklightOverride {
                selection: override_selection,
                expires_at: override_expires_at,
            }),
        };
        Self { bedtime, control }
    }
    /// Restore from a stored override record (startup).
    pub fn restored(bedtime: BedtimeSettings, stored: Option<BacklightOverride>) -> Self {
        let control = stored.map_or(Control::Automatic, Control::Manual);
        Self { bedtime, control }
    }
    fn live_override(&self, now: DateTime<Utc>) -> Option<BacklightOverride> {
        match self.control {
            Control::Manual(value) if value.expires_at.is_none_or(|expiry| now < expiry) => {
                Some(value)
            }
            _ => None,
        }
    }
    /// Current target: an unexpired override keeps its selection and expiry; an
    /// expired or absent one yields the routine's selection. Used by startup and
    /// the background routine.
    #[must_use]
    pub fn resolve(self, now: DateTime<Utc>) -> Target {
        let evaluation = evaluate(&self.bedtime, now);
        let manual_override = self.live_override(now);
        Target::new(
            self.bedtime,
            manual_override,
            evaluation.selection,
            evaluation.next_transition,
        )
    }
    /// Manual selection that lasts until the routine's next actual transition
    /// (indefinitely while the routine is disabled).
    #[must_use]
    pub fn select(self, selection: Selection, now: DateTime<Utc>) -> Target {
        let evaluation = evaluate(&self.bedtime, now);
        let manual_override = BacklightOverride {
            selection,
            expires_at: evaluation.next_transition,
        };
        Target::new(
            self.bedtime,
            Some(manual_override),
            evaluation.selection,
            evaluation.next_transition,
        )
    }
    /// Clear any override and adopt the routine's current selection.
    #[must_use]
    pub fn automatic(self, now: DateTime<Utc>) -> Target {
        let evaluation = evaluate(&self.bedtime, now);
        Target::new(
            self.bedtime,
            None,
            evaluation.selection,
            evaluation.next_transition,
        )
    }
    /// Apply a validated Bedtime edit. Supplying `enabled` clears the override; otherwise an
    /// unexpired override stays selected and takes the edited window's next transition
    /// as its expiry.
    pub fn edit(
        mut self,
        update: BedtimeUpdate,
        now: DateTime<Utc>,
    ) -> crate::error::Result<Target> {
        let reset_override = update.enabled.supplied();
        update.apply(&mut self.bedtime)?;
        let evaluation = evaluate(&self.bedtime, now);
        let manual_override = if reset_override {
            None
        } else {
            self.live_override(now).map(|value| BacklightOverride {
                selection: value.selection,
                expires_at: evaluation.next_transition,
            })
        };
        Ok(Target::new(
            self.bedtime,
            manual_override,
            evaluation.selection,
            evaluation.next_transition,
        ))
    }
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
#[cfg(test)]
mod policy_tests {
    use super::*;
    use serde_json::json;

    fn source(target: &Target) -> String {
        serde_json::to_value(target.control_source())
            .unwrap()
            .as_str()
            .unwrap()
            .into()
    }

    fn bedtime(enabled: bool, start: &str, end: &str) -> BedtimeSettings {
        serde_json::from_value(
            json!({"enabled": enabled, "start": start, "end": end, "brightness": 20}),
        )
        .unwrap()
    }
    fn selection(value: serde_json::Value) -> Selection {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn automatic_source_discards_supplied_override_data() {
        let policy = Policy::new(
            bedtime(true, "23:00", "00:00"),
            ControlSource::Automatic,
            selection(json!({"mode":"off"})),
            None,
        );
        let target = policy.resolve("2026-10-23T21:30:00Z".parse().unwrap());
        // Outside the window's start, an unexpired manual Off would have leaked; instead
        // 23:30 Copenhagen sits inside the window, so Automatic resolves the routine red.
        assert!(matches!(target.selection(), Selection::Color { .. }));
        assert!(target.manual_override().is_none());
        assert_eq!(source(&target), "automatic");
        assert_eq!(target.override_expires_at(), None);
    }

    #[test]
    fn resolve_keeps_unexpired_override_and_drops_it_exactly_at_expiry() {
        let expiry: DateTime<Utc> = "2026-10-23T21:00:00Z".parse().unwrap();
        let override_at = |expires_at| {
            Policy::restored(
                bedtime(true, "23:00", "00:00"),
                Some(BacklightOverride {
                    selection: selection(json!({"mode":"off"})),
                    expires_at,
                }),
            )
        };
        let before = override_at(Some(expiry)).resolve("2026-10-23T20:59:59Z".parse().unwrap());
        assert!(matches!(before.selection(), Selection::Off { .. }));
        assert_eq!(before.override_expires_at(), Some(expiry));
        assert_eq!(source(&before), "manual");
        let at_expiry = override_at(Some(expiry)).resolve(expiry);
        assert!(matches!(at_expiry.selection(), Selection::Color { .. }));
        assert!(at_expiry.manual_override().is_none());
        assert_eq!(source(&at_expiry), "automatic");
    }

    #[test]
    fn edit_on_an_already_expired_override_resolves_automatically() {
        let expired = Policy::restored(
            bedtime(true, "23:00", "00:00"),
            Some(BacklightOverride {
                selection: selection(json!({"mode":"off"})),
                expires_at: Some("2026-10-23T21:00:00Z".parse().unwrap()),
            }),
        );
        let update: BedtimeUpdate = serde_json::from_value(json!({"start":"01:00"})).unwrap();
        let target = expired
            .edit(update, "2026-10-23T22:30:00Z".parse().unwrap())
            .unwrap();
        assert!(matches!(target.selection(), Selection::TvSync { .. }));
        assert!(target.manual_override().is_none());
        assert_eq!(target.bedtime().start, "01:00".parse().unwrap());
    }

    #[test]
    fn select_while_routine_is_disabled_has_no_expiry() {
        let target = Policy::restored(bedtime(false, "23:00", "00:00"), None).select(
            selection(json!({"mode":"off"})),
            "2026-10-23T21:30:00Z".parse().unwrap(),
        );
        assert!(matches!(target.selection(), Selection::Off { .. }));
        assert_eq!(target.manual_override().unwrap().expires_at, None);
        assert_eq!(target.next_transition(), None);
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
