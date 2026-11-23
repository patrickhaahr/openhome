use std::time::Duration;

use sqlx::sqlite::SqlitePoolOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;

async fn fetch(url: &str) -> reqwest::Response {
    reqwest::get(url).await.unwrap()
}

#[tokio::test]
async fn test_should_read_body_within_the_cap() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'a'; 1024]))
        .mount(&server)
        .await;

    let body = read_body(1, fetch(&server.uri()).await).await.unwrap();

    assert_eq!(body.len(), 1024);
}

#[tokio::test]
async fn test_should_refuse_a_declared_oversized_body_before_any_body_arrives() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await;
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n",
            MAX_FEED_BYTES + 1
        );
        socket.write_all(head.as_bytes()).await.unwrap();
        tokio::time::sleep(Duration::from_secs(60)).await;
    });

    let response = fetch(&format!("http://{address}")).await;
    let failure = tokio::time::timeout(Duration::from_secs(5), read_body(1, response))
        .await
        .expect("a declared oversized body must be refused without reading it")
        .unwrap_err();

    assert_eq!(failure, FeedFailure::TooLarge);
}

#[tokio::test]
async fn test_should_stop_reading_an_endless_chunked_body_at_the_cap() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n")
            .await
            .unwrap();
        let chunk = [b"10000\r\n".as_slice(), &vec![b'a'; 0x10000], b"\r\n"].concat();
        while socket.write_all(&chunk).await.is_ok() {}
    });

    let response = fetch(&format!("http://{address}")).await;
    let failure = tokio::time::timeout(Duration::from_secs(10), read_body(1, response))
        .await
        .expect("an endless body must be cut off at the cap")
        .unwrap_err();

    assert_eq!(failure, FeedFailure::TooLarge);
}

#[tokio::test]
async fn test_should_report_a_dropped_connection_as_unavailable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\n\r\nshort")
            .await
            .unwrap();
    });

    let failure = read_body(1, fetch(&format!("http://{address}")).await)
        .await
        .unwrap_err();

    assert_eq!(failure, FeedFailure::Unavailable);
}

#[test]
fn test_should_name_failures_for_storage_and_clients() {
    let names: Vec<_> = [
        FeedFailure::Unavailable,
        FeedFailure::TooLarge,
        FeedFailure::NotAFeed,
    ]
    .iter()
    .map(|failure| serde_json::to_string(failure).unwrap())
    .collect();

    assert_eq!(
        names,
        ["\"unavailable\"", "\"too_large\"", "\"not_a_feed\""]
    );
}

const SEEDED_ETAG: &str = "\"seeded\"";
const SEEDED_MODIFIED: &str = "Mon, 01 Jan 2024 00:00:00 GMT";

struct Stored {
    last_error: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
}

async fn ingest_stored(response: Response) -> (FeedResult, Stored) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO feeds (id, url, etag, last_modified, last_error)
         VALUES (1, 'https://example.com/feed.xml', ?, ?, 'stale')",
    )
    .bind(SEEDED_ETAG)
    .bind(SEEDED_MODIFIED)
    .execute(&pool)
    .await
    .unwrap();
    let mut result = FeedResult {
        feed_id: 1,
        url: String::new(),
        items_inserted: 0,
        duplicates_skipped: 0,
        error: None,
    };

    ingest_response(&pool, response, &mut result).await;

    let (last_error, etag, last_modified) =
        sqlx::query_as("SELECT last_error, etag, last_modified FROM feeds WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    (
        result,
        Stored {
            last_error,
            etag,
            last_modified,
        },
    )
}

fn assert_failure_stored(result: &FeedResult, stored: &Stored, expected: FeedFailure) {
    assert_eq!(result.error, Some(expected));
    assert_eq!(stored.last_error.as_deref(), Some(expected.as_str()));
    assert_eq!(stored.etag.as_deref(), Some(SEEDED_ETAG));
    assert_eq!(stored.last_modified.as_deref(), Some(SEEDED_MODIFIED));
}

#[tokio::test]
async fn test_should_store_too_large_when_the_body_is_over_the_cap() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "\"fresh\"")
                .set_body_bytes(vec![b'a'; MAX_FEED_BYTES + 1]),
        )
        .mount(&server)
        .await;

    let (result, stored) = ingest_stored(fetch(&server.uri()).await).await;

    assert_failure_stored(&result, &stored, FeedFailure::TooLarge);
}

#[tokio::test]
async fn test_should_store_unavailable_when_the_body_is_truncated() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\n\r\nshort")
            .await
            .unwrap();
    });

    let (result, stored) = ingest_stored(fetch(&format!("http://{address}")).await).await;

    assert_failure_stored(&result, &stored, FeedFailure::Unavailable);
}

#[tokio::test]
async fn test_should_store_not_a_feed_when_the_body_does_not_parse() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not a feed"))
        .mount(&server)
        .await;

    let (result, stored) = ingest_stored(fetch(&server.uri()).await).await;

    assert_failure_stored(&result, &stored, FeedFailure::NotAFeed);
}

#[tokio::test]
async fn test_should_store_unavailable_when_the_status_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let (result, stored) = ingest_stored(fetch(&server.uri()).await).await;

    assert_failure_stored(&result, &stored, FeedFailure::Unavailable);
}

#[tokio::test]
async fn test_should_describe_a_request_failure_without_the_feed_url() {
    let error = reqwest::get("http://127.0.0.1:1/feed.xml?token=feed-secret")
        .await
        .unwrap_err();

    let description = describe_without_url(error);

    assert!(!description.contains("feed-secret"), "{description}");
    assert!(!description.contains("feed.xml"), "{description}");
}
