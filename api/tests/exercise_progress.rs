//! Combined Progress Lines through the authenticated fitness routes over a migrated database.

mod common;

use axum::Router;
use common::{send_request, send_request_with_method};
use http::{Method, StatusCode};
use serde_json::{Value, json};

const KEY: Option<&str> = Some("test-api-key");

async fn create_exercise(app: &Router, name: &str) -> i64 {
    let (status, exercise) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::POST,
        Some(json!({ "name": name, "category": "calisthenics" })),
        KEY,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "exercise {name}: {exercise}");
    exercise["id"].as_i64().unwrap()
}

/// Logs one Workout on `date` with one entry per `(exercise, sets)` pair.
async fn log(app: &Router, date: &str, entries: &[(i64, Value)]) {
    let exercises: Vec<Value> = entries
        .iter()
        .enumerate()
        .map(|(order, (id, sets))| json!({ "exercise_id": id, "order_index": order, "sets": sets }))
        .collect();
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::POST,
        Some(json!({ "date": date, "exercises": exercises })),
        KEY,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "workout {date}: {body}");
}

fn reps(values: &[i64]) -> Value {
    values
        .iter()
        .enumerate()
        .map(|(i, reps)| json!({ "set_number": i + 1, "reps": reps }))
        .collect()
}

fn holds(seconds: &[i64]) -> Value {
    seconds
        .iter()
        .enumerate()
        .map(|(i, s)| json!({ "set_number": i + 1, "duration_seconds": s }))
        .collect()
}

async fn progress(app: &Router, query: &str) -> (StatusCode, Value) {
    send_request(
        app.clone(),
        &format!("/api/exercises/progress?{query}"),
        KEY,
    )
    .await
}

fn line<'a>(body: &'a Value, name: &str) -> &'a Value {
    body["exercises"]
        .as_array()
        .unwrap()
        .iter()
        .find(|line| line["exercise"]["name"] == name)
        .unwrap_or_else(|| panic!("no line for {name}: {body}"))
}

/// `(date, raw value, index)` for every sample of a charted line.
fn samples(line: &Value) -> Vec<(String, i64, f64)> {
    line["samples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["date"].as_str().unwrap().to_owned(),
                s["value"].as_i64().unwrap(),
                (s["index"].as_f64().unwrap() * 100.0).round() / 100.0,
            )
        })
        .collect()
}

#[tokio::test]
async fn bodyweight_reps_index_best_set_to_first_day_keeping_gaps_and_declines() {
    let app = common::test_app().await;
    let pull_up = create_exercise(&app, "Synthetic Pull-up").await;
    // Best set per day: 8 (baseline), 10, then a decline to 6 after an unlogged week.
    log(&app, "2026-09-01", &[(pull_up, reps(&[8, 6, 5]))]).await;
    log(&app, "2026-09-03", &[(pull_up, reps(&[10, 7]))]).await;
    log(&app, "2026-09-12", &[(pull_up, reps(&[6]))]).await;

    let (status, body) = progress(
        &app,
        &format!("from=2026-09-01&to=2026-09-30&exercise_ids={pull_up}"),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["from"], "2026-09-01");
    assert_eq!(body["to"], "2026-09-30");
    let line = line(&body, "Synthetic Pull-up");
    assert_eq!(line["status"], "charted");
    assert_eq!(line["metric"], "repetitions");
    assert_eq!(line["unit"], "reps");
    assert_eq!(
        line["baseline"],
        json!({ "date": "2026-09-01", "value": 8 })
    );
    assert_eq!(
        samples(line),
        vec![
            ("2026-09-01".to_owned(), 8, 100.0),
            ("2026-09-03".to_owned(), 10, 125.0),
            ("2026-09-12".to_owned(), 6, 75.0),
        ]
    );
}

