//! Meal photos (`CONTEXT.md`, Meal Log): the user's photo of what they ate, kept with its Meal Log.
//!
//! A JPEG or PNG upload is decoded, turned upright from its EXIF orientation, limited to
//! [`LONG_SIDE`] px on its long side and re-encoded as AVIF with a [`THUMBNAIL_LONG_SIDE`] px
//! thumbnail. Re-encoding from pixels removes every piece of metadata (EXIF, GPS, XMP, ICC), and
//! the original upload is never written. Decoding, ICC-to-sRGB conversion and encoding belong to
//! the shared [`avif_encoder`]; this module picks the sizes, speed 8 and quality, and owns the
//! stored files.
//!
//! # Lifecycle and ownership
//!
//! A photo's bytes pass through three owners, and the Meal Log row is the only reference:
//!
//! 1. **Encoded** ([`MealPhotos::encode`]): in memory only. Nothing exists on disk, so dropping
//!    or failing it needs no cleanup. The encoder permit and the upload move into the blocking
//!    worker, so a caller that is dropped while the worker runs still holds the permit until
//!    that encode ends; its result is then discarded.
//! 2. **Stored** ([`MealPhotos::store`]): two files named by a fresh photo ID that no row
//!    references. The caller owns them until its row write finishes, then either the committed
//!    row owns them or the caller removes them ([`MealPhotos::remove`]).
//! 3. **Referenced**: the committed Meal Log row names the ID. In normal operation the files
//!    are written and renamed into place before the row that names them is committed, and a
//!    replaced, removed or deleted photo's files are removed only after its row change commits.
//!    Removal is best effort and only logged on failure.
//!
//! # Limits
//!
//! The files and the SQL transaction are separate resources with no shared commit. Files and
//! renames are not fsynced, so power-loss durability is not established, and no cross-resource
//! crash guarantee is made. Cleanup is compensating code in the caller, not a guard, so it does
//! not run if the process stops or the calling future is dropped part-way, and nothing collects
//! what is left. Dropping the future is not atomic with the SQL commit. Leftovers to expect:
//!
//! - dropped or stopped between `store` and the row commit: the new files remain unreferenced;
//! - dropped or stopped while `store` writes: any mix of `.partial` and complete files may
//!   remain, including a partial file beside a complete one (a failed write removes its own
//!   partial file and photo ID, but a dropped one cannot);
//! - dropped or stopped after the commit and before `remove`: the replaced, removed or deleted
//!   photo's files remain.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use base64::Engine;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteConnectOptions;
use uuid::Uuid;

pub use crate::services::avif_encoder::Size;
use crate::services::avif_encoder::{self, EncodeError, Settings, Source};

/// The largest photo accepted, before base64 encoding.
pub const MAX_PHOTO_BYTES: usize = 10 * 1024 * 1024;
/// The request body limit of the routes that carry a photo; every other route keeps Axum's
/// default. A base64 photo of [`MAX_PHOTO_BYTES`] is about 13.4 MiB.
pub const PHOTO_BODY_LIMIT: usize = 15 * 1024 * 1024;
/// The stored photo's longest side, in pixels. Smaller photos keep their size.
pub const LONG_SIDE: u32 = 1_600;
/// The thumbnail's longest side, in pixels.
pub const THUMBNAIL_LONG_SIDE: u32 = 400;
/// aom speed 8 of 0-10 (about 0.6 s per 1,600 px photo on one Raspberry Pi 5 core) and libavif
/// quality 60 of 0-100 (about 235 KB for a 1,600 px photo).
const ENCODING: Settings = Settings {
    speed: 8,
    quality: 60,
};
const DIR_NAME: &str = "meal-photos";

#[derive(Debug, thiserror::Error)]
pub enum PhotoError {
    #[error("photo must be a base64-encoded JPEG or PNG image")]
    Undecodable,
    #[error("photo must be at most 10 MB and at most 12000 px on each side")]
    TooLarge,
    #[error("the photo could not be encoded")]
    Encode,
    #[error("the photo could not be stored")]
    Io(#[from] std::io::Error),
}

/// A received JPEG or PNG within [`MAX_PHOTO_BYTES`], not yet decoded.
pub struct Upload {
    source: Source,
}

impl Upload {
    /// Accepts JPEG or PNG bytes, recognised by their signature rather than a declared type.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, PhotoError> {
        if bytes.len() > MAX_PHOTO_BYTES {
            return Err(PhotoError::TooLarge);
        }
        Ok(Self {
            source: Source::from_bytes(bytes)?,
        })
    }

    /// Accepts standard base64, padded or not, optionally as a `data:image/...;base64,` URL.
    pub fn from_base64(text: &str) -> Result<Self, PhotoError> {
        let text = text.trim();
        let text = match text.split_once(";base64,") {
            Some((prefix, data)) if prefix.starts_with("data:") => data,
            _ => text,
        };
        // Reject before decoding anything that cannot decode to at most MAX_PHOTO_BYTES.
        if text.len() > MAX_PHOTO_BYTES.div_ceil(3) * 4 {
            return Err(PhotoError::TooLarge);
        }
        let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(text.trim_end_matches('='))
            .map_err(|_| PhotoError::Undecodable)?;
        Self::from_bytes(bytes)
    }

