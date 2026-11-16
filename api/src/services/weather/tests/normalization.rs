use super::*;

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
        // Drop a timestamp and every corresponding value, preserving a true gap.
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
