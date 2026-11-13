//! Cards (`CONTEXT.md`, Card): every printed variant of a Card Code as the OPTCG catalogue lists
//! it, stored the first time the code is searched for. Details are a snapshot and never edited.
//! Each variant's official image is fetched once and kept as AVIF in the same row, so reads never
//! touch the catalogue. A catalogue failure writes nothing.

use std::sync::LazyLock;

use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::Mutex;

use super::avif_encoder::{self, EncodeError, Settings, Source};
use super::optcg::{CatalogueCard, Optcg, Unavailable};

/// One lock for every first lookup: the catalogue is a volunteer's server and a code is fetched
/// once, so a concurrent search waits here and then finds the stored cards. At household scale
/// serialising the rare miss costs nothing, while a lock per code would need a map to clean up.
static FIRST_LOOKUP: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

const IMAGE: Settings = Settings {
    speed: 8,
    quality: 70,
};

#[derive(Debug, thiserror::Error)]
pub enum CardsError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("The card catalogue is unavailable; try again later")]
    DataUnavailable,
    #[error("Card image encoding failed")]
    Encoder,
    #[error("Card database operation failed")]
    Database(#[from] sqlx::Error),
}

impl From<Unavailable> for CardsError {
    fn from(_: Unavailable) -> Self {
        Self::DataUnavailable
    }
}

type Result<T> = std::result::Result<T, CardsError>;

/// The code printed on a card, such as `OP01-001`: a set prefix (letters, then at most two
/// digits), a hyphen and a three-digit card number. Stored and compared in upper case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardCode(String);

