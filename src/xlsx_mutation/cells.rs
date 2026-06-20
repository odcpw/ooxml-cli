use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::{
    XlsxMatrixCell, add_xlsx_range_mutation_commands, normalize_xlsx_write_cell,
    replace_xlsx_dimension, set_xlsx_range_in_sheet_xml, validate_xlsx_mutation_output_flags,
    xlsx_range_destination_json, xlsx_range_destination_json_with_max,
};
use crate::{
    CellValue, CliError, CliResult, RangeBounds, WorkbookSheet,
    add_xlsx_formula_recalc_package_updates, col_name, copy_zip_with_part_overrides_and_removals,
    is_xlsx_handle, normalize_xl_target, normalize_xlsx_cell_ref, parse_cell_ref, parse_cli_range,
    parse_xlsx_cell_handle, parse_xlsx_row_spans, range_bounds_ref, rebuild_xlsx_sheet_data,
    relationships, render_xlsx_row, resolve_sheet, resolve_sheet_by_sheet_id_unique,
    shared_strings, sheet_cells, validate, workbook_sheets, xlsx_ranges_set_temp_path,
    xlsx_sheet_data_span, xlsx_styles, xlsx_used_range_from_cell_refs, zip_text,
};

pub(crate) struct XlsxCellsSetOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) cell: Option<&'a str>,
    pub(crate) ref_: Option<&'a str>,
    pub(crate) value: Option<&'a str>,
    pub(crate) formula: Option<&'a str>,
    pub(crate) value_type: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxCellsClearOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: Option<&'a str>,
    pub(crate) ref_: Option<&'a str>,
    pub(crate) readback_max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxCellsSetBatchOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) cells: Option<&'a str>,
    pub(crate) cells_file: Option<&'a str>,
    pub(crate) details: bool,
    pub(crate) readback_max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

struct XlsxCellsSetTarget {
    cell_ref: String,
    handle_sheet_id: Option<u32>,
    from_handle: bool,
}

struct XlsxCellsSetPrevious {
    exists: bool,
    previous_type: Option<String>,
    previous_value: Option<String>,
}

struct XlsxCellsClearTarget {
    bounds: RangeBounds,
    handle_sheet_id: Option<u32>,
    from_handle: bool,
}

#[derive(Clone)]
struct XlsxCellsSetBatchAssignment {
    cell_ref: String,
    value_type: String,
    value: String,
    formula: String,
}

struct XlsxCellsClearStats {
    cleared: usize,
    refs: Vec<String>,
    formula_invalidated: bool,
}

pub(crate) fn xlsx_cells_set(file: &str, options: XlsxCellsSetOptions<'_>) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let target = resolve_xlsx_cells_set_cell(options.cell, options.ref_)?;
    let (value_type, value) =
        resolve_xlsx_cells_set_value(options.value, options.formula, options.value_type)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = if let Some(sheet_id) = target.handle_sheet_id {
        resolve_sheet_by_sheet_id_unique(&sheets, sheet_id, "cell handle")?
    } else {
        resolve_sheet(&sheets, options.sheet.unwrap_or("1"))?
    };
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let sheet_target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(sheet_target);
    let sheet_xml = zip_text(file, &sheet_part)?;
    if target.from_handle && !xlsx_cell_exists_in_sheet_xml(&sheet_xml, &target.cell_ref)? {
        return Err(CliError::target_not_found(format!(
            "HANDLE_STALE: cell {} no longer exists on sheet {:?}; row/column structure may have shifted",
            target.cell_ref, sheet.name
        )));
    }
    let previous = xlsx_cells_set_previous(file, &sheet_xml, &target.cell_ref)?;
    let (col, row) = parse_cell_ref(&target.cell_ref)?;
    let bounds = RangeBounds {
        start_col: col,
        start_row: row,
        end_col: col,
        end_row: row,
    };
    let formula = if value_type == "formula" {
        value.clone()
    } else {
        String::new()
    };
    let rows = vec![vec![XlsxMatrixCell {
        kind: value_type.clone(),
        value: value.clone(),
        formula,
        null: false,
    }]];
    let (updated_xml, stats) =
        set_xlsx_range_in_sheet_xml(&sheet_xml, bounds, &rows, "skip", true)?;

    let output_path = options.out.filter(|value| !value.is_empty());
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
    let mut removals = BTreeSet::new();
    overrides.insert(sheet_part.clone(), updated_xml);
    add_xlsx_formula_recalc_package_updates(
        file,
        stats.formula_seen,
        stats.formula_invalidated,
        &mut overrides,
        &mut removals,
    )?;
    copy_zip_with_part_overrides_and_removals(file, &readback_path, &overrides, &removals)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let destination = xlsx_range_destination_json(
        &readback_path,
        commit_path,
        &sheet,
        &sheet_part,
        &target.cell_ref,
    )?;
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.is_empty()) {
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

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("ref".to_string(), json!(target.cell_ref));
    if let Some(handle) = xlsx_cell_handle_string(&sheet, &target.cell_ref, &sheets) {
        result.insert("handle".to_string(), json!(handle));
    }
    result.insert("type".to_string(), json!(value_type));
    result.insert("value".to_string(), json!(value));
    if let Some(previous_type) = previous.previous_type {
        result.insert("previousType".to_string(), json!(previous_type));
    }
    if let Some(previous_value) = previous.previous_value {
        result.insert("previousValue".to_string(), json!(previous_value));
    }
    result.insert("created".to_string(), json!(!previous.exists));
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_range_mutation_commands(
        &mut result,
        commit_path,
        &format!("sheetId:{}", sheet.sheet_id),
        &target.cell_ref,
    );
    Ok(Value::Object(result))
}