#[tokio::test]
async fn timed_holds_index_longest_hold_and_show_plateaus() {
    let app = common::test_app().await;
    let hold = create_exercise(&app, "Synthetic L-sit").await;
    log(&app, "2026-09-02", &[(hold, holds(&[20, 12]))]).await;
    log(&app, "2026-09-04", &[(hold, holds(&[15, 30]))]).await;
    log(&app, "2026-09-06", &[(hold, holds(&[30]))]).await;

    let (status, body) = progress(
        &app,
        &format!("from=2026-09-01&to=2026-09-30&exercise_ids={hold}"),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let line = line(&body, "Synthetic L-sit");
    assert_eq!(line["metric"], "hold_seconds");
    assert_eq!(line["unit"], "s");
    assert_eq!(
        line["baseline"],
        json!({ "date": "2026-09-02", "value": 20 })
    );
    assert_eq!(
        samples(line),
        vec![
            ("2026-09-02".to_owned(), 20, 100.0),
            ("2026-09-04".to_owned(), 30, 150.0),
            ("2026-09-06".to_owned(), 30, 150.0),
        ]
    );
}

#[tokio::test]
async fn baseline_is_first_positive_day_and_earlier_zero_stays_observed() {
    let app = common::test_app().await;
    let dip = create_exercise(&app, "Synthetic Dip").await;
    log(&app, "2026-09-01", &[(dip, reps(&[0]))]).await;
    log(&app, "2026-09-02", &[(dip, reps(&[4]))]).await;
    log(&app, "2026-09-05", &[(dip, reps(&[5]))]).await;
    let zero = create_exercise(&app, "Synthetic Muscle-up").await;
    log(&app, "2026-09-03", &[(zero, reps(&[0, 0]))]).await;

    let (_, body) = progress(
        &app,
        &format!("from=2026-09-01&to=2026-09-30&exercise_ids={dip},{zero}"),
    )
    .await;

    let dip = line(&body, "Synthetic Dip");
    assert_eq!(dip["baseline"], json!({ "date": "2026-09-02", "value": 4 }));
    assert_eq!(
        samples(dip),
        vec![
            ("2026-09-01".to_owned(), 0, 0.0),
            ("2026-09-02".to_owned(), 4, 100.0),
            ("2026-09-05".to_owned(), 5, 125.0),
        ]
    );
    let zero = line(&body, "Synthetic Muscle-up");
    assert_eq!(zero["status"], "unsupported");
    assert_eq!(zero["reason"], "no_positive_value");
    assert!(zero.get("samples").is_none());
}

#[tokio::test]
async fn weighted_mixed_missing_and_never_logged_exercises_have_explicit_outcomes() {
    let app = common::test_app().await;
    let weighted = create_exercise(&app, "Synthetic Weighted Dip").await;
    let mixed = create_exercise(&app, "Synthetic Handstand").await;
    let earlier = create_exercise(&app, "Synthetic Pistol Squat").await;
    let never = create_exercise(&app, "Synthetic Dragon Flag").await;
    let zero_weight = create_exercise(&app, "Synthetic Chin-up").await;
    log(
        &app,
        "2026-09-02",
        &[
            (
                weighted,
                json!([
                    { "set_number": 1, "reps": 12 },
                    { "set_number": 2, "reps": 5, "weight_kg": 10.0 }
                ]),
            ),
            (mixed, holds(&[30])),
            (
                zero_weight,
                json!([{ "set_number": 1, "reps": 7, "weight_kg": 0.0 }]),
            ),
        ],
    )
    .await;
    log(&app, "2026-09-04", &[(mixed, reps(&[3]))]).await;
    log(&app, "2026-08-01", &[(earlier, reps(&[9]))]).await;

    let (status, body) = progress(
        &app,
        &format!(
            "from=2026-09-01&to=2026-09-30&exercise_ids={never},{weighted},{mixed},{earlier},{zero_weight}"
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let names: Vec<&str> = body["exercises"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line["exercise"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "Synthetic Dragon Flag",
            "Synthetic Weighted Dip",
            "Synthetic Handstand",
            "Synthetic Pistol Squat",
            "Synthetic Chin-up"
        ],
        "request order is kept"
    );
    let status_of = |name| {
        let line = line(&body, name);
        (line["status"].clone(), line["reason"].clone())
    };
    assert_eq!(
        status_of("Synthetic Weighted Dip"),
        (json!("charted"), Value::Null)
    );
    assert_eq!(
        status_of("Synthetic Handstand"),
        (json!("unsupported"), json!("mixed_set_kinds"))
    );
    assert_eq!(
        status_of("Synthetic Pistol Squat"),
        (json!("not_logged_in_range"), Value::Null)
    );
    assert_eq!(
        status_of("Synthetic Dragon Flag"),
        (json!("never_logged"), Value::Null)
    );
    // A zero Added Weight adds nothing, so the Set is bodyweight work.
    assert_eq!(
        line(&body, "Synthetic Chin-up")["metric"],
        json!("repetitions")
    );
    assert_eq!(body["logged_exercise_count"], 3);
}

#[tokio::test]
async fn weighted_estimates_select_valid_sets_preserve_declines_and_leave_old_progress_unchanged() {
    let app = common::test_app().await;
    let lift = create_exercise(&app, "Synthetic Press").await;
    for (date, sets) in [
        ("2026-09-01", json!([{ "set_number": 1, "reps": 20 }])),
        (
            "2026-09-02",
            json!([
                { "set_number": 1, "reps": 10, "weight_kg": 60 },
                { "set_number": 2, "reps": 5, "weight_kg": 70 },
                { "set_number": 3, "reps": 11, "weight_kg": 100 },
                { "set_number": 4, "reps": 0, "weight_kg": 200 }
            ]),
        ),
        (
            "2026-09-03",
            json!([{ "set_number": 1, "reps": 1, "weight_kg": 90 }]),
        ),
        (
            "2026-09-04",
            json!([{ "set_number": 1, "reps": 20, "weight_kg": 100 }]),
        ),
        (
            "2026-09-22",
            json!([{ "set_number": 1, "reps": 10, "weight_kg": 45 }]),
        ),
    ] {
        log(&app, date, &[(lift, sets)]).await;
    }
    let (status, body) = progress(
        &app,
        &format!("from=2026-09-01&to=2026-09-30&exercise_ids={lift}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let lift_line = line(&body, "Synthetic Press");
    assert_eq!(lift_line["metric"], "estimated_1rm");
    assert_eq!(lift_line["unit"], "kg");
    assert_eq!(
        lift_line["omitted_dates"],
        json!(["2026-09-01", "2026-09-04"])
    );
    let values = lift_line["samples"].as_array().unwrap();
    assert_eq!(values.len(), 3);
    assert!((lift_line["baseline"]["value"].as_f64().unwrap() - 81.666_666_666_7).abs() < 1e-8);
    for (sample, date, value, index) in [
        (&values[0], "2026-09-02", 81.666_666_666_7, 100.0),
        (&values[1], "2026-09-03", 90.0, 110.204_081_633),
        (&values[2], "2026-09-22", 60.0, 73.469_387_755),
    ] {
        assert_eq!(sample["date"], date);
        assert!((sample["value"].as_f64().unwrap() - value).abs() < 1e-8);
        assert!((sample["index"].as_f64().unwrap() - index).abs() < 1e-8);
    }

    // The older endpoint still estimates all positive-rep weighted sets using its original Epley rule.
    let (_, old) = send_request(app, &format!("/api/exercises/{lift}/progress"), KEY).await;
    let old = old["data"].as_array().unwrap();
    assert!((old[1]["estimated_1rm_kg"].as_f64().unwrap() - 200.0).abs() < 1e-8);
    assert!((old[2]["estimated_1rm_kg"].as_f64().unwrap() - 93.0).abs() < 1e-8);
}

#[tokio::test]
async fn overflowing_estimates_do_not_hide_valid_sets_on_the_same_day() {
    let app = common::test_app().await;
    let lift = create_exercise(&app, "Synthetic Overflow Press").await;
    log(
        &app,
        "2026-09-02",
        &[(
            lift,
            json!([
                { "set_number": 1, "reps": 5, "weight_kg": 70 },
                { "set_number": 2, "reps": 10, "weight_kg": 1.7e308 }
            ]),
        )],
    )
    .await;
    log(
        &app,
        "2026-09-03",
        &[(
            lift,
            json!([
                { "set_number": 1, "reps": 10, "weight_kg": 1.7e308 }
            ]),
        )],
    )
    .await;
    let (status, body) = progress(
        &app,
        &format!("from=2026-09-01&to=2026-09-30&exercise_ids={lift}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let lift = line(&body, "Synthetic Overflow Press");
    assert_eq!(lift["status"], "charted");
    assert!((lift["baseline"]["value"].as_f64().unwrap() - 81.666_666_666_7).abs() < 1e-8);
    assert_eq!(lift["samples"].as_array().unwrap().len(), 1);
    assert_eq!(lift["omitted_dates"], json!(["2026-09-03"]));
}

#[tokio::test]
async fn weighted_holds_mixed_kinds_and_out_of_range_estimates_are_unsupported() {
    let app = common::test_app().await;
    for (name, sets, reason) in [
        (
            "Synthetic Loaded Hold",
            json!([{ "set_number": 1, "duration_seconds": 30, "weight_kg": 10 }]),
            "weighted_holds",
        ),
        (
            "Synthetic Mixed Lift",
            json!([
                { "set_number": 1, "reps": 5, "weight_kg": 40 },
                { "set_number": 2, "duration_seconds": 30 }
            ]),
            "mixed_set_kinds",
        ),
        (
            "Synthetic High Reps",
            json!([
                { "set_number": 1, "reps": 0, "weight_kg": 40 },
                { "set_number": 2, "reps": 11, "weight_kg": 40 }
            ]),
            "no_valid_estimate",
        ),
    ] {
        let id = create_exercise(&app, name).await;
        log(&app, "2026-09-02", &[(id, sets)]).await;
        let (_, body) = progress(
            &app,
            &format!("from=2026-09-01&to=2026-09-30&exercise_ids={id}"),
        )
        .await;
        assert_eq!(line(&body, name)["reason"], reason);
        assert!(line(&body, name).get("samples").is_none());
    }
}

#[tokio::test]
async fn default_selection_is_the_most_recently_logged_exercises_up_to_the_bound() {
    let app = common::test_app().await;
    let mut ids = Vec::new();
    for n in 0..25 {
        ids.push(create_exercise(&app, &format!("Synthetic Move {n:02}")).await);
    }
    let older: Vec<(i64, Value)> = ids[..24].iter().map(|id| (*id, reps(&[5]))).collect();
    log(&app, "2026-09-01", &older).await;
    log(&app, "2026-09-03", &[(ids[24], reps(&[5]))]).await;
    // Outside the period: not part of the default selection or its count.
    let outside = create_exercise(&app, "Synthetic Outside").await;
    log(&app, "2026-10-02", &[(outside, reps(&[5]))]).await;

    let (status, body) = progress(&app, "from=2026-09-01&to=2026-09-30").await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["logged_exercise_count"], 25);
    let lines = body["exercises"].as_array().unwrap();
    assert_eq!(lines.len(), 24, "at most 24 lines per read");
    assert_eq!(lines[0]["exercise"]["name"], "Synthetic Move 24");
    assert_eq!(lines[1]["exercise"]["name"], "Synthetic Move 00");
    assert_eq!(lines[23]["exercise"]["name"], "Synthetic Move 22");
    assert!(lines.iter().all(|line| line["status"] == "charted"));
}

#[tokio::test]
async fn batch_bounds_are_enforced() {
    let app = common::test_app().await;
    let ids: Vec<String> = (1..=25).map(|id| id.to_string()).collect();
    let cases = [
        ("to=2026-09-30", StatusCode::BAD_REQUEST),
        ("from=2026-09-30&to=2026-09-01", StatusCode::BAD_REQUEST),
        ("from=2025-09-29&to=2026-09-30", StatusCode::BAD_REQUEST),
        ("from=2025-09-30&to=2026-09-30", StatusCode::OK),
        ("from=2026-9-01&to=2026-09-30", StatusCode::BAD_REQUEST),
        (
            "from=2026-09-01&to=2026-09-30&exercise_ids=",
            StatusCode::BAD_REQUEST,
        ),
        (
            "from=2026-09-01&to=2026-09-30&exercise_ids=1,1",
            StatusCode::BAD_REQUEST,
        ),
        (
            "from=2026-09-01&to=2026-09-30&exercise_ids=1,x",
            StatusCode::BAD_REQUEST,
        ),
        (
            "from=2026-09-01&to=2026-09-30&exercise_ids=99999",
            StatusCode::NOT_FOUND,
        ),
    ];
    for (query, expected) in cases {
        let (status, body) = progress(&app, query).await;
        assert_eq!(status, expected, "{query}: {body}");
    }

    let (status, body) = progress(
        &app,
        &format!(
            "from=2026-09-01&to=2026-09-30&exercise_ids={}",
            ids.join(",")
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "25 ids: {body}");
    let (status, body) = progress(
        &app,
        &format!(
            "from=2026-09-01&to=2026-09-30&exercise_ids={}",
            ids[..24].join(",")
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "24 seeded ids: {body}");
    assert_eq!(body["exercises"].as_array().unwrap().len(), 24);
}

#[tokio::test]
async fn combined_progress_requires_auth() {
    let app = common::test_app().await;

    let (status, _) = send_request(
        app,
        "/api/exercises/progress?from=2026-09-01&to=2026-09-30",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
