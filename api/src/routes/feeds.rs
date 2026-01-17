use crate::error::{AppError, Result};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
};
use serde::{Deserialize, Serialize};
use sqlx::{Decode, Sqlite, Type};

#[derive(Debug, Serialize)]
struct Feed {
    id: i64,
    url: String,
    enabled: bool,
}

#[derive(Debug)]
struct FeedDb {
    id: i64,
    url: String,
    enabled: DbBool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DbBool(bool);

impl Type<Sqlite> for DbBool {
    fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
        <i64 as Type<Sqlite>>::type_info()
    }
}

impl<'r> Decode<'r, Sqlite> for DbBool {
    fn decode(
        value: <Sqlite as sqlx::Database>::ValueRef<'r>,
    ) -> std::result::Result<Self, sqlx::error::BoxDynError> {
        let int = <i64 as Decode<Sqlite>>::decode(value)?;
        Ok(DbBool(int != 0))
    }
}

impl From<DbBool> for bool {
    fn from(val: DbBool) -> bool {
        val.0
    }
}

impl From<bool> for DbBool {
    fn from(val: bool) -> Self {
        DbBool(val)
    }
}

#[derive(Debug, Deserialize)]
struct CreateFeed {
    url: String,
}

#[derive(Debug, Deserialize)]
struct UpdateFeed {
    enabled: bool,
}

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/feeds", get(get_feeds))
        .route("/api/feeds", post(create_feed))
        .route("/api/feeds/{id}", put(update_feed))
        .route("/api/feeds/{id}", delete(delete_feed))
}

async fn get_feeds(State(state): State<crate::AppState>) -> Result<Json<Vec<Feed>>> {
    let feeds = sqlx::query_as!(
        FeedDb,
        r#"
        SELECT id, url, enabled as "enabled!: DbBool"
        FROM feeds
        WHERE enabled = 1
        "#
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch feeds: {}", e)))?;

    let feeds: Vec<Feed> = feeds
        .into_iter()
        .map(|f| Feed {
            id: f.id,
            url: f.url,
            enabled: f.enabled.into(),
        })
        .collect();

    Ok(Json(feeds))
}

async fn create_feed(
    State(state): State<crate::AppState>,
    Json(payload): Json<CreateFeed>,
) -> Result<(StatusCode, Json<Feed>)> {
    if !payload.url.starts_with("http://") && !payload.url.starts_with("https://") {
        return Err(AppError::Validation(
            "URL must start with http:// or https://".to_string(),
        ));
    }

    let feed = sqlx::query_as!(
        FeedDb,
        r#"
        INSERT INTO feeds (url, enabled)
        VALUES ($1, 1)
        RETURNING id, url, enabled as "enabled!: DbBool"
        "#,
        payload.url
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            AppError::Conflict("Feed with this URL already exists".to_string())
        }
        other => AppError::Internal(anyhow::anyhow!("Failed to create feed: {}", other)),
    })?;

    Ok((
        StatusCode::CREATED,
        Json(Feed {
            id: feed.id,
            url: feed.url,
            enabled: feed.enabled.into(),
        }),
    ))
}

async fn update_feed(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateFeed>,
) -> Result<Json<Feed>> {
    let enabled_db: i64 = if payload.enabled { 1 } else { 0 };
    let feed = sqlx::query_as!(
        FeedDb,
        r#"
        UPDATE feeds
        SET enabled = $1
        WHERE id = $2
        RETURNING id, url, enabled as "enabled!: DbBool"
        "#,
        enabled_db,
        id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to update feed: {}", e)))?
    .ok_or_else(|| AppError::NotFound(format!("Feed with id {} not found", id)))?;

    Ok(Json(Feed {
        id: feed.id,
        url: feed.url,
        enabled: feed.enabled.into(),
    }))
}

async fn delete_feed(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode> {
    let result = sqlx::query!(
        r#"
        DELETE FROM feeds
        WHERE id = $1
        "#,
        id
    )
    .execute(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to delete feed: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Feed with id {} not found", id)));
    }

    Ok(StatusCode::NO_CONTENT)
}
