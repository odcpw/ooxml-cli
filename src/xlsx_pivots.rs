use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

mod model;

pub(crate) use model::XlsxPivotsCreateOptions;
use model::{
    PivotCell, PivotCreateArtifacts, PivotDataField, PivotFieldModel, PivotSource, PivotValueSpec,
    XlsxPivotCacheField, XlsxPivotCacheRef, XlsxPivotFieldRef, XlsxPivotRef,
};

use crate::{
    CliError, CliResult, RelationshipEntry, WorkbookSheet, XlsxRangeExportOptions,
    add_relationship_to_xml, allocate_relationship_id, attr, col_name, command_arg,
    copy_zip_with_part_overrides, ensure_content_type_override, local_name, parse_cell_ref,
    parse_range, relationship_entries, relationship_entries_from_xml,
    relationship_target_from_source_to_target, relationships_part_for, resolve_relationship_target,
    resolve_sheet, select_xlsx_table, selector_candidates, validate,
    validate_xlsx_mutation_output_flags, workbook_sheets, xlsx_range_export_with_options,
    xlsx_ranges_set_temp_path, xlsx_source_command, xlsx_tables, xml_attr_escape, zip_entry_names,
    zip_text,
};

const XLSX_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL_WORKSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
const REL_PIVOT_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable";
const REL_PIVOT_CACHE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition";
const REL_PIVOT_RECORDS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords";
const CONTENT_TYPE_PIVOT_TABLE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml";
const CONTENT_TYPE_PIVOT_CACHE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml";
const CONTENT_TYPE_PIVOT_RECORDS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml";

pub(crate) fn xlsx_pivots_list(file: &str, sheet_selector: Option<&str>) -> CliResult<Value> {
    ensure_xlsx_file_exists(file)?;
    let pivots = xlsx_pivots(file, sheet_selector)?;
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "pivots": pivots.iter().map(|pivot| xlsx_pivot_item_json(file, pivot)).collect::<Vec<_>>(),
    }))
}

pub(crate) fn xlsx_pivots_show(
    file: &str,
    sheet_selector: Option<&str>,
    pivot_selector: Option<&str>,
) -> CliResult<Value> {
    ensure_xlsx_file_exists(file)?;
    let pivots = xlsx_pivots(file, sheet_selector)?;
    let pivot = select_xlsx_pivot(&pivots, pivot_selector.unwrap_or_default())?;
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "pivots": [xlsx_pivot_item_json(file, &pivot)],
    }))
}

pub(crate) fn xlsx_pivots_create(
    file: &str,
    options: XlsxPivotsCreateOptions<'_>,
) -> CliResult<Value> {
    ensure_xlsx_file_exists(file)?;
    let row_fields = split_comma_list(options.rows.unwrap_or_default());
    let col_fields = split_comma_list(options.cols.unwrap_or_default());
    let page_fields = split_comma_list(options.filters.unwrap_or_default());
    if row_fields.is_empty() && col_fields.is_empty() {
        return Err(CliError::invalid_args(
            "specify at least one --rows or --cols field",
        ));
    }
    if options.values.unwrap_or_default().trim().is_empty() {
        return Err(CliError::invalid_args(
            "specify at least one --values field (name or name:agg)",
        ));
    }
    let value_specs =
        parse_pivot_value_fields(&split_comma_list(options.values.unwrap_or_default()))?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let source = resolve_pivot_source(file, &options)?;
    if let Some(expect_range) = options
        .expect_source_range
        .filter(|value| !value.is_empty())
        && !source.range.eq_ignore_ascii_case(expect_range)
    {
        return Err(CliError::invalid_args(format!(
            "source range mismatch: expected {expect_range} but found {}",
            source.range
        )));
    }

    let anchor = match options.anchor.filter(|value| !value.trim().is_empty()) {
        Some(value) => parse_cell_ref(value)
            .map_err(|err| CliError::invalid_args(format!("invalid --anchor: {}", err.message)))?,
        None => (source.bounds.max_col() + 2, source.bounds.min_row()),
    };
    let artifacts = build_pivot_create_artifacts(
        file,
        &source,
        options.target_sheet,
        anchor,
        options.name,
        &row_fields,
        &col_fields,
        &page_fields,
        &value_specs,
    )
    .map_err(|err| CliError::invalid_args(format!("failed to create pivot: {}", err.message)))?;

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
    copy_zip_with_part_overrides(file, &readback_path, &artifacts.overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }

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
            .map_err(|err| CliError::unexpected(format!("failed to replace workbook: {err}")))?;
    }

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("name".to_string(), json!(artifacts.name));
    result.insert("sourceSheet".to_string(), json!(artifacts.source_sheet));
    result.insert("sourceRange".to_string(), json!(artifacts.source_range));
    result.insert("targetSheet".to_string(), json!(artifacts.target_sheet));
    result.insert("location".to_string(), json!(artifacts.location));
    result.insert("cacheId".to_string(), json!(artifacts.cache_id));
    result.insert(
        "cacheDefinitionUri".to_string(),
        json!(artifacts.cache_definition_uri),
    );
    result.insert(
        "cacheRecordsUri".to_string(),
        json!(artifacts.cache_records_uri),
    );
    result.insert(
        "pivotTableUri".to_string(),
        json!(artifacts.pivot_table_uri),
    );
    if !artifacts.row_fields.is_empty() {
        result.insert("rowFields".to_string(), json!(artifacts.row_fields));
    }
    if !artifacts.col_fields.is_empty() {
        result.insert("colFields".to_string(), json!(artifacts.col_fields));
    }
    if !artifacts.page_fields.is_empty() {
        result.insert("pageFields".to_string(), json!(artifacts.page_fields));
    }
    if !artifacts.value_fields.is_empty() {
        result.insert("valueFields".to_string(), json!(artifacts.value_fields));
    }
    if !artifacts.warnings.is_empty() {
        result.insert("warnings".to_string(), json!(artifacts.warnings));
    }
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
        result.insert(
            "validateCommand".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(commit_path)
            )),
        );
        result.insert(
            "pivotsListCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx pivots list {}",
                command_arg(commit_path)
            )),
        );
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    Ok(Value::Object(result))
}

