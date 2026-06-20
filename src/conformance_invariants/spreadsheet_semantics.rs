use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::xml_util::{attr_bound_ns, attr_exact, decode_xml_text, xml_general_ref};
use crate::{
    col_name, element_in_ns, local_name, relationships_part_for, resolve_relationship_target,
    zip_text,
};

use super::relationships::parse_relationship_part;
use super::spec::{
    CONTENT_TYPE_XLSX_CALC_CHAIN, CONTENT_TYPE_XLSX_STYLES, CONTENT_TYPE_XLSX_WORKSHEET,
    REL_TYPE_XLSX_WORKSHEET, SPREADSHEETML_NAMESPACE, is_xlsx_workbook_content_type,
};
use super::types::{PartInfo, RelationshipRecord};
use super::util::{diag, normalize_uri};

const OFFICE_RELATIONSHIPS_NS: &[u8] =
    b"http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[derive(Default)]
pub(super) struct SpreadsheetSemanticContext {
    styles: StylesReferenceInfo,
    calc_chain: CalcChainContext,
}

pub(super) fn collect_spreadsheet_semantic_context(
    file: &str,
    entry_set: &BTreeSet<String>,
    parts: &[PartInfo],
) -> SpreadsheetSemanticContext {
    SpreadsheetSemanticContext {
        styles: collect_styles_reference_info(file, parts),
        calc_chain: collect_calc_chain_context(file, entry_set, parts),
    }
}

