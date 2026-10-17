mod common;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use openhome_api::{
    auth::{ApiKey, auth_middleware},
    routes,
    services::photos::{MAX_ALBUMS, MonthDay, PAGE_SIZE, PhotoService, copenhagen_today},
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
        "/api/photos/on-this-day?date=02-14".to_string(),
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

fn zoned(id: &str, taken: &str, zone: &str) -> Value {
    json!({"id":id,"type":"IMAGE","localDateTime":taken,"originalFileName":"private-name.jpg","exifInfo":{"timeZone":zone,"city":"private-city"}})
}
async fn on_this_day(app: &Router, date: &str) -> (StatusCode, Value, String) {
    let (status, _, body) = request(app, &format!("/api/photos/on-this-day?{date}"), true).await;
    let text = String::from_utf8(body).unwrap();
    (status, serde_json::from_str(&text).unwrap(), text)
}
#[tokio::test]
async fn on_this_day_matches_the_local_capture_day_in_earlier_years_only() {
    let server = MockServer::start().await;
    let app = app_with(Some(&server), &[ALBUM, SECOND, OTHER]).await;
    let this_year = chrono::Datelike::year(&copenhagen_today(chrono::Utc::now()));
    let id = |n: u128| Uuid::from_u128(n).to_string();
    mount(
        &server,
        ALBUM,
        json!({"id":ALBUM,"albumName":"Coast","assets":[
            // 00:30 in Copenhagen on 14 February: Immich did not know the zone, so UTC.
            dated(&id(1), "2024-02-13T23:30:00.000Z"),
            // 00:30 on 15 February in Copenhagen.
            dated(&id(2), "2024-02-14T23:30:00.000Z"),
            // Taken abroad late on 14 February; Immich knew the zone, so its wall clock counts.
            zoned(&id(3), "2025-02-14T23:30:00.000Z", "Asia/Tokyo"),
            dated(&id(4), &format!("{this_year}-02-14T12:00:00.000Z")),
            dated(&id(5), "2024-03-14T12:00:00.000Z"),
            {"id":id(6),"type":"IMAGE"},
            {"id":id(7),"type":"VIDEO","localDateTime":"2022-02-14T12:00:00.000Z"},
            dated(&id(8), "2021-02-14T12:00:00.000Z"),
        ]}),
    )
    .await;
    mount(
        &server,
        SECOND,
        json!({"id":SECOND,"albumName":"Garden","assets":[
            dated(&id(8), "2021-02-14T12:00:00.000Z"),
            dated(&id(9), "2023-02-14T08:00:00.000Z"),
        ]}),
    )
    .await;
    let (status, data, text) = on_this_day(&app, "date=02-14").await;
    assert_eq!(status, StatusCode::OK);
    for secret in [
        "private-name",
        "private-city",
        "Asia/Tokyo",
        "localDateTime",
        "23:30",
        "2024-02",
    ] {
        assert!(!text.contains(secret), "{secret} leaked");
    }
    let photos = &data["photos"];
    assert_eq!(photos["date"], "02-14");
    assert_eq!(photos["photo_count"], 4);
    assert_eq!(photos["next_page"], Value::Null);
    let items: Vec<_> = photos["items"].as_array().unwrap().iter().collect();
    for item in &items {
        let mut keys: Vec<_> = item.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["album", "asset", "year", "years_ago"]);
    }
    let found: Vec<_> = items
        .iter()
        .map(|item| {
            (
                item["album"].as_str().unwrap().to_owned(),
                item["asset"].as_str().unwrap().to_owned(),
                item["year"].as_i64().unwrap(),
                item["years_ago"].as_i64().unwrap(),
            )
        })
        .collect();
    let ago = |year: i64| i64::from(this_year) - year;
    assert_eq!(
        found,
        [
            (ALBUM.to_owned(), id(3), 2025, ago(2025)),
            (ALBUM.to_owned(), id(1), 2024, ago(2024)),
            (SECOND.to_owned(), id(9), 2023, ago(2023)),
            // Shared by both albums, named once by the first that grants access.
            (ALBUM.to_owned(), id(8), 2021, ago(2021)),
        ]
    );
    let (_, next_day, _) = on_this_day(&app, "date=02-15").await;
    assert_eq!(next_day["photos"]["items"][0]["asset"], id(2));
    assert_eq!(next_day["photos"]["photo_count"], 1);
    // Media still goes through the album that grants access.
    Mock::given(method("GET"))
        .and(path(format!("/api/assets/{}/thumbnail", id(9))))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "image/webp")
                .set_body_bytes(vec![1]),
        )
        .mount(&server)
        .await;
    let thumbnail = |album: &str| format!("/api/photos/albums/{album}/assets/{}/thumbnail", id(9));
    assert_eq!(
        request(&app, &thumbnail(SECOND), true).await.0,
        StatusCode::OK
    );
    assert_eq!(
        request(&app, &thumbnail(ALBUM), true).await.0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn on_this_day_checks_the_date_and_pages_within_the_gallery_bounds() {
    let server = MockServer::start().await;
    let app = app(Some(&server)).await;
    let assets: Vec<_> = (0..65u128)
        .map(|i| {
            dated(
                &Uuid::from_u128(i + 100).to_string(),
                &format!("{}-06-03T10:00:00.000Z", 1990 + i % 30),
            )
        })
        .collect();
    mount_album(&server, album(&assets)).await;
    for (page, count, next) in [(0, PAGE_SIZE, json!(1)), (1, 5, Value::Null)] {
        let (status, data, _) = on_this_day(&app, &format!("date=06-03&page={page}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(data["photos"]["photo_count"], 65);
        assert_eq!(data["photos"]["items"].as_array().unwrap().len(), count);
        assert_eq!(data["photos"]["next_page"], next);
    }
    let (status, data, _) = on_this_day(&app, "date=06-03&page=2").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(data["reason"], "invalid-page");
    for date in [
        "6-03", "06-3", "0603", "06-03x", "13-01", "00-10", "02-30", "04-31", "+6-03",
    ] {
        let (status, data, _) = on_this_day(&app, &format!("date={date}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{date}");
        assert_eq!(data["reason"], "invalid-date");
    }
    let (status, data, _) = on_this_day(&app, "date=02-29").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data["photos"]["photo_count"], 0);
    let today = copenhagen_today(chrono::Utc::now());
    let (status, data, _) = on_this_day(&app, "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data["photos"]["date"], today.format("%m-%d").to_string());
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "on this day reads the shared 30-second metadata cache"
    );
    let missing = app_with(None, &[ALBUM]).await;
    let (_, data, _) = on_this_day(&missing, "date=06-03").await;
    assert_eq!(data["reason"], "missing-configuration");
}
#[tokio::test]
async fn leap_day_photos_come_round_on_28_february_in_common_years() {
    let server = MockServer::start().await;
    let leap = Uuid::from_u128(29).to_string();
    let day_before = Uuid::from_u128(28).to_string();
    mount_album(
        &server,
        album(&[
            dated(&leap, "2024-02-29T12:00:00.000Z"),
            dated(&day_before, "2023-02-28T12:00:00.000Z"),
        ]),
    )
    .await;
    let service = PhotoService::new(
        server.uri().parse().unwrap(),
        "private-immich-key",
        vec![ALBUM.parse().unwrap()],
    )
    .unwrap();
    let date = |text: &str| chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap();
    let day = |text: &str| MonthDay::parse(text).unwrap();
    let assets = |page: openhome_api::services::photos::OnThisDayPage| {
        page.items
            .iter()
            .map(|item| (item.asset.to_string(), item.years_ago))
            .collect::<Vec<_>>()
    };
    let common = service
        .on_this_day(day("02-28"), date("2027-02-28"), 0)
        .await
        .unwrap();
    assert_eq!(assets(common), [(leap.clone(), 3), (day_before.clone(), 4)]);
    let leap_year = service
        .on_this_day(day("02-28"), date("2028-02-28"), 0)
        .await
        .unwrap();
    assert_eq!(assets(leap_year), [(day_before, 5)]);
    let leap_day = service
        .on_this_day(day("02-29"), date("2028-02-29"), 0)
        .await
        .unwrap();
    assert_eq!(assets(leap_day), [(leap, 4)]);
    let march = service
        .on_this_day(day("03-01"), date("2027-03-01"), 0)
        .await
        .unwrap();
    assert!(march.items.is_empty());
}
