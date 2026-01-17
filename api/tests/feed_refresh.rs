mod common;

use common::test_app_with_db;
use rpi_api::services::feed;

#[tokio::test]
async fn test_should_return_error_for_invalid_feed_url() {
    let (_app, state) = test_app_with_db().await;

    let feed_id = sqlx::query_scalar!(
        r#"
        INSERT INTO feeds (url) VALUES ('https://example.com/invalid.xml')
        RETURNING id
        "#
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    let result =
        feed::refresh_feed(&state.db, feed_id, "https://example.invalid/invalid.xml").await;

    assert_eq!(result.feed_id, feed_id);
    assert_eq!(result.items_inserted, 0);
    assert_eq!(result.duplicates_skipped, 0);
    assert!(result.error.is_some());
}

#[tokio::test]
async fn test_should_reject_http_feed_url() {
    let (_app, state) = test_app_with_db().await;

    let feed_id = sqlx::query_scalar!(
        r#"
        INSERT INTO feeds (url) VALUES ('https://example.com/invalid.xml')
        RETURNING id
        "#
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    let result = feed::refresh_feed(&state.db, feed_id, "http://example.com/feed.xml").await;

    assert_eq!(result.feed_id, feed_id);
    assert_eq!(result.items_inserted, 0);
    assert_eq!(result.duplicates_skipped, 0);
    assert_eq!(result.error, Some("URL must use HTTPS scheme".to_string()));
}

#[tokio::test]
async fn test_should_reject_private_host_feed_url() {
    let (_app, state) = test_app_with_db().await;

    let feed_id = sqlx::query_scalar!(
        r#"
        INSERT INTO feeds (url) VALUES ('https://example.com/invalid.xml')
        RETURNING id
        "#
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    let result = feed::refresh_feed(&state.db, feed_id, "https://127.0.0.1/feed.xml").await;

    assert_eq!(result.feed_id, feed_id);
    assert_eq!(result.items_inserted, 0);
    assert_eq!(result.duplicates_skipped, 0);
    assert_eq!(
        result.error,
        Some("refusing to fetch non-global address".to_string())
    );
}
