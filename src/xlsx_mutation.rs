use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::Path;

use crate::{
    CellValue, CliError, CliResult, RangeBounds, WorkbookSheet, XlsxRangeExportOptions,
    allocate_relationship_id, attr, builtin_num_format_code, check_range_max_cells, col_name,
    command_arg, copy_zip_with_part_overrides, copy_zip_with_part_overrides_and_removals,
    ensure_content_type_override, is_xlsx_handle, local_name, needs_xml_space_preserve,
    normalize_xl_target, normalize_xlsx_cell_ref, normalize_xlsx_ranges_set_data_format,
    parse_cell_ref, parse_cli_range, parse_range, parse_xlsx_cell_handle, relationship_entries,
    relationships, relationships_part_for, render_xml_attrs, replace_xml_span,
    resolve_relationship_target, resolve_sheet, resolve_sheet_by_sheet_id_unique, shared_strings,
    sheet_cells, validate, workbook_sheets, xlsx_range_export_with_options,
    xlsx_ranges_set_temp_path, xlsx_sheet_selectors, xlsx_styles, xml_attr_escape, xml_attrs,
    xml_attrs_map, xml_escape, zip_text,
};
pub(crate) struct XlsxRangesSetOptions<'a> {
    pub(crate) sheet: &'a str,
    pub(crate) range: Option<&'a str>,
    pub(crate) anchor: Option<&'a str>,
    pub(crate) values: Option<&'a str>,
    pub(crate) values_file: Option<&'a str>,
    pub(crate) data_format: Option<&'a str>,
    pub(crate) null_policy: Option<&'a str>,
    pub(crate) ragged: Option<&'a str>,
    pub(crate) max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
    pub(crate) overwrite_formulas: bool,
}

#[derive(Clone)]
struct XlsxMatrixCell {
    kind: String,
    value: String,
    formula: String,
    null: bool,
}

struct XlsxRangeSetMatrix {
    range: Option<String>,
    null_policy: Option<String>,
    major_dimension: String,
    rows: Vec<Vec<XlsxMatrixCell>>,
}

#[derive(Default)]
struct XlsxRangeSetStats {
    updated: usize,
    created: usize,
    cleared: usize,
    skipped: usize,
    formula_count: usize,
    formula_seen: bool,
    formula_invalidated: bool,
}

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

pub(crate) fn xlsx_ranges_set(file: &str, options: XlsxRangesSetOptions<'_>) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let data_format = normalize_xlsx_ranges_set_data_format(options.data_format)?;
    let data = resolve_xlsx_ranges_set_values(options.values, options.values_file)?;
    let mut matrix = parse_xlsx_range_set_matrix(&data, &data_format)?;
    rectangularize_xlsx_matrix(&mut matrix.rows, options.ragged.unwrap_or("reject"))?;
    let null_policy = options
        .null_policy
        .map(ToString::to_string)
        .or_else(|| matrix.null_policy.clone())
        .unwrap_or_else(|| "skip".to_string());
    validate_xlsx_null_policy(&null_policy)?;
    let bounds = resolve_xlsx_ranges_set_bounds(
        options.range,
        options.anchor,
        matrix.range.as_deref(),
        matrix.rows.len(),
        matrix.rows.first().map_or(0, Vec::len),
    )?;
    let range = range_bounds_ref(bounds);
    check_range_max_cells(&range, bounds, options.max_cells)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheet, sheet_part) = resolve_xlsx_sheet_context(file, options.sheet)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (updated_xml, stats) = set_xlsx_range_in_sheet_xml(
        &sheet_xml,
        bounds,
        &matrix.rows,
        &null_policy,
        options.overwrite_formulas,
    )?;

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
    add_xlsx_formula_recalc_package_updates(file, &stats, &mut overrides, &mut removals)?;
    copy_zip_with_part_overrides_and_removals(file, &readback_path, &overrides, &removals)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let destination =
        xlsx_range_destination_json(&readback_path, commit_path, &sheet, &sheet_part, &range)?;
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

    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert(
        "anchor".to_string(),
        json!(format!(
            "{}{}",
            col_name(bounds.start_col),
            bounds.start_row
        )),
    );
    result.insert("range".to_string(), json!(range));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
    result.insert("updated".to_string(), json!(stats.updated));
    result.insert("created".to_string(), json!(stats.created));
    result.insert("cleared".to_string(), json!(stats.cleared));
    result.insert("skipped".to_string(), json!(stats.skipped));
    result.insert("formulaCount".to_string(), json!(stats.formula_count));
    result.insert("dataFormat".to_string(), json!(data_format));
    result.insert("nullPolicy".to_string(), json!(null_policy));
    result.insert("majorDimension".to_string(), json!(matrix.major_dimension));
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
    add_xlsx_formula_recalc_package_updates(file, &stats, &mut overrides, &mut removals)?;
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

pub(crate) struct XlsxRangesSetFormatOptions<'a> {
    pub(crate) sheet: &'a str,
    pub(crate) range: &'a str,
    pub(crate) preset: Option<&'a str>,
    pub(crate) format_code: Option<&'a str>,
    pub(crate) decimals: i64,
    pub(crate) currency_symbol: Option<&'a str>,
    pub(crate) max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
struct XlsxNumberFormatSpec {
    preset: String,
    format_code: String,
    number_format_id: u32,
    builtin: bool,
}

#[derive(Default)]
struct XlsxRangeFormatStats {
    updated: usize,
    created: usize,
    created_styles: usize,
    style_indexes: BTreeSet<u32>,
}

