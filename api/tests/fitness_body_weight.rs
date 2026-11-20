mod common;

use common::send_request_with_method;
use http::Method;
use http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn test_body_weight_record_and_list_sorted_by_date() {
    let app = common::test_app().await;

    for (date, weight) in [
        ("2026-09-05", 75.5),
        ("2026-09-01", 76.0),
        ("2026-09-03", 75.2),
    ] {
        let (status, body) = send_request_with_method(
            app.clone(),
            "/api/body_weight",
            Method::POST,
            Some(json!({ "date": date, "weight_kg": weight })),
            Some("test-api-key"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "post {date}: {body}");
        assert_eq!(body["date"], date);
        assert_eq!(body["weight_kg"], weight);
        assert!(body["id"].is_number());
    }

    let (status, list) = send_request_with_method(
        app.clone(),
        "/api/body_weight",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let entries = list.as_array().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["date"], "2026-09-01");
    assert_eq!(entries[1]["date"], "2026-09-03");
    assert_eq!(entries[2]["date"], "2026-09-05");
    assert_eq!(entries[0]["weight_kg"], 76.0);

    let (status, filtered) = send_request_with_method(
        app,
        "/api/body_weight?from=2026-09-02&to=2026-09-04",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let entries = filtered.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["date"], "2026-09-03");
}

#[tokio::test]
async fn test_body_weight_ships_empty_and_conflicts_on_duplicate_date() {
    let app = common::test_app().await;

    let (status, list) = send_request_with_method(
        app.clone(),
        "/api/body_weight",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.as_array().unwrap().is_empty());

    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/body_weight",
        Method::POST,
        Some(json!({ "date": "2026-09-05", "weight_kg": 75.5 })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, response) = send_request_with_method(
        app,
        "/api/body_weight",
        Method::POST,
        Some(json!({ "date": "2026-09-05", "weight_kg": 74.0 })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(response["error"].as_str().unwrap().contains("2026-09-05"));
}

#[tokio::test]
async fn test_body_weight_rejects_bad_input() {
    let app = common::test_app().await;

    let (status, response) = send_request_with_method(
        app.clone(),
        "/api/body_weight",
        Method::POST,
        Some(json!({ "date": "09/05/2026", "weight_kg": 75.0 })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(response["error"].as_str().unwrap().contains("YYYY-MM-DD"));

    let (status, _) = send_request_with_method(
        app,
        "/api/body_weight",
        Method::POST,
        Some(json!({ "date": "2026-09-05" })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_body_weight_requires_auth() {
    let app = common::test_app().await;

    let (status, _) =
        send_request_with_method(app, "/api/body_weight", Method::GET, None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
