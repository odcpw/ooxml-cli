use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, InspectPackageKind, RangeBounds, WorkbookSheet, add_selector, attr,
    col_name, command_arg, copy_zip_with_part_overrides, decode_xml_text,
    detect_inspect_package_type, find_xlsx_workbook_part, is_xlsx_handle, local_name,
    parse_cell_ref, parse_cli_range, resolve_sheet, selector_candidates, validate,
    validate_xlsx_mutation_output_flags, workbook_sheets, xlsx_ranges_set_temp_path,
    xlsx_source_command, xml_attr_escape, xml_escape, zip_entry_names, zip_text,
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

#[derive(Clone)]
struct XlsxDefinedNameSpan {
    name: XlsxDefinedName,
}

struct XlsxDefinedNamesBlock {
    start: usize,
    end: usize,
    names: Vec<XlsxDefinedNameSpan>,
}

pub(crate) struct XlsxNameMutationOptions<'a> {
    pub(crate) name: Option<&'a str>,
    pub(crate) new_name: Option<&'a str>,
    pub(crate) ref_: Option<&'a str>,
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: Option<&'a str>,
    pub(crate) scope_sheet: Option<&'a str>,
    pub(crate) expect_ref: Option<&'a str>,
    pub(crate) hidden: bool,
    pub(crate) comment: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
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

pub(crate) fn xlsx_names_add(file: &str, options: XlsxNameMutationOptions<'_>) -> CliResult<Value> {
    xlsx_names_mutate(file, "add", options)
}

pub(crate) fn xlsx_names_update(
    file: &str,
    options: XlsxNameMutationOptions<'_>,
) -> CliResult<Value> {
    xlsx_names_mutate(file, "update", options)
}

pub(crate) fn xlsx_names_rename(
    file: &str,
    options: XlsxNameMutationOptions<'_>,
) -> CliResult<Value> {
    xlsx_names_mutate(file, "rename", options)
}

pub(crate) fn xlsx_names_delete(
    file: &str,
    options: XlsxNameMutationOptions<'_>,
) -> CliResult<Value> {
    xlsx_names_mutate(file, "delete", options)
}

