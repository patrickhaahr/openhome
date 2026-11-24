use openhome_api::services::noop_push;
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

pub async fn memory_noop_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    noop_push::initialize(&pool).await.unwrap();
    pool
}
