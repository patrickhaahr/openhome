use std::time::Duration;

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

    let body = read_body(fetch(&server.uri()).await).await.unwrap();

    assert_eq!(body.len(), 1024);
}

#[tokio::test]
async fn test_should_refuse_a_declared_oversized_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'a'; MAX_FEED_BYTES + 1]))
        .mount(&server)
        .await;

    let failure = read_body(fetch(&server.uri()).await).await.unwrap_err();

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
    let failure = tokio::time::timeout(Duration::from_secs(10), read_body(response))
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

    let failure = read_body(fetch(&format!("http://{address}")).await)
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
