use axum::Router;
use openhome_api::AppState;
use sqlx::SqlitePool;

use crate::test_app_with_pools::test_app_with_pools;

pub fn test_app_with_pool(db: SqlitePool) -> (Router, AppState) {
    let noop_db = SqlitePool::connect_lazy("sqlite::memory:").unwrap();
    test_app_with_pools(db, noop_db)
}
