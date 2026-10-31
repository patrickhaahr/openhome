//! Open Food Facts barcode reads (`CONTEXT.md`, Product).
//!
//! One product read per call: only the fields a Product needs, an identifying User-Agent without
//! any personal contact detail, a short timeout, a byte cap and no retries, so a rate limit is
//! answered as unavailable rather than hammered. Upstream text never reaches a caller: every
//! failure is [`ProviderError::Unavailable`].
//!
//! Label values are normalised into the parameter vocabulary and its units. Open Food Facts
//! stores every `_100g` mass in grams (vitamin D 1.5 µg is `0.0000015`), energy in kJ and kcal,
//! and alcohol in % vol.

use std::collections::HashMap;
use std::time::Duration;

use reqwest::{Client, StatusCode, header};
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use super::parameters;

/// The public Open Food Facts origin.
pub const PRODUCTION_URL: &str = "https://world.openfoodfacts.org/";
/// Identifies the app to Open Food Facts, as its API asks; never a person or an email address.
pub const USER_AGENT: &str = concat!(
    "OpenHome/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/patrickhaahr/openhome)"
);
/// The product fields a Product is built from; everything else stays upstream.
pub const FIELDS: &str = "code,product_name,product_name_en,product_name_da,generic_name,brands,\
categories_tags,nova_group,nutriments";
const TIMEOUT: Duration = Duration::from_secs(4);
/// A product limited to [`FIELDS`] is a few kilobytes.
const MAX_BODY_BYTES: usize = 256 * 1024;
const MAX_TEXT_CHARS: usize = 200;
/// Category tags used as Food search terms, most specific first.
const MAX_CATEGORIES: usize = 3;
const KJ_PER_KCAL: f64 = 4.184;
/// Salt is sodium × 2.5 on EU labels.
const SALT_PER_SODIUM: f64 = 2.5;
/// Grams of ethanol per millilitre: alcohol in % vol × 0.789 is grams per 100 ml, taken as per
/// 100 g for drinks near the density of water.
const ETHANOL_DENSITY: f64 = 0.789;

/// Open Food Facts mass nutriments and their vocabulary keys; the first source of a key wins.
const MASSES: &[(&str, &str)] = &[
    ("proteins", parameters::PROTEIN),
    // EU labels declare available carbohydrate; fibre is declared separately.
    ("carbohydrates", parameters::AVAILABLE_CARBOHYDRATE),
    ("sugars", parameters::SUGARS),
    ("added-sugars", "added_sugars"),
    ("fiber", parameters::DIETARY_FIBRE),
    ("fat", parameters::FAT),
    ("saturated-fat", "saturated_fat"),
    ("monounsaturated-fat", "monounsaturated_fat"),
    ("polyunsaturated-fat", "polyunsaturated_fat"),
    ("trans-fat", "trans_fat"),
    ("omega-3-fat", "omega_3_fatty_acids"),
    ("omega-6-fat", "omega_6_fatty_acids"),
    ("alpha-linolenic-acid", "ala"),
    ("eicosapentaenoic-acid", "epa"),
    ("docosahexaenoic-acid", "dha"),
    ("linoleic-acid", "linoleic_acid"),
    ("cholesterol", "cholesterol"),
    ("starch", "starch"),
    ("lactose", "lactose"),
    ("polyols", "sum_sugar_alcohols"),
    ("salt", SALT),
    ("sodium", SODIUM),
    ("vitamin-a", "vitamin_a"),
    ("vitamin-d", "vitamin_d"),
    ("vitamin-e", "vitamin_e"),
    ("vitamin-k", "vitamin_k"),
    ("vitamin-c", "vitamin_c"),
    ("vitamin-b1", "vitamin_b1"),
    ("vitamin-b2", "vitamin_b2"),
    // Labels declare niacin as niacin equivalents.
    ("vitamin-pp", "niacin_equivalent"),
    ("vitamin-b6", "vitamin_b6"),
    ("vitamin-b9", "folate"),
    ("folates", "folate"),
    ("vitamin-b12", "vitamin_b12"),
    ("biotin", "biotin"),
    ("pantothenic-acid", "pantothenic_acid"),
    ("calcium", "calcium"),
    ("phosphorus", "phosphorus"),
    ("iron", "iron"),
    ("magnesium", "magnesium"),
    ("zinc", "zinc"),
    ("copper", "copper"),
    ("manganese", "manganese"),
    ("fluoride", "fluoride"),
    ("selenium", "selenium"),
    ("chromium", "chromium"),
    ("molybdenum", "molybdenum"),
    ("iodine", "iodine"),
    ("potassium", "potassium"),
    ("chloride", "chloride"),
    ("caffeine", "caffeine"),
];
const SALT: &str = "salt_labelling";
const SODIUM: &str = "sodium";

