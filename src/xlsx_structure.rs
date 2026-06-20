use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::xlsx_sheet_xml::XlsxRowSpan;
use crate::{
    CliError, CliResult, WorkbookSheet, attr, col_name, command_arg, copy_zip_with_part_override,
    local_name, normalize_xl_target, parse_cell_ref, parse_xlsx_row_spans, relationships,
    remove_xml_span, render_xml_attrs, replace_xml_span, resolve_sheet, validate,
    validate_xlsx_mutation_output_flags, workbook_sheets, xlsx_dimension_declared,
    xlsx_ranges_set_temp_path, xlsx_sheet_data_span, xlsx_used_range_from_cell_refs, xml_attrs_map,
    xml_direct_child_ranges, xml_open_tag_from_start, xml_tag_prefix, zip_entry_names, zip_text,
};

const XLSX_MAX_ROW: u32 = 1_048_576;
const XLSX_MAX_COL: u32 = 16_384;
const STRUCTURE_DRY_RUN_PLACEHOLDER: &str = "<out.pptx>";

pub(crate) struct XlsxRowsInsertOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) at: Option<i64>,
    pub(crate) count: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxRowsDeleteOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) row: Option<i64>,
    pub(crate) count: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxColsInsertOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) at: Option<&'a str>,
    pub(crate) count: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxColsDeleteOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) col: Option<&'a str>,
    pub(crate) count: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Default)]
struct StructureMutationStats {
    shifted_rows: u32,
    shifted_cells: u32,
    removed_rows: u32,
    removed_cells: u32,
}

struct StructureResult {
    file: String,
    sheet: String,
    sheet_number: u32,
    axis: &'static str,
    operation: &'static str,
    start: u32,
    start_column: Option<String>,
    count: u32,
    shifted_rows: u32,
    shifted_cells: u32,
    removed_rows: u32,
    removed_cells: u32,
    old_used_range: Option<String>,
    new_used_range: Option<String>,
    output: Option<String>,
    dry_run: bool,
}

struct StructureResultSeed {
    axis: &'static str,
    operation: &'static str,
    start: u32,
    start_column: Option<String>,
    count: u32,
    stats: StructureMutationStats,
    old_used_range: Option<String>,
    new_used_range: Option<String>,
}

struct StructureMutationWriteOptions<'a> {
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
    in_place: bool,
}

pub(crate) fn xlsx_rows_insert(file: &str, options: XlsxRowsInsertOptions<'_>) -> CliResult<Value> {
    let sheet_selector = required_structure_sheet(options.sheet)?;
    let at = required_positive_position(options.at.unwrap_or(0), "at")?;
    let count = required_positive_count(options.count)?;
    validate_row_span_in_bounds(at, count)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheet, sheet_part, sheet_xml, workbook_xml) =
        resolve_structure_sheet(file, sheet_selector)?;
    validate_structure_request(file, &workbook_xml, &sheet_xml, false)?;
    let (updated_xml, mut result) = insert_rows_xml(file, &sheet, &sheet_xml, at, count)?;
    result.output = write_structure_mutation_output(
        file,
        &sheet_part,
        &updated_xml,
        StructureMutationWriteOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )?;
    result.dry_run = options.dry_run;
    Ok(structure_result_json(result))
}

pub(crate) fn xlsx_rows_delete(file: &str, options: XlsxRowsDeleteOptions<'_>) -> CliResult<Value> {
    let sheet_selector = required_structure_sheet(options.sheet)?;
    let row = required_positive_position(options.row.unwrap_or(0), "row")?;
    let count = required_positive_count(options.count)?;
    validate_row_span_in_bounds(row, count)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheet, sheet_part, sheet_xml, workbook_xml) =
        resolve_structure_sheet(file, sheet_selector)?;
    validate_structure_request(file, &workbook_xml, &sheet_xml, false)?;
    let (updated_xml, mut result) = delete_rows_xml(file, &sheet, &sheet_xml, row, count)?;
    result.output = write_structure_mutation_output(
        file,
        &sheet_part,
        &updated_xml,
        StructureMutationWriteOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )?;
    result.dry_run = options.dry_run;
    Ok(structure_result_json(result))
}

pub(crate) fn xlsx_cols_insert(file: &str, options: XlsxColsInsertOptions<'_>) -> CliResult<Value> {
    let sheet_selector = required_structure_sheet(options.sheet)?;
    let at = parse_structure_column(options.at.unwrap_or(""), "at")?;
    let count = required_positive_count(options.count)?;
    validate_column_span_in_bounds(at, count)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheet, sheet_part, sheet_xml, workbook_xml) =
        resolve_structure_sheet(file, sheet_selector)?;
    validate_structure_request(file, &workbook_xml, &sheet_xml, true)?;
    let (updated_xml, mut result) = insert_cols_xml(file, &sheet, &sheet_xml, at, count)?;
    result.output = write_structure_mutation_output(
        file,
        &sheet_part,
        &updated_xml,
        StructureMutationWriteOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )?;
    result.dry_run = options.dry_run;
    Ok(structure_result_json(result))
}

