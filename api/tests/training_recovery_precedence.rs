mod common;
mod training_recovery_support;

use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use training_recovery_support::{
    COMPUTED, DERIVED, IMPORTED, SEP_20, absent, covered_mirror, daily, measured, night_a, night_b,
    real_computed_days, real_imported_days, real_mirror, sleep,
};

#[tokio::test]
async fn imported_values_win_field_by_field_and_computed_values_fill_nulls() {
    let real = real_mirror().await.recovery_day("2026-09-14").await;
    assert_eq!(
        real["derived_scores"]["strain"],
        measured(json!(3.1), "score_0_100", IMPORTED)
    );
    assert_eq!(real["derived_scores"]["recovery"], absent("score_0_100"));
    assert_eq!(real["sleep"]["total"], absent("min"));

    let mirror = covered_mirror(
        &[daily(
            "2026-09-20",
            json!({"restingHr": 52.0, "avgHrv": null, "recovery": null, "strain": null}),
        )],
        &[daily(
            "2026-09-20",
            json!({"restingHr": 50.0, "avgHrv": 71.5, "recovery": 64.0, "strain": 0.0}),
        )],
        &[],
        &[],
    )
    .await;
    let body = mirror.recovery_day("2026-09-20").await;

    assert_eq!(
        body["resting_hr"],
        measured(json!(52.0), "beats/min", IMPORTED)
    );
    assert_eq!(body["hrv_rmssd"], measured(json!(71.5), "ms", COMPUTED));
    assert_eq!(
        body["derived_scores"],
        json!({
            "recovery": measured(json!(64.0), "score_0_100", COMPUTED),
            "strain": measured(json!(0.0), "score_0_100", COMPUTED),
            "rest": absent("score_0_100"),
        })
    );
}

#[tokio::test]
async fn skin_temperature_deviation_and_respiratory_rate_follow_imported_first_precedence() {
    let mirror = covered_mirror(
        &[
            daily(
                "2026-09-20",
                json!({"skinTempDevC": -0.3, "respRateBpm": null}),
            ),
            daily("2026-09-21", json!({"skinTempDevC": 0.0})),
            daily(
                "2026-09-24",
                json!({"skinTempDevC": null, "respRateBpm": 16.2}),
            ),
        ],
        &[
            daily(
                "2026-09-20",
                json!({"skinTempDevC": 0.4, "respRateBpm": 14.8}),
            ),
            daily(
                "2026-09-22",
                json!({"skinTempDevC": 0.0, "respRateBpm": 15.2}),
            ),
            daily("2026-09-23", json!({})),
            daily(
                "2026-09-24",
                json!({"skinTempDevC": 0.2, "respRateBpm": 14.0}),
            ),
        ],
        &[],
        &[],
    )
    .await;

    let both = mirror.recovery_day("2026-09-20").await;
    assert_eq!(
        both["skin_temp_deviation"],
        measured(json!(-0.3), "degC", IMPORTED)
    );
    assert_eq!(
        both["respiratory_rate"],
        measured(json!(14.8), "breaths/min", COMPUTED)
    );

    let imported_only = mirror.recovery_day("2026-09-21").await;
    assert_eq!(
        imported_only["skin_temp_deviation"],
        measured(json!(0.0), "degC", IMPORTED)
    );
    assert_eq!(imported_only["respiratory_rate"], absent("breaths/min"));

    let computed_only = mirror.recovery_day("2026-09-22").await;
    assert_eq!(
        computed_only["skin_temp_deviation"],
        measured(json!(0.0), "degC", COMPUTED)
    );
    assert_eq!(
        computed_only["respiratory_rate"],
        measured(json!(15.2), "breaths/min", COMPUTED)
    );

    let neither = mirror.recovery_day("2026-09-23").await;
    assert_eq!(neither["skin_temp_deviation"], absent("degC"));
    assert_eq!(neither["respiratory_rate"], absent("breaths/min"));

    let opposite_sources = mirror.recovery_day("2026-09-24").await;
    assert_eq!(
        opposite_sources["skin_temp_deviation"],
        measured(json!(0.2), "degC", COMPUTED)
    );
    assert_eq!(
        opposite_sources["respiratory_rate"],
        measured(json!(16.2), "breaths/min", IMPORTED)
    );
}

