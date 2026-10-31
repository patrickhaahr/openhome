//! The reference values the micronutrient panel compares intake with, transcribed by hand
//! because neither source publishes machine-readable tables. Each value names its table so it can
//! be audited against the source.
//!
//! - **NNR 2023**: Blomhoff R, Andersen R, Arnesen EK, et al. *Nordic Nutrition Recommendations
//!   2023*. Nordic Council of Ministers, Nord 2023:003, doi:10.6027/nord2023-003, with the
//!   corrections of 25 September 2023 applied (selenium for females 18–24 years is 75 µg, not
//!   80). Recommended intakes (RI) come from Table 12 (vitamins, p. 68) and Table 14 (minerals,
//!   p. 70); adequate intakes (AI) from Table 13 (vitamins, p. 69) and Table 15 (minerals, p. 71).
//!   Pages are those of the PDF report. The adult age bands are the report's own: 18–24, 25–50,
//!   51–70 and over 70 years.
//! - **Per-MJ references**: thiamin (0.1 mg/MJ) and niacin (1.6 mg NE/MJ) are converted with the
//!   band's reference energy intake at PAL 1.6 from Table 8 (p. 57), the activity level the
//!   report itself uses when it states these RIs per day (thiamin p. 136: 0.9 mg females, 1.1 mg
//!   males; niacin p. 140: 14 and 18 mg NE, both for 25–50 years).
//! - **EFSA Upper Limits**: EFSA, *Overview on Tolerable Upper Intake Levels as derived by the
//!   Scientific Committee on Food (SCF) and the EFSA NDA Panel*, Version 11 (August 2025),
//!   Table 1 (minerals, p. 2) and Table 3 (vitamins, p. 4), adult column. Iron, manganese and
//!   fluoride have only "safe levels of intake" (Table 2), which EFSA says cannot be used to
//!   judge risk, so they carry no Upper Limit here.
//! - **EPA+DHA**: the adequate intake of 250 mg/day for adults from EFSA NDA Panel (2010),
//!   *Scientific Opinion on Dietary Reference Values for fats*, EFSA Journal 8(3):1461. NNR 2023
//!   sets no EPA+DHA intake, and EFSA's overview (Table 4) sets no Upper Limit for it.
//!
//! Every amount is in the unit of the Food database parameter it is compared with, so copper is
//! in mg and molybdenum and fluoride in µg although NNR writes them in µg and mg; EPA+DHA, which
//! the database holds in grams, is in mg.
//!
//! Left out: sodium (NNR 2023 gives an AI of 1.5 g, p. 161, but recommends limiting intake to
//! 2.3 g, Table 16, and EFSA sets no Upper Limit, so "reaching it is good" would mislead), boron
//! (an EFSA Upper Limit of 10 mg but no NNR reference), and chromium, which neither sets.

use crate::services::nutrition::targets::Sex;

/// An NNR 2023 adult age band, by whole years of age on the day evaluated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgeBand {
    From18To24,
    From25To50,
    From51To70,
    From71,
}

impl AgeBand {
    /// The band of `age`; `None` below 18, which the adult references do not cover.
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

/// One value per sex for each band, in the order 18–24, 25–50, 51–70, 71+.
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

/// NNR 2023 Table 8 (p. 57): reference energy intake at PAL 1.6 in MJ/day.
pub const REFERENCE_ENERGY_MJ: ByBand = by_band([9.4, 9.0, 8.3, 8.2], [11.8, 11.3, 10.3, 10.1]);

/// How a Recommended Intake is stated.
#[derive(Clone, Copy, Debug)]
pub enum Recommended {
    PerDay(ByBand),
    /// Per MJ of the band's reference energy intake.
    PerMegajoule(f64),
}

impl Recommended {
    /// The daily amount for `sex` in `band`.
    #[must_use]
    pub fn daily(self, sex: Sex, band: AgeBand) -> f64 {
        match self {
            Self::PerDay(values) => values.get(sex, band),
            Self::PerMegajoule(per_mj) => per_mj * REFERENCE_ENERGY_MJ.get(sex, band),
        }
    }
}

/// What a nutrient's amount is read from.
#[derive(Clone, Copy, Debug)]
pub enum Measure {
    /// One Food database parameter, in its own unit.
    Parameter(&'static str),
    /// EPA plus DHA, converted from grams to mg.
    EpaDha,
}

/// The share of intake an Upper Limit applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Share {
    /// Food and supplements together.
    Total,
    /// Supplements only: EFSA's magnesium limit covers readily dissociable salts and oxide in
    /// supplements, water and fortified foods, not magnesium naturally in food.
    Supplements,
    /// Retinol and retinyl esters from food and supplements, not provitamin A carotenoids.
    PreformedVitaminA,
    /// Folic acid and L-methylfolate salts in supplements, not folate naturally in food.
    SupplementalFolate,
    /// Nicotinic acid in supplements.
    NicotinicAcid,
    /// Nicotinamide in supplements.
    Nicotinamide,
}

/// An EFSA Tolerable Upper Intake Level for adults, in the nutrient's unit.
#[derive(Clone, Copy, Debug)]
pub struct UpperLimit {
    pub share: Share,
    pub value: f64,
    pub source: &'static str,
}

/// Which list the panel shows a nutrient in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Vitamins,
    Minerals,
    FattyAcids,
}

/// One nutrient of the panel with its references.
#[derive(Debug)]
pub struct Reference {
    /// The Food database parameter key, or `epa_dha`.
    pub key: &'static str,
    pub name: &'static str,
    pub group: Group,
    pub measure: Measure,
    pub unit: &'static str,
    pub recommended: Recommended,
    /// An adequate intake (AI) rather than a recommended intake (RI).
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

/// The panel's nutrients in the order NNR 2023 lists them. Columns are females then males,
/// 18–24, 25–50, 51–70 and 71+ years.
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
        // 20 µg over 70, and for anyone with little or no sun exposure (footnote 3).
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
    // Females 51–70 who still menstruate should use 15 mg (footnote 7).
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
    // NNR: 900 µg.
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
    // EFSA: 0.6 mg.
    mineral(
        "molybdenum",
        "Molybdenum",
        "µg",
        all(65.0),
        true,
        &[total(600.0, EFSA_MINERALS)],
    ),
    // NNR: 3.2, 3.2, 3.1, 3.0 mg (females) and 3.8, 3.7, 3.7, 3.5 mg (males).
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
