//! Binder Entry corrections: partial updates, the language collision, deletion and the owner cascade.
mod common;

use axum::http::{Method, StatusCode};
use common::binder::*;
use serde_json::{Value, json};

async fn entry_of(app: &axum::Router, owner: &str, card: &str, language: &str) -> Value {
    let (status, added) = add(
        app,
        json!({"owner_id": owner, "card_id": card, "language": language,
               "quantity": 2, "note": "sleeved", "request_id": format!("{owner}-{card}-{language}")}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    added["entry"].clone()
}

fn path(owner: &str, entry: &Value) -> String {
    format!(
        "/api/household-users/{owner}/binder/entries/{}",
        entry["id"].as_str().unwrap()
    )
}

async fn patch(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    send(app, Method::PATCH, uri, Some(body)).await
}

#[tokio::test]
async fn a_partial_update_changes_only_what_it_names_and_reads_back_the_stored_entry() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let entry = entry_of(&app, &ada, "OP01-001", "en").await;
    let uri = path(&ada, &entry);

    let (status, updated) = patch(&app, &uri, json!({"quantity": 5})).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["quantity"], 5);
    assert_eq!(updated["note"], "sleeved", "an absent note is kept");
    assert_eq!(updated["language"], "en", "an absent language is kept");
    assert_eq!(updated["card"], entry["card"]);
    assert_eq!(updated["date_added"], entry["date_added"]);

    let (_, noted) = patch(&app, &uri, json!({"note": " in binder 2 "})).await;
    assert_eq!(noted["note"], "in binder 2");
    assert_eq!(noted["quantity"], 5);

    let (_, cleared) = patch(&app, &uri, json!({"note": null})).await;
    assert!(cleared["note"].is_null(), "{cleared}");
    assert_eq!(cleared["quantity"], 5);

    let (_, unchanged) = patch(&app, &uri, json!({})).await;
    assert_eq!(unchanged, cleared);

    let (status, japanese) = patch(&app, &uri, json!({"language": "ja"})).await;
    assert_eq!(status, StatusCode::OK, "{japanese}");
    assert_eq!(japanese["language"], "ja");
    assert!(
        japanese["card"]["card_value"].is_null()
            && japanese["card"]["inventory_price_usd"].is_null(),
        "an English price is never shown for a Japanese entry: {japanese}"
    );

    let (status, read) = send(&app, Method::GET, &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read, japanese);
    assert_eq!(
        list(&app, &format!("?owner_id={ada}")).await,
        vec![japanese]
    );
}

#[tokio::test]
async fn an_invalid_update_is_rejected_and_changes_nothing() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let entry = entry_of(&app, &ada, "OP01-001", "en").await;
    let uri = path(&ada, &entry);

    for body in [
        json!({"quantity": 0}),
        json!({"quantity": -3}),
        json!({"quantity": 4294967296_i64}),
        json!({"quantity": 1.5}),
        json!({"quantity": null}),
        json!({"language": null}),
        json!({"language": "fr"}),
        json!({"note": "  "}),
        json!({"note": "x".repeat(501)}),
        json!({"note": "bad\ncontrol"}),
        json!({"owner_id": "someone-else"}),
        json!({"quantity": 9, "note": ""}),
    ] {
        let (status, error) = patch(&app, &uri, body.clone()).await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
            "{body} gave {status}: {error}"
        );
        assert_eq!(list(&app, "").await, vec![entry.clone()], "{body}");
    }
}

#[tokio::test]
async fn a_language_change_that_would_duplicate_an_entry_conflicts_and_changes_nothing() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let bo = owner(&app, "Bo").await;
    let english = entry_of(&app, &ada, "OP01-001", "en").await;
    entry_of(&app, &ada, "OP01-001", "ja").await;
    entry_of(&app, &bo, "OP01-001", "en").await;
    let before = list(&app, "").await;

    let (status, error) = patch(
        &app,
        &path(&ada, &english),
        json!({"language": "ja", "quantity": 9, "note": null}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert_eq!(
        list(&app, "").await,
        before,
        "a rejected edit writes nothing"
    );

    let (status, same) = patch(&app, &path(&ada, &english), json!({"language": "en"})).await;
    assert_eq!(status, StatusCode::OK, "{same}");
    assert_eq!(same, english);
}

#[tokio::test]
async fn a_wrong_owner_or_entry_is_not_found_and_changes_nothing() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let bo = owner(&app, "Bo").await;
    let entry = entry_of(&app, &ada, "OP01-001", "en").await;
    let before = list(&app, "").await;
    let missing = json!({"id": "missing"});

    for uri in [
        path(&bo, &entry),
        path(&ada, &missing),
        path("missing", &entry),
    ] {
        let (status, error) = patch(&app, &uri, json!({"quantity": 7})).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {error}");
        let (status, _) = send(&app, Method::GET, &uri, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        let (status, _) = send(&app, Method::DELETE, &uri, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(list(&app, "").await, before);
    }
}

#[tokio::test]
async fn deletion_is_final_so_a_repeat_is_not_found_and_receipts_stay_retired() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let entry = entry_of(&app, &ada, "OP01-001", "en").await;
    let kept = entry_of(&app, &ada, "OP01-002", "en").await;
    let uri = path(&ada, &entry);

    let (status, _) = send(&app, Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(list(&app, "").await, vec![kept]);
    for _ in 0..2 {
        let (status, error) = send(&app, Method::DELETE, &uri, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{error}");
    }
    let (status, _) = patch(&app, &uri, json!({"quantity": 3})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = add(
        &app,
        json!({"owner_id": ada, "card_id": "OP01-001", "quantity": 2, "language": "en",
               "note": "sleeved", "request_id": format!("{ada}-OP01-001-en")}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

#[tokio::test]
async fn removing_a_household_user_removes_their_entries_and_keeps_the_cards() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let bo = owner(&app, "Bo").await;
    entry_of(&app, &ada, "OP01-001", "en").await;
    entry_of(&app, &ada, "OP01-001", "ja").await;
    let bos = entry_of(&app, &bo, "OP01-001", "en").await;
    let requests = catalogue_requests(&server).await;

    let (status, _) = send(
        &app,
        Method::DELETE,
        &format!("/api/household-users/{ada}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(list(&app, &format!("?owner_id={ada}")).await.is_empty());
    assert_eq!(list(&app, "").await, vec![bos]);

    let (status, _) = send(&app, Method::GET, "/api/binder/cards/OP01-001/image", None).await;
    assert_eq!(status, StatusCode::OK, "the Card outlives its owner");
    let (status, _) = send(
        &app,
        Method::DELETE,
        &format!("/api/household-users/{bo}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = send(&app, Method::GET, "/api/binder/cards/OP01-001/image", None).await;
    assert_eq!(status, StatusCode::OK, "the Card outlives every owner");
    assert_eq!(catalogue_requests(&server).await, requests);
}