pub(crate) fn xlsx_cells_clear(file: &str, options: XlsxCellsClearOptions<'_>) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    if options.readback_max_cells < 0 {
        return Err(CliError::invalid_args("--readback-max-cells must be >= 0"));
    }
    let target = resolve_xlsx_cells_clear_range(options.range, options.ref_)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = if let Some(sheet_id) = target.handle_sheet_id {
        resolve_sheet_by_sheet_id_unique(&sheets, sheet_id, "cell handle")?
    } else {
        resolve_sheet(&sheets, options.sheet.unwrap_or("1"))?
    };
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let sheet_target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(sheet_target);
    let sheet_xml = zip_text(file, &sheet_part)?;
    if target.from_handle
        && !xlsx_cell_exists_in_sheet_xml(&sheet_xml, &range_bounds_ref(target.bounds))?
    {
        return Err(CliError::target_not_found(format!(
            "HANDLE_STALE: cell {} no longer exists on sheet {:?}; row/column structure may have shifted",
            range_bounds_ref(target.bounds),
            sheet.name
        )));
    }
    let bounds = target.bounds.normalized();
    let range = range_bounds_ref(bounds);
    let (updated_xml, stats) = clear_xlsx_cells_in_sheet_xml(&sheet_xml, bounds)?;

    let output_path = options.out.filter(|value| !value.is_empty());
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
    let mut removals = BTreeSet::new();
    overrides.insert(sheet_part.clone(), updated_xml);
    add_xlsx_formula_recalc_package_updates(
        file,
        false,
        stats.formula_invalidated,
        &mut overrides,
        &mut removals,
    )?;
    copy_zip_with_part_overrides_and_removals(file, &readback_path, &overrides, &removals)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let destination = xlsx_range_destination_json_with_max(
        &readback_path,
        commit_path,
        &sheet,
        &sheet_part,
        &range,
        options.readback_max_cells,
    )?;
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.is_empty()) {
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

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("range".to_string(), json!(range.clone()));
    result.insert("cleared".to_string(), json!(stats.cleared));
    result.insert("refs".to_string(), json!(stats.refs));
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_range_mutation_commands(
        &mut result,
        commit_path,
        &format!("sheetId:{}", sheet.sheet_id),
        &range,
    );
    Ok(Value::Object(result))
}

