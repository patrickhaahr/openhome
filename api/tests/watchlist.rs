mod common;

use axum::{
    Router,
    http::{Method, StatusCode},
};
use common::{send_request_with_method, test_app_with_db};
use serde_json::{Value, json};

async fn request(app: &Router, method: Method, path: &str, body: Value) -> (StatusCode, Value) {
    send_request_with_method(app.clone(), path, method, Some(body), Some("test-api-key")).await
}

async fn owner(app: &Router, name: &str) -> Value {
    let (status, body) = request(
        app,
        Method::POST,
        "/api/household-users",
        json!({"name":name}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

fn entry_path(owner: &Value, entry: &Value) -> String {
    format!(
        "/api/household-users/{}/watchlist/entries/{}",
        owner["id"].as_str().unwrap(),
        entry["id"].as_str().unwrap()
    )
}

#[tokio::test]
async fn manual_plans_persist_and_owner_targeted_removal_cannot_change_another_list() {
    let (app, state) = test_app_with_db().await;
    let first = owner(&app, "Member A").await;
    let second = owner(&app, "Member B").await;
    let mut entries = Vec::new();
    for (member, kind, request_id) in [
        (&first, "movie", "movie-1"),
        (&second, "series", "series-1"),
    ] {
        let (status, added) = request(&app, Method::POST, "/api/watchlist/entries", json!({"owner_id":member["id"],"kind":kind,"title":" The Lighthouse Journey ","request_id":request_id})).await;
        assert_eq!(status, StatusCode::CREATED, "{added}");
        let entry = &added["entry"];
        assert_eq!(entry["owner_id"], member["id"]);
        assert_eq!(entry["kind"], kind);
        assert_eq!(entry["title"], "The Lighthouse Journey");
        assert_eq!(entry["status"], "planned");
        uuid::Uuid::parse_str(entry["id"].as_str().unwrap()).unwrap();
        let date =
            chrono::NaiveDate::parse_from_str(entry["date_added"].as_str().unwrap(), "%Y-%m-%d")
                .unwrap();
        assert_eq!(
            date,
            chrono::Utc::now()
                .with_timezone(&chrono_tz::Europe::Copenhagen)
                .date_naive()
        );
        assert_eq!(
            request(&app, Method::GET, &entry_path(member, entry), Value::Null).await,
            (StatusCode::OK, entry.clone())
        );
        entries.push(entry.clone());
    }
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM watchlist_entries")
            .fetch_one(&state.db)
            .await
            .unwrap(),
        2
    );
    for method in [Method::GET, Method::DELETE] {
        assert_eq!(
            request(&app, method, &entry_path(&second, &entries[0]), Value::Null)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        request(
            &app,
            Method::DELETE,
            &entry_path(&first, &entries[0]),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1,
        json!([entries[1]])
    );
    assert_eq!(
        request(
            &app,
            Method::DELETE,
            &entry_path(&first, &entries[0]),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // Existing training records retain their shared schema and endpoint.
    assert_eq!(
        request(&app, Method::GET, "/api/workouts", Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    request(
        &app,
        Method::DELETE,
        &format!("/api/household-users/{}", second["id"].as_str().unwrap()),
        Value::Null,
    )
    .await;
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1,
        json!([])
    );
}

#[tokio::test]
async fn missing_owner_requires_clarification_without_consuming_the_request_id() {
    let (app, _) = test_app_with_db().await;
    let payload = json!({"title":"A Quiet Orbit","kind":"movie","request_id":"clarify"});
    let first = owner(&app, "Member A").await;
    // Even a single household member is not inferred ownership.
    let (status, clarification) = request(
        &app,
        Method::POST,
        "/api/watchlist/entries",
        payload.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(clarification["code"], "watchlist_destination_required");
    assert!(
        clarification["question"]
            .as_str()
            .unwrap()
            .contains("owner")
    );
    assert_eq!(clarification["choices"], json!([first]));
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1,
        json!([])
    );
    let mut selected = payload;
    selected["owner_id"] = first["id"].clone();
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", selected)
            .await
            .0,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn retries_are_atomic_and_receipts_survive_entry_and_owner_deletion() {
    let (app, _) = test_app_with_db().await;
    let first = owner(&app, "Member A").await;
    let second = owner(&app, "Member B").await;
    let payload = json!({"owner_id":first["id"],"title":" A Quiet Orbit ","kind":"movie","request_id":"retry"});
    let (left, right) = tokio::join!(
        request(
            &app,
            Method::POST,
            "/api/watchlist/entries",
            payload.clone()
        ),
        request(
            &app,
            Method::POST,
            "/api/watchlist/entries",
            payload.clone()
        )
    );
    assert!([left.0, right.0].contains(&StatusCode::CREATED));
    assert!([left.0, right.0].contains(&StatusCode::OK));
    assert_eq!(left.1["entry"], right.1["entry"]);
    let mut retry = payload;
    retry["title"] = json!("A Quiet Orbit");
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", retry.clone())
            .await
            .1,
        json!({"entry":left.1["entry"],"replayed":true})
    );
    for (field, value) in [
        ("title", json!("Another title")),
        ("kind", json!("series")),
        ("owner_id", second["id"].clone()),
    ] {
        let mut changed = retry.clone();
        changed[field] = value;
        assert_eq!(
            request(&app, Method::POST, "/api/watchlist/entries", changed)
                .await
                .0,
            StatusCode::CONFLICT
        );
    }
    assert_eq!(
        request(
            &app,
            Method::DELETE,
            &entry_path(&first, &left.1["entry"]),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", retry.clone())
            .await
            .0,
        StatusCode::CONFLICT
    );
    retry["request_id"] = json!("new-intention");
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", retry.clone())
            .await
            .0,
        StatusCode::CREATED
    );
    request(
        &app,
        Method::DELETE,
        &format!("/api/household-users/{}", first["id"].as_str().unwrap()),
        Value::Null,
    )
    .await;
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", retry)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1,
        json!([])
    );
}

#[tokio::test]
async fn authenticated_routes_reject_invalid_manual_plans_without_writing() {
    let (app, _) = test_app_with_db().await;
    let member = owner(&app, "Member A").await;
    let payload = json!({"owner_id":member["id"],"title":"A Quiet Orbit","kind":"movie","request_id":"validation"});
    for (method, path) in [
        (Method::GET, "/api/watchlist/entries"),
        (Method::POST, "/api/watchlist/entries"),
        (
            Method::GET,
            "/api/household-users/missing/watchlist/entries/missing",
        ),
        (
            Method::DELETE,
            "/api/household-users/missing/watchlist/entries/missing",
        ),
    ] {
        for authorization in [None, Some("wrong-key")] {
            assert_eq!(
                send_request_with_method(
                    app.clone(),
                    path,
                    method.clone(),
                    Some(payload.clone()),
                    authorization
                )
                .await
                .0,
                StatusCode::UNAUTHORIZED
            );
        }
    }
    for (field, value) in [
        ("title", json!(" ")),
        ("title", json!("x".repeat(501))),
        ("title", json!("bad\nname")),
        ("request_id", json!("")),
        ("request_id", json!("x".repeat(129))),
        ("kind", json!("episode")),
        ("status", json!("completed")),
        ("date_added", json!("2020-01-01")),
    ] {
        let mut invalid = payload.clone();
        invalid[field] = value;
        assert!(
            request(&app, Method::POST, "/api/watchlist/entries", invalid)
                .await
                .0
                .is_client_error()
        );
    }
    let mut missing = payload.clone();
    missing["owner_id"] = json!("missing");
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", missing)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1,
        json!([])
    );
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", payload)
            .await
            .0,
        StatusCode::CREATED
    );
}

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
        json!({"entry":{"id":"legacy-entry","owner_id":owner_id,"kind":"series","title":"A Quiet Orbit","status":"planned","date_added":"2024-02-28","completion_date":null},"replayed":true})
    );
}
