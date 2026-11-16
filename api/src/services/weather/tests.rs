use super::*;
use chrono::Offset;
use chrono_tz::Europe::Copenhagen;
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, query_param},
};

mod normalization;

fn fixture(now: DateTime<Utc>) -> Value {
    let mut body: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/weather-copenhagen.json"
    ))
    .unwrap();
    let first = now.with_timezone(&Copenhagen).date_naive();
    // Open-Meteo daily Unix timestamps use one fixed request offset, even across DST.
    let offset = now
        .with_timezone(&Copenhagen)
        .offset()
        .fix()
        .local_minus_utc();
    body["utc_offset_seconds"] = json!(offset);
    let first_timestamp =
        first.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() - i64::from(offset);
    body["daily"]["time"] = json!(
        (0..7)
            .map(|index| first_timestamp + index * 86_400)
            .collect::<Vec<_>>()
    );
    body["current"]["time"] = json!(now.timestamp());
    let start = now.timestamp().div_euclid(3600) * 3600;
    body["hourly_units"] = json!({
        "temperature_2m": "°C", "wind_speed_10m": "m/s",
        "precipitation": "mm", "precipitation_probability": "%"
    });
    body["hourly"] = json!({
        "time": (0..36).map(|i| start + i * 3600).collect::<Vec<_>>(),
        "temperature_2m": vec![Some(12.5); 36],
        "wind_speed_10m": vec![Some(4.2); 36],
        "precipitation": vec![Some(0.3); 36],
        "precipitation_probability": vec![Some(25.0); 36]
    });
    body
}

fn service(server: &MockServer) -> WeatherService {
    WeatherService::new(
        WeatherLocation::parse(Some("55.67"), Some("12.57"))
            .unwrap()
            .unwrap(),
        Url::parse(&server.uri()).unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn shares_refreshes_expires_at_ttl_backs_off_and_recovers() {
    let now: DateTime<Utc> = "2026-10-11T10:00:00Z".parse().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture(now)))
        .expect(1)
        .mount(&server)
        .await;
    let service = service(&server);
    let clone = service.clone();
    let (a, b) = tokio::join!(service.read_at(now), clone.read_at(now));
    assert!(matches!(a, WeatherRead::Fresh { .. }));
    assert!(matches!(b, WeatherRead::Fresh { .. }));
    assert!(matches!(
        service
            .read_at(now + TTL - chrono::Duration::seconds(1))
            .await,
        WeatherRead::Fresh { .. }
    ));
    server.verify().await;
    server.reset().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_body_string("synthetic-private-error"))
        .expect(1)
        .mount(&server)
        .await;
    let stale = serde_json::to_value(service.read_at(now + TTL).await).unwrap();
    assert_eq!(stale["state"], "stale");
    assert_eq!(stale["forecast"]["fetched_at"], "2026-10-11T10:00:00Z");
    assert_eq!(stale["forecast"]["valid_until"], "2026-10-11T10:15:00Z");
    assert!(matches!(
        service
            .read_at(now + TTL + chrono::Duration::seconds(59))
            .await,
        WeatherRead::Stale { .. }
    ));
    server.verify().await;
    server.reset().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture(now)))
        .expect(1)
        .mount(&server)
        .await;
    let fresh = serde_json::to_value(service.read_at(now + TTL + RETRY).await).unwrap();
    assert_eq!(fresh["state"], "fresh");
    assert_eq!(fresh["forecast"]["fetched_at"], "2026-10-11T10:16:00Z");
}

#[tokio::test]
async fn stops_serving_old_forecasts_after_six_hours() {
    let now: DateTime<Utc> = "2026-10-11T10:00:00Z".parse().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture(now)))
        .mount(&server)
        .await;
    let service = service(&server);
    service.read_at(now).await;
    server.reset().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    assert!(matches!(
        service
            .read_at(now + MAX_STALE - chrono::Duration::seconds(1))
            .await,
        WeatherRead::Stale { .. }
    ));
    // The stale cutoff falls inside the renewed backoff, so exactly one attempt
    // precedes both boundary reads and the cutoff itself is projected, not fetch.
    let response = serde_json::to_value(service.read_at(now + MAX_STALE).await).unwrap();
    assert_eq!(
        response,
        json!({ "state": "unavailable", "reason": "provider-unavailable" })
    );
    server.verify().await;
}

#[tokio::test]
async fn serves_unavailable_after_empty_cache_failures_and_recovers_at_the_retry_deadline() {
    let now: DateTime<Utc> = "2026-10-11T10:00:00Z".parse().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .expect(2)
        .mount(&server)
        .await;
    let service = service(&server);
    for instant in [
        now,
        now + chrono::Duration::seconds(30),
        now + RETRY - chrono::Duration::seconds(1),
    ] {
        assert_eq!(
            serde_json::to_value(service.read_at(instant).await).unwrap(),
            json!({ "state": "unavailable", "reason": "provider-unavailable" })
        );
    }
    // The first minute has one attempt; the next attempt happens exactly at the
    // deadline and fails again, so the cache stays empty instead of serving data.
    assert_eq!(
        serde_json::to_value(service.read_at(now + RETRY).await).unwrap(),
        json!({ "state": "unavailable", "reason": "provider-unavailable" })
    );
    server.verify().await;
    server.reset().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture(now)))
        .expect(1)
        .mount(&server)
        .await;
    let fresh = serde_json::to_value(service.read_at(now + RETRY + RETRY).await).unwrap();
    assert_eq!(fresh["state"], "fresh");
    assert_eq!(fresh["forecast"]["fetched_at"], "2026-10-11T10:02:00Z");
    server.verify().await;
}

