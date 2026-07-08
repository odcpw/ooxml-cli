use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{
    CliError, CliResult, InspectPackageKind, WorkbookSheet, append_xml_text_event, attr,
    detect_inspect_package_type, find_xlsx_workbook_part, is_xlsx_handle, is_xml_text_event,
    local_name, resolve_sheet, selector_candidates, workbook_sheets, zip_entry_names, zip_text,
};

use super::model::{XlsxDefinedName, XlsxDefinedNameSpan, XlsxDefinedNamesBlock};

pub(super) fn xlsx_defined_names(
    file: &str,
) -> CliResult<(Vec<WorkbookSheet>, Vec<XlsxDefinedName>)> {
    xlsx_defined_names_with_workbook(file).map(|(sheets, names, _, _)| (sheets, names))
}

pub(super) fn xlsx_defined_names_with_workbook(
    file: &str,
) -> CliResult<(Vec<WorkbookSheet>, Vec<XlsxDefinedName>, String, String)> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Xlsx {
        return Err(CliError::unsupported_type(format!(
            "unsupported type: {}",
            inspect_package_kind_label(package_kind)
        )));
    }
    let workbook_part = find_xlsx_workbook_part(file, &entries)?;
    let workbook = zip_text(file, &workbook_part)?;
    let sheets = workbook_sheets(&workbook)?;
    let names = parse_xlsx_defined_names(&workbook, &sheets)?;
    Ok((sheets, names, workbook_part, workbook))
}

fn inspect_package_kind_label(kind: InspectPackageKind) -> &'static str {
    match kind {
        InspectPackageKind::Pptx => "pptx",
        InspectPackageKind::Xlsx => "xlsx",
        InspectPackageKind::Docx => "docx",
        InspectPackageKind::Unknown => "unknown",
    }
}

fn parse_xlsx_defined_names(
    workbook_xml: &str,
    sheets: &[WorkbookSheet],
) -> CliResult<Vec<XlsxDefinedName>> {
    Ok(parse_xlsx_defined_name_block(workbook_xml, sheets)?
        .map(|block| block.names.into_iter().map(|span| span.name).collect())
        .unwrap_or_default())
}

