use crate::error::{AppError, Result};
use crate::services::feed;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct FeedItemResponse {
    id: i64,
    feed_id: i64,
    feed_title: Option<String>,
    title: String,
    description: Option<String>,
    link: String,
    pub_date: Option<String>,
    read_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TimelineQuery {
    limit: Option<i64>,
    unread: Option<bool>,
}

#[derive(Debug, Serialize)]
struct RefreshSummary {
    feeds_processed: usize,
    items_inserted: usize,
    duplicates_skipped: usize,
    errors: Vec<FeedRefreshError>,
}

#[derive(Debug, Serialize)]
struct FeedRefreshError {
    feed_id: i64,
    url: String,
    error: String,
}

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/timeline", get(get_timeline))
        .route("/api/items/{id}/read", post(mark_read))
        .route("/api/feeds/refresh", post(refresh_feeds))
}

async fn get_timeline(
    State(state): State<crate::AppState>,
    Query(query): Query<TimelineQuery>,
) -> Result<Json<Vec<FeedItemResponse>>> {
    let limit = query.limit.map(|limit| limit.clamp(1, 200)).unwrap_or(50);

    #[derive(Debug)]
    struct FeedItemRow {
        id: i64,
        feed_id: i64,
        feed_title: Option<String>,
        title: String,
        description: Option<String>,
        link: String,
        pub_date: Option<String>,
        read_at: Option<String>,
    }

    let items: Vec<FeedItemResponse> = if query.unread == Some(true) {
        sqlx::query_as!(
            FeedItemRow,
            r#"
            SELECT
                fi.id as "id!",
                fi.feed_id as "feed_id!",
                f.title as feed_title,
                fi.title,
                fi.description,
                fi.link,
                CAST(fi.pub_date AS TEXT) as pub_date,
                CAST(fi.read_at AS TEXT) as read_at
            FROM feed_items fi
            JOIN feeds f ON f.id = fi.feed_id
            WHERE fi.read_at IS NULL
            ORDER BY (fi.pub_date IS NULL) ASC, fi.pub_date DESC, fi.id DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch timeline: {}", e)))?
        .into_iter()
        .map(|row| FeedItemResponse {
            id: row.id,
            feed_id: row.feed_id,
            feed_title: row.feed_title,
            title: row.title,
            description: row.description,
            link: row.link,
            pub_date: row.pub_date,
            read_at: row.read_at,
        })
        .collect()
    } else {
        sqlx::query_as!(
            FeedItemRow,
            r#"
            SELECT
                fi.id as "id!",
                fi.feed_id as "feed_id!",
                f.title as feed_title,
                fi.title,
                fi.description,
                fi.link,
                CAST(fi.pub_date AS TEXT) as pub_date,
                CAST(fi.read_at AS TEXT) as read_at
            FROM feed_items fi
            JOIN feeds f ON f.id = fi.feed_id
            ORDER BY (fi.pub_date IS NULL) ASC, fi.pub_date DESC, fi.id DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch timeline: {}", e)))?
        .into_iter()
        .map(|row| FeedItemResponse {
            id: row.id,
            feed_id: row.feed_id,
            feed_title: row.feed_title,
            title: row.title,
            description: row.description,
            link: row.link,
            pub_date: row.pub_date,
            read_at: row.read_at,
        })
        .collect()
    };

    Ok(Json(items))
}

async fn mark_read(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode> {
    let result = sqlx::query!(
        r#"
        UPDATE feed_items SET read_at = datetime('now')
        WHERE id = $1 AND read_at IS NULL
        "#,
        id
    )
    .execute(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to mark read: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Item with id {} not found or already read",
            id
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn refresh_feeds(State(state): State<crate::AppState>) -> Result<Json<RefreshSummary>> {
    let results = feed::refresh_all_feeds(&state.db)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to refresh feeds: {}", e)))?;

    let mut summary = RefreshSummary {
        feeds_processed: results.len(),
        items_inserted: 0,
        duplicates_skipped: 0,
        errors: Vec::new(),
    };

    for result in results {
        summary.items_inserted += result.items_inserted;
        summary.duplicates_skipped += result.duplicates_skipped;

        if let Some(error) = result.error {
            summary.errors.push(FeedRefreshError {
                feed_id: result.feed_id,
                url: result.url,
                error,
            });
        }
    }

    Ok(Json(summary))
}
