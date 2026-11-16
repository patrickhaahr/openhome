use super::*;

pub(super) fn fixture(now: DateTime<Utc>) -> Value {
    let mut body: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/weather-copenhagen.json"
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

pub(super) fn service(server: &MockServer) -> WeatherService {
    WeatherService::new(
        WeatherLocation::parse(Some("55.67"), Some("12.57"))
            .unwrap()
            .unwrap(),
        Url::parse(&server.uri()).unwrap(),
    )
    .unwrap()
}
