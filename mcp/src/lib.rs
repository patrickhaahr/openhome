//! Hermes MCP adapter for OpenHome.
//!
//! A Streamable HTTP MCP server for read-only training and Food database reads, and writable
//! Wishlists, Watchlists, Important Dates, Meals, Supplements and Meal Logs. Each tool
//! makes one authenticated API request and passes its JSON unchanged. The adapter has no
//! database access or domain rules; Axum owns training interpretation, destination resolution
//! and mutation deduplication (ADRs 0002 and 0005).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::{
    ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde_json::{Value, json};
use subtle::ConstantTimeEq;
use url::Url;

mod important_dates;
mod instructions;
mod nutrition;
mod patch;
mod training;
mod watchlist;
mod wishlist;

use instructions::INSTRUCTIONS;
pub use training::{
    ExerciseHistoryRequest, MetricTrendRequest, NoopWorkoutDetailRequest, RecoveryOnDayRequest,
    SleepRecentRequest, TrainingContextRequest, TrendMetric, WorkoutsOnDayRequest,
};

/// Where the MCP endpoint is mounted.
pub const MCP_PATH: &str = "/mcp";

const BEARER_PREFIX: &str = "Bearer ";
const API_TIMEOUT: Duration = Duration::from_secs(15);

/// The OpenHome `API_KEY`: it authorizes MCP clients and the adapter's own API requests. Never
/// printed.
#[derive(Clone)]
pub struct ApiKey(Arc<str>);

impl ApiKey {
    pub fn new(key: String) -> anyhow::Result<Self> {
        anyhow::ensure!(!key.trim().is_empty(), "API_KEY must not be blank");
        anyhow::ensure!(
            key.trim() == key,
            "API_KEY must not have leading or trailing whitespace (check quoting in .env)"
        );
        Ok(Self(key.into()))
    }

    fn matches(&self, presented: &str) -> bool {
        presented.as_bytes().ct_eq(self.0.as_bytes()).into()
    }
}

/// Runtime settings, read from the environment.
pub struct Config {
    /// Base URL of the OpenHome API, e.g. `http://openhome-api:8000`.
    pub api_url: Url,
    pub api_key: ApiKey,
    pub bind_addr: SocketAddr,
    /// `Host` values the MCP endpoint answers to (DNS-rebinding protection).
    pub allowed_hosts: Vec<String>,
}

impl Config {
    /// Reads `OPENHOME_API_URL` and `API_KEY` (required), `MCP_BIND_ADDR` (default
    /// `0.0.0.0:8001`) and `MCP_ALLOWED_HOSTS` (comma-separated, default loopback only).
    pub fn from_env() -> anyhow::Result<Self> {
        let required = |name: &str| {
            std::env::var(name)
                .map_err(|_| anyhow::anyhow!("{name} environment variable must be set"))
        };
        let api_url = parse_api_url(&required("OPENHOME_API_URL")?)?;
        let api_key = ApiKey::new(required("API_KEY")?)?;
        let bind_addr = std::env::var("MCP_BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8001".to_string())
            .parse()
            .map_err(|err| anyhow::anyhow!("MCP_BIND_ADDR is not a socket address: {err}"))?;
        let allowed_hosts = match std::env::var("MCP_ALLOWED_HOSTS") {
            Ok(hosts) => parse_allowed_hosts(&hosts)?,
            Err(_) => StreamableHttpServerConfig::default().allowed_hosts,
        };
        Ok(Self {
            api_url,
            api_key,
            bind_addr,
            allowed_hosts,
        })
    }
}

pub fn parse_api_url(text: &str) -> anyhow::Result<Url> {
    let url =
        Url::parse(text).map_err(|err| anyhow::anyhow!("OPENHOME_API_URL is invalid: {err}"))?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https") && !url.cannot_be_a_base(),
        "OPENHOME_API_URL must be an http(s) base URL"
    );
    Ok(url)
}

/// Parses a comma-separated `MCP_ALLOWED_HOSTS` list of hosts or `host:port` authorities. An empty
/// list is refused: rmcp would read it as "allow every host" and drop DNS-rebinding protection.
pub fn parse_allowed_hosts(text: &str) -> anyhow::Result<Vec<String>> {
    let hosts: Vec<String> = text
        .split(',')
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(str::to_owned)
        .collect();
    anyhow::ensure!(
        !hosts.is_empty(),
        "MCP_ALLOWED_HOSTS must list at least one host; unset it to allow loopback only"
    );
    Ok(hosts)
}

/// Authenticated requests against the OpenHome API.
#[derive(Clone)]
pub struct OpenHomeApi {
    http: reqwest::Client,
    base_url: Url,
    api_key: ApiKey,
}

