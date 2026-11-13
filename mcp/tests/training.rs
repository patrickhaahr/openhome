mod common;

use common::{
    API_KEY, api_get, api_get_read, api_get_sleep_recent, call_day_tool, call_sleep_recent,
    connect, recovery_on_day, start_api, start_mcp,
};
use pretty_assertions::assert_eq;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};

#[tokio::test]
async fn metric_trend_passes_through_daily_points_and_api_errors() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let call = |metric: &str, to_day: &str| {
        CallToolRequestParams::new("metric_trend").with_arguments(
            json!({"metric": metric, "from_day": "2026-09-14", "to_day": to_day})
                .as_object()
                .unwrap()
                .clone(),
        )
    };
    let direct = |metric: &str, to_day: &str| {
        format!("{api}/api/training/trends/{metric}/2026-09-14/{to_day}")
    };
    let http = reqwest::Client::new();
    let result = client
        .call_tool(call("body_weight", "2026-09-15"))
        .await
        .unwrap();
    let api_result: Value = http
        .get(direct("body_weight", "2026-09-15"))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content, Some(api_result.clone()));
    assert_eq!(api_result["daily_points"][0]["value"], Value::Null);
    assert_eq!(api_result["weekly_summaries"][0]["observed_days"], 0);

    let invalid = client
        .call_tool(call("body_weight", "2026-12-15"))
        .await
        .unwrap();
    let api_error: Value = http
        .get(direct("body_weight", "2026-12-15"))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(invalid.is_error, Some(true));
    assert_eq!(invalid.structured_content, Some(api_error));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn training_context_passes_through_days_and_api_errors() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let call = |to_day: &str| {
        CallToolRequestParams::new("training_context").with_arguments(
            json!({"from_day": "2026-09-13", "to_day": to_day})
                .as_object()
                .unwrap()
                .clone(),
        )
    };
    let direct = |to_day: &str| {
        let url = format!("{api}/api/training/context/2026-09-13/{to_day}");
        async move {
            reqwest::Client::new()
                .get(url)
                .bearer_auth(API_KEY)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };

    let result = client.call_tool(call("2026-09-14")).await.unwrap();
    let api_result = direct("2026-09-14").await;
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content, Some(api_result.clone()));
    assert_eq!(api_result["days"][0]["workouts"], json!([]));
    assert_eq!(
        api_result["days"][1]["workouts"][0]["exercises"][0]["best_added_weight_kg"],
        12.5
    );
    assert_eq!(
        api_result["days"][1]["noop_workouts"][0]["start"],
        "2026-09-14T18:00:00+02:00"
    );
    assert_eq!(
        api_result["days"][1]["noop"]["recovery"]["freshness"],
        "confirmed"
    );
    assert_eq!(
        api_result["days"][1]["noop"]["workouts"],
        json!({"freshness": "confirmed", "coverage": {"workouts": "covered"}})
    );

    let too_long = client.call_tool(call("2026-12-31")).await.unwrap();
    assert_eq!(too_long.is_error, Some(true));
    assert_eq!(
        too_long.structured_content,
        Some(direct("2026-12-31").await)
    );
    assert_eq!(too_long.structured_content.unwrap()["status"], 400);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn sleep_recent_passes_through_nights_and_api_bounds() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();

    let result = call_sleep_recent(&client, 1).await;
    assert_eq!(result.is_error, Some(false));
    assert_eq!(
        result.structured_content,
        Some(api_get_sleep_recent(&api, 1).await)
    );
    let invalid = call_sleep_recent(&client, 15).await;
    assert_eq!(invalid.is_error, Some(true));
    assert_eq!(
        invalid.structured_content,
        Some(api_get_sleep_recent(&api, 15).await)
    );

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn returns_the_api_exercise_history_and_errors_unchanged() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let call = |name: &str| {
        CallToolRequestParams::new("exercise_history").with_arguments(
            json!({"exercise_name": name, "from_day": "2026-09-14", "to_day": "2026-09-14"})
                .as_object()
                .unwrap()
                .clone(),
        )
    };

    let result = client.call_tool(call("Pull-up")).await.unwrap();
    let direct: Value = reqwest::Client::new()
        .get(format!(
            "{api}/api/training/exercises/Pull-up/history/2026-09-14/2026-09-14"
        ))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content, Some(direct.clone()));
    assert_eq!(direct["workouts"][0]["entries"][0]["sets"][0]["reps"], 5);

    let missing = client.call_tool(call("Unknown")).await.unwrap();
    assert_eq!(missing.is_error, Some(true));
    assert_eq!(missing.structured_content.unwrap()["status"], 404);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn returns_the_api_recovery_day_unchanged() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();

    let result = recovery_on_day(&client, "2026-09-14").await;
    let direct = api_get(&api, "2026-09-14").await;

    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content, Some(direct.clone()));
    assert_eq!(
        direct["derived_scores"]["strain"],
        json!({"value": 3.1, "unit": "score_0_100", "source": "my-whoop"})
    );
    assert_eq!(direct["noop"]["freshness"], "confirmed");

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn returns_the_api_workouts_on_a_day_unchanged() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();

    let result = call_day_tool(&client, "workouts_on_day", "2026-09-14").await;
    let direct = api_get_read(&api, "2026-09-14", "workouts").await;

    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content, Some(direct.clone()));
    assert_eq!(
        direct["workouts"][0]["exercises"][0]["sets"],
        json!([{"set_number": 1, "reps": 5, "added_weight_kg": 12.5, "hold_duration_s": null,
                "rpe": 10, "notes": "+2 partials"}])
    );
    assert_eq!(
        direct["noop_workouts"][0]["start"],
        "2026-09-14T18:00:00+02:00"
    );
    assert_eq!(direct["noop"]["coverage"], json!({"workouts": "covered"}));

    let invalid = call_day_tool(&client, "workouts_on_day", "2026-02-30").await;
    assert_eq!(invalid.is_error, Some(true));
    assert_eq!(
        invalid.structured_content,
        Some(api_get_read(&api, "2026-02-30", "workouts").await)
    );
    assert_eq!(invalid.structured_content.unwrap()["status"], 400);

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn noop_workout_detail_forwards_one_workout_and_api_errors_unchanged() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let call = |source: &str, start: &str, sport: Option<&str>| {
        let mut arguments = json!({"source": source, "start": start});
        if let Some(sport) = sport {
            arguments["sport"] = json!(sport);
        }
        let request = CallToolRequestParams::new("noop_workout_detail")
            .with_arguments(arguments.as_object().unwrap().clone());
        client.call_tool(request)
    };
    let direct = |path: &str| {
        let url = format!("{api}/api/training/noop-workouts/{path}");
        async move {
            reqwest::Client::new()
                .get(url)
                .bearer_auth(API_KEY)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };

    // The NOOP Workout exactly as workouts_on_day lists it.
    let listed = api_get_read(&api, "2026-09-14", "workouts").await["noop_workouts"][0].clone();
    let result = call(
        listed["source"].as_str().unwrap(),
        listed["start"].as_str().unwrap(),
        None,
    )
    .await
    .unwrap();
    let api_result = direct("my-whoop/2026-09-14T18:00:00+02:00").await;
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.structured_content, Some(api_result.clone()));
    assert_eq!(api_result["start"], listed["start"]);
    assert_eq!(
        api_result["hr"],
        json!({"basis": "workout_row", "avg_bpm": 88.0, "min_bpm": null, "max_bpm": 141.0})
    );

    // The listed sport, when passed, is a third path segment.
    let with_sport = call(
        listed["source"].as_str().unwrap(),
        listed["start"].as_str().unwrap(),
        Some(listed["sport"].as_str().unwrap()),
    )
    .await
    .unwrap();
    assert_eq!(with_sport.is_error, Some(false));
    assert_eq!(
        with_sport.structured_content,
        Some(direct("my-whoop/2026-09-14T18:00:00+02:00/Calisthenics").await)
    );
    assert_eq!(with_sport.structured_content, Some(api_result.clone()));
    let other_sport = call("my-whoop", "2026-09-14T18:00:00+02:00", Some("Running"))
        .await
        .unwrap();
    assert_eq!(other_sport.is_error, Some(true));
    assert_eq!(
        other_sport.structured_content,
        Some(direct("my-whoop/2026-09-14T18:00:00+02:00/Running").await)
    );
    assert_eq!(other_sport.structured_content.unwrap()["status"], 404);

    let invalid = call("my-whoop", "2026-09-14", None).await.unwrap();
    assert_eq!(invalid.is_error, Some(true));
    assert_eq!(
        invalid.structured_content,
        Some(direct("my-whoop/2026-09-14").await)
    );
    assert_eq!(invalid.structured_content.unwrap()["status"], 400);

    // A slash stays inside the one encoded `source` segment, so it cannot reach another route.
    let missing = call("my-whoop/../../health", "2026-09-14T18:00:00+02:00", None)
        .await
        .unwrap();
    assert_eq!(missing.is_error, Some(true));
    assert_eq!(
        missing.structured_content,
        Some(json!({
            "error": "No NOOP Workout from 'my-whoop/../../health' starts at \
                      2026-09-14T18:00:00+02:00",
            "status": 404,
        }))
    );
    client.cancel().await.unwrap();
}
