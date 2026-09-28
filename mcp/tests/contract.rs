//! MCP-to-API contract: the adapter in front of the real OpenHome API training routes, driven by
//! an MCP Streamable HTTP client.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use openhome_api::services::noop_push;
use openhome_api::{AppState, DockerCache, auth, routes};
use openhome_mcp::{ApiKey, MCP_PATH, OpenHomeApi, parse_api_url};
use pretty_assertions::assert_eq;
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, CallToolResult, ClientConfig},
    service::RunningService,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
        streamable_http_server::StreamableHttpServerConfig,
    },
};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

const API_KEY: &str = "test-api-key";
const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";
const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"contract-test","version":"1.0"}}}"#;

async fn serve(app: Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    address
}

/// An in-memory database. One connection, because every in-memory connection is its own database.
async fn memory_db() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap()
}

/// The API's training routes behind its API-key layer, over a NOOP mirror where both strap
/// namespaces pushed 2026-09-14 (the real rows: imported strain 3.1, computed strain 0.0, and an
/// evening NOOP Workout) and a training log with one Workout that day.
async fn start_api() -> String {
    let state = AppState {
        db: training_log().await,
        noop_db: noop_mirror().await,
        adguard_service: None,
        docker_service: None,
        ir_service: None,
        switchbot_service: None,
        docker_cache: Arc::new(tokio::sync::Mutex::new(DockerCache::default())),
    };
    let api_key = auth::ApiKey::new(API_KEY.to_string());
    let app = routes::training::router()
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            auth::auth_middleware(request, next, api_key.clone())
        }));
    format!("http://{}", serve(app).await)
}

/// The NOOP mirror: both strap namespaces' 2026-09-14 rows and replacement coverage.
async fn noop_mirror() -> SqlitePool {
    let noop_db = memory_db().await;
    noop_push::initialize(&noop_db).await.unwrap();
    for (device, strain) in [("my-whoop", 3.1), ("my-whoop-noop", 0.0)] {
        for (stream, selector, start, end) in [
            ("dailyMetric", "day", "2026-09-13", "2026-09-27"),
            ("sleepSession", "startTs", "1789250400", "1790460000"),
            ("workout", "startTs", "1789250400", "1790460000"),
        ] {
            sqlx::query(
                "INSERT INTO batch_ledger (receiver_state_id, source_id, device_id, batch_id, \
                     stream, body_sha256, ack, accepted_at) \
                 SELECT receiver_state_id, ?, ?, ?, ?, x'00', '{}', \
                     '2026-09-26T01:51:38.318Z' FROM receiver_state",
            )
            .bind(SOURCE)
            .bind(device)
            .bind(stream)
            .bind(stream)
            .execute(&noop_db)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO replacement (receiver_state_id, source_id, device_id, replacement_id, \
                     stream, selector, start_inclusive, end_exclusive, parts, state) \
                 SELECT receiver_state_id, ?, ?, ?, ?, ?, ?, ?, 1, 'applied' FROM receiver_state",
            )
            .bind(SOURCE)
            .bind(device)
            .bind(stream)
            .bind(stream)
            .bind(selector)
            .bind(start)
            .bind(end)
            .execute(&noop_db)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO replacement_part (receiver_state_id, source_id, device_id, \
                     replacement_id, part, batch_id) \
                 SELECT receiver_state_id, ?, ?, ?, 1, ? FROM receiver_state",
            )
            .bind(SOURCE)
            .bind(device)
            .bind(stream)
            .bind(stream)
            .execute(&noop_db)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO daily_metric (source_id, device_id, day, strain) \
             VALUES (?, ?, '2026-09-14', ?)",
        )
        .bind(SOURCE)
        .bind(device)
        .bind(strain)
        .execute(&noop_db)
        .await
        .unwrap();
    }

    // 18:00-19:15 local time on 2026-09-14.
    sqlx::query(
        "INSERT INTO workout (source_id, device_id, start_ts, sport, end_ts, source, duration_s, \
             avg_hr, max_hr) \
         VALUES (?, 'my-whoop', 1789401600, 'Calisthenics', 1789406100, 'manual', 4500.0, 88.0, \
             141.0)",
    )
    .bind(SOURCE)
    .execute(&noop_db)
    .await
    .unwrap();
    noop_db
}

