mod backlight_support;
mod common;

#[test]
fn bedtime_default_calendar_includes_start_excludes_end_and_tracks_copenhagen_dates() {
    use openhome_api::services::backlight::routine::{BedtimeSettings, evaluate};
    let settings: BedtimeSettings = serde_json::from_value(
        serde_json::json!({"enabled":true,"start":"23:00","end":"00:00","brightness":20}),
    )
    .unwrap();
    for (instant, mode, next) in [
        ("2026-10-23T20:59:59Z", "tv-sync", "2026-10-23T21:00:00Z"),
        ("2026-10-23T21:00:00Z", "color", "2026-10-23T22:00:00Z"),
        ("2026-10-23T21:59:59Z", "color", "2026-10-23T22:00:00Z"),
        ("2026-10-23T22:00:00Z", "tv-sync", "2026-10-24T21:00:00Z"),
        ("2026-10-25T22:00:00Z", "color", "2026-10-25T23:00:00Z"),
    ] {
        let result = evaluate(&settings, instant.parse().unwrap());
        assert_eq!(
            serde_json::to_value(result.selection).unwrap()["mode"],
            mode
        );
        assert_eq!(
            result.next_transition.unwrap(),
            next.parse::<chrono::DateTime<chrono::Utc>>().unwrap()
        );
    }
}

#[test]
fn edited_windows_follow_actual_copenhagen_minutes_through_midnight_skip_and_repeat() {
    use openhome_api::services::backlight::routine::{BedtimeSettings, evaluate};
    for (start, end, instant, mode, next) in [
        (
            "22:00",
            "02:00",
            "2026-01-15T23:00:00Z",
            "color",
            "2026-01-16T01:00:00Z",
        ),
        (
            "22:00",
            "02:00",
            "2026-01-16T01:00:00Z",
            "tv-sync",
            "2026-01-16T21:00:00Z",
        ),
        (
            "02:30",
            "03:30",
            "2026-03-29T00:59:59Z",
            "tv-sync",
            "2026-03-29T01:00:00Z",
        ),
        (
            "02:30",
            "03:30",
            "2026-03-29T01:00:00Z",
            "color",
            "2026-03-29T01:30:00Z",
        ),
        (
            "01:30",
            "02:30",
            "2026-03-29T00:45:00Z",
            "color",
            "2026-03-29T01:00:00Z",
        ),
        (
            "02:10",
            "02:20",
            "2026-03-29T00:59:00Z",
            "tv-sync",
            "2026-03-30T00:10:00Z",
        ),
        (
            "02:15",
            "02:45",
            "2026-10-25T00:14:59Z",
            "tv-sync",
            "2026-10-25T00:15:00Z",
        ),
        (
            "02:15",
            "02:45",
            "2026-10-25T00:15:00Z",
            "color",
            "2026-10-25T00:45:00Z",
        ),
        (
            "02:15",
            "02:45",
            "2026-10-25T00:45:00Z",
            "tv-sync",
            "2026-10-25T01:15:00Z",
        ),
        (
            "02:15",
            "02:45",
            "2026-10-25T01:15:00Z",
            "color",
            "2026-10-25T01:45:00Z",
        ),
        (
            "02:30",
            "03:30",
            "2026-10-25T00:45:00Z",
            "color",
            "2026-10-25T01:00:00Z",
        ),
        (
            "02:30",
            "03:30",
            "2026-10-25T01:00:00Z",
            "tv-sync",
            "2026-10-25T01:30:00Z",
        ),
        (
            "01:30",
            "03:30",
            "2026-10-25T00:30:00Z",
            "color",
            "2026-10-25T02:30:00Z",
        ),
    ] {
        let settings: BedtimeSettings = serde_json::from_value(
            serde_json::json!({"enabled":true,"start":start,"end":end,"brightness":20}),
        )
        .unwrap();
        let result = evaluate(&settings, instant.parse().unwrap());
        assert_eq!(
            serde_json::to_value(result.selection).unwrap()["mode"],
            mode,
            "{start}–{end} at {instant}"
        );
        assert_eq!(
            result.next_transition.unwrap(),
            next.parse::<chrono::DateTime<chrono::Utc>>().unwrap(),
            "{start}–{end} at {instant}"
        );
    }
}
