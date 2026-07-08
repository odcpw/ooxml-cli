use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, append_xml_text_event, attr, attr_exact, is_xml_text_event, local_name,
    zip_text,
};

mod range;
mod render;
mod styles;

pub(crate) use range::{
    RangeBounds, col_name, normalize_xlsx_cell_ref, parse_cell_ref, parse_cli_range, parse_range,
    range_contains_cell,
};
pub(crate) use render::{
    XlsxCellEntry, build_dense_xlsx_rows, build_sparse_xlsx_rows, sorted_xlsx_cells,
    used_range_for_cells, used_range_json, used_range_ref,
};
pub(crate) use styles::{XlsxStyle, builtin_num_format_code, xlsx_styles};

#[derive(Clone)]
pub(crate) struct WorkbookSheet {
    pub(crate) name: String,
    pub(crate) sheet_id: u32,
    pub(crate) position: u32,
    pub(crate) rel_id: String,
    pub(crate) state: String,
}

pub(crate) fn workbook_sheets(xml: &str) -> CliResult<Vec<WorkbookSheet>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut sheets = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut saw_workbook = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.is_empty() && name != "workbook" {
                    return Err(CliError::unexpected(format!(
                        "workbook root is {name:?}, expected workbook"
                    )));
                }
                if stack.is_empty() {
                    saw_workbook = true;
                }
                let parent = stack.last().map(String::as_str);
                if parent == Some("sheets") && name == "sheet" {
                    parse_workbook_sheet(&e, &mut sheets)?;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.is_empty() && name != "workbook" {
                    return Err(CliError::unexpected(format!(
                        "workbook root is {name:?}, expected workbook"
                    )));
                }
                if stack.is_empty() {
                    saw_workbook = true;
                }
                let parent = stack.last().map(String::as_str);
                if parent == Some("sheets") && name == "sheet" {
                    parse_workbook_sheet(&e, &mut sheets)?;
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(CliError::unexpected("unexpected EOF"));
    }
    if !saw_workbook {
        return Err(CliError::unexpected("workbook part has no root element"));
    }
    Ok(sheets)
}

fn parse_workbook_sheet(e: &BytesStart<'_>, sheets: &mut Vec<WorkbookSheet>) -> CliResult<()> {
    let position = sheets.len() as u32 + 1;
    if let (Some(name), Some(number), Some(rel_id)) =
        (attr(e, "name"), attr(e, "sheetId"), attr_exact(e, "r:id"))
    {
        let number = number.parse::<u32>().map_err(|_| {
            CliError::unexpected(format!("sheet at position {position} has invalid sheetId"))
        })?;
        sheets.push(WorkbookSheet {
            name,
            sheet_id: number,
            position,
            rel_id,
            state: attr(e, "state").unwrap_or_else(|| "visible".to_string()),
        });
        Ok(())
    } else {
        Err(CliError::unexpected(format!(
            "sheet at position {position} is missing name, sheetId, or r:id"
        )))
    }
}

pub(crate) fn resolve_sheet(sheets: &[WorkbookSheet], selector: &str) -> CliResult<WorkbookSheet> {
    if let Some(sheet_id) = parse_xlsx_sheet_handle(selector)? {
        return resolve_sheet_by_sheet_id_unique(sheets, sheet_id, selector);
    }
    if let Some(sheet_id) = selector.strip_prefix("sheetId:")
        && let Ok(sheet_id) = sheet_id.parse::<u32>()
        && let Some(sheet) = sheets.iter().find(|sheet| sheet.sheet_id == sheet_id)
    {
        return Ok(sheet.clone());
    }
    if let Some(position) = selector
        .strip_prefix("sheet:")
        .or_else(|| selector.strip_prefix('#'))
        && let Ok(position) = position.parse::<u32>()
        && let Some(sheet) = sheets.iter().find(|sheet| sheet.position == position)
    {
        return Ok(sheet.clone());
    }
    if let Some(name) = selector
        .strip_prefix("name:")
        .or_else(|| selector.strip_prefix('~'))
        && let Some(sheet) = sheets.iter().find(|sheet| sheet.name == name)
    {
        return Ok(sheet.clone());
    }
    if let Some(rel_id) = selector
        .strip_prefix("rId:")
        .or_else(|| selector.strip_prefix("rid:"))
        && let Some(sheet) = sheets.iter().find(|sheet| sheet.rel_id == rel_id)
    {
        return Ok(sheet.clone());
    }
    if let Ok(number) = selector.parse::<u32>()
        && let Some(sheet) = sheets.iter().find(|sheet| sheet.position == number)
    {
        return Ok(sheet.clone());
    }
    sheets
        .iter()
        .find(|sheet| sheet.name == selector)
        .cloned()
        .ok_or_else(|| CliError::invalid_args(format!("sheet not found: {selector}")))
}

