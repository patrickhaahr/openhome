mod common;

use axum::{
    Router,
    http::{Method, StatusCode},
};
use common::{send_request_with_method, test_app_with_db};
use openhome_api::{
    auth::{ApiKey, auth_middleware},
    routes,
    services::tmdb::TmdbService,
};
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path, query_param},
};

async fn app(server: &MockServer) -> Router {
    let (_, state) = test_app_with_db().await;
    routes::watchlist::router(Some(
        TmdbService::new(
            "synthetic-private-provider-token",
            format!("{}/3/", server.uri()).parse().unwrap(),
        )
        .unwrap(),
    ))
    .merge(routes::wishlist::router(None))
    .with_state(state)
    .layer(axum::middleware::from_fn(|request, next| {
        auth_middleware(request, next, ApiKey::new("test-api-key".into()))
    }))
}

async fn request(app: &Router, method: Method, path: &str, body: Value) -> (StatusCode, Value) {
    send_request_with_method(app.clone(), path, method, Some(body), Some("test-api-key")).await
}

fn metadata(kind: &str) -> Value {
    let mut value = json!({ "id": 42, "overview": "An entirely fictional journey among the stars.", "poster_path": "/synthetic.jpg" });
    if kind == "movie" {
        value["title"] = json!("A Quiet Orbit");
        value["release_date"] = json!("2020-02-29");
    } else {
        value["name"] = json!("The Lighthouse Journey");
        value["first_air_date"] = json!("2021-01-01");
    }
    value
}

async fn owner(app: &Router) -> Value {
    request(
        app,
        Method::POST,
        "/api/household-users",
        json!({"name":"Member A"}),
    )
    .await
    .1
}