/// The training log: one Pull Workout on 2026-09-14.
async fn training_log() -> SqlitePool {
    let db = memory_db().await;
    for migration in [
        include_str!("../../api/migrations/0003_fitness.up.sql"),
        include_str!("../../api/migrations/0004_remove_workout_body_weight.up.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&db).await.unwrap();
    }
    sqlx::raw_sql(
        "INSERT INTO workouts (id, date, name) VALUES (1, '2026-09-14', 'Pull'); \
         INSERT INTO workout_exercises (id, workout_id, exercise_id, order_index) \
             VALUES (1, 1, 11, 0); \
         INSERT INTO sets (workout_exercise_id, set_number, reps, weight_kg, rpe, notes) \
             VALUES (1, 1, 5, 12.5, 10, '+2 partials');",
    )
    .execute(&db)
    .await
    .unwrap();
    db
}

/// The adapter, answering clients that present `API_KEY` and calling `api_url` with
/// `key_for_api`.
async fn start_mcp(api_url: &str, key_for_api: &str) -> String {
    let api = OpenHomeApi::new(
        parse_api_url(api_url).unwrap(),
        ApiKey::new(key_for_api.to_string()).unwrap(),
    )
    .unwrap();
    let app = openhome_mcp::router(
        api,
        ApiKey::new(API_KEY.to_string()).unwrap(),
        StreamableHttpServerConfig::default(),
    );
    format!("http://{}{MCP_PATH}", serve(app).await)
}

async fn connect(
    mcp_url: &str,
    key: &str,
) -> anyhow::Result<RunningService<RoleClient, ClientConfig>> {
    let transport = StreamableHttpClientTransport::from_config(
        StreamableHttpClientTransportConfig::with_uri(mcp_url.to_string()).auth_header(key),
    );
    ClientConfig::default()
        .serve(transport)
        .await
        .map_err(|err| anyhow::anyhow!("MCP client failed to start: {err:?}"))
}

async fn call_day_tool(
    client: &RunningService<RoleClient, ClientConfig>,
    tool: &'static str,
    day: &str,
) -> CallToolResult {
    let arguments = json!({"day": day}).as_object().unwrap().clone();
    client
        .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments))
        .await
        .unwrap()
}

async fn recovery_on_day(
    client: &RunningService<RoleClient, ClientConfig>,
    day: &str,
) -> CallToolResult {
    call_day_tool(client, "recovery_on_day", day).await
}

async fn api_get(api_url: &str, day: &str) -> Value {
    api_get_read(api_url, day, "recovery").await
}