fn xlsx_names_mutate(
    file: &str,
    action: &str,
    options: XlsxNameMutationOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheets, names, workbook_part, workbook_xml) = xlsx_defined_names_with_workbook(file)?;
    let mut updated_names = names.clone();
    let mut changed_name: Option<XlsxDefinedName> = None;
    let mut deleted_name: Option<XlsxDefinedName> = None;
    let mut previous_name = String::new();
    let mut previous_ref = String::new();

    match action {
        "add" => {
            let name = options
                .name
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| CliError::invalid_args("--name is required"))?;
            validate_defined_name(name).map_err(|message| {
                CliError::invalid_args(format!("failed to add defined name: {message}"))
            })?;
            let ref_text = resolve_defined_name_ref_from_flags(
                &sheets,
                options.ref_,
                options.sheet,
                options.range,
            )?;
            let local_sheet_id = resolve_defined_name_scope(&sheets, options.scope_sheet)?;
            if duplicate_defined_name(&updated_names, name, local_sheet_id, None) {
                return Err(CliError::invalid_args(format!(
                    "failed to add defined name: defined name {:?} already exists in {} scope",
                    name,
                    defined_name_scope_text(local_sheet_id)
                )));
            }
            let mut added = XlsxDefinedName {
                number: updated_names.len() as u32 + 1,
                name: name.to_string(),
                scope: defined_name_scope_text(local_sheet_id).to_string(),
                local_sheet_id,
                ref_text,
                hidden: options.hidden,
                comment: options.comment.unwrap_or("").trim().to_string(),
                ..XlsxDefinedName::default()
            };
            apply_defined_name_sheet_context(&mut added, &sheets);
            added.apply_selectors();
            updated_names.push(added.clone());
            changed_name = Some(added);
        }
        "update" => {
            let target = select_xlsx_defined_name(
                &sheets,
                &updated_names,
                options.name.unwrap_or(""),
                options.scope_sheet,
            )?;
            let ref_text = resolve_defined_name_ref_from_flags(
                &sheets,
                options.ref_,
                options.sheet,
                options.range,
            )?;
            let index = defined_name_index(&updated_names, &target)?;
            previous_ref = updated_names[index].ref_text.trim().to_string();
            check_expected_defined_name_ref(&previous_ref, options.expect_ref).map_err(
                |message| {
                    CliError::invalid_args(format!("failed to update defined name: {message}"))
                },
            )?;
            updated_names[index].ref_text = ref_text;
            updated_names[index].apply_selectors();
            changed_name = Some(updated_names[index].clone());
        }
        "rename" => {
            let target = select_xlsx_defined_name(
                &sheets,
                &updated_names,
                options.name.unwrap_or(""),
                options.scope_sheet,
            )?;
            let new_name = options.new_name.unwrap_or("").trim();
            validate_defined_name(new_name).map_err(|message| {
                CliError::invalid_args(format!("failed to rename defined name: {message}"))
            })?;
            let index = defined_name_index(&updated_names, &target)?;
            previous_ref = updated_names[index].ref_text.trim().to_string();
            check_expected_defined_name_ref(&previous_ref, options.expect_ref).map_err(
                |message| {
                    CliError::invalid_args(format!("failed to rename defined name: {message}"))
                },
            )?;
            let local_sheet_id = updated_names[index].local_sheet_id;
            if duplicate_defined_name(&updated_names, new_name, local_sheet_id, Some(index)) {
                return Err(CliError::invalid_args(format!(
                    "failed to rename defined name: defined name {:?} already exists in {} scope",
                    new_name,
                    defined_name_scope_text(local_sheet_id)
                )));
            }
            previous_name = updated_names[index].name.clone();
            updated_names[index].name = new_name.to_string();
            updated_names[index].apply_selectors();
            changed_name = Some(updated_names[index].clone());
        }
        "delete" => {
            let target = select_xlsx_defined_name(
                &sheets,
                &updated_names,
                options.name.unwrap_or(""),
                options.scope_sheet,
            )?;
            let index = defined_name_index(&updated_names, &target)?;
            previous_ref = updated_names[index].ref_text.trim().to_string();
            check_expected_defined_name_ref(&previous_ref, options.expect_ref).map_err(
                |message| {
                    CliError::invalid_args(format!("failed to delete defined name: {message}"))
                },
            )?;
            deleted_name = Some(updated_names.remove(index));
            renumber_defined_names(&mut updated_names);
        }
        _ => {
            return Err(CliError::unexpected(format!(
                "unknown name mutation action {action:?}"
            )));
        }
    }

    let updated_workbook_xml = rewrite_workbook_defined_names(&workbook_xml, &updated_names)?;
    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        xlsx_ranges_set_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    let mut overrides = BTreeMap::new();
    overrides.insert(workbook_part, updated_workbook_xml);
    copy_zip_with_part_overrides(file, &readback_path, &overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }

    let readback_changed = if let Some(changed) = &changed_name {
        Some(readback_xlsx_defined_name(
            &readback_path,
            &changed.name,
            changed.local_sheet_id,
        )?)
    } else {
        None
    };
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }

    let readback_file = commit_path.unwrap_or("");
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("action".to_string(), json!(action));
    if let Some(name) = &readback_changed {
        result.insert(
            "name".to_string(),
            xlsx_defined_name_item_json(readback_file, name, None),
        );
    }
    if let Some(name) = &deleted_name {
        result.insert(
            "deleted".to_string(),
            xlsx_defined_name_item_json(file, name, None),
        );
    }
    if !previous_name.is_empty() {
        result.insert("previousName".to_string(), json!(previous_name));
    }
    if !previous_ref.is_empty() && action == "update" {
        result.insert("previousRef".to_string(), json!(previous_ref));
    }
    let commands = xlsx_name_mutation_readback_commands(readback_file, readback_changed.as_ref());
    result.extend(commands);
    Ok(Value::Object(result))
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
    xlsx_defined_names_with_workbook(file).map(|(sheets, names, _, _)| (sheets, names))
}

