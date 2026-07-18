use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, ResolveResult};
use quick_xml::reader::NsReader;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;

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

const SPREADSHEETML_TRANSITIONAL_NS: &[u8] =
    b"http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const SPREADSHEETML_STRICT_NS: &[u8] = b"http://purl.oclc.org/ooxml/spreadsheetml/main";

#[derive(Clone, Debug, Default)]
pub(crate) struct RawCellValue {
    pub(crate) cell_type: String,
    pub(crate) raw_value: String,
    pub(crate) inline_text: String,
    pub(crate) formula: String,
    pub(crate) style_index: Option<u32>,
}

pub(crate) struct RawRangeScan {
    pub(crate) cells: BTreeMap<(u32, u32), RawCellValue>,
    pub(crate) saw_nonzero_style_index: bool,
    pub(crate) saw_style_zero: bool,
}

#[derive(Clone)]
struct SpreadsheetFrame {
    local_name: String,
    valid_namespace: bool,
}

pub(crate) fn sheet_raw_cells_in_range(
    source: &mut dyn BufRead,
    bounds: RangeBounds,
    output_cell_limit: Option<u64>,
) -> CliResult<RawRangeScan> {
    let mut reader = NsReader::from_reader(source);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut stack = Vec::<SpreadsheetFrame>::new();
    let mut saw_root = false;
    let mut allow_unbound = false;
    let mut active_cell: Option<(usize, u32, u32, RawCellValue)> = None;
    let mut value_depth = None;
    let mut inline_text_depth = None;
    let mut formula_depth = None;
    let mut cells = BTreeMap::new();
    let mut saw_nonzero_style_index = false;
    let mut saw_style_zero = false;

    loop {
        let (resolved, event) = reader
            .read_resolved_event_into(&mut buffer)
            .map_err(|err| CliError::unexpected(format!("invalid worksheet XML: {err}")))?;
        match event {
            Event::Start(element) => {
                let local = local_name(element.name().as_ref()).to_string();
                let namespace = spreadsheet_namespace_kind(resolved);
                if stack.is_empty() {
                    if saw_root {
                        return Err(CliError::unexpected(
                            "invalid worksheet XML: multiple root elements",
                        ));
                    }
                    if local != "worksheet" || namespace == SpreadsheetNamespace::Foreign {
                        return Err(CliError::unexpected(format!(
                            "invalid worksheet root element {local:?} or namespace"
                        )));
                    }
                    saw_root = true;
                    allow_unbound = namespace == SpreadsheetNamespace::Unbound;
                }
                let valid_namespace = namespace == SpreadsheetNamespace::Spreadsheet
                    || (allow_unbound && namespace == SpreadsheetNamespace::Unbound);
                let depth = stack.len();

                if local == "c"
                    && valid_namespace
                    && worksheet_parent_path(&stack, &["worksheet", "sheetData", "row"])
                {
                    active_cell =
                        raw_cell_from_element(&element, bounds).and_then(|(col, row, raw)| {
                            if let Some(style_index) = raw.style_index {
                                saw_nonzero_style_index |= style_index != 0;
                                saw_style_zero |= style_index == 0;
                            }
                            coordinate_in_output_prefix(bounds, col, row, output_cell_limit)
                                .then_some((depth, col, row, raw))
                        });
                } else if active_cell.is_some()
                    && local == "v"
                    && valid_namespace
                    && worksheet_parent_path(&stack, &["worksheet", "sheetData", "row", "c"])
                {
                    value_depth = Some(depth);
                } else if active_cell.is_some()
                    && local == "f"
                    && valid_namespace
                    && worksheet_parent_path(&stack, &["worksheet", "sheetData", "row", "c"])
                {
                    formula_depth = Some(depth);
                } else if active_cell.is_some()
                    && local == "t"
                    && valid_namespace
                    && worksheet_inline_text_parent_path(&stack)
                {
                    inline_text_depth = Some(depth);
                }

                stack.push(SpreadsheetFrame {
                    local_name: local,
                    valid_namespace,
                });
            }
            Event::Empty(element) => {
                let local = local_name(element.name().as_ref()).to_string();
                let namespace = spreadsheet_namespace_kind(resolved);
                if stack.is_empty() {
                    if saw_root {
                        return Err(CliError::unexpected(
                            "invalid worksheet XML: multiple root elements",
                        ));
                    }
                    if local != "worksheet" || namespace == SpreadsheetNamespace::Foreign {
                        return Err(CliError::unexpected(format!(
                            "invalid worksheet root element {local:?} or namespace"
                        )));
                    }
                    saw_root = true;
                    allow_unbound = namespace == SpreadsheetNamespace::Unbound;
                }
                let valid_namespace = namespace == SpreadsheetNamespace::Spreadsheet
                    || (allow_unbound && namespace == SpreadsheetNamespace::Unbound);
                if local == "c"
                    && valid_namespace
                    && worksheet_parent_path(&stack, &["worksheet", "sheetData", "row"])
                    && let Some((col, row, raw)) = raw_cell_from_element(&element, bounds)
                {
                    if let Some(style_index) = raw.style_index {
                        saw_nonzero_style_index |= style_index != 0;
                        saw_style_zero |= style_index == 0;
                    }
                    if coordinate_in_output_prefix(bounds, col, row, output_cell_limit) {
                        cells.insert((col, row), raw);
                    }
                }
            }
            event if is_xml_text_event(&event) => {
                if let Some((_, _, _, raw)) = active_cell.as_mut() {
                    if value_depth.is_some() {
                        append_xml_text_event(&mut raw.raw_value, &event);
                    } else if inline_text_depth.is_some() {
                        append_xml_text_event(&mut raw.inline_text, &event);
                    } else if formula_depth.is_some() {
                        append_xml_text_event(&mut raw.formula, &event);
                    }
                }
            }
            Event::End(element) => {
                let depth = stack.len().checked_sub(1).ok_or_else(|| {
                    CliError::unexpected("invalid worksheet XML: unmatched end element")
                })?;
                let local = local_name(element.name().as_ref()).to_string();
                let frame = stack.last().ok_or_else(|| {
                    CliError::unexpected("invalid worksheet XML: unmatched end element")
                })?;
                if frame.local_name != local {
                    return Err(CliError::unexpected(format!(
                        "invalid worksheet XML: closing {local:?} does not match {:?}",
                        frame.local_name
                    )));
                }
                if value_depth == Some(depth) {
                    value_depth = None;
                }
                if inline_text_depth == Some(depth) {
                    inline_text_depth = None;
                }
                if formula_depth == Some(depth) {
                    formula_depth = None;
                }
                if matches!(active_cell, Some((cell_depth, _, _, _)) if cell_depth == depth)
                    && local == "c"
                {
                    let (_, col, row, raw) = active_cell.take().expect("active cell");
                    cells.insert((col, row), raw);
                }
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }

    if !saw_root {
        return Err(CliError::unexpected(
            "invalid worksheet XML: no root element",
        ));
    }
    if !stack.is_empty() {
        return Err(CliError::unexpected(
            "invalid worksheet XML: unexpected EOF",
        ));
    }
    Ok(RawRangeScan {
        cells,
        saw_nonzero_style_index,
        saw_style_zero,
    })
}

pub(crate) fn shared_strings_for_indices(
    source: &mut dyn BufRead,
    wanted_indices: &BTreeSet<usize>,
) -> CliResult<BTreeMap<usize, String>> {
    let mut reader = NsReader::from_reader(source);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut stack = Vec::<SpreadsheetFrame>::new();
    let mut saw_root = false;
    let mut allow_unbound = false;
    let mut string_index = 0usize;
    let mut active_string: Option<(usize, String)> = None;
    let mut text_depth = None;
    let mut strings = BTreeMap::new();

    loop {
        let (resolved, event) = reader
            .read_resolved_event_into(&mut buffer)
            .map_err(|err| CliError::unexpected(format!("invalid shared strings XML: {err}")))?;
        match event {
            Event::Start(element) => {
                let local = local_name(element.name().as_ref()).to_string();
                let namespace = spreadsheet_namespace_kind(resolved);
                if stack.is_empty() {
                    if saw_root {
                        return Err(CliError::unexpected(
                            "invalid shared strings XML: multiple root elements",
                        ));
                    }
                    if local != "sst" || namespace == SpreadsheetNamespace::Foreign {
                        return Err(CliError::unexpected(format!(
                            "invalid shared strings root element {local:?} or namespace"
                        )));
                    }
                    saw_root = true;
                    allow_unbound = namespace == SpreadsheetNamespace::Unbound;
                }
                let valid_namespace = namespace == SpreadsheetNamespace::Spreadsheet
                    || (allow_unbound && namespace == SpreadsheetNamespace::Unbound);
                let depth = stack.len();
                if local == "si" && valid_namespace && shared_string_parent_path(&stack, &["sst"]) {
                    if wanted_indices.contains(&string_index) {
                        active_string = Some((depth, String::new()));
                    }
                } else if active_string.is_some()
                    && local == "t"
                    && valid_namespace
                    && shared_string_text_parent_path(&stack)
                {
                    text_depth = Some(depth);
                }
                stack.push(SpreadsheetFrame {
                    local_name: local,
                    valid_namespace,
                });
            }
            Event::Empty(element) => {
                let local = local_name(element.name().as_ref()).to_string();
                let namespace = spreadsheet_namespace_kind(resolved);
                if stack.is_empty() {
                    if saw_root {
                        return Err(CliError::unexpected(
                            "invalid shared strings XML: multiple root elements",
                        ));
                    }
                    if local != "sst" || namespace == SpreadsheetNamespace::Foreign {
                        return Err(CliError::unexpected(format!(
                            "invalid shared strings root element {local:?} or namespace"
                        )));
                    }
                    saw_root = true;
                    allow_unbound = namespace == SpreadsheetNamespace::Unbound;
                } else if local == "si"
                    && (namespace == SpreadsheetNamespace::Spreadsheet
                        || (allow_unbound && namespace == SpreadsheetNamespace::Unbound))
                    && shared_string_parent_path(&stack, &["sst"])
                {
                    if wanted_indices.contains(&string_index) {
                        strings.insert(string_index, String::new());
                    }
                    string_index = string_index.saturating_add(1);
                }
            }
            event if is_xml_text_event(&event) => {
                if text_depth.is_some()
                    && let Some((_, text)) = active_string.as_mut()
                {
                    append_xml_text_event(text, &event);
                }
            }
            Event::End(element) => {
                let depth = stack.len().checked_sub(1).ok_or_else(|| {
                    CliError::unexpected("invalid shared strings XML: unmatched end element")
                })?;
                let local = local_name(element.name().as_ref()).to_string();
                let frame = stack.last().ok_or_else(|| {
                    CliError::unexpected("invalid shared strings XML: unmatched end element")
                })?;
                if frame.local_name != local {
                    return Err(CliError::unexpected(format!(
                        "invalid shared strings XML: closing {local:?} does not match {:?}",
                        frame.local_name
                    )));
                }
                if text_depth == Some(depth) {
                    text_depth = None;
                }
                let closes_shared_string = local == "si"
                    && frame.valid_namespace
                    && stack.len() == 2
                    && shared_string_parent_path(&stack[..1], &["sst"]);
                if closes_shared_string {
                    if matches!(active_string, Some((si_depth, _)) if si_depth == depth) {
                        let (_, text) = active_string.take().expect("active shared string");
                        strings.insert(string_index, text);
                    }
                    string_index = string_index.saturating_add(1);
                }
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }

    if !saw_root {
        return Err(CliError::unexpected(
            "invalid shared strings XML: no root element",
        ));
    }
    if !stack.is_empty() {
        return Err(CliError::unexpected(
            "invalid shared strings XML: unexpected EOF",
        ));
    }
    Ok(strings)
}

pub(crate) fn decode_xlsx_raw_cell(
    raw: &RawCellValue,
    shared_strings: &BTreeMap<usize, String>,
    styles: &[XlsxStyle],
) -> CellValue {
    let style = raw
        .style_index
        .and_then(|index| styles.get(index as usize).cloned())
        .unwrap_or_default();
    let (kind, matrix_value, display_value) = decode_xlsx_cell_value_with_lookup(
        &raw.cell_type,
        &raw.raw_value,
        &raw.inline_text,
        &raw.formula,
        |index| shared_strings.get(&index).cloned(),
        &style,
    );
    CellValue {
        kind,
        matrix_value,
        display_value,
        raw_value: if raw.cell_type == "inlineStr" {
            String::new()
        } else {
            raw.raw_value.clone()
        },
        formula: raw.formula.clone(),
        style_index: raw.style_index,
        number_format_id: style.number_format_id,
        number_format_code: style.number_format_code,
        date_style: style.date_style,
        has_formula: !raw.formula.is_empty(),
    }
}

fn raw_cell_from_element(
    element: &BytesStart<'_>,
    bounds: RangeBounds,
) -> Option<(u32, u32, RawCellValue)> {
    let raw_ref = attr(element, "r")?;
    let (col, row) = parse_cell_ref(&raw_ref).ok()?;
    if raw_ref != format!("{}{}", col_name(col), row) || !range_contains_cell(bounds, col, row) {
        return None;
    }
    Some((
        col,
        row,
        RawCellValue {
            cell_type: attr(element, "t").unwrap_or_default(),
            style_index: attr(element, "s").and_then(|value| value.parse::<u32>().ok()),
            ..RawCellValue::default()
        },
    ))
}

fn coordinate_in_output_prefix(
    bounds: RangeBounds,
    col: u32,
    row: u32,
    output_cell_limit: Option<u64>,
) -> bool {
    let normalized = bounds.normalized();
    let ordinal = u64::from(row - normalized.start_row)
        .saturating_mul(u64::from(normalized.col_count()))
        .saturating_add(u64::from(col - normalized.start_col));
    output_cell_limit.is_none_or(|limit| ordinal < limit)
}

fn worksheet_parent_path(stack: &[SpreadsheetFrame], path: &[&str]) -> bool {
    stack.len() == path.len()
        && stack
            .iter()
            .zip(path)
            .all(|(frame, expected)| frame.valid_namespace && frame.local_name == *expected)
}

fn worksheet_inline_text_parent_path(stack: &[SpreadsheetFrame]) -> bool {
    stack.len() >= 5
        && worksheet_parent_path(&stack[..4], &["worksheet", "sheetData", "row", "c"])
        && stack[4].valid_namespace
        && stack[4].local_name == "is"
        && stack[5..].iter().all(|frame| frame.valid_namespace)
}

fn shared_string_parent_path(stack: &[SpreadsheetFrame], path: &[&str]) -> bool {
    worksheet_parent_path(stack, path)
}

fn shared_string_text_parent_path(stack: &[SpreadsheetFrame]) -> bool {
    stack.len() >= 2
        && shared_string_parent_path(&stack[..2], &["sst", "si"])
        && stack[2..].iter().all(|frame| frame.valid_namespace)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpreadsheetNamespace {
    Spreadsheet,
    Unbound,
    Foreign,
}

fn spreadsheet_namespace_kind(resolved: ResolveResult<'_>) -> SpreadsheetNamespace {
    match resolved {
        ResolveResult::Bound(Namespace(uri))
            if uri == SPREADSHEETML_TRANSITIONAL_NS || uri == SPREADSHEETML_STRICT_NS =>
        {
            SpreadsheetNamespace::Spreadsheet
        }
        ResolveResult::Unbound => SpreadsheetNamespace::Unbound,
        _ => SpreadsheetNamespace::Foreign,
    }
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
    decode_xlsx_cell_value_with_lookup(
        cell_type,
        raw,
        inline_text,
        formula,
        |index| shared_strings.get(index).cloned(),
        style,
    )
}

fn decode_xlsx_cell_value_with_lookup(
    cell_type: &str,
    raw: &str,
    inline_text: &str,
    formula: &str,
    shared_string: impl Fn(usize) -> Option<String>,
    style: &XlsxStyle,
) -> (String, Value, String) {
    match cell_type {
        "s" => {
            let idx = raw.parse::<usize>().unwrap_or(usize::MAX);
            let text = shared_string(idx).unwrap_or_default();
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

#[cfg(test)]
mod streaming_tests {
    use super::*;
    use std::io::{BufReader, Cursor, Read};

    struct ChunkedReader<R> {
        inner: R,
        chunk_size: usize,
    }

    impl<R: Read> Read for ChunkedReader<R> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let len = buffer.len().min(self.chunk_size);
            self.inner.read(&mut buffer[..len])
        }
    }

    fn parse_sheet(
        xml: &str,
        range: &str,
        limit: Option<u64>,
    ) -> CliResult<BTreeMap<(u32, u32), RawCellValue>> {
        let chunked = ChunkedReader {
            inner: Cursor::new(xml.as_bytes()),
            chunk_size: 3,
        };
        let mut reader = BufReader::with_capacity(5, chunked);
        sheet_raw_cells_in_range(&mut reader, parse_range(range)?, limit).map(|scan| scan.cells)
    }

    #[test]
    fn streamed_sheet_reader_handles_prefixed_transitional_and_strict_xml() {
        for namespace in [
            "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
            "http://purl.oclc.org/ooxml/spreadsheetml/main",
        ] {
            let xml = format!(
                r#"<s:worksheet xmlns:s="{namespace}"><s:sheetData><s:row r="1"><s:c r="A1" t="inlineStr"><s:is><s:r><s:t>rich &amp; </s:t></s:r><s:r><s:t>text</s:t></s:r></s:is></s:c><s:c r="B1"><s:f>SUM(1,2)</s:f><s:v>3</s:v></s:c></s:row></s:sheetData></s:worksheet>"#
            );
            let cells = parse_sheet(&xml, "A1:B1", None).expect("streamed worksheet");
            assert_eq!(cells[&(1, 1)].inline_text, "rich & text");
            assert_eq!(cells[&(2, 1)].formula, "SUM(1,2)");
            assert_eq!(cells[&(2, 1)].raw_value, "3");
        }
    }

    #[test]
    fn streamed_sheet_reader_is_ancestry_and_namespace_aware() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x="urn:foreign"><x:sheetData><x:row><x:c r="A1"><x:v>bad-wrapper</x:v></x:c></x:row></x:sheetData><sheetData><row><x:c r="A1"><x:v>bad-cell</x:v></x:c><c r="A1"><x:v>bad-value</x:v><v>good</v></c></row></sheetData></worksheet>"#;
        let cells = parse_sheet(xml, "A1", None).expect("worksheet");
        assert_eq!(cells[&(1, 1)].raw_value, "good");
    }

    #[test]
    fn streamed_sheet_reader_preserves_last_duplicate_and_coordinate_prefix() {
        let xml = r#"<worksheet><sheetData><row r="2"><c r="B2"><v>outside-prefix</v></c><c r="A2"><v>row-two</v></c></row><row r="1"><c r="A1"><v>first</v></c><c r="a1"><v>lowercase</v></c><c r="$A$1"><v>absolute</v></c><c r="A1"><v>last</v></c><c r="C1"><v>third</v></c></row></sheetData></worksheet>"#;
        let cells = parse_sheet(xml, "C2:A1", Some(4)).expect("worksheet");
        assert_eq!(cells[&(1, 1)].raw_value, "last");
        assert_eq!(cells[&(1, 2)].raw_value, "row-two");
        assert_eq!(cells[&(3, 1)].raw_value, "third");
        assert!(!cells.contains_key(&(2, 2)));
        assert_eq!(cells.len(), 3);
    }

    #[test]
    fn streamed_sheet_reader_fails_on_malformed_tail() {
        let xml = r#"<worksheet><sheetData><row><c r="A1"><v>1</v></c></row></sheetData><broken"#;
        let err = parse_sheet(xml, "A1", None).expect_err("malformed tail must fail");
        assert!(err.message.contains("invalid worksheet XML"));
    }

    #[test]
    fn streamed_shared_strings_are_sparse_rich_and_namespace_aware() {
        let xml = r#"<s:sst xmlns:s="http://purl.oclc.org/ooxml/spreadsheetml/main" xmlns:x="urn:foreign"><s:si><s:t>zero</s:t></s:si><x:si><x:t>does not consume an index</x:t></x:si><s:si><x:t>ignore</x:t><s:r><s:t>want &amp; </s:t></s:r><s:r><s:t>this</s:t></s:r></s:si><s:si><s:t>two</s:t></s:si></s:sst>"#;
        let chunked = ChunkedReader {
            inner: Cursor::new(xml.as_bytes()),
            chunk_size: 2,
        };
        let mut reader = BufReader::with_capacity(4, chunked);
        let strings =
            shared_strings_for_indices(&mut reader, &BTreeSet::from([1])).expect("shared strings");
        assert_eq!(strings, BTreeMap::from([(1, "want & this".to_string())]));
    }

    #[test]
    fn streamed_shared_strings_fail_on_malformed_tail() {
        let xml = r#"<sst><si><t>complete</t></si><broken"#;
        let mut reader = BufReader::new(Cursor::new(xml.as_bytes()));
        let err = shared_strings_for_indices(&mut reader, &BTreeSet::from([0]))
            .expect_err("malformed shared strings tail must fail");
        assert!(err.message.contains("invalid shared strings XML"));
    }
}
