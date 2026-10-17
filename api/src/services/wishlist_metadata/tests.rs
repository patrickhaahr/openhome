use super::*;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header_exists, method, path},
};

const OWNER: &str = "member-a";
const LIST: &str = "ideas-a";

async fn app(service: MetadataService) -> Router {
    app_with_db(service).await.0
}

async fn app_with_db(service: MetadataService) -> (Router, sqlx::SqlitePool) {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    sqlx::query("INSERT INTO household_users (id, name) VALUES (?, 'Member A')")
        .bind(OWNER)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO wishlists (id, owner_id, name) VALUES (?, ?, 'Ideas')")
        .bind(LIST)
        .bind(OWNER)
        .execute(&db)
        .await
        .unwrap();
    let state = crate::AppState {
        db: db.clone(),
        noop_db: sqlx::SqlitePool::connect_lazy("sqlite::memory:").unwrap(),
        adguard_service: None,
        docker_service: None,
        ir_service: None,
        switchbot_service: None,
        docker_cache: std::sync::Arc::new(tokio::sync::Mutex::new(crate::DockerCache::default())),
    };
    let key = crate::auth::ApiKey::new("test-api-key".into());
    let router = crate::routes::wishlist::router_with_metadata(None, service)
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            crate::auth::auth_middleware(request, next, key.clone())
        }));
    (router, db)
}

