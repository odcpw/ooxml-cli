use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, RelationshipEntry, WorkbookSheet, add_relationship_to_xml,
    allocate_relationship_id, command_arg, copy_zip_with_part_overrides_and_removals,
    ensure_content_type_override, local_name, normalize_xl_target, relationships,
    relationships_part_for, render_xml_attrs, replace_xml_span, resolve_relationship_target,
    resolve_sheet, validate, validate_xlsx_mutation_output_flags, workbook_sheets,
    xlsx_ranges_set_temp_path, xlsx_sheet_selectors, xml_attr_escape, xml_attrs_map, zip_text,
};

const REL_WORKSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
const REL_CALC_CHAIN: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain";
const CONTENT_TYPE_WORKSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
const CONTENT_TYPE_CALC_CHAIN: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml";
const SHEET_ID_RANDOM_CEILING: u32 = 65_534;

pub(crate) struct XlsxSheetsAddOptions<'a> {
    pub(crate) name: Option<&'a str>,
    pub(crate) after: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxSheetsRenameOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) name: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxSheetsMoveOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) to: Option<i64>,
    pub(crate) to_present: bool,
    pub(crate) before: Option<&'a str>,
    pub(crate) after: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxSheetsDeleteOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

struct XlsxSheetMutationWrite<'a> {
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
    in_place: bool,
}

struct XlsxSheetMutationPackage {
    readback_path: String,
    commit_path: Option<String>,
}

struct XlsxSheetLifecycleContext {
    workbook_xml: String,
    sheets: Vec<WorkbookSheet>,
    rels_xml: String,
    rels_part: String,
    rels: Vec<RelationshipEntry>,
    rel_targets: BTreeMap<String, String>,
    content_types_xml: String,
    entries: BTreeSet<String>,
}

struct SheetElementSpan {
    start: usize,
    end: usize,
    rel_id: String,
    attrs: BTreeMap<String, String>,
    fragment: String,
}

struct ElementOpenSpan {
    name: String,
    attrs: BTreeMap<String, String>,
    empty: bool,
}