pub(crate) fn xlsx_ranges_set_format(
    file: &str,
    options: XlsxRangesSetFormatOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let bounds = parse_cli_range(options.range)?;
    let range = range_bounds_ref(bounds);
    check_range_max_cells(&range, bounds, options.max_cells)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let spec = resolve_xlsx_number_format(
        options.preset,
        options.format_code,
        options.decimals,
        options.currency_symbol,
    )?;

    let (sheet, sheet_part) = resolve_xlsx_sheet_context(file, options.sheet)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (styles_part, rels_override) = resolve_or_add_xlsx_styles_part(file)?;
    let styles_xml = zip_text(file, &styles_part).unwrap_or_else(|_| default_xlsx_styles_xml());
    let (styles_xml, number_format_id) = ensure_xlsx_number_format(styles_xml, &spec)?;
    let (updated_sheet_xml, styles_xml, stats) =
        set_xlsx_range_number_format_xml(&sheet_xml, styles_xml, bounds, number_format_id)?;
    let content_types_xml = ensure_content_type_override(
        zip_text(file, "[Content_Types].xml")?,
        &format!("/{styles_part}"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
    );

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
    overrides.insert(sheet_part.clone(), updated_sheet_xml);
    overrides.insert(styles_part.clone(), styles_xml);
    overrides.insert("[Content_Types].xml".to_string(), content_types_xml);
    if let Some(rels_xml) = rels_override {
        overrides.insert("xl/_rels/workbook.xml.rels".to_string(), rels_xml);
    }
    copy_zip_with_part_overrides(file, &readback_path, &overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let destination =
        xlsx_range_destination_json(&readback_path, commit_path, &sheet, &sheet_part, &range)?;
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

    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("range".to_string(), json!(range));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
    if !spec.preset.is_empty() {
        result.insert("preset".to_string(), json!(spec.preset));
    }
    result.insert("formatCode".to_string(), json!(spec.format_code));
    result.insert("numberFormatId".to_string(), json!(number_format_id));
    result.insert("builtin".to_string(), json!(spec.builtin));
    result.insert("updated".to_string(), json!(stats.updated));
    result.insert("created".to_string(), json!(stats.created));
    result.insert("createdStyles".to_string(), json!(stats.created_styles));
    if !stats.style_indexes.is_empty() {
        result.insert(
            "styleIndexes".to_string(),
            json!(stats.style_indexes.into_iter().collect::<Vec<_>>()),
        );
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

fn resolve_xlsx_number_format(
    preset: Option<&str>,
    format_code: Option<&str>,
    decimals: i64,
    currency_symbol: Option<&str>,
) -> CliResult<XlsxNumberFormatSpec> {
    let preset = preset.unwrap_or_default().trim().to_ascii_lowercase();
    let format_code = format_code.unwrap_or_default().trim();
    if preset.is_empty() == format_code.is_empty() {
        return Err(CliError::invalid_args(
            "specify exactly one of preset or format code",
        ));
    }
    if !(0..=10).contains(&decimals) {
        return Err(CliError::invalid_args("decimals must be between 0 and 10"));
    }
    if !format_code.is_empty() {
        return Ok(XlsxNumberFormatSpec {
            preset: "custom".to_string(),
            format_code: format_code.to_string(),
            number_format_id: 0,
            builtin: false,
        });
    }
    match preset.as_str() {
        "general" => builtin_xlsx_number_format_spec("general", 0),
        "integer" => builtin_xlsx_number_format_spec("integer", 3),
        "number" => {
            let code = fixed_decimal_format("#,##0", decimals);
            match decimals {
                0 => builtin_xlsx_number_format_spec("number", 3),
                2 => builtin_xlsx_number_format_spec("number", 4),
                _ => custom_xlsx_number_format_spec("number", &code),
            }
        }
        "percent" => {
            let code = format!("{}%", fixed_decimal_format("0", decimals));
            match decimals {
                0 => builtin_xlsx_number_format_spec("percent", 9),
                2 => builtin_xlsx_number_format_spec("percent", 10),
                _ => custom_xlsx_number_format_spec("percent", &code),
            }
        }
        "currency" => {
            let symbol = currency_symbol.unwrap_or("$");
            let code = format!(
                "{}{}",
                xlsx_format_literal(symbol),
                fixed_decimal_format("#,##0", decimals)
            );
            custom_xlsx_number_format_spec("currency", &code)
        }
        "date" => custom_xlsx_number_format_spec("date", "yyyy-mm-dd"),
        "datetime" => custom_xlsx_number_format_spec("datetime", "yyyy-mm-dd h:mm"),
        "text" => builtin_xlsx_number_format_spec("text", 49),
        _ => Err(CliError::invalid_args(format!(
            "invalid preset {:?} (must be integer, number, currency, percent, date, datetime, text, or general)",
            preset
        ))),
    }
}

fn builtin_xlsx_number_format_spec(
    preset: &str,
    number_format_id: u32,
) -> CliResult<XlsxNumberFormatSpec> {
    let code = builtin_num_format_code(number_format_id).ok_or_else(|| {
        CliError::unexpected(format!(
            "unknown built-in number format id {number_format_id}"
        ))
    })?;
    Ok(XlsxNumberFormatSpec {
        preset: preset.to_string(),
        format_code: code.to_string(),
        number_format_id,
        builtin: true,
    })
}

fn custom_xlsx_number_format_spec(preset: &str, code: &str) -> CliResult<XlsxNumberFormatSpec> {
    Ok(XlsxNumberFormatSpec {
        preset: preset.to_string(),
        format_code: code.to_string(),
        number_format_id: 0,
        builtin: false,
    })
}

fn fixed_decimal_format(base: &str, decimals: i64) -> String {
    if decimals == 0 {
        base.to_string()
    } else {
        format!("{base}.{}", "0".repeat(decimals as usize))
    }
}

fn xlsx_format_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn resolve_or_add_xlsx_styles_part(file: &str) -> CliResult<(String, Option<String>)> {
    let rels_part = "xl/_rels/workbook.xml.rels";
    let rels_xml = zip_text(file, rels_part)?;
    let rels = relationship_entries(file, rels_part)?;
    for rel in &rels {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
        {
            return Ok((normalize_xl_target(&rel.target), None));
        }
    }
    let next_id = allocate_relationship_id(&rels);
    let rel = format!(
        r#"<Relationship Id="{next_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#
    );
    let updated = if let Some(pos) = rels_xml.rfind("</Relationships>") {
        let mut out = String::with_capacity(rels_xml.len() + rel.len());
        out.push_str(&rels_xml[..pos]);
        out.push_str(&rel);
        out.push_str(&rels_xml[pos..]);
        out
    } else {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rel}</Relationships>"#
        )
    };
    Ok(("xl/styles.xml".to_string(), Some(updated)))
}

fn add_xlsx_formula_recalc_package_updates(
    file: &str,
    stats: &XlsxRangeSetStats,
    overrides: &mut BTreeMap<String, String>,
    removals: &mut BTreeSet<String>,
) -> CliResult<()> {
    if !stats.formula_seen && !stats.formula_invalidated {
        return Ok(());
    }

    let workbook_part = "xl/workbook.xml";
    overrides.insert(
        workbook_part.to_string(),
        ensure_xlsx_full_calc_on_load(zip_text(file, workbook_part)?),
    );

    if !stats.formula_invalidated {
        return Ok(());
    }

    let content_types_xml = zip_text(file, "[Content_Types].xml")?;
    for part in xlsx_calc_chain_parts_from_content_types(&content_types_xml) {
        removals.insert(part.trim_start_matches('/').to_string());
    }
    overrides.insert(
        "[Content_Types].xml".to_string(),
        remove_xlsx_calc_chain_content_type_overrides(&content_types_xml),
    );

    let rels_part = relationships_part_for(workbook_part);
    if let Ok(rels_xml) = zip_text(file, &rels_part) {
        let (updated_rels, calc_chain_parts) =
            remove_xlsx_calc_chain_relationships(&rels_xml, workbook_part);
        for part in calc_chain_parts {
            removals.insert(part.trim_start_matches('/').to_string());
        }
        if updated_rels != rels_xml {
            overrides.insert(rels_part, updated_rels);
        }
    }

    removals.insert("xl/calcChain.xml".to_string());
    Ok(())
}

fn ensure_xlsx_full_calc_on_load(xml: String) -> String {
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "calcPr" => {
                let end = reader.buffer_position() as usize;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = xml_attrs_map(&e);
                attrs.insert("fullCalcOnLoad".to_string(), "1".to_string());
                attrs.insert("forceFullCalc".to_string(), "1".to_string());
                return replace_xml_span(
                    &xml,
                    start,
                    end,
                    &format!("<{name}{}/>", render_xml_attrs(&attrs)),
                );
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "calcPr" => {
                let end = reader.buffer_position() as usize;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = xml_attrs_map(&e);
                attrs.insert("fullCalcOnLoad".to_string(), "1".to_string());
                attrs.insert("forceFullCalc".to_string(), "1".to_string());
                return replace_xml_span(
                    &xml,
                    start,
                    end,
                    &format!("<{name}{}>", render_xml_attrs(&attrs)),
                );
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    let calc_pr = r#"<calcPr fullCalcOnLoad="1" forceFullCalc="1"/>"#;
    if let Some(pos) = xml.rfind("</workbook>") {
        let mut out = String::with_capacity(xml.len() + calc_pr.len());
        out.push_str(&xml[..pos]);
        out.push_str(calc_pr);
        out.push_str(&xml[pos..]);
        out
    } else {
        xml
    }
}

fn xlsx_calc_chain_parts_from_content_types(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut parts = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Override" =>
            {
                let attrs = xml_attrs_map(&e);
                if attrs.get("ContentType").is_some_and(|value| {
                    value
                        == "application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml"
                }) && let Some(part_name) = attrs.get("PartName")
                {
                    parts.push(part_name.trim_start_matches('/').to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    parts
}

fn remove_xlsx_calc_chain_content_type_overrides(xml: &str) -> String {
    remove_xml_elements_matching(xml, "Override", |attrs| {
        attrs.get("ContentType").is_some_and(|value| {
            value == "application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml"
        })
    })
}

fn remove_xlsx_calc_chain_relationships(xml: &str, workbook_part: &str) -> (String, Vec<String>) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut parts = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Relationship" =>
            {
                let attrs = xml_attrs_map(&e);
                if attrs.get("Type").is_some_and(|value| {
                    value == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain"
                }) && let Some(target) = attrs.get("Target")
                {
                    parts.push(
                        resolve_relationship_target(workbook_part, target)
                            .trim_start_matches('/')
                            .to_string(),
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    let updated = remove_xml_elements_matching(xml, "Relationship", |attrs| {
        attrs.get("Type").is_some_and(|value| {
            value == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain"
        })
    });
    (updated, parts)
}

fn remove_xml_elements_matching<F>(xml: &str, element_local: &str, predicate: F) -> String
where
    F: Fn(&BTreeMap<String, String>) -> bool,
{
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut spans = Vec::<(usize, usize)>::new();
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == element_local => {
                if predicate(&xml_attrs_map(&e)) {
                    spans.push((start, reader.buffer_position() as usize));
                }
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == element_local => {
                if predicate(&xml_attrs_map(&e)) {
                    let mut depth = 1usize;
                    loop {
                        match reader.read_event() {
                            Ok(Event::Start(inner))
                                if local_name(inner.name().as_ref()) == element_local =>
                            {
                                depth += 1;
                            }
                            Ok(Event::End(inner))
                                if local_name(inner.name().as_ref()) == element_local =>
                            {
                                depth -= 1;
                                if depth == 0 {
                                    spans.push((start, reader.buffer_position() as usize));
                                    break;
                                }
                            }
                            Ok(Event::Eof) | Err(_) => {
                                spans.push((start, reader.buffer_position() as usize));
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    if spans.is_empty() {
        return xml.to_string();
    }
    let mut out = String::with_capacity(xml.len());
    let mut cursor = 0usize;
    for (start, end) in spans {
        if start > cursor {
            out.push_str(&xml[cursor..start]);
        }
        cursor = end;
    }
    out.push_str(&xml[cursor..]);
    out
}

fn default_xlsx_styles_xml() -> String {
    r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font/></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#.to_string()
}

fn ensure_xlsx_number_format(
    styles_xml: String,
    spec: &XlsxNumberFormatSpec,
) -> CliResult<(String, u32)> {
    let styles_xml = ensure_xlsx_style_defaults(styles_xml);
    if spec.builtin {
        return Ok((styles_xml, spec.number_format_id));
    }
    for (id, code) in parse_xlsx_num_formats(&styles_xml) {
        if code == spec.format_code {
            return Ok((styles_xml, id));
        }
    }
    let mut next_id = 164u32;
    for (id, _) in parse_xlsx_num_formats(&styles_xml) {
        if id >= next_id {
            next_id = id + 1;
        }
    }
    let num_fmt = format!(
        r#"<numFmt numFmtId="{next_id}" formatCode="{}"/>"#,
        xml_attr_escape(&spec.format_code)
    );
    let updated = if let Some(span) = element_span_by_local_name(&styles_xml, "numFmts") {
        let mut out = String::with_capacity(styles_xml.len() + num_fmt.len());
        out.push_str(&styles_xml[..span.close_start]);
        out.push_str(&num_fmt);
        out.push_str(&styles_xml[span.close_start..]);
        set_collection_count(out, "numFmts", "numFmt")
    } else {
        insert_xlsx_styles_collection(
            &styles_xml,
            "numFmts",
            &format!(r#"<numFmts count="1">{num_fmt}</numFmts>"#),
        )
    };
    Ok((updated, next_id))
}

fn ensure_xlsx_style_defaults(mut styles_xml: String) -> String {
    if !styles_xml.contains("<styleSheet") {
        return default_xlsx_styles_xml();
    }
    let defaults = [
        ("fonts", r#"<fonts count="1"><font/></fonts>"#),
        (
            "fills",
            r#"<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>"#,
        ),
        ("borders", r#"<borders count="1"><border/></borders>"#),
        (
            "cellStyleXfs",
            r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
        ),
        (
            "cellXfs",
            r#"<cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs>"#,
        ),
        (
            "cellStyles",
            r#"<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>"#,
        ),
    ];
    for (name, block) in defaults {
        if element_span_by_local_name(&styles_xml, name).is_none() {
            styles_xml = insert_xlsx_styles_collection(&styles_xml, name, block);
        }
    }
    styles_xml
}

fn insert_xlsx_styles_collection(styles_xml: &str, name: &str, block: &str) -> String {
    let target_order = xlsx_styles_collection_order(name);
    for candidate in [
        "numFmts",
        "fonts",
        "fills",
        "borders",
        "cellStyleXfs",
        "cellXfs",
        "cellStyles",
        "dxfs",
        "tableStyles",
        "colors",
        "extLst",
    ] {
        if xlsx_styles_collection_order(candidate) > target_order
            && let Some(span) = element_span_by_local_name(styles_xml, candidate)
        {
            let mut out = String::with_capacity(styles_xml.len() + block.len());
            out.push_str(&styles_xml[..span.start]);
            out.push_str(block);
            out.push_str(&styles_xml[span.start..]);
            return out;
        }
    }
    if let Some(pos) = styles_xml.rfind("</styleSheet>") {
        let mut out = String::with_capacity(styles_xml.len() + block.len());
        out.push_str(&styles_xml[..pos]);
        out.push_str(block);
        out.push_str(&styles_xml[pos..]);
        out
    } else {
        styles_xml.to_string()
    }
}

fn xlsx_styles_collection_order(name: &str) -> u32 {
    match name {
        "numFmts" => 10,
        "fonts" => 20,
        "fills" => 30,
        "borders" => 40,
        "cellStyleXfs" => 50,
        "cellXfs" => 60,
        "cellStyles" => 70,
        "dxfs" => 80,
        "tableStyles" => 90,
        "colors" => 100,
        "extLst" => 110,
        _ => 1000,
    }
}

#[derive(Clone, Copy)]
struct XmlElementSpan {
    start: usize,
    open_end: usize,
    close_start: usize,
}

fn element_span_by_local_name(xml: &str, wanted: &str) -> Option<XmlElementSpan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == wanted => {
                let open_end = reader.buffer_position() as usize;
                let mut depth = 1usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::Start(e)) if local_name(e.name().as_ref()) == wanted => {
                            depth += 1;
                        }
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == wanted => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(XmlElementSpan {
                                    start: before,
                                    open_end,
                                    close_start: inner_before,
                                });
                            }
                        }
                        Ok(Event::Eof) | Err(_) => return None,
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == wanted => {
                let end = reader.buffer_position() as usize;
                return Some(XmlElementSpan {
                    start: before,
                    open_end: end,
                    close_start: before,
                });
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn parse_xlsx_num_formats(styles_xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(styles_xml);
    reader.config_mut().trim_text(false);
    let mut formats = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "numFmt" =>
            {
                if let (Some(id), Some(code)) = (attr(&e, "numFmtId"), attr(&e, "formatCode"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    formats.push((id, code));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    formats
}

#[derive(Clone)]
struct XlsxXfEntry {
    attrs: BTreeMap<String, String>,
    inner_xml: String,
}

fn parse_xlsx_cell_xfs(styles_xml: &str) -> CliResult<Vec<XlsxXfEntry>> {
    let Some(parent) = element_span_by_local_name(styles_xml, "cellXfs") else {
        return Ok(Vec::new());
    };
    let fragment = &styles_xml[parent.open_end..parent.close_start];
    let base = parent.open_end;
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut entries = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "xf" => {
                let attrs = xml_attrs(&e);
                let open_end = reader.buffer_position() as usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "xf" => {
                            entries.push(XlsxXfEntry {
                                attrs,
                                inner_xml: styles_xml[base + open_end..base + inner_before]
                                    .to_string(),
                            });
                            break;
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("xf has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "xf" => {
                let _ = before;
                entries.push(XlsxXfEntry {
                    attrs: xml_attrs(&e),
                    inner_xml: String::new(),
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(entries)
}

fn ensure_xlsx_cell_style(
    styles_xml: String,
    base_style_index: u32,
    number_format_id: u32,
) -> CliResult<(String, u32, bool)> {
    let styles_xml = ensure_xlsx_style_defaults(styles_xml);
    let xfs = parse_xlsx_cell_xfs(&styles_xml)?;
    let base_index = if (base_style_index as usize) < xfs.len() {
        base_style_index
    } else {
        0
    };
    let base = xfs
        .get(base_index as usize)
        .cloned()
        .unwrap_or_else(default_xlsx_xf_entry);
    if xlsx_xf_num_fmt_id(&base.attrs) == number_format_id {
        return Ok((styles_xml, base_index, false));
    }
    let mut attrs = base.attrs.clone();
    for (key, value) in [
        ("fontId", "0"),
        ("fillId", "0"),
        ("borderId", "0"),
        ("xfId", "0"),
    ] {
        attrs
            .entry(key.to_string())
            .or_insert_with(|| value.to_string());
    }
    attrs.insert("numFmtId".to_string(), number_format_id.to_string());
    attrs.insert("applyNumberFormat".to_string(), "1".to_string());
    let candidate = XlsxXfEntry {
        attrs,
        inner_xml: base.inner_xml,
    };
    let candidate_sig = render_xlsx_xf(&candidate);
    for (index, xf) in xfs.iter().enumerate() {
        if render_xlsx_xf(xf) == candidate_sig {
            return Ok((styles_xml, index as u32, false));
        }
    }
    let Some(parent) = element_span_by_local_name(&styles_xml, "cellXfs") else {
        return Err(CliError::unexpected("styles cellXfs not found"));
    };
    let mut out = String::with_capacity(styles_xml.len() + candidate_sig.len());
    out.push_str(&styles_xml[..parent.close_start]);
    out.push_str(&candidate_sig);
    out.push_str(&styles_xml[parent.close_start..]);
    let out = set_collection_count(out, "cellXfs", "xf");
    Ok((out, xfs.len() as u32, true))
}

fn default_xlsx_xf_entry() -> XlsxXfEntry {
    let mut attrs = BTreeMap::new();
    attrs.insert("numFmtId".to_string(), "0".to_string());
    attrs.insert("fontId".to_string(), "0".to_string());
    attrs.insert("fillId".to_string(), "0".to_string());
    attrs.insert("borderId".to_string(), "0".to_string());
    attrs.insert("xfId".to_string(), "0".to_string());
    XlsxXfEntry {
        attrs,
        inner_xml: String::new(),
    }
}

fn render_xlsx_xf(xf: &XlsxXfEntry) -> String {
    if xf.inner_xml.is_empty() {
        format!("<xf{}/>", render_xml_attrs(&xf.attrs))
    } else {
        format!("<xf{}>{}</xf>", render_xml_attrs(&xf.attrs), xf.inner_xml)
    }
}

fn xlsx_xf_num_fmt_id(attrs: &BTreeMap<String, String>) -> u32 {
    attrs
        .get("numFmtId")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0)
}

fn set_collection_count(xml: String, parent: &str, child: &str) -> String {
    let count = count_children_in_parent(&xml, parent, child);
    let Some(span) = element_span_by_local_name(&xml, parent) else {
        return xml;
    };
    set_start_tag_count_attr(&xml, span, count)
}

fn count_children_in_parent(xml: &str, parent: &str, child: &str) -> usize {
    let Some(span) = element_span_by_local_name(xml, parent) else {
        return 0;
    };
    let fragment = &xml[span.open_end..span.close_start];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut count = 0usize;
    let mut depth = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if depth == 0 && local_name(e.name().as_ref()) == child {
                    count += 1;
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if depth == 0 && local_name(e.name().as_ref()) == child {
                    count += 1;
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    count
}

fn set_start_tag_count_attr(xml: &str, span: XmlElementSpan, count: usize) -> String {
    let open = &xml[span.start..span.open_end];
    let replacement = if let Some(pos) = open.find("count=\"") {
        let value_start = pos + "count=\"".len();
        if let Some(value_end_rel) = open[value_start..].find('"') {
            let value_end = value_start + value_end_rel;
            let mut tag = String::new();
            tag.push_str(&open[..value_start]);
            tag.push_str(&count.to_string());
            tag.push_str(&open[value_end..]);
            tag
        } else {
            open.to_string()
        }
    } else if let Some(pos) = open.rfind("/>") {
        format!("{} count=\"{}\"/>", &open[..pos].trim_end(), count)
    } else if let Some(pos) = open.rfind('>') {
        format!("{} count=\"{}\">", &open[..pos].trim_end(), count)
    } else {
        open.to_string()
    };
    let mut out = String::with_capacity(xml.len() + replacement.len());
    out.push_str(&xml[..span.start]);
    out.push_str(&replacement);
    out.push_str(&xml[span.open_end..]);
    out
}

fn set_xlsx_range_number_format_xml(
    sheet_xml: &str,
    mut styles_xml: String,
    bounds: RangeBounds,
    number_format_id: u32,
) -> CliResult<(String, String, XlsxRangeFormatStats)> {
    let sheet_data = xlsx_sheet_data_span(sheet_xml)?;
    let row_spans = parse_xlsx_row_spans(sheet_xml, sheet_data.as_ref())?;
    let mut stats = XlsxRangeFormatStats::default();
    let mut changed_rows = BTreeMap::<u32, String>::new();
    let mut style_by_base = BTreeMap::<u32, u32>::new();
    let write_bounds = bounds.normalized();
    for row_num in write_bounds.start_row..=write_bounds.end_row {
        let existing_row = row_spans.get(&row_num);
        let mut rendered_cells = existing_row
            .map(|span| {
                span.cells
                    .iter()
                    .map(|(col, cell)| (*col, cell.xml.clone()))
                    .collect::<BTreeMap<u32, String>>()
            })
            .unwrap_or_default();
        let mut row_changed = false;
        for col_num in write_bounds.start_col..=write_bounds.end_col {
            let addr = format!("{}{}", col_name(col_num), row_num);
            let existing_cell = existing_row.and_then(|span| span.cells.get(&col_num));
            let base_style = existing_cell
                .and_then(|cell| cell.attrs.get("s"))
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0);
            let style_index = if let Some(style_index) = style_by_base.get(&base_style).copied() {
                style_index
            } else {
                let (new_styles_xml, style_index, created) =
                    ensure_xlsx_cell_style(styles_xml, base_style, number_format_id)?;
                styles_xml = new_styles_xml;
                if created {
                    stats.created_styles += 1;
                }
                style_by_base.insert(base_style, style_index);
                style_index
            };
            let cell_xml = if let Some(existing_cell) = existing_cell {
                render_xlsx_existing_cell_with_style(&addr, existing_cell, style_index)
            } else {
                let mut attrs = BTreeMap::new();
                attrs.insert("r".to_string(), addr.clone());
                attrs.insert("s".to_string(), style_index.to_string());
                stats.created += 1;
                render_empty_xlsx_cell_with_attrs(&addr, Some(&attrs))
            };
            rendered_cells.insert(col_num, cell_xml);
            stats.updated += 1;
            stats.style_indexes.insert(style_index);
            row_changed = true;
        }
        if row_changed {
            changed_rows.insert(
                row_num,
                render_xlsx_row(row_num, existing_row, rendered_cells),
            );
        }
    }
    let updated =
        rebuild_xlsx_sheet_data(sheet_xml, sheet_data.as_ref(), &row_spans, &changed_rows)?;
    let used_range = xlsx_used_range_from_cell_refs(&updated);
    Ok((
        replace_xlsx_dimension(&updated, used_range.as_deref()),
        styles_xml,
        stats,
    ))
}

fn render_xlsx_existing_cell_with_style(
    addr: &str,
    cell: &XlsxCellSpan,
    style_index: u32,
) -> String {
    let mut attrs = cell.attrs.clone();
    attrs.insert("r".to_string(), addr.to_string());
    attrs.insert("s".to_string(), style_index.to_string());
    if cell.xml.trim_end().ends_with("/>") {
        return render_empty_xlsx_cell_with_attrs(addr, Some(&attrs));
    }
    if let Some(open_end) = cell.xml.find('>') {
        let mut out = format!("<c{}>", render_xml_attrs(&attrs));
        out.push_str(&cell.xml[open_end + 1..]);
        out
    } else {
        render_empty_xlsx_cell_with_attrs(addr, Some(&attrs))
    }
}

fn resolve_xlsx_ranges_set_values(
    values: Option<&str>,
    values_file: Option<&str>,
) -> CliResult<String> {
    match (values, values_file) {
        (Some(_), Some(_)) | (None, None) => Err(CliError::invalid_args(
            "must specify exactly one of --values or --values-file",
        )),
        (Some(values), None) => Ok(values.to_string()),
        (None, Some("-")) => {
            let mut data = String::new();
            std::io::stdin()
                .read_to_string(&mut data)
                .map_err(|err| CliError::unexpected(format!("failed to read stdin: {err}")))?;
            Ok(data)
        }
        (None, Some(path)) => fs::read_to_string(path)
            .map_err(|_| CliError::file_not_found(format!("file not found: {path}"))),
    }
}

fn parse_xlsx_range_set_matrix(data: &str, data_format: &str) -> CliResult<XlsxRangeSetMatrix> {
    match data_format {
        "json" => parse_xlsx_range_set_json_matrix(data),
        "csv" => parse_xlsx_delimited_matrix(data, ','),
        "tsv" => parse_xlsx_delimited_matrix(data, '\t'),
        _ => Err(CliError::invalid_args(format!(
            "invalid data format {data_format:?} (must be json, csv, or tsv)",
        ))),
    }
}

fn parse_xlsx_range_set_json_matrix(data: &str) -> CliResult<XlsxRangeSetMatrix> {
    let raw: Value = serde_json::from_str(data)
        .map_err(|err| CliError::invalid_args(format!("invalid json values: {err}")))?;
    let (range, null_policy, major_dimension, values) = if let Some(object) = raw.as_object() {
        if object.contains_key("values") {
            (
                object
                    .get("range")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                object
                    .get("nullPolicy")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                object
                    .get("majorDimension")
                    .and_then(Value::as_str)
                    .unwrap_or("rows")
                    .to_string(),
                object
                    .get("values")
                    .cloned()
                    .ok_or_else(|| CliError::invalid_args("JSON object must contain values"))?,
            )
        } else {
            (None, None, "rows".to_string(), raw)
        }
    } else {
        (None, None, "rows".to_string(), raw)
    };
    let mut rows = parse_xlsx_matrix_rows(&values)?;
    let major_dimension = match major_dimension.trim().to_ascii_lowercase().as_str() {
        "" | "rows" => "rows".to_string(),
        "columns" => {
            rows = transpose_xlsx_matrix(rows)?;
            "columns".to_string()
        }
        _ => {
            return Err(CliError::invalid_args(
                "majorDimension must be rows or columns",
            ));
        }
    };
    Ok(XlsxRangeSetMatrix {
        range,
        null_policy,
        major_dimension,
        rows,
    })
}

fn parse_xlsx_delimited_matrix(data: &str, delimiter: char) -> CliResult<XlsxRangeSetMatrix> {
    let records = parse_delimited_records(data, delimiter)?;
    let rows = records
        .into_iter()
        .map(|record| {
            record
                .into_iter()
                .map(|value| XlsxMatrixCell {
                    kind: "string".to_string(),
                    value,
                    formula: String::new(),
                    null: false,
                })
                .collect()
        })
        .collect();
    Ok(XlsxRangeSetMatrix {
        range: None,
        null_policy: None,
        major_dimension: "rows".to_string(),
        rows,
    })
}

fn parse_delimited_records(data: &str, delimiter: char) -> CliResult<Vec<Vec<String>>> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut chars = data.chars().peekable();
    let mut in_quotes = false;
    let mut field_started = false;
    let mut just_closed_quote = false;

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                    just_closed_quote = true;
                }
            } else {
                field.push(ch);
            }
            continue;
        }

        if ch == '"' {
            if !field_started {
                in_quotes = true;
                field_started = true;
                continue;
            }
            return Err(CliError::invalid_args(
                "parse error on line 1, column 1: bare \" in non-quoted-field",
            ));
        }

        if ch == delimiter {
            record.push(std::mem::take(&mut field));
            field_started = false;
            just_closed_quote = false;
            continue;
        }

        if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            record.push(std::mem::take(&mut field));
            records.push(std::mem::take(&mut record));
            field_started = false;
            just_closed_quote = false;
            continue;
        }

        if just_closed_quote {
            return Err(CliError::invalid_args(
                "parse error on line 1, column 1: extraneous or missing \" in quoted-field",
            ));
        }
        field_started = true;
        field.push(ch);
    }

    if in_quotes {
        return Err(CliError::invalid_args(
            "parse error on line 1, column 1: extraneous or missing \" in quoted-field",
        ));
    }
    if field_started || !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    Ok(records)
}

fn parse_xlsx_matrix_rows(value: &Value) -> CliResult<Vec<Vec<XlsxMatrixCell>>> {
    let rows = value
        .as_array()
        .ok_or_else(|| CliError::invalid_args("values must be an array of arrays"))?;
    rows.iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let cells = row.as_array().ok_or_else(|| {
                CliError::invalid_args(format!("values[{row_idx}] must be an array"))
            })?;
            cells
                .iter()
                .enumerate()
                .map(|(col_idx, cell)| {
                    parse_xlsx_matrix_cell(cell).map_err(|err| {
                        CliError::invalid_args(format!(
                            "values[{row_idx}][{col_idx}]: {}",
                            err.message
                        ))
                    })
                })
                .collect()
        })
        .collect()
}

fn parse_xlsx_matrix_cell(value: &Value) -> CliResult<XlsxMatrixCell> {
    if value.is_null() {
        return Ok(XlsxMatrixCell {
            kind: "empty".to_string(),
            value: String::new(),
            formula: String::new(),
            null: true,
        });
    }
    if let Some(text) = value.as_str() {
        return Ok(XlsxMatrixCell {
            kind: "string".to_string(),
            value: text.to_string(),
            formula: String::new(),
            null: false,
        });
    }
    if let Some(number) = value.as_number() {
        return Ok(XlsxMatrixCell {
            kind: "number".to_string(),
            value: number.to_string(),
            formula: String::new(),
            null: false,
        });
    }
    if let Some(boolean) = value.as_bool() {
        return Ok(XlsxMatrixCell {
            kind: "boolean".to_string(),
            value: boolean.to_string(),
            formula: String::new(),
            null: false,
        });
    }
    let object = value
        .as_object()
        .ok_or_else(|| CliError::invalid_args("unsupported JSON cell type"))?;
    if let Some(formula) = object.get("formula") {
        let formula = formula
            .as_str()
            .ok_or_else(|| CliError::invalid_args("formula must be a string"))?;
        if formula.trim().is_empty() {
            return Err(CliError::invalid_args("formula cannot be empty"));
        }
        return Ok(XlsxMatrixCell {
            kind: "formula".to_string(),
            value: formula.to_string(),
            formula: formula.to_string(),
            null: false,
        });
    }
    let raw_value = object
        .get("value")
        .ok_or_else(|| CliError::invalid_args("object cell must contain value or formula"))?;
    let mut cell = parse_xlsx_matrix_cell(raw_value)?;
    if let Some(kind) = object.get("type").and_then(Value::as_str) {
        cell.kind = kind.trim().to_ascii_lowercase();
        if cell.kind == "formula" {
            cell.formula = cell.value.clone();
        }
    }
    Ok(cell)
}

fn transpose_xlsx_matrix(rows: Vec<Vec<XlsxMatrixCell>>) -> CliResult<Vec<Vec<XlsxMatrixCell>>> {
    if rows.is_empty() {
        return Ok(rows);
    }
    let cols = rows[0].len();
    if rows.iter().any(|row| row.len() != cols) {
        return Err(CliError::invalid_args(
            "ragged columns matrix cannot be transposed",
        ));
    }
    let mut out = vec![Vec::with_capacity(rows.len()); cols];
    for row in rows {
        for (col_idx, cell) in row.into_iter().enumerate() {
            out[col_idx].push(cell);
        }
    }
    Ok(out)
}

fn rectangularize_xlsx_matrix(rows: &mut Vec<Vec<XlsxMatrixCell>>, ragged: &str) -> CliResult<()> {
    if rows.is_empty() {
        return Err(CliError::invalid_args("values matrix cannot be empty"));
    }
    let cols = rows[0].len();
    let max_cols = rows.iter().map(Vec::len).max().unwrap_or(cols);
    if max_cols == 0 {
        return Err(CliError::invalid_args(
            "values matrix must contain at least one column",
        ));
    }
    match ragged.trim().to_ascii_lowercase().as_str() {
        "" | "reject" => {
            for (idx, row) in rows.iter().enumerate().skip(1) {
                if row.len() != cols {
                    return Err(CliError::invalid_args(format!(
                        "ragged matrix row {} has {} columns, want {}",
                        idx + 1,
                        row.len(),
                        cols
                    )));
                }
            }
        }
        "fill-empty" => {
            for row in rows {
                while row.len() < max_cols {
                    row.push(XlsxMatrixCell {
                        kind: "string".to_string(),
                        value: String::new(),
                        formula: String::new(),
                        null: false,
                    });
                }
            }
        }
        _ => {
            return Err(CliError::invalid_args(
                "invalid ragged mode (must be reject or fill-empty)",
            ));
        }
    }
    Ok(())
}

fn validate_xlsx_null_policy(policy: &str) -> CliResult<()> {
    match policy.trim().to_ascii_lowercase().as_str() {
        "skip" | "clear" | "empty-string" => Ok(()),
        _ => Err(CliError::invalid_args(
            "invalid null policy (must be skip, clear, or empty-string)",
        )),
    }
}

fn resolve_xlsx_ranges_set_bounds(
    range: Option<&str>,
    anchor: Option<&str>,
    input_range: Option<&str>,
    rows: usize,
    cols: usize,
) -> CliResult<RangeBounds> {
    let mut sources = 0;
    if range.is_some_and(|value| !value.trim().is_empty()) {
        sources += 1;
    }
    if anchor.is_some_and(|value| !value.trim().is_empty()) {
        sources += 1;
    }
    if input_range.is_some_and(|value| !value.trim().is_empty()) {
        sources += 1;
    }
    if sources != 1 {
        return Err(CliError::invalid_args(
            "must specify exactly one of --anchor, --range, or JSON input range",
        ));
    }
    if let Some(anchor) = anchor.filter(|value| !value.trim().is_empty()) {
        let (start_col, start_row) = parse_cell_ref(anchor)
            .map_err(|err| CliError::invalid_args(format!("invalid --anchor: {}", err.message)))?;
        let end_col = start_col + cols as u32 - 1;
        let end_row = start_row + rows as u32 - 1;
        return Ok(RangeBounds {
            start_col,
            start_row,
            end_col,
            end_row,
        });
    }
    let range_text = input_range
        .filter(|value| !value.trim().is_empty())
        .or(range)
        .unwrap_or_default();
    let bounds = parse_cli_range(range_text)?;
    let range_rows = bounds.row_count();
    let range_cols = bounds.col_count();
    if range_rows as usize != rows || range_cols as usize != cols {
        return Err(CliError::invalid_args(format!(
            "range {} is {}x{} but values matrix is {}x{}",
            range_text, range_rows, range_cols, rows, cols
        )));
    }
    Ok(bounds)
}

pub(crate) fn validate_xlsx_mutation_output_flags(
    out: Option<&str>,
    in_place: bool,
    backup: Option<&str>,
    dry_run: bool,
) -> CliResult<()> {
    let has_out = out.is_some_and(|value| !value.trim().is_empty());
    let has_backup = backup.is_some_and(|value| !value.trim().is_empty());
    if dry_run && (has_out || in_place) {
        return Err(CliError::invalid_args(
            "--dry-run cannot be combined with --out or --in-place",
        ));
    }
    if dry_run && has_backup {
        return Err(CliError::invalid_args(
            "--backup cannot be used with --dry-run",
        ));
    }
    if !dry_run && !has_out && !in_place {
        return Err(CliError::invalid_args(
            "must specify exactly one of --out, --in-place, or --dry-run",
        ));
    }
    if has_out && in_place {
        return Err(CliError::invalid_args(
            "cannot specify both --out and --in-place",
        ));
    }
    if has_backup && !in_place {
        return Err(CliError::invalid_args(
            "--backup can only be used with --in-place",
        ));
    }
    Ok(())
}

fn resolve_xlsx_sheet_context(
    file: &str,
    sheet_selector: &str,
) -> CliResult<(WorkbookSheet, String)> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    Ok((sheet, sheet_part))
}

fn set_xlsx_range_in_sheet_xml(
    xml: &str,
    bounds: RangeBounds,
    rows: &[Vec<XlsxMatrixCell>],
    null_policy: &str,
    overwrite_formulas: bool,
) -> CliResult<(String, XlsxRangeSetStats)> {
    reject_xlsx_merged_cell_intersection(xml, bounds)?;
    let sheet_data = xlsx_sheet_data_span(xml)?;
    let row_spans = parse_xlsx_row_spans(xml, sheet_data.as_ref())?;

    let mut stats = XlsxRangeSetStats::default();
    let mut changed_rows = BTreeMap::<u32, String>::new();
    let write_bounds = bounds.normalized();
    for (row_offset, row) in rows.iter().enumerate() {
        let row_number = write_bounds.start_row + row_offset as u32;
        let existing_row = row_spans.get(&row_number);
        let mut rendered_cells = existing_row
            .map(|span| {
                span.cells
                    .iter()
                    .map(|(col, cell)| (*col, cell.xml.clone()))
                    .collect::<BTreeMap<u32, String>>()
            })
            .unwrap_or_default();
        let mut row_changed = false;
        for (col_offset, cell) in row.iter().enumerate() {
            let col_number = write_bounds.start_col + col_offset as u32;
            let addr = format!("{}{}", col_name(col_number), row_number);
            let existing_cell = existing_row.and_then(|span| span.cells.get(&col_number));
            if !overwrite_formulas
                && existing_cell.is_some_and(|span| span.has_formula)
                && xlsx_range_cell_touches_existing(cell, null_policy)
            {
                return Err(CliError::invalid_args(format!(
                    "range write would overwrite existing formula: {addr}"
                )));
            }
            if cell.null {
                match null_policy.trim().to_ascii_lowercase().as_str() {
                    "skip" => {
                        stats.skipped += 1;
                    }
                    "clear" => {
                        if let Some(existing_cell) = existing_cell {
                            stats.cleared += 1;
                            row_changed = true;
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
                                    render_empty_xlsx_cell_with_attrs(
                                        &addr,
                                        Some(&existing_cell.attrs),
                                    ),
                                );
                            } else {
                                rendered_cells.remove(&col_number);
                            }
                        } else {
                            rendered_cells.remove(&col_number);
                        }
                    }
                    "empty-string" => {
                        let empty = XlsxMatrixCell {
                            kind: "string".to_string(),
                            value: String::new(),
                            formula: String::new(),
                            null: false,
                        };
                        let (rendered, wrote_formula) = render_xlsx_cell_with_attrs(
                            &addr,
                            &empty,
                            existing_cell.map(|span| &span.attrs),
                        )?;
                        rendered_cells.insert(col_number, rendered);
                        row_changed = true;
                        stats.updated += 1;
                        if existing_cell.is_none() {
                            stats.created += 1;
                        }
                        if wrote_formula {
                            stats.formula_count += 1;
                            stats.formula_seen = true;
                        }
                    }
                    _ => unreachable!("null policy validated earlier"),
                }
                continue;
            }
            let (rendered, wrote_formula) =
                render_xlsx_cell_with_attrs(&addr, cell, existing_cell.map(|span| &span.attrs))?;
            rendered_cells.insert(col_number, rendered);
            row_changed = true;
            if existing_cell.is_some_and(|span| span.has_formula) {
                stats.formula_invalidated = true;
            }
            if existing_cell.is_none() {
                stats.created += 1;
            }
            if wrote_formula {
                stats.formula_count += 1;
                stats.formula_seen = true;
            }
            stats.updated += 1;
        }
        if row_changed {
            changed_rows.insert(
                row_number,
                render_xlsx_row(row_number, existing_row, rendered_cells),
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

fn xlsx_range_cell_touches_existing(cell: &XlsxMatrixCell, null_policy: &str) -> bool {
    !(cell.null && null_policy.trim().eq_ignore_ascii_case("skip"))
}

fn render_xlsx_cell_with_attrs(
    addr: &str,
    cell: &XlsxMatrixCell,
    attrs: Option<&BTreeMap<String, String>>,
) -> CliResult<(String, bool)> {
    let mut attrs = attrs.cloned().unwrap_or_default();
    attrs.insert("r".to_string(), addr.to_string());
    attrs.remove("t");
    let (kind, value) = normalize_xlsx_write_cell(cell)?;
    let (content, wrote_formula) = match kind.as_str() {
        "string" => {
            attrs.insert("t".to_string(), "inlineStr".to_string());
            let space_attr = if needs_xml_space_preserve(&value) {
                " xml:space=\"preserve\""
            } else {
                ""
            };
            (
                format!("<is><t{space_attr}>{}</t></is>", xml_escape(&value)),
                false,
            )
        }
        "number" => (format!("<v>{}</v>", xml_escape(&value)), false),
        "bool" | "boolean" => {
            let value = match cell.value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => "1",
                _ => "0",
            };
            attrs.insert("t".to_string(), "b".to_string());
            (format!("<v>{value}</v>"), false)
        }
        "formula" => (format!("<f>{}</f>", xml_escape(&value)), true),
        _ => unreachable!("cell kind normalized earlier"),
    };
    Ok((
        format!("<c{}>{content}</c>", render_xml_attrs(&attrs)),
        wrote_formula,
    ))
}

fn normalize_xlsx_write_cell(cell: &XlsxMatrixCell) -> CliResult<(String, String)> {
    let kind = if !cell.formula.is_empty() {
        "formula".to_string()
    } else {
        cell.kind.trim().to_ascii_lowercase()
    };
    match kind.as_str() {
        "" | "string" => Ok(("string".to_string(), cell.value.clone())),
        "number" => {
            let literal = cell.value.trim();
            let parsed = literal.parse::<f64>().map_err(|_| {
                CliError::invalid_args(format!("invalid number value {:?}", cell.value))
            })?;
            if !parsed.is_finite() || literal.is_empty() {
                return Err(CliError::invalid_args(format!(
                    "invalid number value {:?}",
                    cell.value
                )));
            }
            Ok(("number".to_string(), literal.to_string()))
        }
        "bool" | "boolean" => match cell.value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(("bool".to_string(), "1".to_string())),
            "false" | "0" => Ok(("bool".to_string(), "0".to_string())),
            _ => Err(CliError::invalid_args(format!(
                "invalid bool value {:?}",
                cell.value
            ))),
        },
        "formula" => {
            let formula = if cell.formula.is_empty() {
                &cell.value
            } else {
                &cell.formula
            };
            let formula = formula.trim().trim_start_matches('=').to_string();
            if formula.is_empty() {
                return Err(CliError::invalid_args("formula value cannot be empty"));
            }
            Ok(("formula".to_string(), formula))
        }
        "auto" => {
            let trimmed = cell.value.trim();
            if trimmed.starts_with('=') {
                return normalize_xlsx_write_cell(&XlsxMatrixCell {
                    kind: "formula".to_string(),
                    value: trimmed.to_string(),
                    formula: trimmed.to_string(),
                    null: false,
                });
            }
            if matches!(trimmed.to_ascii_lowercase().as_str(), "true" | "false") {
                return normalize_xlsx_write_cell(&XlsxMatrixCell {
                    kind: "bool".to_string(),
                    value: trimmed.to_string(),
                    formula: String::new(),
                    null: false,
                });
            }
            if let Ok(parsed) = trimmed.parse::<f64>()
                && parsed.is_finite()
            {
                return normalize_xlsx_write_cell(&XlsxMatrixCell {
                    kind: "number".to_string(),
                    value: trimmed.to_string(),
                    formula: String::new(),
                    null: false,
                });
            }
            Ok(("string".to_string(), cell.value.clone()))
        }
        _ => Err(CliError::invalid_args(format!(
            "invalid cell value type {:?} (must be string, number, bool, formula, or auto)",
            cell.kind
        ))),
    }
}

fn render_empty_xlsx_cell_with_attrs(
    addr: &str,
    attrs: Option<&BTreeMap<String, String>>,
) -> String {
    let mut attrs = attrs.cloned().unwrap_or_default();
    attrs.insert("r".to_string(), addr.to_string());
    attrs.remove("t");
    format!("<c{}/>", render_xml_attrs(&attrs))
}

fn replace_xlsx_dimension(xml: &str, range: Option<&str>) -> String {
    let dimension = range.map(|range| format!("<dimension ref=\"{range}\"/>"));
    if let Some(start) = xml.find("<dimension")
        && let Some(end) = xml[start..]
            .find("/>")
            .map(|offset| start + offset + "/>".len())
            .or_else(|| xml[start..].find('>').map(|offset| start + offset + 1))
    {
        let mut updated =
            String::with_capacity(xml.len() + dimension.as_ref().map_or(0, String::len));
        updated.push_str(&xml[..start]);
        if let Some(dimension) = dimension.as_deref() {
            updated.push_str(dimension);
        }
        updated.push_str(&xml[end..]);
        return updated;
    }
    if let Some(dimension) = dimension
        && let Some(sheet_data_start) = xml.find("<sheetData")
    {
        let mut updated = String::with_capacity(xml.len() + dimension.len());
        updated.push_str(&xml[..sheet_data_start]);
        updated.push_str(&dimension);
        updated.push_str(&xml[sheet_data_start..]);
        return updated;
    }
    xml.to_string()
}

#[derive(Clone)]
struct XlsxSheetDataSpan {
    start: usize,
    open_end: usize,
    close_start: usize,
    end: usize,
    empty: bool,
}

#[derive(Clone)]
struct XlsxRowSpan {
    row: u32,
    start: usize,
    end: usize,
    attrs: BTreeMap<String, String>,
    cells: BTreeMap<u32, XlsxCellSpan>,
}

#[derive(Clone)]
struct XlsxCellSpan {
    xml: String,
    attrs: BTreeMap<String, String>,
    has_formula: bool,
}

fn xlsx_sheet_data_span(xml: &str) -> CliResult<Option<XlsxSheetDataSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                let open_end = reader.buffer_position() as usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                            return Ok(Some(XlsxSheetDataSpan {
                                start: before,
                                open_end,
                                close_start: inner_before,
                                end: reader.buffer_position() as usize,
                                empty: false,
                            }));
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("sheetData has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                let end = reader.buffer_position() as usize;
                return Ok(Some(XlsxSheetDataSpan {
                    start: before,
                    open_end: end,
                    close_start: end,
                    end,
                    empty: true,
                }));
            }
            Ok(Event::Eof) => return Ok(None),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn parse_xlsx_row_spans(
    xml: &str,
    sheet_data: Option<&XlsxSheetDataSpan>,
) -> CliResult<BTreeMap<u32, XlsxRowSpan>> {
    let Some(sheet_data) = sheet_data.filter(|span| !span.empty) else {
        return Ok(BTreeMap::new());
    };
    let fragment = &xml[sheet_data.open_end..sheet_data.close_start];
    let base = sheet_data.open_end;
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut rows = BTreeMap::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "row" => {
                let Some(row) = attr(&e, "r").and_then(|value| value.parse::<u32>().ok()) else {
                    continue;
                };
                let attrs = xml_attrs(&e);
                loop {
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "row" => {
                            let start = base + before;
                            let end = base + reader.buffer_position() as usize;
                            let row_xml = &xml[start..end];
                            rows.insert(
                                row,
                                XlsxRowSpan {
                                    row,
                                    start,
                                    end,
                                    attrs,
                                    cells: parse_xlsx_cell_spans(row_xml, start)?,
                                },
                            );
                            break;
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("row has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "row" => {
                if let Some(row) = attr(&e, "r").and_then(|value| value.parse::<u32>().ok()) {
                    let start = base + before;
                    let end = base + reader.buffer_position() as usize;
                    rows.insert(
                        row,
                        XlsxRowSpan {
                            row,
                            start,
                            end,
                            attrs: xml_attrs(&e),
                            cells: BTreeMap::new(),
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(rows)
}

fn parse_xlsx_cell_spans(row_xml: &str, base: usize) -> CliResult<BTreeMap<u32, XlsxCellSpan>> {
    let mut reader = Reader::from_str(row_xml);
    reader.config_mut().trim_text(false);
    let mut cells = BTreeMap::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "c" => {
                let Some(addr) = attr(&e, "r") else {
                    continue;
                };
                let (col, _) = parse_cell_ref(&addr)?;
                let attrs = xml_attrs(&e);
                loop {
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "c" => {
                            let end = reader.buffer_position() as usize;
                            let xml = row_xml[before..end].to_string();
                            cells.insert(
                                col,
                                XlsxCellSpan {
                                    has_formula: xlsx_cell_xml_has_formula(&xml),
                                    xml,
                                    attrs,
                                },
                            );
                            break;
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("cell has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "c" => {
                if let Some(addr) = attr(&e, "r") {
                    let (col, _) = parse_cell_ref(&addr)?;
                    let end = reader.buffer_position() as usize;
                    let xml = row_xml[before..end].to_string();
                    cells.insert(
                        col,
                        XlsxCellSpan {
                            has_formula: false,
                            xml,
                            attrs: xml_attrs(&e),
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    let _ = base;
    Ok(cells)
}

fn xlsx_cell_xml_has_formula(cell_xml: &str) -> bool {
    let mut reader = Reader::from_str(cell_xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "f" => {
                return true;
            }
            Ok(Event::Eof) => return false,
            Err(_) => return false,
            _ => {}
        }
    }
}

fn render_xlsx_row(
    row_number: u32,
    row_span: Option<&XlsxRowSpan>,
    cells: BTreeMap<u32, String>,
) -> String {
    let mut attrs = row_span.map(|span| span.attrs.clone()).unwrap_or_default();
    attrs.insert("r".to_string(), row_number.to_string());
    attrs.remove("spans");
    let mut out = format!("<row{}>", render_xml_attrs(&attrs));
    for cell_xml in cells.into_values() {
        out.push_str(&cell_xml);
    }
    out.push_str("</row>");
    out
}

fn rebuild_xlsx_sheet_data(
    xml: &str,
    sheet_data: Option<&XlsxSheetDataSpan>,
    row_spans: &BTreeMap<u32, XlsxRowSpan>,
    changed_rows: &BTreeMap<u32, String>,
) -> CliResult<String> {
    if changed_rows.is_empty() {
        return Ok(xml.to_string());
    }
    let new_sheet_data = if let Some(sheet_data) = sheet_data.filter(|span| !span.empty) {
        let mut out = String::new();
        out.push_str(&xml[sheet_data.start..sheet_data.open_end]);
        let mut last = sheet_data.open_end;
        let mut emitted = BTreeSet::new();
        let mut rows_by_start = row_spans.values().collect::<Vec<_>>();
        rows_by_start.sort_by_key(|span| span.start);
        for row_span in rows_by_start {
            for (row, row_xml) in changed_rows.range(..row_span.row) {
                if !row_spans.contains_key(row) && emitted.insert(*row) {
                    out.push_str(row_xml);
                }
            }
            out.push_str(&xml[last..row_span.start]);
            if let Some(row_xml) = changed_rows.get(&row_span.row) {
                out.push_str(row_xml);
                emitted.insert(row_span.row);
            } else {
                out.push_str(&xml[row_span.start..row_span.end]);
            }
            last = row_span.end;
        }
        out.push_str(&xml[last..sheet_data.close_start]);
        for (row, row_xml) in changed_rows {
            if emitted.insert(*row) {
                out.push_str(row_xml);
            }
        }
        out.push_str(&xml[sheet_data.close_start..sheet_data.end]);
        out
    } else {
        let mut out = String::from("<sheetData>");
        for row_xml in changed_rows.values() {
            out.push_str(row_xml);
        }
        out.push_str("</sheetData>");
        out
    };
    if let Some(sheet_data) = sheet_data {
        let mut updated = String::with_capacity(xml.len() + new_sheet_data.len());
        updated.push_str(&xml[..sheet_data.start]);
        updated.push_str(&new_sheet_data);
        updated.push_str(&xml[sheet_data.end..]);
        return Ok(updated);
    }
    let insert_at = xml
        .find("</worksheet>")
        .ok_or_else(|| CliError::unexpected("worksheet has no closing tag"))?;
    let mut updated = String::with_capacity(xml.len() + new_sheet_data.len());
    updated.push_str(&xml[..insert_at]);
    updated.push_str(&new_sheet_data);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
}

fn reject_xlsx_merged_cell_intersection(xml: &str, bounds: RangeBounds) -> CliResult<()> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "mergeCell" =>
            {
                if let Some(merge_ref) = attr(&e, "ref") {
                    let merged = parse_range(&merge_ref)?;
                    if ranges_intersect(bounds, merged) {
                        return Err(CliError::invalid_args(format!(
                            "range write intersects merged cells: {} intersects {}",
                            range_bounds_ref(bounds),
                            range_bounds_ref(merged)
                        )));
                    }
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn ranges_intersect(a: RangeBounds, b: RangeBounds) -> bool {
    a.min_col() <= b.max_col()
        && a.max_col() >= b.min_col()
        && a.min_row() <= b.max_row()
        && a.max_row() >= b.min_row()
}

fn range_bounds_ref(bounds: RangeBounds) -> String {
    let start = format!("{}{}", col_name(bounds.start_col), bounds.start_row);
    let end = format!("{}{}", col_name(bounds.end_col), bounds.end_row);
    if start == end {
        start
    } else {
        format!("{start}:{end}")
    }
}

fn xlsx_used_range_from_cell_refs(xml: &str) -> Option<String> {
    let mut min_row = u32::MAX;
    let mut max_row = 0;
    let mut min_col = u32::MAX;
    let mut max_col = 0;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "c" => {
                if let Some(addr) = attr(&e, "r")
                    && let Ok((col, row)) = parse_cell_ref(&addr)
                {
                    min_row = min_row.min(row);
                    max_row = max_row.max(row);
                    min_col = min_col.min(col);
                    max_col = max_col.max(col);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    if max_row == 0 {
        None
    } else {
        Some(format!(
            "{}{}:{}{}",
            col_name(min_col),
            min_row,
            col_name(max_col),
            max_row
        ))
    }
}

fn xlsx_range_destination_json(
    readback_file: &str,
    destination_file: Option<&str>,
    sheet: &WorkbookSheet,
    sheet_part: &str,
    range: &str,
) -> CliResult<Value> {
    let exported = xlsx_range_export_with_options(
        readback_file,
        &sheet.name,
        range,
        XlsxRangeExportOptions {
            include_types: true,
            include_formulas: true,
            include_formats: true,
            data_out: None,
            max_cells: 0,
        },
    )?;
    let mut destination = Map::new();
    if let Some(file) = destination_file {
        destination.insert("file".to_string(), json!(file));
    }
    destination.insert("sheet".to_string(), json!(sheet.name));
    destination.insert("sheetNumber".to_string(), json!(sheet.position));
    destination.insert(
        "sheetPrimarySelector".to_string(),
        json!(format!("sheetId:{}", sheet.sheet_id)),
    );
    destination.insert(
        "sheetSelectors".to_string(),
        json!(xlsx_sheet_selectors(
            &sheet.name,
            sheet.sheet_id,
            sheet.position,
            &sheet.rel_id,
            &format!("/{sheet_part}")
        )),
    );
    for key in [
        "range",
        "rows",
        "cols",
        "values",
        "types",
        "formulas",
        "styleIndexes",
        "numberFormatIds",
        "numberFormatCodes",
        "formulaCount",
        "truncated",
    ] {
        if let Some(value) = exported.get(key) {
            destination.insert(key.to_string(), value.clone());
        }
    }
    Ok(Value::Object(destination))
}

fn add_xlsx_range_mutation_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    sheet_selector: &str,
    range: &str,
) {
    let target = output_path.unwrap_or("<out.xlsx>");
    let validate_key = if output_path.is_some() {
        "validateCommand"
    } else {
        "validateCommandTemplate"
    };
    let cells_key = if output_path.is_some() {
        "cellsExtractCommand"
    } else {
        "cellsExtractCommandTemplate"
    };
    let ranges_key = if output_path.is_some() {
        "rangesExportCommand"
    } else {
        "rangesExportCommandTemplate"
    };
    result.insert(
        validate_key.to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(target))),
    );
    result.insert(
        cells_key.to_string(),
        json!(format!(
            "ooxml --json xlsx cells extract {} --sheet {} --range {} --include-empty",
            command_arg(target),
            command_arg(sheet_selector),
            command_arg(range)
        )),
    );
    result.insert(
        ranges_key.to_string(),
        json!(format!(
            "ooxml --json xlsx ranges export {} --sheet {} --range {} --include-types --include-formulas --include-formats",
            command_arg(target),
            command_arg(sheet_selector),
            command_arg(range)
        )),
    );
}
