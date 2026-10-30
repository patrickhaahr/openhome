//! A synthetic workbook shaped like DTU's Danish Food Composition Database, for Food database
//! and Meal Log tests.
#![allow(dead_code)]

use std::fmt::Write;

use rust_xlsxwriter::Workbook;
use sha2::{Digest, Sha256};

pub struct SyntheticParameter {
    pub id: i64,
    pub name_da: &'static str,
    pub name_en: &'static str,
    pub unit: &'static str,
    pub sort_key: i64,
    pub group_da: &'static str,
    pub group_en: &'static str,
    /// Glycerol is defined on the Parameter sheet but, as in DTU's workbook, has no data column.
    pub has_column: bool,
}

pub const fn parameter(
    id: i64,
    names: (&'static str, &'static str),
    unit: &'static str,
    sort_key: i64,
    group: (&'static str, &'static str),
) -> SyntheticParameter {
    SyntheticParameter {
        id,
        name_da: names.0,
        name_en: names.1,
        unit,
        sort_key,
        group_da: group.0,
        group_en: group.1,
        has_column: true,
    }
}

pub const MACRO: (&str, &str) = ("Makronæringstoffer m.m.", "Macro nutrients etc.");
pub const FACTORS: (&str, &str) = ("Faktorer m.m.", "Factors etc.");

pub fn parameters(protein_unit: &'static str) -> Vec<SyntheticParameter> {
    vec![
        parameter(137, ("Energi (kJ)", "Energy (kJ)"), "kJ/100g", 1010, MACRO),
        parameter(
            356,
            ("Energi (kcal)", "Energy (kcal)"),
            "kcal/100 g",
            1030,
            MACRO,
        ),
        parameter(218, ("Protein", "Protein"), protein_unit, 1110, MACRO),
        parameter(
            172,
            ("Tilgængelig kulhydrat", "Available carbohydrates"),
            "g/100g",
            1220,
            MACRO,
        ),
        parameter(168, ("Kostfibre", "Dietary fibre"), "g/100g", 1240, MACRO),
        parameter(141, ("Fedt", "Fat"), "g/100g", 1310, MACRO),
        parameter(19, ("Alkohol", "Alcohol"), "g/100g", 1410, MACRO),
        parameter(
            126,
            ("D-vitamin", "Vitamin D"),
            "µg/100g",
            2110,
            ("Vitaminer", "Vitamins"),
        ),
        SyntheticParameter {
            has_column: false,
            ..parameter(
                447,
                ("Glycerin", "Glycerol"),
                "g/100g",
                4580,
                ("Sukkeralkoholer", "Sugar alcohols"),
            )
        },
        parameter(
            87,
            ("C20:5,n-3", "C20:5,n-3"),
            "g/100g",
            9110,
            ("Flerumættede fedtsyrer", "Polyunsaturated fatty acids"),
        ),
        parameter(252, ("Svind", "Waste"), "%", 20010, FACTORS),
        parameter(363, ("Massefylde", "Density"), "g/ml", 20210, FACTORS),
        parameter(432, ("pH", "pH"), "No Unit", 20220, FACTORS),
    ]
}

pub struct SyntheticFood {
    pub id: i64,
    pub name_da: &'static str,
    pub name_en: &'static str,
    pub group: (i64, &'static str, &'static str),
    /// `ParameterID` and value; an absent parameter is an empty cell.
    pub values: Vec<(i64, f64)>,
}

pub fn skyr(protein: f64) -> SyntheticFood {
    SyntheticFood {
        id: 1693,
        name_da: "Skyr, 0.2 % fedt",
        name_en: "Skyr, 0.2 % fat",
        group: (3, "Syrnede mælkeprodukter", "Fermented milk products"),
        values: vec![
            (137, 260.0),
            (356, 62.0),
            (218, protein),
            (172, 4.0),
            (168, 0.0),
            (141, 0.2),
            (19, 0.0),
            (126, 0.0),
            (252, 0.0),
            (363, 1.03),
            (432, 4.2),
        ],
    }
}

pub fn chicken_breast() -> SyntheticFood {
    SyntheticFood {
        id: 907,
        name_da: "Kylling, bryst, kød og skind, rå",
        name_en: "Chicken, breast, flesh and skin, raw",
        group: (80, "Fjerkræ", "Poultry"),
        values: vec![
            (356, 150.0),
            (218, 21.0),
            (141, 7.0),
            (87, 0.01),
            (252, 10.0),
        ],
    }
}

pub fn banana() -> SyntheticFood {
    SyntheticFood {
        id: 3,
        name_da: "Banan, rå",
        name_en: "Banana, raw",
        group: (52, "Sydfrugt", "Tropical or subtropical fruit"),
        values: vec![(356, 90.0), (172, 20.0), (252, 40.0)],
    }
}

pub fn apple() -> SyntheticFood {
    SyntheticFood {
        id: 2,
        name_da: "Æble, uspec., råt",
        name_en: "Apple, raw, all varieties",
        group: (49, "Kernefrugt", "Pome fruit"),
        values: vec![(356, 50.0), (172, 11.0), (252, 12.0)],
    }
}

pub fn rye_bread() -> SyntheticFood {
    SyntheticFood {
        id: 5,
        name_da: "Rugbrød, fuldkorn",
        name_en: "Rye bread, wholemeal",
        group: (27, "Brød", "Bread"),
        values: vec![(356, 210.0), (218, 6.0)],
    }
}