pub(crate) fn xlsx_cols_delete(file: &str, options: XlsxColsDeleteOptions<'_>) -> CliResult<Value> {
    let sheet_selector = required_structure_sheet(options.sheet)?;
    let col = parse_structure_column(options.col.unwrap_or(""), "col")?;
    let count = required_positive_count(options.count)?;
    validate_column_span_in_bounds(col, count)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheet, sheet_part, sheet_xml, workbook_xml) =
        resolve_structure_sheet(file, sheet_selector)?;
    validate_structure_request(file, &workbook_xml, &sheet_xml, true)?;
    let (updated_xml, mut result) = delete_cols_xml(file, &sheet, &sheet_xml, col, count)?;
    result.output = write_structure_mutation_output(
        file,
        &sheet_part,
        &updated_xml,
        StructureMutationWriteOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )?;
    result.dry_run = options.dry_run;
    Ok(structure_result_json(result))
}

fn insert_rows_xml(
    file: &str,
    sheet: &WorkbookSheet,
    xml: &str,
    at: u32,
    count: u32,
) -> CliResult<(String, StructureResult)> {
    let old_used_range = xlsx_dimension_declared(xml).filter(|value| !value.is_empty());
    let xml = ensure_sheet_data_xml(xml)?;
    let sheet_data = xlsx_sheet_data_span(&xml)?.expect("sheetData ensured");
    let row_spans = parse_xlsx_row_spans(&xml, Some(&sheet_data))?;
    let root = worksheet_root_bounds(&xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let mut rows = BTreeMap::new();
    let mut stats = StructureMutationStats::default();

    for (row_num, row_span) in &row_spans {
        if *row_num >= at {
            let new_row = offset_row(*row_num, count as i64)?;
            stats.shifted_rows += 1;
            stats.shifted_cells += row_span.cells.len() as u32;
            rows.insert(
                new_row,
                render_structure_row(&prefix, new_row, row_span, |col, _row| {
                    Ok(Some((col, new_row)))
                })?,
            );
        } else {
            rows.insert(*row_num, xml[row_span.start..row_span.end].to_string());
        }
    }

    let updated = replace_sheet_data_with_rows(&xml, &sheet_data, &prefix, &rows)?;
    let updated = update_dimension_xml(&updated)?;
    let new_used_range = xlsx_dimension_declared(&updated).filter(|value| !value.is_empty());
    Ok((
        updated,
        base_structure_result(
            file,
            sheet,
            StructureResultSeed {
                axis: "rows",
                operation: "insert",
                start: at,
                start_column: None,
                count,
                stats,
                old_used_range,
                new_used_range,
            },
        ),
    ))
}

fn delete_rows_xml(
    file: &str,
    sheet: &WorkbookSheet,
    xml: &str,
    row_start: u32,
    count: u32,
) -> CliResult<(String, StructureResult)> {
    let old_used_range = xlsx_dimension_declared(xml).filter(|value| !value.is_empty());
    let xml = ensure_sheet_data_xml(xml)?;
    let sheet_data = xlsx_sheet_data_span(&xml)?.expect("sheetData ensured");
    let row_spans = parse_xlsx_row_spans(&xml, Some(&sheet_data))?;
    let root = worksheet_root_bounds(&xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let row_end = row_start + count - 1;
    let mut rows = BTreeMap::new();
    let mut stats = StructureMutationStats::default();

    for (row_num, row_span) in &row_spans {
        if (*row_num >= row_start) && (*row_num <= row_end) {
            stats.removed_rows += 1;
            stats.removed_cells += row_span.cells.len() as u32;
        } else if *row_num > row_end {
            let new_row = offset_row(*row_num, -(count as i64))?;
            stats.shifted_rows += 1;
            stats.shifted_cells += row_span.cells.len() as u32;
            if !row_span.cells.is_empty() {
                rows.insert(
                    new_row,
                    render_structure_row(&prefix, new_row, row_span, |col, _row| {
                        Ok(Some((col, new_row)))
                    })?,
                );
            }
        } else if !row_span.cells.is_empty() {
            rows.insert(*row_num, xml[row_span.start..row_span.end].to_string());
        }
    }

    let updated = replace_sheet_data_with_rows(&xml, &sheet_data, &prefix, &rows)?;
    let updated = update_dimension_xml(&updated)?;
    let new_used_range = xlsx_dimension_declared(&updated).filter(|value| !value.is_empty());
    Ok((
        updated,
        base_structure_result(
            file,
            sheet,
            StructureResultSeed {
                axis: "rows",
                operation: "delete",
                start: row_start,
                start_column: None,
                count,
                stats,
                old_used_range,
                new_used_range,
            },
        ),
    ))
}

fn insert_cols_xml(
    file: &str,
    sheet: &WorkbookSheet,
    xml: &str,
    at: u32,
    count: u32,
) -> CliResult<(String, StructureResult)> {
    let old_used_range = xlsx_dimension_declared(xml).filter(|value| !value.is_empty());
    let xml = ensure_sheet_data_xml(xml)?;
    let sheet_data = xlsx_sheet_data_span(&xml)?.expect("sheetData ensured");
    let row_spans = parse_xlsx_row_spans(&xml, Some(&sheet_data))?;
    let root = worksheet_root_bounds(&xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let mut rows = BTreeMap::new();
    let mut stats = StructureMutationStats::default();

    for (row_num, row_span) in &row_spans {
        rows.insert(
            *row_num,
            render_structure_row(&prefix, *row_num, row_span, |col, row| {
                let new_col = if col >= at {
                    stats.shifted_cells += 1;
                    offset_col(col, count as i64)?
                } else {
                    col
                };
                Ok(Some((new_col, row)))
            })?,
        );
    }

    let updated = replace_sheet_data_with_rows(&xml, &sheet_data, &prefix, &rows)?;
    let updated = update_dimension_xml(&updated)?;
    let new_used_range = xlsx_dimension_declared(&updated).filter(|value| !value.is_empty());
    Ok((
        updated,
        base_structure_result(
            file,
            sheet,
            StructureResultSeed {
                axis: "cols",
                operation: "insert",
                start: at,
                start_column: Some(col_name(at)),
                count,
                stats,
                old_used_range,
                new_used_range,
            },
        ),
    ))
}

fn delete_cols_xml(
    file: &str,
    sheet: &WorkbookSheet,
    xml: &str,
    col_start: u32,
    count: u32,
) -> CliResult<(String, StructureResult)> {
    let old_used_range = xlsx_dimension_declared(xml).filter(|value| !value.is_empty());
    let xml = ensure_sheet_data_xml(xml)?;
    let sheet_data = xlsx_sheet_data_span(&xml)?.expect("sheetData ensured");
    let row_spans = parse_xlsx_row_spans(&xml, Some(&sheet_data))?;
    let root = worksheet_root_bounds(&xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let col_end = col_start + count - 1;
    let mut rows = BTreeMap::new();
    let mut stats = StructureMutationStats::default();

    for (row_num, row_span) in &row_spans {
        let row_xml = render_structure_row(&prefix, *row_num, row_span, |col, row| {
            if (col >= col_start) && (col <= col_end) {
                stats.removed_cells += 1;
                Ok(None)
            } else if col > col_end {
                stats.shifted_cells += 1;
                Ok(Some((offset_col(col, -(count as i64))?, row)))
            } else {
                Ok(Some((col, row)))
            }
        })?;
        if row_xml_has_cells(&row_xml) {
            rows.insert(*row_num, row_xml);
        }
    }

    let updated = replace_sheet_data_with_rows(&xml, &sheet_data, &prefix, &rows)?;
    let updated = update_dimension_xml(&updated)?;
    let new_used_range = xlsx_dimension_declared(&updated).filter(|value| !value.is_empty());
    Ok((
        updated,
        base_structure_result(
            file,
            sheet,
            StructureResultSeed {
                axis: "cols",
                operation: "delete",
                start: col_start,
                start_column: Some(col_name(col_start)),
                count,
                stats,
                old_used_range,
                new_used_range,
            },
        ),
    ))
}

fn base_structure_result(
    file: &str,
    sheet: &WorkbookSheet,
    seed: StructureResultSeed,
) -> StructureResult {
    StructureResult {
        file: file.to_string(),
        sheet: sheet.name.clone(),
        sheet_number: sheet.position,
        axis: seed.axis,
        operation: seed.operation,
        start: seed.start,
        start_column: seed.start_column,
        count: seed.count,
        shifted_rows: seed.stats.shifted_rows,
        shifted_cells: seed.stats.shifted_cells,
        removed_rows: seed.stats.removed_rows,
        removed_cells: seed.stats.removed_cells,
        old_used_range: seed.old_used_range,
        new_used_range: seed.new_used_range,
        output: None,
        dry_run: false,
    }
}

fn structure_result_json(result: StructureResult) -> Value {
    let mut out = Map::new();
    out.insert("file".to_string(), json!(result.file));
    out.insert("sheet".to_string(), json!(result.sheet));
    out.insert("sheetNumber".to_string(), json!(result.sheet_number));
    out.insert("axis".to_string(), json!(result.axis));
    out.insert("operation".to_string(), json!(result.operation));
    out.insert("start".to_string(), json!(result.start));
    if let Some(start_column) = result.start_column {
        out.insert("startColumn".to_string(), json!(start_column));
    }
    out.insert("count".to_string(), json!(result.count));
    if result.shifted_rows > 0 {
        out.insert("shiftedRows".to_string(), json!(result.shifted_rows));
    }
    out.insert("shiftedCells".to_string(), json!(result.shifted_cells));
    if result.removed_rows > 0 {
        out.insert("removedRows".to_string(), json!(result.removed_rows));
    }
    if result.removed_cells > 0 {
        out.insert("removedCells".to_string(), json!(result.removed_cells));
    }
    if let Some(old_used_range) = result.old_used_range {
        out.insert("oldUsedRange".to_string(), json!(old_used_range));
    }
    if let Some(new_used_range) = result.new_used_range {
        out.insert("newUsedRange".to_string(), json!(new_used_range));
    }
    if let Some(output) = result.output.as_deref() {
        out.insert("output".to_string(), json!(output));
    }
    out.insert("dryRun".to_string(), json!(result.dry_run));
    let sheet_selector = if result.sheet.is_empty() {
        format!("sheet:{}", result.sheet_number)
    } else {
        result.sheet.clone()
    };
    if let Some(output) = result.output.as_deref() {
        out.insert(
            "validateCommand".to_string(),
            json!(xlsx_validate_command(output)),
        );
        out.insert(
            "sheetShowCommand".to_string(),
            json!(xlsx_sheet_show_command(output, &sheet_selector)),
        );
        out.insert(
            "sheetsListCommand".to_string(),
            json!(xlsx_sheets_list_command(output)),
        );
    } else {
        out.insert(
            "validateCommandTemplate".to_string(),
            json!(xlsx_validate_command(STRUCTURE_DRY_RUN_PLACEHOLDER)),
        );
        out.insert(
            "sheetShowCommandTemplate".to_string(),
            json!(xlsx_sheet_show_command(
                STRUCTURE_DRY_RUN_PLACEHOLDER,
                &sheet_selector
            )),
        );
        out.insert(
            "sheetsListCommandTemplate".to_string(),
            json!(xlsx_sheets_list_command(STRUCTURE_DRY_RUN_PLACEHOLDER)),
        );
    }
    Value::Object(out)
}

fn xlsx_validate_command(file: &str) -> String {
    format!("ooxml validate --strict {}", command_arg(file))
}

fn xlsx_sheet_show_command(file: &str, sheet_selector: &str) -> String {
    format!(
        "ooxml --json xlsx sheets show {} --sheet {}",
        command_arg(file),
        command_arg(sheet_selector)
    )
}

fn xlsx_sheets_list_command(file: &str) -> String {
    format!("ooxml --json xlsx sheets list {}", command_arg(file))
}

fn required_structure_sheet(sheet: Option<&str>) -> CliResult<&str> {
    sheet
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            CliError::invalid_args("--sheet is required for structural row/column edits")
        })
}

fn required_positive_position(value: i64, name: &str) -> CliResult<u32> {
    if value < 1 {
        return Err(CliError::invalid_args(format!("--{name} must be >= 1")));
    }
    u32::try_from(value).map_err(|_| CliError::invalid_args(format!("--{name} must be >= 1")))
}

fn required_positive_count(value: i64) -> CliResult<u32> {
    if value < 1 {
        return Err(CliError::invalid_args("--count must be positive"));
    }
    u32::try_from(value).map_err(|_| CliError::invalid_args("--count must be positive"))
}

fn parse_structure_column(value: &str, name: &str) -> CliResult<u32> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args(format!(
            "invalid --{name}: column reference cannot be empty"
        )));
    }
    if value.chars().any(|ch| ch == '$' || ch.is_ascii_digit()) {
        return Err(CliError::invalid_args(format!(
            "invalid --{name}: invalid column reference {value:?}"
        )));
    }
    let mut col = 0_u32;
    for ch in value.chars() {
        let upper = ch.to_ascii_uppercase();
        if !upper.is_ascii_alphabetic() {
            return Err(CliError::invalid_args(format!(
                "invalid --{name}: invalid column letter {upper:?}"
            )));
        }
        col = col * 26 + (upper as u32 - 'A' as u32 + 1);
        if col > XLSX_MAX_COL {
            return Err(CliError::invalid_args(format!(
                "invalid --{name}: column {value:?} out of XLSX bounds A-XFD"
            )));
        }
    }
    Ok(col)
}

