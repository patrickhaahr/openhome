//! Downloads, verifies, caches and imports the pinned workbook (ADR 0013).

use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::{Client, redirect};
use sha2::{Digest, Sha256};
use sqlx::{QueryBuilder, SqlitePool};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use url::Url;

use super::workbook::{self, Workbook, WorkbookError};
use crate::services::nutrition::parameters::{self, Unit};

/// DTU Data serves files through a short redirect to its object store.
const MAX_REDIRECTS: usize = 5;
/// The whole download, including the body.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(5);
/// The 6.1 workbook is about 12.6 MB.
const MAX_WORKBOOK_BYTES: u64 = 64 * 1024 * 1024;
const USER_AGENT: &str = concat!(
    "OpenHome/",
    env!("CARGO_PKG_VERSION"),
    " (food database import)"
);
const CACHE_PREFIX: &str = "dtu-fcdb-";
/// Rows per multi-row insert, within SQLite's bound parameter limit.
const INSERT_BATCH: usize = 300;

/// The dataset version the API imports, with where to fetch it and its SHA-256.
#[derive(Clone, Debug)]
pub struct DatasetPin {
    pub version: String,
    pub url: Url,
    /// Lower-case hex.
    pub sha256: String,
}

impl DatasetPin {
    /// The Danish Food Composition Database 6.1 workbook (doi:10.11583/DTU.32312844, CC BY 4.0),
    /// `FCDB_6.1_Dataset.xlsx` on DTU Data.
    #[must_use]
    pub fn pinned() -> Self {
        Self {
            version: "6.1".into(),
            url: Url::parse("https://ndownloader.figshare.com/files/65016537")
                .expect("the pinned URL is valid"),
            sha256: "aba52ade9dd75f085954ca5561f001efb58e4b37d412b135622fd34d77f45c09".into(),
        }
    }

    fn cache_file_name(&self) -> String {
        let version: String = self
            .version
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        format!("{CACHE_PREFIX}{version}.xlsx")
    }
}

