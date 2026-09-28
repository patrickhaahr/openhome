//! Hermes MCP adapter for OpenHome.
//!
//! A Streamable HTTP MCP server that exposes read-only training context as typed tools. Each tool
//! with representable path arguments is one authenticated GET against the OpenHome API whose
//! JSON becomes the tool's structured content unchanged. The adapter has no database access
//! and no training rules of its own: days,
//! units, source resolution, freshness and coverage all come from the API (ADR 0002).

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
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use subtle::ConstantTimeEq;
use url::Url;

/// Where the MCP endpoint is mounted.
pub const MCP_PATH: &str = "/mcp";

const BEARER_PREFIX: &str = "Bearer ";
const API_TIMEOUT: Duration = Duration::from_secs(15);

const INSTRUCTIONS: &str = "Read-only training context from OpenHome. Every tool reads the \
OpenHome API, which owns days (Europe/Copenhagen), units, NOOP source selection, freshness and \
coverage; results are returned as the API sent them. A null value means no value was recorded or \
synced; check each result's freshness and coverage before treating a gap as real.";

const RECOVERY_ON_DAY: &str = "Sleep and recovery for one Europe/Copenhagen calendar day, from \
the NOOP strap mirror. `sleep` is the Sleep Night that wakes on `day`: NOOP's daily totals plus the \
sessions ending that day. Every value has a unit (sleep in minutes, efficiency as a 0-1 fraction, \
resting heart rate in beats/min, HRV as RMSSD in ms) and names the NOOP namespace it was selected \
from. `derived_scores` are NOOP's own 0-100 recovery and strain scores: model outputs, not \
measurements or diagnoses. Null means no value. Before treating a null or a night without sessions \
as real, check `noop.freshness` (`unconfirmed`: complete recovery coverage for this day has not \
arrived yet) and `noop.coverage` (`covered`: an empty result is a recorded absence; `unknown`: rows may be \
missing).";

const WORKOUTS_ON_DAY: &str = "Workouts on one Europe/Copenhagen Training Day. `workouts` are the \
Workouts logged in OpenHome with that date: ordered Exercises with their exact Sets (reps, \
added_weight_kg (null for bodyweight), hold_duration_s for Timed Holds, RPE 1-10). \
`noop_workouts` are the separate NOOP Workouts whose start falls on that local day, with start and \
end timestamps, NOOP's recorded duration in minutes, stored average and max heart rate in \
beats/min, NOOP's derived 0-100 strain score, energy in kcal and origin (manual, detected, ...). \
The two lists are never paired: a NOOP Workout on the same day is context, not proof it is the \
same session. Null means no value. A missing NOOP Workout never means the user did not train; \
check `noop.freshness` (`unconfirmed`: complete workout coverage for this day has not arrived yet) \
and `noop.coverage.workouts` (`covered`: NOOP recorded no other workout that day; `unknown`: \
rows may be missing).";

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

/// Authenticated reads against the OpenHome API.
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
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .expect("parse_api_url accepts only base URLs")
            .pop_if_empty()
            .extend(segments);
        let response = self
            .http
            .get(url)
            .header(
                header::AUTHORIZATION,
                format!("{BEARER_PREFIX}{}", self.api_key.0),
            )
            .header(header::ACCEPT, "application/json")
            .send()
            .await;
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RecoveryOnDayRequest {
    /// The Europe/Copenhagen calendar day, written YYYY-MM-DD. The Sleep Night reported is the
    /// one that wakes on this day.
    pub day: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WorkoutsOnDayRequest {
    /// The Europe/Copenhagen Training Day, written YYYY-MM-DD.
    pub day: String,
}

/// The read-only training tools.
#[derive(Clone)]
pub struct TrainingTools {
    api: OpenHomeApi,
    tool_router: ToolRouter<Self>,
}

impl TrainingTools {
    #[must_use]
    pub fn new(api: OpenHomeApi) -> Self {
        Self {
            api,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl TrainingTools {
    #[tool(
        description = RECOVERY_ON_DAY,
        annotations(
            title = "Recovery on a day",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn recovery_on_day(
        &self,
        Parameters(RecoveryOnDayRequest { day }): Parameters<RecoveryOnDayRequest>,
    ) -> CallToolResult {
        self.api
            .get(&["api", "training", "days", &day, "recovery"])
            .await
    }

    #[tool(
        description = WORKOUTS_ON_DAY,
        annotations(
            title = "Workouts on a day",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn workouts_on_day(
        &self,
        Parameters(WorkoutsOnDayRequest { day }): Parameters<WorkoutsOnDayRequest>,
    ) -> CallToolResult {
        self.api
            .get(&["api", "training", "days", &day, "workouts"])
            .await
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "the async methods are generated by rmcp's `tool_handler` macro"
)]
#[tool_handler(router = self.tool_router)]
impl ServerHandler for TrainingTools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }
}

/// The MCP endpoint at [`MCP_PATH`], answering only clients that present `client_key` as a
/// bearer token.
pub fn router(api: OpenHomeApi, client_key: ApiKey, config: StreamableHttpServerConfig) -> Router {
    let service: StreamableHttpService<TrainingTools, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(TrainingTools::new(api.clone())),
            Arc::default(),
            config,
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
