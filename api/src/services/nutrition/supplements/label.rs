use super::{
    Amount, AmountRequest, Compound, CompoundRequest, KJ_PER_KCAL, MAX_GRAMS_PER_DOSE,
    MAX_KCAL_PER_DOSE, MAX_LABEL_ITEMS, Result, forms, validation,
};
use crate::services::nutrition::meals::UniqueName;
use crate::services::nutrition::open_food_facts::significant;
use crate::services::nutrition::parameters::{self, Unit};
use std::collections::HashSet;

/// A mass unit's symbol and how many of it one gram is.
fn mass_unit(text: &str) -> Option<(&'static str, f64)> {
    match text {
        "g" => Some(("g", 1.0)),
        "mg" => Some(("mg", 1_000.0)),
        // The micro sign, the Greek mu and ASCII spellings.
        "µg" | "μg" | "ug" | "mcg" => Some(("µg", 1_000_000.0)),
        _ => None,
    }
}

pub(super) fn amount(request: &AmountRequest) -> std::result::Result<Amount, String> {
    let key = request.key.trim();
    let parameter = parameters::by_key(key)
        .filter(|parameter| parameter.unit.per_100g())
        .ok_or_else(|| {
            format!(
                "amounts: {key} is not a Food database parameter key; give vitamins, minerals, \
                 fatty acids, energy and macronutrients by their key (vitamin_d, zinc, epa, \
                 energy_kcal, protein) and anything else in compounds"
            )
        })?;
    let symbol = parameter.unit.symbol().unwrap_or_default();
    let per_gram = parameter.unit.per_gram();
    let unit = request.unit.trim();
    let value = if unit == symbol {
        request.value
    } else {
        match (per_gram, mass_unit(unit)) {
            (Some(to), Some((_, from))) => request.value * to / from,
            (Some(_), None) => {
                return Err(format!("amounts.{key}: unit must be {symbol}, g, mg or µg"));
            }
            (None, _) => return Err(format!("amounts.{key}: unit must be {symbol}")),
        }
    };
    let limit = match parameter.unit {
        Unit::Kilocalorie => MAX_KCAL_PER_DOSE,
        Unit::Kilojoule => MAX_KCAL_PER_DOSE * KJ_PER_KCAL,
        _ => per_gram.map_or(f64::MAX, |per_gram| MAX_GRAMS_PER_DOSE * per_gram),
    };
    if !value.is_finite() || !(0.0..=limit).contains(&value) {
        return Err(format!(
            "amounts.{key} must be between 0 and {limit} {symbol} per dose"
        ));
    }
    let form = match (forms(key), request.form.as_deref().map(str::trim)) {
        (None, None) => None,
        (None, Some(_)) => {
            return Err(format!(
                "amounts.{key} has no forms to tell apart; leave form out"
            ));
        }
        (Some(forms), Some(form)) if forms.contains(&form) => Some(form.to_owned()),
        (Some(forms), _) => {
            return Err(format!(
                "amounts.{key} needs form, one of {}: Upper Limits differ by form, so read it \
                 from the label's ingredients (folic acid, retinyl acetate, nicotinamide)",
                forms.join(", ")
            ));
        }
    };
    Ok(Amount {
        key: parameter.key.to_owned(),
        form,
        value: significant(value),
        unit: symbol.to_owned(),
    })
}

/// The parameter key a compound name would be written as, e.g. `Vitamin D` as `vitamin_d`.
fn as_key(name: &str) -> String {
    name.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

pub(super) fn compound(request: &CompoundRequest) -> std::result::Result<Compound, String> {
    let name = UniqueName::parse(&request.name, "compounds.name", "compound")?;
    if let Some(parameter) = parameters::by_key(&as_key(&name.key)) {
        return Err(format!(
            "compounds: {} is the Food database parameter {}; give it in amounts",
            name.name, parameter.key
        ));
    }
    let Some((unit, per_gram)) = mass_unit(request.unit.trim()) else {
        return Err(format!("compounds.{}: unit must be g, mg or µg", name.name));
    };
    let limit = MAX_GRAMS_PER_DOSE * per_gram;
    if !request.value.is_finite() || !(0.0..=limit).contains(&request.value) {
        return Err(format!(
            "compounds.{} must be between 0 and {limit} {unit} per dose",
            name.name
        ));
    }
    Ok(Compound {
        name: name.name,
        value: significant(request.value),
        unit: unit.to_owned(),
    })
}

/// Completes energy in kcal and kJ from each other, as Product labels are.
fn complete_energy(mut amounts: Vec<Amount>) -> Vec<Amount> {
    let find = |amounts: &[Amount], key: &str| {
        amounts
            .iter()
            .find(|amount| amount.key == key)
            .map(|amount| amount.value)
    };
    let kcal = find(&amounts, parameters::ENERGY_KCAL);
    let kj = find(&amounts, parameters::ENERGY_KJ);
    let completed = match (kcal, kj) {
        (Some(kcal), None) => Some((parameters::ENERGY_KJ, "kJ", kcal * KJ_PER_KCAL)),
        (None, Some(kj)) => Some((parameters::ENERGY_KCAL, "kcal", kj / KJ_PER_KCAL)),
        _ => None,
    };
    if let Some((key, unit, value)) = completed {
        amounts.push(Amount {
            key: key.to_owned(),
            form: None,
            value: significant(value),
            unit: unit.to_owned(),
        });
    }
    amounts
}

pub(super) fn amounts(requests: &[AmountRequest]) -> Result<Vec<Amount>> {
    let amounts = requests
        .iter()
        .map(amount)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(validation)?;
    let mut seen = HashSet::new();
    for amount in &amounts {
        if !seen.insert((&amount.key, &amount.form)) {
            let form = amount
                .form
                .as_ref()
                .map(|form| format!(" as {form}"))
                .unwrap_or_default();
            return Err(validation(format!(
                "amounts list {}{form} twice; give each parameter once per dose",
                amount.key
            )));
        }
    }
    Ok(complete_energy(amounts))
}

pub(super) fn compounds(requests: &[CompoundRequest]) -> Result<Vec<Compound>> {
    let compounds = requests
        .iter()
        .map(compound)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(validation)?;
    let mut seen = HashSet::new();
    for compound in &compounds {
        if !seen.insert(compound.name.to_lowercase()) {
            return Err(validation(format!(
                "compounds list {} twice; give each compound once per dose",
                compound.name
            )));
        }
    }
    Ok(compounds)
}

/// A Supplement lists 1-[`MAX_LABEL_ITEMS`] amounts and compounds together.
pub(super) fn check_label(amounts: &[Amount], compounds: &[Compound]) -> Result<()> {
    let items = amounts.len() + compounds.len();
    if items == 0 || items > MAX_LABEL_ITEMS {
        return Err(validation(format!(
            "a Supplement lists 1-{MAX_LABEL_ITEMS} amounts and compounds per dose together"
        )));
    }
    Ok(())
}

/// Converts `value` of a compound from one mass unit to another; `None` for other units.
pub(in crate::services::nutrition) fn convert_mass(
    value: f64,
    from: &str,
    to: &str,
) -> Option<f64> {
    let (_, from) = mass_unit(from)?;
    let (_, to) = mass_unit(to)?;
    Some(value * to / from)
}
