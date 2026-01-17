mod common;

use common::{send_request_with_method, test_app_with_db};
use http::{Method, StatusCode};
use serde_json::{Value, json};

#[tokio::test]
async fn test_should_get_timeline_compact_items_with_limit() {
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

    let expected_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM feed_items ORDER BY (pub_date IS NULL) ASC, pub_date DESC, id DESC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap();

    let (status, response) = send_request_with_method(
        app,
        "/api/timeline?view=compact&limit=2",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let items = response.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], json!(expected_ids[0]));
    assert_eq!(items[0]["title"], json!("Item 0"));
    assert_eq!(items[0]["description"], json!("Desc"));
    assert!(items[0].get("feed_id").is_none());
    assert!(items[0].get("feed_title").is_none());
    assert!(items[0].get("link").is_none());
    assert!(items[0].get("pub_date").is_none());
    assert!(items[0].get("read_at").is_none());
}

#[tokio::test]
async fn test_should_paginate_timeline_compact_with_before_id() {
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

    for i in 0..5 {
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

    let expected_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM feed_items ORDER BY (pub_date IS NULL) ASC, pub_date DESC, id DESC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap();

    let (status, first_page) = send_request_with_method(
        app.clone(),
        "/api/timeline?view=compact&limit=2",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let first_items = first_page.as_array().unwrap();
    assert_eq!(first_items.len(), 2);
    assert_eq!(first_items[0]["id"], json!(expected_ids[0]));
    assert_eq!(first_items[1]["id"], json!(expected_ids[1]));

    let second_item_id = first_items[1]["id"].as_i64().unwrap();
    let (status, second_page) = send_request_with_method(
        app.clone(),
        &format!(
            "/api/timeline?view=compact&limit=2&before_id={}",
            second_item_id
        ),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let second_items = second_page.as_array().unwrap();
    assert_eq!(second_items.len(), 2);
    assert_eq!(second_items[0]["id"], json!(expected_ids[2]));
    assert_eq!(second_items[1]["id"], json!(expected_ids[3]));

    assert_ne!(first_items[0]["id"], second_items[0]["id"]);
    assert_ne!(first_items[1]["id"], second_items[1]["id"]);
}

#[tokio::test]
async fn test_should_return_422_for_invalid_before_id_compact_view() {
    let app = common::test_app().await;

    let (status, response) = send_request_with_method(
        app,
        "/api/timeline?view=compact&before_id=9999",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response["error"], "before_id 9999 does not exist");
}

#[tokio::test]
async fn test_should_filter_unread_items_in_compact_view() {
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
        VALUES ($1, 'Read Item', 'Desc', $2, $3, datetime('now'), datetime('now'))
        "#,
        feed_id,
        "https://example.com/read",
        "guid-read"
    )
    .execute(&state.db)
    .await
    .unwrap();

    sqlx::query!(
        r#"
        INSERT INTO feed_items (feed_id, title, description, link, guid, pub_date)
        VALUES ($1, 'Unread Item', 'Desc', $2, $3, datetime('now'))
        "#,
        feed_id,
        "https://example.com/unread",
        "guid-unread"
    )
    .execute(&state.db)
    .await
    .unwrap();

    let (status, response) = send_request_with_method(
        app,
        "/api/timeline?view=compact&unread=true",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let items = response.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], json!("Unread Item"));
}

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
async fn test_should_return_422_for_invalid_before_id_full_view() {
    let app = common::test_app().await;

    let (status, response) = send_request_with_method(
        app,
        "/api/timeline?before_id=9999",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response["error"], "before_id 9999 does not exist");
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
