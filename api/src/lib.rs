use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::Mutex;

use chrono::{DateTime, Utc};

use crate::services::adguard::AdguardService;
use crate::services::docker::DockerService;
use crate::services::ir::IrService;
use crate::services::switchbot::SwitchbotService;

pub mod auth;
pub mod db;
pub mod error;
pub mod models;
pub mod routes;
pub mod services;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    /// The NOOP push mirror (`noop.db`); training reads only ever query it.
    pub noop_db: SqlitePool,
    pub adguard_service: Option<AdguardService>,
    pub docker_service: Option<DockerService>,
    pub ir_service: Option<IrService>,
    pub switchbot_service: Option<SwitchbotService>,
    pub docker_cache: Arc<Mutex<DockerCache>>,
}

#[derive(Clone, Default)]
pub struct DockerCache {
    pub containers: Vec<models::docker::ContainerStatus>,
    pub last_updated: Option<DateTime<Utc>>,
}

impl DockerCache {
    #[must_use]
    pub fn is_stale(&self, max_age: chrono::Duration) -> bool {
        match self.last_updated {
            Some(last) => Utc::now() - last > max_age,
            None => true,
        }
    }
}

pub const CONTAINER_CACHE_TTL_SECONDS: i64 = 5;
