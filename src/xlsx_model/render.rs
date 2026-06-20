use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use super::{CellValue, RangeBounds, WorkbookSheet, col_name, parse_cell_ref, range_contains_cell};

#[derive(Clone)]
pub(crate) struct XlsxCellEntry {
    pub(crate) ref_name: String,
    pub(crate) row: u32,
    pub(crate) col: u32,
    pub(crate) value: CellValue,
}

#[derive(Clone, Copy)]
pub(crate) struct UsedRangeSummary {
    pub(crate) min_row: u32,
    pub(crate) max_row: u32,
    pub(crate) min_col: u32,
    pub(crate) max_col: u32,
    pub(crate) empty: bool,
}

pub(crate) fn sorted_xlsx_cells(
    cells: &BTreeMap<String, CellValue>,
    range: Option<RangeBounds>,
) -> Vec<XlsxCellEntry> {
    let mut entries: Vec<XlsxCellEntry> = cells
        .iter()
        .filter_map(|(ref_name, value)| {
            let (col, row) = parse_cell_ref(ref_name).ok()?;
            if let Some(bounds) = range
                && !range_contains_cell(bounds, col, row)
            {
                return None;
            }
            Some(XlsxCellEntry {
                ref_name: ref_name.clone(),
                row,
                col,
                value: value.clone(),
            })
        })
        .collect();
    entries.sort_by_key(|entry| (entry.row, entry.col));
    entries
}

pub(crate) fn used_range_for_cells(cells: &[XlsxCellEntry]) -> UsedRangeSummary {
    let Some(first) = cells.first() else {
        return UsedRangeSummary {
            min_row: 0,
            max_row: 0,
            min_col: 0,
            max_col: 0,
            empty: true,
        };
    };
    let mut used = UsedRangeSummary {
        min_row: first.row,
        max_row: first.row,
        min_col: first.col,
        max_col: first.col,
        empty: false,
    };
    for cell in cells.iter().skip(1) {
        used.min_row = used.min_row.min(cell.row);
        used.max_row = used.max_row.max(cell.row);
        used.min_col = used.min_col.min(cell.col);
        used.max_col = used.max_col.max(cell.col);
    }
    used
}

pub(crate) fn used_range_json(used: UsedRangeSummary) -> Value {
    if used.empty {
        return json!({
            "rows": 0,
            "cols": 0,
            "empty": true,
        });
    }
    json!({
        "ref": format!(
            "{}{}:{}{}",
            col_name(used.min_col),
            used.min_row,
            col_name(used.max_col),
            used.max_row
        ),
        "minRow": used.min_row,
        "maxRow": used.max_row,
        "minCol": used.min_col,
        "maxCol": used.max_col,
        "rows": used.max_row - used.min_row + 1,
        "cols": used.max_col - used.min_col + 1,
        "empty": false,
    })
}

pub(crate) fn used_range_ref(used: UsedRangeSummary) -> Option<String> {
    if used.empty {
        None
    } else {
        Some(format!(
            "{}{}:{}{}",
            col_name(used.min_col),
            used.min_row,
            col_name(used.max_col),
            used.max_row
        ))
    }
}

pub(crate) fn build_sparse_xlsx_rows(
    cells: &[XlsxCellEntry],
    max_rows: u32,
    max_cells: u32,
    sheet: &WorkbookSheet,
) -> (Vec<Value>, bool) {
    let mut rows = Vec::<Value>::new();
    let mut row_cells = Vec::<Value>::new();
    let mut current_row = None::<u32>;
    let mut truncated = false;

    for (emitted_cells, cell) in cells.iter().enumerate() {
        if max_cells > 0 && emitted_cells as u32 >= max_cells {
            truncated = true;
            break;
        }
        if current_row != Some(cell.row) {
            if let Some(row_number) = current_row {
                rows.push(json!({"number": row_number, "cells": row_cells}));
                row_cells = Vec::new();
            }
            if max_rows > 0 && rows.len() as u32 >= max_rows {
                truncated = true;
                break;
            }
            current_row = Some(cell.row);
        }
        row_cells.push(xlsx_cell_json(
            &cell.ref_name,
            cell.row,
            cell.col,
            &cell.value,
            sheet,
        ));
    }

    if let Some(row_number) = current_row
        && !row_cells.is_empty()
    {
        rows.push(json!({"number": row_number, "cells": row_cells}));
    }
    (rows, truncated)
}