/// A bounded, redirect-free Open Food Facts client.
#[derive(Clone)]
pub struct OpenFoodFacts {
    client: Client,
    base_url: Url,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// Unreachable, slow, rate limited, oversized or malformed; the cause is only logged.
    #[error("Open Food Facts is unavailable")]
    Unavailable,
}

/// What Open Food Facts knows about a barcode.
#[derive(Debug)]
pub enum Lookup {
    Found(LabelProduct),
    NotFound,
}

/// A product as its label describes it, normalised to the vocabulary.
#[derive(Debug)]
pub struct LabelProduct {
    pub name: Option<String>,
    /// The first listed brand.
    pub brand: Option<String>,
    /// Open Food Facts' `nova_group`, 1-4.
    pub nova_class: Option<u8>,
    /// Every recognised per-100 g label value, in vocabulary order.
    pub label: Vec<LabelValue>,
    pub generic_name: Option<String>,
    /// Category names, most specific first, for suggesting Foods.
    pub categories: Vec<String>,
}

/// One per-100 g label value in its vocabulary unit.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelValue {
    pub key: &'static str,
    pub value: f64,
    pub unit: &'static str,
}

#[derive(Deserialize)]
struct ProviderResponse {
    status: Option<Value>,
    product: Option<ProviderProduct>,
}

#[derive(Deserialize)]
struct ProviderProduct {
    product_name: Option<Value>,
    product_name_en: Option<Value>,
    product_name_da: Option<Value>,
    generic_name: Option<Value>,
    brands: Option<Value>,
    categories_tags: Option<Value>,
    nova_group: Option<Value>,
    nutriments: Option<HashMap<String, Value>>,
}

impl OpenFoodFacts {
    /// A client for `base_url`: [`PRODUCTION_URL`] in production, a local stand-in in tests.
    pub fn new(base_url: Url) -> anyhow::Result<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self { client, base_url })
    }

    /// Reads one product by its canonical barcode digits. One request, never retried.
    pub async fn product(&self, barcode: &str) -> Result<Lookup, ProviderError> {
        let mut url = self
            .base_url
            .join(&format!("api/v2/product/{barcode}"))
            .map_err(|_| ProviderError::Unavailable)?;
        url.query_pairs_mut().append_pair("fields", FIELDS);
        let mut response = self
            .client
            .get(url)
            .header(header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|error| unavailable("request failed", &error))?;
        match response.status() {
            StatusCode::NOT_FOUND => return Ok(Lookup::NotFound),
            status if !status.is_success() => {
                tracing::warn!(%status, "Open Food Facts refused a product read");
                return Err(ProviderError::Unavailable);
            }
            _ => {}
        }
        // Bound untrusted bodies even when Content-Length is missing or wrong.
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| unavailable("reading the body failed", &error))?
        {
            if bytes.len() + chunk.len() > MAX_BODY_BYTES {
                tracing::warn!("Open Food Facts sent an oversized product");
                return Err(ProviderError::Unavailable);
            }
            bytes.extend_from_slice(&chunk);
        }
        let response: ProviderResponse = serde_json::from_slice(&bytes)
            .map_err(|error| unavailable("the product was not valid JSON", &error))?;
        // Version 2 answers an unknown barcode with status 0, usually with a 404.
        let missing = response
            .status
            .as_ref()
            .and_then(number)
            .is_some_and(|status| status.abs() < 1e-9);
        match response.product {
            Some(product) if !missing => Ok(Lookup::Found(product.normalise())),
            _ => Ok(Lookup::NotFound),
        }
    }
}

fn unavailable(what: &str, error: &dyn std::fmt::Display) -> ProviderError {
    tracing::warn!(%error, "Open Food Facts product read: {what}");
    ProviderError::Unavailable
}

/// A number written as a JSON number or a numeric string.
fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .filter(|value: &f64| value.is_finite())
}

/// Display text: trimmed, single-spaced, without control characters and of bounded length.
fn text(value: Option<&Value>) -> Option<String> {
    let Some(Value::String(text)) = value else {
        return None;
    };
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()
        && text.chars().count() <= MAX_TEXT_CHARS
        && !text.chars().any(char::is_control))
    .then_some(text)
}