impl CardCode {
    pub fn parse(text: &str) -> Result<Self> {
        let code = text.trim().to_ascii_uppercase();
        let well_formed = code.split_once('-').is_some_and(|(prefix, number)| {
            let letters = prefix.bytes().take_while(u8::is_ascii_uppercase).count();
            let digits = &prefix[letters..];
            (1..=4).contains(&letters)
                && digits.len() <= 2
                && digits.bytes().all(|byte| byte.is_ascii_digit())
                && number.len() == 3
                && number.bytes().all(|byte| byte.is_ascii_digit())
        });
        if well_formed {
            Ok(Self(code))
        } else {
            Err(CardsError::Validation(
                "card_code must look like OP01-001: a set prefix, a hyphen and three digits".into(),
            ))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A variant identifier: the Card Code, plus `_` and a lower-case suffix for a parallel
/// (`OP01-001_p1`). It is the catalogue's image identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardId(String);

impl CardId {
    pub fn parse(text: &str) -> Result<Self> {
        let (code, suffix) = match text.split_once('_') {
            Some((code, suffix)) => (code, Some(suffix)),
            None => (text, None),
        };
        let suffix_ok = suffix.is_none_or(|suffix| {
            (1..=8).contains(&suffix.len())
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        });
        if CardCode::parse(code).is_ok_and(|parsed| parsed.as_str() == code) && suffix_ok {
            Ok(Self(text.to_owned()))
        } else {
            Err(CardsError::Validation(
                "card id must look like OP01-001 or OP01-001_p1".into(),
            ))
        }
    }

    #[must_use]
    pub fn code(&self) -> CardCode {
        let code = self.0.split('_').next().unwrap_or(&self.0);
        CardCode(code.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The catalogue's `TCGplayer` market price in USD and the day the catalogue observed it. Other
/// currencies join this object when exchange rates exist.
#[derive(Debug, Serialize)]
pub struct CardValue {
    pub usd: f64,
    pub observed_on: String,
}

#[derive(Debug, Serialize)]
pub struct Card {
    pub id: String,
    pub card_code: String,
    pub name: String,
    pub set_id: Option<String>,
    pub set_name: Option<String>,
    pub rarity: Option<String>,
    pub color: Option<String>,
    pub card_type: Option<String>,
    pub life: Option<String>,
    pub cost: Option<String>,
    pub power: Option<String>,
    pub counter_amount: Option<String>,
    pub attribute: Option<String>,
    pub sub_types: Option<String>,
    pub text: Option<String>,
    pub inventory_price_usd: Option<f64>,
    pub card_value: Option<CardValue>,
    /// Authenticated read of the stored AVIF.
    pub image_path: String,
}

#[derive(Debug, Serialize)]
pub struct Search {
    pub card_code: String,
    pub cards: Vec<Card>,
}

/// Every variant of `code`, in catalogue order; empty when the catalogue does not list it. Asks
/// the catalogue only when nothing is stored for the code.
pub async fn search(db: &SqlitePool, catalogue: &Optcg, code: &CardCode) -> Result<Search> {
    let mut cards = stored(db, code).await?;
    if cards.is_empty() {
        let _first_lookup = FIRST_LOOKUP.lock().await;
        cards = stored(db, code).await?;
        if cards.is_empty() {
            let listed = catalogue.card(code).await?;
            if !listed.is_empty() {
                store(db, catalogue, &listed).await?;
                cards = stored(db, code).await?;
            }
        }
    }
    Ok(Search {
        card_code: code.as_str().to_owned(),
        cards,
    })
}

pub async fn image(db: &SqlitePool, id: &CardId) -> Result<Vec<u8>> {
    let id = id.as_str();
    sqlx::query_scalar!(
        r#"SELECT image_avif AS "image_avif!" FROM cards WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| CardsError::NotFound("Card not found".into()))
}

async fn stored(db: &SqlitePool, code: &CardCode) -> Result<Vec<Card>> {
    let code = code.as_str();
    let rows = sqlx::query!(
        "SELECT id, card_code, name, set_id, set_name, rarity, color, card_type, life, cost, power,
                counter_amount, attribute, sub_types, text, inventory_price_usd, price_usd,
                price_observed_on
         FROM cards WHERE card_code = ? ORDER BY position",
        code
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| Card {
            image_path: format!("/api/binder/cards/{}/image", row.id),
            card_value: row
                .price_usd
                .zip(row.price_observed_on)
                .map(|(usd, observed_on)| CardValue { usd, observed_on }),
            id: row.id,
            card_code: row.card_code,
            name: row.name,
            set_id: row.set_id,
            set_name: row.set_name,
            rarity: row.rarity,
            color: row.color,
            card_type: row.card_type,
            life: row.life,
            cost: row.cost,
            power: row.power,
            counter_amount: row.counter_amount,
            attribute: row.attribute,
            sub_types: row.sub_types,
            text: row.text,
            inventory_price_usd: row.inventory_price_usd,
        })
        .collect())
}

/// Fetches and encodes every image first, then writes all variants in one transaction, so a
/// failure leaves nothing behind. A concurrent search that stored the same variants wins.
async fn store(db: &SqlitePool, catalogue: &Optcg, listed: &[CatalogueCard]) -> Result<()> {
    let mut images = Vec::with_capacity(listed.len());
    for card in listed {
        let source = Source::from_bytes(catalogue.image(&card.id).await?)
            .map_err(|_| CardsError::DataUnavailable)?;
        // The encoder's decode bound is the target, so the stored image keeps its source size.
        let [avif] = avif_encoder::encode(source, IMAGE, [avif_encoder::MAX_SIDE])
            .await
            .map_err(|error| match error {
                EncodeError::Encode => CardsError::Encoder,
                EncodeError::Undecodable | EncodeError::TooLarge => CardsError::DataUnavailable,
            })?;
        images.push(avif.bytes);
    }
    let mut tx = db.begin().await?;
    for (position, (card, image)) in (0_i64..).zip(listed.iter().zip(&images)) {
        let id = card.id.as_str();
        let code = card.id.code();
        let code = code.as_str();
        let price_usd = card.price.as_ref().map(|price| price.usd);
        let price_observed_on = card.price.as_ref().map(|price| price.observed_on.as_str());
        sqlx::query!(
            "INSERT INTO cards (id, card_code, position, name, set_id, set_name, rarity, color,
                card_type, life, cost, power, counter_amount, attribute, sub_types, text,
                inventory_price_usd, price_usd, price_observed_on, image_avif)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO NOTHING",
            id,
            code,
            position,
            card.name,
            card.set_id,
            card.set_name,
            card.rarity,
            card.color,
            card.card_type,
            card.life,
            card.cost,
            card.power,
            card.counter_amount,
            card.attribute,
            card.sub_types,
            card.text,
            card.inventory_price_usd,
            price_usd,
            price_observed_on,
            image
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_codes_are_normalised_and_malformed_ones_rejected() {
        for (text, code) in [
            ("OP01-001", "OP01-001"),
            (" op01-001 ", "OP01-001"),
            ("ST10-012", "ST10-012"),
            ("EB01-061", "EB01-061"),
            ("PRB01-001", "PRB01-001"),
            ("P-001", "P-001"),
        ] {
            assert_eq!(CardCode::parse(text).unwrap().as_str(), code);
        }
        for text in [
            "",
            "OP01",
            "OP01-1",
            "OP01-0001",
            "OP001-001",
            "OPTCG1-001",
            "-001",
            "01-001",
            "OP01_001",
            "OP01-001_p1",
            "OP01-001/..",
            "OP01-00A",
        ] {
            assert!(CardCode::parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn card_ids_are_a_code_with_an_optional_parallel_suffix() {
        for text in ["OP01-001", "OP01-001_p1", "OP01-001_p12", "P-001_r1"] {
            let id = CardId::parse(text).unwrap();
            assert_eq!(id.as_str(), text);
            assert_eq!(id.code().as_str(), text.split('_').next().unwrap());
        }
        for text in [
            "",
            "op01-001",
            "OP01-001_",
            "OP01-001_P1",
            "OP01-001_p1_p2",
            "OP01-001_p1/..",
            "OP01-001_toolongsuffix",
        ] {
            assert!(CardId::parse(text).is_err(), "{text}");
        }
    }
}
