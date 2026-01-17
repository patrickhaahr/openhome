use sqlx::SqlitePool;

pub mod auth;
pub mod error;
pub mod routes;
pub mod services;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}
