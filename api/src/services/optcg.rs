//! OPTCG catalogue reads (optcgapi.com, ADR 0017). It is one volunteer's server, so every call is
//! a single bounded GET: a short timeout, a byte cap and no redirects or retries. Nothing the
//! catalogue sends is trusted. A variant's image URL is fetched only when it names the configured
//! origin, and every failure is [`Unavailable`], so upstream text never reaches a caller.
//!
//! The catalogue answers `api/sets/card/{code}/` with a JSON array of variants, or 404
//! `{"error":"Card not found."}` for a code it does not list. It serves PNGs under `.jpg` names.

use std::time::Duration;

use reqwest::{Client, Response, StatusCode, header};
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use super::cards::{CardCode, CardId};
use super::nutrition::open_food_facts::USER_AGENT;

pub const PRODUCTION_URL: &str = "https://optcgapi.com/";
const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CARD_BYTES: usize = 256 * 1024;
const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
/// Normal print plus parallels; a code with more variants than this is not believable.
const MAX_VARIANTS: usize = 20;
const MAX_TEXT_CHARS: usize = 200;
const MAX_CARD_TEXT_CHARS: usize = 4_000;
const MAX_PRICE_USD: f64 = 1_000_000.0;

/// Unreachable, slow, refused, oversized or malformed; the cause is only logged.
#[derive(Debug)]
pub struct Unavailable;

#[derive(Clone)]
pub struct Optcg {
    client: Client,
    base_url: Url,
}

#[derive(Debug)]
pub struct Price {
    pub usd: f64,
    /// `YYYY-MM-DD`: the day the catalogue observed the price.
    pub observed_on: String,
}

#[derive(Debug)]
pub struct CatalogueCard {
    pub id: CardId,
    pub card_code: CardCode,
    /// On the adapter's configured origin, which is checked before a card is returned.
    pub image_url: Url,
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
    pub price: Option<Price>,
}

/// Every field is read loosely (`life` is `"5"`, other cards may send numbers) and checked after.
#[derive(Deserialize)]
struct Raw {
    card_name: Option<Value>,
    set_id: Option<Value>,
    set_name: Option<Value>,
    rarity: Option<Value>,
    card_set_id: Option<Value>,
    card_color: Option<Value>,
    card_type: Option<Value>,
    life: Option<Value>,
    card_cost: Option<Value>,
    card_power: Option<Value>,
    counter_amount: Option<Value>,
    attribute: Option<Value>,
    sub_types: Option<Value>,
    card_text: Option<Value>,
    market_price: Option<Value>,
    date_scraped: Option<Value>,
    card_image_id: Option<Value>,
    card_image: Option<Value>,
}

impl Optcg {
    pub fn new(base_url: Url) -> anyhow::Result<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { client, base_url })
    }

    /// Every variant of `code`; empty when the catalogue does not list it.
    pub async fn card(&self, code: &CardCode) -> Result<Vec<CatalogueCard>, Unavailable> {
        let url = self
            .base_url
            .join(&format!("api/sets/card/{}/", code.as_str()))
            .map_err(|error| unavailable("bad URL", &error))?;
        let response = self.get(url).await?;
        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return not_listed(&read_capped(response, MAX_CARD_BYTES).await?);
        }
        if !status.is_success() {
            return Err(unavailable("refused", &status));
        }
        let bytes = read_capped(response, MAX_CARD_BYTES).await?;
        let raw: Vec<Raw> = serde_json::from_slice(&bytes)
            .map_err(|error| unavailable("the card was not valid JSON", &error))?;
        if raw.len() > MAX_VARIANTS {
            return Err(unavailable("too many variants", &raw.len()));
        }
        let cards = raw
            .into_iter()
            .map(|raw| raw.validate(code, &self.base_url))
            .collect::<Result<Vec<_>, _>>()?;
        let mut ids: Vec<_> = cards.iter().map(|card| card.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != cards.len() {
            return Err(unavailable("duplicate variants", &code.as_str()));
        }
        Ok(cards)
    }

    /// The variant's official image bytes (a PNG or JPEG) from its catalogue URL.
    pub async fn image(&self, url: &Url) -> Result<Vec<u8>, Unavailable> {
        let response = self.get(url.clone()).await?;
        if !response.status().is_success() {
            return Err(unavailable("refused an image", &response.status()));
        }
        read_capped(response, MAX_IMAGE_BYTES).await
    }

    async fn get(&self, url: Url) -> Result<Response, Unavailable> {
        self.client
            .get(url)
            .send()
            .await
            .map_err(|error| unavailable("request failed", &error))
    }
}

