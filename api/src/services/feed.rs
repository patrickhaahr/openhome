use std::net::IpAddr;
use std::time::Duration;

use reqwest::{Response, header};
use serde::Serialize;
use sqlx::SqlitePool;
use url::Url;

const MAX_FEED_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedFailure {
    Unavailable,
    TooLarge,
    NotAFeed,
}

impl FeedFailure {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::TooLarge => "too_large",
            Self::NotAFeed => "not_a_feed",
        }
    }
}

fn unavailable(feed_id: i64, what: &str) -> FeedFailure {
    tracing::warn!(feed_id, "RSS feed: {what}");
    FeedFailure::Unavailable
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransportFault {
    Timeout,
    Connect,
    Body,
    Decode,
    Redirect,
    Request,
    Other,
}

impl TransportFault {
    fn classify(error: &reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Timeout
        } else if error.is_connect() {
            Self::Connect
        } else if error.is_body() {
            Self::Body
        } else if error.is_decode() {
            Self::Decode
        } else if error.is_redirect() {
            Self::Redirect
        } else if error.is_request() {
            Self::Request
        } else {
            Self::Other
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Connect => "connect",
            Self::Body => "body",
            Self::Decode => "decode",
            Self::Redirect => "redirect",
            Self::Request => "request",
            Self::Other => "other",
        }
    }
}

fn unavailable_after(feed_id: i64, what: &str, error: &reqwest::Error) -> FeedFailure {
    let fault = TransportFault::classify(error).as_str();
    tracing::warn!(fault, feed_id, "RSS feed: {what}");
    FeedFailure::Unavailable
}

async fn read_body(feed_id: i64, mut response: Response) -> Result<Vec<u8>, FeedFailure> {
    if response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|length| length.to_str().ok()?.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_FEED_BYTES)
    {
        return Err(FeedFailure::TooLarge);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| unavailable_after(feed_id, "reading the body failed", &error))?
    {
        if body.len() + chunk.len() > MAX_FEED_BYTES {
            return Err(FeedFailure::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn record_failure(pool: &SqlitePool, feed_id: i64, failure: FeedFailure) {
    let failure = failure.as_str();
    if let Err(e) = sqlx::query!(
        r#"
        UPDATE feeds SET last_fetched_at = datetime('now'), last_error = $1 WHERE id = $2
        "#,
        failure,
        feed_id
    )
    .execute(pool)
    .await
    {
        tracing::warn!(error = ?e, feed_id, "Failed to update feed metadata");
    }
}

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

const fn is_global_ip(ip: IpAddr) -> bool {
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
    pub error: Option<FeedFailure>,
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
            tracing::warn!(error = ?e, feed_id, "RSS feed: invalid URL");
            result.error = Some(FeedFailure::Unavailable);
            return result;
        }
    };

    if parsed_url.scheme() != "https" {
        result.error = Some(unavailable(feed_id, "URL must use HTTPS"));
        return result;
    }

    if !parsed_url.username().is_empty() || parsed_url.password().is_some() {
        result.error = Some(unavailable(feed_id, "URL must not contain userinfo"));
        return result;
    }

    let Some(host) = parsed_url.host_str() else {
        result.error = Some(unavailable(feed_id, "URL missing host"));
        return result;
    };

    let port = parsed_url.port_or_known_default().unwrap_or(443);

    if let Err(e) = check_dns_is_global(host, port).await {
        tracing::warn!(error = ?e, feed_id, "RSS feed: refusing to fetch");
        result.error = Some(FeedFailure::Unavailable);
        return result;
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| anyhow::anyhow!("HTTP client error: {e}"))
        .ok();

    let Some(client) = client else {
        result.error = Some(unavailable(feed_id, "building the HTTP client failed"));
        return result;
    };

    fetch_feed(pool, &client, parsed_url, &mut result).await;
    result
}

async fn fetch_feed(
    pool: &SqlitePool,
    client: &reqwest::Client,
    url: Url,
    result: &mut FeedResult,
) {
    let feed_id = result.feed_id;
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

    let mut request = client.get(url);

    if let Some(ref meta) = feed_meta {
        if let Some(etag) = &meta.etag {
            request = request.header("If-None-Match", etag);
        }
        if let Some(last_modified) = &meta.last_modified {
            request = request.header("If-Modified-Since", last_modified);
        }
    }

    match request.send().await {
        Ok(response) => ingest_response(pool, response, result).await,
        Err(e) => {
            let failure = unavailable_after(feed_id, "request failed", &e);
            fail(pool, result, failure).await;
        }
    }
}

async fn fail(pool: &SqlitePool, result: &mut FeedResult, failure: FeedFailure) {
    result.error = Some(failure);
    record_failure(pool, result.feed_id, failure).await;
}

async fn ingest_response(pool: &SqlitePool, response: Response, result: &mut FeedResult) {
    if let Err(failure) = ingest(pool, response, result).await {
        fail(pool, result, failure).await;
    }
}

async fn ingest(
    pool: &SqlitePool,
    response: Response,
    result: &mut FeedResult,
) -> Result<(), FeedFailure> {
    let feed_id = result.feed_id;
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
        return Ok(());
    }

    if !status.is_success() {
        tracing::warn!(%status, feed_id, "RSS feed: refused");
        return Err(FeedFailure::Unavailable);
    }

    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(std::string::ToString::to_string);
    let last_modified = response
        .headers()
        .get("last-modified")
        .and_then(|v| v.to_str().ok())
        .map(std::string::ToString::to_string);

    let bytes = read_body(feed_id, response).await?;

    let body = String::from_utf8_lossy(&bytes);

    let feed = feed_rs::parser::parse(body.as_bytes()).map_err(|_| {
        tracing::warn!(
            failure = FeedFailure::NotAFeed.as_str(),
            feed_id,
            "RSS feed: parse failed"
        );
        FeedFailure::NotAFeed
    })?;

    let feed_title = feed.title.map(|t| t.content);

    let mut inserted = 0;
    let mut skipped = 0;

    for entry in feed.entries {
        let Some(link) = entry.links.first().map(|l| l.href.clone()) else {
            continue;
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
                tracing::warn!(error = ?e, feed_id, "Failed to insert feed item");
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

    Ok(())
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
        .map_err(|e| anyhow::anyhow!("Failed to load feeds: {e}"))?;

    let mut results = Vec::with_capacity(feeds.len());

    for feed in feeds {
        let result = refresh_feed(pool, feed.id, &feed.url).await;
        results.push(result);
    }

    Ok(results)
}

const REFRESH_INTERVAL: Duration = Duration::from_hours(24);
const ERROR_PAUSE: Duration = Duration::from_secs(60);

pub async fn run_scheduler(pool: SqlitePool) {
    tracing::info!("Starting RSS feed scheduler (initial fetch + 24h interval)");
    if let Err(e) = refresh_all_feeds(&pool).await {
        tracing::warn!(error = %e, "Initial RSS feed refresh failed");
    }
    loop {
        tokio::time::sleep(REFRESH_INTERVAL).await;
        tracing::info!("Running scheduled RSS feed refresh");
        if let Err(e) = refresh_all_feeds(&pool).await {
            tracing::warn!(error = %e, "Scheduled RSS feed refresh failed");
            tokio::time::sleep(ERROR_PAUSE).await;
        }
    }
}

#[cfg(test)]
mod tests;
