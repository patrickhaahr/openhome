use crate::mirror::Mirror;
use crate::mirror::Window;
use crate::recovery_support::{COMPUTED, IMPORTED, SOURCE, covered_mirror, sleep};
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

#[tokio::test]
async fn sleep_nights_are_grouped_by_copenhagen_wake_day() {
    let sessions = [
        sleep(1_789_848_000, 1_789_854_600, json!({})),
        sleep(1_789_854_900, 1_789_855_800, json!({})),
        sleep(1_789_938_000, 1_789_941_599, json!({})),
        sleep(1_789_939_800, 1_789_941_600, json!({})),
    ];
    let mirror = covered_mirror(&[], &[], &[], &sessions).await;
    let spans = |body: Value| -> Vec<(Value, Value)> {
        body["sleep"]["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| (session["start"].clone(), session["end"].clone()))
            .collect()
    };

    assert_eq!(
        spans(mirror.recovery_day("2026-09-19").await),
        [(
            json!("2026-09-19T22:00:00+02:00"),
            json!("2026-09-19T23:50:00+02:00")
        )]
    );
    assert_eq!(
        spans(mirror.recovery_day("2026-09-20").await),
        [
            (
                json!("2026-09-19T23:55:00+02:00"),
                json!("2026-09-20T00:10:00+02:00")
            ),
            (
                json!("2026-09-20T23:00:00+02:00"),
                json!("2026-09-20T23:59:59+02:00")
            ),
        ]
    );
    assert_eq!(
        spans(mirror.recovery_day("2026-09-21").await),
        [(
            json!("2026-09-20T23:30:00+02:00"),
            json!("2026-09-21T00:00:00+02:00")
        )]
    );
}

#[tokio::test]
async fn day_bounds_and_offsets_follow_daylight_saving() {
    let mut mirror = Mirror::empty().await;
    let pushed_at = "2026-10-28T12:00:00.000Z";
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(
                SOURCE,
                device,
                Window::Days("2026-03-28", "2026-03-31"),
                &[],
                pushed_at,
            )
            .await;
        mirror
            .push(
                SOURCE,
                device,
                Window::Days("2026-10-24", "2026-10-28"),
                &[],
                pushed_at,
            )
            .await;
    }
    let spring = [
        sleep(1_774_737_000, 1_774_764_000, json!({})),
        sleep(1_774_818_900, 1_774_820_700, json!({})),
        sleep(1_774_821_900, 1_774_822_500, json!({})),
    ];
    let autumn = [
        sleep(1_792_875_600, 1_792_908_000, json!({})),
        sleep(1_792_965_600, 1_792_967_400, json!({})),
        sleep(1_792_967_700, 1_792_969_800, json!({})),
    ];
    mirror
        .push(
            SOURCE,
            IMPORTED,
            Window::Starts(1_774_652_400, 1_774_908_000),
            &[],
            pushed_at,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Starts(1_774_652_400, 1_774_908_000),
            &spring,
            pushed_at,
        )
        .await;
    mirror
        .push(
            SOURCE,
            IMPORTED,
            Window::Starts(1_792_792_800, 1_793_142_000),
            &[],
            pushed_at,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Starts(1_792_792_800, 1_793_142_000),
            &autumn,
            pushed_at,
        )
        .await;
    let night = |body: &Value| -> Vec<Value> {
        body["sleep"]["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| json!([session["start"], session["end"], session["duration_min"]]))
            .collect()
    };

    let spring_day = mirror.recovery_day("2026-03-29").await;
    assert_eq!(spring_day["day_start"], "2026-03-29T00:00:00+01:00");
    assert_eq!(spring_day["day_end"], "2026-03-30T00:00:00+02:00");
    assert_eq!(
        night(&spring_day),
        [
            json!([
                "2026-03-28T23:30:00+01:00",
                "2026-03-29T08:00:00+02:00",
                450.0
            ]),
            json!([
                "2026-03-29T23:15:00+02:00",
                "2026-03-29T23:45:00+02:00",
                30.0
            ]),
        ]
    );
    assert_eq!(
        spring_day["noop"]["coverage"],
        json!({"daily_metrics": "covered", "sleep_sessions": "covered", "journal": "unknown"})
    );
    assert_eq!(
        night(&mirror.recovery_day("2026-03-30").await),
        [json!([
            "2026-03-30T00:05:00+02:00",
            "2026-03-30T00:15:00+02:00",
            10.0
        ])]
    );

    let autumn_day = mirror.recovery_day("2026-10-25").await;
    assert_eq!(autumn_day["day_start"], "2026-10-25T00:00:00+02:00");
    assert_eq!(autumn_day["day_end"], "2026-10-26T00:00:00+01:00");
    assert_eq!(
        night(&autumn_day),
        [
            json!([
                "2026-10-24T23:00:00+02:00",
                "2026-10-25T07:00:00+01:00",
                540.0
            ]),
            json!([
                "2026-10-25T23:00:00+01:00",
                "2026-10-25T23:30:00+01:00",
                30.0
            ]),
        ]
    );
    assert_eq!(
        autumn_day["noop"]["coverage"],
        json!({"daily_metrics": "covered", "sleep_sessions": "covered", "journal": "unknown"})
    );
    assert_eq!(
        night(&mirror.recovery_day("2026-10-26").await),
        [json!([
            "2026-10-25T23:35:00+01:00",
            "2026-10-26T00:10:00+01:00",
            35.0
        ])]
    );
    assert_eq!(
        autumn_day["noop"]["last_push_at"],
        "2026-10-28T13:00:00+01:00"
    );
}
