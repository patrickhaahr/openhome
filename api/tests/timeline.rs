mod common;

use common::{send_request_with_method, test_app_with_db};
use http::{Method, StatusCode};
use serde_json::{Value, json};

#[tokio::test]
async fn test_should_get_timeline_items_with_limit() {
    let (app, state) = test_app_with_db().await;

    let feed_id = sqlx::query_scalar!(
        r#"
        INSERT INTO feeds (url, title) VALUES ('https://example.com/feed.xml', 'Example Feed')
        RETURNING id
        "#
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    for i in 0..3 {
        let title = format!("Item {i}");
        let link = format!("https://example.com/item/{i}");
        let guid = format!("guid-{i}");
        sqlx::query!(
            r#"
            INSERT INTO feed_items (feed_id, title, description, link, guid, pub_date)
            VALUES ($1, $2, 'Desc', $3, $4, datetime('now', '-' || $5 || ' minutes'))
            "#,
            feed_id,
            title,
            link,
            guid,
            i
        )
        .execute(&state.db)
        .await
        .unwrap();
    }

    let (status, response) = send_request_with_method(
        app,
        "/api/timeline?limit=2",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let items = response.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["feed_id"], json!(feed_id));
    assert_eq!(items[0]["feed_title"], json!("Example Feed"));
}

#[tokio::test]
async fn test_should_filter_unread_items_only() {
    let (app, state) = test_app_with_db().await;

    let feed_id = sqlx::query_scalar!(
        r#"
        INSERT INTO feeds (url, title) VALUES ('https://example.com/feed.xml', 'Unread Feed')
        RETURNING id
        "#
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    sqlx::query!(
        r#"
        INSERT INTO feed_items (feed_id, title, description, link, guid, pub_date, read_at)
        VALUES ($1, 'Read Item', 'Desc', 'https://example.com/read', 'guid-read', datetime('now'), datetime('now'))
        "#,
        feed_id
    )
    .execute(&state.db)
    .await
    .unwrap();

    sqlx::query!(
        r#"
        INSERT INTO feed_items (feed_id, title, description, link, guid, pub_date)
        VALUES ($1, 'Unread Item', 'Desc', 'https://example.com/unread', 'guid-unread', datetime('now'))
        "#,
        feed_id
    )
    .execute(&state.db)
    .await
    .unwrap();

    let (status, response) = send_request_with_method(
        app,
        "/api/timeline?unread=true",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let items = response.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], json!("Unread Item"));
    assert!(items[0]["read_at"].is_null());
}

#[tokio::test]
async fn test_should_mark_item_as_read() {
    let (app, state) = test_app_with_db().await;

    let feed_id = sqlx::query_scalar!(
        r#"
        INSERT INTO feeds (url, title) VALUES ('https://example.com/feed.xml', 'Mark Read Feed')
        RETURNING id
        "#
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    let item_id = sqlx::query_scalar!(
        r#"
        INSERT INTO feed_items (feed_id, title, description, link, guid, pub_date)
        VALUES ($1, 'Unread Item', 'Desc', 'https://example.com/unread', 'guid-unread', datetime('now'))
        RETURNING id
        "#,
        feed_id
    )
    .fetch_one(&state.db)
    .await
    .unwrap()
    .expect("id should be returned");

    let (status, response) = send_request_with_method(
        app.clone(),
        &format!("/api/items/{item_id}/read"),
        Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(response, Value::Null);

    let read_at: Option<String> = sqlx::query_scalar!(
        "SELECT CAST(read_at AS TEXT) as read_at FROM feed_items WHERE id = ?",
        item_id
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    assert!(read_at.is_some());
}

#[tokio::test]
async fn test_should_return_404_when_marking_unknown_item() {
    let app = common::test_app().await;

    let (status, response) = send_request_with_method(
        app,
        "/api/items/99999/read",
        Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        response["error"],
        "Item with id 99999 not found or already read"
    );
}
