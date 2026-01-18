use sqlx::SqlitePool;

use crate::services::adguard::AdguardService;

pub mod auth;
pub mod error;
pub mod routes;
pub mod services;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub adguard_service: Option<AdguardService>,
}
