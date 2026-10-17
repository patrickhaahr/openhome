mod common;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use openhome_api::{
    auth::{ApiKey, auth_middleware},
    routes,
    services::photos::{MAX_ALBUMS, PAGE_SIZE, PhotoService},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path, query_param},
};
const ALBUM: &str = "00000000-0000-4000-8000-000000000001";
const ASSET: &str = "00000000-0000-4000-8000-000000000002";
const OTHER: &str = "00000000-0000-4000-8000-000000000003";
const SECOND: &str = "00000000-0000-4000-8000-000000000004";
async fn app(server: Option<&MockServer>) -> Router {
    app_with(server, &[ALBUM]).await
}
async fn app_with(server: Option<&MockServer>, albums: &[&str]) -> Router {
    let (_, state) = common::test_app_with_db().await;
    let service = server.map(|server| {
        PhotoService::new(
            server.uri().parse().unwrap(),
            "private-immich-key",
            albums.iter().map(|id| id.parse().unwrap()).collect(),
        )
        .unwrap()
    });
    routes::photos::router(service)
        .with_state(state)
        .layer(axum::middleware::from_fn(|req, next| {
            auth_middleware(req, next, ApiKey::new("test-api-key".into()))
        }))
}
async fn request(
    app: &Router,
    path: &str,
    key: bool,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut req = Request::builder().uri(path);
    if key {
        req = req.header("authorization", "Bearer test-api-key");
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    (
        response.status(),
        response.headers().clone(),
        to_bytes(response.into_body(), 20 * 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
}
fn album(assets: &[Value]) -> Value {
    json!({"id":ALBUM,"albumName":"Demo places","assets":assets,"owner":{"name":"private-person"},"hasSharedLink":true})
}
fn photo(id: &str) -> Value {
    json!({"id":id,"type":"IMAGE","originalPath":"private/path","exifInfo":{"latitude":55.1},"people":["private-person"]})
}
async fn mount_album(server: &MockServer, value: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/api/albums/{ALBUM}")))
        .and(header("x-api-key", "private-immich-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(value))
        .mount(server)
        .await;
}
#[tokio::test]
async fn selected_albums_paginate_and_strip_private_provider_metadata() {
    let server = MockServer::start().await;
    let app = app(Some(&server)).await;
    let mut assets: Vec<_> = (0..65)
        .map(|i| photo(&Uuid::from_u128(i + 100).to_string()))
        .collect();
    assets.push(json!({"id":ASSET,"type":"VIDEO"}));
    assets.push(json!({"id":OTHER,"type":"IMAGE","isTrashed":true}));
    mount_album(&server, album(&assets)).await;
    let (status, headers, body) = request(&app, "/api/photos/albums", true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["cache-control"], "no-store");
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(data["albums"][0]["photo_count"], 65);
    let text = String::from_utf8(body).unwrap();
    for secret in [
        "private-person",
        "private/path",
        "latitude",
        "private-immich-key",
        "hasSharedLink",
    ] {
        assert!(!text.contains(secret));
    }
    for (page, count, next) in [(0, PAGE_SIZE, json!(1)), (1, 5, Value::Null)] {
        let (status, _, body) = request(
            &app,
            &format!("/api/photos/albums/{ALBUM}?page={page}"),
            true,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["photos"]["assets"].as_array().unwrap().len(), count);
        assert_eq!(body["photos"]["next_page"], next);
    }
    assert_eq!(
        request(&app, &format!("/api/photos/albums/{ALBUM}?page=2"), true)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "parallel gallery/media consumers share bounded metadata"
    );
}
fn dated(id: &str, taken: &str) -> Value {
    json!({"id":id,"type":"IMAGE","localDateTime":taken,"fileCreatedAt":taken,"originalFileName":"private-name.jpg","exifInfo":{"city":"private-city"}})
}
async fn mount(server: &MockServer, id: &str, value: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/api/albums/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(value))
        .mount(server)
        .await;
}
#[tokio::test]
async fn albums_carry_their_cover_and_months_and_merge_newest_first() {
    let server = MockServer::start().await;
    let app = app_with(Some(&server), &[ALBUM, SECOND, OTHER]).await;
    let shared = Uuid::from_u128(7).to_string();
    let newest = Uuid::from_u128(8).to_string();
    let undated = Uuid::from_u128(9).to_string();
    mount(
        &server,
        ALBUM,
        json!({"id":ALBUM,"albumName":"Coast","albumThumbnailAssetId":shared,"assets":[
            dated(ASSET, "2024-06-02T09:00:00.000Z"),
            dated(&shared, "2025-08-30T18:00:00.000Z"),
            {"id":undated,"type":"IMAGE"},
        ]}),
    )
    .await;
    mount(
        &server,
        SECOND,
        json!({"id":SECOND,"albumName":"Garden","albumThumbnailAssetId":OTHER,"assets":[
            dated(&shared, "2025-08-30T18:00:00.000Z"),
            dated(&newest, "2026-01-04T08:00:00.000Z"),
        ]}),
    )
    .await;
    // The third selected album was removed from Immich; it drops out of the merged view.
    let (_, _, body) = request(&app, "/api/photos/albums", true).await;
    let data: Value = serde_json::from_slice(&body).unwrap();
    let coast = &data["albums"][0];
    assert_eq!(coast["cover_id"], shared);
    assert_eq!(coast["first_month"], "2024-06");
    assert_eq!(coast["last_month"], "2025-08");
    assert_eq!(
        data["albums"][1]["cover_id"], shared,
        "a cover that is not a served still falls back to the first photo"
    );
    assert_eq!(data["albums"][2]["missing"], true);
    let (status, headers, body) = request(&app, "/api/photos/all?page=0", true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["cache-control"], "no-store");
    let text = String::from_utf8(body.clone()).unwrap();
    for secret in [
        "private-name",
        "private-city",
        "2025-08-30",
        "localDateTime",
    ] {
        assert!(!text.contains(secret));
    }
    let merged: Value = serde_json::from_slice(&body).unwrap();
    let order: Vec<_> = merged["photos"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["album"].as_str().unwrap().to_owned(),
                item["asset"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        order,
        [
            (SECOND.to_owned(), newest),
            (ALBUM.to_owned(), shared),
            (ALBUM.to_owned(), ASSET.to_owned()),
            (ALBUM.to_owned(), undated),
        ]
    );
    assert_eq!(merged["photos"]["photo_count"], 4);
    assert_eq!(merged["photos"]["next_page"], Value::Null);
    assert_eq!(
        request(&app, "/api/photos/all?page=1", true).await.0,
        StatusCode::BAD_REQUEST
    );
}
#[tokio::test]
async fn media_requires_auth_and_album_membership_and_never_serves_originals() {
    let server = MockServer::start().await;
    let app = app(Some(&server)).await;
    for url in [
        "/api/photos/albums".to_string(),
        "/api/photos/all".to_string(),
        format!("/api/photos/albums/{ALBUM}"),
        format!("/api/photos/albums/{ALBUM}/assets/{ASSET}/preview"),
    ] {
        assert_eq!(request(&app, &url, false).await.0, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        request(&app, &format!("/api/photos/albums/{OTHER}"), true)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert!(server.received_requests().await.unwrap().is_empty());
    mount_album(&server, album(&[photo(ASSET)])).await;
    for (size, content_type) in [("thumbnail", "image/webp"), ("preview", "image/jpeg")] {
        Mock::given(method("GET"))
            .and(path(format!("/api/assets/{ASSET}/thumbnail")))
            .and(query_param("size", size))
            .and(header("x-api-key", "private-immich-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", content_type)
                    .insert_header("x-private-secret", "private-immich-key")
                    .set_body_bytes(vec![1, 2, 3]),
            )
            .mount(&server)
            .await;
        let (status, headers, bytes) = request(
            &app,
            &format!("/api/photos/albums/{ALBUM}/assets/{ASSET}/{size}"),
            true,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["content-type"], content_type);
        assert_eq!(headers["cache-control"], "no-store");
        assert_eq!(headers["x-content-type-options"], "nosniff");
        assert!(!headers.contains_key("x-private-secret"));
        assert_eq!(bytes, vec![1, 2, 3]);
    }
    let count = server.received_requests().await.unwrap().len();
    for (album, asset, size) in [
        (ALBUM, OTHER, "preview"),
        (OTHER, ASSET, "preview"),
        (ALBUM, ASSET, "original"),
    ] {
        assert!(
            request(
                &app,
                &format!("/api/photos/albums/{album}/assets/{asset}/{size}"),
                true
            )
            .await
            .0
            .is_client_error()
        );
    }
    assert_eq!(server.received_requests().await.unwrap().len(), count);
}
#[tokio::test]
async fn missing_setup_albums_photos_and_provider_failures_are_distinct_and_sanitized() {
    let missing = app(None).await;
    let body = request(&missing, "/api/photos/albums", true).await.2;
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["reason"],
        "missing-configuration"
    );
    let server = MockServer::start().await;
    for (response, expected) in [
        (ResponseTemplate::new(404), "missing"),
        (
            ResponseTemplate::new(401).set_body_string("private-immich-key"),
            "provider-unavailable",
        ),
        (ResponseTemplate::new(500), "provider-unavailable"),
        (
            ResponseTemplate::new(200).set_body_string("malformed"),
            "provider-unavailable",
        ),
        (
            ResponseTemplate::new(302).insert_header("location", format!("{}/leak", server.uri())),
            "provider-unavailable",
        ),
        (
            ResponseTemplate::new(200)
                .set_body_json(json!({"id":OTHER,"albumName":"Wrong album","assets":[]})),
            "provider-unavailable",
        ),
        (
            ResponseTemplate::new(200).set_body_bytes(vec![0; 8 * 1024 * 1024 + 1]),
            "provider-unavailable",
        ),
    ] {
        server.reset().await;
        let app = app(Some(&server)).await;
        Mock::given(method("GET"))
            .respond_with(response)
            .mount(&server)
            .await;
        let (status, _, body) = request(&app, "/api/photos/albums", true).await;
        let data: Value = serde_json::from_slice(&body).unwrap();
        if expected == "missing" {
            assert_eq!(status, StatusCode::OK);
            assert_eq!(data["albums"][0]["missing"], true);
        } else {
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(data["reason"], expected);
        }
        assert!(
            !String::from_utf8(body)
                .unwrap()
                .contains("private-immich-key")
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
    server.reset().await;
    let app = app(Some(&server)).await;
    mount_album(&server, album(&[photo(ASSET)])).await;
    for response in [
        ResponseTemplate::new(404),
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/html")
            .set_body_string("private-immich-key"),
        ResponseTemplate::new(200)
            .insert_header("content-type", "image/webp")
            .set_body_bytes(vec![0; 2 * 1024 * 1024 + 1]),
    ] {
        server.reset().await;
        mount_album(&server, album(&[photo(ASSET)])).await;
        Mock::given(path(format!("/api/assets/{ASSET}/thumbnail")))
            .respond_with(response)
            .mount(&server)
            .await;
        let (status, _, body) = request(
            &app,
            &format!("/api/photos/albums/{ALBUM}/assets/{ASSET}/thumbnail"),
            true,
        )
        .await;
        assert!(status.is_client_error() || status.is_server_error());
        assert!(
            !String::from_utf8(body)
                .unwrap()
                .contains("private-immich-key")
        );
    }
}
#[test]
fn partial_or_unsafe_configuration_cannot_enable_a_library() {
    assert!(PhotoService::configure(None, None, None).unwrap().is_none());
    for (url, key, albums) in [
        (None, Some("key"), None),
        (Some("file:///tmp"), Some("key"), Some(ALBUM)),
        (
            Some("https://user:secret@private.invalid"),
            Some("key"),
            Some(ALBUM),
        ),
        (
            Some("https://private.invalid?key=secret"),
            Some("key"),
            Some(ALBUM),
        ),
        (
            Some("https://private.invalid"),
            Some("key"),
            Some("not-an-id"),
        ),
    ] {
        assert!(PhotoService::configure(url, key, albums).is_err());
    }
    let ids = |count: u128| {
        (1..=count)
            .map(|i| Uuid::from_u128(i).to_string())
            .collect::<Vec<_>>()
            .join(" , ")
    };
    let most = ids(MAX_ALBUMS as u128);
    assert!(
        PhotoService::configure(Some("https://private.invalid"), Some("key"), Some(&most))
            .unwrap()
            .is_some()
    );
    let too_many = ids(MAX_ALBUMS as u128 + 1);
    assert!(
        PhotoService::configure(
            Some("https://private.invalid"),
            Some("key"),
            Some(&too_many)
        )
        .is_err()
    );
}