pub(crate) fn xlsx_sheet_selectors(
    name: &str,
    sheet_id: u32,
    position: u32,
    rel_id: &str,
    part_uri: &str,
) -> Vec<String> {
    vec![
        format!("sheetId:{sheet_id}"),
        format!("sheet:{position}"),
        format!("#{position}"),
        format!("rId:{rel_id}"),
        format!("rid:{rel_id}"),
        format!("part:{part_uri}"),
        format!("name:{name}"),
        format!("~{name}"),
        name.to_string(),
    ]
}

pub(crate) fn normalize_xl_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("xl/") {
        target.to_string()
    } else {
        format!("xl/{}", target.trim_start_matches("../"))
    }
}

pub(crate) fn is_xlsx_handle(value: &str) -> bool {
    value.trim().starts_with("H:")
}

pub(crate) fn parse_xlsx_sheet_handle(value: &str) -> CliResult<Option<u32>> {
    let value = value.trim();
    if !is_xlsx_handle(value) {
        return Ok(None);
    }
    let body = value.trim_start_matches("H:");
    let parts = body.split('/').collect::<Vec<_>>();
    if parts.len() == 2
        && parts[0] == "xlsx"
        && let Some(sheet_id) = parts[1].strip_prefix("ws:")
    {
        return parse_xlsx_handle_sheet_id(sheet_id, value).map(Some);
    }
    if parts.first().copied() != Some("xlsx") {
        return Err(CliError::invalid_args(format!(
            "HANDLE_FORMAT_MISMATCH: handle format tag does not match package format \"xlsx\" (handle {value:?})"
        )));
    }
    Err(CliError::invalid_args(format!(
        "HANDLE_MALFORMED: expected a sheet handle (H:xlsx/ws:<sheetId>); a cell/comment handle belongs on the cell/comment flag (handle {value:?})"
    )))
}

pub(crate) fn parse_xlsx_cell_handle(value: &str) -> CliResult<(u32, String)> {
    let value = value.trim();
    let body = value.trim_start_matches("H:");
    let parts = body.split('/').collect::<Vec<_>>();
    if parts.first().copied() != Some("xlsx") {
        return Err(CliError::invalid_args(format!(
            "HANDLE_FORMAT_MISMATCH: handle format tag does not match package format \"xlsx\" (handle {value:?})"
        )));
    }
    if parts.len() != 3 {
        return Err(CliError::invalid_args(
            "--cell handle must be a cell handle (H:xlsx/ws:<sheetId>/cell:a:<A1>)",
        ));
    }
    let Some(sheet_id) = parts[1].strip_prefix("ws:") else {
        return Err(CliError::invalid_args(format!(
            "HANDLE_MALFORMED: worksheet scope is malformed (handle {value:?})"
        )));
    };
    let Some(cell_ref) = parts[2].strip_prefix("cell:a:") else {
        return Err(CliError::invalid_args(
            "--cell handle must be a cell handle (H:xlsx/ws:<sheetId>/cell:a:<A1>)",
        ));
    };
    Ok((
        parse_xlsx_handle_sheet_id(sheet_id, value)?,
        normalize_xlsx_cell_ref(cell_ref, "cell ref in handle")?,
    ))
}