fn validate_row_span_in_bounds(start: u32, count: u32) -> CliResult<()> {
    if count > XLSX_MAX_ROW {
        return Err(CliError::invalid_args(format!(
            "worksheet structural edit out of bounds: row count {count} exceeds XLSX row limit {XLSX_MAX_ROW}"
        )));
    }
    let delta = count - 1;
    if start as u64 + delta as u64 > XLSX_MAX_ROW as u64 {
        return Err(CliError::invalid_args(format!(
            "worksheet structural edit out of bounds: row {start} offset by {delta} is out of XLSX bounds 1-{XLSX_MAX_ROW}"
        )));
    }
    Ok(())
}

fn validate_column_span_in_bounds(start: u32, count: u32) -> CliResult<()> {
    if count > XLSX_MAX_COL {
        return Err(CliError::invalid_args(format!(
            "worksheet structural edit out of bounds: column count {count} exceeds XLSX column limit {XLSX_MAX_COL}"
        )));
    }
    let delta = count - 1;
    if start as u64 + delta as u64 > XLSX_MAX_COL as u64 {
        return Err(CliError::invalid_args(format!(
            "worksheet structural edit out of bounds: column {start} offset by {delta} is out of XLSX bounds 1-{XLSX_MAX_COL}"
        )));
    }
    Ok(())
}

