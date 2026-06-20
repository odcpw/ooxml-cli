use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, WorkbookSheet, attr, col_name, command_arg, local_name,
    normalize_xl_target, relationships, resolve_sheet, workbook_sheets, zip_text,
};

const DEFAULT_COL_WIDTH: f64 = 8.43;
const DEFAULT_ROW_HEIGHT: f64 = 15.0;
const XLSX_MAX_COL: u32 = 16_384;
const DIMENSION_TOLERANCE: f64 = 1e-6;

#[derive(Clone, Copy)]
struct ColumnWidthInfo {
    width: f64,
    explicit: bool,
    custom: bool,
    hidden: bool,
}

impl ColumnWidthInfo {
    fn default_with(width: f64) -> Self {
        Self {
            width,
            explicit: false,
            custom: false,
            hidden: false,
        }
    }
}

#[derive(Clone, Copy)]
struct RowHeightInfo {
    height: f64,
    explicit: bool,
    custom: bool,
    hidden: bool,
}

impl RowHeightInfo {
    fn default_with(height: f64) -> Self {
        Self {
            height,
            explicit: false,
            custom: false,
            hidden: false,
        }
    }
}

pub(crate) fn xlsx_colwidths_show(
    file: &str,
    sheet_selector: Option<&str>,
    range: &str,
) -> CliResult<Value> {
    let (min_col, max_col) = parse_column_span(range)?;
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector.unwrap_or("1"))?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (widths, fallback) = read_column_widths(&sheet_xml, min_col, max_col)?;
    let min_column = col_name(min_col);
    let max_column = col_name(max_col);
    let normalized_range = format!("{min_column}:{max_column}");
    let mut columns = BTreeMap::new();
    let mut distinct = Vec::new();
    for col in min_col..=max_col {
        let info = widths
            .get(&col)
            .copied()
            .unwrap_or_else(|| ColumnWidthInfo::default_with(fallback));
        distinct = append_distinct_float(distinct, info.width);
        columns.insert(
            col_name(col),
            json!({
                "width": dimension_json(info.width),
                "explicit": info.explicit,
                "custom": info.custom,
                "hidden": info.hidden,
            }),
        );
    }
    Ok(json!({
        "file": file,
        "sheet": sheet.name,
        "sheetNumber": sheet.position,
        "range": normalized_range,
        "minColumn": min_column,
        "maxColumn": max_column,
        "count": max_col - min_col + 1,
        "defaultWidth": dimension_json(fallback),
        "uniform": distinct.len() <= 1,
        "columns": columns,
        "colwidthsSetCommandTemplate": colwidths_set_command_template(file, &sheet, &normalized_range),
    }))
}

pub(crate) fn xlsx_rowheights_show(
    file: &str,
    sheet_selector: Option<&str>,
    range: &str,
) -> CliResult<Value> {
    let (min_row, max_row) = parse_row_span(range)?;
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector.unwrap_or("1"))?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (heights, fallback) = read_row_heights(&sheet_xml, min_row, max_row)?;
    let normalized_range = format!("{min_row}:{max_row}");
    let mut rows = BTreeMap::new();
    let mut distinct = Vec::new();
    for row in min_row..=max_row {
        let info = heights
            .get(&row)
            .copied()
            .unwrap_or_else(|| RowHeightInfo::default_with(fallback));
        distinct = append_distinct_float(distinct, info.height);
        rows.insert(
            row.to_string(),
            json!({
                "height": dimension_json(info.height),
                "explicit": info.explicit,
                "custom": info.custom,
                "hidden": info.hidden,
            }),
        );
    }
    Ok(json!({
        "file": file,
        "sheet": sheet.name,
        "sheetNumber": sheet.position,
        "range": normalized_range,
        "minRow": min_row,
        "maxRow": max_row,
        "count": max_row - min_row + 1,
        "defaultHeight": dimension_json(fallback),
        "uniform": distinct.len() <= 1,
        "rows": rows,
        "rowheightsSetCommandTemplate": rowheights_set_command_template(file, &sheet, &normalized_range),
    }))
}

fn colwidths_set_command_template(file: &str, sheet: &WorkbookSheet, range: &str) -> String {
    format!(
        "ooxml xlsx colwidths set {} --sheet {} --range {} --width <width> --in-place",
        command_arg(file),
        command_arg(&format!("sheetId:{}", sheet.sheet_id)),
        command_arg(range)
    )
}

fn rowheights_set_command_template(file: &str, sheet: &WorkbookSheet, range: &str) -> String {
    format!(
        "ooxml xlsx rowheights set {} --sheet {} --range {} --height <height> --in-place",
        command_arg(file),
        command_arg(&format!("sheetId:{}", sheet.sheet_id)),
        command_arg(range)
    )
}

