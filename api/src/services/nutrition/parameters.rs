//! The nutrient parameter vocabulary.
//!
//! Every value in the Danish Food Composition Database is one parameter of one Food. `OpenHome`
//! names each DTU `ParameterID` with a stable key (`protein`, `vitamin_d`, `epa`) and a unit, so
//! Foods, and later Product labels, Supplement amounts and Portion snapshots, share one
//! vocabulary. DTU renumbers Foods between versions but keeps `ParameterIDs`, and the import
//! refuses a workbook whose unit for a known parameter differs from the unit listed here, so a
//! key never silently changes meaning.

/// What one parameter value measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Kilojoule,
    Kilocalorie,
    Gram,
    Milligram,
    Microgram,
    /// Vitamin A as retinol equivalents.
    MicrogramRetinolEquivalent,
    /// Vitamin E as alpha-tocopherol equivalents.
    MilligramAlphaTocopherolEquivalent,
    /// Niacin equivalents.
    MilligramNiacinEquivalent,
    /// Waste: the inedible share of the Food as bought, in percent.
    Percent,
    /// Density of the Food itself.
    GramPerMillilitre,
    /// Conversion factors and pH.
    Unitless,
}

impl Unit {
    /// Reads a unit as the DTU workbook's Parameter sheet writes it.
    #[must_use]
    pub fn from_dtu(text: &str) -> Option<Self> {
        Some(match text.trim() {
            "kJ/100g" | "kJ/100 g" => Self::Kilojoule,
            "kcal/100g" | "kcal/100 g" => Self::Kilocalorie,
            "g/100g" | "g/100 g" => Self::Gram,
            "mg/100g" | "mg/100 g" => Self::Milligram,
            "µg/100g" | "µg/100 g" => Self::Microgram,
            "RE (µg/100g)" | "RE (µg/100 g)" => Self::MicrogramRetinolEquivalent,
            "alfa-TE" => Self::MilligramAlphaTocopherolEquivalent,
            "NE" => Self::MilligramNiacinEquivalent,
            "%" => Self::Percent,
            "g/ml" => Self::GramPerMillilitre,
            "No Unit" => Self::Unitless,
            _ => return None,
        })
    }

    /// The unit symbol shown beside a value; `None` for unitless factors.
    #[must_use]
    pub const fn symbol(self) -> Option<&'static str> {
        match self {
            Self::Kilojoule => Some("kJ"),
            Self::Kilocalorie => Some("kcal"),
            Self::Gram => Some("g"),
            Self::Milligram => Some("mg"),
            Self::Microgram => Some("µg"),
            Self::MicrogramRetinolEquivalent => Some("µg RE"),
            Self::MilligramAlphaTocopherolEquivalent => Some("mg α-TE"),
            Self::MilligramNiacinEquivalent => Some("mg NE"),
            Self::Percent => Some("%"),
            Self::GramPerMillilitre => Some("g/ml"),
            Self::Unitless => None,
        }
    }

    /// Whether a value in this unit is an amount per 100 g edible part, which scales with grams
    /// eaten. Waste, density and factors describe the Food instead.
    #[must_use]
    pub const fn per_100g(self) -> bool {
        !matches!(
            self,
            Self::Percent | Self::GramPerMillilitre | Self::Unitless
        )
    }
}

/// One parameter of the vocabulary.
#[derive(Debug)]
pub struct Parameter {
    /// DTU's `ParameterID`.
    pub id: i64,
    pub key: &'static str,
    pub unit: Unit,
}

pub const ENERGY_KCAL: &str = "energy_kcal";
pub const ENERGY_KJ: &str = "energy_kj";
pub const PROTEIN: &str = "protein";
pub const FAT: &str = "fat";
pub const AVAILABLE_CARBOHYDRATE: &str = "available_carbohydrate";
pub const SUGARS: &str = "sugars";
pub const DIETARY_FIBRE: &str = "dietary_fibre";
pub const ALCOHOL: &str = "alcohol";
/// Inedible share of the Food as bought: edible grams = gross grams × (1 − waste / 100).
pub const WASTE: &str = "waste";
/// Grams per millilitre: grams = millilitres × density.
pub const DENSITY: &str = "density";

/// The vocabulary entry for a DTU `ParameterID`.
#[must_use]
pub fn by_id(id: i64) -> Option<&'static Parameter> {
    PARAMETERS.iter().find(|parameter| parameter.id == id)
}