fn offset_row(row: u32, delta: i64) -> CliResult<u32> {
    let shifted = row as i64 + delta;
    if shifted < 1 || shifted > XLSX_MAX_ROW as i64 {
        return Err(CliError::invalid_args(format!(
            "worksheet structural edit out of bounds: row {row} offset by {delta} is out of XLSX bounds 1-{XLSX_MAX_ROW}"
        )));
    }
    Ok(shifted as u32)
}

fn offset_col(col: u32, delta: i64) -> CliResult<u32> {
    let shifted = col as i64 + delta;
    if shifted < 1 || shifted > XLSX_MAX_COL as i64 {
        return Err(CliError::invalid_args(format!(
            "worksheet structural edit out of bounds: column {col} offset by {delta} is out of XLSX bounds 1-{XLSX_MAX_COL}"
        )));
    }
    Ok(shifted as u32)
}

fn resolve_structure_sheet(
    file: &str,
    sheet_selector: &str,
) -> CliResult<(WorkbookSheet, String, String, String)> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    if sheets.is_empty() {
        return Err(CliError::invalid_args("workbook has no sheets"));
    }
    let sheet = resolve_sheet(&sheets, sheet_selector).map_err(|err| {
        if err.code == "target_not_found" {
            return err;
        }
        if let Ok(number) = sheet_selector.parse::<u32>()
            && (number < 1 || number > sheets.len() as u32)
        {
            return CliError::target_not_found(format!(
                "sheet {number} is out of range (1-{})",
                sheets.len()
            ));
        }
        CliError::target_not_found(format!(
            "sheet not found: {sheet_selector}; did you mean: {}; discover with `ooxml --json xlsx sheets list <file>`",
            structure_sheet_candidate(&sheets)
        ))
    })?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let sheet_xml = zip_text(file, &sheet_part)?;
    Ok((sheet, sheet_part, sheet_xml, workbook))
}