fn xlsx_defined_names_with_workbook(
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

fn parse_xlsx_defined_name_block(
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

fn xlsx_name_mutation_readback_commands(
    file: &str,
    name: Option<&XlsxDefinedName>,
) -> Map<String, Value> {
    let mut object = Map::new();
    if file.trim().is_empty() {
        let placeholder = "<out.xlsx>";
        object.insert(
            "validateCommandTemplate".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(placeholder)
            )),
        );
        object.insert(
            "namesListCommandTemplate".to_string(),
            json!(xlsx_names_list_command(placeholder)),
        );
        if let Some(name) = name {
            object.insert(
                "nameShowCommandTemplate".to_string(),
                json!(xlsx_name_show_command(placeholder, name)),
            );
        }
    } else {
        object.insert(
            "validateCommand".to_string(),
            json!(format!("ooxml validate --strict {}", command_arg(file))),
        );
        object.insert(
            "namesListCommand".to_string(),
            json!(xlsx_names_list_command(file)),
        );
        if let Some(name) = name {
            object.insert(
                "nameShowCommand".to_string(),
                json!(xlsx_name_show_command(file, name)),
            );
        }
    }
    object
}

fn xlsx_names_list_command(file: &str) -> String {
    format!("ooxml --json xlsx names list {}", command_arg(file))
}

fn readback_xlsx_defined_name(
    file: &str,
    name: &str,
    local_sheet_id: Option<i64>,
) -> CliResult<XlsxDefinedName> {
    let (_, names) = xlsx_defined_names(file)?;
    names
        .into_iter()
        .find(|candidate| {
            candidate.name.eq_ignore_ascii_case(name) && candidate.local_sheet_id == local_sheet_id
        })
        .ok_or_else(|| {
            CliError::unexpected(format!("changed defined name {name:?} did not read back"))
        })
}

fn resolve_defined_name_ref_from_flags(
    sheets: &[WorkbookSheet],
    exact_ref: Option<&str>,
    sheet_selector: Option<&str>,
    range_text: Option<&str>,
) -> CliResult<String> {
    let exact_ref = exact_ref.unwrap_or("").trim();
    let range_text = range_text.unwrap_or("").trim();
    if exact_ref.is_empty() == range_text.is_empty() {
        return Err(CliError::invalid_args(
            "must specify exactly one of --ref or --range",
        ));
    }
    if !exact_ref.is_empty() {
        return normalize_defined_name_ref(exact_ref).map_err(CliError::invalid_args);
    }
    let sheet_selector = sheet_selector.unwrap_or("").trim();
    if sheet_selector.is_empty() {
        return Err(CliError::invalid_args(
            "--sheet is required when using --range",
        ));
    }
    let sheet = resolve_sheet(sheets, sheet_selector)?;
    let range = parse_cli_range(range_text)?;
    Ok(format!(
        "{}!{}",
        quote_defined_name_sheet(&sheet.name),
        absolute_range_ref(range)
    ))
}

fn resolve_defined_name_scope(
    sheets: &[WorkbookSheet],
    scope_sheet: Option<&str>,
) -> CliResult<Option<i64>> {
    let Some(scope_sheet) = scope_sheet.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let sheet = resolve_sheet(sheets, scope_sheet)?;
    Ok(Some(sheet.position as i64 - 1))
}

fn apply_defined_name_sheet_context(name: &mut XlsxDefinedName, sheets: &[WorkbookSheet]) {
    if let Some(local_sheet_id) = name.local_sheet_id {
        name.scope = "sheet".to_string();
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
    } else {
        name.scope = "workbook".to_string();
        name.sheet_number = 0;
        name.sheet_name.clear();
    }
}

