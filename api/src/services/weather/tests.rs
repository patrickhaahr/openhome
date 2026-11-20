use super::*;
use chrono::Offset;
use chrono_tz::Europe::Copenhagen;
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, query_param},
};

mod support;
use support::{fixture, service};

#[tokio::test]
async fn serves_upcoming_hours_across_dst_with_gaps_and_end_timestamped_precipitation() {
    for instant in ["2026-03-29T00:30:00Z", "2026-10-25T00:30:00Z"] {
        let now: DateTime<Utc> = instant.parse().unwrap();
        let server = MockServer::start().await;
        let mut body = fixture(now);
        body["hourly"]["precipitation"][0] = json!(9.0);
        body["hourly"]["precipitation"][1] = json!(0.7);
        body["hourly"]["precipitation_probability"][1] = json!(60.0);
        body["hourly"]["temperature_2m"][1] = Value::Null;
        body["hourly"]["wind_speed_10m"][1] = json!(-1);
        body["hourly"]["precipitation_probability"][2] = json!(101);
        for field in [
            "time",
            "temperature_2m",
            "wind_speed_10m",
            "precipitation",
            "precipitation_probability",
        ] {
            body["hourly"][field].as_array_mut().unwrap().remove(3);
        }
        Mock::given(method("GET"))
            .and(query_param(
                "hourly",
                "temperature_2m,wind_speed_10m,precipitation,precipitation_probability",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let service = service(&server);
        let response = serde_json::to_value(service.read_at(now).await).unwrap();
        assert_eq!(response["state"], "fresh");
        let hours = response["forecast"]["hourly"].as_array().unwrap();
        assert_eq!(hours.len(), 24);
        assert_eq!(hours[0]["starts_at"], instant.replace("30:00", "00:00"));
        assert_eq!(hours[0]["temperature_c"], 12.5);
        assert_eq!(hours[0]["wind_ms"], 4.2);
        assert_eq!(hours[0]["precipitation_mm"], 0.7);
        assert_eq!(hours[0]["precipitation_probability_percent"], 60.0);
        assert_eq!(hours[1]["temperature_c"], Value::Null);
        assert_eq!(hours[1]["wind_ms"], Value::Null);
        assert_eq!(hours[1]["precipitation_probability_percent"], Value::Null);
        assert_eq!(hours[2]["precipitation_mm"], Value::Null);
        assert_eq!(hours[3]["temperature_c"], Value::Null);
        assert_eq!(hours[4]["temperature_c"], 12.5);
        server.verify().await;
        server.reset().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .expect(1)
            .mount(&server)
            .await;
        let stale =
            serde_json::to_value(service.read_at(now + chrono::Duration::hours(1)).await).unwrap();
        assert_eq!(stale["state"], "stale");
        assert_eq!(
            stale["forecast"]["fetched_at"],
            response["forecast"]["fetched_at"]
        );
        assert_eq!(stale["forecast"]["hourly"][0], hours[1]);
    }
}

#[tokio::test]
async fn normalizes_danish_calendar_days_across_dst_and_preserves_missing_values() {
    for instant in ["2026-03-28T23:30:00Z", "2026-10-24T22:30:00Z"] {
        let now: DateTime<Utc> = instant.parse().unwrap();
        let server = MockServer::start().await;
        let mut body = fixture(now);
        body["current"]["temperature_2m"] = Value::Null;
        body.as_object_mut().unwrap().remove("hourly");
        body.as_object_mut().unwrap().remove("hourly_units");
        body["daily"]["precipitation_sum"] = json!([null, 0.2]);
        Mock::given(method("GET"))
            .and(query_param("timezone", "Europe/Copenhagen"))
            .and(query_param("wind_speed_unit", "ms"))
            .and(query_param("timeformat", "unixtime"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let response = serde_json::to_value(service(&server).read_at(now).await).unwrap();
        assert_eq!(response["state"], "fresh");
        assert_eq!(response["forecast"]["hourly"].as_array().unwrap().len(), 24);
        assert_eq!(
            response["forecast"]["hourly"][0]["temperature_c"],
            Value::Null
        );
        assert_eq!(
            response["forecast"]["hourly"][0]["precipitation_mm"],
            Value::Null
        );
        assert_eq!(
            response["forecast"]["days"][0]["date"],
            if instant.starts_with("2026-03") {
                "2026-03-29"
            } else {
                "2026-10-25"
            }
        );
        assert_eq!(
            response["forecast"]["current"]["temperature_c"],
            Value::Null
        );
        assert_eq!(response["forecast"]["current"]["wind_ms"], 3.3);
        assert_eq!(
            response["forecast"]["days"][0]["precipitation_mm"],
            Value::Null
        );
        assert_eq!(response["forecast"]["days"][1]["precipitation_mm"], 0.2);
        assert_eq!(
            response["forecast"]["days"][2]["precipitation_mm"],
            Value::Null
        );
        assert_eq!(
            response["forecast"]["days"][6]["date"],
            if instant.starts_with("2026-03") {
                "2026-04-04"
            } else {
                "2026-10-31"
            }
        );
        assert!(response.to_string().find("latitude").is_none());
    }
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
    assert!(matches!(
        service.read_at(now + chrono::Duration::seconds(30)).await,
        WeatherRead::Unavailable { .. }
    ));
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    server.verify().await;
}

#[tokio::test]
async fn rejects_wrong_units_timezone_and_incomplete_days_without_inventing_weather() {
    let now: DateTime<Utc> = "2026-10-11T10:00:00Z".parse().unwrap();
    for (field, value) in [
        ("timezone", json!("UTC")),
        ("utc_offset_seconds", json!(0)),
        ("daily_units", json!({})),
        ("daily", json!({ "time": [] })),
        (
            "hourly_units",
            json!({ "temperature_2m": "°C", "wind_speed_10m": "mph", "precipitation": "mm", "precipitation_probability": "%" }),
        ),
        ("hourly", json!({ "time": [1, 1] })),
        ("hourly", json!({ "time": [i64::MAX] })),
        (
            "current_units",
            json!({ "temperature_2m": "°F", "apparent_temperature": "°F", "wind_speed_10m": "mph" }),
        ),
    ] {
        let server = MockServer::start().await;
        let mut body = fixture(now);
        body[field] = value;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let service = service(&server);
        assert!(matches!(
            service.read_at(now).await,
            WeatherRead::Unavailable { .. }
        ));
        assert!(matches!(
            service.read_at(now + chrono::Duration::seconds(59)).await,
            WeatherRead::Unavailable { .. }
        ));
    }
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