fn ensure_xlsx_file_exists(file: &str) -> CliResult<()> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    Ok(())
}

fn xlsx_pivots(file: &str, sheet_selector: Option<&str>) -> CliResult<Vec<XlsxPivotRef>> {
    let workbook_xml = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook_xml)?;
    let selected_sheets = if let Some(selector) = sheet_selector.filter(|value| !value.is_empty()) {
        vec![resolve_sheet(&sheets, selector)?]
    } else {
        sheets
    };
    let workbook_rels = relationship_entries(file, "xl/_rels/workbook.xml.rels")?;
    let caches = workbook_pivot_caches(file, &workbook_xml, &workbook_rels)?;
    let mut pivots = Vec::new();
    for sheet in selected_sheets {
        let Some(sheet_part_uri) = sheet_part_uri(&sheet, &workbook_rels) else {
            continue;
        };
        let rels = optional_relationship_entries(file, &sheet_part_uri)?;
        for rel in rels.iter().filter(|rel| rel.rel_type == REL_PIVOT_TABLE) {
            let part_uri = resolve_relationship_target(&sheet_part_uri, &rel.target);
            let mut pivot = parse_pivot_table_part(
                file,
                &part_uri,
                &sheet,
                &sheet_part_uri,
                &rel.id,
                (pivots.len() + 1) as u32,
                &caches,
            )?;
            pivot.apply_selectors();
            pivots.push(pivot);
        }
    }
    Ok(pivots)
}