fn structure_sheet_candidate(sheets: &[WorkbookSheet]) -> String {
    sheets
        .first()
        .map(|sheet| format!("sheetId:{}", sheet.sheet_id))
        .unwrap_or_else(|| "1".to_string())
}

fn validate_structure_request(
    file: &str,
    workbook_xml: &str,
    sheet_xml: &str,
    column_edit: bool,
) -> CliResult<()> {
    validate_structure_references(sheet_xml)?;
    scan_structure_hazards(file, workbook_xml, sheet_xml, column_edit)
}

fn scan_structure_hazards(
    file: &str,
    workbook_xml: &str,
    sheet_xml: &str,
    column_edit: bool,
) -> CliResult<()> {
    if xml_has_element(workbook_xml, "definedNames") {
        return Err(CliError::invalid_args("workbook has defined names"));
    }
    if zip_entry_names(file)?
        .iter()
        .any(|entry| entry == "xl/calcChain.xml")
    {
        return Err(CliError::invalid_args("workbook has calc chain"));
    }
    if xml_has_element(sheet_xml, "f") {
        return Err(CliError::invalid_args("worksheet has formulas"));
    }
    if xml_has_element(sheet_xml, "mergeCells") || xml_has_element(sheet_xml, "mergeCell") {
        return Err(CliError::invalid_args("worksheet has merged cells"));
    }
    let direct = worksheet_direct_child_kinds(sheet_xml)?;
    if direct.contains("tableParts") {
        return Err(CliError::invalid_args("worksheet has tables"));
    }
    if direct.contains("autoFilter") || direct.contains("sortState") {
        return Err(CliError::invalid_args(
            "worksheet has autofilter or sort state",
        ));
    }
    if direct.contains("drawing")
        || direct.contains("legacyDrawing")
        || direct.contains("legacyDrawingHF")
    {
        return Err(CliError::invalid_args("worksheet has drawings or comments"));
    }
    if direct.contains("hyperlinks") {
        return Err(CliError::invalid_args("worksheet has hyperlinks"));
    }
    if direct.contains("conditionalFormatting") {
        return Err(CliError::invalid_args(
            "worksheet has conditional formatting",
        ));
    }
    if direct.contains("dataValidations") {
        return Err(CliError::invalid_args("worksheet has data validations"));
    }
    if column_edit && direct.contains("cols") {
        return Err(CliError::invalid_args("worksheet has column metadata"));
    }
    for name in [
        "protectedRanges",
        "scenarios",
        "rowBreaks",
        "colBreaks",
        "dataConsolidate",
        "customSheetViews",
        "cellWatches",
        "ignoredErrors",
        "smartTags",
        "picture",
        "oleObjects",
        "controls",
        "webPublishItems",
        "extLst",
    ] {
        if direct.contains(name) {
            return Err(CliError::invalid_args(format!(
                "worksheet has unsupported structural references: {name}"
            )));
        }
    }
    Ok(())
}

