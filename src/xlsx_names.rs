use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, InspectPackageKind, WorkbookSheet, add_selector, attr, command_arg,
    decode_xml_text, detect_inspect_package_type, find_xlsx_workbook_part, is_xlsx_handle,
    local_name, resolve_sheet, selector_candidates, workbook_sheets, xlsx_source_command,
    zip_entry_names, zip_text,
};

#[derive(Clone, Default)]
struct XlsxDefinedName {
    number: u32,
    name: String,
    scope: String,
    local_sheet_id: Option<i64>,
    sheet_number: u32,
    sheet_name: String,
    ref_text: String,
    hidden: bool,
    comment: String,
    description: String,
    primary_selector: String,
    selectors: Vec<String>,
}

impl XlsxDefinedName {
    fn apply_selectors(&mut self) {
        self.primary_selector = if self.scope == "workbook" && !self.name.trim().is_empty() {
            format!("name:{}", self.name)
        } else if self.scope == "sheet" && self.sheet_number > 0 && !self.name.trim().is_empty() {
            format!("sheet:{}/name:{}", self.sheet_number, self.name)
        } else if self.number > 0 {
            format!("definedName:{}", self.number)
        } else {
            String::new()
        };

        let mut selectors = Vec::new();
        add_selector(&mut selectors, self.primary_selector.clone());
        if self.number > 0 {
            add_selector(&mut selectors, format!("definedName:{}", self.number));
            add_selector(&mut selectors, format!("#{}", self.number));
        }
        if !self.name.trim().is_empty() {
            add_selector(&mut selectors, format!("name:{}", self.name));
            add_selector(&mut selectors, format!("~{}", self.name));
            add_selector(&mut selectors, self.name.clone());
        }
        if self.scope == "workbook" && !self.name.trim().is_empty() {
            add_selector(&mut selectors, format!("scope:workbook/name:{}", self.name));
            add_selector(&mut selectors, format!("workbook:{}", self.name));
        }
        if self.scope == "sheet" && !self.name.trim().is_empty() {
            if self.sheet_number > 0 {
                add_selector(
                    &mut selectors,
                    format!("scope:sheet:{}/name:{}", self.sheet_number, self.name),
                );
                add_selector(
                    &mut selectors,
                    format!("sheet:{}/name:{}", self.sheet_number, self.name),
                );
            }
            if !self.sheet_name.trim().is_empty() {
                add_selector(
                    &mut selectors,
                    format!("scope:sheet:{}/name:{}", self.sheet_name, self.name),
                );
                add_selector(
                    &mut selectors,
                    format!("sheet:{}/name:{}", self.sheet_name, self.name),
                );
            }
        }
        self.selectors = selectors;
    }
}

pub(crate) fn xlsx_names_list(file: &str, scope_sheet: Option<&str>) -> CliResult<Value> {
    let (sheets, names) = xlsx_defined_names(file)?;
    let names = filter_xlsx_defined_names_by_scope_sheet(&sheets, names, scope_sheet)?;
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "names": xlsx_defined_name_items_json(file, &names),
    }))
}

pub(crate) fn xlsx_names_show(
    file: &str,
    selector: &str,
    scope_sheet: Option<&str>,
) -> CliResult<Value> {
    let (sheets, names) = xlsx_defined_names(file)?;
    let name = select_xlsx_defined_name(&sheets, &names, selector, scope_sheet)?;
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "name": xlsx_defined_name_item_json(file, &name, None),
    }))
}

fn xlsx_defined_names(file: &str) -> CliResult<(Vec<WorkbookSheet>, Vec<XlsxDefinedName>)> {
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
    Ok((sheets, names))
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
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(false);
    let mut in_defined_names = false;
    let mut current: Option<XlsxDefinedName> = None;
    let mut current_ref = String::new();
    let mut names = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "definedNames" && current.is_none() {
                    in_defined_names = true;
                } else if in_defined_names && current.is_none() && name == "definedName" {
                    current = Some(xlsx_defined_name_from_element(&e, names.len() + 1, sheets));
                    current_ref.clear();
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "definedNames" {
                    in_defined_names = false;
                } else if in_defined_names && current.is_none() && name == "definedName" {
                    let mut item = xlsx_defined_name_from_element(&e, names.len() + 1, sheets);
                    item.apply_selectors();
                    names.push(item);
                }
            }
            Ok(Event::Text(e)) => {
                if current.is_some() {
                    current_ref.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "definedName" && current.is_some() {
                    let mut item = current.take().expect("defined name current");
                    item.ref_text = current_ref.trim().to_string();
                    item.apply_selectors();
                    names.push(item);
                    current_ref.clear();
                } else if name == "definedNames" {
                    in_defined_names = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(names)
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

fn filter_xlsx_defined_names_by_scope_sheet(
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

fn select_xlsx_defined_name(
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

fn xlsx_defined_name_items_json(file: &str, names: &[XlsxDefinedName]) -> Vec<Value> {
    let counts = workbook_scoped_defined_name_counts(names);
    names
        .iter()
        .map(|name| xlsx_defined_name_item_json(file, name, Some(&counts)))
        .collect()
}

fn xlsx_defined_name_item_json(
    file: &str,
    name: &XlsxDefinedName,
    counts: Option<&BTreeMap<String, usize>>,
) -> Value {
    let mut object = Map::new();
    object.insert("number".to_string(), json!(name.number));
    object.insert("name".to_string(), json!(name.name));
    object.insert("scope".to_string(), json!(name.scope));
    if let Some(local_sheet_id) = name.local_sheet_id {
        object.insert("localSheetId".to_string(), json!(local_sheet_id));
    }
    if name.sheet_number > 0 {
        object.insert("sheetNumber".to_string(), json!(name.sheet_number));
    }
    if !name.sheet_name.is_empty() {
        object.insert("sheetName".to_string(), json!(name.sheet_name));
    }
    object.insert("ref".to_string(), json!(name.ref_text));
    if name.hidden {
        object.insert("hidden".to_string(), json!(true));
    }
    if !name.comment.is_empty() {
        object.insert("comment".to_string(), json!(name.comment));
    }
    if !name.description.is_empty() {
        object.insert("description".to_string(), json!(name.description));
    }
    let unique =
        counts.is_none_or(|counts| counts.get(&name.name).copied().unwrap_or_default() == 1);
    if unique && name.scope == "workbook" && !name.name.trim().is_empty() {
        object.insert(
            "handle".to_string(),
            json!(format!("H:xlsx/wb/name:n:{}", name.name)),
        );
    }
    if !name.primary_selector.is_empty() {
        object.insert("primarySelector".to_string(), json!(name.primary_selector));
    }
    if !name.selectors.is_empty() {
        object.insert("selectors".to_string(), json!(name.selectors));
    }
    if !file.is_empty() {
        object.insert(
            "showCommand".to_string(),
            json!(xlsx_name_show_command(file, name)),
        );
    }
    Value::Object(object)
}

fn workbook_scoped_defined_name_counts(names: &[XlsxDefinedName]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for name in names {
        if name.scope == "workbook" && !name.name.trim().is_empty() {
            *counts.entry(name.name.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn xlsx_name_show_command(file: &str, name: &XlsxDefinedName) -> String {
    let mut command = xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "names", "show", file],
        &[("--name", &name.name)],
    );
    if name.scope == "sheet" && name.sheet_number > 0 {
        command.push_str(&format!(
            " --scope-sheet {}",
            command_arg(&format!("sheet:{}", name.sheet_number))
        ));
    }
    command
}
