//! Calendar arithmetic for Important Dates. Every figure is a whole Europe/Copenhagen calendar
//! day: clock time and daylight saving never enter, so a figure changes only at local midnight.
//!
//! 29 February has no anniversary in a common year; it falls back to 28 February there, both
//! for the next occurrence and for counting whole years and months.

use super::DateKind;
use chrono::{DateTime, Datelike, Months, NaiveDate, Utc};
use chrono_tz::Europe::Copenhagen;
use serde::Serialize;

/// One milestone for the requested day. Anniversaries take precedence over round day counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Milestone {
    Anniversary { years: u32 },
    DayCount { days: u32 },
    Yearly { years: u32 },
}

/// Decide the milestone from figures computed for the same Copenhagen day.
#[must_use]
pub fn milestone(kind: DateKind, elapsed: Option<Elapsed>, next: Next) -> Option<Milestone> {
    match kind {
        DateKind::Yearly if next.days_until == 0 => Some(Milestone::Yearly { years: next.years }),
        DateKind::Yearly => None,
        DateKind::Since if next.days_until == 0 && next.years > 0 => {
            Some(Milestone::Anniversary { years: next.years })
        }
        DateKind::Since => elapsed.and_then(|elapsed| {
            let days = elapsed.total_days;
            (days == 100 || days == 500 || (days >= 1000 && days.is_multiple_of(1000)))
                .then_some(Milestone::DayCount { days })
        }),
    }
}

/// The Europe/Copenhagen calendar day at an instant.
#[must_use]
pub fn day_at(instant: DateTime<Utc>) -> NaiveDate {
    instant.with_timezone(&Copenhagen).date_naive()
}

/// Whole years, months and days from a date to a later day, plus the total in days.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Elapsed {
    pub years: u32,
    pub months: u32,
    pub days: u32,
    pub total_days: u32,
}

/// The next anniversary on or after a day: its date, how many days away it is (0 on the day)
/// and which anniversary it is (0 when the date itself has not yet happened).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Next {
    pub date: NaiveDate,
    pub days_until: u32,
    pub years: u32,
}

/// Time elapsed from `date` to `on`, or `None` while `date` is still ahead.
///
/// Months are counted from `date` itself, clamped to the end of shorter months: 31 January
/// plus one month is 28 (or 29) February, so 31 January to 28 February is one month exactly.
#[must_use]
pub fn elapsed(date: NaiveDate, on: NaiveDate) -> Option<Elapsed> {
    if on < date {
        return None;
    }
    let span =
        u32::try_from((on.year() - date.year()) * 12 + on.month() as i32 - date.month() as i32)
            .unwrap_or(0);
    // One fewer month when the clamped month anniversary falls after `on`.
    let months = if add_months(date, span) > on {
        span - 1
    } else {
        span
    };
    let anchor = add_months(date, months);
    Some(Elapsed {
        years: months / 12,
        months: months % 12,
        days: days_between(anchor, on),
        total_days: days_between(date, on),
    })
}

/// The first anniversary of `date` on or after `on`; the date itself while it is ahead.
#[must_use]
pub fn next(date: NaiveDate, on: NaiveDate) -> Next {
    if on <= date {
        return Next {
            date,
            days_until: days_between(on, date),
            years: 0,
        };
    }
    let mut year = on.year();
    let mut occurrence = anniversary(date, year);
    if occurrence < on {
        year += 1;
        occurrence = anniversary(date, year);
    }
    Next {
        date: occurrence,
        days_until: days_between(on, occurrence),
        years: u32::try_from(year - date.year()).unwrap_or(0),
    }
}

/// `date`'s month and day in `year`; 29 February becomes 28 February in a common year.
fn anniversary(date: NaiveDate, year: i32) -> NaiveDate {
    date.with_year(year)
        .or_else(|| NaiveDate::from_ymd_opt(year, 2, 28))
        .expect("every year has 28 February")
}

fn add_months(date: NaiveDate, months: u32) -> NaiveDate {
    date.checked_add_months(Months::new(months))
        .expect("validated dates stay within the calendar")
}