fn validate_structure_references(xml: &str) -> CliResult<()> {
    let Some(sheet_data) = xlsx_sheet_data_span(xml)?.filter(|span| !span.empty) else {
        return Ok(());
    };
    let fragment = &xml[sheet_data.open_end..sheet_data.close_start];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut seen_rows = BTreeSet::new();
    let mut in_row = false;
    let mut row_depth = 0_u32;
    let mut current_row = 0_u32;
    let mut seen_cells = BTreeSet::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if !in_row && local_name(e.name().as_ref()) == "row" => {
                current_row = validate_row_reference(&e, &mut seen_rows)?;
                seen_cells.clear();
                in_row = true;
                row_depth = 1;
            }
            Ok(Event::Empty(e)) if !in_row && local_name(e.name().as_ref()) == "row" => {
                validate_row_reference(&e, &mut seen_rows)?;
            }
            Ok(Event::Start(e)) if in_row => {
                if row_depth == 1 && local_name(e.name().as_ref()) == "c" {
                    validate_cell_reference(&e, current_row, &mut seen_cells)?;
                }
                row_depth += 1;
            }
            Ok(Event::Empty(e)) if in_row => {
                if row_depth == 1 && local_name(e.name().as_ref()) == "c" {
                    validate_cell_reference(&e, current_row, &mut seen_cells)?;
                }
            }
            Ok(Event::End(e)) if in_row => {
                if row_depth == 1 && local_name(e.name().as_ref()) == "row" {
                    in_row = false;
                    row_depth = 0;
                } else {
                    row_depth = row_depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn validate_row_reference(e: &BytesStart<'_>, seen_rows: &mut BTreeSet<u32>) -> CliResult<u32> {
    let Some(row_text) = attr(e, "r").filter(|value| !value.trim().is_empty()) else {
        return invalid_structure_reference("row is missing r attribute");
    };
    let row = row_text
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|row| (1..=XLSX_MAX_ROW).contains(row))
        .ok_or_else(|| {
            CliError::invalid_args(format!(
                "worksheet has missing or invalid row/cell references: invalid row reference {row_text:?}"
            ))
        })?;
    if !seen_rows.insert(row) {
        return invalid_structure_reference(format!("duplicate row reference {row}"));
    }
    Ok(row)
}

fn validate_cell_reference(
    e: &BytesStart<'_>,
    row: u32,
    seen_cells: &mut BTreeSet<String>,
) -> CliResult<()> {
    let Some(ref_text) = attr(e, "r").filter(|value| !value.trim().is_empty()) else {
        return invalid_structure_reference(format!("cell in row {row} is missing r attribute"));
    };
    let (col, cell_row) = parse_cell_ref(&ref_text).map_err(|_| {
        CliError::invalid_args(format!(
            "worksheet has missing or invalid row/cell references: invalid cell reference {ref_text:?}"
        ))
    })?;
    let normalized = format!("{}{}", col_name(col), cell_row);
    if cell_row != row {
        return invalid_structure_reference(format!("cell {normalized} is stored in row {row}"));
    }
    if !seen_cells.insert(normalized.clone()) {
        return invalid_structure_reference(format!("duplicate cell reference {normalized}"));
    }
    Ok(())
}

fn invalid_structure_reference<T>(message: impl Into<String>) -> CliResult<T> {
    Err(CliError::invalid_args(format!(
        "worksheet has missing or invalid row/cell references: {}",
        message.into()
    )))
}

fn xml_has_element(xml: &str, local: &str) -> bool {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == local => {
                return true;
            }
            Ok(Event::Eof) => return false,
            Err(_) => return false,
            _ => {}
        }
    }
}