/// The vocabulary entry for a key.
#[must_use]
pub fn by_key(key: &str) -> Option<&'static Parameter> {
    PARAMETERS.iter().find(|parameter| parameter.key == key)
}

const fn p(id: i64, key: &'static str, unit: Unit) -> Parameter {
    Parameter { id, key, unit }
}

use Unit::{
    Gram, GramPerMillilitre, Kilocalorie, Kilojoule, Microgram, MicrogramRetinolEquivalent,
    Milligram, MilligramAlphaTocopherolEquivalent, MilligramNiacinEquivalent, Percent, Unitless,
};

/// Every parameter of the Danish Food Composition Database 6.1 Parameter sheet, in its sort
/// order. Comments give DTU's English name.
pub const PARAMETERS: &[Parameter] = &[
    p(137, "energy_kj", Kilojoule),                          // Energy (kJ)
    p(316, "energy_labelling_kj", Kilojoule),                // Energy, labelling (kJ)
    p(356, "energy_kcal", Kilocalorie),                      // Energy (kcal)
    p(359, "energy_labelling_kcal", Kilocalorie),            // Energy, labelling (kcal)
    p(218, "protein", Gram),                                 // Protein
    p(421, "protein_from_amino_acids", Gram),                // Protein from Amino Acids
    p(317, "protein_labelling", Gram),                       // Protein, labeling
    p(170, "carbohydrate_by_difference", Gram),              // Carbohydrate by difference
    p(172, "available_carbohydrate", Gram),                  // Available carbohydrates
    p(318, "available_carbohydrate_labelling", Gram),        // Available carbohydrate, labelling
    p(168, "dietary_fibre", Gram),                           // Dietary fibre
    p(141, "fat", Gram),                                     // Fat
    p(19, "alcohol", Gram),                                  // Alcohol
    p(327, "salt_labelling", Gram),                          // Salt labelling
    p(33, "ash", Gram),                                      // Ash
    p(265, "dry_matter", Gram),                              // Dry matter
    p(268, "water", Gram),                                   // Water
    p(12, "vitamin_a", MicrogramRetinolEquivalent),          // Vitamin A
    p(225, "retinol", Microgram),                            // Retinol
    p(303, "beta_carotene", Microgram),                      // beta-Carotene
    p(440, "cis_beta_carotene", Microgram),                  // cis-beta-Caroten
    p(126, "vitamin_d", Microgram),                          // Vitamin D
    p(128, "vitamin_d3", Microgram),                         // Vitamin D3
    p(127, "vitamin_d2", Microgram),                         // Vitamin D2
    p(11, "hydroxy_25_vitamin_d3", Microgram),               // 25-hydroxy vitamin D3
    p(354, "hydroxy_25_vitamin_d2", Microgram),              // 25-hydroxy Vitamin D2
    p(135, "vitamin_e", MilligramAlphaTocopherolEquivalent), // Vitamin E
    p(276, "alpha_tocopherol", Milligram),                   // alpha-Tocopherol
    p(279, "beta_tocopherol", Milligram),                    // beta-Tocopherol
    p(286, "gamma_tocopherol", Milligram),                   // gamma-Tocopherol
    p(282, "delta_tocopherol", Milligram),                   // delta-Tocopherol
    p(277, "alpha_tocotrienol", Milligram),                  // alpha-Tocotrienol
    p(442, "vitamin_k", Microgram),                          // Vitamin K
    p(164, "vitamin_k1", Microgram),                         // Vitamin K1
    p(441, "vitamin_k2", Microgram),                         // Vitamin K2
    p(433, "menaquinone_4", Microgram),                      // Menaquinone 4
    p(434, "menaquinone_5", Microgram),                      // Menaquinone 5
    p(435, "menaquinone_6", Microgram),                      // Menaquinone 6
    p(436, "menaquinone_7", Microgram),                      // Menaquinone 7
    p(437, "menaquinone_8", Microgram),                      // Menaquinone 8
    p(438, "menaquinone_9", Microgram),                      // Menaquinone 9
    p(439, "menaquinone_10", Microgram),                     // Menaquinone 10
    p(37, "vitamin_b1", Milligram),                          // Thiamin (Vitamin B1)
    p(36, "thiamine", Milligram),                            // Thiamine
    p(39, "vitamin_b2", Milligram),                          // Riboflavin (Vitamin B2)
    p(203, "niacin_equivalent", MilligramNiacinEquivalent),  // Niacin equivalent
    p(294, "niacin", Milligram),                             // Niacin
    p(40, "vitamin_b6", Milligram),                          // Vitamin B6
    p(210, "pantothenic_acid", Milligram),                   // Pantothenic acid
    p(42, "biotin", Microgram),                              // Biotin
    p(143, "folate", Microgram),                             // Folate
    p(145, "folate_free", Microgram),                        // Folate, free
    p(38, "vitamin_b12", Microgram),                         // Vitamin B12
    p(47, "vitamin_c", Milligram),                           // Vitamin C
    p(175, "ascorbic_acid", Milligram),                      // Ascorbic acid
    p(177, "dehydroascorbic_acid", Milligram),               // Dehydroascorbic acid
    p(201, "sodium", Milligram),                             // Sodium
    p(165, "potassium", Milligram),                          // Potassium
    p(108, "calcium", Milligram),                            // Calcium
    p(184, "magnesium", Milligram),                          // Magnesium
    p(227, "rubidium", Microgram),                           // Rubidium
    p(162, "iron", Milligram),                               // Iron
    p(166, "copper", Milligram),                             // Copper
    p(274, "zinc", Milligram),                               // Zinc
    p(187, "manganese", Milligram),                          // Manganese
    p(117, "chromium", Microgram),                           // Chromium
    p(190, "molybdenum", Microgram),                         // Molybdenum
    p(120, "cobalt", Microgram),                             // Cobalt
    p(230, "selenium", Microgram),                           // Selenium
    p(20, "aluminium", Milligram),                           // Aluminum
    p(234, "silicon", Milligram),                            // Silicon
    p(44, "boron", Microgram),                               // Boron
    p(214, "phosphorus", Milligram),                         // Phosphorus
    p(142, "fluoride", Microgram),                           // Fluoride
    p(114, "chloride", Milligram),                           // Chloride
    p(46, "bromide", Microgram),                             // Bromide
    p(163, "iodine", Microgram),                             // Iodine
    p(253, "sulfur", Milligram),                             // Sulfur
    p(204, "nickel", Microgram),                             // Nickel
    p(174, "mercury", Microgram),                            // Mercury
    p(32, "arsenic", Microgram),                             // Arsenic
    p(107, "cadmium", Microgram),                            // Cadmium
    p(43, "lead", Microgram),                                // Lead
    p(260, "tin", Microgram),                                // Tin
    p(443, "sum_minerals", Milligram),                       // Sum minerals
    p(146, "fructose", Gram),                                // Fructose
    p(148, "galactose", Gram),                               // Galactose
    p(149, "glucose", Gram),                                 // Glucose
    p(191, "sum_monosaccharides", Gram),                     // Sum monosaccharides
    p(179, "lactose", Gram),                                 // Lactose
    p(185, "maltose", Gram),                                 // Maltose
    p(228, "sucrose", Gram),                                 // Sucrose
    p(18, "sum_disaccharides", Gram),                        // Sum disaccharides
    p(186, "maltotriose", Gram),                             // Maltotriose
    p(222, "raffinose", Gram),                               // Raffinose
    p(245, "sugars", Gram),                                  // Sum sugars
    p(417, "added_sugars", Gram),                            // Added Sugar
    p(418, "free_sugars", Gram),                             // Free Sugars
    p(237, "sorbitol", Gram),                                // Sorbitol
    p(188, "mannitol", Gram),                                // Mannitol
    p(160, "inositol", Gram),                                // Inositol
    p(343, "maltitol", Gram),                                // Maltitol
    p(344, "isomalt", Gram),                                 // Isomalt
    p(445, "xylitol", Gram),                                 // Xylitol
    p(446, "lactitol", Gram),                                // Lactitol
    p(447, "glycerol", Gram),                                // Glycerol
    p(244, "sum_sugar_alcohols", Gram),                      // Sum sugar alcohols
    p(243, "starch", Gram),                                  // Starch/Glycogen
    p(112, "cellulose", Gram),                               // Cellulose
    p(181, "lignin", Gram),                                  // Lignin
    p(414, "insoluble_dietary_fibre", Gram),                 // Insoluble dietary fibers
    p(415, "soluble_dietary_fibre_high_molecular_weight", Gram), // High molecular weight soluble dietary fibre
    p(416, "soluble_dietary_fibre_low_molecular_weight", Gram), // Low molecular weight soluble dietary fibre
    p(123, "crude_fibre", Gram),                                // Crude fibre
    p(202, "neutral_detergent_fibre", Gram),                    // Neutral detergent fibre
    p(178, "l_lactic_acid", Gram),                              // L-lactic acid
    p(125, "d_lactic_acid", Gram),                              // D-lactic acid
    p(192, "lactic_acid", Gram),                                // Lactic acid
    p(119, "citric_acid", Gram),                                // Citric acid
    p(209, "oxalic_acid", Gram),                                // Oxalic acid
    p(136, "acetic_acid", Gram),                                // Acetic acid
    p(217, "propionic_acid", Gram),                             // Propionic acid
    p(41, "benzoic_acid", Milligram),                           // Benzoic acid
    p(208, "sum_organic_acids", Gram),                          // Sum organic acids
    p(103, "fa_c4_0", Gram),                                    // C4:0
    p(104, "fa_c6_0", Gram),                                    // C6:0
    p(105, "fa_c8_0", Gram),                                    // C8:0
    p(48, "fa_c10_0", Gram),                                    // C10:0
    p(49, "fa_c12_0", Gram),                                    // C12:0
    p(50, "fa_c13_0", Gram),                                    // C13:0
    p(51, "fa_c14_0", Gram),                                    // C14:0
    p(56, "fa_c15_0", Gram),                                    // C15:0
    p(58, "fa_c16_0", Gram),                                    // C16:0
    p(63, "fa_c17_0", Gram),                                    // C17:0
    p(65, "fa_c18_0", Gram),                                    // C18:0
    p(77, "fa_c20_0", Gram),                                    // C20:0
    p(409, "fa_c21_0", Gram),                                   // C21:0
    p(89, "fa_c22_0", Gram),                                    // C22:0
    p(413, "fa_c23_0", Gram),                                   // C23:0
    p(100, "fa_c24_0", Gram),                                   // C24:0
    p(401, "fa_c12_1_n1", Gram),                                // C12:1,n-1
    p(52, "fa_c14_1_n5", Gram),                                 // C14:1,n-5
    p(57, "fa_c15_1_n5", Gram),                                 // C15:1,n-5
    p(59, "fa_c16_1_n7", Gram),                                 // C16:1,n-7
    p(64, "fa_c17_1_n7", Gram),                                 // C17:1,n-7
    p(66, "fa_c18_1_n7", Gram),                                 // C18:1,n-7
    p(67, "fa_c18_1_n9", Gram),                                 // C18:1,n-9
    p(425, "fa_c18_1_n12", Gram),                               // C18:1,n-12
    p(347, "fa_c20_1_n9", Gram),                                // C20:1,n-9
    p(78, "fa_c20_1_n11", Gram),                                // C20:1,n-11
    p(424, "fa_c20_1_n15", Gram),                               // C20:1,n-15
    p(92, "fa_c22_1_n9", Gram),                                 // C22:1,n-9
    p(90, "fa_c22_1_n11", Gram),                                // C22:1,n-11
    p(101, "fa_c24_1_n9", Gram),                                // C24:1,n-9
    p(53, "fa_c14_1_n5_trans", Gram),                           // C14:1,n-5,trans
    p(60, "fa_c16_1_n7_trans", Gram),                           // C16:1,n-7,trans
    p(70, "fa_c18_1_trans", Gram),                              // C18:1,trans
    p(79, "fa_c20_1_trans", Gram),                              // C20:1,trans
    p(93, "fa_c22_1_trans", Gram),                              // C22:1,trans
    p(71, "linoleic_acid", Gram),                               // C18:2,n-6
    p(72, "fa_c18_2_conjugated", Gram),                         // C18:2,conjugated
    p(74, "ala", Gram),                                         // C18:3,n-3
    p(75, "fa_c18_3_n6", Gram),                                 // C18:3,n-6
    p(76, "fa_c18_4_n3", Gram),                                 // C18:4,n-3
    p(80, "fa_c20_2_n6", Gram),                                 // C20:2,n-6
    p(82, "fa_c20_3_n3", Gram),                                 // C20:3,n-3
    p(85, "fa_c20_3_n6", Gram),                                 // C20:3,n-6
    p(349, "fa_c20_4_n3", Gram),                                // C20:4,n-3
    p(86, "fa_c20_4_n6", Gram),                                 // C20:4,n-6
    p(87, "epa", Gram),                                         // C20:5,n-3
    p(428, "fa_c21_5_n3", Gram),                                // C21:5,n-3
    p(410, "fa_c22_2_n6", Gram),                                // C22:2,n-6
    p(426, "fa_c22_3_n3", Gram),                                // C22:3,n-3
    p(411, "fa_c22_4_n6", Gram),                                // C22:4,n-6
    p(98, "dpa", Gram),                                         // C22:5,n-3
    p(412, "fa_c22_5_n6", Gram),                                // C22:5,n-6
    p(99, "dha", Gram),                                         // C22:6,n-3
    p(73, "fa_c18_2_trans", Gram),                              // C18:2,trans
    p(355, "fa_c18_3_trans", Gram),                             // C18:3,trans
    p(24, "other_fatty_acids", Gram),                           // Other fatty acids
    p(248, "saturated_fat", Gram),                              // Sum saturated fatty acids
    p(247, "monounsaturated_fat", Gram),                        // Sum monounsaturated fatty acids
    p(251, "polyunsaturated_fat", Gram),                        // Sum polyunsaturated fatty acids
    p(261, "trans_fat", Gram),                                  // Sum trans fatty acids
    p(427, "sum_fatty_acids_below_detection_limit", Gram), // Sum fatty acids below the detection limit
    p(340, "sum_fatty_acids", Gram),                       // Sum fatty acids
    p(249, "omega_3_fatty_acids", Gram),                   // Sum n-3 fatty acids
    p(250, "omega_6_fatty_acids", Gram),                   // Sum n-6 fatty acids
    p(115, "cholesterol", Milligram),                      // Cholesterol
    p(300, "nitrogen", Gram),                              // Nitrogen
    p(161, "isoleucine", Milligram),                       // Isoleucine
    p(180, "leucine", Milligram),                          // Leucine
    p(183, "lysine", Milligram),                           // Lysine
    p(189, "methionine", Milligram),                       // Methionine
    p(124, "cystine", Milligram),                          // Cystine
    p(211, "phenylalanine", Milligram),                    // Phenylalanine
    p(264, "tyrosine", Milligram),                         // Tyrosine
    p(258, "threonine", Milligram),                        // Threonine
    p(262, "tryptophan", Milligram),                       // Tryptophan
    p(266, "valine", Milligram),                           // Valine
    p(31, "arginine", Milligram),                          // Arginine
    p(159, "histidine", Milligram),                        // Histidine
    p(17, "alanine", Milligram),                           // Alanine
    p(34, "aspartic_acid", Milligram),                     // Aspartic acid
    p(150, "glutamic_acid", Milligram),                    // Glutamic acid
    p(153, "glycine", Milligram),                          // Glycine
    p(216, "proline", Milligram),                          // Proline
    p(231, "serine", Milligram),                           // Serine
    p(400, "hydroxyproline", Milligram),                   // Hydroxyproline
    p(423, "ornithine", Milligram),                        // Ornithine
    p(430, "sum_essential_amino_acids", Milligram),        // Sum essential amino acids
    p(431, "sum_non_essential_amino_acids", Milligram),    // Sum non-essential amino acids
    p(158, "histamine", Milligram),                        // Histamine
    p(232, "serotonin", Milligram),                        // Serotonin
    p(116, "choline", Milligram),                          // Choline
    p(263, "tyramine", Milligram),                         // Tyramine
    p(212, "phenylethylamine", Milligram),                 // Phenylethylamine
    p(221, "putrescine", Milligram),                       // Putrescine
    p(106, "cadaverine", Milligram),                       // Cadaverine
    p(239, "spermine", Milligram),                         // Spermine
    p(238, "spermidine", Milligram),                       // Spermidine
    p(255, "taurine", Milligram),                          // Taurine
    p(422, "sum_biogenic_amines", Milligram),              // Sum biogenic amines
    p(121, "caffeine", Milligram),                         // Caffeine
    p(444, "residual", Gram),                              // Residual
    p(252, "waste", Percent),                              // Waste
    p(219, "nitrogen_conversion_factor", Unitless),        // Nitrogen conversion factor (Jones)
    p(140, "fatty_acid_conversion_factor", Unitless),      // Fatty acid conversion factor
    p(363, "density", GramPerMillilitre),                  // Density
    p(432, "ph", Unitless),                                // pH
];

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn vocabulary_ids_and_keys_are_unique() {
        let ids: HashSet<i64> = PARAMETERS.iter().map(|parameter| parameter.id).collect();
        let keys: HashSet<&str> = PARAMETERS.iter().map(|parameter| parameter.key).collect();
        assert_eq!(
            (ids.len(), keys.len()),
            (PARAMETERS.len(), PARAMETERS.len())
        );
    }
}
