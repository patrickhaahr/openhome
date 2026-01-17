use std::net::IpAddr;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{AppError, Result};

#[derive(Debug, Serialize)]
struct Feed {
    id: i64,
    url: String,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateFeed {
    url: String,
}

fn validate_url(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).map_err(|e| AppError::Validation(format!("Invalid URL: {}", e)))?;

    if url.scheme() != "https" {
        return Err(AppError::Validation(
            "URL must use HTTPS scheme".to_string(),
        ));
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::Validation(
            "URL must not contain userinfo (username or password)".to_string(),
        ));
    }

    let Some(host) = url.host_str() else {
        return Err(AppError::Validation("URL missing host".to_string()));
    };

    let host_lower = host.to_lowercase();
    if host_lower == "localhost"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
    {
        return Err(AppError::Validation(
            "URL host is not allowed (localhost or .local)".to_string(),
        ));
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        let is_private = match ip {
            IpAddr::V4(addr) => {
                addr.is_private()
                    || addr.is_loopback()
                    || addr.is_link_local()
                    || addr.is_multicast()
                    || addr.is_unspecified()
            }
            IpAddr::V6(addr) => {
                addr.is_loopback()
                    || addr.is_unicast_link_local()
                    || addr.is_unspecified()
                    || addr.is_unique_local()
                    || addr.is_multicast()
            }
        };
        if is_private {
            return Err(AppError::Validation(
                "URL host is a private or reserved IP address".to_string(),
            ));
        }
    }

    Ok(url)
}

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/feeds", get(get_feeds))
        .route("/api/feeds", post(create_feed))
        .route("/api/feeds/{id}", delete(delete_feed))
}

async fn get_feeds(State(state): State<crate::AppState>) -> Result<Json<Vec<Feed>>> {
    let feeds = sqlx::query_as!(
        Feed,
        r#"
        SELECT id, url, title
        FROM feeds
        "#
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch feeds: {}", e)))?;

    Ok(Json(feeds))
}

async fn create_feed(
    State(state): State<crate::AppState>,
    Json(payload): Json<CreateFeed>,
) -> Result<(StatusCode, Json<Feed>)> {
    let validated_url = match validate_url(&payload.url) {
        Ok(url) => url,
        Err(e) => return Err(e),
    };

    let feed_url = validated_url.as_str().to_string();
    let feed = sqlx::query_as!(
        Feed,
        r#"
        INSERT INTO feeds (url)
        VALUES ($1)
        RETURNING id, url, title
        "#,
        feed_url
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            AppError::Conflict("Feed with this URL already exists".to_string())
        }
        other => AppError::Internal(anyhow::anyhow!("Failed to create feed: {}", other)),
    })?;

    Ok((StatusCode::CREATED, Json(feed)))
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
