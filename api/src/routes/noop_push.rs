//! NOOP self-hosted push endpoint. The route is fixed at [`PUSH_PATH`] and authenticated with
//! `NOOP_PUSH_TOKEN`, not `API_KEY`, so this router is merged outside the API-key middleware.
//! `GET` serves capabilities; `POST` accepts one NDJSON batch.

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::StreamExt;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::services::noop_push::ingest::{self, ContentCoding, IngestError, MAX_WIRE_BYTES};
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
        .route("/push", get(capabilities).post(push_batch))
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
        return unauthorized(protocol_version);
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

async fn push_batch(State(state): State<PushState>, headers: HeaderMap, body: Body) -> Response {
    // Authenticate before reading any of the entity.
    if !is_authorized(&headers, &state.token) {
        tracing::warn!(
            route = PUSH_PATH,
            authorization_present = headers.contains_key(header::AUTHORIZATION),
            status = StatusCode::UNAUTHORIZED.as_u16(),
            "NOOP push batch: missing or invalid bearer token"
        );
        return unauthorized(CURRENT_VERSION);
    }
    if !is_ndjson(&headers) {
        return rejected(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_media_type");
    }
    let Some(coding) = content_coding(&headers) else {
        return rejected(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_content_encoding",
        );
    };

    let wire = match read_bounded(body, MAX_WIRE_BYTES).await {
        Ok(wire) => wire,
        Err(error) => return rejected(status_for(&error), error.code()),
    };
    // Decoding, parsing and hashing up to 4 MiB is CPU-bound: keep it off the async workers.
    let prepared = match tokio::task::spawn_blocking(move || ingest::prepare(coding, wire)).await {
        Ok(prepared) => prepared,
        Err(error) => {
            tracing::error!(route = PUSH_PATH, error = ?error, "NOOP push batch: prepare task failed");
            return push_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                CURRENT_VERSION,
                "internal_error",
            );
        }
    };
    let outcome = match prepared {
        Ok(prepared) => ingest::apply(&state.db, prepared).await,
        Err(error) => Err(error),
    };

    match outcome {
        Ok(accepted) => {
            tracing::info!(
                route = PUSH_PATH,
                stream = accepted.stream,
                records = accepted.records,
                replayed = accepted.replayed,
                "NOOP push batch accepted"
            );
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                accepted.ack,
            )
                .into_response()
        }
        Err(IngestError::Storage(source)) => {
            tracing::error!(
                route = PUSH_PATH,
                error = ?source,
                "NOOP push batch: storage failure"
            );
            push_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                CURRENT_VERSION,
                "storage_unavailable",
            )
        }
        Err(error) => rejected(status_for(&error), error.code()),
    }
}

fn status_for(error: &IngestError) -> StatusCode {
    match error {
        IngestError::Malformed(_) => StatusCode::BAD_REQUEST,
        IngestError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        IngestError::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
        IngestError::Conflict => StatusCode::CONFLICT,
        IngestError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Accepts `application/x-ndjson`, optionally with a UTF-8 charset parameter.
fn is_ndjson(headers: &HeaderMap) -> bool {
    let Some(value) = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let mut parts = value.split(';').map(str::trim);
    let media_type_ok = parts
        .next()
        .is_some_and(|media_type| media_type.eq_ignore_ascii_case("application/x-ndjson"));
    media_type_ok
        && parts.all(|parameter| match parameter.split_once('=') {
            Some((name, value)) if name.trim().eq_ignore_ascii_case("charset") => {
                let value = value.trim().trim_matches('"');
                value.eq_ignore_ascii_case("utf-8") || value.eq_ignore_ascii_case("utf8")
            }
            _ => true,
        })
}

/// Identity or gzip; any other or repeated `Content-Encoding` is unsupported.
fn content_coding(headers: &HeaderMap) -> Option<ContentCoding> {
    let mut values = headers.get_all(header::CONTENT_ENCODING).iter();
    let value = match (values.next(), values.next()) {
        (None, _) => None,
        (Some(value), None) => Some(value.to_str().ok()?),
        (Some(_), Some(_)) => return None,
    };
    ContentCoding::parse(value)
}

/// Buffers the wire entity, failing as soon as it exceeds `limit`.
async fn read_bounded(body: Body, limit: usize) -> Result<Vec<u8>, IngestError> {
    let mut stream = body.into_data_stream();
    let mut wire = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| IngestError::Malformed("unreadable_body"))?;
        if wire.len() + chunk.len() > limit {
            return Err(IngestError::TooLarge);
        }
        wire.extend_from_slice(&chunk);
    }
    Ok(wire)
}

fn rejected(status: StatusCode, code: &'static str) -> Response {
    tracing::warn!(
        route = PUSH_PATH,
        status = status.as_u16(),
        code,
        "NOOP push batch rejected"
    );
    push_error(status, CURRENT_VERSION, code)
}

fn unauthorized(protocol_version: &'static str) -> Response {
    let mut response = push_error(StatusCode::UNAUTHORIZED, protocol_version, "unauthorized");
    response
        .headers_mut()
        .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    response
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
