//! Europe/Copenhagen calendar days and ISO 8601 rendering.
//!
//! A Training Day and a Sleep Night are local calendar days, so their bounds are the local
//! midnights computed through the IANA zone. Around a daylight-saving change a day is 23 or 25
//! hours long; nothing here assumes 24.

use std::fmt;

use chrono::{DateTime, NaiveDate, NaiveTime, SecondsFormat, TimeZone, Utc};
use chrono_tz::Tz;

pub const TIME_ZONE: Tz = chrono_tz::Europe::Copenhagen;
pub const TIME_ZONE_NAME: &str = "Europe/Copenhagen";

/// One Copenhagen calendar day, written `YYYY-MM-DD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalendarDay(NaiveDate);

impl CalendarDay {
    /// Parses a canonical `YYYY-MM-DD` day. Returns `None` for any other spelling and for the
    /// extreme dates whose neighbours cannot be represented.
    pub fn parse(text: &str) -> Option<Self> {
        let date = NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()?;
        // chrono accepts unpadded fields; only the canonical form is part of the contract.
        if date.format("%Y-%m-%d").to_string() != text {
            return None;
        }
        date.pred_opt()?.succ_opt()?.succ_opt()?;
        Some(Self(date))
    }

    pub fn previous(self) -> Self {
        Self(
            self.0
                .pred_opt()
                .expect("parse keeps a representable neighbour"),
        )
    }

    pub fn next(self) -> Self {
        Self(
            self.0
                .succ_opt()
                .expect("parse keeps a representable neighbour"),
        )
    }

    /// Local midnight at the start of the day.
    pub fn start(self) -> DateTime<Utc> {
        let midnight = self.0.and_time(NaiveTime::MIN);
        TIME_ZONE
            .from_local_datetime(&midnight)
            .earliest()
            // Copenhagen never skips midnight; a zone that did would start at the gap's end.
            .unwrap_or_else(|| TIME_ZONE.from_utc_datetime(&midnight))
            .with_timezone(&Utc)
    }

    /// Local midnight at the end of the day (exclusive), i.e. the start of the next day.
    pub fn end(self) -> DateTime<Utc> {
        self.next().start()
    }

    /// The local calendar day an instant falls on.
    pub fn of(instant: DateTime<Utc>) -> Self {
        Self(instant.with_timezone(&TIME_ZONE).date_naive())
    }
}

impl fmt::Display for CalendarDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%d"))
    }
}

/// Renders an instant as ISO 8601 in Copenhagen time with an explicit offset.
pub fn iso(instant: DateTime<Utc>) -> String {
    instant
        .with_timezone(&TIME_ZONE)
        .to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// Converts NOOP's Unix seconds to an instant; `None` outside chrono's range.
pub fn from_unix(seconds: i64) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(seconds, 0)
}

/// Renders NOOP's Unix seconds with [`iso`]; an error outside chrono's range.
pub fn iso_from_unix(seconds: i64) -> anyhow::Result<String> {
    from_unix(seconds)
        .map(iso)
        .ok_or_else(|| anyhow::anyhow!("NOOP timestamp {seconds} is out of range"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> CalendarDay {
        CalendarDay::parse(text).unwrap()
    }

    #[test]
    fn only_canonical_days_parse() {
        for text in [
            "2026-9-15",
            "2026-09-5",
            "2026-02-30",
            "15-09-2026",
            "",
            " 2026-09-15",
        ] {
            assert_eq!(CalendarDay::parse(text), None, "{text:?}");
        }
        assert_eq!(day("2026-09-15").to_string(), "2026-09-15");
    }

    #[test]
    fn day_bounds_follow_daylight_saving() {
        // Summer time ends on 2026-10-25: that local day lasts 25 hours.
        let autumn = day("2026-10-25");
        assert_eq!(iso(autumn.start()), "2026-10-25T00:00:00+02:00");
        assert_eq!(iso(autumn.end()), "2026-10-26T00:00:00+01:00");
        assert_eq!((autumn.end() - autumn.start()).num_hours(), 25);

        // Summer time starts on 2026-03-29: that local day lasts 23 hours.
        let spring = day("2026-03-29");
        assert_eq!(iso(spring.start()), "2026-03-29T00:00:00+01:00");
        assert_eq!((spring.end() - spring.start()).num_hours(), 23);
    }

    #[test]
    fn instants_land_on_their_local_day() {
        // 22:30 UTC on 2026-09-14 is already 00:30 on the 15th in Copenhagen.
        let instant = from_unix(1_789_425_000).unwrap();
        assert_eq!(CalendarDay::of(instant), day("2026-09-15"));
        assert_eq!(iso(instant), "2026-09-15T00:30:00+02:00");
    }
}
