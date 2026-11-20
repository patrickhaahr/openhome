mod common;
mod training_noop_workout_detail_support;

use axum::http::StatusCode;
use common::mirror::Mirror;
use common::mirror::noop_workout;
use common::reference_run;
use common::reference_run::COMPUTED;
use common::reference_run::IMPORTED;
use common::reference_run::PUSHED_AT;
use common::reference_run::SOURCE;
use common::reference_run::WORKOUT_WINDOW;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use training_noop_workout_detail_support::{
    API_KEY, app, detail, detail_of, mirror_with, workouts_on,
};

#[tokio::test]
async fn repeats_the_listed_workout_and_its_training_day_sync() {
    let app = app(&reference_run::mirror().await).await;
    let day = workouts_on(&app, "2026-09-27").await;
    let listed = &day["noop_workouts"][0];

    let body = detail_of(
        &app,
        listed["source"].as_str().unwrap(),
        listed["start"].as_str().unwrap(),
    )
    .await;
    for field in [
        "start",
        "end",
        "sport",
        "origin",
        "source",
        "duration_min",
        "distance_m",
        "energy_kcal",
        "strain_score",
    ] {
        assert_eq!(body[field], listed[field], "{field}");
    }
    assert_eq!(body["noop"], day["noop"]);
    assert!(!body.to_string().contains("route-stand-in"), "{body}");
}

#[tokio::test]
async fn start_is_matched_as_an_instant_whatever_its_offset() {
    let app = app(&reference_run::mirror().await).await;
    let listed = detail_of(&app, IMPORTED, reference_run::START).await;

    for start in [
        "2026-09-27T14:33:52Z",
        "2026-09-27T14:33:52.000Z",
        "2026-09-27T09:33:52-05:00",
    ] {
        assert_eq!(detail_of(&app, IMPORTED, start).await, listed, "{start}");
    }
}

