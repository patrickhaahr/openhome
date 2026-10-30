//! MCP-to-API contract: the adapter in front of the real OpenHome API training routes, driven by
//! an MCP Streamable HTTP client.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use openhome_api::services::noop_push;
use openhome_api::services::nutrition::food_database::{DatasetPin, FoodDatabase};
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
    start_api_with_foods(food_database(true).await).await
}

/// The API routes over `foods`, for Food database reads.
async fn start_api_with_foods(foods: FoodDatabase) -> String {
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
        .merge(routes::wishlist::router(None))
        .merge(routes::watchlist::router(None))
        .merge(routes::important_dates::router())
        .merge(routes::nutrition::router(foods))
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            auth::auth_middleware(request, next, api_key.clone())
        }));
    format!("http://{}", serve(app).await)
}

/// A Food database: when `imported`, one complete import of the pinned dataset holding a
/// synthetic Skyr; otherwise empty, so lookups report importing.
async fn food_database(imported: bool) -> FoodDatabase {
    let db = memory_db().await;
    let pin = DatasetPin::pinned();
    if imported {
        sqlx::migrate!("../api/food_migrations")
            .run(&db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO dataset (id, version, sha256, imported_at) \
             VALUES (1, ?, ?, '2026-10-30T00:00:00Z')",
        )
        .bind(&pin.version)
        .bind(&pin.sha256)
        .execute(&db)
        .await
        .unwrap();
        sqlx::raw_sql(
            "INSERT INTO parameter (parameter_id, key, name_da, name_en, group_da, group_en, unit, \
                 per_100g, sort_key) VALUES \
                 (356, 'energy_kcal', 'Energi (kcal)', 'Energy (kcal)', 'Makronæringstoffer m.m.', \
                     'Macro nutrients etc.', 'kcal', 1, 1030), \
                 (218, 'protein', 'Protein', 'Protein', 'Makronæringstoffer m.m.', \
                     'Macro nutrients etc.', 'g', 1, 1110), \
                 (252, 'waste', 'Svind', 'Waste', 'Faktorer m.m.', 'Factors etc.', '%', 0, 20010), \
                 (363, 'density', 'Massefylde', 'Density', 'Faktorer m.m.', 'Factors etc.', 'g/ml', \
                     0, 20210); \
             INSERT INTO food (food_id, name_da, name_en, food_group_id, food_group_da, food_group_en) \
                 VALUES (1693, 'Skyr, 0.2 % fedt', 'Skyr, 0.2 % fat', 3, 'Syrnede mælkeprodukter', \
                     'Fermented milk products'); \
             INSERT INTO food_value (food_id, parameter_id, value) VALUES \
                 (1693, 356, 62.0), (1693, 218, 11.0), (1693, 252, 0.0), (1693, 363, 1.03);",
        )
        .execute(&db)
        .await
        .unwrap();
    }
    let unused_cache = std::env::temp_dir().join("openhome-mcp-contract-food-dataset");
    let (foods, import) = FoodDatabase::open(db, pin, unused_cache).await.unwrap();
    assert_eq!(import.is_some(), !imported);
    foods
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
    sqlx::migrate!("../api/migrations").run(&db).await.unwrap();
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

async fn wishlist_call(
    client: &RunningService<RoleClient, ClientConfig>,
    tool: &'static str,
    arguments: Value,
) -> CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new(tool).with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one real MCP workflow owns pass-through, clarification, receipt replay and owner-targeted removal"
)]
async fn manual_watchlist_tools_forward_owner_targeted_plans_and_api_receipts_unchanged() {
    let api = start_api().await;
    let http = reqwest::Client::new();
    let mut owners = Vec::new();
    for name in ["Member A", "Member B"] {
        let owner: Value = http
            .post(format!("{api}/api/household-users"))
            .bearer_auth(API_KEY)
            .json(&json!({"name":name}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        owners.push(owner);
    }
    let mcp = start_mcp(&api, API_KEY).await;
    assert!(connect(&mcp, "wrong-key").await.is_err());
    let client = connect(&mcp, API_KEY).await.unwrap();
    assert_eq!(
        wishlist_call(&client, "household_users", json!({}))
            .await
            .structured_content,
        Some(json!(owners))
    );
    let payload = json!({"kind":"series","title":"A Quiet Orbit","request_id":"mcp-watch-1"});
    let clarification = wishlist_call(&client, "add_watchlist_entry", payload.clone()).await;
    let direct: Value = http
        .post(format!("{api}/api/watchlist/entries"))
        .bearer_auth(API_KEY)
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(clarification.is_error, Some(true));
    assert_eq!(direct["code"], "watchlist_destination_required");
    assert_eq!(clarification.structured_content, Some(direct));
    let mut payload = payload;
    payload["owner_id"] = owners[0]["id"].clone();
    let added = wishlist_call(&client, "add_watchlist_entry", payload.clone()).await;
    assert_eq!(added.is_error, Some(false));
    let addition = added.structured_content.unwrap();
    assert_eq!(addition["entry"]["status"], "planned");
    assert_eq!(addition["entry"]["kind"], "series");
    let replayed = wishlist_call(&client, "add_watchlist_entry", payload.clone()).await;
    assert_eq!(
        replayed.structured_content,
        Some(json!({"entry":addition["entry"],"replayed":true}))
    );
    let mut changed = payload.clone();
    changed["owner_id"] = owners[1]["id"].clone();
    assert_eq!(
        wishlist_call(&client, "add_watchlist_entry", changed)
            .await
            .is_error,
        Some(true)
    );
    let listed = wishlist_call(&client, "watchlist_entries", json!({})).await;
    let direct: Value = http
        .get(format!("{api}/api/watchlist/entries"))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed.structured_content, Some(direct));
    let target = json!({"owner_id":owners[0]["id"],"entry_id":addition["entry"]["id"]});
    assert_eq!(
        wishlist_call(&client, "watchlist_entry", target.clone())
            .await
            .structured_content,
        Some(addition["entry"].clone())
    );
    let wrong = json!({"owner_id":owners[1]["id"],"entry_id":addition["entry"]["id"]});
    assert_eq!(
        wishlist_call(&client, "delete_watchlist_entry", wrong)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "delete_watchlist_entry", target.clone())
            .await
            .structured_content,
        Some(json!({"deleted":true}))
    );
    assert_eq!(
        wishlist_call(&client, "watchlist_entry", target.clone())
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "delete_watchlist_entry", target)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "add_watchlist_entry", payload)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "watchlist_entries", json!({}))
            .await
            .structured_content,
        Some(json!([]))
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one real MCP workflow owns explicit tags, receipt replay, partial updates and removal"
)]
async fn important_date_tools_forward_explicit_tags_receipts_and_figures_unchanged() {
    let api = start_api().await;
    let http = reqwest::Client::new();
    let mut people = Vec::new();
    for name in ["Member A", "Member B"] {
        let person: Value = http
            .post(format!("{api}/api/household-users"))
            .bearer_auth(API_KEY)
            .json(&json!({"name":name}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        people.push(person["id"].clone());
    }
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    // Tags are required: a date with no person_ids at all is rejected before Axum.
    let untagged = client
        .call_tool(
            CallToolRequestParams::new("add_important_date").with_arguments(
                json!({"title":"Together","date":"2024-02-14","kind":"since","request_id":"mcp-date-1"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;
    assert!(untagged.is_err() || untagged.unwrap().is_error == Some(true));
    let payload = json!({"title":"Together","date":"2024-02-14","kind":"since","person_ids":people,"request_id":"mcp-date-1"});
    let added = wishlist_call(&client, "add_important_date", payload.clone()).await;
    assert_eq!(added.is_error, Some(false));
    let addition = added.structured_content.unwrap();
    assert_eq!(addition["replayed"], false);
    assert_eq!(addition["important_date"]["show_when_idle"], false);
    assert_eq!(
        wishlist_call(&client, "add_important_date", payload.clone())
            .await
            .structured_content,
        Some(json!({"important_date":addition["important_date"],"replayed":true}))
    );
    let mut changed = payload.clone();
    changed["person_ids"] = json!([]);
    assert_eq!(
        wishlist_call(&client, "add_important_date", changed)
            .await
            .is_error,
        Some(true)
    );
    let friend = wishlist_call(
        &client,
        "add_important_date",
        json!({"title":"A friend's birthday","date":"2000-02-29","kind":"yearly","person_ids":[],"show_when_idle":true,"request_id":"mcp-date-2"}),
    )
    .await;
    assert_eq!(
        friend.structured_content.unwrap()["important_date"]["person_ids"],
        json!([])
    );

    let listed = wishlist_call(&client, "important_dates", json!({"on":"2026-05-26"})).await;
    let direct: Value = http
        .get(format!("{api}/api/important-dates?on=2026-05-26"))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed.structured_content, Some(direct.clone()));
    assert_eq!(
        direct["dates"][0]["elapsed"],
        json!({"years":2,"months":3,"days":12,"total_days":832})
    );
    assert_eq!(direct["dates"][1]["next"]["date"], "2027-02-28");
    let anniversary = wishlist_call(&client, "important_dates", json!({"on":"2026-02-14"})).await;
    assert_eq!(
        anniversary.structured_content.unwrap()["dates"][0]["milestone"],
        json!({"kind":"anniversary","years":2})
    );
    let idle = wishlist_call(&client, "important_dates", json!({"show_when_idle":true})).await;
    assert_eq!(
        idle.structured_content.unwrap()["dates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        wishlist_call(&client, "important_dates", json!({"on":"2026-02-30"}))
            .await
            .is_error,
        Some(true)
    );

    let target = json!({"date_id":addition["important_date"]["id"]});
    let mut update = target.clone();
    update["person_ids"] = json!([people[1]]);
    update["show_when_idle"] = json!(true);
    let updated = wishlist_call(&client, "update_important_date", update)
        .await
        .structured_content
        .unwrap();
    assert_eq!(updated["person_ids"], json!([people[1]]));
    assert_eq!(updated["title"], "Together");
    assert_eq!(updated["show_when_idle"], true);
    let mut null_title = target.clone();
    null_title["title"] = Value::Null;
    assert_eq!(
        wishlist_call(&client, "update_important_date", null_title)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "important_date", target.clone())
            .await
            .structured_content,
        Some(updated)
    );
    assert_eq!(
        wishlist_call(&client, "delete_important_date", target.clone())
            .await
            .structured_content,
        Some(json!({"deleted":true}))
    );
    for tool in ["important_date", "delete_important_date"] {
        assert_eq!(
            wishlist_call(&client, tool, target.clone()).await.is_error,
            Some(true)
        );
    }
    assert_eq!(
        wishlist_call(&client, "add_important_date", payload)
            .await
            .is_error,
        Some(true)
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one real MCP workflow protects partial corrections, filtering and pass-through errors"
)]
async fn watchlist_corrections_and_filters_forward_to_real_axum_with_omission_and_null_preserved() {
    let api = start_api().await;
    let http = reqwest::Client::new();
    let owner: Value = http
        .post(format!("{api}/api/household-users"))
        .bearer_auth(API_KEY)
        .json(&json!({"name":"Member A"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let added = wishlist_call(&client, "add_watchlist_entry", json!({"owner_id":owner["id"],"kind":"series","title":"A Quiet Orbit","request_id":"correct-watch"})).await.structured_content.unwrap();
    let target = json!({"owner_id":owner["id"],"entry_id":added["entry"]["id"]});
    let path = format!(
        "{api}/api/household-users/{}/watchlist/entries/{}",
        owner["id"].as_str().unwrap(),
        added["entry"]["id"].as_str().unwrap()
    );
    for patch in [
        json!({"status":"watching"}),
        json!({"status":"completed","date_added":"2024-02-28","completion_date":"2024-02-29"}),
        json!({"date_added":"2024-02-27"}),
        json!({"status":"completed"}),
        json!({"status":"planned","completion_date":null}),
    ] {
        let mut args = target.clone();
        args.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let result = wishlist_call(&client, "update_watchlist_entry", args).await;
        assert_eq!(result.is_error, Some(false), "{result:?}");
        let stored: Value = http
            .get(&path)
            .bearer_auth(API_KEY)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(result.structured_content, Some(stored.clone()));
        assert_eq!(stored["kind"], "series");
        if patch.get("completion_date").is_none() && patch["status"] != "watching" {
            assert_eq!(stored["completion_date"], "2024-02-29");
        }
    }
    let filtered = wishlist_call(
        &client,
        "watchlist_entries",
        json!({"owner_id":owner["id"],"status":"planned"}),
    )
    .await;
    let direct: Value = http
        .get(format!(
            "{api}/api/watchlist/entries?owner_id={}&status=planned",
            owner["id"].as_str().unwrap()
        ))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(filtered.structured_content, Some(direct));
    assert_eq!(
        wishlist_call(&client, "watchlist_entries", json!({"status":"completed"}))
            .await
            .structured_content,
        Some(json!([]))
    );
    // Owner IDs with query punctuation must remain one encoded value, never additional filters.
    assert_eq!(
        wishlist_call(
            &client,
            "watchlist_entries",
            json!({"owner_id":"missing&status=planned"})
        )
        .await
        .structured_content,
        Some(json!([]))
    );
    for patch in [
        json!({"completion_date":"2024-02-29"}),
        json!({"date_added":null}),
        json!({"status":"completed","completion_date":null}),
        json!({"date_added":"2024-02-30"}),
    ] {
        let mut args = target.clone();
        args.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let forwarded = wishlist_call(&client, "update_watchlist_entry", args).await;
        let direct: Value = http
            .patch(&path)
            .bearer_auth(API_KEY)
            .json(&patch)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(forwarded.is_error, Some(true));
        assert_eq!(forwarded.structured_content, Some(direct));
    }
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn watchlist_write_auth_and_transport_failures_remain_tool_errors() {
    let api = start_api().await;
    for (api_url, key, expected_status) in [
        (api.as_str(), "wrong-key", json!(401)),
        ("http://127.0.0.1:1", API_KEY, Value::Null),
    ] {
        let client = connect(&start_mcp(api_url, key).await, API_KEY)
            .await
            .unwrap();
        for (tool, args) in [
            (
                "add_watchlist_entry",
                json!({"owner_id":"synthetic-owner","kind":"movie","title":"A Quiet Orbit","request_id":"failed-watch"}),
            ),
            (
                "update_watchlist_entry",
                json!({"owner_id":"synthetic-owner","entry_id":"synthetic-entry","status":"completed"}),
            ),
        ] {
            let result = wishlist_call(&client, tool, args).await;
            assert_eq!(result.is_error, Some(true));
            assert_eq!(
                result.structured_content.unwrap()["status"],
                expected_status
            );
        }
        client.cancel().await.unwrap();
    }
}

#[tokio::test]
async fn wishlist_mutations_and_clarification_pass_through_the_authenticated_api() {
    let api = start_api().await;
    let http = reqwest::Client::new();
    let owner: Value = http
        .post(format!("{api}/api/household-users"))
        .bearer_auth(API_KEY)
        .json(&json!({"name":"Member A"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let owners = wishlist_call(&client, "household_users", json!({})).await;
    assert_eq!(owners.structured_content, Some(json!([owner])));
    let created = wishlist_call(
        &client,
        "create_wishlist",
        json!({"owner_id":owner["id"],"name":"Ideas"}),
    )
    .await;
    assert_eq!(created.is_error, Some(false));
    let list = created.structured_content.unwrap();
    assert_eq!(list["owner_id"], owner["id"]);
    let lists = wishlist_call(&client, "wishlists", json!({})).await;
    assert_eq!(lists.structured_content, Some(json!([list])));
    let payload = json!({"title":"A warm jacket","request_id":"mcp-wish-1"});
    let ambiguous = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    assert_eq!(ambiguous.is_error, Some(true));
    let direct: Value = http
        .post(format!("{api}/api/wishlist/items"))
        .bearer_auth(API_KEY)
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(ambiguous.structured_content, Some(direct.clone()));
    assert_eq!(direct["code"], "wishlist_destination_required");
    let mut payload = payload;
    payload["list_id"] = list["id"].clone();
    let added = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    assert_eq!(added.is_error, Some(false));
    let addition = added.structured_content.unwrap();
    assert_eq!(addition["item"]["title"], "A warm jacket");
    let replayed = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    assert_eq!(replayed.is_error, Some(false));
    assert_eq!(
        replayed.structured_content,
        Some(json!({"item":addition["item"],"replayed":true}))
    );
    let items = wishlist_call(&client, "wishlist_items", json!({"list_id":list["id"]})).await;
    let direct: Value = http
        .get(format!(
            "{api}/api/wishlists/{}/items",
            list["id"].as_str().unwrap()
        ))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(items.is_error, Some(false));
    assert_eq!(items.structured_content, Some(direct.clone()));
    assert_eq!(direct, json!([addition["item"]]));
    payload["title"] = json!("Another wish");
    let conflict = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    let direct: Value = http
        .post(format!("{api}/api/wishlist/items"))
        .bearer_auth(API_KEY)
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(conflict.is_error, Some(true));
    assert_eq!(conflict.structured_content, Some(direct));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn wishlist_write_auth_and_transport_failures_remain_tool_errors() {
    let api = start_api().await;
    for (api_url, key, expected_status) in [
        (api.as_str(), "wrong-key", json!(401)),
        ("http://127.0.0.1:1", API_KEY, Value::Null),
    ] {
        let client = connect(&start_mcp(api_url, key).await, API_KEY)
            .await
            .unwrap();
        let result = wishlist_call(
            &client,
            "add_wishlist_item",
            json!({"title":"A warm jacket","request_id":"failed-1"}),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content.unwrap()["status"],
            expected_status
        );
        client.cancel().await.unwrap();
    }
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

async fn api_get_sleep_recent(api_url: &str, n: u8) -> Value {
    reqwest::Client::new()
        .get(format!("{api_url}/api/training/sleep/recent/{n}"))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn call_sleep_recent(
    client: &RunningService<RoleClient, ClientConfig>,
    n: u8,
) -> CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new("sleep_recent")
                .with_arguments(json!({"n": n}).as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one discovery contract verifies training and Wishlist tool schemas and annotations"
)]
async fn discovers_read_only_training_and_additive_wishlist_tools() {
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
        [
            "add_important_date",
            "add_watchlist_entry",
            "add_wishlist_item",
            "create_wishlist",
            "delete_important_date",
            "delete_meal_log",
            "delete_watchlist_entry",
            "delete_wishlist_item",
            "exercise_history",
            "food",
            "household_users",
            "important_date",
            "important_dates",
            "log_meal",
            "meal_logs",
            "metric_trend",
            "noop_workout_detail",
            "nutrition_day",
            "recovery_on_day",
            "search_foods",
            "sleep_recent",
            "training_context",
            "update_important_date",
            "update_meal_log",
            "update_watchlist_entry",
            "update_wishlist_item",
            "watchlist_entries",
            "watchlist_entry",
            "wishlist_item",
            "wishlist_items",
            "wishlist_url_metadata",
            "wishlists",
            "workouts_on_day"
        ]
    );
    for tool in &tools {
        assert_eq!(
            tool.annotations
                .as_ref()
                .and_then(|annotations| annotations.read_only_hint),
            Some(!matches!(
                tool.name.as_ref(),
                "create_wishlist"
                    | "add_wishlist_item"
                    | "update_wishlist_item"
                    | "delete_wishlist_item"
                    | "add_watchlist_entry"
                    | "delete_watchlist_entry"
                    | "update_watchlist_entry"
                    | "add_important_date"
                    | "update_important_date"
                    | "delete_important_date"
                    | "log_meal"
                    | "update_meal_log"
                    | "delete_meal_log"
            )),
            "{}",
            tool.name
        );
        if matches!(
            tool.name.as_ref(),
            "create_wishlist"
                | "add_wishlist_item"
                | "add_watchlist_entry"
                | "add_important_date"
                | "log_meal"
        ) {
            let annotations = tool.annotations.as_ref().unwrap();
            assert_eq!(annotations.destructive_hint, Some(false));
            assert_eq!(
                annotations.idempotent_hint,
                Some(tool.name != "create_wishlist")
            );
        }
        if matches!(
            tool.name.as_ref(),
            "update_wishlist_item"
                | "delete_wishlist_item"
                | "delete_watchlist_entry"
                | "update_watchlist_entry"
                | "update_important_date"
                | "delete_important_date"
                | "update_meal_log"
                | "delete_meal_log"
        ) {
            let annotations = tool.annotations.as_ref().unwrap();
            assert_eq!(annotations.destructive_hint, Some(true));
            assert_eq!(annotations.idempotent_hint, Some(true));
        }
        if tool.name == "wishlist_url_metadata" {
            let annotations = tool.annotations.as_ref().unwrap();
            assert_eq!(annotations.open_world_hint, Some(true));
            assert_eq!(annotations.destructive_hint, Some(false));
        }
        let required = match tool.name.as_ref() {
            "household_users" | "wishlists" | "watchlist_entries" | "important_dates"
            | "meal_logs" => {
                assert!(tool.input_schema.get("required").is_none());
                continue;
            }
            "create_wishlist" => json!(["owner_id", "name"]),
            "add_wishlist_item" => json!(["title", "request_id"]),
            "add_watchlist_entry" => json!(["kind", "title", "request_id"]),
            "watchlist_entry" | "delete_watchlist_entry" | "update_watchlist_entry" => {
                json!(["owner_id", "entry_id"])
            }
            "wishlist_items" => json!(["list_id"]),
            "add_important_date" => json!(["title", "date", "kind", "person_ids", "request_id"]),
            "important_date" | "delete_important_date" | "update_important_date" => {
                json!(["date_id"])
            }
            "wishlist_url_metadata" => json!(["owner_id", "list_id", "url"]),
            "wishlist_item" | "delete_wishlist_item" | "update_wishlist_item" => {
                json!(["owner_id", "list_id", "item_id"])
            }
            "exercise_history" => json!(["exercise_name", "from_day", "to_day"]),
            "search_foods" => json!(["query"]),
            "food" => json!(["food_id"]),
            "log_meal" => json!(["eaten_at", "portions", "request_id"]),
            "update_meal_log" | "delete_meal_log" => json!(["meal_log_id"]),
            "metric_trend" => json!(["metric", "from_day", "to_day"]),
            "noop_workout_detail" => json!(["source", "start"]),
            "sleep_recent" => json!(["n"]),
            "training_context" => json!(["from_day", "to_day"]),
            _ => json!(["day"]),
        };
        assert_eq!(tool.input_schema.get("required"), Some(&required));
        if tool.name == "metric_trend" {
            assert_eq!(
                tool.input_schema["properties"]["metric"]["$ref"],
                "#/$defs/TrendMetric"
            );
            assert_eq!(
                tool.input_schema["$defs"]["TrendMetric"]["enum"],
                json!(["body_weight", "sleep_duration", "resting_heart_rate", "hrv"])
            );
        }
        assert!(tool.description.as_deref().is_some_and(|description| {
            (description.contains("Europe/Copenhagen")
                || tool.name.contains("wishlist")
                || tool.name.contains("watchlist")
                || tool.name.contains("important_date")
                || tool.name.contains("food"))
                && !description.contains('\n')
        }));
    }

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn food_tools_forward_searches_reads_and_api_errors_unchanged() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let instructions = client
        .peer_info()
        .unwrap()
        .instructions
        .clone()
        .unwrap_or_default();
    assert!(
        instructions.contains("density (g/ml) are grams")
            && instructions.contains("(1 - waste/100) is the edible weight"),
        "{instructions}"
    );
    let http = reqwest::Client::new();
    let direct = |path: &'static str| {
        let http = http.clone();
        let url = format!("{api}{path}");
        async move {
            http.get(url)
                .bearer_auth(API_KEY)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };

    let search = wishlist_call(
        &client,
        "search_foods",
        json!({"query": "skyr", "limit": 5}),
    )
    .await;
    assert_eq!(search.is_error, Some(false));
    let expected = direct("/api/nutrition/foods?query=skyr&limit=5").await;
    assert_eq!(expected["foods"][0]["food_id"], 1693, "{expected}");
    assert_eq!(search.structured_content, Some(expected));

    let food = wishlist_call(&client, "food", json!({"food_id": 1693})).await;
    assert_eq!(food.is_error, Some(false));
    let expected = direct("/api/nutrition/foods/1693").await;
    assert_eq!(expected["density"]["value"], 1.03, "{expected}");
    assert_eq!(food.structured_content, Some(expected));

    for (tool, arguments, path) in [
        (
            "food",
            json!({"food_id": 424_242}),
            "/api/nutrition/foods/424242",
        ),
        (
            "search_foods",
            json!({"query": " "}),
            "/api/nutrition/foods?query=%20",
        ),
    ] {
        let result = wishlist_call(&client, tool, arguments).await;
        assert_eq!(result.is_error, Some(true), "{tool}");
        assert_eq!(
            result.structured_content,
            Some(direct(path).await),
            "{tool}"
        );
    }

    let importing = start_api_with_foods(food_database(false).await).await;
    let client = connect(&start_mcp(&importing, API_KEY).await, API_KEY)
        .await
        .unwrap();
    for (tool, arguments) in [
        ("search_foods", json!({"query": "skyr"})),
        ("food", json!({"food_id": 1693})),
    ] {
        let result = wishlist_call(&client, tool, arguments).await;
        assert_eq!(result.is_error, Some(true), "{tool}");
        let error = result.structured_content.unwrap();
        assert_eq!(
            (&error["code"], &error["state"], &error["status"]),
            (
                &json!("food_database_unavailable"),
                &json!("importing"),
                &json!(503)
            ),
            "{tool}: {error}"
        );
    }
    client.cancel().await.unwrap();
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one real MCP workflow owns logging, receipt replay, day reads, correction and deletion"
)]
async fn meal_log_tools_forward_logs_receipts_days_and_errors_unchanged() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let instructions = client
        .peer_info()
        .unwrap()
        .instructions
        .clone()
        .unwrap_or_default();
    assert!(
        instructions.contains("explicit eaten_at")
            && instructions.contains("Split a mixed dish")
            && instructions.contains("reuse the identical arguments and request_id"),
        "{instructions}"
    );
    let http = reqwest::Client::new();
    let direct = |path: String| {
        let http = http.clone();
        let url = format!("{api}{path}");
        async move {
            http.get(url)
                .bearer_auth(API_KEY)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };

    let payload = json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "note": "after the run",
        "portions": [{"food_id": 1693, "grams": 150}],
        "request_id": "mcp-meal-1",
    });
    let logged = wishlist_call(&client, "log_meal", payload.clone()).await;
    assert_eq!(logged.is_error, Some(false), "{logged:?}");
    let logged = logged.structured_content.unwrap();
    assert_eq!(logged["replayed"], false);
    let meal_log = logged["meal_log"].clone();
    assert_eq!(meal_log["day"], "2026-10-29");
    assert_eq!(
        meal_log["portions"][0]["nutrients"]["energy_kcal"],
        json!({"value": 93.0, "unit": "kcal", "source": "food"})
    );
    assert_eq!(
        wishlist_call(&client, "log_meal", payload.clone())
            .await
            .structured_content,
        Some(json!({"meal_log": meal_log, "replayed": true}))
    );
    let mut changed = payload.clone();
    changed["portions"][0]["grams"] = json!(200);
    let conflict = wishlist_call(&client, "log_meal", changed).await;
    assert_eq!(conflict.is_error, Some(true));
    assert_eq!(conflict.structured_content.unwrap()["status"], 409);

    let day = wishlist_call(&client, "meal_logs", json!({"day": "2026-10-29"})).await;
    let expected = direct("/api/nutrition/meal-logs?day=2026-10-29".into()).await;
    assert_eq!(expected["meal_logs"], json!([meal_log]));
    assert_eq!(day.structured_content, Some(expected));
    let range = wishlist_call(
        &client,
        "meal_logs",
        json!({"from": "2026-10-28", "to": "2026-10-29"}),
    )
    .await;
    assert_eq!(
        range.structured_content,
        Some(direct("/api/nutrition/meal-logs?from=2026-10-28&to=2026-10-29".into()).await)
    );
    let nutrition_day = wishlist_call(&client, "nutrition_day", json!({"day": "2026-10-29"})).await;
    let expected = direct("/api/nutrition/days/2026-10-29".into()).await;
    assert_eq!(
        expected["totals"]["protein"],
        json!({"value": 16.5, "unit": "g", "unknown_portions": 0})
    );
    assert_eq!(nutrition_day.structured_content, Some(expected));

    let id = meal_log["id"].as_str().unwrap();
    let updated = wishlist_call(
        &client,
        "update_meal_log",
        json!({"meal_log_id": id, "eaten_at": "2026-10-28T21:00:00+01:00", "note": null}),
    )
    .await
    .structured_content
    .unwrap();
    assert_eq!(
        (&updated["day"], &updated["note"], &updated["portions"]),
        (&json!("2026-10-28"), &Value::Null, &meal_log["portions"])
    );
    let null_portions = wishlist_call(
        &client,
        "update_meal_log",
        json!({"meal_log_id": id, "portions": null}),
    )
    .await;
    assert_eq!(null_portions.is_error, Some(true));

    let target = json!({"meal_log_id": id});
    assert_eq!(
        wishlist_call(&client, "delete_meal_log", target.clone())
            .await
            .structured_content,
        Some(json!({"deleted": true}))
    );
    for (tool, arguments) in [
        ("delete_meal_log", target.clone()),
        (
            "update_meal_log",
            json!({"meal_log_id": id, "note": "late"}),
        ),
        ("log_meal", payload),
    ] {
        assert_eq!(
            wishlist_call(&client, tool, arguments).await.is_error,
            Some(true),
            "{tool}"
        );
    }
    for (tool, arguments, path) in [
        (
            "meal_logs",
            json!({"day": "2026-02-30"}),
            "/api/nutrition/meal-logs?day=2026-02-30",
        ),
        (
            "nutrition_day",
            json!({"day": "yesterday"}),
            "/api/nutrition/days/yesterday",
        ),
    ] {
        let result = wishlist_call(&client, tool, arguments).await;
        assert_eq!(result.is_error, Some(true), "{tool}");
        assert_eq!(
            result.structured_content,
            Some(direct(path.into()).await),
            "{tool}"
        );
    }

    let importing = start_api_with_foods(food_database(false).await).await;
    let client = connect(&start_mcp(&importing, API_KEY).await, API_KEY)
        .await
        .unwrap();
    for (tool, arguments) in [
        (
            "log_meal",
            json!({
                "eaten_at": "2026-10-29T08:15:00+01:00",
                "portions": [{"food_id": 1693, "grams": 150}],
                "request_id": "mcp-meal-2",
            }),
        ),
        ("nutrition_day", json!({"day": "2026-10-29"})),
    ] {
        let result = wishlist_call(&client, tool, arguments).await;
        assert_eq!(result.is_error, Some(true), "{tool}");
        let error = result.structured_content.unwrap();
        assert_eq!(
            (&error["code"], &error["state"], &error["status"]),
            (
                &json!("food_database_unavailable"),
                &json!("importing"),
                &json!(503)
            ),
            "{tool}: {error}"
        );
    }
    client.cancel().await.unwrap();
}

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

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one real transport lifecycle protects partial writes, retries and deletion"
)]
async fn rich_item_management_uses_real_transport_with_partial_updates_and_deletion() {
    let api = start_api().await;
    let http = reqwest::Client::new();
    let owner: Value = http
        .post(format!("{api}/api/household-users"))
        .bearer_auth(API_KEY)
        .json(&json!({"name":"Member A"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let list = wishlist_call(
        &client,
        "create_wishlist",
        json!({"owner_id":owner["id"],"name":"Outdoors"}),
    )
    .await
    .structured_content
    .unwrap();
    let payload = json!({"owner_id":owner["id"],"list_id":list["id"],"title":"A jacket","request_id":"rich-mcp-1",
        "url":"https://example.com/jacket","image_url":"https://example.com/jacket.png","notes":"For winter","price":"DKK 199.95","quantity":2,"priority":"high"});
    let blocked = wishlist_call(
        &client,
        "wishlist_url_metadata",
        json!({"owner_id":owner["id"],"list_id":list["id"],"url":"http://127.0.0.1/product"}),
    )
    .await;
    assert_eq!(blocked.is_error, Some(true));
    assert_eq!(blocked.structured_content.as_ref().unwrap()["status"], 400);
    assert_eq!(
        wishlist_call(&client, "wishlist_items", json!({"list_id":list["id"]}))
            .await
            .structured_content,
        Some(json!([]))
    );
    let wrong = wishlist_call(
        &client,
        "wishlist_url_metadata",
        json!({"owner_id":"wrong-owner","list_id":list["id"],"url":"https://example.com/jacket"}),
    )
    .await;
    assert_eq!(wrong.is_error, Some(true));
    assert_eq!(wrong.structured_content.unwrap()["status"], 409);
    let added = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    assert_eq!(added.is_error, Some(false));
    let item = added.structured_content.unwrap()["item"].clone();
    assert_eq!(item["quantity"], 2);
    assert_eq!(item["priority"], "high");
    let target = json!({"owner_id":owner["id"],"list_id":list["id"],"item_id":item["id"]});
    assert_eq!(
        wishlist_call(&client, "wishlist_item", target.clone())
            .await
            .structured_content,
        Some(item.clone())
    );
    let mut edit = target.clone();
    edit["notes"] = Value::Null;
    edit["quantity"] = json!(3);
    let result = wishlist_call(&client, "update_wishlist_item", edit.clone()).await;
    assert_eq!(result.is_error, Some(false));
    let edited = result.structured_content.unwrap();
    assert_eq!(edited["notes"], Value::Null);
    assert_eq!(edited["quantity"], 3);
    assert_eq!(edited["title"], item["title"]);
    assert_eq!(edited["url"], item["url"]);
    assert_eq!(edited["price"], item["price"]);
    let direct: Value = http
        .get(format!(
            "{api}/api/household-users/{}/wishlists/{}/items/{}",
            owner["id"].as_str().unwrap(),
            list["id"].as_str().unwrap(),
            item["id"].as_str().unwrap()
        ))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(edited, direct);
    assert_eq!(
        wishlist_call(&client, "update_wishlist_item", edit.clone())
            .await
            .structured_content,
        Some(edited.clone())
    );
    edit["quantity"] = json!(0);
    let invalid = wishlist_call(&client, "update_wishlist_item", edit).await;
    assert_eq!(invalid.is_error, Some(true));
    assert_eq!(invalid.structured_content.unwrap()["status"], 400);
    assert_eq!(
        wishlist_call(&client, "wishlist_item", target.clone())
            .await
            .structured_content,
        Some(edited.clone())
    );
    let replay = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    assert_eq!(
        replay.structured_content,
        Some(json!({"item":edited,"replayed":true}))
    );
    let mut wrong = target.clone();
    wrong["owner_id"] = json!("other-owner");
    for tool in [
        "wishlist_item",
        "update_wishlist_item",
        "delete_wishlist_item",
    ] {
        let result = wishlist_call(&client, tool, wrong.clone()).await;
        assert_eq!(result.is_error, Some(true));
        assert_eq!(result.structured_content.unwrap()["status"], 404);
    }
    let removed = wishlist_call(&client, "delete_wishlist_item", target.clone()).await;
    assert_eq!(removed.is_error, Some(false));
    assert_eq!(removed.structured_content, Some(json!({"deleted":true})));
    assert_eq!(
        wishlist_call(&client, "delete_wishlist_item", target)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "add_wishlist_item", payload)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "wishlist_items", json!({"list_id":list["id"]}))
            .await
            .structured_content,
        Some(json!([]))
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn unexpected_no_content_never_confirms_a_read_or_item_write() {
    let upstream = Router::new().route(
        "/api/{*path}",
        axum::routing::any(|| async { axum::http::StatusCode::NO_CONTENT }),
    );
    let api = format!("http://{}", serve(upstream).await);
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    for (tool, args) in [
        (
            "training_context",
            json!({"from_day":"2026-09-14","to_day":"2026-09-14"}),
        ),
        (
            "add_wishlist_item",
            json!({"title":"A jacket","request_id":"missing-receipt"}),
        ),
        (
            "update_watchlist_entry",
            json!({"owner_id":"owner","entry_id":"entry","status":"watching"}),
        ),
        (
            "update_wishlist_item",
            json!({"owner_id":"owner","list_id":"list","item_id":"item","quantity":2}),
        ),
    ] {
        let result = wishlist_call(&client, tool, args).await;
        assert_eq!(
            result.is_error,
            Some(true),
            "{tool} must not confirm an empty result"
        );
        assert_eq!(result.structured_content.unwrap()["status"], 204);
    }
    client.cancel().await.unwrap();
}
