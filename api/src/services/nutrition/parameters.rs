mod table;
pub use table::PARAMETERS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Kilojoule,
    Kilocalorie,
    Gram,
    Milligram,
    Microgram,
    MicrogramRetinolEquivalent,
    MilligramAlphaTocopherolEquivalent,
    MilligramNiacinEquivalent,
    Percent,
    GramPerMillilitre,
    Unitless,
}

impl Unit {
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

    #[must_use]
    pub const fn per_gram(self) -> Option<f64> {
        match self {
            Self::Gram => Some(1.0),
            Self::Milligram
            | Self::MilligramAlphaTocopherolEquivalent
            | Self::MilligramNiacinEquivalent => Some(1_000.0),
            Self::Microgram | Self::MicrogramRetinolEquivalent => Some(1_000_000.0),
            _ => None,
        }
    }

    #[must_use]
    pub const fn per_100g(self) -> bool {
        !matches!(
            self,
            Self::Percent | Self::GramPerMillilitre | Self::Unitless
        )
    }
}

#[derive(Debug)]
pub struct Parameter {
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
pub const WASTE: &str = "waste";
pub const DENSITY: &str = "density";

#[must_use]
pub fn by_id(id: i64) -> Option<&'static Parameter> {
    PARAMETERS.iter().find(|parameter| parameter.id == id)
}

#[must_use]
pub fn by_key(key: &str) -> Option<&'static Parameter> {
    PARAMETERS.iter().find(|parameter| parameter.key == key)
}

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