    /// The SHA-256 of the uploaded bytes, so a retry can tell the same photo from another.
    #[must_use]
    pub fn sha256(&self) -> String {
        use std::fmt::Write;
        Sha256::digest(self.source.as_bytes()).iter().fold(
            String::with_capacity(64),
            |mut hex, byte| {
                let _ = write!(hex, "{byte:02x}");
                hex
            },
        )
    }
}

/// A photo re-encoded as AVIF with its thumbnail; nothing is stored yet.
pub struct Encoded {
    photo: Vec<u8>,
    thumbnail: Vec<u8>,
    pub size: Size,
    pub thumbnail_size: Size,
}

/// Which stored file of a photo to read.
#[derive(Clone, Copy, Debug)]
pub enum Variant {
    Photo,
    Thumbnail,
}

/// The stored AVIF photos and thumbnails in the data directory, named by photo ID.
#[derive(Clone)]
pub struct MealPhotos {
    dir: PathBuf,
}

/// Where photos are stored: a `meal-photos` directory beside `app.db`.
pub fn dir_beside(database_url: &str) -> anyhow::Result<PathBuf> {
    let options = SqliteConnectOptions::from_str(database_url)?;
    let file = options.get_filename();
    anyhow::ensure!(
        file != Path::new(":memory:") && !file.as_os_str().is_empty(),
        "DATABASE_URL must name a file so meal photos can be stored beside it"
    );
    Ok(file
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(DIR_NAME))
}

impl MealPhotos {
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Decodes, resizes and re-encodes `upload` with the shared [`avif_encoder`], waiting for any
    /// encode already running.
    pub async fn encode(&self, upload: Upload) -> Result<Encoded, PhotoError> {
        let [photo, thumbnail] =
            avif_encoder::encode(upload.source, ENCODING, [LONG_SIDE, THUMBNAIL_LONG_SIDE]).await?;
        Ok(Encoded {
            photo: photo.bytes,
            thumbnail: thumbnail.bytes,
            size: photo.size,
            thumbnail_size: thumbnail.size,
        })
    }

    /// Stores `encoded` under a new photo ID. Either both files are written or neither is, when
    /// a write fails. On success the caller owns the unreferenced files until it commits a row
    /// naming the ID or calls [`Self::remove`]. A dropped call may leave files behind.
    pub async fn store(&self, encoded: &Encoded) -> Result<String, PhotoError> {
        tokio::fs::create_dir_all(&self.dir).await?;
        let id = Uuid::new_v4().to_string();
        let written = async {
            write(&self.path(&id, Variant::Photo), &encoded.photo).await?;
            write(&self.path(&id, Variant::Thumbnail), &encoded.thumbnail).await
        }
        .await;
        if let Err(error) = written {
            self.remove(&id).await;
            return Err(error.into());
        }
        Ok(id)
    }

    /// Removes a photo's files, the new photo of a write that failed or the old photo of one that
    /// committed. A failure leaves an orphaned file behind and is only logged: no Meal Log row
    /// references it.
    pub async fn remove(&self, id: &str) {
        for variant in [Variant::Photo, Variant::Thumbnail] {
            match tokio::fs::remove_file(self.path(id, variant)).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    tracing::warn!(photo_id = id, error = %error, "Could not remove a meal photo file");
                }
            }
        }
    }

    /// A stored AVIF file, or `None` when `id` names no stored photo.
    pub async fn read(&self, id: &str, variant: Variant) -> Result<Option<Vec<u8>>, PhotoError> {
        // Only a UUID can name a file, so no path can leave the photo directory.
        let Ok(id) = Uuid::parse_str(id) else {
            return Ok(None);
        };
        match tokio::fs::read(self.path(&id.to_string(), variant)).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn path(&self, id: &str, variant: Variant) -> PathBuf {
        self.dir.join(match variant {
            Variant::Photo => format!("{id}.avif"),
            Variant::Thumbnail => format!("{id}.thumbnail.avif"),
        })
    }
}

/// Writes through a temporary name, so a crash never leaves a partial photo under its real name.
async fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let partial = path.with_extension("partial");
    let written = async {
        tokio::fs::write(&partial, bytes).await?;
        tokio::fs::rename(&partial, path).await
    }
    .await;
    if written.is_err() {
        match tokio::fs::remove_file(&partial).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(path = %partial.display(), error = %error, "Could not remove a partial meal photo");
            }
        }
    }
    written
}

impl From<EncodeError> for PhotoError {
    fn from(error: EncodeError) -> Self {
        match error {
            EncodeError::Undecodable => Self::Undecodable,
            EncodeError::TooLarge => Self::TooLarge,
            EncodeError::Encode => Self::Encode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_failed_photo_rename_removes_its_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("photo.avif");
        // This real filesystem failure happens after bytes reached the temporary file.
        std::fs::create_dir(&path).unwrap();
        assert!(write(&path, b"private photo bytes").await.is_err());
        assert!(!directory.path().join("photo.partial").exists());
    }
}