async fn api_get_read(api_url: &str, day: &str, read: &str) -> Value {
    reqwest::Client::new()
        .get(format!("{api_url}/api/training/days/{day}/{read}"))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn discovers_the_read_only_training_tools() {
    let mcp = start_mcp(&start_api().await, API_KEY).await;
    let client = connect(&mcp, API_KEY).await.unwrap();

    let info = client.peer_info().unwrap();
    let server = info.server_info.as_ref().unwrap();
    assert_eq!(server.name, "openhome-mcp");
    assert_eq!(server.version, env!("CARGO_PKG_VERSION"));

    let tools = client.list_all_tools().await.unwrap();
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(
        names,
        ["exercise_history", "recovery_on_day", "workouts_on_day"]
    );
    for tool in &tools {
        assert_eq!(
            tool.annotations
                .as_ref()
                .and_then(|annotations| annotations.read_only_hint),
            Some(true),
            "{}",
            tool.name
        );
        let required = if tool.name == "exercise_history" {
            json!(["exercise_name", "from_day", "to_day"])
        } else {
            json!(["day"])
        };
        assert_eq!(tool.input_schema.get("required"), Some(&required));
        assert!(tool.description.as_deref().is_some_and(|description| {
            description.contains("Europe/Copenhagen") && !description.contains('\n')
        }));
    }

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
async fn does_not_follow_api_redirects() {
    let redirected_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls = redirected_calls.clone();
    let error = json!({"error": "Recovery endpoint moved", "status": 302});
    let response_error = error.clone();
    let api = Router::new()
        .route(
            "/api/training/days/{day}/recovery",
            axum::routing::get(move || async move {
                (
                    axum::http::StatusCode::FOUND,
                    [(axum::http::header::LOCATION, "/other")],
                    axum::Json(response_error),
                )
            }),
        )
        .route(
            "/other",
            axum::routing::get(move || async move {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                axum::Json(json!({"unexpected": true}))
            }),
        );
    let api_url = format!("http://{}", serve(api).await);
    let client = connect(&start_mcp(&api_url, API_KEY).await, API_KEY)
        .await
        .unwrap();

    let result = recovery_on_day(&client, "2026-09-14").await;
    assert_eq!(
        redirected_calls.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(result.is_error, Some(true));
    assert_eq!(result.structured_content, Some(error));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn rejects_dot_path_arguments() {
    let api_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls = api_calls.clone();
    let api = Router::new().fallback(move || async move {
        calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        axum::http::StatusCode::NOT_FOUND
    });
    let api_url = format!("http://{}", serve(api).await);
    let client = connect(&start_mcp(&api_url, API_KEY).await, API_KEY)
        .await
        .unwrap();
    for day in [".", ".."] {
        let result = recovery_on_day(&client, day).await;
        assert_eq!(result.is_error, Some(true));
        assert_eq!(result.structured_content.unwrap()["status"], 400);
    }
    assert_eq!(api_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn rejects_clients_without_the_api_key() {
    let mcp = start_mcp(&start_api().await, API_KEY).await;

    for authorization in [None, Some("Bearer wrong-key"), Some(API_KEY)] {
        let mut request = reqwest::Client::new()
            .post(&mcp)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(INITIALIZE);
        if let Some(authorization) = authorization {
            request = request.header("authorization", authorization);
        }
        let response = request.send().await.unwrap();

        assert_eq!(response.status(), 401, "{authorization:?}");
        assert_eq!(response.headers()["www-authenticate"], "Bearer");
        assert_eq!(
            response.json::<Value>().await.unwrap(),
            json!({"error": "Missing or invalid API key", "status": 401})
        );
    }
    assert!(connect(&mcp, "wrong-key").await.is_err());
}

#[tokio::test]
async fn returns_api_errors_as_tool_errors() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();

    let invalid = recovery_on_day(&client, "2026-02-30").await;
    assert_eq!(invalid.is_error, Some(true));
    assert_eq!(
        invalid.structured_content,
        Some(json!({
            "error": "Invalid day '2026-02-30': must be a Europe/Copenhagen calendar date written \
                      YYYY-MM-DD",
            "status": 400,
        }))
    );
    assert_eq!(
        invalid.structured_content,
        Some(api_get(&api, "2026-02-30").await)
    );

    // The argument stays one path segment, so it cannot reach another API route.
    let escape = recovery_on_day(&client, "../../health").await;
    assert_eq!(escape.is_error, Some(true));
    assert_eq!(escape.structured_content.unwrap()["status"], 400);
    client.cancel().await.unwrap();

    // The API rejecting the adapter's key reaches the caller as the API's 401.
    let misconfigured = connect(&start_mcp(&api, "wrong-key").await, API_KEY)
        .await
        .unwrap();
    let rejected = recovery_on_day(&misconfigured, "2026-09-14").await;
    assert_eq!(rejected.is_error, Some(true));
    assert_eq!(
        rejected.structured_content,
        Some(json!({"error": "Missing or invalid API key", "status": 401}))
    );
    misconfigured.cancel().await.unwrap();

    let unreachable = connect(&start_mcp("http://127.0.0.1:1", API_KEY).await, API_KEY)
        .await
        .unwrap();
    let down = recovery_on_day(&unreachable, "2026-09-14").await;
    assert_eq!(down.is_error, Some(true));
    assert_eq!(
        down.structured_content,
        Some(json!({"error": "The OpenHome API could not be reached", "status": null}))
    );
    unreachable.cancel().await.unwrap();
}