#[tokio::test]
async fn opens_only_workouts_in_the_merged_list() {
    let manual = noop_workout(
        1_790_438_400,
        1_790_442_600,
        "Calisthenics",
        "manual",
        json!({"avgHr": 96.0}),
    );
    let mut mirror = Mirror::empty().await;
    mirror
        .push(
            SOURCE,
            IMPORTED,
            WORKOUT_WINDOW,
            std::slice::from_ref(&manual),
            PUSHED_AT,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            WORKOUT_WINDOW,
            &[
                manual.clone(),
                noop_workout(
                    1_790_437_800,
                    1_790_443_200,
                    "detected",
                    COMPUTED,
                    json!({"avgHr": 90.0}),
                ),
            ],
            PUSHED_AT,
        )
        .await;
    let app = app(&mirror).await;
    let listed = workouts_on(&app, "2026-09-26").await["noop_workouts"].clone();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["source"], IMPORTED);
    assert_eq!(listed[0]["start"], "2026-09-26T18:00:00+02:00");
    detail_of(&app, IMPORTED, "2026-09-26T18:00:00+02:00").await;

    for (source, start) in [
        (COMPUTED, "2026-09-26T18:00:00+02:00"),
        (COMPUTED, "2026-09-26T17:50:00+02:00"),
        (IMPORTED, "2026-09-26T18:00:01+02:00"),
        ("apple-health", "2026-09-26T18:00:00+02:00"),
    ] {
        let (status, body) = detail(&app, source, start, Some(API_KEY)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{source} {start}: {body}");
        assert_eq!(
            body,
            json!({
                "error": format!("No NOOP Workout from '{source}' starts at {start}"),
                "status": 404,
            })
        );
    }

    let (status, _) = detail(
        &common::test_app_with_noop_db(Mirror::empty().await.db).await,
        IMPORTED,
        "2026-09-26T18:00:00+02:00",
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn requires_the_api_key() {
    let app = app(&reference_run::mirror().await).await;
    for api_key in [None, Some("wrong-key")] {
        let (status, body) = detail(&app, IMPORTED, reference_run::START, api_key).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            body,
            json!({"error": "Missing or invalid API key", "status": 401})
        );
    }
}

#[tokio::test]
async fn rejects_a_start_that_is_not_rfc_3339() {
    let app = app(&reference_run::mirror().await).await;
    for start in [
        "2026-09-27",
        "2026-09-27T16:33:52",
        "2026-09-27T25:33:52+02:00",
        "1790519632",
        "2026-02-30T16:33:52+02:00",
    ] {
        let (status, body) = detail(&app, IMPORTED, start, Some(API_KEY)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{start}");
        assert_eq!(
            body,
            json!({
                "error": format!(
                    "Invalid start '{start}': must be an RFC 3339 timestamp with an offset, as \
                     workouts_on_day lists it"
                ),
                "status": 400,
            })
        );
    }
}

#[tokio::test]
async fn rejects_an_empty_or_malformed_source() {
    let app = app(&reference_run::mirror().await).await;
    for (source, shown) in [("", ""), ("my-whoop%0A", "my-whoop\n"), ("%09", "\t")] {
        let (status, body) = detail(&app, source, reference_run::START, Some(API_KEY)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{source}: {body}");
        assert_eq!(
            body,
            json!({
                "error": format!(
                    "Invalid source {shown:?}: must be a NOOP device namespace as \
                     workouts_on_day lists it"
                ),
                "status": 400,
            })
        );
    }
}

#[tokio::test]
async fn rejects_oversized_workouts_and_days_instead_of_truncating() {
    let long = noop_workout(1_790_402_400, 1_790_488_801, "Hiking", "manual", json!({}));
    let exactly_a_day = noop_workout(1_790_406_000, 1_790_492_400, "Walking", "manual", json!({}));
    let long_app = app(&mirror_with(&[long, exactly_a_day]).await).await;
    detail_of(&long_app, IMPORTED, "2026-09-26T09:00:00+02:00").await;
    let (status, body) = detail(
        &long_app,
        IMPORTED,
        "2026-09-26T08:00:00+02:00",
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body,
        json!({
            "error": "The NOOP Workout from 'my-whoop' starting at 2026-09-26T08:00:00+02:00 \
                      lasts more than 24 hours; refusing to read its heart rate",
            "status": 422,
        })
    );

    let crowded: Vec<Value> = (0..25)
        .map(|n| {
            let start = 1_790_373_600 + n * 3_000;
            noop_workout(start, start + 600, "Running", "manual", json!({}))
        })
        .collect();
    let crowded_app = app(&mirror_with(&crowded).await).await;
    let (status, body) = detail(
        &crowded_app,
        IMPORTED,
        "2026-09-26T00:00:00+02:00",
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body,
        json!({
            "error": "More than 24 NOOP workouts start on 2026-09-26; refusing to return a \
                      truncated day",
            "status": 422,
        })
    );
}

#[tokio::test]
async fn the_listed_sport_tells_apart_workouts_sharing_a_source_and_start() {
    let app = app(&mirror_with(&[
        noop_workout(
            1_790_398_800,
            1_790_401_500,
            "Running",
            "manual",
            json!({"distanceM": 7400.0}),
        ),
        noop_workout(
            1_790_398_800,
            1_790_402_400,
            "Cycling",
            "manual",
            json!({"distanceM": 20000.0}),
        ),
    ])
    .await)
    .await;
    let listed = workouts_on(&app, "2026-09-26").await["noop_workouts"].clone();
    let mut listed: Vec<(&str, &str, &str)> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|workout| {
            (
                workout["source"].as_str().unwrap(),
                workout["start"].as_str().unwrap(),
                workout["sport"].as_str().unwrap(),
            )
        })
        .collect();
    listed.sort_unstable();
    assert_eq!(
        listed,
        [
            (IMPORTED, "2026-09-26T07:00:00+02:00", "Cycling"),
            (IMPORTED, "2026-09-26T07:00:00+02:00", "Running"),
        ]
    );

    for (source, start, sport) in listed {
        let body = detail_of(&app, source, &format!("{start}/{sport}")).await;
        assert_eq!(body["sport"], sport);
        assert_eq!(body["start"], start);
    }

    let (status, body) = detail(&app, IMPORTED, "2026-09-26T07:00:00+02:00", Some(API_KEY)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body,
        json!({
            "error": "2 NOOP Workouts from 'my-whoop' start at 2026-09-26T07:00:00+02:00 \
                      (Cycling, Running); pass the listed sport to choose one",
            "status": 409,
        })
    );

    for sport in ["running", "RUNNING"] {
        let body = detail_of(
            &app,
            IMPORTED,
            &format!("2026-09-26T07:00:00+02:00/{sport}"),
        )
        .await;
        assert_eq!(body["sport"], "Running", "{sport}");
    }
    let (status, body) = detail(
        &app,
        IMPORTED,
        "2026-09-26T07:00:00+02:00/Walking",
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body["error"],
        "No Walking NOOP Workout from 'my-whoop' starts at 2026-09-26T07:00:00+02:00"
    );
}

#[tokio::test]
async fn an_exact_sport_wins_over_one_differing_only_in_case() {
    let app = app(&mirror_with(&[
        noop_workout(1_790_398_800, 1_790_398_800, "Running", "manual", json!({})),
        noop_workout(1_790_398_800, 1_790_398_800, "running", "manual", json!({})),
    ])
    .await)
    .await;

    for sport in ["Running", "running"] {
        let body = detail_of(
            &app,
            IMPORTED,
            &format!("2026-09-26T07:00:00+02:00/{sport}"),
        )
        .await;
        assert_eq!(body["sport"], sport);
    }
    let (status, body) = detail(
        &app,
        IMPORTED,
        "2026-09-26T07:00:00+02:00/RUNNING",
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"],
        "2 NOOP Workouts from 'my-whoop' start at 2026-09-26T07:00:00+02:00 (Running, running); \
         pass the listed sport to choose one"
    );
}

#[tokio::test]
async fn a_unique_workout_opens_with_or_without_its_sport() {
    let app = app(&reference_run::mirror().await).await;
    let without = detail_of(&app, IMPORTED, reference_run::START).await;
    let with = detail_of(&app, IMPORTED, &format!("{}/Running", reference_run::START)).await;
    assert_eq!(with, without);

    for (sport, shown) in [("%0A", "\n"), ("Run%09ning", "Run\tning")] {
        let (status, body) = detail(
            &app,
            IMPORTED,
            &format!("{}/{sport}", reference_run::START),
            Some(API_KEY),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{sport}: {body}");
        assert_eq!(
            body["error"],
            format!(
                "Invalid sport {shown:?}: must be a NOOP Workout sport as workouts_on_day lists it"
            )
        );
    }
}
