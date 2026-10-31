//! Intake: how much of each parameter was eaten and taken, split into the food share (Portions)
//! and the supplement share (Supplement Doses), and the named compounds taken.
//!
//! Totals add both shares, so energy and macronutrients a Supplement's label states count toward
//! what was eaten. The shares stay apart because some EFSA Upper Limits apply only to the
//! supplement share, and they are kept per chemical form where a Supplement names one
//! (`supplements::FORMS`).
//!
//! [`Intake`] and [`Compounds`] add up, so a range sums each day's values, or all its Portions and
//! doses at once, and divides afterwards; never average daily shares.

use std::collections::BTreeMap;
use std::iter::Sum;
use std::ops::Add;

use serde::Serialize;

use super::meal_logs::{StoredPortion, Total, round};
use super::supplements::{self, Compound, StoredDose};

/// One parameter's intake.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Intake {
    food: Option<f64>,
    unknown_portions: usize,
    supplements: Option<f64>,
    forms: BTreeMap<String, f64>,
}

/// The supplement share of one chemical form of a parameter.
#[derive(Debug, Serialize)]
pub struct FormAmount {
    pub form: String,
    pub value: f64,
}

fn add(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (None, None) => None,
        (left, right) => Some(left.unwrap_or(0.0) + right.unwrap_or(0.0)),
    }
}

impl Intake {
    /// `key` over `portions` and `doses`. A Portion without a value is unknown and counted; a
    /// Supplement whose label does not list `key` supplies none of it.
    pub(super) fn of(key: &str, portions: &[&StoredPortion], doses: &[&StoredDose]) -> Self {
        let mut intake = Self::default();
        for portion in portions {
            match portion.amount(key) {
                Some(amount) => *intake.food.get_or_insert(0.0) += amount,
                None => intake.unknown_portions += 1,
            }
        }
        for dose in doses {
            for amount in dose.amounts.iter().filter(|amount| amount.key == key) {
                let taken = amount.value * dose.doses;
                *intake.supplements.get_or_insert(0.0) += taken;
                if let Some(form) = &amount.form {
                    *intake.forms.entry(form.clone()).or_default() += taken;
                }
            }
        }
        intake
    }

    /// From Portions; `None` when no Portion has a value.
    #[must_use]
    pub fn food(&self) -> Option<f64> {
        self.food
    }

    /// From Supplement Doses; `None` when no dose lists the parameter.
    #[must_use]
    pub fn supplements(&self) -> Option<f64> {
        self.supplements
    }

    /// Portions without a value for the parameter.
    #[must_use]
    pub fn unknown_portions(&self) -> usize {
        self.unknown_portions
    }

    /// Both shares; `None` when neither is known.
    #[must_use]
    pub fn total(&self) -> Option<f64> {
        add(self.food, self.supplements)
    }

    /// The supplement share of `form`, e.g. `folic_acid` of `folate`.
    #[must_use]
    pub fn supplement_form(&self, form: &str) -> Option<f64> {
        self.forms.get(form).copied()
    }

    /// The supplement share of each form taken, by form.
    #[must_use]
    pub fn forms(&self) -> Vec<FormAmount> {
        self.forms
            .iter()
            .map(|(form, value)| FormAmount {
                form: form.clone(),
                value: round(*value),
            })
            .collect()
    }

    pub(super) fn into_total(self, unit: Option<String>) -> Total {
        Total {
            value: self.total().map(round),
            unit,
            unknown_portions: self.unknown_portions,
            food: self.food.map(round),
            supplements: self.supplements.map(round),
        }
    }
}

impl Add for Intake {
    type Output = Self;

    fn add(mut self, other: Self) -> Self {
        for (form, value) in other.forms {
            *self.forms.entry(form).or_default() += value;
        }
        Self {
            food: add(self.food, other.food),
            unknown_portions: self.unknown_portions + other.unknown_portions,
            supplements: add(self.supplements, other.supplements),
            forms: self.forms,
        }
    }
}

impl Sum for Intake {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

/// Named compounds taken, by name case-insensitively, in the order first taken. A compound taken
/// in several mass units is summed in the unit it was first taken in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Compounds(Vec<Compound>);

impl Compounds {
    pub(super) fn of(doses: &[&StoredDose]) -> Self {
        doses
            .iter()
            .flat_map(|dose| {
                dose.compounds.iter().map(|compound| {
                    Self(vec![Compound {
                        value: compound.value * dose.doses,
                        ..compound.clone()
                    }])
                })
            })
            .sum()
    }

    /// Each compound with its amount taken.
    #[must_use]
    pub fn into_amounts(self) -> Vec<Compound> {
        self.0
            .into_iter()
            .map(|compound| Compound {
                value: round(compound.value),
                ..compound
            })
            .collect()
    }
}

impl Add for Compounds {
    type Output = Self;

    fn add(mut self, other: Self) -> Self {
        for compound in other.0 {
            let same = self
                .0
                .iter_mut()
                .find(|known| known.name.to_lowercase() == compound.name.to_lowercase());
            match same {
                Some(known) => {
                    known.value +=
                        supplements::convert_mass(compound.value, &compound.unit, &known.unit)
                            .unwrap_or(compound.value);
                }
                None => self.0.push(compound),
            }
        }
        self
    }
}

impl Sum for Compounds {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}