fn validate_defined_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("defined name cannot be empty".to_string());
    }
    if name.chars().count() > 255 {
        return Err(format!(
            "defined name {:?} exceeds Excel's 255-character limit",
            name
        ));
    }
    let first = name.chars().next().unwrap_or_default();
    if !(first.is_alphabetic() || first == '_' || first == '\\') {
        return Err(format!(
            "defined name {:?} must start with a letter, underscore, or backslash",
            name
        ));
    }
    for ch in name.chars() {
        if ch.is_alphabetic() || ch.is_ascii_digit() || ch == '_' || ch == '.' || ch == '\\' {
            continue;
        }
        return Err(format!(
            "defined name {:?} contains invalid characters",
            name
        ));
    }
    if parse_cell_ref(name).is_ok() {
        return Err(format!(
            "defined name {:?} cannot be an A1 cell reference",
            name
        ));
    }
    if is_r1c1_name(name) {
        return Err(format!(
            "defined name {:?} cannot be an R1C1 cell reference",
            name
        ));
    }
    Ok(())
}

fn is_r1c1_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    let Some(rest) = upper.strip_prefix('R') else {
        return false;
    };
    let Some((row, col)) = rest.split_once('C') else {
        return false;
    };
    !row.is_empty()
        && !col.is_empty()
        && row.chars().all(|ch| ch.is_ascii_digit())
        && col.chars().all(|ch| ch.is_ascii_digit())
}

fn normalize_defined_name_ref(ref_text: &str) -> Result<String, String> {
    let normalized = ref_text
        .trim()
        .strip_prefix('=')
        .unwrap_or(ref_text.trim())
        .trim()
        .to_string();
    if normalized.is_empty() {
        Err("defined name ref cannot be empty".to_string())
    } else {
        Ok(normalized)
    }
}

fn quote_defined_name_sheet(sheet_name: &str) -> String {
    format!("'{}'", sheet_name.replace('\'', "''"))
}

fn absolute_range_ref(range: RangeBounds) -> String {
    let range = range.normalized();
    let start = format!("${}${}", col_name(range.start_col), range.start_row);
    let end = format!("${}${}", col_name(range.end_col), range.end_row);
    if start == end {
        start
    } else {
        format!("{start}:{end}")
    }
}

fn duplicate_defined_name(
    names: &[XlsxDefinedName],
    name: &str,
    local_sheet_id: Option<i64>,
    skip_index: Option<usize>,
) -> bool {
    names.iter().enumerate().any(|(index, candidate)| {
        skip_index != Some(index)
            && candidate.name.eq_ignore_ascii_case(name)
            && candidate.local_sheet_id == local_sheet_id
    })
}

fn defined_name_scope_text(local_sheet_id: Option<i64>) -> &'static str {
    if local_sheet_id.is_some() {
        "sheet"
    } else {
        "workbook"
    }
}

fn defined_name_index(names: &[XlsxDefinedName], target: &XlsxDefinedName) -> CliResult<usize> {
    if target.number > 0 {
        let index = target.number as usize - 1;
        if let Some(candidate) = names.get(index)
            && candidate.name.eq_ignore_ascii_case(&target.name)
            && candidate.local_sheet_id == target.local_sheet_id
        {
            return Ok(index);
        }
    }
    names
        .iter()
        .position(|candidate| {
            candidate.name.eq_ignore_ascii_case(&target.name)
                && candidate.local_sheet_id == target.local_sheet_id
        })
        .ok_or_else(|| {
            CliError::unexpected(format!(
                "defined name {:?} is no longer present in {} scope",
                target.name, target.scope
            ))
        })
}

fn check_expected_defined_name_ref(actual: &str, expected: Option<&str>) -> Result<(), String> {
    let expected = expected
        .unwrap_or("")
        .trim()
        .strip_prefix('=')
        .unwrap_or(expected.unwrap_or("").trim())
        .trim();
    if expected.is_empty() {
        return Ok(());
    }
    if actual != expected {
        Err(format!(
            "defined name ref mismatch: expected {:?}, found {:?}",
            expected, actual
        ))
    } else {
        Ok(())
    }
}

fn renumber_defined_names(names: &mut [XlsxDefinedName]) {
    for (index, name) in names.iter_mut().enumerate() {
        name.number = index as u32 + 1;
        name.apply_selectors();
    }
}