impl OpenHomeApi {
    pub fn new(base_url: Url, api_key: ApiKey) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(API_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            http,
            base_url,
            api_key,
        })
    }

    /// GETs the API path made of `segments` and turns the reply into a tool result: the JSON body
    /// on success, the API's error body as a tool error otherwise. Every segment is
    /// percent-encoded; literal dot segments are rejected before URL construction.
    async fn get(&self, segments: &[&str]) -> CallToolResult {
        self.request(reqwest::Method::GET, segments, None, &[])
            .await
    }

    async fn post(&self, segments: &[&str], body: Value) -> CallToolResult {
        self.request(reqwest::Method::POST, segments, Some(body), &[])
            .await
    }

    async fn request(
        &self,
        method: reqwest::Method,
        segments: &[&str],
        body: Option<Value>,
        query: &[(String, String)],
    ) -> CallToolResult {
        // URL path construction discards literal dot segments. Reject them before they can
        // change the endpoint; calendar validation remains the API's responsibility.
        if segments
            .iter()
            .any(|segment| matches!(*segment, "." | ".."))
        {
            return CallToolResult::structured_error(json!({
                "error": "Tool arguments must not be '.' or '..' path segments",
                "status": 400,
            }));
        }
        let deleting = method == reqwest::Method::DELETE;
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .expect("parse_api_url accepts only base URLs")
            .pop_if_empty()
            .extend(segments);
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(
                query
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str())),
            );
        }
        let mut request = self
            .http
            .request(method, url)
            .header(
                header::AUTHORIZATION,
                format!("{BEARER_PREFIX}{}", self.api_key.0),
            )
            .header(header::ACCEPT, "application/json");
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await;
        let response = match response {
            Ok(response) => response,
            Err(err) => {
                tracing::warn!(error = %err, "OpenHome API request failed");
                return CallToolResult::structured_error(json!({
                    "error": "The OpenHome API could not be reached",
                    "status": null,
                }));
            }
        };
        let status = response.status();
        // DELETE is the only API operation whose success deliberately has no JSON body.
        if deleting && status == reqwest::StatusCode::NO_CONTENT {
            return CallToolResult::structured(json!({"deleted":true}));
        }
        let body: Option<Value> = match response.bytes().await {
            Ok(bytes) => serde_json::from_slice(&bytes).ok(),
            Err(_) => None,
        };
        match body {
            Some(body) if status.is_success() => CallToolResult::structured(body),
            // The API's own `{"error", "status"}` body carries its meaning; pass it through.
            Some(body) if body.get("error").is_some() => CallToolResult::structured_error(body),
            _ => CallToolResult::structured_error(json!({
                "error": format!(
                    "The OpenHome API answered {status} without a JSON {}",
                    if status.is_success() { "result" } else { "error" }
                ),
                "status": status.as_u16(),
            })),
        }
    }
}

/// Read-only training and Food tools, and household list and date tools.
#[derive(Clone)]
pub struct OpenHomeTools {
    api: OpenHomeApi,
    tool_router: ToolRouter<Self>,
}

impl OpenHomeTools {
    #[must_use]
    pub fn new(api: OpenHomeApi) -> Self {
        Self {
            api,
            tool_router: Self::household_router()
                + Self::watchlist_router()
                + Self::important_dates_router()
                + Self::wishlist_router()
                + Self::foods_router()
                + Self::meals_router()
                + Self::supplements_router()
                + Self::meal_logs_router()
                + Self::nutrition_summary_router()
                + Self::training_router(),
        }
    }
}

#[tool_router(router = household_router, vis = "pub(crate)")]
impl OpenHomeTools {
    #[tool(
        description = "Discover configured Household Users and their stable IDs. All Wishlists, Watchlists and Important Dates are household-visible; ownership and tags are not per-user authentication.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn household_users(&self) -> CallToolResult {
        self.api.get(&["api", "household-users"]).await
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "the async methods are generated by rmcp's `tool_handler` macro"
)]
#[tool_handler(router = self.tool_router)]
impl ServerHandler for OpenHomeTools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }
}

/// The largest MCP request body: a `log_meal` or `update_meal_log` call carrying a base64 meal
/// photo of up to 10 MB (about 13.4 MiB encoded) with its JSON-RPC envelope. rmcp's own default
/// is 4 MiB.
pub const MAX_REQUEST_BODY_BYTES: usize = 16 * 1024 * 1024;

/// The MCP endpoint at [`MCP_PATH`], answering only clients that present `client_key` as a
/// bearer token, with bodies up to [`MAX_REQUEST_BODY_BYTES`].
pub fn router(api: OpenHomeApi, client_key: ApiKey, config: StreamableHttpServerConfig) -> Router {
    let service: StreamableHttpService<OpenHomeTools, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(OpenHomeTools::new(api.clone())),
            Arc::default(),
            config.with_max_request_body_bytes(MAX_REQUEST_BODY_BYTES),
        );
    Router::new()
        .nest_service(MCP_PATH, service)
        .layer(middleware::from_fn_with_state(client_key, require_api_key))
}

async fn require_api_key(State(api_key): State<ApiKey>, request: Request, next: Next) -> Response {
    let presented = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix(BEARER_PREFIX));
    match presented {
        Some(key) if api_key.matches(key) => next.run(request).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            Json(json!({"error": "Missing or invalid API key", "status": 401})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_hosts_must_name_at_least_one_host() {
        assert_eq!(
            parse_allowed_hosts(" openhome , openhome.example.ts.net:8001,").unwrap(),
            ["openhome", "openhome.example.ts.net:8001"]
        );
        assert!(parse_allowed_hosts("").is_err());
        assert!(parse_allowed_hosts(" , ").is_err());
    }

    #[test]
    fn api_key_rejects_blank_or_padded_values() {
        assert!(ApiKey::new("key".to_string()).unwrap().matches("key"));
        assert!(!ApiKey::new("key".to_string()).unwrap().matches("key "));
        assert!(ApiKey::new(String::new()).is_err());
        assert!(ApiKey::new("key\n".to_string()).is_err());
    }

    #[test]
    fn api_url_must_be_an_http_base() {
        assert!(parse_api_url("http://openhome-api:8000").is_ok());
        assert!(parse_api_url("https://openhome.example/base/").is_ok());
        assert!(parse_api_url("openhome-api:8000").is_err());
        assert!(parse_api_url("mailto:someone@example.com").is_err());
    }
}
