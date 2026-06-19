use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::{
    XlsxMatrixCell, add_xlsx_range_mutation_commands, normalize_xlsx_write_cell,
    set_xlsx_range_in_sheet_xml, validate_xlsx_mutation_output_flags, xlsx_range_destination_json,
};
use crate::{
    CellValue, CliError, CliResult, RangeBounds, WorkbookSheet,
    add_xlsx_formula_recalc_package_updates, copy_zip_with_part_overrides_and_removals,
    is_xlsx_handle, normalize_xl_target, normalize_xlsx_cell_ref, parse_cell_ref,
    parse_xlsx_cell_handle, parse_xlsx_row_spans, relationships, resolve_sheet,
    resolve_sheet_by_sheet_id_unique, shared_strings, sheet_cells, validate, workbook_sheets,
    xlsx_ranges_set_temp_path, xlsx_sheet_data_span, xlsx_styles, zip_text,
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
