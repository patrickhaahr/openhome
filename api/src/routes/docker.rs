use tokio::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use bollard::errors::Error;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::Deserialize;

use crate::error::{AppError, Result};
use crate::models::docker::{
    ContainerDetailResponse, ContainerListResponse, RestartRequest, RestartResponse,
};
use crate::{AppState, CONTAINER_CACHE_TTL_SECONDS};

const LOGS_DEFAULT_TAIL: usize = 100;
const LOGS_MAX_TAIL: usize = 1000;

#[derive(Deserialize)]
pub struct LogsQuery {
    tail: Option<usize>,
    since: Option<String>,
    #[serde(default)]
    timestamps: bool,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/docker", get(list_containers))
        .route("/api/docker/{name}", get(get_container))
        .route("/api/docker/{name}/restart", post(restart_container))
        .route("/api/docker/{name}/logs", get(get_logs))
}

async fn list_containers(State(state): State<AppState>) -> Result<Json<ContainerListResponse>> {
    let cache = state.docker_cache.lock().await;
    let max_age = ChronoDuration::seconds(CONTAINER_CACHE_TTL_SECONDS);
    if !cache.is_stale(max_age) {
        return Ok(Json(ContainerListResponse {
            containers: cache.containers.clone(),
            timestamp: cache
                .last_updated
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| Utc::now().to_rfc3339()),
        }));
    }
    drop(cache);

    let service = state
        .docker_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Docker service not available".to_string()))?;
    let containers = tokio::time::timeout(Duration::from_secs(10), service.list_containers(false))
        .await
        .map_err(|_| anyhow::anyhow!("Docker request timed out"))?
        .map_err(|e| anyhow::anyhow!("Failed to list containers: {e}"))?;

    let mut cache = state.docker_cache.lock().await;
    cache.containers = containers.clone();
    cache.last_updated = Some(Utc::now());
    Ok(Json(ContainerListResponse {
        containers,
        timestamp: Utc::now().to_rfc3339(),
    }))
}

async fn get_container(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ContainerDetailResponse>> {
    let service = state
        .docker_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Docker service not available".to_string()))?;
    let detail = tokio::time::timeout(Duration::from_secs(10), service.inspect_container(&name))
        .await
        .map_err(|_| anyhow::anyhow!("Docker request timed out"))?
        .map_err(|err| map_docker_error(err, &name))?;
    Ok(Json(detail))
}

async fn restart_container(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<RestartRequest>,
) -> Result<Json<RestartResponse>> {
    let service = state
        .docker_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Docker service not available".to_string()))?;
    let _ = tokio::time::timeout(Duration::from_secs(5), service.inspect_container(&name))
        .await
        .map_err(|_| anyhow::anyhow!("Docker request timed out"))?
        .map_err(|err| map_docker_error(err, &name))?;
    tokio::time::timeout(
        Duration::from_secs(30),
        service.restart_container(&name, req.timeout_seconds),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Restart timed out"))?
    .map_err(|err| map_docker_error(err, &name))?;
    {
        let mut cache = state.docker_cache.lock().await;
        cache.last_updated = None;
    }
    Ok(Json(RestartResponse {
        success: true,
        message: format!("Container {} restart initiated", name),
    }))
}

async fn get_logs(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<LogsQuery>,
) -> Result<String> {
    let since = query
        .since
        .as_deref()
        .map(|value| {
            DateTime::parse_from_rfc3339(value)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| {
                    AppError::Validation("Invalid RFC3339 timestamp for 'since'".to_string())
                })
        })
        .transpose()?;
    let tail = clamp_log_tail(query.tail);
    let service = state
        .docker_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("Docker service not available".to_string()))?;
    let logs = tokio::time::timeout(
        Duration::from_secs(30),
        service.get_container_logs(&name, tail, since, query.timestamps),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Logs request timed out"))?
    .map_err(|err| map_docker_error(err, &name))?;
    Ok(logs)
}

fn map_docker_error(error: Error, container_name: &str) -> AppError {
    match error {
        Error::DockerResponseServerError {
            status_code: 404, ..
        } => AppError::ContainerNotFound(container_name.to_string()),
        other => AppError::DockerError(other.to_string()),
    }
}

fn clamp_log_tail(tail: Option<usize>) -> Option<usize> {
    let value = tail.unwrap_or(LOGS_DEFAULT_TAIL);
    Some(value.clamp(1, LOGS_MAX_TAIL))
}