fn worksheet_direct_child_kinds(xml: &str) -> CliResult<BTreeSet<String>> {
    let root = worksheet_root_bounds(xml)?;
    if root.self_closing || root.open_end >= root.close_start {
        return Ok(BTreeSet::new());
    }
    Ok(
        xml_direct_child_ranges(xml, root.open_end, root.close_start)?
            .into_iter()
            .map(|range| range.kind)
            .collect(),
    )
}

fn render_structure_row<F>(
    prefix: &str,
    row_number: u32,
    row_span: &XlsxRowSpan,
    mut map_cell: F,
) -> CliResult<String>
where
    F: FnMut(u32, u32) -> CliResult<Option<(u32, u32)>>,
{
    let mut attrs = row_span.attrs.clone();
    attrs.insert("r".to_string(), row_number.to_string());
    attrs.remove("spans");
    let mut cells = BTreeMap::new();
    for (col, cell) in &row_span.cells {
        if let Some((new_col, new_row)) = map_cell(*col, row_number)? {
            cells.insert(new_col, update_cell_ref_xml(&cell.xml, new_col, new_row)?);
        }
    }
    let mut out = format!(
        "<{}{}>",
        element_name(prefix, "row"),
        render_xml_attrs(&attrs)
    );
    for cell in cells.into_values() {
        out.push_str(&cell);
    }
    out.push_str(&format!("</{}>", element_name(prefix, "row")));
    Ok(out)
}

