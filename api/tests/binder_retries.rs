//! Binder addition receipts: replays, conflicts, deletion and simultaneous retries.
mod common;

use axum::http::{Method, StatusCode};
use common::binder::*;
use serde_json::{Value, json};

#[tokio::test]
async fn an_identical_retry_replays_the_original_result_without_adding_again() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let request =
        json!({"owner_id": ada, "card_id": "OP01-001", "quantity": 2, "request_id": "r1"});

    let (_, first) = add(&app, request.clone()).await;
    let (status, retry) = add(&app, request.clone()).await;
    assert_eq!(status, StatusCode::OK, "{retry}");
    assert_eq!(retry["replayed"], true);
    assert_eq!(retry["entry"], first["entry"]);
    assert_eq!(ids(&list(&app, "").await), [("OP01-001", "en", 2)]);

    let (_, merged) = add(
        &app,
        json!({"owner_id": ada, "card_id": "OP01-001", "request_id": "r2"}),
    )
    .await;
    assert_eq!(merged["entry"]["quantity"], 3);
    let (status, later) = add(&app, request).await;
    assert_eq!(status, StatusCode::OK, "{later}");
    assert_eq!(
        later["entry"]["quantity"], 2,
        "the original result, not the merged total"
    );
    assert_eq!(ids(&list(&app, "").await), [("OP01-001", "en", 3)]);
}

#[tokio::test]
async fn a_used_request_id_with_changed_arguments_conflicts_and_changes_nothing() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let bo = owner(&app, "Bo").await;
    let base = json!({"owner_id": ada, "card_id": "OP01-001", "quantity": 1, "request_id": "r1"});
    add(&app, base.clone()).await;

    for changed in [
        json!({"quantity": 2}),
        json!({"language": "ja"}),
        json!({"note": "new"}),
        json!({"card_id": "OP01-002"}),
        json!({"owner_id": bo}),
    ] {
        let mut request = base.clone();
        for (key, value) in changed.as_object().unwrap() {
            request[key] = value.clone();
        }
        let (status, body) = add(&app, request).await;
        assert_eq!(status, StatusCode::CONFLICT, "{changed}: {body}");
    }
    assert_eq!(ids(&list(&app, "").await), [("OP01-001", "en", 1)]);
}

#[tokio::test]
async fn receipts_outlive_the_entry_and_its_owner_so_old_retries_never_recreate_it() {
    let server = catalogue().await;
    let (app, state) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let bo = owner(&app, "Bo").await;
    let for_ada = json!({"owner_id": ada, "card_id": "OP01-001", "request_id": "r1"});
    let for_bo = json!({"owner_id": bo, "card_id": "OP01-001", "request_id": "r2"});
    add(&app, for_ada.clone()).await;
    add(&app, for_bo.clone()).await;

    sqlx::query("DELETE FROM binder_entries WHERE owner_id = ?")
        .bind(&ada)
        .execute(&state.db)
        .await
        .unwrap();
    let (status, body) = add(&app, for_ada).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, _) = send(
        &app,
        Method::DELETE,
        &format!("/api/household-users/{bo}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(list(&app, "").await.is_empty());
    let (status, body) = add(&app, for_bo).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(list(&app, "").await.is_empty());
}

#[tokio::test]
async fn concurrent_additions_claim_one_request_id() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let request =
        json!({"owner_id": ada, "card_id": "OP01-001", "quantity": 2, "request_id": "same"});
    let ((first, a), (second, b)) =
        tokio::join!(add(&app, request.clone()), add(&app, request.clone()));
    let mut statuses = [first, second];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::CREATED], "{a} {b}");
    assert_ne!(a["replayed"], b["replayed"], "{a} {b}");
    assert_eq!(a["entry"]["id"], b["entry"]["id"]);
    let entries = list(&app, "").await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["quantity"], 2);
}

#[tokio::test]
async fn a_retry_of_an_addition_accepted_before_card_value_still_replays_it() {
    let server = catalogue().await;
    let (app, state) = app(&server).await;
    store_rates(&state).await;
    let ada = owner(&app, "Ada").await;
    let seen = json!({"usd": 2.25, "observed_on": days_ago(1).to_string()});
    for (request_id, card, language, legacy_value) in [
        ("r1", "OP01-001", "en", seen),
        ("r2", "OP01-001", "ja", Value::Null),
        ("r3", "OP01-002", "en", Value::Null),
    ] {
        let request = json!({"owner_id": ada, "card_id": card, "language": language,
                             "quantity": 2, "request_id": request_id});
        let (_, first) = add(&app, request.clone()).await;

        let mut legacy = first["entry"].clone();
        legacy["card"]["card_value"] = legacy_value;
        legacy["card"]["inventory_price_usd"] = json!(1.95);
        legacy["card"]
            .as_object_mut()
            .unwrap()
            .remove("value_unavailable");
        let legacy = legacy.to_string();
        sqlx::query("UPDATE binder_addition_receipts SET result = ? WHERE request_id = ?")
            .bind(&legacy)
            .bind(request_id)
            .execute(&state.db)
            .await
            .unwrap();

        let (status, retry) = add(&app, request).await;
        assert_eq!(status, StatusCode::OK, "{request_id}: {retry}");
        assert_eq!(retry["replayed"], true);
        assert_eq!(retry["entry"], first["entry"], "{request_id}");
        let stored: String =
            sqlx::query_scalar("SELECT result FROM binder_addition_receipts WHERE request_id = ?")
                .bind(request_id)
                .fetch_one(&state.db)
                .await
                .unwrap();
        assert_eq!(
            stored, legacy,
            "{request_id}: the receipt is never rewritten"
        );
    }
}
