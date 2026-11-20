use super::{
    FUTURE_TOLERANCE_MINUTES, MAX_GRAMS, MAX_NOTE_CHARS, MAX_PORTIONS, MAX_REQUEST_ID_CHARS,
    MealLogError, PortionRequest, Result,
};
use crate::services::nutrition::parameters::{self, Unit};
use crate::services::nutrition::products::Barcode;
use crate::services::nutrition::supplements::{self, DoseRequest};
use crate::services::patch::Patch;
use crate::services::training::calendar::CalendarDay;
use chrono::{DateTime, TimeDelta, Utc};
use std::collections::BTreeMap;

pub(crate) fn parse_request_id(value: &str) -> std::result::Result<String, String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_REQUEST_ID_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "request_id must contain 1-{MAX_REQUEST_ID_CHARS} characters without control characters"
        ));
    }
    Ok(value.to_owned())
}

pub(super) fn request_id(value: &str) -> Result<String> {
    parse_request_id(value).map_err(MealLogError::Validation)
}

pub(super) fn eaten_at(value: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>> {
    let at = DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .and_then(|at| DateTime::from_timestamp(at.timestamp(), 0))
        .ok_or_else(|| {
            MealLogError::Validation(
                "eaten_at must be an RFC 3339 timestamp with an offset, e.g. \
                 2026-10-30T08:15:00+01:00"
                    .into(),
            )
        })?;
    if at > now + TimeDelta::minutes(FUTURE_TOLERANCE_MINUTES) {
        return Err(MealLogError::Validation(
            "eaten_at is in the future; log a meal once it has been eaten".into(),
        ));
    }
    if CalendarDay::parse(&CalendarDay::of(at).to_string()).is_none() {
        return Err(MealLogError::Validation(
            "eaten_at is outside the supported range".into(),
        ));
    }
    Ok(at)
}

pub(super) fn utc_text(at: DateTime<Utc>) -> String {
    at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub(super) fn note(value: Option<String>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > MAX_NOTE_CHARS
        || value
            .chars()
            .any(|character| character.is_control() && character != '\n')
    {
        return Err(MealLogError::Validation(format!(
            "note must be at most {MAX_NOTE_CHARS} characters without control characters"
        )));
    }
    Ok(Some(value.to_owned()))
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::services::nutrition) enum Item {
    Food(i64),
    Product(Barcode),
}

impl Item {
    pub(in crate::services::nutrition) fn describe(&self) -> String {
        match self {
            Self::Food(food_id) => format!("food_id {food_id}"),
            Self::Product(barcode) => format!("barcode {}", barcode.as_str()),
        }
    }

    pub(in crate::services::nutrition) fn normalized(&self, grams: f64) -> serde_json::Value {
        match self {
            Self::Food(food_id) => serde_json::json!({"food_id": food_id, "grams": grams}),
            Self::Product(barcode) => {
                serde_json::json!({"barcode": barcode.as_str(), "grams": grams})
            }
        }
    }
}

pub(in crate::services::nutrition) fn parse_item(
    food_id: Option<i64>,
    barcode: Option<String>,
) -> std::result::Result<Item, String> {
    match (food_id, barcode) {
        (Some(food_id), None) if food_id > 0 => Ok(Item::Food(food_id)),
        (Some(_), None) => Err("food_id must be a positive whole number from search_foods".into()),
        (None, Some(barcode)) => Barcode::parse(&barcode)
            .map(Item::Product)
            .map_err(|error| error.to_string()),
        _ => Err("each item needs either a food_id or the barcode of a linked Product".into()),
    }
}

pub(in crate::services::nutrition) fn check_grams(grams: f64) -> std::result::Result<(), String> {
    if !grams.is_finite() || grams <= 0.0 || grams > MAX_GRAMS {
        return Err(format!(
            "grams must be edible grams above 0 and at most {MAX_GRAMS}"
        ));
    }
    Ok(())
}

pub(super) struct ValidPortion {
    pub(super) item: Item,
    pub(super) grams: f64,
    pub(super) estimates: BTreeMap<String, f64>,
}

impl ValidPortion {
    pub(super) fn normalized(&self) -> serde_json::Value {
        match &self.item {
            Item::Food(food_id) if self.estimates.is_empty() => {
                serde_json::json!([food_id, self.grams])
            }
            Item::Food(food_id) => serde_json::json!({
                "food_id": food_id, "grams": self.grams, "estimates": self.estimates,
            }),
            Item::Product(barcode) => serde_json::json!({
                "barcode": barcode.as_str(), "grams": self.grams, "estimates": self.estimates,
            }),
        }
    }
}

const MAX_KCAL_PER_100G: f64 = 900.0;
const MAX_KJ_PER_100G: f64 = 3_800.0;

fn estimate(key: &str, value: f64) -> Result<()> {
    let invalid = |reason: &str| {
        Err(MealLogError::Validation(format!(
            "estimates.{key} {reason}; estimates are per 100 g edible part, keyed and united as in \
             the Food database's parameters"
        )))
    };
    let Some(parameter) = parameters::by_key(key).filter(|parameter| parameter.unit.per_100g())
    else {
        return invalid("is not a per-100 g parameter key");
    };
    let limit = match parameter.unit {
        Unit::Kilocalorie => MAX_KCAL_PER_100G,
        Unit::Kilojoule => MAX_KJ_PER_100G,
        unit => unit
            .per_gram()
            .map_or(f64::MAX, |per_gram| 100.0 * per_gram),
    };
    if !value.is_finite() || value < 0.0 || value > limit {
        return invalid(&format!("must be between 0 and {limit}"));
    }
    Ok(())
}

pub(super) fn portions(requests: Vec<PortionRequest>) -> Result<Vec<ValidPortion>> {
    if requests.len() > MAX_PORTIONS {
        return Err(MealLogError::Validation(format!(
            "portions must list at most {MAX_PORTIONS} Portions"
        )));
    }
    requests
        .into_iter()
        .map(|portion| {
            let item =
                parse_item(portion.food_id, portion.barcode).map_err(MealLogError::Validation)?;
            check_grams(portion.grams).map_err(MealLogError::Validation)?;
            for (key, value) in &portion.estimates {
                estimate(key, *value)?;
            }
            Ok(ValidPortion {
                item,
                grams: portion.grams,
                estimates: portion.estimates,
            })
        })
        .collect()
}

pub(super) fn doses(requests: Vec<DoseRequest>) -> Result<Vec<(String, f64)>> {
    supplements::parse_doses(requests).map_err(MealLogError::Validation)
}

pub(super) fn check_items(portions: usize, doses: usize) -> Result<()> {
    if portions + doses == 0 {
        return Err(MealLogError::Validation(
            "a Meal Log needs at least one Portion or Supplement Dose".into(),
        ));
    }
    Ok(())
}

pub(super) fn required<T>(patch: Patch<T>, field: &str) -> Result<Option<T>> {
    match patch {
        Patch::Missing => Ok(None),
        Patch::Value(Some(value)) => Ok(Some(value)),
        Patch::Value(None) => Err(MealLogError::Validation(format!("{field} cannot be null"))),
    }
}

pub fn parse_day(text: &str, field: &str) -> Result<CalendarDay> {
    CalendarDay::parse(text).ok_or_else(|| {
        MealLogError::Validation(format!(
            "{field} must be a Europe/Copenhagen calendar date written YYYY-MM-DD"
        ))
    })
}