pub(crate) fn xlsx_cells_set_batch(
    file: &str,
    options: XlsxCellsSetBatchOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    if options.readback_max_cells < 0 {
        return Err(CliError::invalid_args("--readback-max-cells must be >= 0"));
    }
    let assignments = resolve_xlsx_cells_set_batch_assignments(options.cells, options.cells_file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, options.sheet.unwrap_or("1"))?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let sheet_target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(sheet_target);
    let sheet_xml = zip_text(file, &sheet_part)?;

    let mut cells_json = Vec::new();
    let mut min_col = u32::MAX;
    let mut min_row = u32::MAX;
    let mut max_col = 0u32;
    let mut max_row = 0u32;
    let mut formula_count = 0usize;
    for assignment in &assignments {
        let (col, row) = parse_cell_ref(&assignment.cell_ref)?;
        min_col = min_col.min(col);
        min_row = min_row.min(row);
        max_col = max_col.max(col);
        max_row = max_row.max(row);
    }
    let bounds = RangeBounds {
        start_col: min_col,
        start_row: min_row,
        end_col: max_col,
        end_row: max_row,
    };
    let cols = bounds.col_count() as usize;
    let rows = bounds.row_count() as usize;
    let mut matrix = vec![
        vec![
            XlsxMatrixCell {
                kind: "empty".to_string(),
                value: String::new(),
                formula: String::new(),
                null: true,
            };
            cols
        ];
        rows
    ];
    for assignment in &assignments {
        let previous = xlsx_cells_set_previous(file, &sheet_xml, &assignment.cell_ref)?;
        let (col, row) = parse_cell_ref(&assignment.cell_ref)?;
        let row_idx = (row - min_row) as usize;
        let col_idx = (col - min_col) as usize;
        if assignment.value_type == "formula" {
            formula_count += 1;
        }
        matrix[row_idx][col_idx] = XlsxMatrixCell {
            kind: assignment.value_type.clone(),
            value: assignment.value.clone(),
            formula: assignment.formula.clone(),
            null: false,
        };
        if options.details {
            let mut cell = Map::new();
            cell.insert("ref".to_string(), json!(assignment.cell_ref));
            cell.insert("type".to_string(), json!(assignment.value_type));
            cell.insert("value".to_string(), json!(assignment.value));
            if let Some(previous_type) = previous.previous_type {
                cell.insert("previousType".to_string(), json!(previous_type));
            }
            if let Some(previous_value) = previous.previous_value {
                cell.insert("previousValue".to_string(), json!(previous_value));
            }
            cell.insert("created".to_string(), json!(!previous.exists));
            cells_json.push(Value::Object(cell));
        }
    }
    let range = range_bounds_ref(bounds);
    let (updated_xml, stats) =
        set_xlsx_range_in_sheet_xml(&sheet_xml, bounds, &matrix, "skip", true)?;

    let output_path = options.out.filter(|value| !value.is_empty());
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
    let mut removals = BTreeSet::new();
    overrides.insert(sheet_part.clone(), updated_xml);
    add_xlsx_formula_recalc_package_updates(
        file,
        stats.formula_seen,
        stats.formula_invalidated,
        &mut overrides,
        &mut removals,
    )?;
    copy_zip_with_part_overrides_and_removals(file, &readback_path, &overrides, &removals)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let destination = xlsx_range_destination_json_with_max(
        &readback_path,
        commit_path,
        &sheet,
        &sheet_part,
        &range,
        options.readback_max_cells,
    )?;
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.is_empty()) {
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

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("updated".to_string(), json!(stats.updated));
    result.insert("created".to_string(), json!(stats.created));
    result.insert("formulaCount".to_string(), json!(formula_count));
    result.insert("range".to_string(), json!(range.clone()));
    if options.details {
        result.insert("cells".to_string(), Value::Array(cells_json));
    }
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_range_mutation_commands(
        &mut result,
        commit_path,
        &format!("sheetId:{}", sheet.sheet_id),
        &range,
    );
    Ok(Value::Object(result))
}