impl ProviderProduct {
    fn normalise(self) -> LabelProduct {
        let name = [
            &self.product_name,
            &self.product_name_en,
            &self.product_name_da,
        ]
        .into_iter()
        .find_map(|name| text(name.as_ref()));
        let brand = match &self.brands {
            Some(Value::String(brands)) => brands
                .split(',')
                .find_map(|brand| text(Some(&Value::String(brand.to_owned())))),
            _ => None,
        };
        let nova_class =
            self.nova_group.as_ref().and_then(number).and_then(|class| {
                (1..=4_u8).find(|known| (f64::from(*known) - class).abs() < 1e-9)
            });
        let categories = match &self.categories_tags {
            Some(Value::Array(tags)) => tags
                .iter()
                .rev()
                .filter_map(|tag| tag.as_str().and_then(category))
                .take(MAX_CATEGORIES)
                .collect(),
            _ => Vec::new(),
        };
        LabelProduct {
            name,
            brand,
            nova_class,
            label: label_values(&self.nutriments.unwrap_or_default()),
            generic_name: text(self.generic_name.as_ref()),
            categories,
        }
    }
}

/// A category tag such as `en:ice-cream-bars` or `da:skyr` as words: `ice cream bars`.
fn category(tag: &str) -> Option<String> {
    let (language, name) = tag.split_once(':')?;
    if language.len() != 2 || !language.chars().all(|c| c.is_ascii_lowercase()) {
        return None;
    }
    text(Some(&Value::String(name.replace('-', " "))))
}

/// Six significant digits, so a unit conversion (`0.0000015 g` to `1.5 µg`) leaves no float noise.
pub(super) fn significant(value: f64) -> f64 {
    if value == 0.0 {
        return 0.0;
    }
    let scale = 10_f64.powi(5 - value.abs().log10().floor() as i32);
    (value * scale).round() / scale
}

/// The label's per-100 g values in vocabulary keys and units, in vocabulary order. Implausible
/// values (negative, or more than 100 g in 100 g) are left out as unknown.
fn label_values(nutriments: &HashMap<String, Value>) -> Vec<LabelValue> {
    let per_100g = |name: &str| {
        nutriments
            .get(&format!("{name}_100g"))
            .and_then(number)
            .filter(|value| *value >= 0.0)
    };
    // Grams per 100 g, by vocabulary key.
    let mut grams: HashMap<&'static str, f64> = HashMap::new();
    for (name, key) in MASSES {
        if let Some(value) = per_100g(name).filter(|value| *value <= 100.0) {
            grams.entry(key).or_insert(value);
        }
    }
    match (grams.get(SALT).copied(), grams.get(SODIUM).copied()) {
        (Some(salt), None) => {
            grams.insert(SODIUM, salt / SALT_PER_SODIUM);
        }
        (None, Some(sodium)) => {
            grams.insert(SALT, sodium * SALT_PER_SODIUM);
        }
        _ => {}
    }
    let mut values: HashMap<&'static str, f64> = grams
        .into_iter()
        .filter_map(|(key, grams)| {
            let unit = parameters::by_key(key)?.unit;
            Some((key, grams * unit.per_gram()?))
        })
        .collect();
    if let Some(alcohol) = per_100g("alcohol").filter(|value| *value <= 100.0) {
        values.insert(parameters::ALCOHOL, alcohol * ETHANOL_DENSITY);
    }
    let kcal = per_100g("energy-kcal").filter(|value| *value <= 900.0);
    let kj = per_100g("energy-kj")
        .or_else(|| per_100g("energy"))
        .filter(|value| *value <= 900.0 * KJ_PER_KCAL);
    if let Some(kcal) = kcal.or_else(|| kj.map(|kj| kj / KJ_PER_KCAL)) {
        values.insert(parameters::ENERGY_KCAL, kcal);
    }
    if let Some(kj) = kj.or_else(|| kcal.map(|kcal| kcal * KJ_PER_KCAL)) {
        values.insert(parameters::ENERGY_KJ, kj);
    }
    parameters::PARAMETERS
        .iter()
        .filter_map(|parameter| {
            let value = values.get(parameter.key)?;
            Some(LabelValue {
                key: parameter.key,
                value: significant(*value),
                unit: parameter.unit.symbol()?,
            })
        })
        .collect()
}