async fn request(app: &Router, payload: Value, key: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/wishlist/url-metadata")
                .header("Authorization", format!("Bearer {key}"))
                .header("Content-Type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 2 * MAX_BYTES).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

fn payload(url: &str) -> Value {
    json!({"owner_id":OWNER,"list_id":LIST,"url":url})
}

fn controlled(server: &MockServer) -> MetadataService {
    MetadataService {
        hosts: std::collections::HashMap::from([
            (
                "shop.example.com".into(),
                (vec!["93.184.215.14:80".parse().unwrap()], *server.address()),
            ),
            (
                "internal.example.com".into(),
                (
                    vec![
                        "93.184.215.14:80".parse().unwrap(),
                        "127.0.0.1:80".parse().unwrap(),
                    ],
                    *server.address(),
                ),
            ),
        ]),
        timeout: Some(Duration::from_millis(200)),
    }
}

fn shop(server: &MockServer, path: &str) -> String {
    format!("http://shop.example.com:{}{path}", server.address().port())
}

#[tokio::test]
async fn authenticated_preview_follows_public_redirects_and_returns_editable_fields_without_writing()
 {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "/product"))
        .mount(&server)
        .await;
    Mock::given(method("GET")).and(path("/product")).respond_with(ResponseTemplate::new(200).set_body_raw(r"<html><head><title>Fallback</title>
        <meta property='og:title' content='A coat &amp; hat'><meta property='og:image' content='/coat.png'>
        <meta property='product:price:amount' content='199.95'><meta property='product:price:currency' content='DKK'>
        <meta property='og:url' content='http://127.0.0.1/admin'></head></html>", "text/html")).mount(&server).await;
    Mock::given(header_exists("authorization"))
        .respond_with(ResponseTemplate::new(401))
        .expect(0)
        .mount(&server)
        .await;
    let app = app(controlled(&server)).await;
    let (status, preview) = request(&app, payload(&shop(&server, "/start")), "test-api-key").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        preview,
        json!({"status":"ready","url":shop(&server,"/product"),"title":"A coat & hat","image_url":shop(&server,"/coat.png"),"price":"DKK 199.95"})
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/wishlists/{LIST}/items"))
                .header("Authorization", "Bearer test-api-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        to_bytes(response.into_body(), MAX_BYTES).await.unwrap(),
        "[]"
    );
}

#[tokio::test]
async fn partial_and_failed_extraction_preserve_url_and_manual_entry() {
    let server = MockServer::start().await;
    for (path, body, content_type, expected) in [
        (
            "/partial",
            "<title>Useful &amp; editable</title><meta property='og:image' content='javascript:alert(1)'>",
            "text/html",
            json!({"status":"partial","title":"Useful & editable"}),
        ),
        (
            "/empty",
            "<html></html>",
            "text/html",
            json!({"status":"partial","title":null}),
        ),
        (
            "/binary",
            "ignored",
            "image/png",
            json!({"status":"unavailable","title":null}),
        ),
    ] {
        Mock::given(wiremock::matchers::path(path))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, content_type))
            .mount(&server)
            .await;
        let (status, preview) = request(
            &app(controlled(&server)).await,
            payload(&shop(&server, path)),
            "test-api-key",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(preview["status"], expected["status"]);
        assert_eq!(preview["title"], expected["title"]);
        assert_eq!(preview["url"], shop(&server, path));
        assert_eq!(preview["image_url"], Value::Null);
        assert_eq!(preview["price"], Value::Null);
    }
}

#[tokio::test]
async fn rejects_internal_dns_addresses_and_redirects_before_contact() {
    let server = MockServer::start().await;
    Mock::given(path("/private-ip"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", "http://127.0.0.1/admin"),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/private-dns"))
        .respond_with(ResponseTemplate::new(302).insert_header(
            "Location",
            shop(&server, "/unused").replace("shop.", "internal."),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/unused"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let app = app(controlled(&server)).await;
    for path in ["/private-ip", "/private-dns"] {
        assert_eq!(
            request(&app, payload(&shop(&server, path)), "test-api-key")
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    for url in [
        "http://localhost/",
        "http://printer.local/",
        "http://user:secret@example.com/",
        "file:///etc/passwd",
        "http://[::ffff:127.0.0.1]/",
        "http://100.64.0.1/",
        "http://0.0.0.0/",
        "http://[2001:db8::1]/",
        "http://169.254.169.254/",
    ] {
        assert_eq!(
            request(&app, payload(url), "test-api-key").await.0,
            StatusCode::BAD_REQUEST,
            "{url}"
        );
    }
}

#[tokio::test]
async fn bounds_response_bytes_time_and_redirect_count() {
    let server = MockServer::start().await;
    Mock::given(path("/large"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw("x".repeat(MAX_BYTES + 1), "text/html"),
        )
        .mount(&server)
        .await;
    Mock::given(path("/slow"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw("<title>Too late</title>", "text/html")
                .set_delay(Duration::from_secs(1)),
        )
        .mount(&server)
        .await;
    Mock::given(path("/loop"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "/loop"))
        .expect(4)
        .mount(&server)
        .await;
    Mock::given(path("/error"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let app = app(controlled(&server)).await;
    for path in ["/large", "/slow", "/loop", "/error"] {
        let (status, preview) = request(&app, payload(&shop(&server, path)), "test-api-key").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(preview["status"], "unavailable", "{path}");
        assert_eq!(preview["title"], Value::Null);
    }
}

#[tokio::test]
async fn streamed_html_without_content_length_still_obeys_the_byte_limit() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let source = Router::new().route(
            "/stream",
            axum::routing::get(|| async {
                let chunks = futures_util::stream::iter([
                    Ok::<_, std::io::Error>("x".repeat(524_288)),
                    Ok("x".repeat(524_289)),
                ]);
                ([("Content-Type", "text/html")], Body::from_stream(chunks))
            }),
        );
        axum::serve(listener, source).await.unwrap();
    });
    let service = MetadataService {
        hosts: std::collections::HashMap::from([(
            "shop.example.com".into(),
            (vec!["93.184.215.14:80".parse().unwrap()], address),
        )]),
        timeout: Some(Duration::from_secs(1)),
    };
    let url = format!("http://shop.example.com:{}/stream", address.port());
    let (status, result) = request(&app(service).await, payload(&url), "test-api-key").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["status"], "unavailable");
    server.abort();
}

#[tokio::test]
async fn authentication_and_explicit_ownership_precede_external_fetches() {
    let server = MockServer::start().await;
    let app = app(controlled(&server)).await;
    let url = shop(&server, "/never");
    assert_eq!(
        request(&app, payload(&url), "wrong-key").await.0,
        StatusCode::UNAUTHORIZED
    );
    let mut wrong = payload(&url);
    wrong["owner_id"] = json!("member-b");
    assert_eq!(
        request(&app, wrong, "test-api-key").await.0,
        StatusCode::CONFLICT
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

// ---------- Stored product pictures ----------

const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d,
];

/// One item whose stored picture address is `image_url`, written straight to the database.
async fn picture_app(service: MetadataService, image_url: Option<&str>) -> Router {
    let (app, db) = app_with_db(service).await;
    sqlx::query(
        "INSERT INTO wishlist_items (id, list_id, title, image_url, request_id) VALUES ('item-a', ?, 'A coat', ?, 'picture-request')",
    )
    .bind(LIST)
    .bind(image_url)
    .execute(&db)
    .await
    .unwrap();
    app
}

async fn picture(app: &Router, item: &str, key: Option<&str>) -> axum::response::Response {
    let mut request = Request::builder().uri(format!(
        "/api/household-users/{OWNER}/wishlists/{LIST}/items/{item}/image"
    ));
    if let Some(key) = key {
        request = request.header("Authorization", format!("Bearer {key}"));
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn serves_a_stored_raster_picture_with_private_caching_and_no_sniffing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/coat.png"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PNG, "image/png"))
        .expect(1)
        .mount(&server)
        .await;
    let app = picture_app(controlled(&server), Some(&shop(&server, "/coat.png"))).await;
    let response = picture(&app, "item-a", Some("test-api-key")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers();
    assert_eq!(headers["content-type"], "image/png");
    assert_eq!(headers["cache-control"], "private, max-age=3600");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(
        headers["content-security-policy"],
        "default-src 'none'; sandbox"
    );
    assert_eq!(
        to_bytes(response.into_body(), MAX_IMAGE_BYTES)
            .await
            .unwrap(),
        PNG
    );
}

#[tokio::test]
async fn pictures_require_the_api_key_before_any_fetch() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PNG, "image/png"))
        .expect(0)
        .mount(&server)
        .await;
    let app = picture_app(controlled(&server), Some(&shop(&server, "/coat.png"))).await;
    assert_eq!(
        picture(&app, "item-a", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        picture(&app, "item-a", Some("wrong-key")).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn unknown_items_and_items_without_pictures_are_not_found() {
    let app = picture_app(MetadataService::default(), None).await;
    assert_eq!(
        picture(&app, "item-b", Some("test-api-key")).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        picture(&app, "item-a", Some("test-api-key")).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn private_and_internal_picture_addresses_are_refused_before_contact() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(PNG, "image/png"))
        .expect(0)
        .mount(&server)
        .await;
    for address in [
        "http://127.0.0.1/coat.png".to_string(),
        "http://192.168.1.10/coat.png".into(),
        "http://[::1]/coat.png".into(),
        "file:///etc/passwd".into(),
        format!(
            "http://internal.example.com:{}/coat.png",
            server.address().port()
        ),
    ] {
        let app = picture_app(controlled(&server), Some(&address)).await;
        let response = picture(&app, "item-a", Some("test-api-key")).await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{address}"
        );
        let body = to_bytes(response.into_body(), MAX_BYTES).await.unwrap();
        assert!(!String::from_utf8_lossy(&body).contains(&address));
    }
}

#[tokio::test]
async fn documents_svg_and_mislabelled_bytes_are_never_relayed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/page"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("<html></html>", "text/html"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/drawing.svg"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            "<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>",
            "image/svg+xml",
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/disguised.png"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("<html></html>", "image/png"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/huge.png"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw([PNG, &vec![0; MAX_IMAGE_BYTES]].concat(), "image/png"),
        )
        .mount(&server)
        .await;
    for resource in [
        "/page",
        "/drawing.svg",
        "/disguised.png",
        "/huge.png",
        "/missing.png",
    ] {
        let app = picture_app(controlled(&server), Some(&shop(&server, resource))).await;
        let response = picture(&app, "item-a", Some("test-api-key")).await;
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{resource}"
        );
        assert_ne!(response.headers()["content-type"], "image/svg+xml");
    }
}
