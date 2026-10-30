use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

/// Opens a SQLite pool with the settings shared by `app.db`, `noop.db` and `food.db`.
pub async fn connect_sqlite(url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    Ok(pool)
}

/// The URL of a database kept beside the main one: `override_url` when set, otherwise
/// `database_url` with its file name swapped to `file_name` (query parameters are kept). `None`
/// when `database_url` is not a SQLite file URL or already names `file_name`.
#[must_use]
pub fn sibling_db_url(
    database_url: &str,
    override_url: Option<&str>,
    file_name: &str,
) -> Option<String> {
    if let Some(url) = override_url.map(str::trim).filter(|url| !url.is_empty()) {
        return Some(url.to_string());
    }

    let (location, query) = match database_url.split_once('?') {
        Some((location, query)) => (location, Some(query)),
        None => (database_url, None),
    };
    let path = location.strip_prefix("sqlite:")?;
    let file_start = path.rfind('/').map_or(0, |index| index + 1);
    let current = &path[file_start..];
    if current.is_empty() || current == ":memory:" || current == file_name {
        return None;
    }

    let mut url = format!("sqlite:{}{file_name}", &path[..file_start]);
    if let Some(query) = query {
        url.push('?');
        url.push_str(query);
    }
    Some(url)
}
