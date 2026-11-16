//! Fitness profile fields.

mod common;

use common::send_request_with_method;
use http::Method;
use http::StatusCode;
use serde_json::json;

// ---------------------------------------------------------------------------
// Profile (#8)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_profile_get_not_configured_then_patch_sets_fields() {
    let app = common::test_app().await;

    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height_cm"], serde_json::Value::Null);
    assert_eq!(body["sex"], serde_json::Value::Null);

    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(json!({ "height_cm": 180.5, "sex": "male" })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height_cm"], 180.5);
    assert_eq!(body["sex"], "male");

    let (status, fetched) =
        send_request_with_method(app, "/api/profile", Method::GET, None, Some("test-api-key"))
            .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["height_cm"], 180.5);
    assert_eq!(fetched["sex"], "male");
}

#[tokio::test]
async fn test_profile_patch_keeps_absent_fields() {
    let app = common::test_app().await;

    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(json!({ "height_cm": 180.5, "sex": "male" })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send_request_with_method(
        app,
        "/api/profile",
        Method::PATCH,
        Some(json!({ "height_cm": 181.0 })),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height_cm"], 181.0);
    assert_eq!(body["sex"], "male", "absent field must keep current value");
}

#[tokio::test]
async fn test_profile_patch_null_clears_fields() {
    let app = common::test_app().await;

    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(json!({ "height_cm": 180.5, "sex": "male" })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // explicit null clears sex; absent height_cm keeps its value
    let (status, body) = send_request_with_method(
        app,
        "/api/profile",
        Method::PATCH,
        Some(json!({ "sex": null })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["sex"], serde_json::Value::Null);
    assert_eq!(body["height_cm"], 180.5);
}

#[tokio::test]
async fn test_profile_requires_auth() {
    let app = common::test_app().await;

    let (status, _) = send_request_with_method(app, "/api/profile", Method::GET, None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_profile_hr_max_bpm_set_get_clear() {
    let app = common::test_app().await;

    let (_, body) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(body["hr_max_bpm"], serde_json::Value::Null);

    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(json!({ "hr_max_bpm": 190 })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["hr_max_bpm"], 190);

    let (_, fetched) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(fetched["hr_max_bpm"], 190);

    let (status, body) = send_request_with_method(
        app,
        "/api/profile",
        Method::PATCH,
        Some(json!({ "hr_max_bpm": null })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["hr_max_bpm"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_profile_hr_max_bpm_bounds() {
    for (value, expected) in [
        (99, StatusCode::BAD_REQUEST),
        (100, StatusCode::OK),
        (240, StatusCode::OK),
        (241, StatusCode::BAD_REQUEST),
    ] {
        let app = common::test_app().await;
        let (status, _) = send_request_with_method(
            app,
            "/api/profile",
            Method::PATCH,
            Some(json!({ "hr_max_bpm": value })),
            Some("test-api-key"),
        )
        .await;
        assert_eq!(status, expected, "hr_max_bpm {value}");
    }
}

#[tokio::test]
async fn test_profile_hr_max_bpm_independent_of_height_and_sex() {
    let app = common::test_app().await;

    send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(json!({ "height_cm": 180.5, "sex": "male", "hr_max_bpm": 190 })),
        Some("test-api-key"),
    )
    .await;

    let (_, body) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(json!({ "hr_max_bpm": 185 })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(body["hr_max_bpm"], 185);
    assert_eq!(body["height_cm"], 180.5);
    assert_eq!(body["sex"], "male");

    let (_, body) = send_request_with_method(
        app,
        "/api/profile",
        Method::PATCH,
        Some(json!({ "height_cm": 181.0, "sex": null })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(body["hr_max_bpm"], 185);
    assert_eq!(body["height_cm"], 181.0);
    assert_eq!(body["sex"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_profile_birth_date_set_kept_and_cleared_with_patch_semantics() {
    let app = common::test_app().await;
    let patch = |body: serde_json::Value| {
        send_request_with_method(
            app.clone(),
            "/api/profile",
            Method::PATCH,
            Some(body),
            Some("test-api-key"),
        )
    };

    let (_, body) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(body["birth_date"], serde_json::Value::Null);

    let (status, body) = patch(json!({ "birth_date": "1994-05-17" })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["birth_date"], "1994-05-17");

    let (_, body) = patch(json!({ "height_cm": 180.0 })).await;
    assert_eq!(
        body["birth_date"], "1994-05-17",
        "absent field must keep current value"
    );

    for invalid in [
        "1994-5-17",
        "17-05-1994",
        "1899-12-31",
        "2999-01-01",
        "yesterday",
    ] {
        let (status, _) = patch(json!({ "birth_date": invalid })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "birth_date {invalid}");
    }

    let (status, body) = patch(json!({ "birth_date": null })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["birth_date"], serde_json::Value::Null);
    assert_eq!(body["height_cm"], 180.0);
}