pub(crate) fn build_dense_xlsx_rows(
    cells: &[XlsxCellEntry],
    range: Option<RangeBounds>,
    used: UsedRangeSummary,
    max_rows: u32,
    max_cells: u32,
    sheet: &WorkbookSheet,
) -> (Vec<Value>, bool) {
    let Some((min_col, min_row, max_col, max_row)) = output_xlsx_bounds(range, used) else {
        return (Vec::new(), false);
    };
    let max_cells = if max_cells == 0 { 10_000 } else { max_cells };
    let by_ref: BTreeMap<String, &XlsxCellEntry> = cells
        .iter()
        .map(|cell| (cell.ref_name.clone(), cell))
        .collect();
    let mut rows = Vec::new();
    let mut emitted_cells = 0u32;
    let mut truncated = false;

    for row in min_row..=max_row {
        if max_rows > 0 && rows.len() as u32 >= max_rows {
            truncated = true;
            break;
        }
        let mut row_cells = Vec::new();
        for col in min_col..=max_col {
            if max_cells > 0 && emitted_cells >= max_cells {
                truncated = true;
                break;
            }
            let ref_name = format!("{}{}", col_name(col), row);
            let cell_value;
            let value = if let Some(cell) = by_ref.get(&ref_name) {
                &cell.value
            } else {
                cell_value = CellValue {
                    kind: "empty".to_string(),
                    matrix_value: Value::Null,
                    display_value: String::new(),
                    raw_value: String::new(),
                    formula: String::new(),
                    style_index: None,
                    number_format_id: None,
                    number_format_code: None,
                    date_style: false,
                    has_formula: false,
                };
                &cell_value
            };
            row_cells.push(xlsx_cell_json(&ref_name, row, col, value, sheet));
            emitted_cells += 1;
        }
        rows.push(json!({"number": row, "cells": row_cells}));
        if truncated {
            break;
        }
    }
    (rows, truncated)
}

fn output_xlsx_bounds(
    range: Option<RangeBounds>,
    used: UsedRangeSummary,
) -> Option<(u32, u32, u32, u32)> {
    if let Some(range) = range {
        return Some((
            range.start_col,
            range.start_row,
            range.end_col,
            range.end_row,
        ));
    }
    if used.empty {
        None
    } else {
        Some((used.min_col, used.min_row, used.max_col, used.max_row))
    }
}

fn xlsx_cell_json(
    ref_name: &str,
    row: u32,
    col: u32,
    value: &CellValue,
    sheet: &WorkbookSheet,
) -> Value {
    let mut object = Map::new();
    object.insert("ref".to_string(), json!(ref_name));
    object.insert(
        "handle".to_string(),
        json!(format!("H:xlsx/ws:{}/cell:a:{ref_name}", sheet.sheet_id)),
    );
    object.insert("primarySelector".to_string(), json!(ref_name));
    object.insert("selectors".to_string(), json!([ref_name]));
    object.insert("row".to_string(), json!(row));
    object.insert("col".to_string(), json!(col));
    object.insert("column".to_string(), json!(col_name(col)));
    object.insert("type".to_string(), json!(value.kind));
    if !value.display_value.is_empty() {
        object.insert("value".to_string(), json!(value.display_value));
    }
    if !value.raw_value.is_empty() {
        object.insert("rawValue".to_string(), json!(value.raw_value));
    }
    if !value.formula.is_empty() {
        object.insert("formula".to_string(), json!(value.formula));
    }
    if let Some(style_index) = value.style_index.filter(|style_index| *style_index > 0) {
        object.insert("styleIndex".to_string(), json!(style_index));
    }
    if let Some(number_format_id) = value
        .number_format_id
        .filter(|number_format_id| *number_format_id > 0)
    {
        object.insert("numberFormatId".to_string(), json!(number_format_id));
    }
    if let Some(number_format_code) = value
        .number_format_code
        .as_ref()
        .filter(|number_format_code| !number_format_code.is_empty())
    {
        object.insert("numberFormatCode".to_string(), json!(number_format_code));
    }
    if value.date_style {
        object.insert("dateStyle".to_string(), json!(true));
    }
    Value::Object(object)
}
