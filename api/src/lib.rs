use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::Mutex;

use chrono::{DateTime, Utc};

use crate::services::adguard::AdguardService;
use crate::services::docker::DockerService;

pub mod auth;
pub mod error;
pub mod models;
pub mod routes;
pub mod services;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub adguard_service: Option<AdguardService>,
    pub docker_service: Option<DockerService>,
    pub docker_cache: Arc<Mutex<DockerCache>>,
}

#[derive(Clone, Default)]
pub struct DockerCache {
    pub containers: Vec<models::docker::ContainerStatus>,
    pub last_updated: Option<DateTime<Utc>>,
}

impl DockerCache {
    pub fn is_stale(&self, max_age: chrono::Duration) -> bool {
        match self.last_updated {
            Some(last) => Utc::now() - last > max_age,
            None => true,
        }
    }
}

pub const CONTAINER_CACHE_TTL_SECONDS: i64 = 5;