fn days_between(from: NaiveDate, to: NaiveDate) -> u32 {
    u32::try_from((to - from).num_days()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    fn span(from: &str, to: &str) -> (u32, u32, u32, u32) {
        let value = elapsed(day(from), day(to)).unwrap();
        (value.years, value.months, value.days, value.total_days)
    }

    fn ahead(date: &str, on: &str) -> (NaiveDate, u32, u32) {
        let value = next(day(date), day(on));
        (value.date, value.days_until, value.years)
    }

    #[test]
    fn elapsed_counts_whole_years_months_then_days() {
        assert_eq!(span("2024-02-14", "2024-02-14"), (0, 0, 0, 0));
        assert_eq!(span("2024-02-14", "2024-02-15"), (0, 0, 1, 1));
        assert_eq!(span("2024-02-14", "2024-03-13"), (0, 0, 28, 28));
        assert_eq!(span("2024-02-14", "2024-03-14"), (0, 1, 0, 29));
        assert_eq!(span("2024-02-14", "2026-05-26"), (2, 3, 12, 832));
        assert_eq!(span("2016-10-18", "2026-10-17"), (9, 11, 29, 3651));
        assert!(elapsed(day("2026-10-18"), day("2026-10-17")).is_none());
    }

    #[test]
    fn month_ends_clamp_instead_of_overflowing() {
        // 31 January plus one month is the last day of February.
        assert_eq!(span("2025-01-31", "2025-02-27"), (0, 0, 27, 27));
        assert_eq!(span("2025-01-31", "2025-02-28"), (0, 1, 0, 28));
        assert_eq!(span("2025-01-31", "2025-03-01"), (0, 1, 1, 29));
        assert_eq!(span("2025-01-31", "2025-03-31"), (0, 2, 0, 59));
        assert_eq!(span("2024-01-31", "2024-02-29"), (0, 1, 0, 29));
        assert_eq!(span("2025-08-31", "2025-09-30"), (0, 1, 0, 30));
        assert_eq!(span("2025-12-31", "2026-01-01"), (0, 0, 1, 1));
    }

    #[test]
    fn leap_day_counts_its_years_on_28_february_in_common_years() {
        assert_eq!(span("2024-02-29", "2025-02-27"), (0, 11, 29, 364));
        assert_eq!(span("2024-02-29", "2025-02-28"), (1, 0, 0, 365));
        assert_eq!(span("2024-02-29", "2025-03-01"), (1, 0, 1, 366));
        assert_eq!(span("2024-02-29", "2028-02-28"), (3, 11, 30, 1460));
        assert_eq!(span("2024-02-29", "2028-02-29"), (4, 0, 0, 1461));
        assert_eq!(span("2023-03-01", "2024-02-29"), (0, 11, 28, 365));
    }

    #[test]
    fn next_anniversary_counts_down_and_is_zero_on_the_day() {
        assert_eq!(
            ahead("1990-11-04", "2026-10-17"),
            (day("2026-11-04"), 18, 36)
        );
        assert_eq!(
            ahead("1990-10-17", "2026-10-17"),
            (day("2026-10-17"), 0, 36)
        );
        assert_eq!(
            ahead("1990-10-16", "2026-10-17"),
            (day("2027-10-16"), 364, 37)
        );
        assert_eq!(ahead("2019-01-01", "2026-12-31"), (day("2027-01-01"), 1, 8));
        // A date still ahead is its own next occurrence, the zeroth anniversary.
        assert_eq!(
            ahead("2026-12-24", "2026-10-17"),
            (day("2026-12-24"), 68, 0)
        );
    }

    #[test]
    fn leap_day_anniversary_falls_back_to_28_february() {
        assert_eq!(
            ahead("2000-02-29", "2026-02-01"),
            (day("2026-02-28"), 27, 26)
        );
        assert_eq!(
            ahead("2000-02-29", "2026-02-28"),
            (day("2026-02-28"), 0, 26)
        );
        assert_eq!(
            ahead("2000-02-29", "2026-03-01"),
            (day("2027-02-28"), 364, 27)
        );
        assert_eq!(
            ahead("2000-02-29", "2027-03-01"),
            (day("2028-02-29"), 365, 28)
        );
        assert_eq!(
            ahead("2000-02-29", "2028-02-29"),
            (day("2028-02-29"), 0, 28)
        );
    }

    #[test]
    fn days_follow_copenhagen_midnight_across_daylight_saving() {
        let at = |y, m, d, h, min| Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap();
        // Spring forward (29 March 2026): local midnight is 23:00 UTC before, 22:00 UTC after.
        assert_eq!(day_at(at(2026, 3, 28, 22, 59)), day("2026-03-28"));
        assert_eq!(day_at(at(2026, 3, 28, 23, 0)), day("2026-03-29"));
        assert_eq!(day_at(at(2026, 3, 29, 21, 59)), day("2026-03-29"));
        assert_eq!(day_at(at(2026, 3, 29, 22, 0)), day("2026-03-30"));
        // Fall back (25 October 2026): the 25-hour day still counts once.
        assert_eq!(day_at(at(2026, 10, 24, 21, 59)), day("2026-10-24"));
        assert_eq!(day_at(at(2026, 10, 24, 22, 0)), day("2026-10-25"));
        assert_eq!(day_at(at(2026, 10, 25, 22, 59)), day("2026-10-25"));
        assert_eq!(day_at(at(2026, 10, 25, 23, 0)), day("2026-10-26"));
        // A 23-hour and a 25-hour day are each one day.
        assert_eq!(span("2026-03-28", "2026-03-30"), (0, 0, 2, 2));
        assert_eq!(span("2026-10-24", "2026-10-26"), (0, 0, 2, 2));
        assert_eq!(ahead("2020-03-30", "2026-03-28"), (day("2026-03-30"), 2, 6));
    }
}