/// Why an import failed. Logged only: responses say the database is unavailable, never why, so
/// no upstream text reaches a client.
#[derive(Debug, Error)]
pub enum ImportError {
    #[error("the download failed: {0}")]
    Download(&'static str),
    #[error("the download answered HTTP {0}")]
    Status(u16),
    #[error("the download exceeded {MAX_WORKBOOK_BYTES} bytes")]
    TooLarge,
    #[error("the download's SHA-256 {actual} does not match the pinned {expected}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("the workbook cache failed: {0}")]
    Cache(#[from] std::io::Error),
    #[error("the workbook is malformed: {0}")]
    Workbook(#[from] WorkbookError),
    #[error("parameter {id} is in {found:?}, but the vocabulary expects {expected:?}")]
    UnitChanged {
        id: i64,
        found: String,
        expected: Unit,
    },
    #[error("parameter {id} has the unknown unit {unit:?}")]
    UnknownUnit { id: i64, unit: String },
    #[error("the parse task failed")]
    Task,
    #[error("the database write failed: {0}")]
    Database(#[from] sqlx::Error),
}

/// Fetches the pinned workbook into `cache_dir` unless a verified copy is already there, then
/// replaces `food.db`'s contents with it in one transaction.
pub async fn import(
    pool: &SqlitePool,
    pin: &DatasetPin,
    cache_dir: &Path,
) -> Result<(), ImportError> {
    let path = cached_workbook(pin, cache_dir).await?;
    let workbook = tokio::task::spawn_blocking(move || workbook::read(&path))
        .await
        .map_err(|_| ImportError::Task)??;
    let parameters = resolve_parameters(&workbook)?;
    write(pool, pin, &workbook, &parameters).await?;
    tracing::info!(
        version = %pin.version,
        foods = workbook.foods.len(),
        parameters = parameters.len(),
        values = workbook.values.len(),
        "Imported the Danish Food Composition Database"
    );
    remove_stale_cache(pin, cache_dir).await;
    Ok(())
}

async fn cached_workbook(pin: &DatasetPin, cache_dir: &Path) -> Result<PathBuf, ImportError> {
    tokio::fs::create_dir_all(cache_dir).await?;
    let path = cache_dir.join(pin.cache_file_name());
    match tokio::fs::read(&path).await {
        Ok(bytes) if hex_sha256(&bytes) == pin.sha256 => {
            tracing::info!(version = %pin.version, "Using the cached food database workbook");
            return Ok(path);
        }
        Ok(_) => {
            tracing::warn!("The cached food database workbook does not match the pin");
            tokio::fs::remove_file(&path).await?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let partial = path.with_extension("xlsx.part");
    let downloaded = download(pin, &partial).await;
    if let Err(error) = downloaded {
        // Best effort: a leftover partial file is overwritten by the next attempt.
        let _ = tokio::fs::remove_file(&partial).await;
        return Err(error);
    }
    tokio::fs::rename(&partial, &path).await?;
    Ok(path)
}

/// Streams the workbook to `partial`, bounded by a timeout and a byte cap, and verifies it.
async fn download(pin: &DatasetPin, partial: &Path) -> Result<(), ImportError> {
    tracing::info!(version = %pin.version, "Downloading the food database workbook");
    let client = Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .redirect(redirect::Policy::limited(MAX_REDIRECTS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|_| ImportError::Download("the HTTP client could not be built"))?;
    let mut response = client
        .get(pin.url.clone())
        .send()
        .await
        .map_err(|_| ImportError::Download("the request failed"))?;
    if !response.status().is_success() {
        return Err(ImportError::Status(response.status().as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_WORKBOOK_BYTES)
    {
        return Err(ImportError::TooLarge);
    }
    let mut file = tokio::fs::File::create(partial).await?;
    let mut hasher = Sha256::new();
    let mut received: u64 = 0;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ImportError::Download("the body could not be read"))?
    {
        received += chunk.len() as u64;
        if received > MAX_WORKBOOK_BYTES {
            return Err(ImportError::TooLarge);
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.sync_all().await?;
    let actual = hex(&hasher.finalize());
    if actual != pin.sha256 {
        return Err(ImportError::ChecksumMismatch {
            expected: pin.sha256.clone(),
            actual,
        });
    }
    Ok(())
}

/// A workbook parameter named by the vocabulary.
struct ResolvedParameter<'a> {
    key: String,
    unit: Unit,
    source: &'a workbook::WorkbookParameter,
}

fn resolve_parameters(workbook: &Workbook) -> Result<Vec<ResolvedParameter<'_>>, ImportError> {
    workbook
        .parameters
        .iter()
        .map(|source| {
            let unit = Unit::from_dtu(&source.unit);
            match parameters::by_id(source.id) {
                Some(known) if unit == Some(known.unit) => Ok(ResolvedParameter {
                    key: known.key.into(),
                    unit: known.unit,
                    source,
                }),
                Some(known) => Err(ImportError::UnitChanged {
                    id: source.id,
                    found: source.unit.clone(),
                    expected: known.unit,
                }),
                // A parameter added after this vocabulary still imports under a DTU-derived key.
                None => {
                    let unit = unit.ok_or_else(|| ImportError::UnknownUnit {
                        id: source.id,
                        unit: source.unit.clone(),
                    })?;
                    tracing::warn!(
                        parameter_id = source.id,
                        "A food database parameter is not in the vocabulary"
                    );
                    Ok(ResolvedParameter {
                        key: format!("dtu_{}", source.id),
                        unit,
                        source,
                    })
                }
            }
        })
        .collect()
}

async fn write(
    pool: &SqlitePool,
    pin: &DatasetPin,
    workbook: &Workbook,
    parameters: &[ResolvedParameter<'_>],
) -> Result<(), ImportError> {
    let mut tx = pool.begin().await?;
    for table in ["food_value", "food", "parameter", "dataset"] {
        sqlx::query(&format!("DELETE FROM {table}"))
            .execute(&mut *tx)
            .await?;
    }
    for batch in parameters.chunks(INSERT_BATCH / 3) {
        QueryBuilder::new(
            "INSERT INTO parameter (parameter_id, key, name_da, name_en, group_da, group_en, \
             unit, per_100g, sort_key) ",
        )
        .push_values(batch, |mut row, parameter| {
            let source = parameter.source;
            row.push_bind(source.id)
                .push_bind(&parameter.key)
                .push_bind(&source.name_da)
                .push_bind(&source.name_en)
                .push_bind(&source.group_da)
                .push_bind(&source.group_en)
                .push_bind(parameter.unit.symbol())
                .push_bind(parameter.unit.per_100g())
                .push_bind(source.sort_key);
        })
        .build()
        .execute(&mut *tx)
        .await?;
    }
    for batch in workbook.foods.chunks(INSERT_BATCH / 2) {
        QueryBuilder::new(
            "INSERT INTO food (food_id, name_da, name_en, food_group_id, food_group_da, \
             food_group_en) ",
        )
        .push_values(batch, |mut row, food| {
            row.push_bind(food.id)
                .push_bind(&food.name_da)
                .push_bind(&food.name_en)
                .push_bind(food.group_id)
                .push_bind(&food.group_da)
                .push_bind(&food.group_en);
        })
        .build()
        .execute(&mut *tx)
        .await?;
    }
    for batch in workbook.values.chunks(INSERT_BATCH) {
        QueryBuilder::new("INSERT INTO food_value (food_id, parameter_id, value) ")
            .push_values(batch, |mut row, value| {
                row.push_bind(value.food_id)
                    .push_bind(value.parameter_id)
                    .push_bind(value.value);
            })
            .build()
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("INSERT INTO dataset (id, version, sha256, imported_at) VALUES (1, ?, ?, ?)")
        .bind(&pin.version)
        .bind(&pin.sha256)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Removes workbooks of earlier pins, so upgrades do not accumulate cached copies.
async fn remove_stale_cache(pin: &DatasetPin, cache_dir: &Path) {
    let current = pin.cache_file_name();
    let Ok(mut entries) = tokio::fs::read_dir(cache_dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(CACHE_PREFIX)
            && name != current.as_str()
            && let Err(error) = tokio::fs::remove_file(entry.path()).await
        {
            tracing::warn!(%error, file = %name, "Could not remove a stale food database workbook");
        }
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}
