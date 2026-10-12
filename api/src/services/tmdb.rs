//! Private TMDB reads. Provider metadata never supplies personal viewing state.

use std::time::Duration;

use reqwest::{Client, header};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

use super::watchlist::{MediaKind, WatchlistError};

/// A bounded, redirect-free client holding a private provider credential.
#[derive(Clone)]
pub struct TmdbService {
    client: Client,
    base_url: Url,
}

/// Recognizable metadata for a movie or whole series, keyed by kind plus TMDB ID.
#[derive(Debug, Serialize)]
pub struct MediaMetadata {
    pub kind: MediaKind,
    pub tmdb_id: i64,
    pub title: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub release_date: Option<String>,
}

#[derive(Deserialize)]
struct ProviderTitle {
    id: i64,
    title: Option<String>,
    name: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<ProviderTitle>,
}

impl TmdbService {
    /// Use a fixed production origin; an explicit origin also supports isolated provider tests.
    pub fn new(token: &str, base_url: Url) -> anyhow::Result<Self> {
        let mut authorization = header::HeaderValue::from_str(&format!("Bearer {}", token.trim()))?;
        authorization.set_sensitive(true);
        let mut headers = header::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, authorization);
        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { client, base_url })
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, WatchlistError> {
        let mut url = self
            .base_url
            .join(path)
            .map_err(|_| WatchlistError::ProviderUnavailable)?;
        url.query_pairs_mut().extend_pairs(query.iter().copied());
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| WatchlistError::ProviderUnavailable)?;
        if !response.status().is_success() {
            return Err(WatchlistError::ProviderUnavailable);
        }
        // Bound untrusted bodies even when a provider omits Content-Length.
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| WatchlistError::ProviderUnavailable)?
        {
            if bytes.len() + chunk.len() > 1024 * 1024 {
                return Err(WatchlistError::ProviderUnavailable);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| WatchlistError::ProviderUnavailable)
    }

    /// Fetch the first page for an explicitly selected media kind; never search people or episodes.
    pub async fn search(
        &self,
        kind: MediaKind,
        query: &str,
    ) -> Result<Vec<MediaMetadata>, WatchlistError> {
        let response: SearchResponse = self
            .get(
                &format!("search/{}", provider_kind(kind)),
                &[
                    ("query", query),
                    ("include_adult", "false"),
                    ("language", "en-US"),
                ],
            )
            .await?;
        response
            .results
            .into_iter()
            .take(20)
            .map(|item| item.metadata(kind))
            .collect()
    }

    /// Resolve selected identity again in Axum rather than accepting client-supplied metadata.
    pub async fn details(&self, kind: MediaKind, id: i64) -> Result<MediaMetadata, WatchlistError> {
        if id <= 0 || id > 9_007_199_254_740_991 {
            return Err(WatchlistError::Validation(
                "tmdb_id must be a positive safe integer".into(),
            ));
        }
        let response: ProviderTitle = self
            .get(
                &format!("{}/{id}", provider_kind(kind)),
                &[("language", "en-US")],
            )
            .await?;
        if response.id != id {
            return Err(WatchlistError::ProviderUnavailable);
        }
        response.metadata(kind)
    }
}

fn provider_kind(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Movie => "movie",
        MediaKind::Series => "tv",
    }
}

impl ProviderTitle {
    fn metadata(self, kind: MediaKind) -> Result<MediaMetadata, WatchlistError> {
        let title = match kind {
            MediaKind::Movie => self.title,
            MediaKind::Series => self.name,
        }
        .filter(|title| {
            !title.trim().is_empty()
                && title.chars().count() <= 500
                && !title.chars().any(char::is_control)
        })
        .ok_or(WatchlistError::ProviderUnavailable)?;
        if self.id <= 0 || self.id > 9_007_199_254_740_991 {
            return Err(WatchlistError::ProviderUnavailable);
        }
        let release_date = match kind {
            MediaKind::Movie => self.release_date,
            MediaKind::Series => self.first_air_date,
        }
        .filter(|date| {
            date.len() == 10 && chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok()
        });
        // Only a single provider image filename can reach the browser's fixed image.tmdb.org origin.
        let poster_path = self.poster_path.filter(|path| {
            path.starts_with('/')
                && path
                    .as_bytes()
                    .get(1)
                    .is_some_and(u8::is_ascii_alphanumeric)
                && path.len() <= 200
                && path[1..]
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'_' || c == b'-')
        });
        Ok(MediaMetadata {
            kind,
            tmdb_id: self.id,
            title,
            overview: self
                .overview
                .filter(|value| !value.is_empty() && value.chars().count() <= 10_000),
            poster_path,
            release_date,
        })
    }
}
