//! NOOP self-hosted push endpoint. The route is fixed at [`PUSH_PATH`] and authenticated with
//! `NOOP_PUSH_TOKEN`, not `API_KEY`, so this router is merged outside the API-key middleware.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::services::noop_push::{self, CURRENT_VERSION, PushToken, V1_STREAMS, negotiate_version};

/// The fixed push route the NOOP client is configured with (`https://<host>/api/noop/push`).
pub const PUSH_PATH: &str = "/api/noop/push";

const ACCEPT_VERSION_HEADER: &str = "noop-push-accept-version";

#[derive(Clone)]
pub struct PushState {
    pub db: SqlitePool,
    pub token: PushToken,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Capabilities {
    #[serde(rename = "type")]
    kind: &'static str,
    protocol_version: &'static str,
    receiver_state_id: String,
    streams: &'static [&'static str],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PushError {
    #[serde(rename = "type")]
    kind: &'static str,
    protocol_version: &'static str,
    code: &'static str,
}

pub fn router(state: PushState) -> Router {
    let push_routes = Router::new()
        .route("/push", get(capabilities))
        .fallback(not_found);

    Router::new()
        .nest("/api/noop", push_routes)
        .with_state(state)
}

async fn capabilities(State(state): State<PushState>, headers: HeaderMap) -> Response {
    let offers = headers
        .get_all(ACCEPT_VERSION_HEADER)
        .iter()
        .filter_map(|value| value.to_str().ok());
    let Some(protocol_version) = negotiate_version(offers) else {
        tracing::warn!(
            route = PUSH_PATH,
            accept_version_present = headers.contains_key(ACCEPT_VERSION_HEADER),
            status = StatusCode::NOT_ACCEPTABLE.as_u16(),
            "NOOP push capabilities: no common protocol version"
        );
        return push_error(
            StatusCode::NOT_ACCEPTABLE,
            CURRENT_VERSION,
            "unsupported_version",
        );
    };

    if !is_authorized(&headers, &state.token) {
        tracing::warn!(
            route = PUSH_PATH,
            protocol_version,
            authorization_present = headers.contains_key(header::AUTHORIZATION),
            status = StatusCode::UNAUTHORIZED.as_u16(),
            "NOOP push capabilities: missing or invalid bearer token"
        );
        let mut response = push_error(StatusCode::UNAUTHORIZED, protocol_version, "unauthorized");
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        return response;
    }

    let receiver_state_id = match noop_push::receiver_state_id(&state.db).await {
        Ok(id) => id,
        Err(error) => {
            tracing::error!(
                route = PUSH_PATH,
                error = ?error,
                "NOOP push capabilities: receiver state unavailable"
            );
            return push_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                protocol_version,
                "receiver_state_unavailable",
            );
        }
    };

    Json(Capabilities {
        kind: "capabilities",
        protocol_version,
        receiver_state_id: receiver_state_id.hyphenated().to_string(),
        streams: &V1_STREAMS,
    })
    .into_response()
}

async fn not_found() -> Response {
    push_error(StatusCode::NOT_FOUND, CURRENT_VERSION, "not_found")
}

fn is_authorized(headers: &HeaderMap, token: &PushToken) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .is_some_and(|(_, presented)| token.matches(presented))
}

fn push_error(status: StatusCode, protocol_version: &'static str, code: &'static str) -> Response {
    let body = PushError {
        kind: "error",
        protocol_version,
        code,
    };
    (status, Json(body)).into_response()
}
