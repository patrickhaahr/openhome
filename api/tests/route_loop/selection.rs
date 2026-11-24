use crate::common::send_request_with_method;
use crate::support::{NORTH_EAST, app_with_state, demo, generate, ok, provider, square, walk};
use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use openhome_api::services::route_loop::LoopPlanner;
use openhome_api::services::route_loop::ProviderEndpoint;
use serde_json::Value;
use serde_json::json;

async fn select(app: Router, id: &Value) -> (StatusCode, Value) {
    send_request_with_method(
        app,
        "/api/routes/active",
        Method::PUT,
        Some(json!({ "route_id": id })),
        Some("test-api-key"),
    )
    .await
}

#[tokio::test]
async fn selected_route_survives_restart_without_a_provider_and_versions_replacements() {
    let file = std::env::temp_dir().join(format!("openhome-routes-{}.db", uuid::Uuid::new_v4()));
    let url = format!("sqlite:{}", file.display());
    let db = openhome_api::db::connect_sqlite(&url).await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let (_, state) = crate::common::test_app_with_pool(db.clone());
    let server = provider(ok(square(5000, NORTH_EAST)), 6).await;
    let endpoint = ProviderEndpoint::parse(Some(&server.uri())).unwrap();
    let app = app_with_state(LoopPlanner::new(demo(), endpoint).unwrap(), state);
    let (_, empty) =
        crate::common::send_request(app.clone(), "/api/routes/active", Some("test-api-key")).await;
    assert_eq!(empty, json!({ "state": "none" }));
    let request = json!({ "activity": "running", "target": { "kind": "steps", "steps": 5000 }, "step_length_cm": 100 });
    let (_, generated) = generate(app.clone(), request).await;
    let (_, distance_plan) = generate(app.clone(), json!({ "activity": "running", "target": { "kind": "distance", "meters": 5000 }, "step_length_cm": 100 })).await;
    let (_, stride_plan) = generate(app.clone(), json!({ "activity": "running", "target": { "kind": "distance", "meters": 5000 }, "step_length_cm": 75 })).await;
    let chosen = &generated["plan"]["loops"][0];
    let (status, selected) = select(app.clone(), &chosen["id"]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(selected["state"], "selected");
    let route = &selected["route"];
    assert_eq!(route["id"], chosen["id"]);
    assert_eq!(route["version"], 1);
    assert_eq!(route["geometry"], chosen["geometry"]);
    assert_eq!(route["distance_meters"], chosen["distance_meters"]);
    assert_eq!(route["activity"], "running");
    assert_eq!(
        route["target"],
        json!({ "kind": "steps", "steps": 5000, "meters": 5000 })
    );
    let (_, retry) = select(app.clone(), &chosen["id"]).await;
    assert_eq!(retry, selected);
    drop(app);
    db.close().await;

    let db = openhome_api::db::connect_sqlite(&url).await.unwrap();
    let (_, state) = crate::common::test_app_with_pool(db.clone());
    let app = app_with_state(LoopPlanner::new(None, None).unwrap(), state);
    let (_, restored) =
        crate::common::send_request(app.clone(), "/api/routes/active", Some("test-api-key")).await;
    assert_eq!(restored, selected);
    let (_, failed) = generate(app.clone(), walk(6000)).await;
    assert_eq!(failed["state"], "unavailable");
    let (_, restored) =
        crate::common::send_request(app.clone(), "/api/routes/active", Some("test-api-key")).await;
    assert_eq!(restored, selected);
    let ((status_a, a), (status_b, b)) = tokio::join!(
        select(app.clone(), &distance_plan["plan"]["loops"][0]["id"]),
        select(app.clone(), &stride_plan["plan"]["loops"][0]["id"])
    );
    assert_eq!(status_a, StatusCode::OK);
    assert_eq!(status_b, StatusCode::OK);
    let mut versions = [
        a["route"]["version"].as_i64().unwrap(),
        b["route"]["version"].as_i64().unwrap(),
    ];
    versions.sort_unstable();
    assert_eq!(versions, [2, 3]);
    let (_, latest) =
        crate::common::send_request(app.clone(), "/api/routes/active", Some("test-api-key")).await;
    assert_eq!(latest, if a["route"]["version"] == 3 { a } else { b });
    drop(app);
    db.close().await;
    std::fs::remove_file(file).unwrap();
}

#[tokio::test]
async fn selection_authenticates_rejects_forged_routes_and_preserves_previous_on_failed_write() {
    let (_, state) = crate::common::test_app_with_db().await;
    let db = state.db.clone();
    let server = provider(ok(square(5000, NORTH_EAST)), 6).await;
    let endpoint = ProviderEndpoint::parse(Some(&server.uri())).unwrap();
    let app = app_with_state(LoopPlanner::new(demo(), endpoint).unwrap(), state);
    let (_, generated) = generate(app.clone(), walk(5000)).await;
    let chosen = &generated["plan"]["loops"][0]["id"];
    for method in [Method::GET, Method::PUT] {
        let (status, _) = send_request_with_method(
            app.clone(),
            "/api/routes/active",
            method,
            Some(json!({ "route_id": chosen })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (_, selected) = select(app.clone(), chosen).await;
    for body in [
        json!({"route_id": "invalid"}),
        json!({"route_id": chosen, "geometry": { "type": "LineString", "coordinates": [] }}),
    ] {
        let (status, _) = send_request_with_method(
            app.clone(),
            "/api/routes/active",
            Method::PUT,
            Some(body),
            Some("test-api-key"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, _) = select(app.clone(), &json!(uuid::Uuid::new_v4())).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, generated) = generate(app.clone(), json!({ "activity": "walking", "target": { "kind": "steps", "steps": 5000 }, "step_length_cm": 100 })).await;
    sqlx::query("CREATE TRIGGER refuse_selection BEFORE UPDATE ON active_route BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END")
        .execute(&db).await.unwrap();
    let (status, _) = select(app.clone(), &generated["plan"]["loops"][0]["id"]).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let (_, restored) =
        crate::common::send_request(app.clone(), "/api/routes/active", Some("test-api-key")).await;
    assert_eq!(restored, selected);
    sqlx::query("DROP TRIGGER refuse_selection")
        .execute(&db)
        .await
        .unwrap();
    let (_, replacement) = select(app, &generated["plan"]["loops"][0]["id"]).await;
    assert_eq!(replacement["route"]["version"], 2);
}

#[tokio::test]
async fn a_replaced_selection_loses_its_protection_and_may_be_pruned() {
    let (_, state) = crate::common::test_app_with_db().await;
    let server = provider(ok(square(5000, NORTH_EAST)), 6).await;
    let endpoint = ProviderEndpoint::parse(Some(&format!("{}/ors", server.uri())))
        .unwrap()
        .unwrap();
    let app = app_with_state(LoopPlanner::new(demo(), Some(endpoint)).unwrap(), state);
    let request = |stride| json!({ "activity": "walking", "target": { "kind": "distance", "meters": 5000 }, "step_length_cm": stride });
    let (_, first) = generate(app.clone(), request(75)).await;
    let (status, _) = select(app.clone(), &first["plan"]["loops"][0]["id"]).await;
    assert_eq!(status, StatusCode::OK);
    let (_, second) = generate(app.clone(), request(74)).await;
    let (_, replacement) = select(app.clone(), &second["plan"]["loops"][0]["id"]).await;
    assert_eq!(replacement["route"]["version"], 2);
    for stride in (40..=73).chain(76..=137) {
        let (status, _) = generate(app.clone(), request(stride)).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, _) = select(app.clone(), &first["plan"]["loops"][0]["id"]).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, restored) =
        crate::common::send_request(app.clone(), "/api/routes/active", Some("test-api-key")).await;
    assert_eq!(restored, replacement);
    let (status, repeat) = select(app, &second["plan"]["loops"][0]["id"]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeat, replacement);
}

#[tokio::test]
async fn candidate_retention_keeps_reoffered_and_selected_routes_and_prunes_old_choices() {
    let (_, state) = crate::common::test_app_with_db().await;
    let db = state.db.clone();
    let server = provider(ok(square(5000, NORTH_EAST)), 6).await;
    let endpoint = ProviderEndpoint::parse(Some(&server.uri())).unwrap();
    let app = app_with_state(LoopPlanner::new(demo(), endpoint).unwrap(), state);
    let (_, plan) = generate(app.clone(), walk(5000)).await;
    let chosen = &plan["plan"]["loops"][0]["id"];
    let (_, selected) = select(app.clone(), chosen).await;
    let request = |stride| json!({ "activity": "walking", "target": { "kind": "distance", "meters": 5000 }, "step_length_cm": stride });
    let (_, oldest) = generate(app.clone(), request(40)).await;
    let (_, reoffered) = generate(app.clone(), request(41)).await;
    for stride in (42..=135).filter(|stride| *stride != 75) {
        let (status, _) = generate(app.clone(), request(stride)).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (_, refreshed) = generate(app.clone(), request(41)).await;
    assert_eq!(refreshed, reoffered);
    for stride in 136..=150 {
        generate(app.clone(), request(stride)).await;
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM route_candidates")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(count, 97);
    let (status, _) = select(app.clone(), &oldest["plan"]["loops"][0]["id"]).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, restored) =
        crate::common::send_request(app.clone(), "/api/routes/active", Some("test-api-key")).await;
    assert_eq!(restored, selected);
    let (status, _) = select(app, &reoffered["plan"]["loops"][0]["id"]).await;
    assert_eq!(status, StatusCode::OK);
}
