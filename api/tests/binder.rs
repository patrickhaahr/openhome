mod common;

use axum::http::{Method, StatusCode};
use common::binder::*;
use serde_json::{Value, json};

fn assert_no_english_price(entry: &Value) {
    assert!(
        entry["card"]["card_value"].is_null() && entry["card"]["inventory_price_usd"].is_null(),
        "an English price is never shown for a Japanese entry: {entry}"
    );
}

#[tokio::test]
async fn an_addition_stores_the_card_itself_and_returns_the_entry_with_its_card() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;

    let (status, added) = add(
        &app,
        json!({"owner_id": ada, "card_id": "OP01-001", "note": " from a trade ", "request_id": "r1"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    assert_eq!(added["replayed"], false);
    let entry = &added["entry"];
    assert_eq!(entry["owner_id"], json!(ada));
    assert_eq!(entry["language"], "en");
    assert_eq!(entry["quantity"], 1);
    assert_eq!(entry["note"], "from a trade");
    let today = chrono::Utc::now()
        .with_timezone(&chrono_tz::Europe::Copenhagen)
        .date_naive()
        .to_string();
    assert_eq!(entry["date_added"], json!(today));
    assert_eq!(entry["card"]["id"], "OP01-001");
    assert_eq!(entry["card"]["card_code"], "OP01-001");
    assert_eq!(entry["card"]["name"], "Invented OP01-001");
    assert_eq!(
        entry["card"]["card_value"],
        json!({"usd": 2.25, "observed_on": "2026-11-12"})
    );
    assert_eq!(
        entry["card"]["image_path"],
        "/api/binder/cards/OP01-001/image"
    );

    let stored = list(&app, &format!("?owner_id={ada}")).await;
    assert_eq!(stored, vec![entry.clone()]);
    let (status, searched) = send(
        &app,
        Method::GET,
        "/api/binder/cards?card_code=OP01-001",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(searched["cards"][0]["id"], "OP01-001");
    assert_eq!(catalogue_requests(&server).await, 3);
}

#[tokio::test]
async fn a_missing_owner_asks_which_household_user_and_writes_nothing() {
    let server = catalogue().await;
    let (app, state) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let _bo = owner(&app, "Bo").await;

    let (status, question) = add(&app, json!({"card_id": "OP01-001", "request_id": "r1"})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{question}");
    assert_eq!(question["code"], "binder_destination_required");
    assert!(question["question"].as_str().unwrap().contains("Binder"));
    let choices = question["choices"].as_array().unwrap();
    assert_eq!(
        choices
            .iter()
            .map(|choice| choice["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Ada", "Bo"]
    );
    assert_eq!(choices[0]["id"], json!(ada));
    assert!(list(&app, "").await.is_empty());
    assert_eq!(catalogue_requests(&server).await, 0);
    let receipts = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM binder_addition_receipts")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(receipts, 0);

    let (status, unknown) = add(
        &app,
        json!({"owner_id": "nobody", "card_id": "OP01-001", "request_id": "r2"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{unknown}");
    assert!(list(&app, "").await.is_empty());
}

#[tokio::test]
async fn the_same_card_and_language_merges_while_another_language_is_its_own_entry() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let bo = owner(&app, "Bo").await;

    add(&app, json!({"owner_id": ada, "card_id": "OP01-001", "quantity": 2, "note": "sleeved", "request_id": "r1"})).await;
    let (status, merged) = add(
        &app,
        json!({"owner_id": ada, "card_id": "OP01-001", "quantity": 3, "request_id": "r2"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{merged}");
    assert_eq!(merged["entry"]["quantity"], 5);
    assert_eq!(
        merged["entry"]["note"], "sleeved",
        "an omitted note is kept"
    );
    let (_, renoted) = add(
        &app,
        json!({"owner_id": ada, "card_id": "OP01-001", "note": "binder 2", "request_id": "r3"}),
    )
    .await;
    assert_eq!(renoted["entry"]["quantity"], 6);
    assert_eq!(renoted["entry"]["note"], "binder 2");

    let (_, japanese) = add(
        &app,
        json!({"owner_id": ada, "card_id": "OP01-001", "language": "ja", "request_id": "r4"}),
    )
    .await;
    assert_eq!(japanese["entry"]["language"], "ja");
    assert_eq!(japanese["entry"]["quantity"], 1);
    assert_no_english_price(&japanese["entry"]);
    let (status, replayed) = add(
        &app,
        json!({"owner_id": ada, "card_id": "OP01-001", "language": "ja", "request_id": "r4"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["replayed"], true);
    assert_no_english_price(&replayed["entry"]);
    add(
        &app,
        json!({"owner_id": bo, "card_id": "OP01-001", "request_id": "r5"}),
    )
    .await;

    let ada_entries = list(&app, &format!("?owner_id={ada}")).await;
    assert_eq!(
        ids(&ada_entries),
        [("OP01-001", "en", 6), ("OP01-001", "ja", 1)]
    );
    assert!(ada_entries[0]["card"]["card_value"]["usd"].is_number());
    assert_eq!(ada_entries[0]["card"]["inventory_price_usd"], 1.95);
    assert_no_english_price(&ada_entries[1]);
    assert_eq!(
        ids(&list(&app, &format!("?owner_id={bo}")).await),
        [("OP01-001", "en", 1)]
    );
}

#[tokio::test]
async fn a_card_the_catalogue_does_not_list_is_rejected_and_writes_nothing() {
    let server = catalogue().await;
    let (app, state) = app(&server).await;
    let ada = owner(&app, "Ada").await;

    for (card_id, request_id) in [
        ("OP05-777", "unlisted"),
        ("OP05-777_p1", "unlisted-parallel"),
        ("OP01-001_p9", "unknown-variant"),
        ("op01-001", "lower-case"),
        ("SPECIAL_BUGGY_BALL", "no-code"),
        ("EB03_OP05-777_p1", "unlisted-code-in-id"),
    ] {
        let (status, body) = add(
            &app,
            json!({"owner_id": ada, "card_id": card_id, "request_id": request_id}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{card_id}: {body}");
        assert_eq!(body["code"], "card_not_listed");
    }
    assert!(list(&app, "").await.is_empty());
    let rows = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM binder_addition_receipts")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(rows, 0, "a rejected addition consumes no request_id");

    let (status, body) = add(
        &app,
        json!({"owner_id": ada, "card_id": "OP01-001", "request_id": "unlisted"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[tokio::test]
async fn a_catalogue_outage_is_the_existing_503_and_writes_nothing() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;

    for (card_id, request_id) in [
        ("OP77-503", "r1"),
        ("OP77-503_p1", "r2"),
        ("EB03_OP77-503_p1", "r3"),
    ] {
        let (status, body) = add(
            &app,
            json!({"owner_id": ada, "card_id": card_id, "request_id": request_id}),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{card_id}: {body}");
        assert_eq!(body["code"], "card_data_unavailable");
    }
    assert!(list(&app, "").await.is_empty());
}

#[tokio::test]
async fn a_parallel_nobody_searched_for_is_added_and_its_card_stored() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;

    for (card_id, code) in [
        ("OP01-001_p1", "OP01-001"),
        ("EB03_OP09-034_p1", "OP09-034"),
    ] {
        let (status, added) = add(
            &app,
            json!({"owner_id": ada, "card_id": card_id, "request_id": card_id}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{card_id}: {added}");
        assert_eq!(added["entry"]["card"]["id"], card_id);
        assert_eq!(added["entry"]["card"]["card_code"], code);
    }
    assert_eq!(
        ids(&list(&app, "").await),
        [("OP01-001_p1", "en", 1), ("EB03_OP09-034_p1", "en", 1)]
    );
    let asked = catalogue_requests(&server).await;
    let (_, searched) = send(
        &app,
        Method::GET,
        "/api/binder/cards?card_code=OP01-001",
        None,
    )
    .await;
    assert_eq!(searched["cards"].as_array().unwrap().len(), 2);
    assert_eq!(catalogue_requests(&server).await, asked);
}

#[tokio::test]
async fn quantity_has_no_upper_bound_below_what_a_sum_of_additions_can_hold() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let most = u64::from(u32::MAX);

    for request_id in ["a", "b"] {
        let (status, body) = add(
            &app,
            json!({"owner_id": ada, "card_id": "OP01-001", "quantity": most, "request_id": request_id}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    assert_eq!(
        ids(&list(&app, "").await),
        [("OP01-001", "en", 2 * most as i64)]
    );
}

#[tokio::test]
async fn a_card_found_by_search_is_added_by_its_parallel_id_without_asking_the_catalogue_again() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    send(
        &app,
        Method::GET,
        "/api/binder/cards?card_code=OP09-034",
        None,
    )
    .await;
    let asked = catalogue_requests(&server).await;

    let (status, added) = add(
        &app,
        json!({"owner_id": ada, "card_id": "EB03_OP09-034_p1", "request_id": "r1"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    assert_eq!(added["entry"]["card"]["id"], "EB03_OP09-034_p1");
    assert_eq!(added["entry"]["card"]["card_code"], "OP09-034");
    assert_eq!(catalogue_requests(&server).await, asked);
}

#[tokio::test]
async fn invalid_additions_are_rejected_before_anything_is_written() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let valid = json!({"owner_id": ada, "card_id": "OP01-001", "request_id": "r1"});

    for changed in [
        json!({"quantity": 0}),
        json!({"quantity": -1}),
        json!({"quantity": 4_294_967_296_u64}),
        json!({"note": "  "}),
        json!({"note": "a".repeat(501)}),
        json!({"card_id": "OP01-001/../x"}),
        json!({"request_id": ""}),
    ] {
        let mut request = valid.clone();
        for (key, value) in changed.as_object().unwrap() {
            request[key] = value.clone();
        }
        let (status, body) = add(&app, request).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{changed}: {body}");
    }
    assert!(list(&app, "").await.is_empty());
    assert_eq!(catalogue_requests(&server).await, 0);
}

#[tokio::test]
async fn lists_come_back_in_card_code_order_with_the_normal_print_before_its_parallels() {
    let server = catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let bo = owner(&app, "Bo").await;

    for code in ["OP01-010", "OP09-034"] {
        send(
            &app,
            Method::GET,
            &format!("/api/binder/cards?card_code={code}"),
            None,
        )
        .await;
    }
    for (request, (member, card_id, language)) in [
        (&ada, "ST01-001", "en"),
        (&ada, "OP10-001", "en"),
        (&ada, "OP01-010_p1", "en"),
        (&ada, "OP09-034", "en"),
        (&ada, "EB03_OP09-034_p1", "en"),
        (&ada, "OP01-010", "ja"),
        (&ada, "OP01-010", "en"),
        (&ada, "OP01-002", "en"),
        (&bo, "OP01-001", "en"),
    ]
    .into_iter()
    .enumerate()
    {
        let (status, body) = add(&app, json!({"owner_id": member, "card_id": card_id, "language": language, "request_id": format!("r{request}")})).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }

    assert_eq!(
        ids(&list(&app, &format!("?owner_id={ada}")).await),
        [
            ("OP01-002", "en", 1),
            ("OP01-010", "en", 1),
            ("OP01-010", "ja", 1),
            ("OP01-010_p1", "en", 1),
            ("OP09-034", "en", 1),
            ("EB03_OP09-034_p1", "en", 1),
            ("OP10-001", "en", 1),
            ("ST01-001", "en", 1),
        ]
    );
    let everyone = list(&app, "").await;
    assert_eq!(everyone.len(), 9);
    assert_eq!(
        everyone[0]["owner_id"],
        json!(ada),
        "owners are listed by name"
    );
    assert_eq!(everyone[8]["owner_id"], json!(bo));
    assert_eq!(list(&app, "?owner_id=nobody").await, Vec::<Value>::new());
}
