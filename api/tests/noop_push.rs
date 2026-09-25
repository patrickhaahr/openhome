mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use openhome_api::routes::noop_push::{PUSH_PATH, PushState, router as push_router};
use openhome_api::services::noop_push::{self, PushToken, V1_STREAMS};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

const PUSH_TOKEN: &str = "test-push-token";
const API_KEY: &str = "test-api-key";

async fn memory_push_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    noop_push::initialize(&pool).await.unwrap();
    pool
}

/// Composes the app the way `main` does: API routes behind the API-key layer (applied by
/// `common`), the push router merged afterwards with its own token.
async fn app_with_push_db(push_db: SqlitePool) -> Router {
    let (api, _) = common::test_app_with_db().await;
    let push_state = PushState {
        db: push_db,
        token: PushToken::new(PUSH_TOKEN.to_string()).unwrap(),
    };

    api.merge(push_router(push_state))
}

async fn app() -> Router {
    app_with_push_db(memory_push_db().await).await
}

struct Reply {
    status: StatusCode,
    headers: http::HeaderMap,
    raw: Vec<u8>,
    json: Value,
}

async fn get(app: Router, uri: &str, headers: &[(&str, &str)]) -> Reply {
    let mut builder = Request::builder().method(Method::GET).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let response = app
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let raw = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    let json = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    Reply {
        status,
        headers,
        raw,
        json,
    }
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

async fn capabilities(app: Router, version: &str, token: &str) -> Reply {
    let auth = bearer(token);
    get(
        app,
        PUSH_PATH,
        &[
            ("accept", "application/json"),
            ("authorization", &auth),
            ("noop-push-accept-version", version),
        ],
    )
    .await
}

fn assert_canonical_uuid(value: &Value) -> String {
    let id = value.as_str().expect("receiverStateId is a string");
    let parsed = uuid::Uuid::parse_str(id).expect("receiverStateId is a UUID");
    assert_eq!(id, parsed.hyphenated().to_string(), "canonical lowercase");
    id.to_string()
}

#[tokio::test]
async fn returns_capabilities_with_all_v1_streams() {
    let reply = capabilities(app().await, "1.0", PUSH_TOKEN).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        reply.headers[header::CONTENT_TYPE].to_str().unwrap(),
        "application/json"
    );
    let receiver_state_id = assert_canonical_uuid(&reply.json["receiverStateId"]);
    assert_eq!(
        reply.json,
        json!({
            "type": "capabilities",
            "protocolVersion": "1.0",
            "receiverStateId": receiver_state_id,
            "streams": [
                "hrSample", "rrInterval", "event", "battery", "spo2Sample", "skinTempSample",
                "respSample", "gravitySample", "dailyMetric", "sleepSession", "workout", "journal"
            ],
        })
    );
    assert!(reply.raw.len() < 16 * 1024);
    assert_eq!(V1_STREAMS.len(), 12);
}

#[tokio::test]
async fn selects_first_supported_version_in_sender_order() {
    for offer in ["2.0, 1.0", "1.1,1.0", " 1.0 ", "9.9, 1.0, 1.1"] {
        let reply = capabilities(app().await, offer, PUSH_TOKEN).await;

        assert_eq!(reply.status, StatusCode::OK, "{offer}");
        assert_eq!(reply.json["protocolVersion"], "1.0", "{offer}");
    }
}

#[tokio::test]
async fn unsupported_or_missing_version_is_406_before_auth() {
    let offers: [Option<&str>; 4] = [None, Some(""), Some("2.0, 1.1"), Some("1")];
    for offer in offers {
        for token in [Some(PUSH_TOKEN), Some("wrong"), None] {
            let auth = token.map(bearer);
            let mut headers = vec![];
            if let Some(offer) = offer {
                headers.push(("noop-push-accept-version", offer));
            }
            if let Some(auth) = auth.as_deref() {
                headers.push(("authorization", auth));
            }

            let reply = get(app().await, PUSH_PATH, &headers).await;

            assert_eq!(reply.status, StatusCode::NOT_ACCEPTABLE, "{offer:?}");
            assert_eq!(
                reply.json,
                json!({"type": "error", "protocolVersion": "1.0", "code": "unsupported_version"})
            );
            assert!(reply.raw.len() < 256);
        }
    }
}