#[tokio::test]
async fn coalesces_reads_queued_behind_a_delayed_failure_into_one_shared_backoff() {
    let now: DateTime<Utc> = "2026-10-11T10:00:00Z".parse().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_delay(std::time::Duration::from_millis(200)))
        .expect(1)
        .mount(&server)
        .await;
    let service = service(&server);
    let clone = service.clone();
    let (a, b) = tokio::join!(
        service.read_at(now),
        clone.read_at(now + chrono::Duration::seconds(30))
    );
    assert!(matches!(a, WeatherRead::Unavailable { .. }));
    assert!(matches!(b, WeatherRead::Unavailable { .. }));
    // The queued reader rechecks admission after the failed refresh, so reads still
    // inside the shared backoff never re-attempt despite their later sampled time.
    assert!(matches!(
        service
            .read_at(now + RETRY - chrono::Duration::seconds(1))
            .await,
        WeatherRead::Unavailable { .. }
    ));
    server.verify().await;
    server.reset().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture(now)))
        .expect(1)
        .mount(&server)
        .await;
    let fresh = serde_json::to_value(service.read_at(now + RETRY).await).unwrap();
    assert_eq!(fresh["state"], "fresh");
    assert_eq!(fresh["forecast"]["fetched_at"], "2026-10-11T10:01:00Z");
    server.verify().await;
}

#[tokio::test]
async fn serves_queued_reads_from_one_delayed_success_until_ttl_admits_a_refresh() {
    let now: DateTime<Utc> = "2026-10-11T10:00:00Z".parse().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture(now))
                .set_delay(std::time::Duration::from_millis(200)),
        )
        .expect(1)
        .mount(&server)
        .await;
    let service = service(&server);
    let clone = service.clone();
    let (a, b) = tokio::join!(
        service.read_at(now),
        clone.read_at(now + chrono::Duration::minutes(5))
    );
    assert!(matches!(a, WeatherRead::Fresh { .. }));
    assert!(matches!(b, WeatherRead::Fresh { .. }));
    server.verify().await;
    server.reset().await;
    // The queued reader rechecks admission against its own later time and reuses
    // the completed result; the refresh is re-admitted exactly at the new TTL.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture(now)))
        .expect(1)
        .mount(&server)
        .await;
    let fresh = serde_json::to_value(service.read_at(now + TTL).await).unwrap();
    assert_eq!(fresh["state"], "fresh");
    assert_eq!(fresh["forecast"]["fetched_at"], "2026-10-11T10:15:00Z");
    server.verify().await;
}

#[tokio::test]
async fn dropping_a_timed_out_read_leaves_refresh_admission_and_data_unchanged() {
    let now: DateTime<Utc> = "2026-10-11T10:00:00Z".parse().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture(now))
                .set_delay(std::time::Duration::from_millis(500)),
        )
        .mount(&server)
        .await;
    let service = service(&server);
    let attempt =
        tokio::time::timeout(std::time::Duration::from_millis(50), service.read_at(now)).await;
    assert!(attempt.is_err(), "the read must be waiting on the provider");
    // Cancellation drops the guard without completing a refresh, so the still
    // unattempted cache retries immediately; a stranded refresh or a backoff
    // invented by a cancelled attempt would leave the cache empty and skip it.
    let fresh = serde_json::to_value(service.read_at(now).await).unwrap();
    assert_eq!(fresh["state"], "fresh");
    assert_eq!(fresh["forecast"]["fetched_at"], "2026-10-11T10:00:00Z");
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    server.verify().await;
}

#[tokio::test]
async fn abandons_a_silent_provider_after_five_seconds_into_shared_backoff() {
    let now: DateTime<Utc> = "2026-10-11T10:00:00Z".parse().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixture(now))
                .set_delay(std::time::Duration::from_secs(6)),
        )
        .mount(&server)
        .await;
    let service = service(&server);
    let started = std::time::Instant::now();
    let response = serde_json::to_value(service.read_at(now).await).unwrap();
    let elapsed = started.elapsed();
    assert!(
        std::time::Duration::from_millis(4800) < elapsed
            && elapsed < std::time::Duration::from_millis(5600),
        "{elapsed:?}"
    );
    assert_eq!(
        response,
        json!({ "state": "unavailable", "reason": "provider-unavailable" })
    );
    // The timed-out attempt returns to the same backoff as any other failure.
    assert!(matches!(
        service.read_at(now + chrono::Duration::seconds(30)).await,
        WeatherRead::Unavailable { .. }
    ));
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    server.verify().await;
}

#[test]
fn validates_private_location_without_echoing_input() {
    assert!(WeatherLocation::parse(None, None).unwrap().is_none());
    for (lat, lon) in [
        (Some("secret-invalid"), Some("12")),
        (Some("NaN"), Some("12")),
        (Some("91"), Some("12")),
        (Some("55"), Some("181")),
        (Some("55"), None),
    ] {
        let error = WeatherLocation::parse(lat, lon).err().unwrap().to_string();
        assert!(!error.contains("secret-invalid"));
    }
}