pub(crate) fn xlsx_sheets_add(file: &str, options: XlsxSheetsAddOptions<'_>) -> CliResult<Value> {
    require_existing_file(file)?;
    let name = options
        .name
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::invalid_args("--name is required"))?;
    let write = XlsxSheetMutationWrite {
        out: options.out,
        backup: options.backup,
        dry_run: options.dry_run,
        no_validate: options.no_validate,
        in_place: options.in_place,
    };
    validate_xlsx_mutation_output_flags(write.out, write.in_place, write.backup, write.dry_run)?;

    let ctx = load_sheet_lifecycle_context(file)?;
    validate_new_sheet_name(name, &ctx.sheets, "")?;
    let after_position = if let Some(after) = options.after.filter(|value| !value.is_empty()) {
        resolve_sheet(&ctx.sheets, after)?.position as usize
    } else {
        0
    };
    if after_position > ctx.sheets.len() {
        return Err(CliError::invalid_args(format!(
            "after position {after_position} out of range"
        )));
    }
    let sheet_id = allocate_sheet_id(&ctx.sheets)?;
    let rel_id = allocate_relationship_id(&ctx.rels);
    let part_uri = next_worksheet_part_uri(&ctx.entries);
    let part_name = part_uri.trim_start_matches('/').to_string();
    let sheet_number = inserted_sheet_number(ctx.sheets.len(), after_position);
    let sheet_xml = new_worksheet_xml(workbook_prefix(&ctx.workbook_xml).as_deref());
    let updated_workbook =
        insert_workbook_sheet(&ctx.workbook_xml, name, sheet_id, &rel_id, after_position)?;
    let updated_rels = add_relationship_to_xml(
        ctx.rels_xml.clone(),
        &rel_id,
        REL_WORKSHEET,
        &relationship_target("xl/workbook.xml", &part_uri),
    );
    let updated_content_types = ensure_content_type_override(
        ctx.content_types_xml.clone(),
        &part_uri,
        CONTENT_TYPE_WORKSHEET,
    );

    let mut overrides = BTreeMap::new();
    overrides.insert("xl/workbook.xml".to_string(), updated_workbook);
    overrides.insert(ctx.rels_part.clone(), updated_rels);
    overrides.insert("[Content_Types].xml".to_string(), updated_content_types);
    overrides.insert(part_name, sheet_xml);
    let removals = BTreeSet::new();
    let package = write_sheet_mutation_package(file, &write, overrides, removals)?;
    let destination = collect_xlsx_sheets_mutation_destination(
        &package.readback_path,
        package.commit_path.as_deref(),
        Some(&rel_id),
        Some(&part_uri),
    )?;
    finish_sheet_mutation_package(file, &write, &package)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("number".to_string(), json!(sheet_number));
    result.insert("name".to_string(), json!(name));
    result.insert("sheetId".to_string(), json!(sheet_id.to_string()));
    result.insert("relationshipId".to_string(), json!(rel_id));
    result.insert("partUri".to_string(), json!(part_uri));
    if let Some(commit_path) = package.commit_path.as_deref() {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(write.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_sheets_mutation_readback_commands(
        &mut result,
        package.commit_path.as_deref(),
        Some(sheet_id),
    );
    Ok(Value::Object(result))
}

pub(crate) fn xlsx_sheets_rename(
    file: &str,
    options: XlsxSheetsRenameOptions<'_>,
) -> CliResult<Value> {
    require_existing_file(file)?;
    let sheet_selector = options
        .sheet
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::invalid_args("--sheet is required"))?;
    let name = options
        .name
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::invalid_args("--name is required"))?;
    let write = XlsxSheetMutationWrite {
        out: options.out,
        backup: options.backup,
        dry_run: options.dry_run,
        no_validate: options.no_validate,
        in_place: options.in_place,
    };
    validate_xlsx_mutation_output_flags(write.out, write.in_place, write.backup, write.dry_run)?;

    let ctx = load_sheet_lifecycle_context(file)?;
    let sheet = resolve_sheet(&ctx.sheets, sheet_selector)?;
    validate_new_sheet_name(name, &ctx.sheets, &sheet.name)?;
    let part_uri = sheet_part_uri(&sheet, &ctx.rel_targets)?;
    let updated_workbook = replace_sheet_element_attr(&ctx.workbook_xml, &sheet.rel_id, |attrs| {
        attrs.insert("name".to_string(), name.to_string());
    })?;
    let mut overrides = BTreeMap::new();
    overrides.insert("xl/workbook.xml".to_string(), updated_workbook);
    let removals = BTreeSet::new();
    let package = write_sheet_mutation_package(file, &write, overrides, removals)?;
    let destination = collect_xlsx_sheets_mutation_destination(
        &package.readback_path,
        package.commit_path.as_deref(),
        Some(&sheet.rel_id),
        Some(&part_uri),
    )?;
    finish_sheet_mutation_package(file, &write, &package)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("number".to_string(), json!(sheet.position));
    result.insert("name".to_string(), json!(name));
    result.insert("previousName".to_string(), json!(sheet.name));
    result.insert("sheetId".to_string(), json!(sheet.sheet_id.to_string()));
    result.insert("relationshipId".to_string(), json!(sheet.rel_id));
    result.insert("partUri".to_string(), json!(part_uri));
    if let Some(commit_path) = package.commit_path.as_deref() {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(write.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_sheets_mutation_readback_commands(
        &mut result,
        package.commit_path.as_deref(),
        Some(sheet.sheet_id),
    );
    Ok(Value::Object(result))
}

pub(crate) fn xlsx_sheets_move(file: &str, options: XlsxSheetsMoveOptions<'_>) -> CliResult<Value> {
    require_existing_file(file)?;
    let sheet_selector = options
        .sheet
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::invalid_args("--sheet is required"))?;
    let write = XlsxSheetMutationWrite {
        out: options.out,
        backup: options.backup,
        dry_run: options.dry_run,
        no_validate: options.no_validate,
        in_place: options.in_place,
    };
    validate_xlsx_mutation_output_flags(write.out, write.in_place, write.backup, write.dry_run)?;

    let ctx = load_sheet_lifecycle_context(file)?;
    let sheet = resolve_sheet(&ctx.sheets, sheet_selector)?;
    let target_position = resolve_move_target_position(
        &ctx.sheets,
        &sheet,
        options.to,
        options.to_present,
        options.before,
        options.after,
    )?;
    let part_uri = sheet_part_uri(&sheet, &ctx.rel_targets)?;
    let old_index = sheet.position as usize - 1;
    let new_index = target_position - 1;
    let remap = move_sheet_position_remap(ctx.sheets.len(), old_index, new_index);
    let moved = reorder_workbook_sheets(&ctx.workbook_xml, &sheet.rel_id, new_index)?;
    let updated_workbook = apply_sheet_position_remap(&moved, &remap);
    let mut overrides = BTreeMap::new();
    overrides.insert("xl/workbook.xml".to_string(), updated_workbook);
    let removals = BTreeSet::new();
    let package = write_sheet_mutation_package(file, &write, overrides, removals)?;
    let destination = collect_xlsx_sheets_mutation_destination(
        &package.readback_path,
        package.commit_path.as_deref(),
        Some(&sheet.rel_id),
        Some(&part_uri),
    )?;
    finish_sheet_mutation_package(file, &write, &package)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("number".to_string(), json!(target_position));
    result.insert("name".to_string(), json!(sheet.name));
    result.insert("sheetId".to_string(), json!(sheet.sheet_id.to_string()));
    result.insert("relationshipId".to_string(), json!(sheet.rel_id));
    result.insert("partUri".to_string(), json!(part_uri));
    result.insert("fromPosition".to_string(), json!(sheet.position));
    result.insert("toPosition".to_string(), json!(target_position));
    result.insert("isNoOp".to_string(), json!(old_index == new_index));
    if let Some(commit_path) = package.commit_path.as_deref() {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(write.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_sheets_mutation_readback_commands(
        &mut result,
        package.commit_path.as_deref(),
        Some(sheet.sheet_id),
    );
    Ok(Value::Object(result))
}

pub(crate) fn xlsx_sheets_delete(
    file: &str,
    options: XlsxSheetsDeleteOptions<'_>,
) -> CliResult<Value> {
    require_existing_file(file)?;
    let sheet_selector = options
        .sheet
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::invalid_args("--sheet is required"))?;
    let write = XlsxSheetMutationWrite {
        out: options.out,
        backup: options.backup,
        dry_run: options.dry_run,
        no_validate: options.no_validate,
        in_place: options.in_place,
    };
    validate_xlsx_mutation_output_flags(write.out, write.in_place, write.backup, write.dry_run)?;

    let ctx = load_sheet_lifecycle_context(file)?;
    let sheet = resolve_sheet(&ctx.sheets, sheet_selector)?;
    if ctx.sheets.len() <= 1 {
        return Err(CliError::invalid_args("cannot delete the last sheet"));
    }
    let part_uri = sheet_part_uri(&sheet, &ctx.rel_targets)?;
    if !part_uri.starts_with("/xl/worksheets/") {
        return Err(CliError::invalid_args(format!(
            "sheet {:?} is not a worksheet",
            sheet.name
        )));
    }
    if visible_sheet_count(&ctx.sheets) <= 1 && sheet.state == "visible" {
        return Err(CliError::invalid_args(
            "cannot delete the last visible sheet",
        ));
    }

    let delete_index = sheet.position as usize - 1;
    let remap = delete_sheet_position_remap(ctx.sheets.len(), delete_index);
    let without_sheet = remove_workbook_sheet(&ctx.workbook_xml, &sheet.rel_id)?;
    let updated_workbook = apply_sheet_position_remap(&without_sheet, &remap);
    let (updated_rels, rel_removed_parts) =
        remove_sheet_and_calc_chain_relationships(&ctx.rels_xml, "xl/workbook.xml", &sheet.rel_id);
    let mut removed_parts = vec![part_uri.clone()];
    let sheet_rels_part = relationships_part_for(part_uri.trim_start_matches('/'));
    if ctx
        .entries
        .contains(&format!("/{}", sheet_rels_part.trim_start_matches('/')))
    {
        removed_parts.push(format!("/{}", sheet_rels_part.trim_start_matches('/')));
    }
    for part in rel_removed_parts {
        if !removed_parts.iter().any(|existing| existing == &part) {
            removed_parts.push(part);
        }
    }

    let mut removals = BTreeSet::new();
    for part in &removed_parts {
        removals.insert(part.trim_start_matches('/').to_string());
    }
    let updated_content_types =
        remove_content_type_overrides(&ctx.content_types_xml, &removed_parts, true);
    let mut overrides = BTreeMap::new();
    overrides.insert("xl/workbook.xml".to_string(), updated_workbook);
    overrides.insert(ctx.rels_part.clone(), updated_rels);
    overrides.insert("[Content_Types].xml".to_string(), updated_content_types);
    let package = write_sheet_mutation_package(file, &write, overrides, removals)?;
    let destination = collect_xlsx_sheets_mutation_destination(
        &package.readback_path,
        package.commit_path.as_deref(),
        None,
        None,
    )?;
    finish_sheet_mutation_package(file, &write, &package)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("number".to_string(), json!(sheet.position));
    result.insert("name".to_string(), json!(sheet.name));
    result.insert("sheetId".to_string(), json!(sheet.sheet_id.to_string()));
    result.insert("relationshipId".to_string(), json!(sheet.rel_id));
    result.insert("partUri".to_string(), json!(part_uri));
    result.insert("removedRelationshipId".to_string(), json!(sheet.rel_id));
    result.insert("removedParts".to_string(), json!(removed_parts));
    result.insert("remainingSheets".to_string(), json!(ctx.sheets.len() - 1));
    if let Some(commit_path) = package.commit_path.as_deref() {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(write.dry_run));
    result.insert(
        "deleted".to_string(),
        xlsx_sheet_ref_json(&sheet, &sheet_part_uri(&sheet, &ctx.rel_targets)?),
    );
    result.insert("destination".to_string(), destination);
    add_xlsx_sheets_mutation_readback_commands(&mut result, package.commit_path.as_deref(), None);
    Ok(Value::Object(result))
}

fn require_existing_file(file: &str) -> CliResult<()> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    Ok(())
}