#[tokio::test]
async fn an_edited_night_takes_the_whole_computed_sleep_block() {
    let imported_night = daily(
        "2026-09-15",
        json!({
            "totalSleepMin": 480.0, "efficiency": 0.91, "deepMin": 90.0, "remMin": 100.0,
            "lightMin": 250.0, "disturbances": 4, "restingHr": 48.0, "avgHrv": 95.0,
        }),
    );
    let mut imported_days = real_imported_days();
    imported_days.push(imported_night);

    for edited in [true, false] {
        let mirror = covered_mirror(
            &imported_days,
            &real_computed_days(),
            &[],
            &[night_a(), night_b(edited)],
        )
        .await;
        let body = mirror.recovery_day("2026-09-15").await;
        let sleep = &body["sleep"];

        if edited {
            assert_eq!(sleep["total"], measured(json!(102.25), "min", COMPUTED));
            assert_eq!(
                sleep["efficiency"],
                measured(json!(0.2237417943), "fraction", COMPUTED)
            );
            assert_eq!(sleep["deep"], measured(json!(36.5), "min", COMPUTED));
            assert_eq!(sleep["disturbances"], absent("count"));
        } else {
            assert_eq!(sleep["total"], measured(json!(480.0), "min", IMPORTED));
            assert_eq!(
                sleep["efficiency"],
                measured(json!(0.91), "fraction", IMPORTED)
            );
            assert_eq!(sleep["deep"], measured(json!(90.0), "min", IMPORTED));
            assert_eq!(sleep["disturbances"], measured(json!(4), "count", IMPORTED));
        }
        assert_eq!(
            body["resting_hr"],
            measured(json!(48.0), "beats/min", IMPORTED)
        );
        assert_eq!(body["hrv_rmssd"], measured(json!(95.0), "ms", IMPORTED));
    }
}

#[tokio::test]
async fn a_scored_computed_night_replaces_a_bare_imported_sleep_total() {
    let computed = [daily(
        "2026-09-20",
        json!({
            "totalSleepMin": 430.0, "efficiency": 0.93, "deepMin": 80.0, "remMin": 95.0,
            "lightMin": 240.0, "disturbances": 6,
        }),
    )];

    let bare = covered_mirror(
        &[daily("2026-09-20", json!({"totalSleepMin": 455.0}))],
        &computed,
        &[],
        &[],
    )
    .await;
    let sleep = bare.recovery_day("2026-09-20").await["sleep"].clone();
    assert_eq!(sleep["total"], measured(json!(430.0), "min", COMPUTED));
    assert_eq!(
        sleep["efficiency"],
        measured(json!(0.93), "fraction", COMPUTED)
    );

    let scored = covered_mirror(
        &[daily(
            "2026-09-20",
            json!({"totalSleepMin": 455.0, "efficiency": 0.88}),
        )],
        &computed,
        &[],
        &[],
    )
    .await;
    let sleep = scored.recovery_day("2026-09-20").await["sleep"].clone();
    assert_eq!(sleep["total"], measured(json!(455.0), "min", IMPORTED));
    assert_eq!(
        sleep["efficiency"],
        measured(json!(0.88), "fraction", IMPORTED)
    );
    assert_eq!(sleep["deep"], measured(json!(80.0), "min", COMPUTED));
    assert_eq!(sleep["disturbances"], measured(json!(6), "count", COMPUTED));
}