fn update_cell_ref_xml(cell_xml: &str, col: u32, row: u32) -> CliResult<String> {
    let mut reader = Reader::from_str(cell_xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "c" => {
                let open_end = reader.buffer_position() as usize;
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = xml_attrs_map(&e);
                attrs.insert("r".to_string(), format!("{}{}", col_name(col), row));
                return Ok(format!(
                    "<{}{}>{}",
                    tag_name,
                    render_xml_attrs(&attrs),
                    &cell_xml[open_end..]
                ));
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "c" => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = xml_attrs_map(&e);
                attrs.insert("r".to_string(), format!("{}{}", col_name(col), row));
                return Ok(format!("<{}{}/>", tag_name, render_xml_attrs(&attrs)));
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("cell element not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn row_xml_has_cells(row_xml: &str) -> bool {
    let mut reader = Reader::from_str(row_xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "c" => {
                return true;
            }
            Ok(Event::Eof) => return false,
            Err(_) => return false,
            _ => {}
        }
    }
}

fn replace_sheet_data_with_rows(
    xml: &str,
    sheet_data: &crate::xlsx_sheet_xml::XlsxSheetDataSpan,
    prefix: &str,
    rows: &BTreeMap<u32, String>,
) -> CliResult<String> {
    let sheet_data_name = element_name(prefix, "sheetData");
    let mut replacement = format!("<{sheet_data_name}>");
    for row_xml in rows.values() {
        replacement.push_str(row_xml);
    }
    replacement.push_str(&format!("</{sheet_data_name}>"));
    Ok(replace_xml_span(
        xml,
        sheet_data.start,
        sheet_data.end,
        &replacement,
    ))
}

fn update_dimension_xml(xml: &str) -> CliResult<String> {
    let used_range = xlsx_used_range_from_cell_refs(xml);
    let root = worksheet_root_bounds(xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let dimension_range = direct_worksheet_child_range(xml, &root, "dimension")?;
    match (used_range, dimension_range) {
        (Some(range_ref), Some(existing)) => Ok(replace_xml_span(
            xml,
            existing.start,
            existing.end,
            &render_dimension_xml(&prefix, &range_ref),
        )),
        (Some(range_ref), None) => insert_worksheet_child(
            xml,
            &root,
            "dimension",
            &render_dimension_xml(&prefix, &range_ref),
        ),
        (None, Some(existing)) => Ok(remove_xml_span(xml, existing.start, existing.end)),
        (None, None) => Ok(xml.to_string()),
    }
}

fn render_dimension_xml(prefix: &str, range_ref: &str) -> String {
    format!(
        "<{} ref=\"{}\"/>",
        element_name(prefix, "dimension"),
        range_ref
    )
}

fn write_structure_mutation_output(
    file: &str,
    sheet_part: &str,
    updated_xml: &str,
    options: StructureMutationWriteOptions<'_>,
) -> CliResult<Option<String>> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
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

    copy_zip_with_part_override(file, &readback_path, sheet_part, updated_xml)?;
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
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(commit_path.map(ToOwned::to_owned))
}

#[derive(Clone)]
struct WorksheetRootBounds {
    start: usize,
    open_end: usize,
    close_start: usize,
    end: usize,
    tag_name: String,
    self_closing: bool,
}

fn worksheet_root_bounds(xml: &str) -> CliResult<WorksheetRootBounds> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "worksheet" => {
                let open_end = reader.buffer_position() as usize;
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let close_tag = format!("</{tag_name}>");
                let close_start = xml
                    .rfind(&close_tag)
                    .ok_or_else(|| CliError::unexpected("worksheet root has no closing tag"))?;
                return Ok(WorksheetRootBounds {
                    start: before,
                    open_end,
                    close_start,
                    end: close_start + close_tag.len(),
                    tag_name,
                    self_closing: false,
                });
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "worksheet" => {
                return Ok(WorksheetRootBounds {
                    start: before,
                    open_end: reader.buffer_position() as usize,
                    close_start: reader.buffer_position() as usize,
                    end: reader.buffer_position() as usize,
                    tag_name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    self_closing: true,
                });
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("worksheet root not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn ensure_sheet_data_xml(xml: &str) -> CliResult<String> {
    if xlsx_sheet_data_span(xml)?.is_some() {
        return Ok(xml.to_string());
    }
    let root = worksheet_root_bounds(xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let sheet_data = element_name(&prefix, "sheetData");
    insert_worksheet_child(
        xml,
        &root,
        "sheetData",
        &format!("<{sheet_data}></{sheet_data}>"),
    )
}

fn direct_worksheet_child_range(
    xml: &str,
    root: &WorksheetRootBounds,
    kind: &str,
) -> CliResult<Option<crate::XmlNamedRange>> {
    if root.self_closing || root.open_end >= root.close_start {
        return Ok(None);
    }
    Ok(
        xml_direct_child_ranges(xml, root.open_end, root.close_start)?
            .into_iter()
            .find(|child| child.kind == kind),
    )
}

fn insert_worksheet_child(
    xml: &str,
    root: &WorksheetRootBounds,
    local: &str,
    child_xml: &str,
) -> CliResult<String> {
    if root.self_closing {
        let start_tag = xml_open_tag_from_start(&xml[root.start..root.open_end]);
        let mut updated = String::new();
        updated.push_str(&xml[..root.start]);
        updated.push_str(&start_tag);
        updated.push_str(child_xml);
        updated.push_str(&format!("</{}>", root.tag_name));
        updated.push_str(&xml[root.end..]);
        return Ok(updated);
    }
    let target_order = worksheet_child_order(local);
    let insert_at = xml_direct_child_ranges(xml, root.open_end, root.close_start)?
        .into_iter()
        .find(|child| worksheet_child_order(&child.kind) > target_order)
        .map(|child| child.start)
        .unwrap_or(root.close_start);
    let mut updated = String::new();
    updated.push_str(&xml[..insert_at]);
    updated.push_str(child_xml);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
}

fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn worksheet_child_order(local_name: &str) -> i32 {
    match local_name {
        "sheetPr" => 10,
        "dimension" => 20,
        "sheetViews" => 30,
        "sheetFormatPr" => 40,
        "cols" => 50,
        "sheetData" => 60,
        "sheetCalcPr" => 70,
        "sheetProtection" => 80,
        "protectedRanges" => 90,
        "scenarios" => 100,
        "autoFilter" => 110,
        "sortState" => 120,
        "dataConsolidate" => 130,
        "customSheetViews" => 140,
        "mergeCells" => 150,
        "phoneticPr" => 160,
        "conditionalFormatting" => 170,
        "dataValidations" => 180,
        "hyperlinks" => 190,
        "printOptions" => 200,
        "pageMargins" => 210,
        "pageSetup" => 220,
        "headerFooter" => 230,
        "rowBreaks" => 240,
        "colBreaks" => 250,
        "customProperties" => 260,
        "cellWatches" => 270,
        "ignoredErrors" => 280,
        "smartTags" => 290,
        "drawing" => 300,
        "legacyDrawing" => 310,
        "legacyDrawingHF" => 320,
        "picture" => 330,
        "oleObjects" => 340,
        "controls" => 350,
        "webPublishItems" => 360,
        "tableParts" => 370,
        "extLst" => 380,
        _ => 1000,
    }
}
