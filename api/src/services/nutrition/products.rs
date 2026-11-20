use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::food_database::{FoodDatabase, FoodError, FoodSnapshot, FoodSummary};
use super::nova;
use super::open_food_facts::{LabelProduct, Lookup, OpenFoodFacts, ProviderError};
use super::parameters;

pub const SUGGESTED_FOODS: usize = 5;
const MAX_REQUEST_ID_CHARS: usize = 128;
const MIN_NAME_WORD_CHARS: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum ProductError {
    #[error("{0}")]
    Validation(String),
    #[error("Open Food Facts has no product with barcode {0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Food(#[from] FoodError),
    #[error("Product database operation failed")]
    Database(#[from] sqlx::Error),
}

type Result<T> = std::result::Result<T, ProductError>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Barcode(String);

impl Barcode {
    pub fn parse(text: &str) -> Result<Self> {
        let digits: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let invalid = || {
            ProductError::Validation(
                "barcode must be the 8, 12, 13 or 14 digits printed under the bars, with a valid \
                 check digit"
                    .into(),
            )
        };
        if !digits.chars().all(|c| c.is_ascii_digit()) || ![8, 12, 13, 14].contains(&digits.len()) {
            return Err(invalid());
        }
        if digits.len() > 8 && !check_digit_matches(&digits) {
            return Err(invalid());
        }
        let canonical = match digits.len() {
            12 => format!("0{digits}"),
            14 if digits.starts_with('0') => digits[1..].to_owned(),
            _ => digits,
        };
        Ok(Self(canonical))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn check_digit_matches(digits: &str) -> bool {
    let values: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    let Some((check, data)) = values.split_last() else {
        return false;
    };
    let sum: u32 = data
        .iter()
        .rev()
        .enumerate()
        .map(|(index, digit)| if index % 2 == 0 { digit * 3 } else { *digit })
        .sum();
    (10 - sum % 10) % 10 == *check
}

#[derive(Debug, Serialize)]
pub struct LabelSource {
    pub name: &'static str,
    pub url: &'static str,
    pub credit: &'static str,
    pub licence: &'static str,
}

pub const LABEL_SOURCE: LabelSource = LabelSource {
    name: "Open Food Facts",
    url: "https://world.openfoodfacts.org",
    credit: "Product data © Open Food Facts contributors",
    licence: "ODbL",
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LabelAmount {
    pub key: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LinkedFood {
    pub food_id: i64,
    pub name_da: String,
    pub name_en: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Product {
    pub barcode: String,
    pub name: Option<String>,
    pub brand: Option<String>,
    pub nova_class: Option<u8>,
    pub label_per_100g: Vec<LabelAmount>,
    pub food: Option<LinkedFood>,
}

#[derive(Debug, Serialize)]
pub struct ProductLookup {
    pub linked: bool,
    pub product: Product,
    pub suggested_foods: Vec<FoodSummary>,
    pub source: LabelSource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkProduct {
    pub barcode: String,
    pub food_id: i64,
    pub request_id: String,
}

#[derive(Debug, Serialize)]
pub struct Linked {
    pub product: Product,
    pub replayed: bool,
    #[serde(skip)]
    pub created: bool,
    pub source: LabelSource,
}

#[derive(Debug)]
pub(super) struct StoredProduct {
    pub barcode: String,
    pub name: Option<String>,
    pub brand: Option<String>,
    pub nova_class: Option<u8>,
    pub food_id: i64,
    food_name_da: String,
    food_name_en: String,
    pub label: Vec<LabelAmount>,
}

impl StoredProduct {
    pub(super) fn links(&self, food: &FoodSnapshot) -> bool {
        food.food_id == self.food_id
            && (food.name_da == self.food_name_da || food.name_en == self.food_name_en)
    }

    pub(super) fn food_name_en(&self) -> &str {
        &self.food_name_en
    }

    fn into_product(self) -> Product {
        Product {
            barcode: self.barcode,
            name: self.name,
            brand: self.brand,
            nova_class: self.nova_class,
            label_per_100g: self.label,
            food: Some(LinkedFood {
                food_id: self.food_id,
                name_da: self.food_name_da,
                name_en: self.food_name_en,
            }),
        }
    }
}

fn vocabulary_position(key: &str) -> usize {
    parameters::PARAMETERS
        .iter()
        .position(|parameter| parameter.key == key)
        .unwrap_or(usize::MAX)
}

pub(super) async fn stored(
    db: &SqlitePool,
    barcodes: &[&Barcode],
) -> std::result::Result<HashMap<String, StoredProduct>, sqlx::Error> {
    stored_on(&mut *db.acquire().await?, barcodes).await
}

pub(super) async fn stored_on(
    db: &mut sqlx::SqliteConnection,
    barcodes: &[&Barcode],
) -> std::result::Result<HashMap<String, StoredProduct>, sqlx::Error> {
    let mut products = HashMap::new();
    for barcode in barcodes {
        let barcode = barcode.as_str();
        if products.contains_key(barcode) {
            continue;
        }
        let Some(row) = sqlx::query!(
            r#"SELECT barcode AS "barcode!", name, brand, nova_class, food_id, food_name_da,
                food_name_en FROM products WHERE barcode = ?"#,
            barcode
        )
        .fetch_optional(&mut *db)
        .await?
        else {
            continue;
        };
        let mut label: Vec<LabelAmount> = sqlx::query_as!(
            LabelAmount,
            "SELECT key, value, unit FROM product_label_values WHERE barcode = ?",
            barcode
        )
        .fetch_all(&mut *db)
        .await?;
        label.sort_by_key(|value| vocabulary_position(&value.key));
        products.insert(
            row.barcode.clone(),
            StoredProduct {
                barcode: row.barcode,
                name: row.name,
                brand: row.brand,
                nova_class: row.nova_class.and_then(|class| u8::try_from(class).ok()),
                food_id: row.food_id,
                food_name_da: row.food_name_da,
                food_name_en: row.food_name_en,
                label,
            },
        );
    }
    Ok(products)
}

async fn find(db: &SqlitePool, barcode: &Barcode) -> Result<Option<StoredProduct>> {
    Ok(stored(db, &[barcode]).await?.remove(barcode.as_str()))
}

async fn fetch(provider: &OpenFoodFacts, barcode: &Barcode) -> Result<LabelProduct> {
    match provider.product(barcode.as_str()).await? {
        Lookup::Found(product) => Ok(product),
        Lookup::NotFound => Err(ProductError::NotFound(barcode.as_str().to_owned())),
    }
}

pub async fn lookup(
    db: &SqlitePool,
    foods: &FoodDatabase,
    provider: &OpenFoodFacts,
    barcode: &str,
) -> Result<ProductLookup> {
    let barcode = Barcode::parse(barcode)?;
    if let Some(product) = find(db, &barcode).await? {
        return Ok(ProductLookup {
            linked: true,
            product: product.into_product(),
            suggested_foods: Vec::new(),
            source: LABEL_SOURCE,
        });
    }
    foods.check_ready()?;
    let label = fetch(provider, &barcode).await?;
    let mut suggested_foods = suggest(foods, &label).await?;
    nova::classify_summaries(db, &foods.version()?, &mut suggested_foods).await?;
    Ok(ProductLookup {
        linked: false,
        product: Product {
            barcode: barcode.0,
            name: label.name,
            brand: label.brand,
            nova_class: label.nova_class,
            label_per_100g: label
                .label
                .into_iter()
                .map(|value| LabelAmount {
                    key: value.key.to_owned(),
                    value: value.value,
                    unit: value.unit.to_owned(),
                })
                .collect(),
            food: None,
        },
        suggested_foods,
        source: LABEL_SOURCE,
    })
}

async fn suggest(foods: &FoodDatabase, label: &LabelProduct) -> Result<Vec<FoodSummary>> {
    let mut terms: Vec<String> = Vec::new();
    terms.extend(label.generic_name.clone());
    terms.extend(label.name.clone());
    terms.extend(label.categories.iter().cloned());
    if let Some(name) = &label.name {
        terms.extend(
            name.split(|c: char| !c.is_alphanumeric())
                .filter(|word| word.chars().count() >= MIN_NAME_WORD_CHARS)
                .filter(|word| {
                    label
                        .brand
                        .as_deref()
                        .is_none_or(|brand| !brand.to_lowercase().contains(&word.to_lowercase()))
                })
                .map(str::to_owned),
        );
    }
    let mut suggested: Vec<FoodSummary> = Vec::new();
    for term in terms {
        if suggested.len() >= SUGGESTED_FOODS {
            break;
        }
        let found = match foods.search(&term, Some(SUGGESTED_FOODS)).await {
            Ok(found) => found.foods,
            Err(FoodError::Validation(_)) => continue,
            Err(error) => return Err(error.into()),
        };
        for food in found {
            if suggested.len() < SUGGESTED_FOODS
                && !suggested.iter().any(|known| known.food_id == food.food_id)
            {
                suggested.push(food);
            }
        }
    }
    Ok(suggested)
}

fn request_id(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_REQUEST_ID_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(ProductError::Validation(format!(
            "request_id must contain 1-{MAX_REQUEST_ID_CHARS} characters without control characters"
        )));
    }
    Ok(value.to_owned())
}

async fn replay(
    db: &SqlitePool,
    request_id: &str,
    barcode: &Barcode,
    food_id: i64,
) -> Result<Option<Linked>> {
    let Some(receipt) = sqlx::query!(
        "SELECT barcode, food_id, result FROM product_link_receipts WHERE request_id = ?",
        request_id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    if receipt.barcode != barcode.as_str() || receipt.food_id != food_id {
        return Err(ProductError::Conflict(
            "request_id was already used to link another barcode or Food; use a new ID".into(),
        ));
    }
    find(db, barcode).await?.ok_or_else(|| {
        ProductError::Conflict("The linked Product is no longer stored; link it again".into())
    })?;
    let result = receipt.result.ok_or_else(|| {
        ProductError::Conflict(
            "This legacy receipt has no original result; look the Product up before retrying"
                .into(),
        )
    })?;
    let product =
        serde_json::from_str(&result).map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
    Ok(Some(Linked {
        product,
        replayed: true,
        created: false,
        source: LABEL_SOURCE,
    }))
}

pub async fn link(
    db: &SqlitePool,
    foods: &FoodDatabase,
    provider: &OpenFoodFacts,
    request: LinkProduct,
) -> Result<Linked> {
    let barcode = Barcode::parse(&request.barcode)?;
    if request.food_id <= 0 {
        return Err(ProductError::Validation(
            "food_id must be a positive whole number from search_foods or suggested_foods".into(),
        ));
    }
    let request_id = request_id(&request.request_id)?;
    if let Some(linked) = replay(db, &request_id, &barcode, request.food_id).await? {
        return Ok(linked);
    }
    let food = foods
        .snapshots(&[request.food_id])
        .await?
        .remove(&request.food_id)
        .ok_or(FoodError::NotFound(request.food_id))?;
    let label = if find(db, &barcode).await?.is_some() {
        None
    } else {
        Some(fetch(provider, &barcode).await?)
    };

    let code = barcode.as_str();
    let mut transaction = db.begin().await?;
    let claimed = sqlx::query_scalar!(
        "INSERT INTO product_link_receipts (request_id, barcode, food_id) VALUES (?, ?, ?) ON CONFLICT (request_id) DO NOTHING RETURNING request_id",
        request_id, code, request.food_id)
        .fetch_optional(&mut *transaction)
        .await?;
    if claimed.is_none() {
        transaction.rollback().await?;
        return replay(db, &request_id, &barcode, request.food_id)
            .await?
            .ok_or_else(|| {
                ProductError::Conflict("Retry receipt unavailable; look the barcode up".into())
            });
    }
    let created = match &label {
        Some(label) => {
            let nova_class = label.nova_class.map(i64::from);
            let inserted = sqlx::query_scalar!(
                r#"INSERT INTO products (barcode, name, brand, nova_class, food_id, food_name_da, food_name_en)
                VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT (barcode) DO NOTHING RETURNING barcode AS "barcode!""#,
                code, label.name, label.brand, nova_class, food.food_id, food.name_da, food.name_en)
                .fetch_optional(&mut *transaction)
                .await?
                .is_some();
            if inserted {
                for value in &label.label {
                    sqlx::query!(
                        "INSERT INTO product_label_values (barcode, key, value, unit) VALUES (?, ?, ?, ?)",
                        code, value.key, value.value, value.unit)
                        .execute(&mut *transaction)
                        .await?;
                }
            }
            inserted
        }
        None => false,
    };
    if !created {
        sqlx::query!(
            "UPDATE products SET food_id = ?, food_name_da = ?, food_name_en = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE barcode = ?",
            food.food_id, food.name_da, food.name_en, code)
            .execute(&mut *transaction)
            .await?;
    }
    let product = stored_on(&mut transaction, &[&barcode])
        .await?
        .remove(code)
        .ok_or_else(|| ProductError::Conflict("The Product was removed while linking".into()))?
        .into_product();
    let result =
        serde_json::to_string(&product).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    sqlx::query!(
        "UPDATE product_link_receipts SET result = ? WHERE request_id = ?",
        result,
        request_id
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Linked {
        product,
        replayed: false,
        created,
        source: LABEL_SOURCE,
    })
}
