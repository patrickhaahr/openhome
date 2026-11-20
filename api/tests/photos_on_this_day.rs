mod common;
mod photos_support;

use axum::Router;
use axum::http::StatusCode;
use openhome_api::services::photos::MonthDay;
use openhome_api::services::photos::PAGE_SIZE;
use openhome_api::services::photos::PhotoService;
use openhome_api::services::photos::copenhagen_today;
use photos_support::{
    ALBUM, OTHER, SECOND, album, app, app_with, dated, mount, mount_album, request,
};
use serde_json::Value;
use serde_json::json;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

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
            dated(&id(1), "2024-02-13T23:30:00.000Z"),
            dated(&id(2), "2024-02-14T23:30:00.000Z"),
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
            (ALBUM.to_owned(), id(8), 2021, ago(2021)),
        ]
    );
    let (_, next_day, _) = on_this_day(&app, "date=02-15").await;
    assert_eq!(next_day["photos"]["items"][0]["asset"], id(2));
    assert_eq!(next_day["photos"]["photo_count"], 1);
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
