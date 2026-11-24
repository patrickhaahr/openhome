use crate::mirror::Window;
use crate::recovery_support::{
    API_KEY, COMPUTED, IMPORTED, REAL_PUSH_AT, SEP_20, SOURCE, covered_mirror, daily, journal,
    measured, real_mirror, sleep,
};
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const JOURNAL_DAYS: Window = Window::JournalDays("2026-09-13", "2026-09-27");

#[tokio::test]
async fn lists_the_journal_entries_logged_against_the_day() {
    let mut mirror = covered_mirror(&[], &[], &[], &[]).await;
    mirror
        .push(
            SOURCE,
            "noop-journal",
            JOURNAL_DAYS,
            &[
                journal("2026-09-19", "Alcohol", true, json!({"numericValue": 4.0})),
                journal("2026-09-20", "Stress", false, json!({})),
                journal(
                    "2026-09-20",
                    "Alcohol",
                    true,
                    json!({"numericValue": 2.0, "notes": "Two beers at dinner"}),
                ),
                journal("2026-09-21", "Felt recovered", true, json!({})),
            ],
            REAL_PUSH_AT,
        )
        .await;

    let body = mirror.recovery_day("2026-09-20").await;

    assert_eq!(
        body["journal"],
        json!([
            {
                "question": "Alcohol",
                "answered_yes": true,
                "numeric_value": 2.0,
                "notes": "Two beers at dinner",
                "source": "noop-journal",
            },
            {
                "question": "Stress",
                "answered_yes": false,
                "numeric_value": null,
                "notes": null,
                "source": "noop-journal",
            },
        ])
    );
}

#[tokio::test]
async fn keeps_one_journal_entry_per_question_by_namespace_preference() {
    let mut mirror = covered_mirror(&[], &[], &[], &[]).await;
    let namespaces: [(&str, &[&str]); 5] = [
        ("noop-app", &["Alcohol", "Caffeine", "Illness", "Stress"]),
        ("my-whoop-noop", &["Alcohol", "Caffeine", "Illness"]),
        ("my-whoop", &["Alcohol", "Caffeine"]),
        ("noop-journal", &["Alcohol"]),
        ("aaa-import", &["Alcohol", "Caffeine", "Illness", "Stress"]),
    ];
    for (namespace, questions) in namespaces {
        let records: Vec<Value> = questions
            .iter()
            .map(|question| journal("2026-09-20", question, true, json!({"notes": namespace})))
            .collect();
        mirror
            .push(SOURCE, namespace, JOURNAL_DAYS, &records, REAL_PUSH_AT)
            .await;
    }

    let body = mirror.recovery_day("2026-09-20").await;

    let picked: Vec<(Value, Value)> = body["journal"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            assert_eq!(entry["source"], entry["notes"], "{entry}");
            (entry["question"].clone(), entry["source"].clone())
        })
        .collect();
    assert_eq!(
        picked,
        [
            (json!("Alcohol"), json!("noop-journal")),
            (json!("Caffeine"), json!(IMPORTED)),
            (json!("Illness"), json!(COMPUTED)),
            (json!("Stress"), json!("aaa-import")),
        ]
    );
}

#[tokio::test]
async fn journal_coverage_needs_an_applied_window_in_every_journal_namespace() {
    let mut mirror = covered_mirror(&[], &[], &[], &[]).await;
    let coverage = |body: &Value| body["noop"]["coverage"]["journal"].clone();

    mirror
        .push(SOURCE, "noop-journal", JOURNAL_DAYS, &[], REAL_PUSH_AT)
        .await;
    let covered = mirror.recovery_day("2026-09-20").await;
    assert_eq!(covered["journal"], json!([]));
    assert_eq!(coverage(&covered), "covered");
    assert_eq!(
        coverage(&mirror.recovery_day("2026-09-27").await),
        "unknown"
    );

    mirror
        .push_first_part(
            SOURCE,
            "noop-app",
            JOURNAL_DAYS,
            &[journal("2026-09-20", "Stress", true, json!({}))],
            2,
            REAL_PUSH_AT,
        )
        .await;
    let staged = mirror.recovery_day("2026-09-20").await;
    assert_eq!(staged["journal"], json!([]));
    assert_eq!(coverage(&staged), "unknown");

    mirror
        .push(
            SOURCE,
            "noop-app",
            Window::JournalDays("2026-09-20", "2026-09-21"),
            &[journal("2026-09-20", "Stress", true, json!({}))],
            REAL_PUSH_AT,
        )
        .await;
    let both = mirror.recovery_day("2026-09-20").await;
    assert_eq!(both["journal"][0]["source"], "noop-app");
    assert_eq!(coverage(&both), "covered");
    assert_eq!(
        coverage(&mirror.recovery_day("2026-09-19").await),
        "unknown"
    );
}

