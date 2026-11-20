use std::path::{Path, PathBuf};
use std::str::FromStr;

use base64::Engine;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteConnectOptions;
use uuid::Uuid;

pub use crate::services::avif_encoder::Size;
use crate::services::avif_encoder::{self, EncodeError, Settings, Source};

pub const MAX_PHOTO_BYTES: usize = 10 * 1024 * 1024;
pub const PHOTO_BODY_LIMIT: usize = 15 * 1024 * 1024;
pub const LONG_SIDE: u32 = 1_600;
pub const THUMBNAIL_LONG_SIDE: u32 = 400;
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

pub struct Upload {
    source: Source,
}

impl Upload {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, PhotoError> {
        if bytes.len() > MAX_PHOTO_BYTES {
            return Err(PhotoError::TooLarge);
        }
        Ok(Self {
            source: Source::from_bytes(bytes)?,
        })
    }

    pub fn from_base64(text: &str) -> Result<Self, PhotoError> {
        let text = text.trim();
        let text = match text.split_once(";base64,") {
            Some((prefix, data)) if prefix.starts_with("data:") => data,
            _ => text,
        };
        if text.len() > MAX_PHOTO_BYTES.div_ceil(3) * 4 {
            return Err(PhotoError::TooLarge);
        }
        let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(text.trim_end_matches('='))
            .map_err(|_| PhotoError::Undecodable)?;
        Self::from_bytes(bytes)
    }

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

pub struct Encoded {
    photo: Vec<u8>,
    thumbnail: Vec<u8>,
    pub size: Size,
    pub thumbnail_size: Size,
}

#[derive(Clone, Copy, Debug)]
pub enum Variant {
    Photo,
    Thumbnail,
}

#[derive(Clone)]
pub struct MealPhotos {
    dir: PathBuf,
}

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

    pub async fn read(&self, id: &str, variant: Variant) -> Result<Option<Vec<u8>>, PhotoError> {
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
        std::fs::create_dir(&path).unwrap();
        assert!(write(&path, b"private photo bytes").await.is_err());
        assert!(!directory.path().join("photo.partial").exists());
    }
}