fn resolve_xlsx_cells_set_cell(
    cell: Option<&str>,
    ref_: Option<&str>,
) -> CliResult<XlsxCellsSetTarget> {
    if cell.is_some() && ref_.is_some() {
        return Err(CliError::invalid_args(
            "cannot specify both --cell and --ref",
        ));
    }
    let Some(raw_ref) = cell.or(ref_) else {
        return Err(CliError::invalid_args("must specify --cell"));
    };
    let raw_ref = raw_ref.trim();
    if raw_ref.is_empty() {
        return Err(CliError::invalid_args("must specify --cell"));
    }
    if is_xlsx_handle(raw_ref) {
        let (sheet_id, cell_ref) = parse_xlsx_cell_handle(raw_ref)?;
        return Ok(XlsxCellsSetTarget {
            cell_ref,
            handle_sheet_id: Some(sheet_id),
            from_handle: true,
        });
    }
    if raw_ref.contains(':') {
        return Err(CliError::invalid_args(
            "--cell must be a single cell reference, not a range",
        ));
    }
    Ok(XlsxCellsSetTarget {
        cell_ref: normalize_xlsx_cell_ref(raw_ref, "--cell")?,
        handle_sheet_id: None,
        from_handle: false,
    })
}

fn resolve_xlsx_cells_clear_range(
    range: Option<&str>,
    ref_: Option<&str>,
) -> CliResult<XlsxCellsClearTarget> {
    if range.is_some() && ref_.is_some() {
        return Err(CliError::invalid_args(
            "cannot specify both --range and --ref",
        ));
    }
    let Some(raw_range) = range.or(ref_) else {
        return Err(CliError::invalid_args("must specify --range"));
    };
    let raw_range = raw_range.trim();
    if raw_range.is_empty() {
        return Err(CliError::invalid_args("must specify --range"));
    }
    if is_xlsx_handle(raw_range) {
        let (sheet_id, cell_ref) = parse_xlsx_cell_handle(raw_range)?;
        let (col, row) = parse_cell_ref(&cell_ref)?;
        return Ok(XlsxCellsClearTarget {
            bounds: RangeBounds {
                start_col: col,
                start_row: row,
                end_col: col,
                end_row: row,
            },
            handle_sheet_id: Some(sheet_id),
            from_handle: true,
        });
    }
    let bounds = parse_cli_range(raw_range)
        .map_err(|err| CliError::invalid_args(format!("invalid --range: {}", err.message)))?;
    Ok(XlsxCellsClearTarget {
        bounds,
        handle_sheet_id: None,
        from_handle: false,
    })
}

fn resolve_xlsx_cells_set_batch_assignments(
    cells: Option<&str>,
    cells_file: Option<&str>,
) -> CliResult<Vec<XlsxCellsSetBatchAssignment>> {
    match (cells, cells_file) {
        (Some(_), Some(_)) | (None, None) => {
            return Err(CliError::invalid_args(
                "must specify exactly one of --cells or --cells-file",
            ));
        }
        _ => {}
    }
    let data = if let Some(cells) = cells {
        cells.to_string()
    } else if cells_file == Some("-") {
        let mut data = String::new();
        let mut stdin = std::io::stdin();
        std::io::Read::read_to_string(&mut stdin, &mut data)
            .map_err(|err| CliError::unexpected(format!("failed to read stdin: {err}")))?;
        data
    } else {
        let path = cells_file.unwrap_or_default();
        fs::read_to_string(path)
            .map_err(|_| CliError::file_not_found(format!("file not found: {path}")))?
    };
    let raw: Value = serde_json::from_str(&data)
        .map_err(|err| CliError::invalid_args(format!("invalid cells JSON: {err}")))?;
    let entries = raw
        .as_array()
        .ok_or_else(|| CliError::invalid_args("invalid cells JSON: expected array"))?;
    if entries.is_empty() {
        return Err(CliError::invalid_args("cells batch cannot be empty"));
    }
    let mut by_ref = BTreeMap::<(u32, u32), XlsxCellsSetBatchAssignment>::new();
    for (idx, entry) in entries.iter().enumerate() {
        let object = entry
            .as_object()
            .ok_or_else(|| CliError::invalid_args(format!("cells[{idx}] must be an object")))?;
        let raw_ref = object
            .get("ref")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                object
                    .get("cell")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
            })
            .ok_or_else(|| CliError::invalid_args(format!("cells[{idx}] missing ref")))?;
        let cell_ref = normalize_xlsx_cell_ref(raw_ref, "cell reference").map_err(|err| {
            CliError::invalid_args(format!(
                "invalid cell reference {raw_ref:?}: {}",
                err.message
            ))
        })?;
        let value_type = object
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("string");
        let mut value_type = normalize_xlsx_cell_value_type(value_type)?;
        let mut value = object
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if let Some(formula) = object.get("formula").and_then(Value::as_str) {
            if !value.is_empty() {
                return Err(CliError::invalid_args(format!(
                    "cells[{idx}] cannot specify both value and formula"
                )));
            }
            value_type = "formula".to_string();
            value = formula.to_string();
        }
        if value.is_empty() {
            return Err(CliError::invalid_args(format!(
                "cells[{idx}] value cannot be empty; use xlsx cells clear"
            )));
        }
        let formula = if value_type == "formula" {
            value.clone()
        } else {
            String::new()
        };
        let cell = XlsxMatrixCell {
            kind: value_type,
            value,
            formula,
            null: false,
        };
        let (value_type, value) = normalize_xlsx_write_cell(&cell).map_err(|err| {
            CliError::invalid_args(format!(
                "invalid value for cell {cell_ref}: {}",
                err.message
            ))
        })?;
        let formula = if value_type == "formula" {
            value.clone()
        } else {
            String::new()
        };
        let (col, row) = parse_cell_ref(&cell_ref)?;
        by_ref.insert(
            (row, col),
            XlsxCellsSetBatchAssignment {
                cell_ref,
                value_type,
                value,
                formula,
            },
        );
    }
    Ok(by_ref.into_values().collect())
}