fn rewrite_workbook_defined_names(
    workbook_xml: &str,
    names: &[XlsxDefinedName],
) -> CliResult<String> {
    let rendered = render_defined_names_block(workbook_xml, names);
    if let Some(block) = parse_xlsx_defined_name_block(workbook_xml, &[])? {
        let mut out = String::with_capacity(workbook_xml.len() + rendered.len());
        out.push_str(&workbook_xml[..block.start]);
        out.push_str(&rendered);
        out.push_str(&workbook_xml[block.end..]);
        return Ok(out);
    }
    if rendered.is_empty() {
        return Ok(workbook_xml.to_string());
    }
    let insert_at = workbook_defined_names_insert_position(workbook_xml)
        .ok_or_else(|| CliError::unexpected("could not locate workbook insertion point"))?;
    let mut out = String::with_capacity(workbook_xml.len() + rendered.len());
    out.push_str(&workbook_xml[..insert_at]);
    out.push_str(&rendered);
    out.push_str(&workbook_xml[insert_at..]);
    Ok(out)
}

fn render_defined_names_block(workbook_xml: &str, names: &[XlsxDefinedName]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let prefix = workbook_element_prefix(workbook_xml);
    let wrapper = xml_qualified_name(prefix.as_deref(), "definedNames");
    let mut out = String::new();
    out.push_str(&format!("<{wrapper}>"));
    for name in names {
        out.push_str(&render_defined_name_element(prefix.as_deref(), name));
    }
    out.push_str(&format!("</{wrapper}>"));
    out
}

fn render_defined_name_element(prefix: Option<&str>, name: &XlsxDefinedName) -> String {
    let tag = xml_qualified_name(prefix, "definedName");
    let mut attrs = format!(r#" name="{}""#, xml_attr_escape(&name.name));
    if let Some(local_sheet_id) = name.local_sheet_id {
        attrs.push_str(&format!(r#" localSheetId="{local_sheet_id}""#));
    }
    if name.hidden {
        attrs.push_str(r#" hidden="1""#);
    }
    if !name.comment.trim().is_empty() {
        attrs.push_str(&format!(
            r#" comment="{}""#,
            xml_attr_escape(name.comment.trim())
        ));
    }
    if !name.description.trim().is_empty() {
        attrs.push_str(&format!(
            r#" description="{}""#,
            xml_attr_escape(name.description.trim())
        ));
    }
    format!("<{tag}{attrs}>{}</{tag}>", xml_escape(&name.ref_text))
}

fn workbook_element_prefix(workbook_xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "workbook" =>
            {
                let raw = String::from_utf8_lossy(e.name().as_ref()).to_string();
                return raw.split_once(':').map(|(prefix, _)| prefix.to_string());
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn xml_qualified_name(prefix: Option<&str>, local: &str) -> String {
    match prefix.filter(|value| !value.is_empty()) {
        Some(prefix) => format!("{prefix}:{local}"),
        None => local.to_string(),
    }
}

fn workbook_defined_names_insert_position(workbook_xml: &str) -> Option<usize> {
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(false);
    let mut depth = 0_u32;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if depth == 1 && workbook_child_order(&name) > workbook_child_order("definedNames")
                {
                    return Some(before);
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if depth == 1 && workbook_child_order(&name) > workbook_child_order("definedNames")
                {
                    return Some(before);
                }
            }
            Ok(Event::End(e)) => {
                if depth == 1 && local_name(e.name().as_ref()) == "workbook" {
                    return Some(before);
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn workbook_child_order(local_name: &str) -> i32 {
    match local_name {
        "fileVersion" => 10,
        "fileSharing" => 20,
        "workbookPr" => 30,
        "workbookProtection" => 40,
        "bookViews" => 50,
        "sheets" => 60,
        "functionGroups" => 70,
        "externalReferences" => 80,
        "definedNames" => 90,
        "calcPr" => 100,
        "oleSize" => 110,
        "customWorkbookViews" => 120,
        "pivotCaches" => 130,
        "smartTagPr" => 140,
        "smartTagTypes" => 150,
        "webPublishing" => 160,
        "fileRecoveryPr" => 170,
        "webPublishObjects" => 180,
        "extLst" => 190,
        _ => 1000,
    }
}
