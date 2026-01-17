use crate::error::{AppError, Result};
use crate::services::feed;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, FromRow)]
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

#[derive(Debug, Serialize, FromRow)]
struct TimelineCompactItem {
    id: i64,
    title: String,
    description: Option<String>,
    link: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum TimelineResponse {
    Full(Vec<FeedItemResponse>),
    Compact(Vec<TimelineCompactItem>),
}

#[derive(Debug, Deserialize)]
struct TimelineQuery {
    limit: Option<i64>,
    unread: Option<bool>,
    before_id: Option<i64>,
    view: Option<String>,
}

#[derive(Debug, PartialEq)]
enum TimelineView {
    Full,
    Compact,
}

impl TimelineView {
    fn from_query(view: &Option<String>) -> Self {
        match view.as_deref() {
            Some("compact") => TimelineView::Compact,
            _ => TimelineView::Full,
        }
    }
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

fn build_timeline_query(view: TimelineView, with_unread_filter: bool, with_cursor: bool) -> String {
    let select_clause = match view {
        TimelineView::Full => {
            r#"
            SELECT
                fi.id,
                fi.feed_id,
                f.title as feed_title,
                fi.title,
                fi.description,
                fi.link,
                CAST(fi.pub_date AS TEXT) as pub_date,
                CAST(fi.read_at AS TEXT) as read_at
        "#
        }
        TimelineView::Compact => {
            r#"
            SELECT
                fi.id,
                fi.title,
                fi.description,
                fi.link
        "#
        }
    };

    let from_clause = match view {
        TimelineView::Full => {
            r#"
        FROM feed_items fi
        JOIN feeds f ON f.id = fi.feed_id
        "#
        }
        TimelineView::Compact => {
            r#"
        FROM feed_items fi
        "#
        }
    };

    let base_query = format!(
        r#"
        {select_clause}
        {from_clause}
        "#
    );

    if with_cursor {
        format!(
            r#"
            WITH cursor AS (
                SELECT pub_date FROM feed_items WHERE id = ?1
            )
            {base_query}, cursor
            WHERE
                {where_clause}
            ORDER BY (fi.pub_date IS NULL) ASC, fi.pub_date DESC, fi.id DESC
            LIMIT ?2
            "#,
            where_clause = if with_unread_filter {
                r#"
                fi.read_at IS NULL
                AND (
                    (cursor.pub_date IS NULL AND fi.pub_date IS NULL AND fi.id < ?1)
                    OR (cursor.pub_date IS NOT NULL AND (
                        fi.pub_date < cursor.pub_date
                        OR (fi.pub_date = cursor.pub_date AND fi.id < ?1)
                        OR fi.pub_date IS NULL
                    ))
                )
                "#
            } else {
                r#"
                (
                    (cursor.pub_date IS NULL AND fi.pub_date IS NULL AND fi.id < ?1)
                    OR (cursor.pub_date IS NOT NULL AND (
                        fi.pub_date < cursor.pub_date
                        OR (fi.pub_date = cursor.pub_date AND fi.id < ?1)
                        OR fi.pub_date IS NULL
                    ))
                )
                "#
            }
        )
    } else {
        format!(
            r#"
            {base_query}
            WHERE
                {where_clause}
            ORDER BY (fi.pub_date IS NULL) ASC, fi.pub_date DESC, fi.id DESC
            LIMIT ?
            "#,
            where_clause = if with_unread_filter {
                "fi.read_at IS NULL"
            } else {
                "1 = 1"
            }
        )
    }
}

async fn get_timeline(
    State(state): State<crate::AppState>,
    Query(query): Query<TimelineQuery>,
) -> Result<Json<TimelineResponse>> {
    let limit = query.limit.map(|limit| limit.clamp(1, 200)).unwrap_or(50);
    let view = TimelineView::from_query(&query.view);

    let items = if let Some(before_id) = query.before_id {
        let before_id_exists = sqlx::query_scalar!(
            "SELECT 1 as found FROM feed_items WHERE id = ? LIMIT 1",
            before_id
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to validate before_id: {}", e)))?
        .is_some();

        if !before_id_exists {
            return Err(AppError::Unprocessable(format!(
                "before_id {} does not exist",
                before_id
            )));
        }

        if view == TimelineView::Compact {
            let query_str =
                build_timeline_query(TimelineView::Compact, query.unread == Some(true), true);
            let compact_items = sqlx::query_as::<_, TimelineCompactItem>(&query_str)
                .bind(before_id)
                .bind(limit)
                .fetch_all(&state.db)
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Failed to fetch timeline: {}", e))
                })?;
            TimelineResponse::Compact(compact_items)
        } else {
            #[derive(Debug, FromRow)]
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

            let query_str =
                build_timeline_query(TimelineView::Full, query.unread == Some(true), true);
            let full_items: Vec<FeedItemResponse> = sqlx::query_as::<_, FeedItemRow>(&query_str)
                .bind(before_id)
                .bind(limit)
                .fetch_all(&state.db)
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Failed to fetch timeline: {}", e))
                })?
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
                .collect();
            TimelineResponse::Full(full_items)
        }
    } else if view == TimelineView::Compact {
        let query_str =
            build_timeline_query(TimelineView::Compact, query.unread == Some(true), false);
        let compact_items = sqlx::query_as::<_, TimelineCompactItem>(&query_str)
            .bind(limit)
            .fetch_all(&state.db)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch timeline: {}", e)))?;
        TimelineResponse::Compact(compact_items)
    } else {
        #[derive(Debug, FromRow)]
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

        let query_str = build_timeline_query(TimelineView::Full, query.unread == Some(true), false);
        let full_items: Vec<FeedItemResponse> = sqlx::query_as::<_, FeedItemRow>(&query_str)
            .bind(limit)
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
            .collect();
        TimelineResponse::Full(full_items)
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
