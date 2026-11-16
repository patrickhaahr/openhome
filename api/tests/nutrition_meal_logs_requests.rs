//! Meal Log requests through the nutrition routes: retries, validation, updates, deletes and the
//! food database being unavailable, over a real `app.db` and a real `food.db`.

mod common;
#[path = "nutrition_support/meal_logs.rs"]
mod support;

use axum::{http::Method, http::StatusCode};
use common::{food_workbook::second_dataset, food_workbook::sha256, send_request_with_method};
use openhome_api::services::nutrition::food_database::DatasetPin;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Nutrition, app, at, breakfast, open, photos};

#[tokio::test]
async fn a_retried_request_id_replays_conflicts_on_changes_and_never_recreates_a_deleted_log() {
    let api = Nutrition::start().await;
    let meal_log = api.logged(breakfast("breakfast-1")).await;

    // The same instant written in UTC with the same note after trimming is the same request.
    let mut retry = breakfast("breakfast-1");
    retry["eaten_at"] = json!("2026-10-29T07:15:00Z");
    retry["note"] = json!("after the run");
    let (status, replayed) = api.log(retry).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed, json!({"meal_log": meal_log, "replayed": true}));

    let mut changed = breakfast("breakfast-1");
    changed["portions"][0]["grams"] = json!(200);
    let (status, body) = api.log(changed).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(api.day("2026-10-29").await.len(), 1);
    let (status, _) = api
        .send(
            Method::PATCH,
            &format!(
                "/api/nutrition/meal-logs/{}",
                meal_log["id"].as_str().unwrap()
            ),
            Some(json!({"note": "corrected later", "portions": [{"food_id": 1693, "grams": 200}]})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, replayed) = api.log(breakfast("breakfast-1")).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["meal_log"], meal_log);

    let id = meal_log["id"].as_str().unwrap().to_owned();
    let (status, _) = api
        .send(
            Method::DELETE,
            &format!("/api/nutrition/meal-logs/{id}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = api.log(breakfast("breakfast-1")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("deleted"),
        "{body}"
    );
    assert_eq!(api.day("2026-10-29").await, Vec::<Value>::new());

    // A rejected request consumes no request_id.
    let mut unknown = at("2026-10-29T09:00:00+01:00", "rejected-first");
    unknown["portions"][0]["food_id"] = json!(424_242);
    let (status, body) = api.log(unknown).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, _) = api
        .log(at("2026-10-29T09:00:00+01:00", "rejected-first"))
        .await;
    assert_eq!(status, StatusCode::CREATED);
}

/// A receipt that predates the stored original result (migration 0025) refuses its retry instead
/// of replaying a fabricated one, and the refusals keep the shared order: changed arguments,
/// then deletion, then the missing result.
#[tokio::test]
async fn a_legacy_receipt_directs_the_client_to_read_first_and_deletion_is_reported_before_it() {
    let api = Nutrition::start().await;
    let meal_log = api.logged(breakfast("breakfast-1")).await;
    sqlx::query("UPDATE meal_log_receipts SET result = NULL WHERE request_id = 'breakfast-1'")
        .execute(&api.state.db)
        .await
        .unwrap();

    let (status, body) = api.log(breakfast("breakfast-1")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("no original result"),
        "{body}"
    );

    let mut changed = breakfast("breakfast-1");
    changed["portions"][0]["grams"] = json!(200);
    let (status, body) = api.log(changed).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("a different Meal Log"),
        "{body}"
    );
    assert_eq!(api.day("2026-10-29").await.len(), 1);

    let (status, _) = api
        .send(
            Method::DELETE,
            &format!(
                "/api/nutrition/meal-logs/{}",
                meal_log["id"].as_str().unwrap()
            ),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = api.log(breakfast("breakfast-1")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("deleted"),
        "{body}"
    );
    assert!(
        !body["error"]
            .as_str()
            .unwrap()
            .contains("no original result"),
        "{body}"
    );
    assert_eq!(api.day("2026-10-29").await, Vec::<Value>::new());
}

