use axum::Router;

use crate::test_app_with_db::test_app_with_db;

pub async fn test_app() -> Router {
    test_app_with_db().await.0
}
