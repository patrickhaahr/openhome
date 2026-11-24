use crate::builder::{SyntheticFood, workbook};
use crate::dataset_v1::{apple, chicken_breast, parameters, skyr};

pub fn rye_bread() -> SyntheticFood {
    SyntheticFood {
        id: 5,
        name_da: "Rugbrød, fuldkorn",
        name_en: "Rye bread, wholemeal",
        group: (27, "Brød", "Bread"),
        values: vec![(356, 210.0), (218, 6.0)],
    }
}

pub fn second_dataset() -> Vec<u8> {
    workbook(
        &parameters("g/100g"),
        &[apple(), chicken_breast(), skyr(12.0), rye_bread()],
    )
}