fn workbook_pivot_caches(
    file: &str,
    workbook_xml: &str,
    workbook_rels: &[RelationshipEntry],
) -> CliResult<BTreeMap<i32, XlsxPivotCacheRef>> {
    let rel_by_id = workbook_rels
        .iter()
        .map(|rel| (rel.id.clone(), rel.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut caches = BTreeMap::new();
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "pivotCache" =>
            {
                let cache_id = parse_i32(attr(&e, "cacheId").as_deref(), 0);
                let relationship_id = attr(&e, "id")
                    .or_else(|| attr(&e, "r:id"))
                    .unwrap_or_default();
                if cache_id <= 0 || relationship_id.is_empty() {
                    continue;
                }
                if let Some(rel) = rel_by_id.get(&relationship_id) {
                    let part_uri = resolve_relationship_target("/xl/workbook.xml", &rel.target);
                    let cache =
                        parse_pivot_cache_definition(file, cache_id, &relationship_id, &part_uri)?;
                    caches.insert(cache_id, cache);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(caches)
}

fn parse_pivot_cache_definition(
    file: &str,
    cache_id: i32,
    relationship_id: &str,
    part_uri: &str,
) -> CliResult<XlsxPivotCacheRef> {
    let xml = zip_text(file, part_uri.trim_start_matches('/'))?;
    let rels = optional_relationship_entries(file, part_uri)?;
    let mut cache = XlsxPivotCacheRef {
        cache_id,
        part_uri: normalize_part_uri(part_uri),
        relationship_id: relationship_id.to_string(),
        ..Default::default()
    };
    if let Some(records_rel) = rels.iter().find(|rel| rel.rel_type == REL_PIVOT_RECORDS) {
        cache.records_part_uri = resolve_relationship_target(part_uri, &records_rel.target);
    }
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut in_cache_fields = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "pivotCacheDefinition" => {
                parse_cache_root_attrs(&e, &mut cache);
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "pivotCacheDefinition" => {
                parse_cache_root_attrs(&e, &mut cache);
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "cacheSource" => {
                cache.source.source_type = attr(&e, "type").unwrap_or_default();
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "worksheetSource" =>
            {
                cache.source.sheet = attr(&e, "sheet").unwrap_or_default();
                cache.source.range = attr(&e, "ref").unwrap_or_default();
                cache.source.name = attr(&e, "name").unwrap_or_default();
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "cacheFields" => {
                in_cache_fields = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "cacheFields" => {
                in_cache_fields = false;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_cache_fields && local_name(e.name().as_ref()) == "cacheField" =>
            {
                let index = cache.fields.len() as i32;
                cache.fields.push(XlsxPivotCacheField {
                    index,
                    name: attr(&e, "name").unwrap_or_default(),
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(cache)
}

fn parse_cache_root_attrs(e: &BytesStart<'_>, cache: &mut XlsxPivotCacheRef) {
    cache.record_count = parse_i32(attr(e, "recordCount").as_deref(), 0);
    cache.created_version = attr(e, "createdVersion").unwrap_or_default();
    cache.refreshed_version = attr(e, "refreshedVersion").unwrap_or_default();
    cache.refresh_on_load = parse_bool_attr(attr(e, "refreshOnLoad").as_deref());
    cache.save_data = attr(e, "saveData")
        .as_deref()
        .map(|value| parse_bool_attr(Some(value)));
}

fn parse_pivot_table_part(
    file: &str,
    part_uri: &str,
    sheet: &WorkbookSheet,
    sheet_part_uri: &str,
    relationship_id: &str,
    number: u32,
    workbook_caches: &BTreeMap<i32, XlsxPivotCacheRef>,
) -> CliResult<XlsxPivotRef> {
    let xml = zip_text(file, part_uri.trim_start_matches('/'))?;
    let mut pivot = XlsxPivotRef {
        number,
        sheet: sheet.name.clone(),
        sheet_number: sheet.position,
        sheet_part_uri: sheet_part_uri.to_string(),
        relationship_id: relationship_id.to_string(),
        part_uri: normalize_part_uri(part_uri),
        ..Default::default()
    };
    let rels = optional_relationship_entries(file, part_uri)?;
    let rel_cache = rels
        .iter()
        .find(|rel| rel.rel_type == REL_PIVOT_CACHE)
        .map(|rel| {
            (
                rel.id.clone(),
                resolve_relationship_target(part_uri, &rel.target),
            )
        });
    let mut section = "";
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "pivotTableDefinition" =>
            {
                pivot.name = attr(&e, "name").unwrap_or_default();
                pivot.cache_id = parse_i32(attr(&e, "cacheId").as_deref(), 0);
                pivot.cache = rel_cache
                    .as_ref()
                    .and_then(|(cache_rid, cache_part)| {
                        workbook_caches
                            .values()
                            .find(|cache| cache.part_uri == *cache_part)
                            .cloned()
                            .map(|mut cache| {
                                cache.relationship_id = cache_rid.clone();
                                cache
                            })
                    })
                    .or_else(|| workbook_caches.get(&pivot.cache_id).cloned());
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "location" =>
            {
                pivot.location = attr(&e, "ref").unwrap_or_default();
                if let Ok(bounds) = parse_range(&pivot.location) {
                    pivot.rows = bounds.row_count();
                    pivot.cols = bounds.col_count();
                }
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "pivotFields" => {
                section = "pivotFields";
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "rowFields" => {
                section = "rowFields";
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "colFields" => {
                section = "colFields";
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "pageFields" => {
                section = "pageFields";
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "dataFields" => {
                section = "dataFields";
            }
            Ok(Event::End(e))
                if matches!(
                    local_name(e.name().as_ref()),
                    "pivotFields" | "rowFields" | "colFields" | "pageFields" | "dataFields"
                ) =>
            {
                section = "";
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if section == "pivotFields" && local_name(e.name().as_ref()) == "pivotField" =>
            {
                let index = pivot.fields.len() as i32;
                let mut field = field_ref_for_cache(
                    &pivot.cache,
                    index,
                    normalize_axis(&attr(&e, "axis").unwrap_or_default()),
                );
                if let Some(subtotal) = first_enabled_subtotal(&e) {
                    field.subtotal = subtotal;
                }
                pivot.fields.push(field);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if section == "rowFields" && local_name(e.name().as_ref()) == "field" =>
            {
                let index = parse_i32(attr(&e, "x").as_deref(), -1);
                pivot
                    .row_fields
                    .push(field_ref_for_cache(&pivot.cache, index, "row".to_string()));
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if section == "colFields" && local_name(e.name().as_ref()) == "field" =>
            {
                let index = parse_i32(attr(&e, "x").as_deref(), -1);
                pivot.column_fields.push(field_ref_for_cache(
                    &pivot.cache,
                    index,
                    "column".to_string(),
                ));
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if section == "pageFields" && local_name(e.name().as_ref()) == "pageField" =>
            {
                let index = parse_i32(attr(&e, "fld").as_deref(), -1);
                pivot.filter_fields.push(field_ref_for_cache(
                    &pivot.cache,
                    index,
                    "filter".to_string(),
                ));
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if section == "dataFields" && local_name(e.name().as_ref()) == "dataField" =>
            {
                let index = parse_i32(attr(&e, "fld").as_deref(), -1);
                let mut field = field_ref_for_cache(&pivot.cache, index, "data".to_string());
                field.subtotal = attr(&e, "subtotal").unwrap_or_default();
                field.caption = attr(&e, "name").unwrap_or_default();
                pivot.data_fields.push(field);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(pivot)
}

fn field_ref_for_cache(
    cache: &Option<XlsxPivotCacheRef>,
    index: i32,
    axis: String,
) -> XlsxPivotFieldRef {
    XlsxPivotFieldRef {
        index,
        name: cache
            .as_ref()
            .map(|cache| cache.field_name(index))
            .unwrap_or_default(),
        axis,
        ..Default::default()
    }
}

fn xlsx_pivot_item_json(file: &str, pivot: &XlsxPivotRef) -> Value {
    let mut object = pivot.to_json_object();
    let pivot_selector = xlsx_pivot_selector(pivot);
    let sheet_selector = xlsx_pivot_sheet_selector(pivot);
    object.insert(
        "showCommand".to_string(),
        json!(xlsx_source_command(
            vec!["ooxml", "--json", "xlsx", "pivots", "show", file],
            &[("--sheet", &sheet_selector), ("--pivot", &pivot_selector)]
        )),
    );
    if let Some(cache) = &pivot.cache
        && !cache.source.sheet.is_empty()
        && !cache.source.range.is_empty()
    {
        object.insert(
            "sourceExportCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx ranges export {} --sheet {} --range {} --include-types",
                command_arg(file),
                command_arg(&cache.source.sheet),
                command_arg(&cache.source.range),
            )),
        );
    }
    Value::Object(object)
}

fn xlsx_pivot_selector(pivot: &XlsxPivotRef) -> String {
    if !pivot.primary_selector.is_empty() {
        pivot.primary_selector.clone()
    } else if !pivot.name.is_empty() {
        pivot.name.clone()
    } else if pivot.number > 0 {
        format!("pivot:{}", pivot.number)
    } else {
        "1".to_string()
    }
}

fn xlsx_pivot_sheet_selector(pivot: &XlsxPivotRef) -> String {
    if !pivot.sheet.is_empty() {
        pivot.sheet.clone()
    } else if pivot.sheet_number > 0 {
        pivot.sheet_number.to_string()
    } else {
        "1".to_string()
    }
}

fn select_xlsx_pivot(pivots: &[XlsxPivotRef], selector: &str) -> CliResult<XlsxPivotRef> {
    if pivots.is_empty() {
        return Err(CliError::invalid_args("workbook has no pivots"));
    }
    let selector = selector.trim();
    if selector.is_empty() {
        if pivots.len() == 1 {
            return Ok(pivots[0].clone());
        }
        return Err(CliError::invalid_args(
            "--pivot is required when workbook has multiple pivots",
        ));
    }
    let matches = pivots
        .iter()
        .filter(|pivot| {
            pivot
                .selectors
                .iter()
                .any(|candidate| candidate == selector)
        })
        .cloned()
        .collect::<Vec<_>>();
    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }
    if matches.len() > 1 {
        let selectors = matches
            .iter()
            .map(xlsx_pivot_selector)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(CliError::invalid_args(format!(
            "pivot selector {selector:?} matched multiple pivots ({selectors}); use a more specific selector"
        )));
    }
    if let Ok(number) = selector.parse::<usize>() {
        if (1..=pivots.len()).contains(&number) {
            return Ok(pivots[number - 1].clone());
        }
        return Err(CliError::target_not_found(format!(
            "pivot {number} is out of range (1-{})",
            pivots.len()
        )));
    }
    let candidates = pivots
        .iter()
        .map(|pivot| (pivot.primary_selector.as_str(), pivot.selectors.as_slice()))
        .collect::<Vec<_>>();
    let suggestions = selector_candidates(&candidates, selector, 5);
    let hint = if suggestions.is_empty() {
        String::new()
    } else {
        format!("; did you mean: {}", suggestions.join(", "))
    };
    Err(CliError::target_not_found(format!(
        "pivot not found: {selector}{hint}; discover with `ooxml --json xlsx pivots list <file>`"
    )))
}

fn resolve_pivot_source(
    file: &str,
    options: &XlsxPivotsCreateOptions<'_>,
) -> CliResult<PivotSource> {
    let source_sheet = options.sheet.unwrap_or_default().trim().to_string();
    let source_range = options.range.unwrap_or_default().trim().to_string();
    let source_table = options.table.unwrap_or_default().trim().to_string();
    if !source_range.is_empty() && !source_table.is_empty() {
        return Err(CliError::invalid_args(
            "specify only one of --range or --table",
        ));
    }
    if source_range.is_empty() && source_table.is_empty() {
        return Err(CliError::invalid_args("must specify --range or --table"));
    }
    let (sheet, range) = if !source_table.is_empty() {
        let tables = xlsx_tables(
            file,
            if source_sheet.is_empty() {
                None
            } else {
                Some(source_sheet.as_str())
            },
        )?;
        let table = select_xlsx_table(&tables, &source_table)?;
        (table.sheet, table.range)
    } else {
        if source_sheet.is_empty() {
            return Err(CliError::invalid_args(
                "--sheet is required when using --range",
            ));
        }
        (source_sheet, source_range)
    };
    let bounds = parse_range(&range)
        .map_err(|err| CliError::invalid_args(format!("invalid --range: {}", err.message)))?
        .normalized();
    crate::check_range_max_cells(&range, bounds, options.max_cells)?;
    let exported = xlsx_range_export_with_options(
        file,
        &sheet,
        &range,
        XlsxRangeExportOptions {
            include_types: true,
            include_formulas: true,
            include_formats: false,
            data_out: None,
            max_cells: options.max_cells,
        },
    )?;
    let cells = pivot_cells_from_range_export(&exported)?;
    Ok(PivotSource {
        sheet,
        range,
        bounds,
        cells,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_pivot_create_artifacts(
    file: &str,
    source: &PivotSource,
    target_sheet_selector: Option<&str>,
    anchor: (u32, u32),
    name: Option<&str>,
    row_fields: &[String],
    col_fields: &[String],
    page_fields: &[String],
    value_specs: &[PivotValueSpec],
) -> CliResult<PivotCreateArtifacts> {
    let workbook_xml = zip_text(file, "xl/workbook.xml")?;
    let workbook_rels = relationship_entries(file, "xl/_rels/workbook.xml.rels")?;
    let sheets = workbook_sheets(&workbook_xml)?;
    let selector = target_sheet_selector
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&source.sheet);
    let target_sheet = resolve_sheet(&sheets, selector)?;
    let target_sheet_part_uri = sheet_part_uri(&target_sheet, &workbook_rels).ok_or_else(|| {
        CliError::unexpected(format!(
            "target sheet {:?} has no worksheet part URI",
            target_sheet.name
        ))
    })?;
    let mut entries = zip_entry_names(file)?.into_iter().collect::<BTreeSet<_>>();
    let cache_definition_uri =
        allocate_numbered_part(&mut entries, "/xl/pivotCache/pivotCacheDefinition", ".xml");
    let cache_records_uri =
        allocate_numbered_part(&mut entries, "/xl/pivotCache/pivotCacheRecords", ".xml");
    let pivot_table_uri =
        allocate_numbered_part(&mut entries, "/xl/pivotTables/pivotTable", ".xml");
    let cache_id = next_pivot_cache_id(&workbook_xml);
    let pivot_name = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("PivotTable{cache_id}"));

    let (headers, name_index) = pivot_headers(&source.cells)?;
    let mut axis = BTreeSet::new();
    let row_idx = resolve_pivot_fields("rows", row_fields, &headers, &name_index, &mut axis)?;
    let col_idx = resolve_pivot_fields("cols", col_fields, &headers, &name_index, &mut axis)?;
    let page_idx = resolve_pivot_fields("filters", page_fields, &headers, &name_index, &mut axis)?;
    let mut data_field_indices = BTreeSet::new();
    let mut data_fields = Vec::new();
    for spec in value_specs {
        let Some(index) = name_index.get(&spec.name).copied() else {
            return Err(CliError::invalid_args(format!(
                "value field {:?} not found in source header ({})",
                spec.name,
                headers.join(", ")
            )));
        };
        let subtotal = normalize_pivot_aggregation(&spec.aggregation).ok_or_else(|| {
            CliError::invalid_args(format!(
                "invalid aggregation {:?} for {:?}",
                spec.aggregation, spec.name
            ))
        })?;
        data_field_indices.insert(index);
        data_fields.push(PivotDataField {
            field_index: index,
            subtotal: subtotal.clone(),
            caption: pivot_data_caption(&subtotal, &spec.name),
        });
    }
    let field_models = build_pivot_field_models(&source.cells, &headers, &axis);
    let mut warnings = Vec::new();
    for spec in value_specs {
        if let Some(index) = name_index.get(&spec.name)
            && !field_models[*index].numeric
        {
            warnings.push(format!(
                "value field {:?} is not fully numeric; aggregation may be approximate",
                spec.name
            ));
        }
    }
    let location = pivot_location(anchor, row_idx.len(), data_fields.len());
    let cache_records_rid = "rId1";
    let cache_definition_xml = render_cache_definition_xml(
        source,
        &field_models,
        source.cells.len().saturating_sub(1),
        cache_records_rid,
    );
    let cache_records_xml = render_cache_records_xml(&source.cells, &field_models);
    let cache_rels_xml = render_relationships_xml(&[(
        cache_records_rid,
        REL_PIVOT_RECORDS,
        relationship_target_from_source_to_target(&cache_definition_uri, &cache_records_uri),
    )]);
    let pivot_table_xml = render_pivot_table_xml(
        &pivot_name,
        cache_id,
        &location,
        &field_models,
        &row_idx,
        &col_idx,
        &page_idx,
        &data_fields,
        &data_field_indices,
    );
    let pivot_rels_xml = render_relationships_xml(&[(
        "rId1",
        REL_PIVOT_CACHE,
        relationship_target_from_source_to_target(&pivot_table_uri, &cache_definition_uri),
    )]);

    let worksheet_rels_part = relationships_part_for(&target_sheet_part_uri);
    let worksheet_rels_xml =
        optional_zip_text(file, &worksheet_rels_part)?.unwrap_or_else(empty_relationships_xml);
    let worksheet_rels = relationship_entries_from_xml(&worksheet_rels_xml);
    let pivot_rid = allocate_relationship_id(&worksheet_rels);
    let worksheet_rels_xml = add_relationship_to_xml(
        worksheet_rels_xml,
        &pivot_rid,
        REL_PIVOT_TABLE,
        &relationship_target_from_source_to_target(&target_sheet_part_uri, &pivot_table_uri),
    );

    let workbook_rels_xml = zip_text(file, "xl/_rels/workbook.xml.rels")?;
    let next_workbook_rid =
        allocate_relationship_id(&relationship_entries_from_xml(&workbook_rels_xml));
    let workbook_rels_xml = add_relationship_to_xml(
        workbook_rels_xml,
        &next_workbook_rid,
        REL_PIVOT_CACHE,
        &relationship_target_from_source_to_target("/xl/workbook.xml", &cache_definition_uri),
    );
    let workbook_xml = add_workbook_pivot_cache(
        &ensure_workbook_r_namespace(workbook_xml),
        cache_id,
        &next_workbook_rid,
    );
    let content_types_xml = add_pivot_content_type_overrides(
        zip_text(file, "[Content_Types].xml")?,
        &cache_definition_uri,
        &cache_records_uri,
        &pivot_table_uri,
    );

    let mut overrides = BTreeMap::new();
    overrides.insert("[Content_Types].xml".to_string(), content_types_xml);
    overrides.insert("xl/workbook.xml".to_string(), workbook_xml);
    overrides.insert("xl/_rels/workbook.xml.rels".to_string(), workbook_rels_xml);
    overrides.insert(part_name(&cache_definition_uri), cache_definition_xml);
    overrides.insert(part_name(&cache_records_uri), cache_records_xml);
    overrides.insert(
        relationships_part_for(&cache_definition_uri),
        cache_rels_xml,
    );
    overrides.insert(part_name(&pivot_table_uri), pivot_table_xml);
    overrides.insert(relationships_part_for(&pivot_table_uri), pivot_rels_xml);
    overrides.insert(worksheet_rels_part, worksheet_rels_xml);

    Ok(PivotCreateArtifacts {
        name: pivot_name,
        source_sheet: source.sheet.clone(),
        source_range: source.range.clone(),
        target_sheet: target_sheet.name,
        location,
        cache_id,
        cache_definition_uri,
        cache_records_uri,
        pivot_table_uri,
        row_fields: row_fields.to_vec(),
        col_fields: col_fields.to_vec(),
        page_fields: page_fields.to_vec(),
        value_fields: value_specs.iter().map(|spec| spec.name.clone()).collect(),
        warnings,
        overrides,
    })
}

fn pivot_cells_from_range_export(exported: &Value) -> CliResult<Vec<Vec<PivotCell>>> {
    let values = exported
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::unexpected("range export omitted values"))?;
    let mut cells = Vec::new();
    for row in values {
        let row = row
            .as_array()
            .ok_or_else(|| CliError::unexpected("range export value row is not an array"))?;
        let mut out_row = Vec::new();
        for value in row {
            out_row.push(json_value_to_pivot_cell(value));
        }
        cells.push(out_row);
    }
    Ok(cells)
}

fn json_value_to_pivot_cell(value: &Value) -> PivotCell {
    match value {
        Value::Null => PivotCell {
            value: String::new(),
            null: true,
        },
        Value::String(text) => PivotCell {
            value: text.clone(),
            null: false,
        },
        Value::Number(number) => PivotCell {
            value: number.to_string(),
            null: false,
        },
        Value::Bool(value) => PivotCell {
            value: value.to_string(),
            null: false,
        },
        other => PivotCell {
            value: other.to_string(),
            null: false,
        },
    }
}

fn pivot_headers(cells: &[Vec<PivotCell>]) -> CliResult<(Vec<String>, BTreeMap<String, usize>)> {
    if cells.len() < 2 {
        return Err(CliError::invalid_args(
            "pivot source needs a header row and at least one data row",
        ));
    }
    let mut headers = Vec::new();
    let mut index = BTreeMap::new();
    for (i, cell) in cells[0].iter().enumerate() {
        let name = if cell.value.trim().is_empty() {
            format!("Field{}", i + 1)
        } else {
            cell.value.trim().to_string()
        };
        headers.push(name.clone());
        index.insert(name, i);
    }
    Ok((headers, index))
}

fn resolve_pivot_fields(
    role: &str,
    fields: &[String],
    headers: &[String],
    index: &BTreeMap<String, usize>,
    axis: &mut BTreeSet<usize>,
) -> CliResult<Vec<usize>> {
    let mut resolved = Vec::new();
    for field in fields {
        let Some(field_index) = index.get(field).copied() else {
            return Err(CliError::invalid_args(format!(
                "{role} field {:?} not found in source header ({})",
                field,
                headers.join(", ")
            )));
        };
        resolved.push(field_index);
        axis.insert(field_index);
    }
    Ok(resolved)
}

fn build_pivot_field_models(
    cells: &[Vec<PivotCell>],
    headers: &[String],
    axis: &BTreeSet<usize>,
) -> Vec<PivotFieldModel> {
    headers
        .iter()
        .enumerate()
        .map(|(col, header)| {
            let mut field = PivotFieldModel {
                name: header.clone(),
                numeric: true,
                has_items: false,
                items: Vec::new(),
                item_is_num: Vec::new(),
                item_index: BTreeMap::new(),
                min_value: 0.0,
                max_value: 0.0,
            };
            let mut has_data = false;
            let mut first_num = true;
            for row in cells.iter().skip(1) {
                let Some(cell) = row.get(col) else {
                    continue;
                };
                if cell.null || cell.value.is_empty() {
                    continue;
                }
                has_data = true;
                if let Ok(number) = cell.value.parse::<f64>() {
                    if first_num || number < field.min_value {
                        field.min_value = number;
                    }
                    if first_num || number > field.max_value {
                        field.max_value = number;
                    }
                    first_num = false;
                } else {
                    field.numeric = false;
                }
            }
            if !has_data {
                field.numeric = false;
            }
            field.has_items = axis.contains(&col) || !field.numeric;
            if field.has_items {
                for row in cells.iter().skip(1) {
                    let value = row
                        .get(col)
                        .filter(|cell| !cell.null)
                        .map(|cell| cell.value.clone())
                        .unwrap_or_default();
                    if field.item_index.contains_key(&value) {
                        continue;
                    }
                    field.item_index.insert(value.clone(), field.items.len());
                    field
                        .item_is_num
                        .push(!value.is_empty() && value.parse::<f64>().is_ok());
                    field.items.push(value);
                }
            }
            field
        })
        .collect()
}

fn render_cache_definition_xml(
    source: &PivotSource,
    fields: &[PivotFieldModel],
    record_count: usize,
    records_rid: &str,
) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotCacheDefinition xmlns="{XLSX_NS}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="{}" refreshOnLoad="1" refreshedBy="ooxml-cli" createdVersion="6" refreshedVersion="6" minRefreshableVersion="3" recordCount="{}">
  <cacheSource type="worksheet"><worksheetSource ref="{}" sheet="{}"/></cacheSource>
  <cacheFields count="{}">"#,
        xml_attr_escape(records_rid),
        record_count,
        xml_attr_escape(&source.range),
        xml_attr_escape(&source.sheet),
        fields.len()
    );
    for field in fields {
        xml.push_str(&format!(
            r#"
    <cacheField name="{}" numFmtId="0">"#,
            xml_attr_escape(&field.name)
        ));
        if field.has_items {
            let has_blank = field.items.iter().any(|item| item.is_empty());
            let has_number = field.item_is_num.iter().any(|value| *value);
            let has_string = field
                .items
                .iter()
                .zip(field.item_is_num.iter())
                .any(|(item, is_num)| !item.is_empty() && !*is_num);
            xml.push_str(&format!(
                r#"<sharedItems containsSemiMixedTypes="{}""#,
                bool_int(has_string || has_blank)
            ));
            if has_blank {
                xml.push_str(r#" containsBlank="1""#);
            }
            xml.push_str(&format!(r#" containsString="{}""#, bool_int(has_string)));
            if has_number {
                xml.push_str(r#" containsNumber="1""#);
            }
            xml.push_str(&format!(r#" count="{}">"#, field.items.len()));
            for (item, is_num) in field.items.iter().zip(field.item_is_num.iter()) {
                if item.is_empty() {
                    xml.push_str("<m/>");
                } else if *is_num {
                    xml.push_str(&format!(r#"<n v="{}"/>"#, xml_attr_escape(item)));
                } else {
                    xml.push_str(&format!(r#"<s v="{}"/>"#, xml_attr_escape(item)));
                }
            }
            xml.push_str("</sharedItems>");
        } else {
            xml.push_str(&format!(
                r#"<sharedItems containsString="0" containsNumber="1" minValue="{}" maxValue="{}"/>"#,
                trim_float(field.min_value),
                trim_float(field.max_value)
            ));
        }
        xml.push_str("</cacheField>");
    }
    xml.push_str(
        r#"
  </cacheFields>
</pivotCacheDefinition>"#,
    );
    xml
}

fn render_cache_records_xml(cells: &[Vec<PivotCell>], fields: &[PivotFieldModel]) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotCacheRecords xmlns="{XLSX_NS}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" count="{}">"#,
        cells.len().saturating_sub(1)
    );
    for row in cells.iter().skip(1) {
        xml.push_str("<r>");
        for (col, field) in fields.iter().enumerate() {
            let value = row
                .get(col)
                .filter(|cell| !cell.null)
                .map(|cell| cell.value.as_str())
                .unwrap_or("");
            if field.has_items {
                let index = field.item_index.get(value).copied().unwrap_or(0);
                xml.push_str(&format!(r#"<x v="{index}"/>"#));
            } else if value.is_empty() {
                xml.push_str("<m/>");
            } else {
                xml.push_str(&format!(r#"<n v="{}"/>"#, xml_attr_escape(value)));
            }
        }
        xml.push_str("</r>");
    }
    xml.push_str("</pivotCacheRecords>");
    xml
}

#[allow(clippy::too_many_arguments)]
fn render_pivot_table_xml(
    name: &str,
    cache_id: i32,
    location: &str,
    fields: &[PivotFieldModel],
    row_idx: &[usize],
    col_idx: &[usize],
    page_idx: &[usize],
    data_fields: &[PivotDataField],
    data_field_indices: &BTreeSet<usize>,
) -> String {
    let mut role = BTreeMap::new();
    for index in row_idx {
        role.insert(*index, "axisRow");
    }
    for index in col_idx {
        role.insert(*index, "axisCol");
    }
    for index in page_idx {
        role.insert(*index, "axisPage");
    }
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotTableDefinition xmlns="{XLSX_NS}" name="{}" cacheId="{cache_id}" applyNumberFormats="0" applyBorderFormats="0" applyFontFormats="0" applyPatternFormats="0" applyAlignmentFormats="0" applyWidthHeightFormats="1" dataCaption="Values" updatedVersion="6" minRefreshableVersion="3" useAutoFormatting="1" itemPrintTitles="1" createdVersion="6" indent="0" outline="1" outlineData="1" multipleFieldFilters="0">"#,
        xml_attr_escape(name)
    );
    xml.push_str(&format!(
        r#"<location ref="{}" firstHeaderRow="1" firstDataRow="2" firstDataCol="1""#,
        xml_attr_escape(location)
    ));
    if !page_idx.is_empty() {
        xml.push_str(&format!(
            r#" rowPageCount="1" colPageCount="{}""#,
            page_idx.len()
        ));
    }
    xml.push_str("/>");
    xml.push_str(&format!(r#"<pivotFields count="{}">"#, fields.len()));
    for (index, field) in fields.iter().enumerate() {
        if let Some(axis) = role.get(&index) {
            xml.push_str(&format!(
                r#"<pivotField axis="{axis}" showAll="0"><items count="{}">"#,
                field.items.len() + 1
            ));
            for item_index in 0..field.items.len() {
                xml.push_str(&format!(r#"<item x="{item_index}"/>"#));
            }
            xml.push_str(r#"<item t="default"/></items></pivotField>"#);
        } else if data_field_indices.contains(&index) {
            xml.push_str(r#"<pivotField dataField="1" showAll="0"/>"#);
        } else {
            xml.push_str(r#"<pivotField showAll="0"/>"#);
        }
    }
    xml.push_str("</pivotFields>");
    if !row_idx.is_empty() {
        xml.push_str(&format!(r#"<rowFields count="{}">"#, row_idx.len()));
        for index in row_idx {
            xml.push_str(&format!(r#"<field x="{index}"/>"#));
        }
        xml.push_str("</rowFields>");
    }
    if !col_idx.is_empty() {
        xml.push_str(&format!(r#"<colFields count="{}">"#, col_idx.len()));
        for index in col_idx {
            xml.push_str(&format!(r#"<field x="{index}"/>"#));
        }
        xml.push_str("</colFields>");
    }
    if !page_idx.is_empty() {
        xml.push_str(&format!(r#"<pageFields count="{}">"#, page_idx.len()));
        for index in page_idx {
            xml.push_str(&format!(r#"<pageField fld="{index}" hier="-1"/>"#));
        }
        xml.push_str("</pageFields>");
    }
    xml.push_str(&format!(r#"<dataFields count="{}">"#, data_fields.len()));
    for field in data_fields {
        xml.push_str(&format!(
            r#"<dataField name="{}" fld="{}""#,
            xml_attr_escape(&field.caption),
            field.field_index
        ));
        if field.subtotal != "sum" {
            xml.push_str(&format!(
                r#" subtotal="{}""#,
                xml_attr_escape(&field.subtotal)
            ));
        }
        xml.push_str(r#" baseField="0" baseItem="0"/>"#);
    }
    xml.push_str(
        r#"</dataFields><pivotTableStyleInfo name="PivotStyleLight16" showRowHeaders="1" showColHeaders="1" showRowStripes="0" showColStripes="0" showLastColumn="1"/></pivotTableDefinition>"#,
    );
    xml
}

fn render_relationships_xml(relationships: &[(&str, &str, String)]) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#.to_string();
    for (id, rel_type, target) in relationships {
        xml.push_str(&format!(
            r#"<Relationship Id="{}" Type="{}" Target="{}"/>"#,
            xml_attr_escape(id),
            xml_attr_escape(rel_type),
            xml_attr_escape(target)
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

fn add_workbook_pivot_cache(workbook_xml: &str, cache_id: i32, rid: &str) -> String {
    let entry = format!(
        r#"<pivotCache cacheId="{cache_id}" r:id="{}"/>"#,
        xml_attr_escape(rid)
    );
    if let Some(pos) = workbook_xml.find("</pivotCaches>") {
        let mut out = String::with_capacity(workbook_xml.len() + entry.len());
        out.push_str(&workbook_xml[..pos]);
        out.push_str(&entry);
        out.push_str(&workbook_xml[pos..]);
        return out;
    }
    let wrapper = format!("<pivotCaches>{entry}</pivotCaches>");
    if let Some(pos) = workbook_xml.find("</sheets>") {
        let pos = pos + "</sheets>".len();
        let mut out = String::with_capacity(workbook_xml.len() + wrapper.len());
        out.push_str(&workbook_xml[..pos]);
        out.push_str(&wrapper);
        out.push_str(&workbook_xml[pos..]);
        return out;
    }
    if let Some(pos) = workbook_xml.find("</workbook>") {
        let mut out = String::with_capacity(workbook_xml.len() + wrapper.len());
        out.push_str(&workbook_xml[..pos]);
        out.push_str(&wrapper);
        out.push_str(&workbook_xml[pos..]);
        return out;
    }
    workbook_xml.to_string()
}

fn ensure_workbook_r_namespace(mut workbook_xml: String) -> String {
    if workbook_xml.contains("xmlns:r=") {
        return workbook_xml;
    }
    if let Some(pos) = workbook_xml.find("<workbook") {
        let insert_pos = workbook_xml[pos..]
            .find('>')
            .map(|offset| pos + offset)
            .unwrap_or(pos);
        workbook_xml.insert_str(
            insert_pos,
            r#" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#,
        );
    }
    workbook_xml
}

fn add_pivot_content_type_overrides(
    xml: String,
    cache_definition_uri: &str,
    cache_records_uri: &str,
    pivot_table_uri: &str,
) -> String {
    let xml = ensure_content_type_override(xml, cache_definition_uri, CONTENT_TYPE_PIVOT_CACHE);
    let xml = ensure_content_type_override(xml, cache_records_uri, CONTENT_TYPE_PIVOT_RECORDS);
    ensure_content_type_override(xml, pivot_table_uri, CONTENT_TYPE_PIVOT_TABLE)
}

fn optional_relationship_entries(file: &str, part_uri: &str) -> CliResult<Vec<RelationshipEntry>> {
    let rel_part = relationships_part_for(part_uri);
    match optional_zip_text(file, &rel_part)? {
        Some(xml) => Ok(relationship_entries_from_xml(&xml)),
        None => Ok(Vec::new()),
    }
}

fn optional_zip_text(file: &str, part: &str) -> CliResult<Option<String>> {
    match zip_text(file, part) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.message.starts_with("missing zip part ") => Ok(None),
        Err(err) => Err(err),
    }
}

fn sheet_part_uri(sheet: &WorkbookSheet, workbook_rels: &[RelationshipEntry]) -> Option<String> {
    workbook_rels
        .iter()
        .find(|rel| rel.id == sheet.rel_id && rel.rel_type == REL_WORKSHEET)
        .map(|rel| resolve_relationship_target("/xl/workbook.xml", &rel.target))
}

fn split_comma_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_pivot_value_fields(specs: &[String]) -> CliResult<Vec<PivotValueSpec>> {
    let mut out = Vec::new();
    for raw in specs {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let (name, aggregation) = if let Some((name, aggregation)) = raw.rsplit_once(':') {
            (name.trim(), aggregation.trim())
        } else {
            (raw, "sum")
        };
        if name.is_empty() {
            return Err(CliError::invalid_args("value field name cannot be empty"));
        }
        out.push(PivotValueSpec {
            name: name.to_string(),
            aggregation: aggregation.to_string(),
        });
    }
    if out.is_empty() {
        return Err(CliError::invalid_args(
            "specify at least one --values field",
        ));
    }
    Ok(out)
}

fn normalize_pivot_aggregation(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "sum" => Some("sum".to_string()),
        "count" => Some("count".to_string()),
        "average" | "avg" => Some("average".to_string()),
        "max" => Some("max".to_string()),
        "min" => Some("min".to_string()),
        "product" => Some("product".to_string()),
        "countnums" => Some("countNums".to_string()),
        "stddev" => Some("stdDev".to_string()),
        "var" => Some("var".to_string()),
        _ => None,
    }
}

fn pivot_data_caption(subtotal: &str, field: &str) -> String {
    let label = match subtotal {
        "sum" => "Sum",
        "count" => "Count",
        "average" => "Average",
        "max" => "Max",
        "min" => "Min",
        "product" => "Product",
        "countNums" => "Count",
        "stdDev" => "StdDev",
        "var" => "Var",
        _ => "Sum",
    };
    format!("{label} of {field}")
}

fn pivot_location(anchor: (u32, u32), row_field_count: usize, value_count: usize) -> String {
    let mut cols = row_field_count.max(1) + value_count;
    if cols < 1 {
        cols = 1;
    }
    let end_col = anchor.0 + cols as u32 - 1;
    let end_row = anchor.1 + 4;
    format!(
        "{}{}:{}{}",
        col_name(anchor.0),
        anchor.1,
        col_name(end_col),
        end_row
    )
}

fn allocate_numbered_part(entries: &mut BTreeSet<String>, prefix: &str, suffix: &str) -> String {
    let mut number = 1u32;
    loop {
        let part = format!("{prefix}{number}{suffix}");
        if !entries.contains(part.trim_start_matches('/')) {
            entries.insert(part.trim_start_matches('/').to_string());
            return part;
        }
        number += 1;
    }
}

fn next_pivot_cache_id(workbook_xml: &str) -> i32 {
    let mut max_id = 0;
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "pivotCache" =>
            {
                max_id = max_id.max(parse_i32(attr(&e, "cacheId").as_deref(), 0));
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    max_id + 1
}

fn normalize_part_uri(part_uri: &str) -> String {
    format!("/{}", part_uri.trim_start_matches('/'))
}

fn part_name(part_uri: &str) -> String {
    part_uri.trim_start_matches('/').to_string()
}

fn empty_relationships_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
}

fn parse_i32(value: Option<&str>, fallback: i32) -> i32 {
    value
        .and_then(|value| value.trim().parse::<i32>().ok())
        .unwrap_or(fallback)
}

fn parse_bool_attr(value: Option<&str>) -> bool {
    matches!(
        value
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "on"
    )
}

fn bool_int(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

fn normalize_axis(value: &str) -> String {
    match value {
        "axisRow" => "row",
        "axisCol" => "column",
        "axisPage" => "filter",
        "axisValues" => "data",
        other => other,
    }
    .to_string()
}

fn first_enabled_subtotal(e: &BytesStart<'_>) -> Option<String> {
    [
        "sumSubtotal",
        "countASubtotal",
        "avgSubtotal",
        "maxSubtotal",
        "minSubtotal",
        "productSubtotal",
        "countSubtotal",
        "stdDevSubtotal",
        "stdDevPSubtotal",
        "varSubtotal",
        "varPSubtotal",
    ]
    .iter()
    .find(|name| parse_bool_attr(attr(e, name).as_deref()))
    .map(|name| name.trim_end_matches("Subtotal").to_string())
}

fn trim_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        let text = value.to_string();
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