fn parse_xlsx_handle_sheet_id(sheet_id: &str, handle: &str) -> CliResult<u32> {
    if sheet_id.trim().is_empty() {
        return Err(CliError::invalid_args(format!(
            "HANDLE_MALFORMED: empty worksheet sheetId (handle {handle:?})"
        )));
    }
    sheet_id.parse::<u32>().map_err(|_| {
        CliError::invalid_args(format!(
            "HANDLE_MALFORMED: worksheet sheetId must be numeric (handle {handle:?})"
        ))
    })
}

pub(crate) fn resolve_sheet_by_sheet_id_unique(
    sheets: &[WorkbookSheet],
    sheet_id: u32,
    selector: &str,
) -> CliResult<WorkbookSheet> {
    let matches = sheets
        .iter()
        .filter(|sheet| sheet.sheet_id == sheet_id)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [sheet] => Ok((*sheet).clone()),
        [] => Err(CliError::target_not_found(format!(
            "HANDLE_SCOPE_STALE: worksheet with sheetId {sheet_id} was not found (selector {selector:?})"
        ))),
        _ => Err(CliError::target_not_found(format!(
            "HANDLE_AMBIGUOUS: worksheet sheetId {sheet_id} is not unique (selector {selector:?})"
        ))),
    }
}

pub(crate) fn shared_strings(file: &str) -> CliResult<Vec<String>> {
    let xml = match zip_text(file, "xl/sharedStrings.xml") {
        Ok(xml) => xml,
        Err(_) => return Ok(Vec::new()),
    };
    let mut reader = Reader::from_str(&xml);
    let mut strings = Vec::new();
    let mut current = String::new();
    let mut in_si = false;
    let mut in_t = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "si" => {
                in_si = true;
                current.clear();
            }
            Ok(Event::Start(e)) if in_si && local_name(e.name().as_ref()) == "t" => in_t = true,
            Ok(event) if in_t && is_xml_text_event(&event) => {
                append_xml_text_event(&mut current, &event);
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => in_t = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "si" => {
                strings.push(current.clone());
                in_si = false;
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(strings)
}

#[derive(Clone)]
pub(crate) struct CellValue {
    pub(crate) kind: String,
    pub(crate) matrix_value: Value,
    pub(crate) display_value: String,
    pub(crate) raw_value: String,
    pub(crate) formula: String,
    pub(crate) style_index: Option<u32>,
    pub(crate) number_format_id: Option<u32>,
    pub(crate) number_format_code: Option<String>,
    pub(crate) date_style: bool,
    pub(crate) has_formula: bool,
}

pub(crate) fn sheet_cells(
    xml: &str,
    shared_strings: &[String],
    styles: &[XlsxStyle],
) -> BTreeMap<String, CellValue> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut cells = BTreeMap::new();
    let mut current_ref = String::new();
    let mut current_type = String::new();
    let mut current_value = String::new();
    let mut current_inline_text = String::new();
    let mut current_formula = String::new();
    let mut current_style_index: Option<u32> = None;
    let mut in_v = false;
    let mut in_t = false;
    let mut in_formula = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "c" => {
                current_ref = attr(&e, "r").unwrap_or_default();
                current_type = attr(&e, "t").unwrap_or_default();
                current_value.clear();
                current_inline_text.clear();
                current_formula.clear();
                current_style_index = attr(&e, "s").and_then(|value| value.parse::<u32>().ok());
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "c" => {
                let cell_ref = attr(&e, "r").unwrap_or_default();
                if !cell_ref.is_empty() {
                    let cell_type = attr(&e, "t").unwrap_or_default();
                    let style_index = attr(&e, "s").and_then(|value| value.parse::<u32>().ok());
                    let style = style_index
                        .and_then(|index| styles.get(index as usize).cloned())
                        .unwrap_or_default();
                    let (kind, matrix_value, display_value) =
                        decode_xlsx_cell_value(&cell_type, "", "", "", shared_strings, &style);
                    cells.insert(
                        cell_ref,
                        CellValue {
                            kind,
                            matrix_value,
                            display_value,
                            raw_value: String::new(),
                            formula: String::new(),
                            style_index,
                            number_format_id: style.number_format_id,
                            number_format_code: style.number_format_code,
                            date_style: style.date_style,
                            has_formula: false,
                        },
                    );
                }
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "v" => in_v = true,
            Ok(Event::Start(e))
                if current_type == "inlineStr" && local_name(e.name().as_ref()) == "t" =>
            {
                in_t = true;
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "f" => {
                in_formula = true;
            }
            Ok(event) if is_xml_text_event(&event) => {
                if in_v {
                    append_xml_text_event(&mut current_value, &event);
                } else if in_t {
                    append_xml_text_event(&mut current_inline_text, &event);
                } else if in_formula {
                    append_xml_text_event(&mut current_formula, &event);
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "v" => in_v = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => in_t = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "f" => in_formula = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "c" => {
                if !current_ref.is_empty() {
                    let style = current_style_index
                        .and_then(|index| styles.get(index as usize).cloned())
                        .unwrap_or_default();
                    let (kind, matrix_value, display_value) = decode_xlsx_cell_value(
                        &current_type,
                        &current_value,
                        &current_inline_text,
                        &current_formula,
                        shared_strings,
                        &style,
                    );
                    let raw_value = if current_type == "inlineStr" {
                        String::new()
                    } else {
                        current_value.clone()
                    };
                    cells.insert(
                        current_ref.clone(),
                        CellValue {
                            kind,
                            matrix_value,
                            display_value,
                            raw_value,
                            formula: current_formula.clone(),
                            style_index: current_style_index,
                            number_format_id: style.number_format_id,
                            number_format_code: style.number_format_code,
                            date_style: style.date_style,
                            has_formula: !current_formula.is_empty(),
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    cells
}

fn decode_xlsx_cell_value(
    cell_type: &str,
    raw: &str,
    inline_text: &str,
    formula: &str,
    shared_strings: &[String],
    style: &XlsxStyle,
) -> (String, Value, String) {
    match cell_type {
        "s" => {
            let idx = raw.parse::<usize>().unwrap_or(usize::MAX);
            let text = shared_strings.get(idx).cloned().unwrap_or_default();
            ("string".to_string(), Value::String(text.clone()), text)
        }
        "inlineStr" => (
            "string".to_string(),
            Value::String(inline_text.to_string()),
            inline_text.to_string(),
        ),
        "str" => (
            "string".to_string(),
            Value::String(raw.to_string()),
            raw.to_string(),
        ),
        "b" => {
            let text = match raw.trim() {
                "1" => "true",
                "0" => "false",
                _ => raw,
            }
            .to_string();
            let matrix = match raw.trim() {
                "1" => Value::Bool(true),
                "0" => Value::Bool(false),
                _ => Value::String(text.clone()),
            };
            ("boolean".to_string(), matrix, text)
        }
        "e" => (
            "error".to_string(),
            Value::String(raw.to_string()),
            raw.to_string(),
        ),
        "d" => (
            "date".to_string(),
            Value::String(raw.to_string()),
            raw.to_string(),
        ),
        "" if raw.is_empty() && formula.is_empty() => {
            ("empty".to_string(), Value::Null, String::new())
        }
        "" if raw.is_empty() && !formula.is_empty() => {
            ("number".to_string(), Value::Null, String::new())
        }
        "" if style.date_style => (
            "date".to_string(),
            Value::String(raw.to_string()),
            raw.to_string(),
        ),
        "" => {
            let matrix = if let Ok(number) = raw.parse::<i64>() {
                json!(number)
            } else if let Ok(number) = raw.parse::<f64>() {
                json!(number)
            } else {
                Value::String(raw.to_string())
            };
            ("number".to_string(), matrix, raw.to_string())
        }
        _ => (
            "unknown".to_string(),
            Value::String(raw.to_string()),
            raw.to_string(),
        ),
    }
}

pub(crate) fn xlsx_dimension_declared(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "dimension" =>
            {
                return attr(&e, "ref");
            }
            Ok(Event::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
    }
}

pub(crate) fn xlsx_merged_cell_count(xml: &str) -> usize {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut count = 0;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "mergeCell" =>
            {
                count += 1;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    count
}