#[tokio::test]
async fn missing_or_invalid_token_is_401_and_never_echoed() {
    let cases: [&[(&str, &str)]; 5] = [
        &[],
        &[("authorization", "Bearer wrong-token")],
        &[("authorization", "Bearer ")],
        &[("authorization", "Basic dGVzdC1wdXNoLXRva2Vu")],
        &[("authorization", "Bearer test-api-key")],
    ];
    for auth in cases {
        let mut headers = vec![("noop-push-accept-version", "1.0")];
        headers.extend_from_slice(auth);

        let reply = get(app().await, PUSH_PATH, &headers).await;

        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{auth:?}");
        assert_eq!(reply.headers[header::WWW_AUTHENTICATE], "Bearer");
        assert_eq!(
            reply.json,
            json!({"type": "error", "protocolVersion": "1.0", "code": "unauthorized"})
        );
        let body = String::from_utf8(reply.raw).unwrap();
        assert!(!body.contains("token") && !body.contains("api-key"));
    }
}

#[tokio::test]
async fn push_token_does_not_unlock_the_main_api() {
    let reply = get(
        app().await,
        "/api/health",
        &[("authorization", &bearer(PUSH_TOKEN))],
    )
    .await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);

    let reply = get(
        app().await,
        "/api/health",
        &[("authorization", &bearer(API_KEY))],
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn unknown_push_paths_are_404() {
    for uri in ["/api/noop/push/extra", "/api/noop/other", "/api/noop"] {
        let reply = get(
            app().await,
            uri,
            &[
                ("noop-push-accept-version", "1.0"),
                ("authorization", &bearer(PUSH_TOKEN)),
            ],
        )
        .await;

        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(reply.json["code"], "not_found", "{uri}");
    }
}

#[tokio::test]
async fn receiver_state_id_is_stable_and_follows_rotation() {
    let db = memory_push_db().await;
    let first = capabilities(app_with_push_db(db.clone()).await, "1.0", PUSH_TOKEN).await;
    let second = capabilities(app_with_push_db(db.clone()).await, "1.0", PUSH_TOKEN).await;
    assert_eq!(
        first.json["receiverStateId"],
        second.json["receiverStateId"]
    );

    // Re-running initialization (as on restart) must not replace the persisted ID.
    noop_push::initialize(&db).await.unwrap();
    let after_restart = capabilities(app_with_push_db(db.clone()).await, "1.0", PUSH_TOKEN).await;
    assert_eq!(
        first.json["receiverStateId"],
        after_restart.json["receiverStateId"]
    );

    let rotated = "5FC7B9A0-8055-4E49-A308-3A290F98D81A";
    sqlx::query("UPDATE receiver_state SET receiver_state_id = ? WHERE id = 1")
        .bind(rotated)
        .execute(&db)
        .await
        .unwrap();
    let reply = capabilities(app_with_push_db(db).await, "1.0", PUSH_TOKEN).await;
    assert_eq!(
        reply.json["receiverStateId"],
        rotated.to_ascii_lowercase(),
        "rotation is picked up and canonicalized"
    );
}

#[tokio::test]
async fn receiver_state_id_persists_across_reconnects() {
    let path = std::env::temp_dir().join(format!("noop-push-{}.db", uuid::Uuid::new_v4()));
    let url = format!("sqlite:{}", path.display());

    let first = noop_push::receiver_state_id(&noop_push::connect(&url).await.unwrap())
        .await
        .unwrap();
    let second = noop_push::receiver_state_id(&noop_push::connect(&url).await.unwrap())
        .await
        .unwrap();

    assert_eq!(first, second);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}
