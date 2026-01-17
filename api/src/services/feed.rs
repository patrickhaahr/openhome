use std::net::IpAddr;
use std::time::Duration;

use sqlx::SqlitePool;
use url::Url;

const MAX_FEED_BYTES: usize = 2 * 1024 * 1024;

async fn check_dns_is_global(host: &str, port: u16) -> anyhow::Result<()> {
    let addrs = tokio::net::lookup_host((host, port)).await?;
    for addr in addrs {
        let ip = addr.ip();
        if !is_global_ip(ip) {
            anyhow::bail!("refusing to fetch non-global address");
        }
    }
    Ok(())
}

fn is_global_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(addr) => {
            !addr.is_private()
                && !addr.is_loopback()
                && !addr.is_link_local()
                && !addr.is_multicast()
                && !addr.is_unspecified()
        }
        IpAddr::V6(addr) => {
            !addr.is_loopback()
                && !addr.is_unicast_link_local()
                && !addr.is_unspecified()
                && !addr.is_unique_local()
                && !addr.is_multicast()
        }
    }
}

#[derive(Debug)]
pub struct FeedResult {
    pub feed_id: i64,
    pub url: String,
    pub items_inserted: usize,
    pub duplicates_skipped: usize,
    pub error: Option<String>,
}

pub async fn refresh_feed(pool: &SqlitePool, feed_id: i64, url: &str) -> FeedResult {
    let mut result = FeedResult {
        feed_id,
        url: url.to_string(),
        items_inserted: 0,
        duplicates_skipped: 0,
        error: None,
    };

    let parsed_url = match Url::parse(url) {
        Ok(parsed) => parsed,
        Err(e) => {
            result.error = Some(format!("Invalid URL: {}", e));
            return result;
        }
    };

    if parsed_url.scheme() != "https" {
        result.error = Some("URL must use HTTPS scheme".to_string());
        return result;
    }

    if !parsed_url.username().is_empty() || parsed_url.password().is_some() {
        result.error = Some("URL must not contain userinfo".to_string());
        return result;
    }

    let Some(host) = parsed_url.host_str() else {
        result.error = Some("URL missing host".to_string());
        return result;
    };

    let port = parsed_url.port_or_known_default().unwrap_or(443);

    if let Err(e) = check_dns_is_global(host, port).await {
        result.error = Some(e.to_string());
        return result;
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| anyhow::anyhow!("HTTP client error: {}", e))
        .ok();

    let Some(client) = client else {
        result.error = Some("Failed to build HTTP client".to_string());
        return result;
    };

    let feed_meta = match sqlx::query!(
        r#"
        SELECT etag, last_modified FROM feeds WHERE id = $1
        "#,
        feed_id
    )
    .fetch_optional(pool)
    .await
    {
        Ok(meta) => meta,
        Err(e) => {
            tracing::warn!(error = ?e, feed_id, "Failed to load feed metadata");
            None
        }
    };

    let mut request = client.get(parsed_url.clone());

    if let Some(ref meta) = feed_meta {
        if let Some(etag) = &meta.etag {
            request = request.header("If-None-Match", etag);
        }
        if let Some(last_modified) = &meta.last_modified {
            request = request.header("If-Modified-Since", last_modified);
        }
    }

    let response = request.send().await;

    let response = match response {
        Ok(r) => r,
        Err(e) => {
            result.error = Some(format!("Request failed: {}", e));
            return result;
        }
    };

    let status = response.status();

    if status == reqwest::StatusCode::NOT_MODIFIED {
        if let Err(e) = sqlx::query!(
            r#"
            UPDATE feeds SET last_fetched_at = datetime('now'), last_error = NULL WHERE id = $1
            "#,
            feed_id
        )
        .execute(pool)
        .await
        {
            tracing::warn!(error = ?e, feed_id, "Failed to update feed metadata");
        }
        return result;
    }

    if !status.is_success() {
        let error_msg = format!("HTTP error: {}", status);
        result.error = Some(error_msg.clone());
        let error_for_sql = result.error.as_deref();
        if let Err(e) = sqlx::query!(
            r#"
            UPDATE feeds SET last_fetched_at = datetime('now'), last_error = $1 WHERE id = $2
            "#,
            error_for_sql,
            feed_id
        )
        .execute(pool)
        .await
        {
            tracing::warn!(error = ?e, feed_id, "Failed to update feed metadata");
        }
        return result;
    }

    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let last_modified = response
        .headers()
        .get("last-modified")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => {
            result.error = Some(format!("Failed to read body: {}", e));
            return result;
        }
    };

    if bytes.len() > MAX_FEED_BYTES {
        result.error = Some(format!(
            "Feed too large: {} bytes (max {})",
            bytes.len(),
            MAX_FEED_BYTES
        ));
        return result;
    }

    let body = String::from_utf8_lossy(&bytes);

    let feed = feed_rs::parser::parse(body.as_bytes());

    let feed = match feed {
        Ok(f) => f,
        Err(e) => {
            let error_msg = format!("Failed to parse RSS: {}", e);
            result.error = Some(error_msg.clone());
            let error_for_sql = result.error.as_deref();
            if let Err(e) = sqlx::query!(
                r#"
                UPDATE feeds SET last_fetched_at = datetime('now'), last_error = $1 WHERE id = $2
                "#,
                error_for_sql,
                feed_id
            )
            .execute(pool)
            .await
            {
                tracing::warn!(error = ?e, feed_id, "Failed to update feed metadata");
            }
            return result;
        }
    };

    let feed_title = feed.title.map(|t| t.content);

    let mut inserted = 0;
    let mut skipped = 0;

    for entry in feed.entries {
        let link = match entry.links.first().map(|l| l.href.clone()) {
            Some(l) => l,
            None => {
                continue;
            }
        };

        let guid = if entry.id.is_empty() {
            link.clone()
        } else {
            entry.id
        };
        let title = entry.title.map(|t| t.content).unwrap_or_default();
        let description = if let Some(content) = &entry.content {
            content
                .body
                .clone()
                .or_else(|| entry.summary.as_ref().map(|s| s.content.clone()))
        } else {
            entry.summary.as_ref().map(|s| s.content.clone())
        };
        let pub_date = entry.published.map(|d| d.to_string());

        let insert_result = sqlx::query!(
            r#"
            INSERT INTO feed_items (feed_id, title, description, link, guid, pub_date)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            feed_id,
            title,
            description,
            link,
            guid,
            pub_date
        )
        .execute(pool)
        .await;

        match insert_result {
            Ok(_) => inserted += 1,
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => skipped += 1,
            Err(e) => {
                tracing::warn!(error = ?e, "Failed to insert feed item");
            }
        }
    }

    result.items_inserted = inserted;
    result.duplicates_skipped = skipped;

    if let Err(e) = sqlx::query!(
        r#"
        UPDATE feeds SET
            title = $1,
            etag = $2,
            last_modified = $3,
            last_fetched_at = datetime('now'),
            last_error = NULL
        WHERE id = $4
        "#,
        feed_title,
        etag,
        last_modified,
        feed_id
    )
    .execute(pool)
    .await
    {
        tracing::warn!(error = ?e, feed_id, "Failed to update feed metadata");
    }

    result
}

pub async fn refresh_all_feeds(pool: &SqlitePool) -> anyhow::Result<Vec<FeedResult>> {
    #[derive(Debug)]
    struct FeedRow {
        id: i64,
        url: String,
    }

    let feeds = sqlx::query_as!(FeedRow, r#"SELECT id as "id!", url FROM feeds"#)
        .fetch_all(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load feeds: {}", e))?;

    let mut results = Vec::with_capacity(feeds.len());

    for feed in feeds {
        let result = refresh_feed(pool, feed.id, &feed.url).await;
        results.push(result);
    }

    Ok(results)
}