fn clear_xlsx_cells_in_sheet_xml(
    xml: &str,
    bounds: RangeBounds,
) -> CliResult<(String, XlsxCellsClearStats)> {
    let sheet_data = xlsx_sheet_data_span(xml)?;
    let row_spans = parse_xlsx_row_spans(xml, sheet_data.as_ref())?;
    let mut changed_rows = BTreeMap::<u32, String>::new();
    let mut stats = XlsxCellsClearStats {
        cleared: 0,
        refs: Vec::new(),
        formula_invalidated: false,
    };
    let bounds = bounds.normalized();
    for row_number in bounds.start_row..=bounds.end_row {
        let Some(existing_row) = row_spans.get(&row_number) else {
            continue;
        };
        let mut rendered_cells = existing_row
            .cells
            .iter()
            .map(|(col, cell)| (*col, cell.xml.clone()))
            .collect::<BTreeMap<u32, String>>();
        let mut row_changed = false;
        for col_number in bounds.start_col..=bounds.end_col {
            let Some(existing_cell) = existing_row.cells.get(&col_number) else {
                continue;
            };
            let addr = format!("{}{}", col_name(col_number), row_number);
            stats.cleared += 1;
            stats.refs.push(addr.clone());
            if existing_cell.has_formula {
                stats.formula_invalidated = true;
            }
            if existing_cell
                .attrs
                .get("s")
                .is_some_and(|value| !value.is_empty())
            {
                rendered_cells.insert(
                    col_number,
                    super::render_empty_xlsx_cell_with_attrs(&addr, Some(&existing_cell.attrs)),
                );
            } else {
                rendered_cells.remove(&col_number);
            }
            row_changed = true;
        }
        if row_changed {
            changed_rows.insert(
                row_number,
                render_xlsx_row(row_number, Some(existing_row), rendered_cells),
            );
        }
    }
    let updated = rebuild_xlsx_sheet_data(xml, sheet_data.as_ref(), &row_spans, &changed_rows)?;
    let used_range = xlsx_used_range_from_cell_refs(&updated);
    Ok((
        replace_xlsx_dimension(&updated, used_range.as_deref()),
        stats,
    ))
}

fn resolve_xlsx_cells_set_value(
    value: Option<&str>,
    formula: Option<&str>,
    value_type: Option<&str>,
) -> CliResult<(String, String)> {
    if formula.is_some() && value.is_some() {
        return Err(CliError::invalid_args(
            "cannot specify both --value and --formula",
        ));
    }
    let value_type = normalize_xlsx_cell_value_type(value_type.unwrap_or("string"))?;
    if let Some(formula) = formula {
        if formula.trim().is_empty() {
            return Err(CliError::invalid_args("--formula cannot be empty"));
        }
        let cell = XlsxMatrixCell {
            kind: "formula".to_string(),
            value: formula.to_string(),
            formula: formula.to_string(),
            null: false,
        };
        let (_, normalized) = normalize_xlsx_write_cell(&cell)?;
        return Ok(("formula".to_string(), normalized));
    }
    let Some(value) = value else {
        return Err(CliError::invalid_args("must specify --value or --formula"));
    };
    if value.is_empty() {
        return Err(CliError::invalid_args(
            "--value cannot be empty; use xlsx cells clear",
        ));
    }
    let cell = XlsxMatrixCell {
        kind: value_type.clone(),
        value: value.to_string(),
        formula: if value_type == "formula" {
            value.to_string()
        } else {
            String::new()
        },
        null: false,
    };
    normalize_xlsx_write_cell(&cell)
}

