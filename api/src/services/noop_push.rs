pub mod ingest;
pub mod registry;
mod replace_window;

use std::fmt;

use sqlx::SqlitePool;
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

pub const SUPPORTED_VERSIONS: &[&str] = &["1.0"];

pub const CURRENT_VERSION: &str = "1.0";

pub const V1_STREAMS: [&str; 12] = [
    "hrSample",
    "rrInterval",
    "event",
    "battery",
    "spo2Sample",
    "skinTempSample",
    "respSample",
    "gravitySample",
    "dailyMetric",
    "sleepSession",
    "workout",
    "journal",
];

pub const NOOP_DB_FILE_NAME: &str = "noop.db";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PushConfigError {
    #[error("NOOP_PUSH_TOKEN must not be blank")]
    BlankToken,
    #[error("NOOP_PUSH_TOKEN must not have leading or trailing whitespace (check quoting in .env)")]
    TokenWhitespace,
    #[error(
        "cannot derive the NOOP push database location from DATABASE_URL; set NOOP_DB_URL explicitly"
    )]
    UnderivableDbUrl,
}

#[derive(Clone)]
pub struct PushToken(std::sync::Arc<str>);

impl PushToken {
    pub fn new(token: String) -> Result<Self, PushConfigError> {
        if token.trim().is_empty() {
            return Err(PushConfigError::BlankToken);
        }
        if token.trim() != token {
            return Err(PushConfigError::TokenWhitespace);
        }
        Ok(Self(token.into()))
    }

    #[must_use]
    pub fn matches(&self, presented: &str) -> bool {
        presented.as_bytes().ct_eq(self.0.as_bytes()).into()
    }
}

impl fmt::Debug for PushToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PushToken(<redacted>)")
    }
}

pub fn negotiate_version<'a>(offers: impl IntoIterator<Item = &'a str>) -> Option<&'static str> {
    offers
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(|offer| offer.trim_matches([' ', '\t']))
        .find_map(|offer| SUPPORTED_VERSIONS.iter().copied().find(|v| *v == offer))
}

pub fn resolve_db_url(
    database_url: &str,
    noop_db_url: Option<&str>,
) -> Result<String, PushConfigError> {
    crate::db::sibling_db_url(database_url, noop_db_url, NOOP_DB_FILE_NAME)
        .ok_or(PushConfigError::UnderivableDbUrl)
}

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let pool = crate::db::connect_sqlite(url).await?;
    initialize(&pool).await?;
    Ok(pool)
}

pub async fn initialize(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./noop_migrations").run(pool).await?;
    sqlx::query(
        "INSERT INTO receiver_state (id, receiver_state_id) VALUES (1, ?) \
         ON CONFLICT(id) DO NOTHING",
    )
    .bind(Uuid::new_v4().hyphenated().to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn receiver_state_id(pool: &SqlitePool) -> anyhow::Result<Uuid> {
    let stored: String =
        sqlx::query_scalar("SELECT receiver_state_id FROM receiver_state WHERE id = 1")
            .fetch_one(pool)
            .await?;
    Ok(Uuid::parse_str(stored.trim())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiates_first_supported_version_in_sender_order() {
        assert_eq!(negotiate_version(["2.0, 1.0"]), Some("1.0"));
        assert_eq!(negotiate_version(["1.0"]), Some("1.0"));
        assert_eq!(negotiate_version(["3.1", " 1.0 "]), Some("1.0"));
    }

    #[test]
    fn negotiation_fails_without_a_common_exact_version() {
        assert_eq!(negotiate_version(Vec::<&str>::new()), None);
        assert_eq!(negotiate_version([""]), None);
        assert_eq!(negotiate_version(["1"]), None);
        assert_eq!(negotiate_version(["1.1, 2.0"]), None);
        assert_eq!(negotiate_version(["1.0.0"]), None);
    }

    #[test]
    fn derives_sibling_noop_db_from_database_url() {
        let cases = [
            ("sqlite:./data/app.db", "sqlite:./data/noop.db"),
            (
                "sqlite:/srv/api/data/app.db",
                "sqlite:/srv/api/data/noop.db",
            ),
            ("sqlite:///app/data/app.db", "sqlite:///app/data/noop.db"),
            (
                "sqlite://data/app.db?mode=rwc",
                "sqlite://data/noop.db?mode=rwc",
            ),
            ("sqlite:app.db", "sqlite:noop.db"),
        ];
        for (database_url, expected) in cases {
            assert_eq!(resolve_db_url(database_url, None).as_deref(), Ok(expected));
        }
    }

    #[test]
    fn noop_db_url_override_wins() {
        assert_eq!(
            resolve_db_url("sqlite:./data/app.db", Some("sqlite:/elsewhere/push.db")).as_deref(),
            Ok("sqlite:/elsewhere/push.db")
        );
        assert_eq!(
            resolve_db_url("sqlite:./data/app.db", Some("  ")).as_deref(),
            Ok("sqlite:./data/noop.db")
        );
    }

    #[test]
    fn refuses_to_derive_from_unusable_database_urls() {
        for database_url in [
            "sqlite::memory:",
            "sqlite:./data/",
            "sqlite:./data/noop.db",
            "postgres://x/y",
        ] {
            assert_eq!(
                resolve_db_url(database_url, None),
                Err(PushConfigError::UnderivableDbUrl),
                "{database_url}"
            );
        }
    }

    #[test]
    fn padded_token_is_rejected() {
        for token in ["abc ", " abc", "abc\n", "\tabc"] {
            assert_eq!(
                PushToken::new(token.to_string()).err(),
                Some(PushConfigError::TokenWhitespace),
                "{token:?}"
            );
        }
        assert!(PushToken::new("a b".to_string()).is_ok());
    }

    #[test]
    fn blank_token_is_rejected_and_token_is_redacted() {
        assert!(PushToken::new("  ".to_string()).is_err());
        let token = PushToken::new("secret".to_string()).unwrap();
        assert!(token.matches("secret"));
        assert!(!token.matches("secret2"));
        assert!(!format!("{token:?}").contains("secret"));
    }
}