pub(super) fn parse_xlsx_defined_name_block(
    workbook_xml: &str,
    sheets: &[WorkbookSheet],
) -> CliResult<Option<XlsxDefinedNamesBlock>> {
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(false);
    let mut in_defined_names = false;
    let mut defined_names_depth = 0_u32;
    let mut block_start = 0_usize;
    let mut block_end = 0_usize;
    let mut current: Option<XlsxDefinedName> = None;
    let mut current_ref = String::new();
    let mut names = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "definedNames" && current.is_none() {
                    in_defined_names = true;
                    defined_names_depth = 1;
                    block_start = before;
                } else if in_defined_names
                    && current.is_none()
                    && defined_names_depth == 1
                    && name == "definedName"
                {
                    current = Some(xlsx_defined_name_from_element(&e, names.len() + 1, sheets));
                    current_ref.clear();
                    defined_names_depth += 1;
                } else if in_defined_names {
                    defined_names_depth += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "definedNames" {
                    block_start = before;
                    block_end = reader.buffer_position() as usize;
                    break;
                } else if in_defined_names
                    && current.is_none()
                    && defined_names_depth == 1
                    && name == "definedName"
                {
                    let mut item = xlsx_defined_name_from_element(&e, names.len() + 1, sheets);
                    item.apply_selectors();
                    names.push(XlsxDefinedNameSpan { name: item });
                }
            }
            Ok(event) if current.is_some() && is_xml_text_event(&event) => {
                append_xml_text_event(&mut current_ref, &event);
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "definedName" && current.is_some() {
                    let mut item = current.take().expect("defined name current");
                    item.ref_text = current_ref.trim().to_string();
                    item.apply_selectors();
                    names.push(XlsxDefinedNameSpan { name: item });
                    current_ref.clear();
                } else if name == "definedNames" {
                    block_end = reader.buffer_position() as usize;
                    break;
                }
                if in_defined_names && defined_names_depth > 0 {
                    defined_names_depth -= 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if block_end > block_start {
        Ok(Some(XlsxDefinedNamesBlock {
            start: block_start,
            end: block_end,
            names,
        }))
    } else {
        Ok(None)
    }
}

fn xlsx_defined_name_from_element(
    e: &BytesStart<'_>,
    number: usize,
    sheets: &[WorkbookSheet],
) -> XlsxDefinedName {
    let mut name = XlsxDefinedName {
        number: number as u32,
        name: attr(e, "name").unwrap_or_default(),
        scope: "workbook".to_string(),
        ref_text: String::new(),
        hidden: xlsx_bool_attr(attr(e, "hidden").as_deref().unwrap_or_default()),
        comment: attr(e, "comment").unwrap_or_default(),
        description: attr(e, "description").unwrap_or_default(),
        ..XlsxDefinedName::default()
    };
    if let Some(local_sheet_id_text) = attr(e, "localSheetId")
        && let Ok(local_sheet_id) = local_sheet_id_text.parse::<i64>()
    {
        name.scope = "sheet".to_string();
        name.local_sheet_id = Some(local_sheet_id);
        name.sheet_number = if local_sheet_id >= 0 {
            local_sheet_id as u32 + 1
        } else {
            0
        };
        if local_sheet_id >= 0
            && let Some(sheet) = sheets.get(local_sheet_id as usize)
        {
            name.sheet_name = sheet.name.clone();
        }
    }
    name
}

fn xlsx_bool_attr(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true")
}

pub(super) fn filter_xlsx_defined_names_by_scope_sheet(
    sheets: &[WorkbookSheet],
    names: Vec<XlsxDefinedName>,
    scope_sheet: Option<&str>,
) -> CliResult<Vec<XlsxDefinedName>> {
    let Some(scope_sheet) = scope_sheet.filter(|value| !value.trim().is_empty()) else {
        return Ok(names);
    };
    let sheet = resolve_sheet(sheets, scope_sheet)?;
    let local_sheet_id = sheet.position as i64 - 1;
    Ok(names
        .into_iter()
        .filter(|name| name.local_sheet_id == Some(local_sheet_id))
        .collect())
}

pub(super) fn select_xlsx_defined_name(
    sheets: &[WorkbookSheet],
    names: &[XlsxDefinedName],
    selector: &str,
    scope_sheet: Option<&str>,
) -> CliResult<XlsxDefinedName> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(CliError::invalid_args("--name is required"));
    }
    if let Some(handle_name) = parse_xlsx_defined_name_handle(selector)? {
        return resolve_xlsx_defined_name_handle(names, selector, &handle_name);
    }
    let scope_local_sheet_id =
        if let Some(scope_sheet) = scope_sheet.filter(|value| !value.trim().is_empty()) {
            Some(resolve_sheet(sheets, scope_sheet)?.position as i64 - 1)
        } else {
            None
        };
    let matches = names
        .iter()
        .filter(|name| {
            scope_local_sheet_id
                .is_none_or(|local_sheet_id| name.local_sheet_id == Some(local_sheet_id))
        })
        .filter(|name| {
            name.selectors.iter().any(|candidate| candidate == selector)
                || name.name.eq_ignore_ascii_case(selector)
        })
        .cloned()
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => {
            let candidates = selector_candidates(
                &names
                    .iter()
                    .map(|name| (name.primary_selector.as_str(), name.selectors.as_slice()))
                    .collect::<Vec<_>>(),
                selector,
                3,
            );
            let mut message = format!("defined name not found: {selector}");
            if !candidates.is_empty() {
                message.push_str(&format!("; did you mean: {}", candidates.join(", ")));
            }
            message.push_str("; discover with `ooxml --json xlsx names list <file>`");
            Err(CliError::target_not_found(message))
        }
        [name] => Ok(name.clone()),
        _ => Err(CliError::invalid_args(format!(
            "defined name {:?} is ambiguous; use --scope-sheet or one of: {}",
            selector,
            matches
                .iter()
                .map(|name| name.primary_selector.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn parse_xlsx_defined_name_handle(selector: &str) -> CliResult<Option<String>> {
    let selector = selector.trim();
    if !is_xlsx_handle(selector) {
        return Ok(None);
    }
    let body = selector.trim_start_matches("H:");
    let parts = body.split('/').collect::<Vec<_>>();
    if parts.first().copied() != Some("xlsx") {
        return Err(CliError::invalid_args(format!(
            "HANDLE_FORMAT_MISMATCH: handle format tag does not match package format \"xlsx\" (handle {selector:?})"
        )));
    }
    if parts.len() == 3
        && parts[1] == "wb"
        && let Some(name) = parts[2].strip_prefix("name:n:")
    {
        return Ok(Some(name.to_string()));
    }
    Err(CliError::invalid_args(
        "expected a defined-name handle (H:xlsx/wb/name:n:<name>)",
    ))
}

fn resolve_xlsx_defined_name_handle(
    names: &[XlsxDefinedName],
    selector: &str,
    handle_name: &str,
) -> CliResult<XlsxDefinedName> {
    let matches = names
        .iter()
        .filter(|name| name.local_sheet_id.is_none() && name.name == handle_name)
        .cloned()
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [name] => Ok(name.clone()),
        [] => Err(CliError::target_not_found(format!(
            "HANDLE_STALE: no defined name {:?} in workbook (selector {:?})",
            handle_name, selector
        ))),
        _ => Err(CliError::invalid_args(format!(
            "HANDLE_AMBIGUOUS: defined name {:?} is not unique in workbook (selector {:?})",
            handle_name, selector
        ))),
    }
}
