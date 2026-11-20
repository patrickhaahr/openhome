use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::Mutex;

use super::avif_encoder::{self, EncodeError, Settings, Source};
use super::card_value::{Valuation, Valuer};
use super::exchange_rates;
use super::optcg::{CatalogueCard, Optcg, Unavailable};
use super::price_history;
use super::training::CalendarDay;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardId(String);

impl CardId {
    pub fn parse(text: &str) -> Result<Self> {
        let well_formed = text.len() <= 64
            && text
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            && text
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric());
        if well_formed {
            Ok(Self(text.to_owned()))
        } else {
            Err(CardsError::Validation(
                "card id must be 1 to 64 letters, digits, hyphens or underscores, starting with a letter or digit"
                    .into(),
            ))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize)]
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
    #[serde(flatten)]
    pub valuation: Valuation,
    pub image_path: String,
}

#[derive(Debug, Serialize)]
pub struct Search {
    pub card_code: String,
    pub cards: Vec<Card>,
}

pub async fn search(db: &SqlitePool, catalogue: &Optcg, code: &CardCode) -> Result<Search> {
    let today = CalendarDay::of(chrono::Utc::now()).to_string();
    Ok(Search {
        card_code: code.as_str().to_owned(),
        cards: cards_of(db, catalogue, code, &today, None).await?,
    })
}

async fn cards_of(
    db: &SqlitePool,
    catalogue: &Optcg,
    code: &CardCode,
    today: &str,
    wanted: Option<&CardId>,
) -> Result<Vec<Card>> {
    if let Some(cards) = known(db, code, today).await? {
        return Ok(cards);
    }
    let _first_lookup = FIRST_LOOKUP.lock().await;
    if let Some(cards) = known(db, code, today).await? {
        return Ok(cards);
    }
    let listed = catalogue.card(code).await?;
    if wanted.is_some_and(|id| !listed.iter().any(|card| card.id == *id)) {
        return Ok(Vec::new());
    }
    if listed.is_empty() {
        remember_unlisted(db, code, today).await?;
        return Ok(Vec::new());
    }
    store(db, catalogue, &listed).await?;
    stored(db, code).await
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

async fn known(db: &SqlitePool, code: &CardCode, today: &str) -> Result<Option<Vec<Card>>> {
    let cards = stored(db, code).await?;
    let code = code.as_str();
    let unlisted = sqlx::query_scalar!(
        r#"SELECT 1 AS "found!: i64" FROM unlisted_cards WHERE card_code = ? AND checked_on = ?"#,
        code,
        today
    )
    .fetch_optional(db)
    .await?
    .is_some();
    Ok((!cards.is_empty() || unlisted).then_some(cards))
}

async fn remember_unlisted(db: &SqlitePool, code: &CardCode, today: &str) -> Result<()> {
    let code = code.as_str();
    sqlx::query!(
        "INSERT INTO unlisted_cards (card_code, checked_on) VALUES (?, ?)
         ON CONFLICT(card_code) DO UPDATE SET checked_on = excluded.checked_on",
        code,
        today
    )
    .execute(db)
    .await?;
    Ok(())
}

struct CardRow {
    id: String,
    card_code: String,
    name: String,
    set_id: Option<String>,
    set_name: Option<String>,
    rarity: Option<String>,
    color: Option<String>,
    card_type: Option<String>,
    life: Option<String>,
    cost: Option<String>,
    power: Option<String>,
    counter_amount: Option<String>,
    attribute: Option<String>,
    sub_types: Option<String>,
    text: Option<String>,
    price_usd: Option<f64>,
    price_observed_on: Option<String>,
}

impl CardRow {
    fn into_card(self, valuer: &Valuer) -> Card {
        Card {
            image_path: format!("/api/binder/cards/{}/image", self.id),
            valuation: valuer.card(self.price_usd.zip(self.price_observed_on.as_deref())),
            id: self.id,
            card_code: self.card_code,
            name: self.name,
            set_id: self.set_id,
            set_name: self.set_name,
            rarity: self.rarity,
            color: self.color,
            card_type: self.card_type,
            life: self.life,
            cost: self.cost,
            power: self.power,
            counter_amount: self.counter_amount,
            attribute: self.attribute,
            sub_types: self.sub_types,
            text: self.text,
        }
    }
}

async fn stored(db: &SqlitePool, code: &CardCode) -> Result<Vec<Card>> {
    let code = code.as_str();
    let rows = sqlx::query_as!(
        CardRow,
        "SELECT id, card_code, name, set_id, set_name, rarity, color, card_type, life, cost, power,
                counter_amount, attribute, sub_types, text, price_usd, price_observed_on
         FROM cards WHERE card_code = ? ORDER BY position",
        code
    )
    .fetch_all(db)
    .await?;
    let valuer = exchange_rates::valuer(db).await?;
    Ok(rows.into_iter().map(|row| row.into_card(&valuer)).collect())
}

pub async fn by_ids(db: &SqlitePool, ids: &[&str]) -> Result<HashMap<String, Card>> {
    let ids = serde_json::to_string(ids).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    let rows = sqlx::query_as!(
        CardRow,
        "SELECT id, card_code, name, set_id, set_name, rarity, color, card_type, life, cost, power,
                counter_amount, attribute, sub_types, text, price_usd, price_observed_on
         FROM cards WHERE id IN (SELECT value FROM json_each(?))",
        ids
    )
    .fetch_all(db)
    .await?;
    let valuer = exchange_rates::valuer(db).await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.id.clone(), row.into_card(&valuer)))
        .collect())
}

