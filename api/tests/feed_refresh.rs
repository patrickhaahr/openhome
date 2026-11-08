mod common;

use common::test_app_with_db;
use openhome_api::services::feed;

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

/// Adds one source entry the way a parsed refresh would: identified per Feed by its guid.
async fn store_feed_item(
    pool: &sqlx::SqlitePool,
    feed_id: i64,
    link: &str,
) -> sqlx::Result<sqlx::sqlite::SqliteQueryResult> {
    sqlx::query!(
        r#"
        INSERT INTO feed_items (feed_id, title, description, link, guid, pub_date)
        VALUES ($1, 'Feed item', NULL, $2, $2, NULL)
        "#,
        feed_id,
        link
    )
    .execute(pool)
    .await
}

/// Deduplication against already stored items rests on the `feed_items` schema: the same item is
/// stored once per Feed, while another Feed may store it.
#[tokio::test]
async fn test_feed_items_are_deduplicated_per_feed() {
    let (_app, state) = test_app_with_db().await;

    let feed_id = sqlx::query_scalar!(
        r#"
        INSERT INTO feeds (url) VALUES ('https://example.com/first.xml')
        RETURNING id
        "#
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    let other_feed_id = sqlx::query_scalar!(
        r#"
        INSERT INTO feeds (url) VALUES ('https://example.com/second.xml')
        RETURNING id
        "#
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    store_feed_item(&state.db, feed_id, "https://example.com/item-1")
        .await
        .unwrap();

    let duplicate = store_feed_item(&state.db, feed_id, "https://example.com/item-1")
        .await
        .unwrap_err();
    match duplicate {
        sqlx::Error::Database(db_err) => assert!(db_err.is_unique_violation()),
        other => panic!("expected a duplicate-guid violation, got {other:?}"),
    }

    store_feed_item(&state.db, other_feed_id, "https://example.com/item-1")
        .await
        .unwrap();
}
