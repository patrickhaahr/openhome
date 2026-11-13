mod common;

use axum::{
    Router,
    http::{Method, StatusCode},
};
use common::{send_request_with_method, test_app_with_db};
use image::{ImageEncoder, Rgb, RgbImage, codecs::png::PngEncoder};
use openhome_api::{
    AppState,
    auth::{ApiKey, auth_middleware},
    routes,
    services::optcg::Optcg,
};
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{method, path_regex},
};

const LISTED: [(&str, &[&str]); 6] = [
    ("OP01-001", &["OP01-001", "OP01-001_p1"]),
    ("OP01-002", &["OP01-002"]),
    ("OP01-010", &["OP01-010", "OP01-010_p1", "OP01-010_p2"]),
    ("OP09-034", &["EB03_OP09-034_p1", "OP09-034"]),
    ("OP10-001", &["OP10-001"]),
    ("ST01-001", &["ST01-001"]),
];
const OUTAGE: &str = "OP77-503";

fn png() -> Vec<u8> {
    let pixels = RgbImage::from_pixel(60, 84, Rgb([200, 30, 30]));
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(pixels.as_raw(), 60, 84, image::ExtendedColorType::Rgb8)
        .unwrap();
    bytes
}

async fn catalogue() -> MockServer {
    let server = MockServer::start().await;
    let origin = server.uri();
    Mock::given(method("GET"))
        .and(path_regex(r"^/api/sets/card/[^/]+/$"))
        .respond_with(move |request: &Request| {
            let code = request.url.path().split('/').nth(4).unwrap();
            if code == OUTAGE {
                return ResponseTemplate::new(503);
            }
            let Some((_, ids)) = LISTED.iter().find(|(listed, _)| *listed == code) else {
                return ResponseTemplate::new(404)
                    .set_body_json(json!({"error": "Card not found."}));
            };
            ResponseTemplate::new(200).set_body_json(
                ids.iter()
                    .map(|id| {
                        json!({
                            "inventory_price": 1.95, "market_price": 2.25,
                            "card_name": format!("Invented {id}"), "set_name": "Synthetic Set",
                            "card_text": "[Your Turn] Gain +1000 power.", "set_id": "OP-01",
                            "rarity": "L", "card_set_id": code, "card_color": "Red",
                            "card_type": "Leader", "life": "5", "card_cost": null,
                            "card_power": "5000", "sub_types": "Invented Crew",
                            "counter_amount": null, "attribute": "Slash",
                            "date_scraped": "2026-11-12", "card_image_id": id,
                            "card_image": format!("{origin}/media/static/Card_Images/{id}.jpg"),
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/media/static/Card_Images/[^/]+\.jpg$"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(png()))
        .mount(&server)
        .await;
    server
}

async fn catalogue_requests(server: &MockServer) -> usize {
    server.received_requests().await.unwrap().len()
}

async fn app(server: &MockServer) -> (Router, AppState) {
    let (_, state) = test_app_with_db().await;
    let catalogue = Optcg::new(server.uri().parse().unwrap()).unwrap();
    let app = routes::wishlist::router(None)
        .merge(routes::cards::router(catalogue.clone()))
        .merge(routes::binder::router(catalogue))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn(|request, next| {
            auth_middleware(request, next, ApiKey::new("test-api-key".into()))
        }));
    (app, state)
}

async fn send(app: &Router, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    send_request_with_method(app.clone(), uri, method, body, Some("test-api-key")).await
}

async fn owner(app: &Router, name: &str) -> String {
    let (status, body) = send(
        app,
        Method::POST,
        "/api/household-users",
        Some(json!({"name": name})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().to_owned()
}

async fn add(app: &Router, body: Value) -> (StatusCode, Value) {
    send(app, Method::POST, "/api/binder/entries", Some(body)).await
}

async fn list(app: &Router, query: &str) -> Vec<Value> {
    let (status, body) = send(
        app,
        Method::GET,
        &format!("/api/binder/entries{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body.as_array().unwrap().clone()
}

fn ids(entries: &[Value]) -> Vec<(&str, &str, i64)> {
    entries
        .iter()
        .map(|entry| {
            (
                entry["card"]["id"].as_str().unwrap(),
                entry["language"].as_str().unwrap(),
                entry["quantity"].as_i64().unwrap(),
            )
        })
        .collect()
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

    let (status, body) = add(
        &app,
        json!({"owner_id": ada, "card_id": "OP77-503", "request_id": "r1"}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["code"], "card_data_unavailable");
    assert!(list(&app, "").await.is_empty());
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
        json!({"quantity": 1000}),
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