#[tokio::test]
async fn rest_scores_the_night_against_the_need_and_regularity_of_the_nights_before() {
    let history: Vec<Value> = (1..=7)
        .map(|nights_ago| {
            let start = SEP_20 - nights_ago * 86_400;
            sleep(start, start + 8 * 3600, json!({"efficiency": 1.0}))
        })
        .collect();
    let night = daily(
        "2026-09-20",
        json!({"totalSleepMin": 480.0, "efficiency": 1.0, "deepMin": 90.0, "remMin": 100.0}),
    );
    let body = covered_mirror(std::slice::from_ref(&night), &[], &history, &[])
        .await
        .recovery_day("2026-09-20")
        .await;
    assert_eq!(
        body["derived_scores"]["rest"],
        measured(json!(95.83), "score_0_100", DERIVED)
    );

    let body = covered_mirror(&[night], &[], &[], &[])
        .await
        .recovery_day("2026-09-20")
        .await;
    assert_eq!(
        body["derived_scores"]["rest"],
        measured(json!(90.83), "score_0_100", DERIVED)
    );
}

#[tokio::test]
async fn sleep_stages_are_ordered_clamped_to_the_session_and_null_without_segments() {
    let stages = r#"[{"start":1789850000,"end":1789851000,"stage":"wake"},{"start":1789848000,"end":1789848900,"stage":"light"},{"start":1789848900,"end":1789850000,"stage":"deep"},{"start":1789851000,"end":1789853000,"stage":"rem"}]"#;
    let adjusted = sleep(
        1_789_848_000,
        1_789_851_600,
        json!({"stagesJSON": stages, "startTsAdjusted": 1_789_848_600}),
    );
    let minutes_only = sleep(
        1_789_851_700,
        1_789_852_700,
        json!({"stagesJSON": r#"{"awake":1,"light":2,"deep":3,"rem":4}"#}),
    );
    let unstaged = sleep(1_789_852_800, 1_789_854_000, json!({}));
    let body = covered_mirror(&[], &[], &[adjusted, minutes_only, unstaged], &[])
        .await
        .recovery_day("2026-09-19")
        .await;
    let sessions = body["sleep"]["sessions"].as_array().unwrap();

    assert_eq!(
        sessions[0]["stages"],
        json!([
            {"start": "2026-09-19T22:10:00+02:00", "end": "2026-09-19T22:15:00+02:00", "stage": "light"},
            {"start": "2026-09-19T22:15:00+02:00", "end": "2026-09-19T22:33:20+02:00", "stage": "deep"},
            {"start": "2026-09-19T22:33:20+02:00", "end": "2026-09-19T22:50:00+02:00", "stage": "awake"},
            {"start": "2026-09-19T22:50:00+02:00", "end": "2026-09-19T23:00:00+02:00", "stage": "rem"},
        ])
    );
    assert_eq!(sessions[1]["stages"], Value::Null);
    assert_eq!(sessions[2]["stages"], Value::Null);
}

#[tokio::test]
async fn richer_computed_sessions_replace_the_imported_night() {
    let night = |staged: bool| {
        let stages = r#"[{"start":1789848000,"end":1789863000,"stage":"light"},{"start":1789863000,"end":1789878600,"stage":"deep"}]"#;
        let stages = if staged { json!(stages) } else { Value::Null };
        sleep(1_789_848_000, 1_789_878_600, json!({"stagesJSON": stages}))
    };
    let sources = |body: &Value| -> Vec<Value> {
        body["sleep"]["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| session["source"].clone())
            .collect()
    };

    let unstaged_import = covered_mirror(&[], &[], &[night(false)], &[night(true)]).await;
    let body = unstaged_import.recovery_day("2026-09-20").await;
    assert_eq!(sources(&body), [json!(COMPUTED)]);
    assert_eq!(
        body["sleep"]["sessions"][0]["stage_min"],
        json!({"awake": 0.0, "light": 250.0, "deep": 260.0, "rem": 0.0})
    );

    let equally_staged = covered_mirror(&[], &[], &[night(true)], &[night(true)]).await;
    let body = equally_staged.recovery_day("2026-09-20").await;
    assert_eq!(sources(&body), [json!(IMPORTED)]);
}
