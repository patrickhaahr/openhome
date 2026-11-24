use crate::common::send_request_with_method;
use crate::common::test_app_with_db;
use crate::support::{entry_path, owner, request};
use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use serde_json::Value;
use serde_json::json;

async fn plan(app: &Router, member: &Value, kind: &str) -> Value {
    let (status, added) = request(app, Method::POST, "/api/watchlist/entries",
        json!({"owner_id":member["id"],"kind":kind,"title":"A Quiet Orbit","request_id":uuid::Uuid::new_v4().to_string()})).await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    added["entry"].clone()
}

#[tokio::test]
async fn every_watch_state_transition_preserves_added_date_and_clears_completion_when_leaving() {
    let (app, _) = test_app_with_db().await;
    let member = owner(&app, "Member A").await;
    for kind in ["movie", "series"] {
        let entry = plan(&app, &member, kind).await;
        let path = entry_path(&member, &entry);
        for from in ["planned", "watching", "completed"] {
            for to in ["planned", "watching", "completed"] {
                assert_eq!(
                    request(&app, Method::PATCH, &path, json!({"status":from}))
                        .await
                        .0,
                    StatusCode::OK
                );
                let (status, updated) =
                    request(&app, Method::PATCH, &path, json!({"status":to})).await;
                assert_eq!(status, StatusCode::OK, "{updated}");
                assert_eq!(updated["status"], to);
                assert_eq!(updated["kind"], kind);
                assert_eq!(updated["date_added"], entry["date_added"]);
                let expected = if to == "completed" {
                    entry["date_added"].clone()
                } else {
                    Value::Null
                };
                assert_eq!(updated["completion_date"], expected);
                assert_eq!(
                    request(&app, Method::GET, &path, Value::Null).await.1,
                    updated
                );
            }
        }
    }
}