#[tokio::test]
async fn selected_movie_and_series_ids_are_distinct_and_retries_work_without_the_provider() {
    let server = MockServer::start().await;
    let app = app(&server).await;
    let owner = owner(&app).await;
    for (kind, provider_kind) in [("movie", "movie"), ("series", "tv")] {
        Mock::given(method("GET"))
            .and(path(format!("/3/search/{provider_kind}")))
            .and(query_param("query", "Orbit & stars"))
            .and(query_param("include_adult", "false"))
            .and(header(
                "authorization",
                "Bearer synthetic-private-provider-token",
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"results":[metadata(kind)]})),
            )
            .mount(&server)
            .await;
        let (status, results) = request(
            &app,
            Method::GET,
            &format!("/api/watchlist/search?kind={kind}&query=Orbit%20%26%20stars"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{results}");
        assert_eq!(results[0]["kind"], kind);
        assert_eq!(results[0]["tmdb_id"], 42);
        assert_eq!(results[0]["poster_path"], "/synthetic.jpg");
        assert_eq!(
            results[0]["overview"],
            "An entirely fictional journey among the stars."
        );
        Mock::given(method("GET"))
            .and(path(format!("/3/{provider_kind}/42")))
            .respond_with(ResponseTemplate::new(200).set_body_json(metadata(kind)))
            .mount(&server)
            .await;
        let payload = json!({"owner_id":owner["id"], "kind":kind, "title":"Client supplied title", "tmdb_id":42, "request_id":kind});
        let (status, added) = request(
            &app,
            Method::POST,
            "/api/watchlist/entries",
            payload.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{added}");
        assert_eq!(
            added["entry"]["title"],
            if kind == "movie" {
                "A Quiet Orbit"
            } else {
                "The Lighthouse Journey"
            }
        );
        assert_eq!(added["entry"]["status"], "planned");
        assert_eq!(added["entry"]["tmdb_id"], 42);
        server.reset().await;
        let (status, replay) = request(
            &app,
            Method::POST,
            "/api/watchlist/entries",
            payload.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{replay}");
        assert_eq!(replay["entry"], added["entry"]);
        assert_eq!(replay["replayed"], true);
        assert!(server.received_requests().await.unwrap().is_empty());
        let mut changed = payload.clone();
        changed["tmdb_id"] = json!(43);
        assert_eq!(
            request(&app, Method::POST, "/api/watchlist/entries", changed)
                .await
                .0,
            StatusCode::CONFLICT
        );
        // Different intentions for the same owner/provider identity cannot create duplicates.
        Mock::given(method("GET"))
            .and(path(format!("/3/{provider_kind}/42")))
            .respond_with(ResponseTemplate::new(200).set_body_json(metadata(kind)))
            .mount(&server)
            .await;
        let mut duplicate = payload;
        duplicate["request_id"] = json!(format!("other-{kind}"));
        assert_eq!(
            request(&app, Method::POST, "/api/watchlist/entries", duplicate)
                .await
                .0,
            StatusCode::CONFLICT
        );
    }
    let (status, entries) = request(&app, Method::GET, "/api/watchlist/entries", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(entries.as_array().unwrap().len(), 2);
    assert_eq!(entries[0]["tmdb_id"], entries[1]["tmdb_id"]);
    assert_ne!(entries[0]["kind"], entries[1]["kind"]);
}

#[tokio::test]
async fn enrichment_is_owner_targeted_and_preserves_completed_status_and_corrected_dates() {
    let server = MockServer::start().await;
    let app = app(&server).await;
    let owner = owner(&app).await;
    let entry = request(&app, Method::POST, "/api/watchlist/entries", json!({"owner_id":owner["id"], "kind":"series", "title":"Manual plan", "request_id":"manual"})).await.1["entry"].clone();
    let path = format!(
        "/api/household-users/{}/watchlist/entries/{}",
        owner["id"].as_str().unwrap(),
        entry["id"].as_str().unwrap()
    );
    let (status, before) = request(
        &app,
        Method::PATCH,
        &path,
        json!({"status":"completed", "date_added":"2024-02-28", "completion_date":"2024-02-29"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/3/tv/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata("series")))
        .mount(&server)
        .await;
    let wrong_owner = format!(
        "/api/household-users/missing/watchlist/entries/{}/metadata",
        entry["id"].as_str().unwrap()
    );
    assert_eq!(
        request(&app, Method::POST, &wrong_owner, json!({"tmdb_id":42}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert!(server.received_requests().await.unwrap().is_empty());
    let (status, enriched) = request(
        &app,
        Method::POST,
        &format!("{path}/metadata"),
        json!({"tmdb_id":42}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{enriched}");
    for field in [
        "id",
        "owner_id",
        "kind",
        "status",
        "date_added",
        "completion_date",
    ] {
        assert_eq!(enriched[field], before[field], "{field}");
    }
    assert_eq!(enriched["title"], "The Lighthouse Journey");
    assert_eq!(enriched["release_date"], "2021-01-01");
    assert_eq!(
        request(&app, Method::GET, &path, Value::Null).await.1,
        enriched
    );
    server.reset().await;
    assert_eq!(
        request(
            &app,
            Method::POST,
            &format!("{path}/metadata"),
            json!({"tmdb_id":42})
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        request(&app, Method::GET, &path, Value::Null).await.1,
        enriched
    );
}

#[tokio::test]
async fn provider_failures_are_sanitized_and_do_not_consume_addition_intentions() {
    let server = MockServer::start().await;
    let app = app(&server).await;
    let owner = owner(&app).await;
    for (i, response) in [
        ResponseTemplate::new(401).set_body_string("synthetic-private-provider-token"),
        ResponseTemplate::new(429),
        ResponseTemplate::new(500),
        ResponseTemplate::new(200).set_body_string("invalid JSON"),
        ResponseTemplate::new(302).insert_header("location", format!("{}/leak", server.uri())),
        ResponseTemplate::new(200).set_body_json(json!({"id":43,"title":"Wrong identity"})),
    ]
    .into_iter()
    .enumerate()
    {
        server.reset().await;
        Mock::given(method("GET"))
            .respond_with(response)
            .mount(&server)
            .await;
        let payload = json!({"owner_id":owner["id"], "kind":"movie", "title":"Manual fallback", "request_id":format!("fallback-{i}"), "tmdb_id":42});
        let (status, body) = request(
            &app,
            Method::POST,
            "/api/watchlist/entries",
            payload.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(body["code"], "tmdb_unavailable");
        assert!(
            !body
                .to_string()
                .contains("synthetic-private-provider-token")
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
        let mut manual = payload;
        manual.as_object_mut().unwrap().remove("tmdb_id");
        let (status, added) = request(&app, Method::POST, "/api/watchlist/entries", manual).await;
        assert_eq!(status, StatusCode::CREATED, "{added}");
        assert_eq!(added["entry"]["tmdb_id"], Value::Null);
        assert_eq!(added["entry"]["title"], "Manual fallback");
    }
}

#[tokio::test]
async fn search_requires_authentication_valid_input_and_safe_image_paths() {
    let server = MockServer::start().await;
    let app = app(&server).await;
    for path in [
        "/api/watchlist/search?kind=movie&query=Orbit",
        "/api/household-users/missing/watchlist/entries/missing/metadata",
    ] {
        let method = if path.contains("/search") {
            Method::GET
        } else {
            Method::POST
        };
        assert_eq!(
            send_request_with_method(app.clone(), path, method, Some(json!({"tmdb_id":42})), None)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    for path in [
        "/api/watchlist/search?kind=person&query=Orbit",
        "/api/watchlist/search?kind=movie&query=%20",
        "/api/watchlist/search?kind=movie&query=%0A",
    ] {
        assert!(
            request(&app, Method::GET, path, Value::Null)
                .await
                .0
                .is_client_error()
        );
    }
    assert!(server.received_requests().await.unwrap().is_empty());
    for unsafe_path in [
        "/",
        "/..",
        "//private.invalid/poster.jpg",
        "/../secret",
        "/image.jpg?token=secret",
        "https://private.invalid/image",
    ] {
        server.reset().await;
        let mut item = metadata("movie");
        item["poster_path"] = json!(unsafe_path);
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"results":[item]})))
            .mount(&server)
            .await;
        let (status, results) = request(
            &app,
            Method::GET,
            "/api/watchlist/search?kind=movie&query=Orbit",
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(results[0]["poster_path"], Value::Null);
    }
}
