//! Selected-album Immich reads. No originals, shared links or EXIF cross this boundary.
use reqwest::{Client, StatusCode, header};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};
use url::Url;
use uuid::Uuid;

/// Maximum photos returned per gallery page.
pub const PAGE_SIZE: usize = 60;
const METADATA_LIMIT: usize = 8 * 1024 * 1024;
const CACHE_TTL: Duration = Duration::from_secs(30);

/// Only resized formats supported by the installed Immich API.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhotoSize {
    Thumbnail,
    Preview,
}

/// Sanitized failures; provider text and URLs are never returned or logged.
#[derive(Debug, thiserror::Error)]
pub enum PhotoError {
    #[error("Photo setup needed")]
    MissingConfiguration,
    #[error("Album or photo no longer available")]
    NotFound,
    #[error("Photo library unavailable; try again")]
    ProviderUnavailable,
    #[error("Invalid photo page")]
    InvalidPage,
}

/// Safe album metadata without provider identities, locations or paths.
#[derive(Clone, Serialize)]
pub struct AlbumSummary {
    pub id: Uuid,
    pub name: String,
    pub photo_count: usize,
    pub cover_id: Option<Uuid>,
    pub missing: bool,
}

/// One bounded page of photo identities, never original filenames or EXIF.
#[derive(Serialize)]
pub struct AlbumPage {
    pub album: AlbumSummary,
    pub assets: Vec<Uuid>,
    pub page: usize,
    pub next_page: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderAlbum {
    id: Uuid,
    album_name: String,
    assets: Vec<ProviderAsset>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderAsset {
    id: Uuid,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    is_trashed: bool,
    #[serde(default)]
    is_offline: bool,
}
struct CachedAlbum {
    fetched: Instant,
    summary: AlbumSummary,
    assets: Vec<Uuid>,
}

/// One shared client with bounded album metadata and four concurrent image reads.
#[derive(Clone)]
pub struct PhotoService {
    client: Client,
    base_url: Url,
    selected: Arc<Vec<Uuid>>,
    albums: Arc<Mutex<HashMap<Uuid, CachedAlbum>>>,
    images: Arc<Semaphore>,
}

impl PhotoService {
    /// Missing configuration disables photos; malformed or partial setup fails startup safely.
    pub fn configure(
        base: Option<&str>,
        key: Option<&str>,
        albums: Option<&str>,
    ) -> anyhow::Result<Option<Self>> {
        if base.is_none() && key.is_none() && albums.is_none() {
            return Ok(None);
        }
        let (Some(base), Some(key), Some(albums)) = (base, key, albums) else {
            anyhow::bail!("Set IMMICH_URL, IMMICH_API_KEY and IMMICH_ALBUM_IDS together");
        };
        let base_url = Url::parse(base).map_err(|_| anyhow::anyhow!("Invalid IMMICH_URL"))?;
        let selected = albums
            .split(',')
            .map(|value| Uuid::parse_str(value.trim()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| anyhow::anyhow!("IMMICH_ALBUM_IDS must contain comma-separated UUIDs"))?;
        Self::new(base_url, key, selected).map(Some)
    }

    /// An installer-supplied server root and explicit album allowlist; no runtime library discovery.
    pub fn new(mut base_url: Url, key: &str, selected: Vec<Uuid>) -> anyhow::Result<Self> {
        if !matches!(base_url.scheme(), "https" | "http")
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
            || selected.is_empty()
            || selected.len() > 8
            || key.trim().is_empty()
        {
            anyhow::bail!(
                "Invalid Immich configuration (server root, key and 1–8 album IDs required)"
            );
        }
        base_url.set_path(&format!("{}/api/", base_url.path().trim_end_matches('/')));
        let mut credential = header::HeaderValue::from_str(key.trim())
            .map_err(|_| anyhow::anyhow!("Invalid IMMICH_API_KEY"))?;
        credential.set_sensitive(true);
        let mut headers = header::HeaderMap::new();
        headers.insert("x-api-key", credential);
        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            base_url,
            selected: Arc::new(selected),
            albums: Arc::new(Mutex::new(HashMap::new())),
            images: Arc::new(Semaphore::new(4)),
        })
    }

    async fn fetch(&self, path: &str) -> Result<reqwest::Response, PhotoError> {
        let url = self
            .base_url
            .join(path)
            .map_err(|_| PhotoError::ProviderUnavailable)?;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| PhotoError::ProviderUnavailable)?;
        match response.status() {
            StatusCode::NOT_FOUND => Err(PhotoError::NotFound),
            status if status.is_success() => Ok(response),
            _ => Err(PhotoError::ProviderUnavailable),
        }
    }

