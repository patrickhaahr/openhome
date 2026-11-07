//! Replay-safe Profile edits (`client_revision`): the contract the Android outbox retries against.

mod common;

use axum::Router;
use common::send_request_with_method;
use http::{Method, StatusCode};
use serde_json::{Value, json};

async fn patch(app: &Router, body: Value) -> (StatusCode, Value) {
    send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(body),
        Some("test-api-key"),
    )
    .await
}

async fn read(app: &Router) -> Value {
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body
}

fn edit(revision: i64, height_cm: f64, sex: Option<&str>) -> Value {
    json!({
        "client_revision": revision,
        "height_cm": height_cm,
        "sex": sex,
        "hr_max_bpm": 190,
        "birth_date": "1994-03-17"
    })
}

#[tokio::test]
async fn an_older_or_repeated_revision_is_acknowledged_and_changes_nothing() {
    let app = common::test_app().await;

    let (status, newest) = patch(&app, edit(5, 180.0, Some("male"))).await;
    assert_eq!(status, StatusCode::OK, "{newest}");
    assert_eq!(newest["client_revision"], 5);
    assert_eq!(newest["height_cm"], 180.0);

    for stale in [edit(5, 170.0, None), edit(4, 170.0, None)] {
        let (status, body) = patch(&app, stale).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body, newest);
    }
    assert_eq!(read(&app).await, newest);
}

#[tokio::test]
async fn a_newer_revision_applies_and_its_null_clears() {
    let app = common::test_app().await;
    patch(&app, edit(1, 180.0, Some("male"))).await;

    let (status, body) = patch(&app, edit(2, 181.5, None)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["height_cm"], 181.5);
    assert_eq!(body["sex"], Value::Null);
    assert_eq!(body["client_revision"], 2);
    assert_eq!(read(&app).await, body);
}

/// The first edit may create the row, and it still has to protect against what follows it.
#[tokio::test]
async fn the_first_edit_records_its_revision() {
    let app = common::test_app().await;

    patch(&app, edit(3, 180.0, None)).await;
    let (status, body) = patch(&app, edit(2, 170.0, None)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height_cm"], 180.0);
    assert_eq!(body["client_revision"], 3);
}

#[tokio::test]
async fn a_client_without_revisions_behaves_as_before_and_leaves_the_revision_alone() {
    let app = common::test_app().await;

    // Never having seen a revision, the shape it reads and writes is the existing one.
    let (status, body) = patch(&app, json!({ "height_cm": 175.0 })).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("client_revision").is_none(), "{body}");
    assert!(read(&app).await.get("client_revision").is_none());

    patch(&app, edit(4, 180.0, Some("male"))).await;
    let (status, body) = patch(&app, json!({ "height_cm": 182.0, "sex": null })).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height_cm"], 182.0);
    assert_eq!(body["sex"], Value::Null);
    assert_eq!(body["hr_max_bpm"], 190);
    assert_eq!(body["client_revision"], 4);
    let (_, stale) = patch(&app, edit(4, 170.0, None)).await;
    assert_eq!(stale["height_cm"], 182.0);
}

#[tokio::test]
async fn a_revision_below_one_or_invalid_values_are_refused_and_change_nothing() {
    let app = common::test_app().await;
    let (_, kept) = patch(&app, edit(1, 180.0, None)).await;

    for bad in [
        edit(0, 170.0, None),
        edit(-1, 170.0, None),
        edit(2, -1.0, None),
        json!({ "client_revision": 2, "hr_max_bpm": 99 }),
        json!({ "client_revision": 2, "birth_date": "1899-12-31" }),
    ] {
        let (status, body) = patch(&app, bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    assert_eq!(read(&app).await, kept);
}