#[tokio::test]
async fn explicit_date_corrections_persist_and_filters_combine_without_cross_owner_edits() {
    let (app, _) = test_app_with_db().await;
    let first = owner(&app, "Member A").await;
    let second = owner(&app, "Member B").await;
    let entry = plan(&app, &first, "series").await;
    let other = plan(&app, &second, "movie").await;
    let path = entry_path(&first, &entry);
    let payload =
        json!({"status":"completed","date_added":"2024-02-28","completion_date":"2024-02-29"});
    let (status, corrected) = request(&app, Method::PATCH, &path, payload).await;
    assert_eq!(status, StatusCode::OK, "{corrected}");
    assert_eq!(corrected["date_added"], "2024-02-28");
    assert_eq!(corrected["completion_date"], "2024-02-29");
    for patch in [
        json!({}),
        json!({"status":"completed"}),
        json!({"date_added":"2024-02-27"}),
    ] {
        let (_, updated) = request(&app, Method::PATCH, &path, patch).await;
        assert_eq!(updated["completion_date"], "2024-02-29");
    }
    let corrected = request(&app, Method::GET, &path, Value::Null).await.1;
    assert_eq!(
        request(
            &app,
            Method::PATCH,
            &entry_path(&second, &entry),
            json!({"status":"watching"})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for (query, expected) in [
        ("status=completed".to_string(), json!([corrected])),
        (
            format!("owner_id={}", second["id"].as_str().unwrap()),
            json!([other]),
        ),
        (
            format!("owner_id={}&status=planned", first["id"].as_str().unwrap()),
            json!([]),
        ),
    ] {
        assert_eq!(
            request(
                &app,
                Method::GET,
                &format!("/api/watchlist/entries?{query}"),
                Value::Null
            )
            .await,
            (StatusCode::OK, expected)
        );
    }
    assert_eq!(
        request(
            &app,
            Method::PATCH,
            &path,
            json!({"status":"watching","completion_date":null})
        )
        .await
        .1["completion_date"],
        Value::Null
    );
}

#[tokio::test]
async fn invalid_dates_and_contradictory_states_are_rejected_atomically_and_patch_requires_auth() {
    let (app, _) = test_app_with_db().await;
    let member = owner(&app, "Member A").await;
    let entry = plan(&app, &member, "movie").await;
    let path = entry_path(&member, &entry);
    for authorization in [None, Some("wrong-key")] {
        assert_eq!(
            send_request_with_method(
                app.clone(),
                &path,
                Method::PATCH,
                Some(json!({"status":"watching"})),
                authorization
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
    }
    for patch in [
        json!({"status":null}),
        json!({"status":"episode"}),
        json!({"date_added":null}),
        json!({"date_added":"2026-02-30"}),
        json!({"date_added":"2026-1-01"}),
        json!({"date_added":"0000-01-01"}),
        json!({"completion_date":"2026-01-01"}),
        json!({"status":"completed","completion_date":null}),
        json!({"status":"completed","date_added":"2024-03-01","completion_date":"2024-02-29"}),
        json!({"status":"completed","completion_date":"2026-02-29"}),
        json!({"title":"Not a status/date correction"}),
        json!({"owner_id":"other"}),
    ] {
        assert!(
            request(&app, Method::PATCH, &path, patch.clone())
                .await
                .0
                .is_client_error(),
            "{patch}"
        );
        assert_eq!(
            request(&app, Method::GET, &path, Value::Null).await.1,
            entry,
            "{patch}"
        );
    }
    assert!(
        request(
            &app,
            Method::GET,
            "/api/watchlist/entries?status=episode",
            Value::Null
        )
        .await
        .0
        .is_client_error()
    );
}

#[tokio::test]
async fn upgrading_manual_plans_preserves_entries_and_accepted_retry_identity() {
    let (app, state) = test_app_with_db().await;
    sqlx::migrate!("./migrations")
        .undo(&state.db, 9)
        .await
        .unwrap();
    let member = owner(&app, "Member A").await;
    let owner_id = member["id"].as_str().unwrap();
    sqlx::query("INSERT INTO watchlist_entries (id, owner_id, kind, title, date_added) VALUES ('legacy-entry', ?, 'series', 'A Quiet Orbit', '2024-02-28')")
        .bind(owner_id).execute(&state.db).await.unwrap();
    sqlx::query("INSERT INTO watchlist_addition_receipts (request_id, entry_id, owner_id, kind, title) VALUES ('legacy-request', 'legacy-entry', ?, 'series', 'A Quiet Orbit')")
        .bind(owner_id).execute(&state.db).await.unwrap();
    sqlx::migrate!("./migrations").run(&state.db).await.unwrap();
    let (status, replayed) = request(&app, Method::POST, "/api/watchlist/entries",
        json!({"owner_id":owner_id,"kind":"series","title":"A Quiet Orbit","request_id":"legacy-request"})).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(
        replayed,
        json!({"entry":{"id":"legacy-entry","owner_id":owner_id,"kind":"series","title":"A Quiet Orbit","status":"planned","date_added":"2024-02-28","completion_date":null,"tmdb_id":null,"overview":null,"poster_path":null,"release_date":null},"replayed":true})
    );
}

#[tokio::test]
async fn migration_backfills_legacy_completed_rows_and_rollback_is_readable_by_the_planned_only_client()
 {
    let (app, state) = test_app_with_db().await;
    sqlx::migrate!("./migrations")
        .undo(&state.db, 9)
        .await
        .unwrap();
    let member = owner(&app, "Member A").await;
    let owner_id = member["id"].as_str().unwrap();
    for status in ["planned", "watching", "completed"] {
        sqlx::query("INSERT INTO watchlist_entries (id, owner_id, kind, title, status, date_added) VALUES (?, ?, 'series', 'A Quiet Orbit', ?, '2024-02-28')")
            .bind(status).bind(owner_id).bind(status).execute(&state.db).await.unwrap();
    }
    sqlx::migrate!("./migrations").run(&state.db).await.unwrap();
    let entries = request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
        .await
        .1;
    for entry in entries.as_array().unwrap() {
        assert_eq!(entry["status"], entry["id"]);
        let expected = if entry["status"] == "completed" {
            json!("2024-02-28")
        } else {
            Value::Null
        };
        assert_eq!(entry["completion_date"], expected);
    }
    sqlx::migrate!("./migrations")
        .undo(&state.db, 9)
        .await
        .unwrap();
    let states =
        sqlx::query_scalar::<_, String>("SELECT status FROM watchlist_entries ORDER BY id")
            .fetch_all(&state.db)
            .await
            .unwrap();
    assert_eq!(states, ["planned", "planned", "planned"]);
    sqlx::migrate!("./migrations").run(&state.db).await.unwrap();
    let entries = request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
        .await
        .1;
    assert_eq!(entries.as_array().unwrap().len(), 3);
    for entry in entries.as_array().unwrap() {
        assert_eq!(entry["date_added"], "2024-02-28");
        assert_eq!(entry["status"], "planned");
        assert_eq!(entry["completion_date"], Value::Null);
    }
}

#[tokio::test]
async fn missing_tmdb_configuration_keeps_manual_planning_available() {
    let (app, _) = test_app_with_db().await;
    let member = owner(&app, "Member A").await;
    let (status, body) = request(
        &app,
        Method::GET,
        "/api/watchlist/search?kind=movie&query=Orbit",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["code"], "tmdb_unavailable");
    let payload = json!({"owner_id":member["id"],"kind":"movie","title":"A Quiet Orbit","request_id":"manual-fallback"});
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", payload)
            .await
            .0,
        StatusCode::CREATED
    );
}