    async fn album(
        &self,
        id: Uuid,
    ) -> Result<tokio::sync::MutexGuard<'_, HashMap<Uuid, CachedAlbum>>, PhotoError> {
        if !self.selected.contains(&id) {
            return Err(PhotoError::NotFound);
        }
        // Coalesce parallel metadata reads, including thumbnail membership checks.
        let mut cache = self.albums.lock().await;
        if cache
            .get(&id)
            .is_some_and(|album| album.fetched.elapsed() < CACHE_TTL)
        {
            return Ok(cache);
        }
        cache.remove(&id);
        let response = self.fetch(&format!("albums/{id}")).await?;
        let body = bounded_body(response, METADATA_LIMIT).await?;
        let album: ProviderAlbum =
            serde_json::from_slice(&body).map_err(|_| PhotoError::ProviderUnavailable)?;
        if album.id != id
            || album.album_name.trim().is_empty()
            || album.album_name.chars().count() > 200
            || album.album_name.chars().any(char::is_control)
            || album.assets.len() > 20_000
        {
            return Err(PhotoError::ProviderUnavailable);
        }
        let assets: Vec<_> = album
            .assets
            .into_iter()
            .filter(|asset| asset.kind == "IMAGE" && !asset.is_trashed && !asset.is_offline)
            .map(|asset| asset.id)
            .collect();
        let summary = AlbumSummary {
            id,
            name: album.album_name,
            photo_count: assets.len(),
            cover_id: assets.first().copied(),
            missing: false,
        };
        cache.insert(
            id,
            CachedAlbum {
                fetched: Instant::now(),
                summary,
                assets,
            },
        );
        Ok(cache)
    }

    /// List only configured albums; deleted albums remain understandable and cannot broaden access.
    pub async fn list(&self) -> Result<Vec<AlbumSummary>, PhotoError> {
        let mut result = Vec::with_capacity(self.selected.len());
        for &id in self.selected.iter() {
            match self.album(id).await {
                Ok(cache) => result.push(cache[&id].summary.clone()),
                Err(PhotoError::NotFound) => result.push(AlbumSummary {
                    id,
                    name: "Unavailable album".into(),
                    photo_count: 0,
                    cover_id: None,
                    missing: true,
                }),
                Err(error) => return Err(error),
            }
        }
        Ok(result)
    }

    /// Bound browser work to 60 thumbnails; out-of-range pages never silently show another page.
    pub async fn page(&self, id: Uuid, page: usize) -> Result<AlbumPage, PhotoError> {
        let start = page.checked_mul(PAGE_SIZE).ok_or(PhotoError::InvalidPage)?;
        let cache = self.album(id).await?;
        let album = &cache[&id];
        if start >= album.assets.len() && page != 0 {
            return Err(PhotoError::InvalidPage);
        }
        Ok(AlbumPage {
            album: album.summary.clone(),
            assets: album
                .assets
                .iter()
                .skip(start)
                .take(PAGE_SIZE)
                .copied()
                .collect(),
            page,
            next_page: (start + PAGE_SIZE < album.assets.len()).then_some(page + 1),
        })
    }

    /// Recheck selected-album membership before reading a resized asset; never fall back to originals.
    pub async fn image(
        &self,
        album_id: Uuid,
        asset_id: Uuid,
        size: PhotoSize,
    ) -> Result<(String, Vec<u8>), PhotoError> {
        let _permit = tokio::time::timeout(Duration::from_secs(5), self.images.acquire())
            .await
            .map_err(|_| PhotoError::ProviderUnavailable)?
            .map_err(|_| PhotoError::ProviderUnavailable)?;
        {
            let cache = self.album(album_id).await?;
            if !cache[&album_id].assets.contains(&asset_id) {
                return Err(PhotoError::NotFound);
            }
        }
        let (size, limit) = match size {
            PhotoSize::Thumbnail => ("thumbnail", 2 * 1024 * 1024),
            PhotoSize::Preview => ("preview", 12 * 1024 * 1024),
        };
        let response = self
            .fetch(&format!("assets/{asset_id}/thumbnail?size={size}"))
            .await?;
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .filter(|value| matches!(*value, "image/jpeg" | "image/webp" | "image/png"))
            .ok_or(PhotoError::ProviderUnavailable)?
            .to_owned();
        let body = bounded_body(response, limit).await?;
        Ok((content_type, body))
    }
}

async fn bounded_body(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, PhotoError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(PhotoError::ProviderUnavailable);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| PhotoError::ProviderUnavailable)?
    {
        if bytes.len() + chunk.len() > limit {
            return Err(PhotoError::ProviderUnavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
