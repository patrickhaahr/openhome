//! Reads DTU's Danish Food Composition Database workbook.
//!
//! The workbook's `Parameter` sheet defines every parameter (names, unit, sort key, group), its
//! `Food` sheet every Food (`FoodID`, names, food group), and `Data_Table` holds one row per Food
//! with one column per `ParameterID`. Columns are found by their header names, so added or
//! reordered columns still import. An empty `Data_Table` cell is an unknown value; a recorded
//! zero stays zero.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use calamine::{Data, Range, Reader, Xlsx, open_workbook};
use thiserror::Error;

const PARAMETER_SHEET: &str = "Parameter";
const FOOD_SHEET: &str = "Food";
const DATA_SHEET: &str = "Data_Table";
/// The `Data_Table` header cell that heads the `FoodID` column and the `ParameterID` row,
/// written `↓FoodID/→ParameterID` in the workbook.
const DATA_MARKER: &str = "FoodID/ParameterID";

#[derive(Debug)]
pub struct Workbook {
    pub parameters: Vec<WorkbookParameter>,
    pub foods: Vec<WorkbookFood>,
    pub values: Vec<WorkbookValue>,
}

#[derive(Debug)]
pub struct WorkbookParameter {
    pub id: i64,
    pub name_da: String,
    pub name_en: String,
    pub unit: String,
    pub sort_key: i64,
    pub group_da: String,
    pub group_en: String,
}

#[derive(Debug)]
pub struct WorkbookFood {
    pub id: i64,
    pub name_da: String,
    pub name_en: String,
    pub group_id: Option<i64>,
    pub group_da: Option<String>,
    pub group_en: Option<String>,
}

#[derive(Debug)]
pub struct WorkbookValue {
    pub food_id: i64,
    pub parameter_id: i64,
    pub value: f64,
}

#[derive(Debug, Error)]
pub enum WorkbookError {
    #[error("the workbook could not be opened as xlsx")]
    Unreadable,
    #[error("the workbook has no readable {0} sheet")]
    MissingSheet(&'static str),
    #[error("the {sheet} sheet has no {column} column")]
    MissingColumn {
        sheet: &'static str,
        column: &'static str,
    },
    #[error("the {sheet} sheet, row {row}: {problem}")]
    Invalid {
        sheet: &'static str,
        row: usize,
        problem: String,
    },
}

/// Reads and validates the whole workbook. Blocking: run it off the async runtime.
pub fn read(path: &Path) -> Result<Workbook, WorkbookError> {
    let mut workbook: Xlsx<_> = open_workbook(path).map_err(|_| WorkbookError::Unreadable)?;
    let mut sheet = |name: &'static str| {
        workbook
            .worksheet_range(name)
            .map_err(|_| WorkbookError::MissingSheet(name))
    };
    let parameters = parameters(&sheet(PARAMETER_SHEET)?)?;
    let foods = foods(&sheet(FOOD_SHEET)?)?;
    let values = values(&sheet(DATA_SHEET)?, &parameters, &foods)?;
    Ok(Workbook {
        parameters,
        foods,
        values,
    })
}

fn parameters(range: &Range<Data>) -> Result<Vec<WorkbookParameter>, WorkbookError> {
    let sheet = PARAMETER_SHEET;
    let columns = [
        "ParameterID",
        "ParameterNavn",
        "ParameterName",
        "Unit",
        "SortKey",
        "ParameterGruppeNavn",
        "ParameterGroupName",
    ];
    let (header, [id, name_da, name_en, unit, sort_key, group_da, group_en]) =
        header(range, sheet, columns)?;
    let mut seen = HashSet::new();
    let mut parameters = Vec::new();
    for (row, cells) in data_rows(range, header) {
        let Some(id) = optional_integer(cells, id, sheet, row)? else {
            continue;
        };
        if !seen.insert(id) {
            return Err(invalid(sheet, row, format!("ParameterID {id} is repeated")));
        }
        parameters.push(WorkbookParameter {
            id,
            name_da: required_text(cells, name_da, sheet, row, "ParameterNavn")?,
            name_en: required_text(cells, name_en, sheet, row, "ParameterName")?,
            unit: required_text(cells, unit, sheet, row, "Unit")?,
            sort_key: optional_integer(cells, sort_key, sheet, row)?
                .ok_or_else(|| invalid(sheet, row, "SortKey is empty".into()))?,
            group_da: required_text(cells, group_da, sheet, row, "ParameterGruppeNavn")?,
            group_en: required_text(cells, group_en, sheet, row, "ParameterGroupName")?,
        });
    }
    Ok(parameters)
}