/// Bounds untrusted bodies even when `Content-Length` is missing or wrong.
async fn read_capped(mut response: Response, limit: usize) -> Result<Vec<u8>, Unavailable> {
    if response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|length| length.to_str().ok()?.parse::<usize>().ok())
        .is_some_and(|length| length > limit)
    {
        return Err(unavailable("oversized body", &limit));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| unavailable("reading the body failed", &error))?
    {
        if bytes.len() + chunk.len() > limit {
            return Err(unavailable("oversized body", &limit));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Only the catalogue's own answer means "unlisted": any other 404 (a proxy, a moved endpoint)
/// would otherwise be remembered as unlisted for the day.
fn not_listed(body: &[u8]) -> Result<Vec<CatalogueCard>, Unavailable> {
    #[derive(Deserialize)]
    struct NotFound {
        error: String,
    }
    match serde_json::from_slice::<NotFound>(body) {
        Ok(NotFound { error }) if error == "Card not found." => Ok(Vec::new()),
        _ => Err(unavailable(
            "a 404 that is not the catalogue's not-found answer",
            &body.len(),
        )),
    }
}

fn unavailable(what: &str, detail: &dyn std::fmt::Debug) -> Unavailable {
    tracing::warn!(?detail, "OPTCG catalogue read: {what}");
    Unavailable
}

fn text(value: Option<Value>, max_chars: usize) -> Result<Option<String>, Unavailable> {
    let text = match value {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(text)) => text.trim().to_owned(),
        Some(Value::Number(number)) => number.to_string(),
        Some(other) => return Err(unavailable("unexpected field type", &other)),
    };
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > max_chars || text.chars().any(|c| c.is_control() && c != '\n') {
        return Err(unavailable("unacceptable text", &text.len()));
    }
    Ok(Some(text))
}

fn number(value: Option<&Value>) -> Result<Option<f64>, Unavailable> {
    let parsed = match value {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(text)) => text.trim().parse().ok(),
        Some(_) => None,
    };
    parsed
        .filter(|price| price.is_finite() && (0.0..=MAX_PRICE_USD).contains(price))
        .map(Some)
        .ok_or_else(|| unavailable("unacceptable price", &value))
}

impl Raw {
    fn validate(self, code: &CardCode, origin: &Url) -> Result<CatalogueCard, Unavailable> {
        let missing = |what| unavailable(what, &code.as_str());
        let listed_code = text(self.card_set_id, MAX_TEXT_CHARS)?;
        let id = text(self.card_image_id, MAX_TEXT_CHARS)?
            .and_then(|id| CardId::parse(&id).ok())
            .ok_or_else(|| missing("no usable variant id"))?;
        if listed_code.as_deref() != Some(code.as_str()) {
            return Err(missing("a variant of another code"));
        }
        let image_url = text(self.card_image, MAX_TEXT_CHARS)?
            .and_then(|url| Url::parse(&url).ok())
            .filter(|url| url.origin() == origin.origin())
            .ok_or_else(|| missing("no image on the catalogue's origin"))?;
        let market_price = number(self.market_price.as_ref())?;
        let observed_on = text(self.date_scraped, MAX_TEXT_CHARS)?.filter(|date| {
            date.len() == 10 && chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok()
        });
        let price = match (market_price, observed_on) {
            (Some(usd), Some(observed_on)) => Some(Price { usd, observed_on }),
            (None, _) => None,
            (Some(_), None) => return Err(missing("a price without an observation date")),
        };
        Ok(CatalogueCard {
            id,
            card_code: code.clone(),
            image_url,
            name: text(self.card_name, MAX_TEXT_CHARS)?.ok_or_else(|| missing("no name"))?,
            set_id: text(self.set_id, MAX_TEXT_CHARS)?,
            set_name: text(self.set_name, MAX_TEXT_CHARS)?,
            rarity: text(self.rarity, MAX_TEXT_CHARS)?,
            color: text(self.card_color, MAX_TEXT_CHARS)?,
            card_type: text(self.card_type, MAX_TEXT_CHARS)?,
            life: text(self.life, MAX_TEXT_CHARS)?,
            cost: text(self.card_cost, MAX_TEXT_CHARS)?,
            power: text(self.card_power, MAX_TEXT_CHARS)?,
            counter_amount: text(self.counter_amount, MAX_TEXT_CHARS)?,
            attribute: text(self.attribute, MAX_TEXT_CHARS)?,
            sub_types: text(self.sub_types, MAX_TEXT_CHARS)?,
            text: text(self.card_text, MAX_CARD_TEXT_CHARS)?,
            price,
        })
    }
}