fn normalize_xlsx_cell_value_type(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "string" => Ok("string".to_string()),
        "number" => Ok("number".to_string()),
        "bool" | "boolean" => Ok("bool".to_string()),
        "formula" => Ok("formula".to_string()),
        "auto" => Ok("auto".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "invalid --type {value:?} (must be string, number, bool, formula, or auto)"
        ))),
    }
}

fn xlsx_cells_set_previous(
    file: &str,
    sheet_xml: &str,
    cell_ref: &str,
) -> CliResult<XlsxCellsSetPrevious> {
    let exists = xlsx_cell_exists_in_sheet_xml(sheet_xml, cell_ref)?;
    if !exists {
        return Ok(XlsxCellsSetPrevious {
            exists: false,
            previous_type: None,
            previous_value: None,
        });
    }
    let shared_strings = shared_strings(file).unwrap_or_default();
    let styles = xlsx_styles(file).unwrap_or_default();
    let cells = sheet_cells(sheet_xml, &shared_strings, &styles);
    let Some(cell) = cells.get(cell_ref) else {
        return Ok(XlsxCellsSetPrevious {
            exists: true,
            previous_type: None,
            previous_value: None,
        });
    };
    let previous_type = xlsx_cells_set_previous_type(cell);
    let previous_value = xlsx_cells_set_previous_value(cell);
    Ok(XlsxCellsSetPrevious {
        exists: true,
        previous_type: previous_type.filter(|value| !value.is_empty()),
        previous_value: previous_value.filter(|value| !value.is_empty()),
    })
}

pub(crate) fn xlsx_cells_set_previous_type(cell: &CellValue) -> Option<String> {
    if cell.has_formula {
        Some("formula".to_string())
    } else if cell.kind == "boolean" {
        Some("bool".to_string())
    } else if cell.kind == "empty" {
        None
    } else {
        Some(cell.kind.clone())
    }
}

pub(crate) fn xlsx_cells_set_previous_value(cell: &CellValue) -> Option<String> {
    if cell.has_formula {
        Some(cell.formula.clone())
    } else if cell.kind == "string" {
        Some(cell.display_value.clone())
    } else if !cell.raw_value.is_empty() {
        Some(cell.raw_value.clone())
    } else if cell.kind == "boolean" {
        Some(match cell.display_value.as_str() {
            "true" => "1".to_string(),
            "false" => "0".to_string(),
            _ => cell.display_value.clone(),
        })
    } else {
        Some(cell.display_value.clone())
    }
}

fn xlsx_cell_exists_in_sheet_xml(sheet_xml: &str, cell_ref: &str) -> CliResult<bool> {
    let (col, row) = parse_cell_ref(cell_ref)?;
    let sheet_data = xlsx_sheet_data_span(sheet_xml)?;
    let rows = parse_xlsx_row_spans(sheet_xml, sheet_data.as_ref())?;
    Ok(rows
        .get(&row)
        .is_some_and(|row_span| row_span.cells.contains_key(&col)))
}

fn xlsx_cell_handle_string(
    sheet: &WorkbookSheet,
    cell_ref: &str,
    sheets: &[WorkbookSheet],
) -> Option<String> {
    if cell_ref.trim().is_empty() {
        return None;
    }
    let count = sheets
        .iter()
        .filter(|candidate| candidate.sheet_id == sheet.sheet_id)
        .count();
    if count != 1 {
        return None;
    }
    Some(format!("H:xlsx/ws:{}/cell:a:{cell_ref}", sheet.sheet_id))
}