pub async fn listed(db: &SqlitePool, catalogue: &Optcg, id: &CardId) -> Result<Option<Card>> {
    if let Some(card) = by_ids(db, &[id.as_str()]).await?.remove(id.as_str()) {
        return Ok(Some(card));
    }
    let today = CalendarDay::of(chrono::Utc::now()).to_string();
    for code in codes_in(id) {
        let found = cards_of(db, catalogue, &code, &today, Some(id))
            .await?
            .into_iter()
            .find(|card| card.id == id.as_str());
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(None)
}

fn codes_in(id: &CardId) -> impl Iterator<Item = CardCode> + '_ {
    (0..id.as_str().len()).filter_map(|start| {
        let tail = &id.as_str()[start..];
        let end = tail.find('-')? + "-000".len();
        CardCode::parse(tail.get(..end)?).ok()
    })
}

async fn store(db: &SqlitePool, catalogue: &Optcg, listed: &[CatalogueCard]) -> Result<()> {
    let mut images = Vec::with_capacity(listed.len());
    for card in listed {
        let source = Source::from_bytes(catalogue.image(&card.image_url).await?)
            .map_err(|_| CardsError::DataUnavailable)?;
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
        let code = card.card_code.as_str();
        let price_usd = card.price.as_ref().map(|price| price.usd);
        let price_observed_on = card.price.as_ref().map(|price| price.observed_on.as_str());
        let inserted = sqlx::query!(
            "INSERT INTO cards (id, card_code, position, name, set_id, set_name, rarity, color,
                card_type, life, cost, power, counter_amount, attribute, sub_types, text,
                price_usd, price_observed_on, image_avif)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
            price_usd,
            price_observed_on,
            image
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if let (1, Some(price)) = (inserted, &card.price) {
            price_history::record_first(&mut tx, id, price.usd, &price.observed_on).await?;
        }
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
    fn card_codes_are_found_inside_ids_in_order() {
        for (id, codes) in [
            ("OP01-001", &["OP01-001", "P01-001"][..]),
            ("OP04-016_P1", &["OP04-016", "P04-016"]),
            ("OP09-078-r1", &["OP09-078", "P09-078"]),
            ("EB03_OP09-034_p1", &["OP09-034", "P09-034"]),
            ("SPECIAL_BUGGY_BALL", &[]),
            ("OP01-01_p1", &[]),
        ] {
            let found: Vec<_> = codes_in(&CardId::parse(id).unwrap())
                .map(|code| code.as_str().to_owned())
                .collect();
            assert_eq!(found, codes, "{id}");
        }
    }

    #[test]
    fn card_ids_are_opaque_url_safe_catalogue_identifiers() {
        for text in [
            "OP01-001",
            "OP04-016_P1",
            "OP09-078-r1",
            "EB03_OP09-034_p1",
            "SPECIAL_BUGGY_BALL",
            &"a".repeat(64),
        ] {
            assert_eq!(CardId::parse(text).unwrap().as_str(), text);
        }
        for text in [
            "",
            "-OP01-001",
            "_p1",
            "OP01-001/..",
            "OP01-001.jpg",
            "OP01-001?x=1",
            "OP01 001",
            "OP01-001\n",
            "ÖP01-001",
            &"a".repeat(65),
        ] {
            assert!(CardId::parse(text).is_err(), "{text}");
        }
    }
}