fn load_sheet_lifecycle_context(file: &str) -> CliResult<XlsxSheetLifecycleContext> {
    let workbook_xml = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook_xml)?;
    let rels_part = "xl/_rels/workbook.xml.rels".to_string();
    let rels_xml = zip_text(file, &rels_part)?;
    let rels = crate::opc::relationship_entries(file, &rels_part)?;
    let rel_targets = relationships(file, &rels_part)?;
    let content_types_xml = zip_text(file, "[Content_Types].xml")?;
    let entries = crate::zip_entry_set(&crate::zip_entry_names(file)?);
    Ok(XlsxSheetLifecycleContext {
        workbook_xml,
        sheets,
        rels_xml,
        rels_part,
        rels,
        rel_targets,
        content_types_xml,
        entries,
    })
}

fn write_sheet_mutation_package(
    file: &str,
    write: &XlsxSheetMutationWrite<'_>,
    overrides: BTreeMap<String, String>,
    removals: BTreeSet<String>,
) -> CliResult<XlsxSheetMutationPackage> {
    let output_path = write.out.filter(|value| !value.trim().is_empty());
    let commit_path = if write.in_place {
        Some(file.to_string())
    } else {
        output_path.map(str::to_string)
    };
    let readback_path = if write.dry_run || write.in_place || output_path == Some(file) {
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

    copy_zip_with_part_overrides_and_removals(file, &readback_path, &overrides, &removals)?;
    if !write.no_validate {
        validate(&readback_path, true)?;
    }
    Ok(XlsxSheetMutationPackage {
        readback_path,
        commit_path,
    })
}

fn finish_sheet_mutation_package(
    file: &str,
    write: &XlsxSheetMutationWrite<'_>,
    package: &XlsxSheetMutationPackage,
) -> CliResult<()> {
    if write.dry_run {
        let _ = fs::remove_file(&package.readback_path);
    } else if write.in_place || write.out.filter(|value| !value.trim().is_empty()) == Some(file) {
        if let Some(backup_path) = write.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&package.readback_path, file)
            .or_else(|_| {
                fs::copy(&package.readback_path, file)?;
                fs::remove_file(&package.readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

fn collect_xlsx_sheets_mutation_destination(
    readback_file: &str,
    destination_file: Option<&str>,
    affected_rel_id: Option<&str>,
    affected_part_uri: Option<&str>,
) -> CliResult<Value> {
    let workbook_xml = zip_text(readback_file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook_xml)?;
    let rel_targets = relationships(readback_file, "xl/_rels/workbook.xml.rels")?;
    let mut destination = Map::new();
    if let Some(file) = destination_file {
        destination.insert("file".to_string(), json!(file));
    }
    let mut sheet_values = Vec::new();
    let mut selected = None;
    for sheet in &sheets {
        let part_uri = sheet_part_uri(sheet, &rel_targets)?;
        let item = xlsx_sheet_ref_json(sheet, &part_uri);
        if affected_rel_id.is_some_and(|rel_id| rel_id == sheet.rel_id)
            || affected_part_uri.is_some_and(|part| part == part_uri)
        {
            selected = Some(item.clone());
        }
        sheet_values.push(item);
    }
    if let Some(selected) = selected {
        destination.insert("sheet".to_string(), selected);
    }
    destination.insert("sheets".to_string(), Value::Array(sheet_values));
    destination.insert("sheetCount".to_string(), json!(sheets.len()));
    Ok(Value::Object(destination))
}

fn xlsx_sheet_ref_json(sheet: &WorkbookSheet, part_uri: &str) -> Value {
    json!({
        "number": sheet.position,
        "position": sheet.position,
        "name": sheet.name,
        "sheetId": sheet.sheet_id.to_string(),
        "state": sheet.state,
        "relationshipId": sheet.rel_id,
        "partUri": part_uri,
        "relationshipType": REL_WORKSHEET,
        "primarySelector": format!("sheetId:{}", sheet.sheet_id),
        "selectors": xlsx_sheet_selectors(&sheet.name, sheet.sheet_id, sheet.position, &sheet.rel_id, part_uri),
    })
}

fn add_xlsx_sheets_mutation_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    affected_sheet_id: Option<u32>,
) {
    let target = output_path.unwrap_or("<out.xlsx>");
    let suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    result.insert(
        format!("validateCommand{suffix}"),
        json!(format!("ooxml validate --strict {}", command_arg(target))),
    );
    result.insert(
        format!("sheetsListCommand{suffix}"),
        json!(format!(
            "ooxml --json xlsx sheets list {}",
            command_arg(target)
        )),
    );
    if let Some(sheet_id) = affected_sheet_id {
        result.insert(
            format!("sheetShowCommand{suffix}"),
            json!(format!(
                "ooxml --json xlsx sheets show {} --sheet sheetId:{sheet_id}",
                command_arg(target)
            )),
        );
    }
}

fn sheet_part_uri(
    sheet: &WorkbookSheet,
    rel_targets: &BTreeMap<String, String>,
) -> CliResult<String> {
    let target = rel_targets
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    Ok(format!("/{}", normalize_xl_target(target)))
}

fn validate_new_sheet_name(
    name: &str,
    existing: &[WorkbookSheet],
    current_name: &str,
) -> CliResult<()> {
    if name.trim().is_empty() {
        return Err(CliError::invalid_args("sheet name cannot be empty"));
    }
    if name.chars().count() > 31 {
        return Err(CliError::invalid_args(format!(
            "sheet name {:?} exceeds Excel's 31-character limit",
            name
        )));
    }
    if name.eq_ignore_ascii_case("History") {
        return Err(CliError::invalid_args(format!(
            "sheet name {:?} is reserved by Excel",
            name
        )));
    }
    if name.starts_with('\'') || name.ends_with('\'') {
        return Err(CliError::invalid_args(
            "sheet name cannot begin or end with apostrophe",
        ));
    }
    if name
        .chars()
        .any(|ch| matches!(ch, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
    {
        return Err(CliError::invalid_args(format!(
            "sheet name {:?} contains invalid Excel sheet-name characters",
            name
        )));
    }
    for sheet in existing {
        if !current_name.is_empty() && sheet.name.eq_ignore_ascii_case(current_name) {
            continue;
        }
        if sheet.name.eq_ignore_ascii_case(name) {
            return Err(CliError::invalid_args(format!(
                "sheet name {:?} already exists",
                name
            )));
        }
    }
    Ok(())
}

fn allocate_sheet_id(sheets: &[WorkbookSheet]) -> CliResult<u32> {
    let existing = sheets
        .iter()
        .map(|sheet| sheet.sheet_id)
        .collect::<BTreeSet<_>>();
    if existing.len() >= SHEET_ID_RANDOM_CEILING as usize {
        return Err(CliError::invalid_args("no available sheetId values remain"));
    }
    let mut seed = crate::chrono_like_counter()
        ^ ((std::process::id() as u128) << 32)
        ^ ((sheets.len() as u128) << 16);
    for _ in 0..SHEET_ID_RANDOM_CEILING {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let candidate = ((seed % SHEET_ID_RANDOM_CEILING as u128) as u32) + 1;
        if !existing.contains(&candidate) {
            return Ok(candidate);
        }
    }
    (1..=SHEET_ID_RANDOM_CEILING)
        .find(|candidate| !existing.contains(candidate))
        .ok_or_else(|| CliError::invalid_args("no available sheetId values remain"))
}

fn next_worksheet_part_uri(entries: &BTreeSet<String>) -> String {
    let mut max_index = 0u32;
    for entry in entries {
        let normalized = entry.trim_start_matches('/');
        let Some(rest) = normalized.strip_prefix("xl/worksheets/sheet") else {
            continue;
        };
        let Some(number) = rest.strip_suffix(".xml") else {
            continue;
        };
        if let Ok(number) = number.parse::<u32>() {
            max_index = max_index.max(number);
        }
    }
    let mut index = max_index + 1;
    loop {
        let uri = format!("/xl/worksheets/sheet{index}.xml");
        if !entries.contains(&uri) {
            return uri;
        }
        index += 1;
    }
}

fn new_worksheet_xml(prefix: Option<&str>) -> String {
    match prefix.filter(|value| !value.is_empty()) {
        Some(prefix) => format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><{prefix}:worksheet xmlns:{prefix}="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><{prefix}:sheetData/></{prefix}:worksheet>"#
        ),
        None => {
            r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"#.to_string()
        }
    }
}

fn workbook_prefix(workbook_xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "workbook" =>
            {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                return name.split_once(':').map(|(prefix, _)| prefix.to_string());
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn insert_workbook_sheet(
    workbook_xml: &str,
    name: &str,
    sheet_id: u32,
    rel_id: &str,
    after_position: usize,
) -> CliResult<String> {
    let workbook_xml = ensure_workbook_r_namespace(workbook_xml);
    let sheets_span = workbook_sheets_span(&workbook_xml)?;
    let sheet_spans = workbook_sheet_spans(&workbook_xml)?;
    let prefix = sheets_span
        .name
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .or_else(|| workbook_prefix(&workbook_xml));
    let tag = qualified_name(prefix.as_deref(), "sheet");
    let sheet_xml = format!(
        r#"<{tag} name="{}" sheetId="{sheet_id}" r:id="{}"/>"#,
        xml_attr_escape(name),
        xml_attr_escape(rel_id)
    );
    let insert_at = if after_position == 0 || after_position >= sheet_spans.len() {
        sheets_span.close_start
    } else {
        sheet_spans[after_position - 1].end
    };
    Ok(replace_xml_span(
        &workbook_xml,
        insert_at,
        insert_at,
        &sheet_xml,
    ))
}

fn ensure_workbook_r_namespace(workbook_xml: &str) -> String {
    if workbook_xml.contains("xmlns:r=") {
        return workbook_xml.to_string();
    }
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(false);
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "workbook" => {
                let end = reader.buffer_position() as usize;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = xml_attrs_map(&e);
                attrs.insert(
                    "xmlns:r".to_string(),
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                        .to_string(),
                );
                return replace_xml_span(
                    workbook_xml,
                    start,
                    end,
                    &format!("<{name}{}>", render_xml_attrs(&attrs)),
                );
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "workbook" => {
                let end = reader.buffer_position() as usize;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = xml_attrs_map(&e);
                attrs.insert(
                    "xmlns:r".to_string(),
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                        .to_string(),
                );
                return replace_xml_span(
                    workbook_xml,
                    start,
                    end,
                    &format!("<{name}{}/>", render_xml_attrs(&attrs)),
                );
            }
            Ok(Event::Eof) | Err(_) => return workbook_xml.to_string(),
            _ => {}
        }
    }
}

struct SheetsSpan {
    name: String,
    content_start: usize,
    close_start: usize,
}

fn workbook_sheets_span(workbook_xml: &str) -> CliResult<SheetsSpan> {
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "sheets" => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let content_start = reader.buffer_position() as usize;
                let mut depth = 1usize;
                loop {
                    let before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::Start(inner))
                            if local_name(inner.name().as_ref()) == "sheets" =>
                        {
                            depth += 1;
                        }
                        Ok(Event::End(inner)) if local_name(inner.name().as_ref()) == "sheets" => {
                            depth -= 1;
                            if depth == 0 {
                                return Ok(SheetsSpan {
                                    name,
                                    content_start,
                                    close_start: before,
                                });
                            }
                        }
                        Ok(Event::Eof) | Err(_) => {
                            return Err(CliError::unexpected(
                                "could not locate workbook sheets close tag",
                            ));
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "sheets" => {
                let end = reader.buffer_position() as usize;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                return Ok(SheetsSpan {
                    name,
                    content_start: end,
                    close_start: end,
                });
            }
            Ok(Event::Eof) => {
                return Err(CliError::invalid_args("workbook has no sheets"));
            }
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn workbook_sheet_spans(workbook_xml: &str) -> CliResult<Vec<SheetElementSpan>> {
    let span = workbook_sheets_span(workbook_xml)?;
    let inner = &workbook_xml[span.content_start..span.close_start];
    let mut reader = Reader::from_str(inner);
    reader.config_mut().trim_text(false);
    let mut spans = Vec::new();
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "sheet" => {
                let end = reader.buffer_position() as usize;
                spans.push(sheet_element_span_from_event(
                    workbook_xml,
                    &e,
                    span.content_start + start,
                    span.content_start + end,
                )?);
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "sheet" => {
                let mut depth = 1usize;
                loop {
                    match reader.read_event() {
                        Ok(Event::Start(inner)) if local_name(inner.name().as_ref()) == "sheet" => {
                            depth += 1;
                        }
                        Ok(Event::End(inner)) if local_name(inner.name().as_ref()) == "sheet" => {
                            depth -= 1;
                            if depth == 0 {
                                let end = reader.buffer_position() as usize;
                                spans.push(sheet_element_span_from_event(
                                    workbook_xml,
                                    &e,
                                    span.content_start + start,
                                    span.content_start + end,
                                )?);
                                break;
                            }
                        }
                        Ok(Event::Eof) | Err(_) => {
                            return Err(CliError::unexpected("unexpected EOF in workbook sheets"));
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(spans)
}

fn sheet_element_span_from_event(
    xml: &str,
    element: &BytesStart<'_>,
    start: usize,
    end: usize,
) -> CliResult<SheetElementSpan> {
    let attrs = xml_attrs_map(element);
    let rel_id = attrs
        .get("r:id")
        .or_else(|| attrs.get("id"))
        .cloned()
        .ok_or_else(|| CliError::unexpected("sheet is missing r:id"))?;
    Ok(SheetElementSpan {
        start,
        end,
        rel_id,
        attrs,
        fragment: xml[start..end].to_string(),
    })
}

fn replace_sheet_element_attr<F>(workbook_xml: &str, rel_id: &str, edit: F) -> CliResult<String>
where
    F: FnOnce(&mut BTreeMap<String, String>),
{
    let spans = workbook_sheet_spans(workbook_xml)?;
    let span = spans
        .iter()
        .find(|span| span.rel_id == rel_id)
        .ok_or_else(|| CliError::invalid_args(format!("sheet not found: {rel_id}")))?;
    let mut attrs = span.attrs.clone();
    edit(&mut attrs);
    let name = element_name_from_fragment(&span.fragment, "sheet");
    let replacement = format!("<{name}{}/>", render_xml_attrs(&attrs));
    Ok(replace_xml_span(
        workbook_xml,
        span.start,
        span.end,
        &replacement,
    ))
}

fn reorder_workbook_sheets(
    workbook_xml: &str,
    rel_id: &str,
    new_index: usize,
) -> CliResult<String> {
    let sheet_spans = workbook_sheet_spans(workbook_xml)?;
    let old_index = sheet_spans
        .iter()
        .position(|span| span.rel_id == rel_id)
        .ok_or_else(|| CliError::invalid_args(format!("sheet not found: {rel_id}")))?;
    if old_index == new_index {
        return Ok(workbook_xml.to_string());
    }
    let mut fragments = sheet_spans
        .iter()
        .map(|span| span.fragment.clone())
        .collect::<Vec<_>>();
    let moving = fragments.remove(old_index);
    if new_index >= fragments.len() {
        fragments.push(moving);
    } else {
        fragments.insert(new_index, moving);
    }
    let sheets_span = workbook_sheets_span(workbook_xml)?;
    let replacement = fragments.join("");
    Ok(replace_xml_span(
        workbook_xml,
        sheets_span.content_start,
        sheets_span.close_start,
        &replacement,
    ))
}

fn remove_workbook_sheet(workbook_xml: &str, rel_id: &str) -> CliResult<String> {
    let span = workbook_sheet_spans(workbook_xml)?
        .into_iter()
        .find(|span| span.rel_id == rel_id)
        .ok_or_else(|| CliError::invalid_args(format!("sheet not found: {rel_id}")))?;
    Ok(replace_xml_span(workbook_xml, span.start, span.end, ""))
}

fn element_name_from_fragment(fragment: &str, fallback: &str) -> String {
    let trimmed = fragment.trim_start();
    let Some(rest) = trimmed.strip_prefix('<') else {
        return fallback.to_string();
    };
    rest.split(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn apply_sheet_position_remap(workbook_xml: &str, remap: &[i32]) -> String {
    let updated = remap_workbook_view_positions(workbook_xml, remap);
    remap_defined_name_positions(&updated, remap)
}

fn remap_workbook_view_positions(workbook_xml: &str, remap: &[i32]) -> String {
    rewrite_element_openings(workbook_xml, "workbookView", |span| {
        let mut attrs = span.attrs.clone();
        for key in ["activeTab", "firstSheet"] {
            let Some(value) = attrs.get(key).and_then(|value| value.parse::<i32>().ok()) else {
                continue;
            };
            attrs.insert(
                key.to_string(),
                remap_workbook_view_index(value, remap).to_string(),
            );
        }
        Some(render_opening(span, &attrs))
    })
}

fn remap_defined_name_positions(workbook_xml: &str, remap: &[i32]) -> String {
    rewrite_element_openings(workbook_xml, "definedName", |span| {
        let value = span
            .attrs
            .get("localSheetId")
            .and_then(|value| value.parse::<i32>().ok())?;
        if value < 0 || value as usize >= remap.len() {
            return None;
        }
        let mapped = remap[value as usize];
        if mapped < 0 {
            return Some(String::new());
        }
        let mut attrs = span.attrs.clone();
        attrs.insert("localSheetId".to_string(), mapped.to_string());
        Some(render_opening(span, &attrs))
    })
}

fn rewrite_element_openings<F>(xml: &str, element_local: &str, mut edit: F) -> String
where
    F: FnMut(&ElementOpenSpan) -> Option<String>,
{
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e))
                if local_name(e.name().as_ref()) == element_local =>
            {
                let end = reader.buffer_position() as usize;
                let span = ElementOpenSpan {
                    name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    attrs: xml_attrs_map(&e),
                    empty: xml[start..end].trim_end().ends_with("/>"),
                };
                if let Some(replacement) = edit(&span) {
                    replacements.push((start, end, replacement));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    apply_replacements(xml, &replacements)
}

fn render_opening(span: &ElementOpenSpan, attrs: &BTreeMap<String, String>) -> String {
    if span.empty {
        format!("<{}{}/>", span.name, render_xml_attrs(attrs))
    } else {
        format!("<{}{}>", span.name, render_xml_attrs(attrs))
    }
}

fn apply_replacements(xml: &str, replacements: &[(usize, usize, String)]) -> String {
    if replacements.is_empty() {
        return xml.to_string();
    }
    let mut out = String::with_capacity(xml.len());
    let mut cursor = 0usize;
    for (start, end, replacement) in replacements {
        if *start > cursor {
            out.push_str(&xml[cursor..*start]);
        }
        out.push_str(replacement);
        cursor = *end;
    }
    out.push_str(&xml[cursor..]);
    out
}

fn move_sheet_position_remap(count: usize, old_index: usize, new_index: usize) -> Vec<i32> {
    let mut order = (0..count)
        .filter(|index| *index != old_index)
        .collect::<Vec<_>>();
    if new_index >= order.len() {
        order.push(old_index);
    } else {
        order.insert(new_index, old_index);
    }
    let mut remap = vec![0; count];
    for (new_pos, old_pos) in order.into_iter().enumerate() {
        remap[old_pos] = new_pos as i32;
    }
    remap
}

fn delete_sheet_position_remap(count: usize, deleted_index: usize) -> Vec<i32> {
    (0..count)
        .map(|index| match index.cmp(&deleted_index) {
            std::cmp::Ordering::Equal => -1,
            std::cmp::Ordering::Less => index as i32,
            std::cmp::Ordering::Greater => index as i32 - 1,
        })
        .collect()
}

fn remap_workbook_view_index(value: i32, remap: &[i32]) -> i32 {
    if remap.is_empty() || value < 0 {
        return 0;
    }
    let value = (value as usize).min(remap.len() - 1);
    if remap[value] >= 0 {
        return remap[value];
    }
    for mapped in remap.iter().skip(value) {
        if *mapped >= 0 {
            return *mapped;
        }
    }
    for mapped in remap[..value].iter().rev() {
        if *mapped >= 0 {
            return *mapped;
        }
    }
    0
}

fn resolve_move_target_position(
    sheets: &[WorkbookSheet],
    moving: &WorkbookSheet,
    to: Option<i64>,
    to_present: bool,
    before: Option<&str>,
    after: Option<&str>,
) -> CliResult<usize> {
    let selected = usize::from(to_present)
        + usize::from(before.is_some_and(|value| !value.is_empty()))
        + usize::from(after.is_some_and(|value| !value.is_empty()));
    if selected != 1 {
        return Err(CliError::invalid_args(
            "must specify exactly one of --to, --before, or --after",
        ));
    }
    if to_present {
        let to = to.unwrap_or(0);
        if to < 1 || to as usize > sheets.len() {
            return Err(CliError::invalid_args(format!(
                "--to must be between 1 and {}",
                sheets.len()
            )));
        }
        return Ok(to as usize);
    }
    let (target_selector, insert_after) =
        if let Some(after) = after.filter(|value| !value.is_empty()) {
            (after, true)
        } else {
            (before.unwrap_or_default(), false)
        };
    let target = resolve_sheet(sheets, target_selector)?;
    if target.rel_id == moving.rel_id {
        return Ok(moving.position as usize);
    }
    let order = sheets
        .iter()
        .filter(|sheet| sheet.rel_id != moving.rel_id)
        .collect::<Vec<_>>();
    for (index, sheet) in order.iter().enumerate() {
        if sheet.rel_id == target.rel_id {
            return Ok(if insert_after { index + 2 } else { index + 1 });
        }
    }
    Err(CliError::invalid_args(format!(
        "sheet not found: {target_selector}"
    )))
}

fn inserted_sheet_number(existing_count: usize, after_position: usize) -> usize {
    if existing_count == 0 {
        1
    } else if after_position == 0 || after_position >= existing_count {
        existing_count + 1
    } else {
        after_position + 1
    }
}

fn relationship_target(workbook_part: &str, part_uri: &str) -> String {
    let workbook_dir = workbook_part
        .trim_start_matches('/')
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or_default();
    let prefix = if workbook_dir.is_empty() {
        String::new()
    } else {
        format!("{workbook_dir}/")
    };
    let normalized = part_uri.trim_start_matches('/');
    normalized
        .strip_prefix(&prefix)
        .unwrap_or(normalized)
        .to_string()
}

fn remove_sheet_and_calc_chain_relationships(
    rels_xml: &str,
    workbook_part: &str,
    sheet_rel_id: &str,
) -> (String, Vec<String>) {
    let mut reader = Reader::from_str(rels_xml);
    reader.config_mut().trim_text(false);
    let mut removals = Vec::<(usize, usize, String)>::new();
    let mut removed_parts = Vec::<String>::new();
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e))
                if local_name(e.name().as_ref()) == "Relationship" =>
            {
                let end = reader.buffer_position() as usize;
                let attrs = xml_attrs_map(&e);
                let should_remove = attrs.get("Id").is_some_and(|id| id == sheet_rel_id)
                    || attrs
                        .get("Type")
                        .is_some_and(|rel_type| rel_type == REL_CALC_CHAIN);
                if should_remove {
                    if attrs
                        .get("TargetMode")
                        .is_none_or(|mode| mode != "External")
                        && let Some(target) = attrs.get("Target")
                    {
                        removed_parts.push(resolve_relationship_target(workbook_part, target));
                    }
                    removals.push((start, end, String::new()));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    (apply_replacements(rels_xml, &removals), removed_parts)
}

fn remove_content_type_overrides(
    xml: &str,
    removed_parts: &[String],
    remove_calc_chain: bool,
) -> String {
    let removed = removed_parts
        .iter()
        .map(|part| format!("/{}", part.trim_start_matches('/')))
        .collect::<BTreeSet<_>>();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut removals = Vec::<(usize, usize, String)>::new();
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e))
                if local_name(e.name().as_ref()) == "Override" =>
            {
                let end = reader.buffer_position() as usize;
                let attrs = xml_attrs_map(&e);
                let part_matches = attrs
                    .get("PartName")
                    .is_some_and(|part| removed.contains(part));
                let calc_matches = remove_calc_chain
                    && attrs
                        .get("ContentType")
                        .is_some_and(|content_type| content_type == CONTENT_TYPE_CALC_CHAIN);
                if part_matches || calc_matches {
                    removals.push((start, end, String::new()));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    apply_replacements(xml, &removals)
}

fn visible_sheet_count(sheets: &[WorkbookSheet]) -> usize {
    sheets
        .iter()
        .filter(|sheet| sheet.state.is_empty() || sheet.state == "visible")
        .count()
}

fn qualified_name(prefix: Option<&str>, local: &str) -> String {
    match prefix.filter(|value| !value.is_empty()) {
        Some(prefix) => format!("{prefix}:{local}"),
        None => local.to_string(),
    }
}