pub fn first_dataset() -> Vec<u8> {
    workbook(
        &parameters("g/100g"),
        &[apple(), banana(), chicken_breast(), skyr(11.0)],
    )
}

/// A later dataset: Skyr's protein changes, Banana is dropped and Rye bread is new.
pub fn second_dataset() -> Vec<u8> {
    workbook(
        &parameters("g/100g"),
        &[apple(), chicken_breast(), skyr(12.0), rye_bread()],
    )
}

/// The sheets and header layout of DTU's `FCDB_6.1_Dataset.xlsx`.
pub fn workbook(parameters: &[SyntheticParameter], foods: &[SyntheticFood]) -> Vec<u8> {
    let mut book = Workbook::new();
    book.add_worksheet()
        .set_name("Readme")
        .unwrap()
        .write_string(0, 0, "Synthetic workbook for tests")
        .unwrap();

    let table = book.add_worksheet().set_name("Data_Table").unwrap();
    let columns: Vec<&SyntheticParameter> = parameters.iter().filter(|p| p.has_column).collect();
    for (row, label) in ["→ParameterNavn", "→ParameterName", "→Unit"]
        .into_iter()
        .enumerate()
    {
        table.write_string(row as u32, 2, label).unwrap();
    }
    table.write_string(3, 0, "↓FødevareNavn").unwrap();
    table.write_string(3, 1, "↓FoodName").unwrap();
    table.write_string(3, 2, "↓FoodID/→ParameterID").unwrap();
    for (index, parameter) in columns.iter().enumerate() {
        let column = 3 + index as u16;
        table.write_string(0, column, parameter.name_da).unwrap();
        table.write_string(1, column, parameter.name_en).unwrap();
        table.write_string(2, column, parameter.unit).unwrap();
        table.write_number(3, column, parameter.id as f64).unwrap();
    }
    let trailing = 3 + columns.len() as u16;
    for (offset, label) in ["↓FoodGroupID", "↓FødevareGruppe", "↓FoodGroup"]
        .into_iter()
        .enumerate()
    {
        table
            .write_string(3, trailing + offset as u16, label)
            .unwrap();
    }
    for (index, food) in foods.iter().enumerate() {
        let row = 4 + index as u32;
        table.write_string(row, 0, food.name_da).unwrap();
        table.write_string(row, 1, food.name_en).unwrap();
        table.write_number(row, 2, food.id as f64).unwrap();
        for (column, parameter) in columns.iter().enumerate() {
            if let Some((_, value)) = food.values.iter().find(|(id, _)| *id == parameter.id) {
                table.write_number(row, 3 + column as u16, *value).unwrap();
            }
        }
        table
            .write_number(row, trailing, food.group.0 as f64)
            .unwrap();
        table.write_string(row, trailing + 1, food.group.1).unwrap();
        table.write_string(row, trailing + 2, food.group.2).unwrap();
    }

    let food_sheet = book.add_worksheet().set_name("Food").unwrap();
    for (column, header) in [
        "FødevareNavn",
        "FoodName",
        "FoodID",
        "TaxonomicName",
        "FoodGroupID",
        "FødevareGruppe",
        "FoodGroup",
    ]
    .into_iter()
    .enumerate()
    {
        food_sheet.write_string(0, column as u16, header).unwrap();
    }
    for (index, food) in foods.iter().enumerate() {
        let row = 1 + index as u32;
        food_sheet.write_string(row, 0, food.name_da).unwrap();
        food_sheet.write_string(row, 1, food.name_en).unwrap();
        food_sheet.write_number(row, 2, food.id as f64).unwrap();
        food_sheet.write_string(row, 3, "NULL").unwrap();
        food_sheet
            .write_number(row, 4, food.group.0 as f64)
            .unwrap();
        food_sheet.write_string(row, 5, food.group.1).unwrap();
        food_sheet.write_string(row, 6, food.group.2).unwrap();
    }

    let parameter_sheet = book.add_worksheet().set_name("Parameter").unwrap();
    for (column, header) in [
        "ParameterNavn",
        "ParameterName",
        "Unit",
        "SortKey",
        "ParameterID",
        "ParameterGroupID",
        "ParameterGruppeNavn",
        "ParameterGroupName",
    ]
    .into_iter()
    .enumerate()
    {
        parameter_sheet
            .write_string(0, column as u16, header)
            .unwrap();
    }
    for (index, parameter) in parameters.iter().enumerate() {
        let row = 1 + index as u32;
        parameter_sheet
            .write_string(row, 0, parameter.name_da)
            .unwrap();
        parameter_sheet
            .write_string(row, 1, parameter.name_en)
            .unwrap();
        parameter_sheet
            .write_string(row, 2, parameter.unit)
            .unwrap();
        parameter_sheet
            .write_number(row, 3, parameter.sort_key as f64)
            .unwrap();
        parameter_sheet
            .write_number(row, 4, parameter.id as f64)
            .unwrap();
        parameter_sheet.write_number(row, 5, 1.0).unwrap();
        parameter_sheet
            .write_string(row, 6, parameter.group_da)
            .unwrap();
        parameter_sheet
            .write_string(row, 7, parameter.group_en)
            .unwrap();
    }

    book.save_to_buffer().unwrap()
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut text, byte| {
            write!(text, "{byte:02x}").unwrap();
            text
        })
}