fn foods(range: &Range<Data>) -> Result<Vec<WorkbookFood>, WorkbookError> {
    let sheet = FOOD_SHEET;
    let columns = [
        "FoodID",
        "FødevareNavn",
        "FoodName",
        "FoodGroupID",
        "FødevareGruppe",
        "FoodGroup",
    ];
    let (header, [id, name_da, name_en, group_id, group_da, group_en]) =
        header(range, sheet, columns)?;
    let mut seen = HashSet::new();
    let mut foods = Vec::new();
    for (row, cells) in data_rows(range, header) {
        let Some(id) = optional_integer(cells, id, sheet, row)? else {
            continue;
        };
        if !seen.insert(id) {
            return Err(invalid(sheet, row, format!("FoodID {id} is repeated")));
        }
        foods.push(WorkbookFood {
            id,
            name_da: required_text(cells, name_da, sheet, row, "FødevareNavn")?,
            name_en: required_text(cells, name_en, sheet, row, "FoodName")?,
            group_id: optional_integer(cells, group_id, sheet, row)?,
            group_da: optional_text(cells, group_da),
            group_en: optional_text(cells, group_en),
        });
    }
    if foods.is_empty() {
        return Err(invalid(
            sheet,
            header + 1,
            "the sheet lists no Foods".into(),
        ));
    }
    Ok(foods)
}

fn values(
    range: &Range<Data>,
    parameters: &[WorkbookParameter],
    foods: &[WorkbookFood],
) -> Result<Vec<WorkbookValue>, WorkbookError> {
    let sheet = DATA_SHEET;
    let (header, food_column) = range
        .rows()
        .enumerate()
        .find_map(|(row, cells)| {
            cells
                .iter()
                .position(|cell| {
                    text(cell)
                        .is_some_and(|text| text.replace(['↓', '→'], "").trim() == DATA_MARKER)
                })
                .map(|column| (row, column))
        })
        .ok_or(WorkbookError::MissingColumn {
            sheet,
            column: DATA_MARKER,
        })?;

    let defined: HashSet<i64> = parameters.iter().map(|parameter| parameter.id).collect();
    let mut columns = Vec::new();
    let header_cells = range.rows().nth(header).unwrap_or_default();
    // ParameterIDs run right of the marker until the trailing food group columns begin.
    for (column, cell) in header_cells.iter().enumerate().skip(food_column + 1) {
        let Some(id) = integer(cell) else { break };
        if !defined.contains(&id) {
            return Err(invalid(
                sheet,
                header + 1,
                format!("ParameterID {id} is not on the Parameter sheet"),
            ));
        }
        columns.push((column, id));
    }
    if columns.is_empty() {
        return Err(invalid(sheet, header + 1, "no ParameterID columns".into()));
    }

    let known: HashSet<i64> = foods.iter().map(|food| food.id).collect();
    let mut seen = HashSet::new();
    let mut values = Vec::new();
    for (row, cells) in data_rows(range, header) {
        let Some(food_id) = optional_integer(cells, food_column, sheet, row)? else {
            continue;
        };
        if !known.contains(&food_id) {
            return Err(invalid(
                sheet,
                row,
                format!("FoodID {food_id} is not on the Food sheet"),
            ));
        }
        if !seen.insert(food_id) {
            return Err(invalid(sheet, row, format!("FoodID {food_id} is repeated")));
        }
        for &(column, parameter_id) in &columns {
            if let Some(value) = number(cells.get(column).unwrap_or(&Data::Empty))
                .map_err(|problem| invalid(sheet, row, problem))?
            {
                values.push(WorkbookValue {
                    food_id,
                    parameter_id,
                    value,
                });
            }
        }
    }
    Ok(values)
}