#[tokio::test]
async fn invalid_requests_are_rejected_without_writing() {
    let api = Nutrition::start().await;
    let valid = at("2026-10-29T08:00:00+01:00", "invalid-1");
    let with = |field: &str, value: Value| {
        let mut body = valid.clone();
        body[field] = value;
        body
    };
    for body in [
        with("portions", json!([])),
        with("portions", json!([{"food_id": 1693, "grams": 0}])),
        with("portions", json!([{"food_id": 1693, "grams": -5}])),
        with("portions", json!([{"food_id": 1693, "grams": 5001}])),
        with("portions", json!([{"food_id": 0, "grams": 100}])),
        with("portions", json!([{"food_id": 1693}])),
        with(
            "portions",
            json!([{"food_id": 1693, "grams": 100, "kcal": 5}]),
        ),
        with("request_id", json!(" ")),
        with("note", json!("x".repeat(501))),
        with("calories", json!(500)),
    ] {
        let (status, response) = api.log(body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {response}");
        assert_eq!(response["status"], 400, "{response}");
    }
    for uri in [
        "/api/nutrition/meal-logs",
        "/api/nutrition/meal-logs?day=29-10-2026",
        "/api/nutrition/meal-logs?day=2026-10-29&from=2026-10-29&to=2026-10-29",
        "/api/nutrition/meal-logs?from=2026-10-29",
        "/api/nutrition/meal-logs?from=2026-10-29&to=2026-10-28",
        "/api/nutrition/meal-logs?from=2026-01-01&to=2026-10-29",
        "/api/nutrition/meal-logs?day=2026-10-29&owner=me",
    ] {
        let (status, response) = api.get(uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {response}");
    }
    assert_eq!(api.day("2026-10-29").await, Vec::<Value>::new());
    let (status, _) = send_request_with_method(
        api.app.clone(),
        "/api/nutrition/meal-logs?day=2026-10-29",
        Method::GET,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_update_changes_time_note_or_portions_and_a_delete_removes_the_log() {
    let mut api = Nutrition::start().await;
    let meal_log = api.logged(breakfast("breakfast-1")).await;
    let id = meal_log["id"].as_str().unwrap().to_owned();
    let uri = format!("/api/nutrition/meal-logs/{id}");

    // Moving it to the evening before moves it to that day; omitted fields stay.
    let (status, moved) = api
        .send(
            Method::PATCH,
            &uri,
            Some(json!({"eaten_at": "2026-10-28T21:00:00+01:00"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(
        (&moved["day"], &moved["note"], &moved["portions"]),
        (
            &json!("2026-10-28"),
            &meal_log["note"],
            &meal_log["portions"]
        )
    );
    assert_eq!(api.day("2026-10-29").await, Vec::<Value>::new());
    assert_eq!(api.day("2026-10-28").await, std::slice::from_ref(&moved));

    // Replacing Portions snapshots the current database; null clears the note.
    api.reimport("2", second_dataset()).await;
    let (status, corrected) = api
        .send(
            Method::PATCH,
            &uri,
            Some(json!({"note": null, "portions": [{"food_id": 1693, "grams": 200}]})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{corrected}");
    assert_eq!(corrected["note"], Value::Null);
    assert_eq!(corrected["eaten_at"], moved["eaten_at"]);
    assert_eq!(corrected["portions"].as_array().unwrap().len(), 1);
    assert_eq!(
        corrected["portions"][0]["nutrients"]["protein"]["value"],
        24.0
    );
    assert_eq!(corrected["totals"]["energy_kcal"]["value"], 124.0);

    for body in [
        json!({"eaten_at": null}),
        json!({"portions": null}),
        json!({"portions": []}),
        json!({"portions": [{"food_id": 1693, "grams": 0}]}),
        json!({"eaten_at": "2026-10-28 21:00"}),
        json!({"request_id": "x"}),
    ] {
        let (status, response) = api.send(Method::PATCH, &uri, Some(body.clone())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {response}");
    }
    let (status, unknown) = api
        .send(
            Method::PATCH,
            &uri,
            Some(json!({"portions": [{"food_id": 3, "grams": 100}]})),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a dropped Food: {unknown}");
    assert_eq!(api.day("2026-10-28").await, [corrected]);

    let (status, _) = api.send(Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(api.day("2026-10-28").await, Vec::<Value>::new());
    for method in [Method::DELETE, Method::PATCH] {
        let (status, body) = api
            .send(method.clone(), &uri, Some(json!({"note": "late"})))
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method}: {body}");
    }
}

#[tokio::test]
async fn while_the_food_database_is_unavailable_logs_stay_readable_and_editable() {
    let api = Nutrition::start().await;
    let meal_log = api.logged(breakfast("breakfast-1")).await;
    let id = meal_log["id"].as_str().unwrap().to_owned();
    // The next start re-imports a new pin; until it completes, Food lookups are unavailable.
    let pin = DatasetPin {
        version: "2".into(),
        url: format!("{}/files/never.xlsx", api.server.uri())
            .parse()
            .unwrap(),
        sha256: sha256(b"never served"),
    };
    let (foods, import) = open(&api.dir, pin).await;
    assert!(import.is_some());
    let importing = Nutrition {
        app: app(foods, photos(&api.dir), api.state.clone()),
        ..api
    };

    for (status, body) in [
        importing
            .log(at("2026-10-29T12:00:00+01:00", "lunch-1"))
            .await,
        importing.get("/api/nutrition/days/2026-10-29").await,
        importing
            .send(
                Method::PATCH,
                &format!("/api/nutrition/meal-logs/{id}"),
                Some(json!({"portions": [{"food_id": 1693, "grams": 10}]})),
            )
            .await,
    ] {
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(
            (&body["code"], &body["state"]),
            (&json!("food_database_unavailable"), &json!("importing"))
        );
    }
    assert_eq!(importing.day("2026-10-29").await, [meal_log]);
    let (status, edited) = importing
        .send(
            Method::PATCH,
            &format!("/api/nutrition/meal-logs/{id}"),
            Some(json!({"note": "rough estimate"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(edited["note"], "rough estimate");
}
