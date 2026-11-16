use super::{
    Item, MealLogError, OverrideRequest, PortionRequest, Result, ValidPortion, check_grams,
    check_items, doses, parse_item, portions,
};
use crate::services::nutrition::meals::UniqueName;
use crate::services::nutrition::supplements::{self, DoseRequest};
use serde::Serialize;

/// The arguments a receipt compares on retry. `photo` is the upload's SHA-256, left out when
/// there is none so receipts written before photos existed still match their retries. A log by
/// Meal name has `meal` (its lowercased name) and `overrides` instead of `portions`, so a retry
/// replays even after the Meal was edited.
#[derive(Serialize)]
pub(super) struct Normalized<'a> {
    pub(super) eaten_at: &'a str,
    pub(super) note: Option<&'a str>,
    /// `[food_id, grams]` for a Food without estimates, as receipts were first written;
    /// otherwise an object.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) portions: Option<Vec<serde_json::Value>>,
    /// Left out without doses, so receipts written before Supplements existed still match.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) supplement_doses: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) meal: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) overrides: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) photo: Option<String>,
}

/// What a log holds: its own Portions and Supplement Doses, or a saved Meal's items with one-off
/// grams and doses.
pub(super) enum Contents {
    Items {
        portions: Vec<ValidPortion>,
        doses: Vec<(String, f64)>,
    },
    Meal {
        name: UniqueName,
        overrides: Vec<(Item, f64)>,
        dose_overrides: Vec<(String, f64)>,
    },
}

impl Contents {
    pub(super) fn normalized<'a>(
        &'a self,
        eaten_at: &'a str,
        note: Option<&'a str>,
        photo: Option<String>,
    ) -> Normalized<'a> {
        let (portions, supplement_doses, meal, overrides) = match self {
            Self::Items { portions, doses } => (
                Some(portions.iter().map(ValidPortion::normalized).collect()),
                (!doses.is_empty()).then(|| {
                    doses
                        .iter()
                        .map(|(id, doses)| supplements::normalized(id, *doses))
                        .collect()
                }),
                None,
                None,
            ),
            Self::Meal {
                name,
                overrides,
                dose_overrides,
            } => {
                let mut overrides: Vec<serde_json::Value> = overrides
                    .iter()
                    .map(|(item, grams)| item.normalized(*grams))
                    .chain(
                        dose_overrides
                            .iter()
                            .map(|(id, doses)| supplements::normalized(id, *doses)),
                    )
                    .collect();
                // Overrides name distinct items, so their order carries no meaning.
                overrides.sort_by_key(ToString::to_string);
                (None, None, Some(name.key.as_str()), Some(overrides))
            }
        };
        Normalized {
            eaten_at,
            note,
            portions,
            supplement_doses,
            meal,
            overrides,
            photo,
        }
    }
}

/// One override: grams for a Food or Product item, or doses for a Supplement.
enum Override {
    Item(Item, f64),
    Dose(String, f64),
}

fn parse_override(request: OverrideRequest) -> std::result::Result<Override, String> {
    match request {
        OverrideRequest {
            supplement_id: Some(id),
            doses: Some(doses),
            food_id: None,
            barcode: None,
            grams: None,
        } => {
            supplements::check_doses(doses)?;
            Ok(Override::Dose(id.trim().to_owned(), doses))
        }
        OverrideRequest {
            supplement_id: None,
            doses: None,
            food_id,
            barcode,
            grams: Some(grams),
        } => {
            let item = parse_item(food_id, barcode)?;
            check_grams(grams)?;
            Ok(Override::Item(item, grams))
        }
        _ => Err(
            "each override names a food_id or barcode with grams, or a supplement_id with doses"
                .into(),
        ),
    }
}

/// Parses either `portions` and `supplement_doses`, or a Meal name with optional overrides that
/// each name a different item.
pub(super) fn contents(
    requests: Option<Vec<PortionRequest>>,
    dose_requests: Option<Vec<DoseRequest>>,
    meal: Option<String>,
    overrides: Option<Vec<OverrideRequest>>,
) -> Result<Contents> {
    let has_items = requests.is_some() || dose_requests.is_some();
    match (has_items, meal) {
        (true, None) => {
            if overrides.is_some() {
                return Err(MealLogError::Validation(
                    "overrides change the grams or doses of a Meal's items; give them with meal, \
                     or put the amounts in portions and supplement_doses"
                        .into(),
                ));
            }
            let portions = portions(requests.unwrap_or_default())?;
            let doses = doses(dose_requests.unwrap_or_default())?;
            check_items(portions.len(), doses.len())?;
            Ok(Contents::Items { portions, doses })
        }
        (false, Some(meal)) => {
            let name =
                UniqueName::parse(&meal, "meal", "Meal").map_err(MealLogError::Validation)?;
            let mut items: Vec<(Item, f64)> = Vec::new();
            let mut doses: Vec<(String, f64)> = Vec::new();
            for request in overrides.unwrap_or_default() {
                let twice = match parse_override(request)
                    .map_err(|message| MealLogError::Validation(format!("overrides: {message}")))?
                {
                    Override::Item(item, grams) => {
                        let twice = items
                            .iter()
                            .any(|(other, _)| *other == item)
                            .then(|| item.describe());
                        items.push((item, grams));
                        twice
                    }
                    Override::Dose(id, count) => {
                        let twice = doses
                            .iter()
                            .any(|(other, _)| *other == id)
                            .then(|| format!("supplement_id {id}"));
                        doses.push((id, count));
                        twice
                    }
                };
                if let Some(item) = twice {
                    return Err(MealLogError::Validation(format!(
                        "overrides name {item} twice; give each item one amount"
                    )));
                }
            }
            Ok(Contents::Meal {
                name,
                overrides: items,
                dose_overrides: doses,
            })
        }
        _ => Err(MealLogError::Validation(
            "give either portions and supplement_doses, or the name of a saved Meal in meal".into(),
        )),
    }
}