fn read_column_widths(
    xml: &str,
    min_col: u32,
    max_col: u32,
) -> CliResult<(BTreeMap<u32, ColumnWidthInfo>, f64)> {
    let fallback = default_column_width(xml);
    let mut widths = BTreeMap::new();
    for col in min_col..=max_col {
        widths.insert(col, ColumnWidthInfo::default_with(fallback));
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut in_cols = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "cols" => {
                in_cols = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "cols" => {
                in_cols = false;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_cols && local_name(e.name().as_ref()) == "col" =>
            {
                apply_column_width_span(&e, min_col, max_col, fallback, &mut widths);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok((widths, fallback))
}

fn read_row_heights(
    xml: &str,
    min_row: u32,
    max_row: u32,
) -> CliResult<(BTreeMap<u32, RowHeightInfo>, f64)> {
    let fallback = default_row_height(xml);
    let mut heights = BTreeMap::new();
    for row in min_row..=max_row {
        heights.insert(row, RowHeightInfo::default_with(fallback));
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut in_sheet_data = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                in_sheet_data = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                in_sheet_data = false;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_sheet_data && local_name(e.name().as_ref()) == "row" =>
            {
                apply_row_height(&e, min_row, max_row, fallback, &mut heights);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok((heights, fallback))
}

fn default_column_width(xml: &str) -> f64 {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sheetFormatPr" =>
            {
                return attr(&e, "defaultColWidth")
                    .and_then(|value| value.parse::<f64>().ok())
                    .unwrap_or(DEFAULT_COL_WIDTH);
            }
            Ok(Event::Eof) => return DEFAULT_COL_WIDTH,
            Err(_) => return DEFAULT_COL_WIDTH,
            _ => {}
        }
    }
}

fn default_row_height(xml: &str) -> f64 {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sheetFormatPr" =>
            {
                return attr(&e, "defaultRowHeight")
                    .and_then(|value| value.parse::<f64>().ok())
                    .unwrap_or(DEFAULT_ROW_HEIGHT);
            }
            Ok(Event::Eof) => return DEFAULT_ROW_HEIGHT,
            Err(_) => return DEFAULT_ROW_HEIGHT,
            _ => {}
        }
    }
}

fn apply_column_width_span(
    element: &BytesStart<'_>,
    min_col: u32,
    max_col: u32,
    fallback: f64,
    widths: &mut BTreeMap<u32, ColumnWidthInfo>,
) {
    let Some((span_min, span_max)) = column_span_bounds(element) else {
        return;
    };
    let width = attr(element, "width").and_then(|value| value.parse::<f64>().ok());
    let custom = attr(element, "customWidth").as_deref() == Some("1");
    let hidden = attr(element, "hidden").as_deref() == Some("1");
    for col in span_min..=span_max {
        if col < min_col || col > max_col {
            continue;
        }
        widths.insert(
            col,
            ColumnWidthInfo {
                width: width.unwrap_or(fallback),
                explicit: width.is_some(),
                custom,
                hidden,
            },
        );
    }
}

fn apply_row_height(
    element: &BytesStart<'_>,
    min_row: u32,
    max_row: u32,
    fallback: f64,
    heights: &mut BTreeMap<u32, RowHeightInfo>,
) {
    let Some(row) = row_number(element) else {
        return;
    };
    if row < min_row || row > max_row {
        return;
    }
    let height = attr(element, "ht").and_then(|value| value.parse::<f64>().ok());
    heights.insert(
        row,
        RowHeightInfo {
            height: height.unwrap_or(fallback),
            explicit: height.is_some(),
            custom: attr(element, "customHeight").as_deref() == Some("1"),
            hidden: attr(element, "hidden").as_deref() == Some("1"),
        },
    );
}

fn column_span_bounds(element: &BytesStart<'_>) -> Option<(u32, u32)> {
    let min_col = attr(element, "min")?.parse::<u32>().ok()?;
    let max_col = attr(element, "max")?.parse::<u32>().ok()?;
    (max_col >= min_col).then_some((min_col, max_col))
}

fn row_number(element: &BytesStart<'_>) -> Option<u32> {
    attr(element, "r")?.parse::<u32>().ok()
}

fn parse_column_span(value: &str) -> CliResult<(u32, u32)> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args(
            "--range is required (e.g. B or B:D)",
        ));
    }
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() > 2 {
        return Err(CliError::invalid_args(format!(
            "invalid column range: {value}"
        )));
    }
    let first = parse_column(parts[0])?;
    let second = if let Some(part) = parts.get(1) {
        parse_column(part)?
    } else {
        first
    };
    Ok((first.min(second), first.max(second)))
}

fn parse_row_span(value: &str) -> CliResult<(u32, u32)> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args(
            "--range is required (e.g. 2 or 2:5)",
        ));
    }
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() > 2 {
        return Err(CliError::invalid_args(format!(
            "invalid row range: {value}"
        )));
    }
    let first = parse_row(parts[0])?;
    let second = if let Some(part) = parts.get(1) {
        parse_row(part)?
    } else {
        first
    };
    Ok((first.min(second), first.max(second)))
}

fn parse_column(value: &str) -> CliResult<u32> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args("invalid column \"\""));
    }
    if !value.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(CliError::invalid_args(format!(
            "invalid column {value:?}: invalid column reference {value:?}"
        )));
    }
    let mut col = 0u32;
    for ch in value.chars() {
        col = col * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        if col > XLSX_MAX_COL {
            return Err(CliError::invalid_args(format!(
                "column {value:?} out of XLSX bounds A-XFD"
            )));
        }
    }
    Ok(col)
}

fn parse_row(value: &str) -> CliResult<u32> {
    let trimmed = value.trim();
    match trimmed.parse::<u32>() {
        Ok(row) if row >= 1 => Ok(row),
        _ => Err(CliError::invalid_args(format!("invalid row {value:?}"))),
    }
}

fn append_distinct_float(mut values: Vec<f64>, value: f64) -> Vec<f64> {
    if values
        .iter()
        .any(|existing| (existing - value).abs() <= DIMENSION_TOLERANCE)
    {
        return values;
    }
    values.push(value);
    values
}

fn dimension_json(value: f64) -> Value {
    if value.is_finite() && value.fract().abs() <= DIMENSION_TOLERANCE {
        json!(value as i64)
    } else {
        json!(value)
    }
}
