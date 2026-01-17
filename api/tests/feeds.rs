mod common;

use common::{send_request_with_method, test_app_with_db};
use http::{Method, StatusCode};
use serde_json::json;

#[tokio::test]
async fn test_should_create_feed_and_return_201() {
    let app = common::test_app().await;

    let body = json!({
        "url": "https://example.com/feed.xml"
    });

    let (status, response) = send_request_with_method(
        app,
        "/api/feeds",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(response["url"], "https://example.com/feed.xml");
    assert_eq!(response["title"], serde_json::Value::Null);
    assert!(response["id"].is_number());
}

#[tokio::test]
async fn test_should_return_409_for_duplicate_feed_url() {
    let (app, _state) = test_app_with_db().await;

    let body = json!({
        "url": "https://example.com/feed.xml"
    });

    let (_status, _response) = send_request_with_method(
        app.clone(),
        "/api/feeds",
        Method::POST,
        Some(body.clone()),
        Some("test-api-key"),
    )
    .await;

    let (status, response) = send_request_with_method(
        app,
        "/api/feeds",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(response["error"], "Feed with this URL already exists");
    assert_eq!(response["status"], 409);
}

#[tokio::test]
async fn test_should_return_400_for_invalid_url() {
    let app = common::test_app().await;

    let body = json!({
        "url": "not-a-valid-url"
    });

    let (status, response) = send_request_with_method(
        app,
        "/api/feeds",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response["error"], "Invalid URL: relative URL without a base");
}

#[tokio::test]
async fn test_should_delete_feed_and_cascade_feed_items() {
    let (app, state) = test_app_with_db().await;

    let create_body = json!({
        "url": "https://example.com/feed.xml"
    });

    let (status, create_response) = send_request_with_method(
        app.clone(),
        "/api/feeds",
        Method::POST,
        Some(create_body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    let feed_id = create_response["id"].as_i64().unwrap();

    sqlx::query!(
        r#"
        INSERT INTO feed_items (feed_id, title, description, link, guid, pub_date)
        VALUES ($1, 'Test Item', 'Test Description', 'https://example.com/item', 'guid-1', datetime('now'))
        "#,
        feed_id
    )
    .execute(&state.db)
    .await
    .unwrap();

    let count_before: i64 =
        sqlx::query_scalar!("SELECT COUNT(*) FROM feed_items WHERE feed_id = ?", feed_id)
            .fetch_one(&state.db)
            .await
            .unwrap();

    assert_eq!(count_before, 1);

    let (status, _) = send_request_with_method(
        app,
        &format!("/api/feeds/{}", feed_id),
        Method::DELETE,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT);

    let count_after: Option<i64> =
        sqlx::query_scalar!("SELECT COUNT(*) FROM feed_items WHERE feed_id = ?", feed_id)
            .fetch_optional(&state.db)
            .await
            .unwrap();

    assert_eq!(count_after, Some(0));
}

#[tokio::test]
async fn test_should_return_404_when_deleting_nonexistent_feed() {
    let app = common::test_app().await;

    let (status, response) = send_request_with_method(
        app,
        "/api/feeds/99999",
        Method::DELETE,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(response["error"], "Feed with id 99999 not found");
}

#[tokio::test]
async fn test_should_get_feeds() {
    let feed1_body = json!({ "url": "https://example.com/feed1.xml" });
    let feed2_body = json!({ "url": "https://example.com/feed2.xml" });

    let (app, _state) = test_app_with_db().await;
    let (_status, _response) = send_request_with_method(
        app.clone(),
        "/api/feeds",
        Method::POST,
        Some(feed1_body),
        Some("test-api-key"),
    )
    .await;

    let (_status, _response) = send_request_with_method(
        app.clone(),
        "/api/feeds",
        Method::POST,
        Some(feed2_body),
        Some("test-api-key"),
    )
    .await;

    let (status, response) =
        send_request_with_method(app, "/api/feeds", Method::GET, None, Some("test-api-key")).await;

    assert_eq!(status, StatusCode::OK);
    let feeds = response.as_array().unwrap();
    assert_eq!(feeds.len(), 2);
    let urls: Vec<&str> = feeds
        .iter()
        .filter_map(|feed| feed["url"].as_str())
        .collect();
    assert!(urls.contains(&"https://example.com/feed1.xml"));
    assert!(urls.contains(&"https://example.com/feed2.xml"));
}
