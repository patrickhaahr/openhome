use crate::services::nutrition::targets::Sex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgeBand {
    From18To24,
    From25To50,
    From51To70,
    From71,
}

impl AgeBand {
    #[must_use]
    pub const fn of(age: u32) -> Option<Self> {
        match age {
            0..=17 => None,
            18..=24 => Some(Self::From18To24),
            25..=50 => Some(Self::From25To50),
            51..=70 => Some(Self::From51To70),
            _ => Some(Self::From71),
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::From18To24 => "18-24",
            Self::From25To50 => "25-50",
            Self::From51To70 => "51-70",
            Self::From71 => "71+",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::From18To24 => 0,
            Self::From25To50 => 1,
            Self::From51To70 => 2,
            Self::From71 => 3,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ByBand {
    female: [f64; 4],
    male: [f64; 4],
}

impl ByBand {
    #[must_use]
    pub const fn get(&self, sex: Sex, band: AgeBand) -> f64 {
        match sex {
            Sex::Female => self.female[band.index()],
            Sex::Male => self.male[band.index()],
        }
    }
}

const fn by_band(female: [f64; 4], male: [f64; 4]) -> ByBand {
    ByBand { female, male }
}

const fn all(value: f64) -> ByBand {
    ByBand {
        female: [value; 4],
        male: [value; 4],
    }
}

pub const REFERENCE_ENERGY_MJ: ByBand = by_band([9.4, 9.0, 8.3, 8.2], [11.8, 11.3, 10.3, 10.1]);

#[derive(Clone, Copy, Debug)]
pub enum Recommended {
    PerDay(ByBand),
    PerMegajoule(f64),
}

impl Recommended {
    #[must_use]
    pub fn daily(self, sex: Sex, band: AgeBand) -> f64 {
        match self {
            Self::PerDay(values) => values.get(sex, band),
            Self::PerMegajoule(per_mj) => per_mj * REFERENCE_ENERGY_MJ.get(sex, band),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Measure {
    Parameter(&'static str),
    EpaDha,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Share {
    Total,
    Supplements,
    PreformedVitaminA,
    SupplementalFolate,
    NicotinicAcid,
    Nicotinamide,
}

#[derive(Clone, Copy, Debug)]
pub struct UpperLimit {
    pub share: Share,
    pub value: f64,
    pub source: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Vitamins,
    Minerals,
    FattyAcids,
}

#[derive(Debug)]
pub struct Reference {
    pub key: &'static str,
    pub name: &'static str,
    pub group: Group,
    pub measure: Measure,
    pub unit: &'static str,
    pub recommended: Recommended,
    pub adequate: bool,
    pub source: &'static str,
    pub upper_limits: &'static [UpperLimit],
}

const NNR_VITAMINS_RI: &str = "NNR 2023 Table 12";
const NNR_VITAMINS_AI: &str = "NNR 2023 Table 13";
const NNR_MINERALS_RI: &str = "NNR 2023 Table 14";
const NNR_MINERALS_AI: &str = "NNR 2023 Table 15";
const NNR_PER_MJ: &str = "NNR 2023 Table 12 per MJ, Table 8 energy at PAL 1.6";
const EFSA_MINERALS: &str = "EFSA Upper Limits overview v11 Table 1";
const EFSA_VITAMINS: &str = "EFSA Upper Limits overview v11 Table 3";

const fn total(value: f64, source: &'static str) -> UpperLimit {
    UpperLimit {
        share: Share::Total,
        value,
        source,
    }
}

const fn vitamin(
    key: &'static str,
    name: &'static str,
    unit: &'static str,
    recommended: Recommended,
    adequate: bool,
    upper_limits: &'static [UpperLimit],
) -> Reference {
    Reference {
        key,
        name,
        group: Group::Vitamins,
        measure: Measure::Parameter(key),
        unit,
        recommended,
        adequate,
        source: match recommended {
            Recommended::PerMegajoule(_) => NNR_PER_MJ,
            Recommended::PerDay(_) if adequate => NNR_VITAMINS_AI,
            Recommended::PerDay(_) => NNR_VITAMINS_RI,
        },
        upper_limits,
    }
}

const fn mineral(
    key: &'static str,
    name: &'static str,
    unit: &'static str,
    recommended: ByBand,
    adequate: bool,
    upper_limits: &'static [UpperLimit],
) -> Reference {
    Reference {
        key,
        name,
        group: Group::Minerals,
        measure: Measure::Parameter(key),
        unit,
        recommended: Recommended::PerDay(recommended),
        adequate,
        source: if adequate {
            NNR_MINERALS_AI
        } else {
            NNR_MINERALS_RI
        },
        upper_limits,
    }
}

use Recommended::{PerDay, PerMegajoule};

pub const REFERENCES: &[Reference] = &[
    vitamin(
        "vitamin_a",
        "Vitamin A",
        "µg RE",
        PerDay(by_band(
            [700.0, 700.0, 700.0, 650.0],
            [800.0, 800.0, 800.0, 750.0],
        )),
        false,
        &[UpperLimit {
            share: Share::PreformedVitaminA,
            value: 3000.0,
            source: EFSA_VITAMINS,
        }],
    ),
    vitamin(
        "vitamin_d",
        "Vitamin D",
        "µg",
        PerDay(by_band([10.0, 10.0, 10.0, 20.0], [10.0, 10.0, 10.0, 20.0])),
        false,
        &[total(100.0, EFSA_VITAMINS)],
    ),
    vitamin(
        "vitamin_e",
        "Vitamin E",
        "mg α-TE",
        PerDay(by_band([10.0, 10.0, 9.0, 9.0], [11.0, 11.0, 11.0, 11.0])),
        true,
        &[total(300.0, EFSA_VITAMINS)],
    ),
    vitamin(
        "vitamin_k",
        "Vitamin K",
        "µg",
        PerDay(by_band([65.0, 65.0, 60.0, 60.0], [75.0, 75.0, 70.0, 70.0])),
        true,
        &[],
    ),
    vitamin("vitamin_b1", "Thiamin", "mg", PerMegajoule(0.1), false, &[]),
    vitamin(
        "vitamin_b2",
        "Riboflavin",
        "mg",
        PerDay(all(1.6)),
        false,
        &[],
    ),
    vitamin(
        "niacin_equivalent",
        "Niacin",
        "mg NE",
        PerMegajoule(1.6),
        false,
        &[
            UpperLimit {
                share: Share::NicotinicAcid,
                value: 10.0,
                source: EFSA_VITAMINS,
            },
            UpperLimit {
                share: Share::Nicotinamide,
                value: 900.0,
                source: EFSA_VITAMINS,
            },
        ],
    ),
    vitamin(
        "vitamin_b6",
        "Vitamin B6",
        "mg",
        PerDay(by_band([1.6, 1.6, 1.6, 1.6], [1.8, 1.8, 1.8, 1.7])),
        false,
        &[total(12.0, EFSA_VITAMINS)],
    ),
    vitamin(
        "folate",
        "Folate",
        "µg",
        PerDay(all(330.0)),
        false,
        &[UpperLimit {
            share: Share::SupplementalFolate,
            value: 1000.0,
            source: EFSA_VITAMINS,
        }],
    ),
    vitamin(
        "vitamin_b12",
        "Vitamin B12",
        "µg",
        PerDay(all(4.0)),
        true,
        &[],
    ),
    vitamin("biotin", "Biotin", "µg", PerDay(all(40.0)), true, &[]),
    vitamin(
        "pantothenic_acid",
        "Pantothenic acid",
        "mg",
        PerDay(all(5.0)),
        true,
        &[],
    ),
    vitamin(
        "vitamin_c",
        "Vitamin C",
        "mg",
        PerDay(by_band([95.0; 4], [110.0; 4])),
        false,
        &[],
    ),
    vitamin("choline", "Choline", "mg", PerDay(all(400.0)), true, &[]),
    mineral(
        "calcium",
        "Calcium",
        "mg",
        by_band([1000.0, 950.0, 950.0, 950.0], [1000.0, 950.0, 950.0, 950.0]),
        false,
        &[total(2500.0, EFSA_MINERALS)],
    ),
    mineral(
        "phosphorus",
        "Phosphorus",
        "mg",
        by_band([550.0, 520.0, 520.0, 520.0], [550.0, 520.0, 520.0, 520.0]),
        true,
        &[],
    ),
    mineral(
        "magnesium",
        "Magnesium",
        "mg",
        by_band([300.0; 4], [350.0; 4]),
        true,
        &[UpperLimit {
            share: Share::Supplements,
            value: 250.0,
            source: EFSA_MINERALS,
        }],
    ),
    mineral("potassium", "Potassium", "mg", all(3500.0), true, &[]),
    mineral(
        "iron",
        "Iron",
        "mg",
        by_band([15.0, 15.0, 8.0, 7.0], [9.0; 4]),
        false,
        &[],
    ),
    mineral(
        "zinc",
        "Zinc",
        "mg",
        by_band([9.7, 9.7, 9.5, 9.3], [12.7, 12.7, 12.4, 12.1]),
        false,
        &[total(25.0, EFSA_MINERALS)],
    ),
    mineral(
        "copper",
        "Copper",
        "mg",
        all(0.9),
        false,
        &[total(5.0, EFSA_MINERALS)],
    ),
    mineral(
        "iodine",
        "Iodine",
        "µg",
        all(150.0),
        true,
        &[total(600.0, EFSA_MINERALS)],
    ),
    mineral(
        "selenium",
        "Selenium",
        "µg",
        by_band([75.0, 75.0, 75.0, 75.0], [90.0, 90.0, 90.0, 85.0]),
        true,
        &[total(255.0, EFSA_MINERALS)],
    ),
    mineral("manganese", "Manganese", "mg", all(3.0), true, &[]),
    mineral(
        "molybdenum",
        "Molybdenum",
        "µg",
        all(65.0),
        true,
        &[total(600.0, EFSA_MINERALS)],
    ),
    mineral(
        "fluoride",
        "Fluoride",
        "µg",
        by_band(
            [3200.0, 3200.0, 3100.0, 3000.0],
            [3800.0, 3700.0, 3700.0, 3500.0],
        ),
        true,
        &[],
    ),
    Reference {
        key: "epa_dha",
        name: "EPA+DHA",
        group: Group::FattyAcids,
        measure: Measure::EpaDha,
        unit: "mg",
        recommended: PerDay(all(250.0)),
        adequate: true,
        source: "EFSA 2010 Dietary Reference Values for fats",
        upper_limits: &[],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::nutrition::parameters;

    #[test]
    fn every_parameter_reference_is_in_its_vocabulary_unit() {
        let mismatched: Vec<&str> = REFERENCES
            .iter()
            .filter(|reference| match reference.measure {
                Measure::Parameter(key) => {
                    parameters::by_key(key).and_then(|parameter| parameter.unit.symbol())
                        != Some(reference.unit)
                }
                Measure::EpaDha => false,
            })
            .map(|reference| reference.key)
            .collect();
        assert_eq!(mismatched, Vec::<&str>::new());
    }
}
