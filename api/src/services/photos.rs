//! Selected-album Immich reads. No originals, shared links or EXIF cross this boundary.
use chrono::{DateTime, Datelike, NaiveDateTime};
use reqwest::{Client, StatusCode, header};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};
use url::Url;
use uuid::Uuid;

/// Maximum photos returned per gallery page.
pub const PAGE_SIZE: usize = 60;
/// Maximum albums in the `IMMICH_ALBUM_IDS` allowlist.
pub const MAX_ALBUMS: usize = 24;
const ASSET_LIMIT: usize = 20_000;
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

/// Safe album metadata without provider identities, locations or paths. The span is only the
/// first and last month (`YYYY-MM`) a photo was taken; no day, time or place leaves Axum.
#[derive(Clone, Serialize)]
pub struct AlbumSummary {
    pub id: Uuid,
    pub name: String,
    pub photo_count: usize,
    pub cover_id: Option<Uuid>,
    pub first_month: Option<String>,
    pub last_month: Option<String>,
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

/// One photo in the merged view, named by the selected album that grants access to it.
#[derive(Serialize)]
pub struct MergedPhoto {
    pub album: Uuid,
    pub asset: Uuid,
}

/// One bounded page across every selected album, newest first; capture times stay in Axum.
#[derive(Serialize)]
pub struct MergedPage {
    pub photo_count: usize,
    pub items: Vec<MergedPhoto>,
    pub page: usize,
    pub next_page: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderAlbum {
    id: Uuid,
    album_name: String,
    #[serde(default)]
    album_thumbnail_asset_id: Option<Uuid>,
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
    /// Wall-clock capture time; read only to order photos and bound the album's months.
    #[serde(default)]
    local_date_time: Option<String>,
}
struct CachedAlbum {
    fetched: Instant,
    summary: AlbumSummary,
    assets: Vec<Uuid>,
    taken: Vec<Option<NaiveDateTime>>,
}

fn taken_at(value: Option<&str>) -> Option<NaiveDateTime> {
    DateTime::parse_from_rfc3339(value?)
        .ok()
        .map(|time| time.naive_utc())
}

fn month(time: NaiveDateTime) -> String {
    format!("{:04}-{:02}", time.year(), time.month())
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
    pub fn new(mut base_url: Url, key: &str, mut selected: Vec<Uuid>) -> anyhow::Result<Self> {
        if !matches!(base_url.scheme(), "https" | "http")
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
            || selected.is_empty()
            || selected.len() > MAX_ALBUMS
            || key.trim().is_empty()
        {
            anyhow::bail!(
                "Invalid Immich configuration (server root, key and 1–{MAX_ALBUMS} album IDs required)"
            );
        }
        let mut seen = HashSet::new();
        selected.retain(|id| seen.insert(*id));
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
            || album.assets.len() > ASSET_LIMIT
        {
            return Err(PhotoError::ProviderUnavailable);
        }
        let (assets, taken): (Vec<_>, Vec<_>) = album
            .assets
            .into_iter()
            .filter(|asset| asset.kind == "IMAGE" && !asset.is_trashed && !asset.is_offline)
            .map(|asset| (asset.id, taken_at(asset.local_date_time.as_deref())))
            .unzip();
        // The owner's chosen cover when it is a served still, else the first photo.
        let cover_id = album
            .album_thumbnail_asset_id
            .filter(|cover| assets.contains(cover))
            .or_else(|| assets.first().copied());
        let summary = AlbumSummary {
            id,
            name: album.album_name,
            photo_count: assets.len(),
            cover_id,
            first_month: taken.iter().flatten().min().copied().map(month),
            last_month: taken.iter().flatten().max().copied().map(month),
            missing: false,
        };
        cache.insert(
            id,
            CachedAlbum {
                fetched: Instant::now(),
                summary,
                assets,
                taken,
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
                    first_month: None,
                    last_month: None,
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

    /// Every selected album's photos merged newest first, de-duplicated by asset; undated photos
    /// follow, in album order. Removed albums drop out; access still goes through each album.
    pub async fn merged(&self, page: usize) -> Result<MergedPage, PhotoError> {
        let start = page.checked_mul(PAGE_SIZE).ok_or(PhotoError::InvalidPage)?;
        let mut seen = HashSet::new();
        let mut photos = Vec::new();
        for &id in self.selected.iter() {
            let cache = match self.album(id).await {
                Ok(cache) => cache,
                Err(PhotoError::NotFound) => continue,
                Err(error) => return Err(error),
            };
            let album = &cache[&id];
            for (&asset, &taken) in album.assets.iter().zip(&album.taken) {
                if seen.insert(asset) {
                    photos.push((taken, id, asset));
                }
            }
        }
        // Stable, so equal times keep album order.
        photos.sort_by_key(|photo| std::cmp::Reverse(photo.0));
        if start >= photos.len() && page != 0 {
            return Err(PhotoError::InvalidPage);
        }
        Ok(MergedPage {
            photo_count: photos.len(),
            items: photos
                .iter()
                .skip(start)
                .take(PAGE_SIZE)
                .map(|&(_, album, asset)| MergedPhoto { album, asset })
                .collect(),
            page,
            next_page: (start + PAGE_SIZE < photos.len()).then_some(page + 1),
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