pub(super) fn check_part_spreadsheet_semantic_invariants(
    file: &str,
    part: &PartInfo,
    context: &SpreadsheetSemanticContext,
) -> Vec<Value> {
    match part.content_type.as_str() {
        ct if is_xlsx_workbook_content_type(ct) => read_workbook_info(file, part)
            .map(|workbook| check_workbook_defined_names(&part.uri, &workbook))
            .unwrap_or_default(),
        CONTENT_TYPE_XLSX_CALC_CHAIN => read_calc_chain_entries(file, part)
            .map(|entries| check_calc_chain_references(&part.uri, &entries, &context.calc_chain))
            .unwrap_or_default(),
        CONTENT_TYPE_XLSX_WORKSHEET => read_worksheet_cells(file, part)
            .map(|cells| check_worksheet_style_references(&part.uri, &cells, &context.styles))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[derive(Clone, Default)]
struct WorkbookInfo {
    sheets: Vec<WorkbookSheetInfo>,
    defined_names: Vec<DefinedNameInfo>,
}

#[derive(Clone, Default)]
struct WorkbookSheetInfo {
    name: String,
    sheet_id: String,
    rid: String,
}

#[derive(Clone, Default)]
struct DefinedNameInfo {
    name: String,
    local_sheet_id: String,
    formula: String,
}

#[derive(Clone, Default)]
struct CalcChainEntry {
    cell_ref: String,
    sheet_id: String,
}

#[derive(Clone, Default)]
struct WorksheetCellInfo {
    cell_ref: String,
    style_index: String,
    has_formula: bool,
}

#[derive(Clone, Default)]
struct CalcChainContext {
    sheet_by_calc_id: BTreeMap<String, String>,
    first_sheet_uri: String,
    formula_cells: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Clone, Default)]
struct StylesReferenceInfo {
    has_part: bool,
    usable: bool,
    cell_xf_count: usize,
}

fn read_workbook_info(file: &str, part: &PartInfo) -> Option<WorkbookInfo> {
    let xml = zip_text(file, &part.entry_name).ok()?;
    let mut reader = NsReader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut workbook = WorkbookInfo::default();
    let mut seen_root = false;
    let mut active_defined_name: Option<DefinedNameInfo> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let in_spreadsheet_ns =
                    element_in_ns(reader.resolver(), &e, SPREADSHEETML_NAMESPACE.as_bytes());
                if stack.is_empty() && !seen_root {
                    seen_root = true;
                    if name != "workbook" || !in_spreadsheet_ns {
                        return None;
                    }
                } else {
                    handle_workbook_element(
                        reader.resolver(),
                        &e,
                        &name,
                        in_spreadsheet_ns,
                        &stack,
                        &mut workbook,
                        &mut active_defined_name,
                    );
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let in_spreadsheet_ns =
                    element_in_ns(reader.resolver(), &e, SPREADSHEETML_NAMESPACE.as_bytes());
                if stack.is_empty() && !seen_root {
                    seen_root = true;
                    if name != "workbook" || !in_spreadsheet_ns {
                        return None;
                    }
                } else {
                    handle_workbook_element(
                        reader.resolver(),
                        &e,
                        &name,
                        in_spreadsheet_ns,
                        &stack,
                        &mut workbook,
                        &mut active_defined_name,
                    );
                    if stack.as_slice() == ["workbook", "definedNames"]
                        && name == "definedName"
                        && let Some(defined_name) = active_defined_name.take()
                    {
                        workbook.defined_names.push(defined_name);
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(defined_name) = active_defined_name.as_mut() {
                    defined_name.formula.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if let Some(defined_name) = active_defined_name.as_mut() {
                    defined_name.formula.push_str(&xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(defined_name) = active_defined_name.as_mut() {
                    defined_name
                        .formula
                        .push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let element_name = e.name();
                let name = local_name(element_name.as_ref());
                if stack.last().map(String::as_str) == Some("definedName")
                    && name == "definedName"
                    && let Some(defined_name) = active_defined_name.take()
                {
                    workbook.defined_names.push(defined_name);
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    seen_root.then_some(workbook)
}

fn handle_workbook_element(
    resolver: &quick_xml::name::NamespaceResolver,
    element: &BytesStart<'_>,
    name: &str,
    in_spreadsheet_ns: bool,
    stack: &[String],
    workbook: &mut WorkbookInfo,
    active_defined_name: &mut Option<DefinedNameInfo>,
) {
    if !in_spreadsheet_ns {
        return;
    }
    if stack == ["workbook", "sheets"] && name == "sheet" {
        workbook.sheets.push(WorkbookSheetInfo {
            name: trim_attr(element, "name"),
            sheet_id: trim_attr(element, "sheetId"),
            rid: relationship_id_attr(element, resolver),
        });
    } else if stack == ["workbook", "definedNames"] && name == "definedName" {
        *active_defined_name = Some(DefinedNameInfo {
            name: trim_attr(element, "name"),
            local_sheet_id: trim_attr(element, "localSheetId"),
            formula: String::new(),
        });
    }
}

fn check_workbook_defined_names(part_uri: &str, workbook: &WorkbookInfo) -> Vec<Value> {
    if workbook.defined_names.is_empty() {
        return Vec::new();
    }
    let sheet_count = workbook.sheets.len();
    let sheet_names = workbook_sheet_names(workbook);
    let mut seen_by_scope = BTreeMap::<String, String>::new();
    let mut diagnostics = Vec::new();

    for (idx, defined_name) in workbook.defined_names.iter().enumerate() {
        let label = defined_name_label(idx + 1, defined_name);
        let name = defined_name.name.trim();
        if name.is_empty() {
            diagnostics.push(diag(
                "XLSX_DEFINED_NAME_REQUIRED",
                format!("{part_uri} {label} is missing required name"),
            ));
        }

        let mut scope_key = "workbook".to_string();
        let raw_scope = defined_name.local_sheet_id.trim();
        if !raw_scope.is_empty() {
            scope_key = format!("sheet:{raw_scope}");
            match raw_scope.parse::<i64>() {
                Ok(local_sheet_id) if local_sheet_id >= 0 => {
                    if local_sheet_id >= sheet_count as i64 {
                        diagnostics.push(diag(
                            "XLSX_DEFINED_NAME_SCOPE",
                            format!(
                                "{part_uri} {label} localSheetId {local_sheet_id} is outside available sheet indexes 0..{}",
                                sheet_count as i64 - 1
                            ),
                        ));
                    }
                }
                _ => diagnostics.push(diag(
                    "XLSX_DEFINED_NAME_SCOPE",
                    format!("{part_uri} {label} has invalid localSheetId {raw_scope:?}"),
                )),
            }
        }

        if !name.is_empty() {
            let seen_key = format!("{}\0{scope_key}", name.to_lowercase());
            if let Some(first) = seen_by_scope.get(&seen_key) {
                diagnostics.push(diag(
                    "XLSX_DEFINED_NAME_DUPLICATE",
                    format!("{part_uri} {label} duplicates {first} in the same scope"),
                ));
            } else {
                seen_by_scope.insert(seen_key, label.clone());
            }
        }

        let formula = defined_name.formula.trim();
        if formula.is_empty() {
            diagnostics.push(diag(
                "XLSX_DEFINED_NAME_REQUIRED",
                format!("{part_uri} {label} has empty formula text"),
            ));
            continue;
        }
        if formula_contains_token_outside_string(formula, "#REF!") {
            diagnostics.push(diag(
                "XLSX_DEFINED_NAME_REFERENCE",
                format!("{part_uri} {label} contains stale #REF! reference"),
            ));
        }
        for sheet_name in extract_defined_name_sheet_references(formula) {
            if !sheet_names.contains(&sheet_name.to_lowercase()) {
                diagnostics.push(diag(
                    "XLSX_DEFINED_NAME_REFERENCE",
                    format!("{part_uri} {label} references missing sheet {sheet_name:?}"),
                ));
            }
        }
    }

    diagnostics
}

fn workbook_sheet_names(workbook: &WorkbookInfo) -> BTreeSet<String> {
    workbook
        .sheets
        .iter()
        .map(|sheet| sheet.name.trim())
        .filter(|name| !name.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn extract_defined_name_sheet_references(formula: &str) -> Vec<String> {
    let trimmed = formula.trim();
    let formula = trimmed.strip_prefix('=').unwrap_or(trimmed).trim();
    let mut refs = Vec::new();
    let mut in_string = false;
    let bytes = formula.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                if in_string && i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                    i += 2;
                    continue;
                }
                in_string = !in_string;
            }
            b'!' if !in_string => {
                if let Some(token) = defined_name_qualifier_before_bang(&formula[..i]) {
                    refs.extend(normalize_defined_name_sheet_qualifier(token));
                }
            }
            _ => {}
        }
        i += 1;
    }
    refs
}

fn formula_contains_token_outside_string(formula: &str, token: &str) -> bool {
    let token = token.as_bytes();
    let mut in_string = false;
    let bytes = formula.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if in_string && i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                i += 2;
                continue;
            }
            in_string = !in_string;
        } else if !in_string && ascii_starts_with_ignore_case(&bytes[i..], token) {
            return true;
        }
        i += 1;
    }
    false
}

fn ascii_starts_with_ignore_case(value: &[u8], prefix: &[u8]) -> bool {
    value.len() >= prefix.len()
        && value[..prefix.len()]
            .iter()
            .zip(prefix)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

fn defined_name_qualifier_before_bang(prefix: &str) -> Option<&str> {
    let bytes = prefix.as_bytes();
    let mut end = bytes.len();
    while end > 0 && matches!(bytes[end - 1], b' ' | b'\t' | b'\r' | b'\n') {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    if bytes[end - 1] == b'\'' {
        let mut i = end - 1;
        while i > 0 {
            i -= 1;
            if bytes[i] != b'\'' {
                continue;
            }
            if i > 0 && bytes[i - 1] == b'\'' {
                i -= 1;
                continue;
            }
            return Some(&prefix[i..end]);
        }
        return None;
    }

    let mut start = end;
    while start > 0 && !defined_name_qualifier_delimiter(bytes[start - 1]) {
        start -= 1;
    }
    let token = prefix[start..end].trim();
    (!token.is_empty()).then_some(token)
}

fn defined_name_qualifier_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b' ' | b'\t'
            | b'\r'
            | b'\n'
            | b','
            | b'('
            | b')'
            | b'+'
            | b'-'
            | b'*'
            | b'/'
            | b'^'
            | b'&'
            | b'='
            | b'<'
            | b'>'
    )
}

fn normalize_defined_name_sheet_qualifier(token: &str) -> Vec<String> {
    let token = token.trim();
    if token.is_empty() || token.contains('[') || token.contains(']') {
        return Vec::new();
    }
    let token = trim_defined_name_sheet_quotes(token);
    token
        .split(':')
        .filter_map(|segment| {
            let name = trim_defined_name_sheet_quotes(segment.trim());
            (!name.is_empty() && !name.eq_ignore_ascii_case("#REF")).then_some(name)
        })
        .collect()
}

fn trim_defined_name_sheet_quotes(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        value[1..value.len() - 1].replace("''", "'")
    } else {
        value.to_string()
    }
}

fn collect_calc_chain_context(
    file: &str,
    entry_set: &BTreeSet<String>,
    parts: &[PartInfo],
) -> CalcChainContext {
    let mut ctx = CalcChainContext::default();
    for part in parts {
        if !is_xlsx_workbook_content_type(&part.content_type) {
            continue;
        }
        let Some(workbook) = read_workbook_info(file, part) else {
            continue;
        };
        let rels = relationships_for_part(file, entry_set, &part.uri);
        let rel_map = relationships_by_id(&rels);
        for (idx, sheet) in workbook.sheets.iter().enumerate() {
            let Some(rel) = rel_map.get(sheet.rid.trim()) else {
                continue;
            };
            if sheet.rid.trim().is_empty()
                || rel.rel_type != REL_TYPE_XLSX_WORKSHEET
                || rel.target_mode.trim().eq_ignore_ascii_case("External")
            {
                continue;
            }
            let sheet_uri = normalize_uri(&resolve_relationship_target(&part.uri, &rel.target));
            if ctx.first_sheet_uri.is_empty() {
                ctx.first_sheet_uri = sheet_uri.clone();
            }
            if !sheet.sheet_id.trim().is_empty() {
                ctx.sheet_by_calc_id
                    .insert(sheet.sheet_id.trim().to_string(), sheet_uri.clone());
            }
            ctx.sheet_by_calc_id
                .entry((idx + 1).to_string())
                .or_insert_with(|| sheet_uri.clone());
            if !ctx.formula_cells.contains_key(&sheet_uri) {
                ctx.formula_cells.insert(
                    sheet_uri.clone(),
                    collect_worksheet_formula_cells(file, &sheet_uri),
                );
            }
        }
    }
    ctx
}

fn read_calc_chain_entries(file: &str, part: &PartInfo) -> Option<Vec<CalcChainEntry>> {
    let xml = zip_text(file, &part.entry_name).ok()?;
    let mut reader = NsReader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut entries = Vec::new();
    let mut seen_root = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let in_spreadsheet_ns =
                    element_in_ns(reader.resolver(), &e, SPREADSHEETML_NAMESPACE.as_bytes());
                if stack.is_empty() && !seen_root {
                    seen_root = true;
                    if name != "calcChain" || !in_spreadsheet_ns {
                        return None;
                    }
                    stack.push(name);
                    continue;
                }
                if in_spreadsheet_ns && stack.as_slice() == ["calcChain"] && name == "c" {
                    entries.push(CalcChainEntry {
                        cell_ref: trim_attr(&e, "r"),
                        sheet_id: trim_attr(&e, "i"),
                    });
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let in_spreadsheet_ns =
                    element_in_ns(reader.resolver(), &e, SPREADSHEETML_NAMESPACE.as_bytes());
                if stack.is_empty() && !seen_root {
                    seen_root = true;
                    if name != "calcChain" || !in_spreadsheet_ns {
                        return None;
                    }
                    continue;
                }
                if in_spreadsheet_ns && stack.as_slice() == ["calcChain"] && name == "c" {
                    entries.push(CalcChainEntry {
                        cell_ref: trim_attr(&e, "r"),
                        sheet_id: trim_attr(&e, "i"),
                    });
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    seen_root.then_some(entries)
}

fn check_calc_chain_references(
    part_uri: &str,
    entries: &[CalcChainEntry],
    ctx: &CalcChainContext,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    let mut current_sheet_uri = String::new();

    for (idx, entry) in entries.iter().enumerate() {
        let label = calc_chain_entry_label(idx + 1, entry);
        let ref_text = entry.cell_ref.trim();
        if ref_text.is_empty() {
            diagnostics.push(diag(
                "XLSX_CALC_CHAIN_REFERENCE",
                format!("{part_uri} {label} is missing required cell reference r"),
            ));
            continue;
        }
        let cell_ref = match normalize_cell_reference(ref_text) {
            Ok(cell_ref) => cell_ref,
            Err(err) => {
                diagnostics.push(diag(
                    "XLSX_CALC_CHAIN_REFERENCE",
                    format!("{part_uri} {label} has invalid cell reference {ref_text:?}: {err}"),
                ));
                continue;
            }
        };

        let raw_sheet_id = entry.sheet_id.trim();
        if !raw_sheet_id.is_empty() {
            let Some(sheet_uri) = ctx.sheet_by_calc_id.get(raw_sheet_id) else {
                diagnostics.push(diag(
                    "XLSX_CALC_CHAIN_REFERENCE",
                    format!(
                        "{part_uri} {label} references unknown sheet id/index {raw_sheet_id:?}"
                    ),
                ));
                current_sheet_uri.clear();
                continue;
            };
            current_sheet_uri = sheet_uri.clone();
        } else if current_sheet_uri.is_empty() {
            current_sheet_uri = ctx.first_sheet_uri.clone();
        }
        if current_sheet_uri.is_empty() {
            diagnostics.push(diag(
                "XLSX_CALC_CHAIN_REFERENCE",
                format!("{part_uri} {label} cannot be resolved to a worksheet"),
            ));
            continue;
        }
        if !ctx
            .formula_cells
            .get(&current_sheet_uri)
            .is_some_and(|formula_cells| formula_cells.contains(&cell_ref))
        {
            diagnostics.push(diag(
                "XLSX_CALC_CHAIN_REFERENCE",
                format!(
                    "{part_uri} {label} points to {current_sheet_uri}!{cell_ref}, but that cell has no formula"
                ),
            ));
        }
    }

    diagnostics
}

fn collect_worksheet_formula_cells(file: &str, sheet_uri: &str) -> BTreeSet<String> {
    let entry_name = sheet_uri.trim_start_matches('/');
    let Some(cells) = read_worksheet_cells_by_entry(file, entry_name) else {
        return BTreeSet::new();
    };
    cells
        .into_iter()
        .filter(|cell| cell.has_formula)
        .filter_map(|cell| normalize_cell_reference(&cell.cell_ref).ok())
        .collect()
}

fn read_worksheet_cells(file: &str, part: &PartInfo) -> Option<Vec<WorksheetCellInfo>> {
    read_worksheet_cells_by_entry(file, &part.entry_name)
}

fn read_worksheet_cells_by_entry(file: &str, entry_name: &str) -> Option<Vec<WorksheetCellInfo>> {
    let xml = zip_text(file, entry_name).ok()?;
    let mut reader = NsReader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut cells = Vec::new();
    let mut seen_root = false;
    let mut active_cell: Option<WorksheetCellInfo> = None;
    let mut active_cell_depth: Option<usize> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let in_spreadsheet_ns =
                    element_in_ns(reader.resolver(), &e, SPREADSHEETML_NAMESPACE.as_bytes());
                if stack.is_empty() && !seen_root {
                    seen_root = true;
                    if name != "worksheet" || !in_spreadsheet_ns {
                        return None;
                    }
                } else if in_spreadsheet_ns && name == "c" {
                    active_cell = Some(WorksheetCellInfo {
                        cell_ref: trim_attr(&e, "r"),
                        style_index: trim_attr(&e, "s"),
                        has_formula: false,
                    });
                    active_cell_depth = Some(stack.len() + 1);
                } else if in_spreadsheet_ns
                    && name == "f"
                    && let Some(cell) = active_cell.as_mut()
                {
                    cell.has_formula = true;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let in_spreadsheet_ns =
                    element_in_ns(reader.resolver(), &e, SPREADSHEETML_NAMESPACE.as_bytes());
                if stack.is_empty() && !seen_root {
                    seen_root = true;
                    if name != "worksheet" || !in_spreadsheet_ns {
                        return None;
                    }
                } else if in_spreadsheet_ns && name == "c" {
                    cells.push(WorksheetCellInfo {
                        cell_ref: trim_attr(&e, "r"),
                        style_index: trim_attr(&e, "s"),
                        has_formula: false,
                    });
                } else if in_spreadsheet_ns
                    && name == "f"
                    && let Some(cell) = active_cell.as_mut()
                {
                    cell.has_formula = true;
                }
            }
            Ok(Event::End(e)) => {
                let element_name = e.name();
                let name = local_name(element_name.as_ref());
                if active_cell_depth == Some(stack.len())
                    && name == "c"
                    && let Some(cell) = active_cell.take()
                {
                    cells.push(cell);
                    active_cell_depth = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    seen_root.then_some(cells)
}

fn collect_styles_reference_info(file: &str, parts: &[PartInfo]) -> StylesReferenceInfo {
    for part in parts {
        if part.content_type != CONTENT_TYPE_XLSX_STYLES {
            continue;
        }
        let mut info = StylesReferenceInfo {
            has_part: true,
            ..StylesReferenceInfo::default()
        };
        let Some(cell_xf_count) = read_styles_cell_xf_count(file, part) else {
            return info;
        };
        info.usable = true;
        info.cell_xf_count = cell_xf_count;
        return info;
    }
    StylesReferenceInfo::default()
}

fn read_styles_cell_xf_count(file: &str, part: &PartInfo) -> Option<usize> {
    let xml = zip_text(file, &part.entry_name).ok()?;
    let mut reader = NsReader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut seen_root = false;
    let mut count = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let in_spreadsheet_ns =
                    element_in_ns(reader.resolver(), &e, SPREADSHEETML_NAMESPACE.as_bytes());
                if stack.is_empty() && !seen_root {
                    seen_root = true;
                    if name != "styleSheet" || !in_spreadsheet_ns {
                        return None;
                    }
                } else if in_spreadsheet_ns
                    && stack.as_slice() == ["styleSheet", "cellXfs"]
                    && name == "xf"
                {
                    count += 1;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let in_spreadsheet_ns =
                    element_in_ns(reader.resolver(), &e, SPREADSHEETML_NAMESPACE.as_bytes());
                if stack.is_empty() && !seen_root {
                    seen_root = true;
                    if name != "styleSheet" || !in_spreadsheet_ns {
                        return None;
                    }
                } else if in_spreadsheet_ns
                    && stack.as_slice() == ["styleSheet", "cellXfs"]
                    && name == "xf"
                {
                    count += 1;
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    seen_root.then_some(count)
}

fn check_worksheet_style_references(
    part_uri: &str,
    cells: &[WorksheetCellInfo],
    styles: &StylesReferenceInfo,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    for cell in cells {
        let raw = cell.style_index.trim();
        if raw.is_empty() {
            continue;
        }
        let label = worksheet_cell_label(cell);
        if !styles.has_part {
            diagnostics.push(diag(
                "XLSX_CELL_STYLE_REFERENCE",
                format!(
                    "{part_uri} {label} has style index {raw:?} but the package has no styles part"
                ),
            ));
            continue;
        }
        if !styles.usable {
            continue;
        }
        let index = match raw.parse::<i64>() {
            Ok(index) if index >= 0 => index,
            _ => {
                diagnostics.push(diag(
                    "XLSX_CELL_STYLE_INDEX_OUT_OF_RANGE",
                    format!(
                        "{part_uri} {label} style index {raw:?} is not a valid non-negative integer"
                    ),
                ));
                continue;
            }
        };
        if index >= styles.cell_xf_count as i64 {
            diagnostics.push(diag(
                "XLSX_CELL_STYLE_INDEX_OUT_OF_RANGE",
                format!(
                    "{part_uri} {label} style index {index} is outside available cellXfs 0..{}",
                    styles.cell_xf_count as i64 - 1
                ),
            ));
        }
    }
    diagnostics
}

fn relationships_for_part(
    file: &str,
    entry_set: &BTreeSet<String>,
    part_uri: &str,
) -> Vec<RelationshipRecord> {
    let rels_part = relationships_part_for(part_uri.trim_start_matches('/'));
    if !entry_set.contains(&normalize_uri(&rels_part)) {
        return Vec::new();
    }
    parse_relationship_part(file, &rels_part).unwrap_or_default()
}

fn relationships_by_id(rels: &[RelationshipRecord]) -> BTreeMap<String, RelationshipRecord> {
    let mut out = BTreeMap::new();
    for rel in rels {
        let id = rel.id.trim();
        if !id.is_empty() {
            out.insert(id.to_string(), rel.clone());
        }
    }
    out
}

fn relationship_id_attr(
    element: &BytesStart<'_>,
    resolver: &quick_xml::name::NamespaceResolver,
) -> String {
    attr_bound_ns(element, resolver, OFFICE_RELATIONSHIPS_NS, b"id")
        .or_else(|| attr_exact(element, "r:id"))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn trim_attr(element: &BytesStart<'_>, name: &str) -> String {
    attr_exact(element, name)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn worksheet_cell_label(cell: &WorksheetCellInfo) -> String {
    let cell_ref = cell.cell_ref.trim();
    if cell_ref.is_empty() {
        "cell".to_string()
    } else {
        format!("cell {cell_ref}")
    }
}

fn calc_chain_entry_label(position: usize, entry: &CalcChainEntry) -> String {
    let cell_ref = entry.cell_ref.trim();
    let sheet_id = entry.sheet_id.trim();
    match (!cell_ref.is_empty(), !sheet_id.is_empty()) {
        (true, true) => format!("<c r={cell_ref:?} i={sheet_id:?}> at position {position}"),
        (true, false) => format!("<c r={cell_ref:?}> at position {position}"),
        (false, true) => format!("<c i={sheet_id:?}> at position {position}"),
        (false, false) => format!("<c> at position {position}"),
    }
}

fn defined_name_label(position: usize, defined_name: &DefinedNameInfo) -> String {
    let name = defined_name.name.trim();
    let scope = defined_name.local_sheet_id.trim();
    match (!name.is_empty(), !scope.is_empty()) {
        (true, true) => {
            format!("<definedName name={name:?} localSheetId={scope:?}> at position {position}")
        }
        (true, false) => format!("<definedName name={name:?}> at position {position}"),
        (false, true) => format!("<definedName localSheetId={scope:?}> at position {position}"),
        (false, false) => format!("<definedName> at position {position}"),
    }
}

#[derive(Default)]
struct ParsedCellReference {
    column: u32,
    row: u32,
    abs_column: bool,
    abs_row: bool,
}

fn normalize_cell_reference(value: &str) -> Result<String, String> {
    let cell = parse_cell_reference(value)?;
    let mut out = String::new();
    if cell.abs_column {
        out.push('$');
    }
    out.push_str(&col_name(cell.column));
    if cell.abs_row {
        out.push('$');
    }
    out.push_str(&cell.row.to_string());
    Ok(out)
}

fn parse_cell_reference(value: &str) -> Result<ParsedCellReference, String> {
    let mut value = value.trim();
    if value.is_empty() {
        return Err("cell reference cannot be empty".to_string());
    }

    let mut cell = ParsedCellReference::default();
    if let Some(rest) = value.strip_prefix('$') {
        cell.abs_column = true;
        value = rest;
        if value.is_empty() {
            return Err("missing column in cell reference".to_string());
        }
    }

    let col_end = value
        .bytes()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    if col_end == 0 {
        return Err("missing column in cell reference".to_string());
    }
    cell.column = column_letters_to_index(&value[..col_end])?;
    value = &value[col_end..];

    if value.is_empty() {
        return Err("missing row in cell reference".to_string());
    }
    if let Some(rest) = value.strip_prefix('$') {
        cell.abs_row = true;
        value = rest;
        if value.is_empty() {
            return Err("missing row in cell reference".to_string());
        }
    }
    if value.contains('$') {
        return Err("invalid absolute marker in row reference".to_string());
    }
    if !value.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("invalid row {value:?} in cell reference"));
    }
    let row = value
        .parse::<u32>()
        .map_err(|err| format!("invalid row {value:?}: {err}"))?;
    if row == 0 || row > 1_048_576 {
        return Err(format!("row {row} out of XLSX bounds 1-1048576"));
    }
    cell.row = row;
    Ok(cell)
}

fn column_letters_to_index(letters: &str) -> Result<u32, String> {
    let letters = letters.trim();
    if letters.is_empty() {
        return Err("column letters cannot be empty".to_string());
    }
    let mut index = 0u32;
    for ch in letters.chars() {
        if !ch.is_ascii_alphabetic() {
            return Err(format!("invalid column letter {ch:?}"));
        }
        index = index * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        if index > 16_384 {
            return Err(format!("column {letters:?} out of XLSX bounds A-XFD"));
        }
    }
    Ok(index)
}
