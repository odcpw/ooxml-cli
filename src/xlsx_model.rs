use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, attr, attr_exact, decode_xml_text, local_name, xml_general_ref, zip_text,
};

mod range;

pub(crate) use range::{
    RangeBounds, col_name, normalize_xlsx_cell_ref, parse_cell_ref, parse_cli_range, parse_range,
    range_contains_cell,
};

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
            Ok(Event::Text(e)) if in_t => current.push_str(&decode_xml_text(e.as_ref())),
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

#[derive(Clone, Default)]
pub(crate) struct XlsxStyle {
    pub(crate) number_format_id: Option<u32>,
    pub(crate) number_format_code: Option<String>,
    pub(crate) date_style: bool,
}

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
            Ok(Event::Text(e)) if in_v => current_value.push_str(&decode_xml_text(e.as_ref())),
            Ok(Event::Text(e)) if in_t => {
                current_inline_text.push_str(&decode_xml_text(e.as_ref()))
            }
            Ok(Event::Text(e)) if in_formula => {
                current_formula.push_str(&decode_xml_text(e.as_ref()))
            }
            Ok(Event::GeneralRef(e)) if in_v => {
                current_value.push_str(&xml_general_ref(e.as_ref()))
            }
            Ok(Event::GeneralRef(e)) if in_t => {
                current_inline_text.push_str(&xml_general_ref(e.as_ref()))
            }
            Ok(Event::GeneralRef(e)) if in_formula => {
                current_formula.push_str(&xml_general_ref(e.as_ref()))
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

pub(crate) fn xlsx_styles(file: &str) -> CliResult<Vec<XlsxStyle>> {
    let xml = match zip_text(file, "xl/styles.xml") {
        Ok(xml) => xml,
        Err(_) => return Ok(Vec::new()),
    };
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut custom_formats = BTreeMap::<u32, String>::new();
    let mut styles = Vec::new();
    let mut in_cell_xfs = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "numFmt" =>
            {
                if let (Some(id), Some(code)) = (attr(&e, "numFmtId"), attr(&e, "formatCode"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    custom_formats.insert(id, code);
                }
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "cellXfs" => {
                in_cell_xfs = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "cellXfs" => {
                in_cell_xfs = false;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_cell_xfs && local_name(e.name().as_ref()) == "xf" =>
            {
                let number_format_id = attr(&e, "numFmtId").and_then(|value| value.parse().ok());
                let number_format_code = number_format_id.and_then(|id| {
                    custom_formats
                        .get(&id)
                        .cloned()
                        .or_else(|| builtin_num_format_code(id).map(ToString::to_string))
                });
                let date_style = number_format_id.is_some_and(is_builtin_date_num_fmt)
                    || number_format_code
                        .as_deref()
                        .is_some_and(is_date_format_code);
                styles.push(XlsxStyle {
                    number_format_id,
                    number_format_code,
                    date_style,
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(styles)
}

pub(crate) fn builtin_num_format_code(id: u32) -> Option<&'static str> {
    match id {
        0 => Some("General"),
        1 => Some("0"),
        2 => Some("0.00"),
        3 => Some("#,##0"),
        4 => Some("#,##0.00"),
        9 => Some("0%"),
        10 => Some("0.00%"),
        14 => Some("m/d/yy"),
        15 => Some("d-mmm-yy"),
        16 => Some("d-mmm"),
        17 => Some("mmm-yy"),
        18 => Some("h:mm AM/PM"),
        19 => Some("h:mm:ss AM/PM"),
        20 => Some("h:mm"),
        21 => Some("h:mm:ss"),
        22 => Some("m/d/yy h:mm"),
        45 => Some("mm:ss"),
        46 => Some("[h]:mm:ss"),
        47 => Some("mmss.0"),
        49 => Some("@"),
        _ => None,
    }
}

fn is_builtin_date_num_fmt(id: u32) -> bool {
    matches!(id, 14..=22 | 45..=47)
}

fn is_date_format_code(code: &str) -> bool {
    let mut cleaned = String::new();
    let mut in_quote = false;
    for ch in code.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            _ if !in_quote => cleaned.push(ch.to_ascii_lowercase()),
            _ => {}
        }
    }
    cleaned.contains('y')
        || cleaned.contains('d')
        || cleaned.contains("h:")
        || cleaned.contains("m/")
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
