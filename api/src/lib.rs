use sqlx::SqlitePool;

pub mod auth;
pub mod error;
pub mod routes;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}