/// Finds the first row holding every named column and returns its index with the columns'
/// positions, in the order asked for.
fn header<const N: usize>(
    range: &Range<Data>,
    sheet: &'static str,
    names: [&'static str; N],
) -> Result<(usize, [usize; N]), WorkbookError> {
    let mut first_missing = names[0];
    for (row, cells) in range.rows().enumerate() {
        let positions: HashMap<&str, usize> = cells
            .iter()
            .enumerate()
            .filter_map(|(column, cell)| text(cell).map(|text| (text.trim(), column)))
            .collect();
        if let Some(missing) = names.iter().find(|name| !positions.contains_key(*name)) {
            if !positions.is_empty() {
                first_missing = missing;
            }
            continue;
        }
        return Ok((row, names.map(|name| positions[name])));
    }
    Err(WorkbookError::MissingColumn {
        sheet,
        column: first_missing,
    })
}

/// The rows below the header, numbered as spreadsheet rows for error messages.
fn data_rows(range: &Range<Data>, header: usize) -> impl Iterator<Item = (usize, &[Data])> {
    let first_row = range.start().map_or(0, |(row, _)| row as usize);
    range
        .rows()
        .enumerate()
        .skip(header + 1)
        .map(move |(index, cells)| (first_row + index + 1, cells))
}

fn invalid(sheet: &'static str, row: usize, problem: String) -> WorkbookError {
    WorkbookError::Invalid {
        sheet,
        row,
        problem,
    }
}

fn text(cell: &Data) -> Option<&str> {
    match cell {
        Data::String(text) => Some(text),
        _ => None,
    }
}

/// The workbook writes `NULL` for a missing descriptive value.
fn optional_text(cells: &[Data], column: usize) -> Option<String> {
    cells
        .get(column)
        .and_then(text)
        .map(str::trim)
        .filter(|text| !text.is_empty() && *text != "NULL")
        .map(str::to_owned)
}

fn required_text(
    cells: &[Data],
    column: usize,
    sheet: &'static str,
    row: usize,
    name: &str,
) -> Result<String, WorkbookError> {
    optional_text(cells, column).ok_or_else(|| invalid(sheet, row, format!("{name} is empty")))
}

fn integer(cell: &Data) -> Option<i64> {
    match cell {
        Data::Int(value) => Some(*value),
        Data::Float(value) if value.is_finite() && value.fract() == 0.0 => Some(*value as i64),
        Data::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn optional_integer(
    cells: &[Data],
    column: usize,
    sheet: &'static str,
    row: usize,
) -> Result<Option<i64>, WorkbookError> {
    let cell = cells.get(column).unwrap_or(&Data::Empty);
    if is_blank(cell) {
        return Ok(None);
    }
    integer(cell)
        .map(Some)
        .ok_or_else(|| invalid(sheet, row, format!("{cell:?} is not a whole number")))
}

fn is_blank(cell: &Data) -> bool {
    match cell {
        Data::Empty => true,
        Data::String(text) => matches!(text.trim(), "" | "NULL"),
        _ => false,
    }
}

/// A parameter value: `None` for an empty cell, the number otherwise.
fn number(cell: &Data) -> Result<Option<f64>, String> {
    let value = match cell {
        _ if is_blank(cell) => return Ok(None),
        Data::Int(value) => *value as f64,
        Data::Float(value) => *value,
        Data::String(text) => text
            .trim()
            .parse()
            .map_err(|_| format!("{text:?} is not a number"))?,
        other => return Err(format!("{other:?} is not a number")),
    };
    if value.is_finite() {
        Ok(Some(value))
    } else {
        Err(format!("{value} is not a finite number"))
    }
}
