use axum::Router;
use openhome_api::AppState;
use sqlx::SqlitePool;

use crate::memory_noop_db::memory_noop_db;
use crate::test_app_with_pools::test_app_with_pools;

pub async fn test_app_with_db() -> (Router, AppState) {
    let db = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    test_app_with_pools(db, memory_noop_db().await)
}
