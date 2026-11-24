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

pub struct SyntheticFood {
    pub id: i64,
    pub name_da: &'static str,
    pub name_en: &'static str,
    pub group: (i64, &'static str, &'static str),
    pub values: Vec<(i64, f64)>,
}

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