#[tokio::test]
async fn journal_pushes_do_not_change_recovery_freshness() {
    let mut mirror = real_mirror().await;
    for namespace in ["noop-journal", IMPORTED, COMPUTED] {
        mirror
            .push(
                SOURCE,
                namespace,
                Window::JournalDays("2026-09-20", "2026-09-28"),
                &[journal("2026-09-26", "Alcohol", false, json!({}))],
                "2026-09-28T04:00:00.000Z",
            )
            .await;
    }

    let body = mirror.recovery_day("2026-09-26").await;

    assert_eq!(body["journal"][0]["source"], "noop-journal");
    assert_eq!(body["noop"]["coverage"]["journal"], "covered");
    assert_eq!(body["noop"]["last_push_at"], "2026-09-26T03:51:38+02:00");
    assert_eq!(body["noop"]["freshness"], "partial");
}

#[tokio::test]
async fn rejects_a_day_over_the_journal_row_cap_instead_of_truncating() {
    let journal_mirror = |journal_rows: usize, app_rows: usize| async move {
        let answers = |count: usize| -> Vec<Value> {
            (0..count)
                .map(|index| journal("2026-09-20", &format!("Q{index:02}"), true, json!({})))
                .collect()
        };
        let mut mirror = covered_mirror(&[], &[], &[], &[]).await;
        mirror
            .push(
                SOURCE,
                "noop-journal",
                JOURNAL_DAYS,
                &answers(journal_rows),
                REAL_PUSH_AT,
            )
            .await;
        mirror
            .push(
                SOURCE,
                "noop-app",
                JOURNAL_DAYS,
                &answers(app_rows),
                REAL_PUSH_AT,
            )
            .await;
        mirror
    };

    let full = journal_mirror(32, 32).await;
    let body = full.recovery_day("2026-09-20").await;
    assert_eq!(body["journal"].as_array().unwrap().len(), 32);

    let over = journal_mirror(32, 33).await;
    let (status, body) = over.recovery("2026-09-20", Some(API_KEY)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body,
        json!({
            "error": "More than 64 Journal Entry rows are logged against 2026-09-20; \
                      refusing to return a truncated journal",
            "status": 422,
        })
    );
    assert_eq!(over.recovery_day("2026-09-19").await["journal"], json!([]));
}

#[tokio::test]
async fn rejects_a_sleep_night_over_the_session_cap_instead_of_truncating() {
    let naps = |count: i64| -> Vec<Value> {
        (0..count)
            .map(|index| {
                let start = SEP_20 + index * 600;
                sleep(start, start + 300, json!({"userEdited": index == 24}))
            })
            .collect()
    };

    let full = covered_mirror(&[], &[], &[], &naps(24)).await;
    let body = full.recovery_day("2026-09-20").await;
    assert_eq!(body["sleep"]["sessions"].as_array().unwrap().len(), 24);

    let over = covered_mirror(
        &[daily(
            "2026-09-20",
            json!({"totalSleepMin": 400.0, "efficiency": 0.8, "restingHr": 50.0}),
        )],
        &[daily(
            "2026-09-20",
            json!({"totalSleepMin": 430.0, "efficiency": 0.9, "restingHr": 40.0, "avgHrv": 70.0}),
        )],
        &[],
        &naps(25),
    )
    .await;
    let (status, body) = over.recovery("2026-09-20", Some(API_KEY)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body,
        json!({
            "error": "The Sleep Night waking on 2026-09-20 has more than 24 sleep sessions; \
                      refusing to return a truncated night",
            "status": 422,
        })
    );
    let previous = over.recovery_day("2026-09-19").await;
    assert_eq!(previous["sleep"]["sessions"], json!([]));

    let app = crate::mirror::test_app_with_noop_db(over.db).await;
    let (status, context) = crate::common::send_request(
        app.clone(),
        "/api/training/context/2026-09-20/2026-09-20",
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{context}");
    assert_eq!(
        context["days"][0]["sleep_duration"],
        measured(json!(430.0), "min", COMPUTED)
    );
    assert_eq!(
        context["days"][0]["resting_hr"],
        measured(json!(50.0), "beats/min", IMPORTED)
    );
    assert_eq!(
        context["days"][0]["hrv_rmssd"],
        measured(json!(70.0), "ms", COMPUTED)
    );

    for (metric, value, unit, source) in [
        ("sleep_duration", 430.0, "min", COMPUTED),
        ("resting_heart_rate", 50.0, "beats/min", IMPORTED),
        ("hrv", 70.0, "ms", COMPUTED),
    ] {
        let (status, trend) = crate::common::send_request(
            app.clone(),
            &format!("/api/training/trends/{metric}/2026-09-20/2026-09-20"),
            Some(API_KEY),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{metric}: {trend}");
        assert_eq!(
            trend["daily_points"],
            json!([{
                "day": "2026-09-20", "value": value, "unit": unit, "source": source,
                "freshness": "confirmed", "coverage": "covered",
            }]),
            "{metric}",
        );
    }
}
