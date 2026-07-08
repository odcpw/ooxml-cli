use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;

use crate::{
    CliError, CliResult, XlsxRangeExportOptions, XmlNamedRange, append_xml_text_event, attr,
    attr_exact, check_range_max_cells, copy_zip_with_part_override, is_xml_text_event, local_name,
    needs_xml_space_preserve, package_mutation_temp_path, package_type, parse_cli_range,
    parse_range, pptx_tables_show, range_bounds_ref, relationship_entries_from_xml,
    resolve_relationship_target, select_xlsx_table, validate, validate_xlsx_mutation_output_flags,
    xlsx_range_export_with_options, xlsx_tables, xml_attr_escape, xml_direct_child_ranges,
    xml_escape, zip_text,
};

mod output;
mod types;

use self::output::{
    delete_col_result_json, delete_row_result_json, insert_col_result_json, insert_row_result_json,
    read_table_destination, set_cell_result_json, update_from_xlsx_result_json,
};
use self::types::{
    DeleteColMutation, DeleteRowMutation, InsertColMutation, InsertRowMutation, PptxSlideRef,
    PptxTableMutationOptions, SetCellMutation, SetCellRequest, UpdateFromXlsxSource,
    UpdateMatrixMutation, XmlSpan,
};

pub(crate) fn pptx_tables_set_cell(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    let row = crate::parse_i64_flag(args, "--row")?.unwrap_or(0);
    let col = crate::parse_i64_flag(args, "--col")?.unwrap_or(0);
    for (name, value) in [("--slide", slide), ("--row", row), ("--col", col)] {
        if value < 1 {
            return Err(CliError::invalid_args(format!("{name} must be >= 1")));
        }
    }
    let table_id = crate::parse_i64_flag(args, "--table-id")?.unwrap_or(0);
    if table_id < 0 {
        return Err(CliError::invalid_args(
            "--table-id must be a positive integer",
        ));
    }
    let target = crate::parse_string_flag(args, "--target")?;
    if table_id > 0 && target.as_deref().unwrap_or_default().trim() != "" {
        return Err(CliError::invalid_args(
            "specify only one of --target or --table-id",
        ));
    }
    if table_id == 0 && target.as_deref().unwrap_or_default().trim() == "" {
        return Err(CliError::invalid_args(
            "must specify --target or --table-id",
        ));
    }
    let text = resolve_required_pptx_table_text(args)?;
    let options = parse_table_mutation_options(args)?;
    set_pptx_table_cell(
        SetCellRequest {
            file,
            slide: slide as u32,
            table_id: table_id as u32,
            target: target.as_deref(),
            row: row as usize,
            col: col as usize,
            text,
        },
        options,
    )
}

pub(crate) fn pptx_tables_delete_row(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    let row = crate::parse_i64_flag(args, "--row")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    if row < 1 {
        return Err(CliError::invalid_args("--row must be >= 1"));
    }
    let table_id = crate::parse_i64_flag(args, "--table-id")?.unwrap_or(0);
    if table_id < 0 {
        return Err(CliError::invalid_args(
            "--table-id must be a positive integer",
        ));
    }
    let target = crate::parse_string_flag(args, "--target")?;
    if table_id > 0 && target.as_deref().unwrap_or_default().trim() != "" {
        return Err(CliError::invalid_args(
            "specify only one of --target or --table-id",
        ));
    }
    if table_id == 0 && target.as_deref().unwrap_or_default().trim() == "" {
        return Err(CliError::invalid_args(
            "must specify --target or --table-id",
        ));
    }
    let options = parse_table_mutation_options(args)?;
    delete_pptx_table_row(
        file,
        slide as u32,
        table_id as u32,
        target.as_deref(),
        row as usize,
        options,
    )
}

pub(crate) fn pptx_tables_insert_row(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    let at = crate::parse_i64_flag(args, "--at")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    if at < 1 {
        return Err(CliError::invalid_args("--at must be >= 1"));
    }
    let table_id = crate::parse_i64_flag(args, "--table-id")?.unwrap_or(0);
    if table_id < 0 {
        return Err(CliError::invalid_args(
            "--table-id must be a positive integer",
        ));
    }
    let target = crate::parse_string_flag(args, "--target")?;
    if table_id > 0 && target.as_deref().unwrap_or_default().trim() != "" {
        return Err(CliError::invalid_args(
            "specify only one of --target or --table-id",
        ));
    }
    if table_id == 0 && target.as_deref().unwrap_or_default().trim() == "" {
        return Err(CliError::invalid_args(
            "must specify --target or --table-id",
        ));
    }
    let options = parse_table_mutation_options(args)?;
    insert_pptx_table_row(
        file,
        slide as u32,
        table_id as u32,
        target.as_deref(),
        at as usize,
        options,
    )
}

pub(crate) fn pptx_tables_delete_col(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    let col = crate::parse_i64_flag(args, "--col")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    if col < 1 {
        return Err(CliError::invalid_args("--col must be >= 1"));
    }
    let table_id = crate::parse_i64_flag(args, "--table-id")?.unwrap_or(0);
    if table_id < 0 {
        return Err(CliError::invalid_args(
            "--table-id must be a positive integer",
        ));
    }
    let target = crate::parse_string_flag(args, "--target")?;
    if table_id > 0 && target.as_deref().unwrap_or_default().trim() != "" {
        return Err(CliError::invalid_args(
            "specify only one of --target or --table-id",
        ));
    }
    if table_id == 0 && target.as_deref().unwrap_or_default().trim() == "" {
        return Err(CliError::invalid_args(
            "must specify --target or --table-id",
        ));
    }
    let options = parse_table_mutation_options(args)?;
    delete_pptx_table_col(
        file,
        slide as u32,
        table_id as u32,
        target.as_deref(),
        col as usize,
        options,
    )
}

pub(crate) fn pptx_tables_insert_col(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    let at = crate::parse_i64_flag(args, "--at")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    if at < 1 {
        return Err(CliError::invalid_args("--at must be >= 1"));
    }
    let width_emu = crate::parse_i64_flag(args, "--width-emu")?.unwrap_or(0);
    if width_emu < 0 {
        return Err(CliError::invalid_args("--width-emu must be >= 0"));
    }
    let table_id = crate::parse_i64_flag(args, "--table-id")?.unwrap_or(0);
    if table_id < 0 {
        return Err(CliError::invalid_args(
            "--table-id must be a positive integer",
        ));
    }
    let target = crate::parse_string_flag(args, "--target")?;
    if table_id > 0 && target.as_deref().unwrap_or_default().trim() != "" {
        return Err(CliError::invalid_args(
            "specify only one of --target or --table-id",
        ));
    }
    if table_id == 0 && target.as_deref().unwrap_or_default().trim() == "" {
        return Err(CliError::invalid_args(
            "must specify --target or --table-id",
        ));
    }
    let options = parse_table_mutation_options(args)?;
    insert_pptx_table_col(
        file,
        slide as u32,
        table_id as u32,
        target.as_deref(),
        at as usize,
        width_emu,
        options,
    )
}

pub(crate) fn pptx_tables_update_from_xlsx(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let workbook = crate::parse_string_flag(args, "--workbook")?.unwrap_or_default();
    if workbook.trim().is_empty() {
        return Err(CliError::invalid_args("--workbook is required"));
    }
    if !std::path::Path::new(&workbook).exists() {
        return Err(CliError::file_not_found(format!(
            "file not found: {workbook}"
        )));
    }
    let formula_mode = normalize_xlsx_formula_mode(
        crate::parse_string_flag(args, "--formula-mode")?.as_deref(),
        "--formula-mode",
    )?;
    let max_cells = crate::parse_i64_flag(args, "--max-cells")?.unwrap_or(100000);
    let table_id = crate::parse_i64_flag(args, "--table-id")?.unwrap_or(0);
    if table_id < 0 {
        return Err(CliError::invalid_args(
            "--table-id must be a positive integer",
        ));
    }
    let target = crate::parse_string_flag(args, "--target")?;
    if table_id > 0 && target.as_deref().unwrap_or_default().trim() != "" {
        return Err(CliError::invalid_args(
            "specify only one of --target or --table-id",
        ));
    }
    if table_id == 0 && target.as_deref().unwrap_or_default().trim() == "" {
        return Err(CliError::invalid_args(
            "must specify --target or --table-id",
        ));
    }
    let source = load_update_from_xlsx_source(
        &workbook,
        crate::parse_string_flag(args, "--sheet")?.as_deref(),
        crate::parse_string_flag(args, "--range")?.as_deref(),
        crate::parse_string_flag(args, "--table")?.as_deref(),
        max_cells,
        &formula_mode,
    )?;
    check_expected_xlsx_source_range(
        &source.range,
        crate::parse_string_flag(args, "--expect-source-range")?.as_deref(),
    )?;
    let options = parse_table_mutation_options(args)?;
    update_pptx_table_from_xlsx(
        file,
        slide as u32,
        table_id as u32,
        target.as_deref(),
        source,
        &formula_mode,
        options,
    )
}

fn value_flag_present(args: &[String], name: &str) -> bool {
    args.iter()
        .any(|arg| arg == name || arg.starts_with(&format!("{name}=")))
}

fn resolve_required_pptx_table_text(args: &[String]) -> CliResult<String> {
    let text_changed = value_flag_present(args, "--text");
    let text_file_changed = value_flag_present(args, "--text-file");
    if text_changed == text_file_changed {
        return Err(CliError::invalid_args(
            "must specify exactly one of --text or --text-file",
        ));
    }
    if text_changed {
        return Ok(crate::parse_string_flag(args, "--text")?.unwrap_or_default());
    }
    let path = crate::parse_string_flag(args, "--text-file")?.unwrap_or_default();
    fs::read(&path)
        .map(|data| String::from_utf8_lossy(&data).to_string())
        .map_err(|_| CliError::file_not_found(format!("file not found: {path}")))
}

fn parse_table_mutation_options(args: &[String]) -> CliResult<PptxTableMutationOptions> {
    let out = crate::parse_string_flag(args, "--out")?;
    let backup = crate::parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxTableMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn load_update_from_xlsx_source(
    workbook: &str,
    sheet: Option<&str>,
    range: Option<&str>,
    table: Option<&str>,
    max_cells: i64,
    formula_mode: &str,
) -> CliResult<UpdateFromXlsxSource> {
    let mut source_sheet = sheet.unwrap_or_default().trim().to_string();
    let mut source_range = range.unwrap_or_default().trim().to_string();
    let mut source_table = table.unwrap_or_default().trim().to_string();
    if !source_range.is_empty() && !source_table.is_empty() {
        return Err(CliError::invalid_args(
            "specify only one of --range or --table",
        ));
    }
    if source_range.is_empty() && source_table.is_empty() {
        return Err(CliError::invalid_args("must specify --range or --table"));
    }
    if !source_table.is_empty() {
        let tables = xlsx_tables(
            workbook,
            if source_sheet.is_empty() {
                None
            } else {
                Some(source_sheet.as_str())
            },
        )?;
        let table_ref = select_xlsx_table(&tables, &source_table)?;
        source_sheet = table_ref.sheet;
        source_range = table_ref.range;
        source_table = table_ref.display_name;
    }
    if source_sheet.is_empty() {
        return Err(CliError::invalid_args(
            "--sheet is required when using --range",
        ));
    }
    let bounds = parse_cli_range(&source_range)?.normalized();
    let source_range = range_bounds_ref(bounds);
    check_range_max_cells(&source_range, bounds, max_cells)?;
    let export = xlsx_range_export_with_options(
        workbook,
        &source_sheet,
        &source_range,
        XlsxRangeExportOptions {
            include_types: false,
            include_formulas: true,
            include_formats: false,
            data_out: None,
            max_cells,
        },
    )?;
    let export_object = export
        .as_object()
        .ok_or_else(|| CliError::unexpected("xlsx range export returned non-object"))?;
    let values = export_object
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::unexpected("xlsx range export missing values"))?;
    let formulas = export_object.get("formulas").and_then(Value::as_array);
    let data = xlsx_update_strings_from_export(values, formulas, formula_mode)?;
    let rows = export_object
        .get("rows")
        .and_then(Value::as_u64)
        .unwrap_or(data.len() as u64) as usize;
    let cols = export_object
        .get("cols")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| data.first().map(Vec::len).unwrap_or_default() as u64)
        as usize;
    let resolved_sheet = export_object
        .get("sheet")
        .and_then(Value::as_str)
        .unwrap_or(&source_sheet);
    let sheet_number = export_object
        .get("sheetNumber")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let formula_count = export_object
        .get("formulaCount")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let mut source = Map::new();
    source.insert("workbook".to_string(), json!(workbook));
    source.insert("sheet".to_string(), json!(resolved_sheet));
    source.insert("sheetNumber".to_string(), json!(sheet_number));
    source.insert("range".to_string(), json!(source_range));
    if !source_table.is_empty() {
        source.insert("table".to_string(), json!(source_table));
    }
    source.insert("rows".to_string(), json!(rows));
    source.insert("cols".to_string(), json!(cols));
    source.insert("formulaCount".to_string(), json!(formula_count));
    Ok(UpdateFromXlsxSource {
        source: Value::Object(source),
        data,
        rows,
        cols,
        range: source_range,
    })
}

fn xlsx_update_strings_from_export(
    values: &[Value],
    formulas: Option<&Vec<Value>>,
    formula_mode: &str,
) -> CliResult<Vec<Vec<String>>> {
    let mut out = Vec::with_capacity(values.len());
    for (row_index, row) in values.iter().enumerate() {
        let row_values = row
            .as_array()
            .ok_or_else(|| CliError::unexpected("xlsx range values must be rows"))?;
        let row_formulas = formulas
            .and_then(|formulas| formulas.get(row_index))
            .and_then(Value::as_array);
        let mut out_row = Vec::with_capacity(row_values.len());
        for (col_index, value) in row_values.iter().enumerate() {
            if formula_mode == "formula"
                && let Some(formula) = row_formulas
                    .and_then(|row| row.get(col_index))
                    .and_then(Value::as_str)
                && !formula.is_empty()
            {
                if formula.starts_with('=') {
                    out_row.push(formula.to_string());
                } else {
                    out_row.push(format!("={formula}"));
                }
                continue;
            }
            out_row.push(xlsx_value_to_table_text(value));
        }
        out.push(out_row);
    }
    Ok(out)
}

fn xlsx_value_to_table_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        other => other.to_string(),
    }
}

fn normalize_xlsx_formula_mode(value: Option<&str>, flag_name: &str) -> CliResult<String> {
    match value
        .unwrap_or("value")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "value" => Ok("value".to_string()),
        "formula" => Ok("formula".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "{flag_name} must be value or formula"
        ))),
    }
}

fn check_expected_xlsx_source_range(
    actual_range: &str,
    expected_range: Option<&str>,
) -> CliResult<()> {
    let expected_range = expected_range.unwrap_or_default().trim();
    if expected_range.is_empty() {
        return Ok(());
    }
    let expected = parse_range(expected_range)
        .map(|bounds| range_bounds_ref(bounds.normalized()))
        .map_err(|err| {
            CliError::invalid_args(format!("invalid --expect-source-range: {}", err.message))
        })?;
    if actual_range != expected {
        return Err(CliError::invalid_args(format!(
            "--expect-source-range mismatch: source resolved to {actual_range}, expected {expected}"
        )));
    }
    Ok(())
}

fn delete_pptx_table_row(
    file: &str,
    slide: u32,
    table_id: u32,
    target: Option<&str>,
    row: usize,
    options: PptxTableMutationOptions,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let resolved_table_id = if table_id > 0 {
        table_id
    } else {
        resolve_pptx_table_target_for_mutation(file, slide, target)?
    };
    let mutation = build_delete_row_mutation(file, slide, resolved_table_id, row)?;
    let output_path = table_mutation_output_path(file, &options);
    let staged_path =
        stage_table_mutation(file, &mutation.slide_part, &mutation.updated_xml, &options)?;
    let mut destination = read_table_destination(
        &staged_path,
        slide,
        mutation.resolved_table_id,
        output_path.as_deref(),
    )?;
    let result = delete_row_result_json(
        file,
        slide,
        row,
        &mutation,
        output_path.as_deref(),
        &mut destination,
    );
    finish_table_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn insert_pptx_table_row(
    file: &str,
    slide: u32,
    table_id: u32,
    target: Option<&str>,
    at: usize,
    options: PptxTableMutationOptions,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let resolved_table_id = if table_id > 0 {
        table_id
    } else {
        resolve_pptx_table_target_for_mutation(file, slide, target)?
    };
    let mutation = build_insert_row_mutation(file, slide, resolved_table_id, at)?;
    let output_path = table_mutation_output_path(file, &options);
    let staged_path =
        stage_table_mutation(file, &mutation.slide_part, &mutation.updated_xml, &options)?;
    let mut destination = read_table_destination(
        &staged_path,
        slide,
        mutation.resolved_table_id,
        output_path.as_deref(),
    )?;
    let result = insert_row_result_json(
        file,
        slide,
        at,
        &mutation,
        output_path.as_deref(),
        &mut destination,
    );
    finish_table_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn delete_pptx_table_col(
    file: &str,
    slide: u32,
    table_id: u32,
    target: Option<&str>,
    col: usize,
    options: PptxTableMutationOptions,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let resolved_table_id = if table_id > 0 {
        table_id
    } else {
        resolve_pptx_table_target_for_mutation(file, slide, target)?
    };
    let mutation = build_delete_col_mutation(file, slide, resolved_table_id, col)?;
    let output_path = table_mutation_output_path(file, &options);
    let staged_path =
        stage_table_mutation(file, &mutation.slide_part, &mutation.updated_xml, &options)?;
    let mut destination = read_table_destination(
        &staged_path,
        slide,
        mutation.resolved_table_id,
        output_path.as_deref(),
    )?;
    let result = delete_col_result_json(
        file,
        slide,
        col,
        &mutation,
        output_path.as_deref(),
        &mut destination,
    );
    finish_table_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn insert_pptx_table_col(
    file: &str,
    slide: u32,
    table_id: u32,
    target: Option<&str>,
    at: usize,
    width_emu: i64,
    options: PptxTableMutationOptions,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let resolved_table_id = if table_id > 0 {
        table_id
    } else {
        resolve_pptx_table_target_for_mutation(file, slide, target)?
    };
    let mutation = build_insert_col_mutation(file, slide, resolved_table_id, at, width_emu)?;
    let output_path = table_mutation_output_path(file, &options);
    let staged_path =
        stage_table_mutation(file, &mutation.slide_part, &mutation.updated_xml, &options)?;
    let mut destination = read_table_destination(
        &staged_path,
        slide,
        mutation.resolved_table_id,
        output_path.as_deref(),
    )?;
    let result = insert_col_result_json(
        file,
        slide,
        at,
        &mutation,
        output_path.as_deref(),
        &mut destination,
    );
    finish_table_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn set_pptx_table_cell(
    request: SetCellRequest<'_>,
    options: PptxTableMutationOptions,
) -> CliResult<Value> {
    let detected = package_type(request.file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let resolved_table_id = if request.table_id > 0 {
        request.table_id
    } else {
        resolve_pptx_table_target_for_mutation(request.file, request.slide, request.target)?
    };
    let mutation = build_set_cell_mutation(
        request.file,
        request.slide,
        resolved_table_id,
        request.row,
        request.col,
        request.text,
    )?;
    let output_path = table_mutation_output_path(request.file, &options);
    let staged_path = stage_table_mutation(
        request.file,
        &mutation.slide_part,
        &mutation.updated_xml,
        &options,
    )?;
    let mut destination = read_table_destination(
        &staged_path,
        request.slide,
        mutation.resolved_table_id,
        output_path.as_deref(),
    )?;
    let result = set_cell_result_json(
        request.file,
        request.slide,
        request.row,
        request.col,
        &mutation,
        output_path.as_deref(),
        &mut destination,
    );
    finish_table_mutation(request.file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn update_pptx_table_from_xlsx(
    file: &str,
    slide: u32,
    table_id: u32,
    target: Option<&str>,
    source: UpdateFromXlsxSource,
    formula_mode: &str,
    options: PptxTableMutationOptions,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let resolved_table_id = if table_id > 0 {
        table_id
    } else {
        resolve_pptx_table_target_for_mutation(file, slide, target)?
    };
    let before = read_table_destination(file, slide, resolved_table_id, None)?;
    let dest_rows = before
        .get("rows")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    let dest_cols = before
        .get("cols")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    if dest_rows != source.rows || dest_cols != source.cols {
        return Err(CliError::invalid_args(format!(
            "source/destination dimension mismatch: source is {}x{}, destination table is {}x{}",
            source.rows, source.cols, dest_rows, dest_cols
        )));
    }

    let mutation = build_update_matrix_mutation(file, slide, resolved_table_id, &source.data)?;
    let output_path = table_mutation_output_path(file, &options);
    let staged_path =
        stage_table_mutation(file, &mutation.slide_part, &mutation.updated_xml, &options)?;
    let mut destination = read_table_destination(
        &staged_path,
        slide,
        mutation.resolved_table_id,
        output_path.as_deref(),
    )?;
    let result = update_from_xlsx_result_json(
        file,
        formula_mode,
        source,
        &mutation,
        output_path.as_deref(),
        &mut destination,
        options.dry_run,
    );
    finish_table_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn resolve_pptx_table_target_for_mutation(
    file: &str,
    slide: u32,
    target: Option<&str>,
) -> CliResult<u32> {
    let show = pptx_tables_show(file, slide, 0, target, false)?;
    let tables = show
        .get("tables")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::unexpected("table readback missing tables array"))?;
    let table = match tables.as_slice() {
        [table] => table,
        [] => {
            return Err(CliError::target_not_found("target not found: table"));
        }
        _ => {
            return Err(CliError::invalid_args(
                "--target must resolve to exactly one table",
            ));
        }
    };
    table
        .get("shapeId")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| CliError::unexpected("table readback missing shapeId"))
}

fn build_delete_row_mutation(
    file: &str,
    slide: u32,
    table_id: u32,
    row: usize,
) -> CliResult<DeleteRowMutation> {
    let slides = pptx_slide_refs_for_table_mutation(file)?;
    let slide_ref = slides.get(slide as usize - 1).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide number {slide} out of range (1-{})",
            slides.len()
        ))
    })?;
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let table_span = find_table_span_for_shape(&slide_xml, table_id)?.ok_or_else(|| {
        CliError::target_not_found(format!(
            "target not found: table with ID {table_id} not found"
        ))
    })?;
    let (updated_xml, cell_count) = delete_table_row_from_slide_xml(&slide_xml, table_span, row)?;
    Ok(DeleteRowMutation {
        slide_part: slide_ref.part.clone(),
        updated_xml,
        resolved_table_id: table_id,
        cell_count,
    })
}

fn build_insert_row_mutation(
    file: &str,
    slide: u32,
    table_id: u32,
    at: usize,
) -> CliResult<InsertRowMutation> {
    let slides = pptx_slide_refs_for_table_mutation(file)?;
    let slide_ref = slides.get(slide as usize - 1).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide number {slide} out of range (1-{})",
            slides.len()
        ))
    })?;
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let table_span = find_table_span_for_shape(&slide_xml, table_id)?.ok_or_else(|| {
        CliError::target_not_found(format!(
            "target not found: table with ID {table_id} not found"
        ))
    })?;
    let (updated_xml, cell_count) = insert_table_row_into_slide_xml(&slide_xml, table_span, at)?;
    Ok(InsertRowMutation {
        slide_part: slide_ref.part.clone(),
        updated_xml,
        resolved_table_id: table_id,
        cell_count,
    })
}

fn build_delete_col_mutation(
    file: &str,
    slide: u32,
    table_id: u32,
    col: usize,
) -> CliResult<DeleteColMutation> {
    let slides = pptx_slide_refs_for_table_mutation(file)?;
    let slide_ref = slides.get(slide as usize - 1).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide number {slide} out of range (1-{})",
            slides.len()
        ))
    })?;
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let table_span = find_table_span_for_shape(&slide_xml, table_id)?.ok_or_else(|| {
        CliError::target_not_found(format!(
            "target not found: table with ID {table_id} not found"
        ))
    })?;
    let (updated_xml, row_count) = delete_table_column_from_slide_xml(&slide_xml, table_span, col)?;
    Ok(DeleteColMutation {
        slide_part: slide_ref.part.clone(),
        updated_xml,
        resolved_table_id: table_id,
        row_count,
    })
}

fn build_insert_col_mutation(
    file: &str,
    slide: u32,
    table_id: u32,
    at: usize,
    width_emu: i64,
) -> CliResult<InsertColMutation> {
    let slides = pptx_slide_refs_for_table_mutation(file)?;
    let slide_ref = slides.get(slide as usize - 1).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide number {slide} out of range (1-{})",
            slides.len()
        ))
    })?;
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let table_span = find_table_span_for_shape(&slide_xml, table_id)?.ok_or_else(|| {
        CliError::target_not_found(format!(
            "target not found: table with ID {table_id} not found"
        ))
    })?;
    let (updated_xml, row_count, width_emu) =
        insert_table_column_into_slide_xml(&slide_xml, table_span, at, width_emu)?;
    Ok(InsertColMutation {
        slide_part: slide_ref.part.clone(),
        updated_xml,
        resolved_table_id: table_id,
        row_count,
        width_emu,
    })
}

fn build_set_cell_mutation(
    file: &str,
    slide: u32,
    table_id: u32,
    row: usize,
    col: usize,
    text: String,
) -> CliResult<SetCellMutation> {
    let slides = pptx_slide_refs_for_table_mutation(file)?;
    let slide_ref = slides.get(slide as usize - 1).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide number {slide} out of range (1-{})",
            slides.len()
        ))
    })?;
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let table_span = find_table_span_for_shape(&slide_xml, table_id)?.ok_or_else(|| {
        CliError::target_not_found(format!(
            "target not found: table with ID {table_id} not found"
        ))
    })?;
    let (updated_xml, previous_text) =
        set_table_cell_text_in_slide_xml(&slide_xml, table_span, row, col, &text)?;
    Ok(SetCellMutation {
        slide_part: slide_ref.part.clone(),
        updated_xml,
        resolved_table_id: table_id,
        previous_text,
        text,
    })
}

fn build_update_matrix_mutation(
    file: &str,
    slide: u32,
    table_id: u32,
    data: &[Vec<String>],
) -> CliResult<UpdateMatrixMutation> {
    let slides = pptx_slide_refs_for_table_mutation(file)?;
    let slide_ref = slides.get(slide as usize - 1).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide number {slide} out of range (1-{})",
            slides.len()
        ))
    })?;
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let table_span = find_table_span_for_shape(&slide_xml, table_id)?.ok_or_else(|| {
        CliError::target_not_found(format!(
            "target not found: table with ID {table_id} not found"
        ))
    })?;
    let (updated_xml, updated_cells, changed_cells) =
        set_table_text_matrix_in_slide_xml(&slide_xml, table_span, data)?;
    Ok(UpdateMatrixMutation {
        slide_part: slide_ref.part.clone(),
        updated_xml,
        resolved_table_id: table_id,
        updated_cells,
        changed_cells,
    })
}

fn delete_table_row_from_slide_xml(
    slide_xml: &str,
    table_span: XmlSpan,
    row: usize,
) -> CliResult<(String, usize)> {
    let table_fragment = &slide_xml[table_span.start..table_span.end];
    let (content_start, content_end) = element_content_bounds(table_fragment)?;
    let rows: Vec<XmlNamedRange> =
        xml_direct_child_ranges(table_fragment, content_start, content_end)?
            .into_iter()
            .filter(|child| child.kind == "tr")
            .collect();
    let row_range = rows
        .get(row - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: row index out of range"))?;
    if rows.len() <= 1 {
        return Err(CliError::invalid_args("cannot delete last row"));
    }

    let row_fragment = &table_fragment[row_range.start..row_range.end];
    let (row_content_start, row_content_end) = element_content_bounds(row_fragment)?;
    let cells: Vec<XmlNamedRange> =
        xml_direct_child_ranges(row_fragment, row_content_start, row_content_end)?
            .into_iter()
            .filter(|child| child.kind == "tc")
            .collect();
    for cell in &cells {
        reject_unsafe_row_delete_cell(&row_fragment[cell.start..cell.end], row - 1)?;
    }

    let global_start = table_span.start + row_range.start;
    let global_end = table_span.start + row_range.end;
    let mut updated =
        String::with_capacity(slide_xml.len().saturating_sub(global_end - global_start));
    updated.push_str(&slide_xml[..global_start]);
    updated.push_str(&slide_xml[global_end..]);
    Ok((updated, cells.len()))
}

fn insert_table_row_into_slide_xml(
    slide_xml: &str,
    table_span: XmlSpan,
    at: usize,
) -> CliResult<(String, usize)> {
    let table_fragment = &slide_xml[table_span.start..table_span.end];
    let prefix = drawing_prefix(table_fragment);
    let (content_start, content_end) = element_content_bounds(table_fragment)?;
    let children = xml_direct_child_ranges(table_fragment, content_start, content_end)?;
    let rows: Vec<XmlNamedRange> = children
        .iter()
        .filter(|child| child.kind == "tr")
        .cloned()
        .collect();
    if at < 1 || at > rows.len() + 1 {
        return Err(CliError::target_not_found(
            "target not found: insert row index out of range",
        ));
    }
    if at <= rows.len() {
        let row_fragment = &table_fragment[rows[at - 1].start..rows[at - 1].end];
        reject_unsafe_row_insert_target(row_fragment, at - 1)?;
    }

    let cell_count = if let Some(first_row) = rows.first() {
        count_row_cells(&table_fragment[first_row.start..first_row.end])?
    } else if let Some(tbl_grid) = children.iter().find(|child| child.kind == "tblGrid") {
        count_grid_columns(&table_fragment[tbl_grid.start..tbl_grid.end])?
    } else {
        0
    };
    let height = adjacent_table_row_height(table_fragment, &rows, at)?;
    let new_row = render_empty_table_row(&prefix, height.as_deref(), cell_count);
    let insert_at = if at <= rows.len() {
        rows[at - 1].start
    } else {
        content_end
    };

    let global_start = table_span.start + insert_at;
    let mut updated = String::with_capacity(slide_xml.len() + new_row.len());
    updated.push_str(&slide_xml[..global_start]);
    updated.push_str(&new_row);
    updated.push_str(&slide_xml[global_start..]);
    Ok((updated, cell_count))
}

fn insert_table_column_into_slide_xml(
    slide_xml: &str,
    table_span: XmlSpan,
    at: usize,
    requested_width_emu: i64,
) -> CliResult<(String, usize, i64)> {
    let table_fragment = &slide_xml[table_span.start..table_span.end];
    let prefix = drawing_prefix(table_fragment);
    let (content_start, content_end) = element_content_bounds(table_fragment)?;
    let children = xml_direct_child_ranges(table_fragment, content_start, content_end)?;
    let tbl_grid = children
        .iter()
        .find(|child| child.kind == "tblGrid")
        .ok_or_else(|| CliError::unexpected("table has no tblGrid element"))?;
    let grid_fragment = &table_fragment[tbl_grid.start..tbl_grid.end];
    let (grid_content_start, grid_content_end) = element_content_bounds(grid_fragment)?;
    let grid_cols: Vec<XmlNamedRange> =
        xml_direct_child_ranges(grid_fragment, grid_content_start, grid_content_end)?
            .into_iter()
            .filter(|child| child.kind == "gridCol")
            .collect();
    if at < 1 || at > grid_cols.len() + 1 {
        return Err(CliError::target_not_found(
            "target not found: insert column index out of range",
        ));
    }
    let rows: Vec<XmlNamedRange> = children
        .iter()
        .filter(|child| child.kind == "tr")
        .cloned()
        .collect();
    if at <= grid_cols.len() {
        for row in &rows {
            let row_fragment = &table_fragment[row.start..row.end];
            let cells = row_cells(row_fragment)?;
            if let Some(cell) = cells.get(at - 1) {
                let attrs = first_element_attrs(&row_fragment[cell.start..cell.end])?;
                if attrs.get("hMerge").map(String::as_str) == Some("1") {
                    return Err(CliError::invalid_args(format!(
                        "cannot insert column at {}: would split a horizontal merge",
                        at - 1
                    )));
                }
            }
        }
    }

    let width_emu = if requested_width_emu > 0 {
        requested_width_emu
    } else {
        average_grid_width(grid_fragment, &grid_cols).unwrap_or(1_828_800)
    };
    let grid_col = render_table_grid_col(&prefix, width_emu);
    let grid_insert_at = if at <= grid_cols.len() {
        tbl_grid.start + grid_cols[at - 1].start
    } else {
        tbl_grid.start + grid_content_end
    };
    let mut insertions = vec![(grid_insert_at, grid_col)];
    for row in &rows {
        let row_fragment = &table_fragment[row.start..row.end];
        let (row_content_start, row_content_end) = element_content_bounds(row_fragment)?;
        let cells = row_cells(row_fragment)?;
        let insert_at = if at <= cells.len() {
            row.start + cells[at - 1].start
        } else {
            row.start + row_content_end
        };
        let _ = row_content_start;
        insertions.push((insert_at, render_empty_table_cell(&prefix)));
    }

    let updated_table = apply_table_insertions(table_fragment, insertions);
    let mut updated =
        String::with_capacity(slide_xml.len() + updated_table.len() - table_fragment.len());
    updated.push_str(&slide_xml[..table_span.start]);
    updated.push_str(&updated_table);
    updated.push_str(&slide_xml[table_span.end..]);
    Ok((updated, rows.len(), width_emu))
}

fn delete_table_column_from_slide_xml(
    slide_xml: &str,
    table_span: XmlSpan,
    col: usize,
) -> CliResult<(String, usize)> {
    let table_fragment = &slide_xml[table_span.start..table_span.end];
    let (content_start, content_end) = element_content_bounds(table_fragment)?;
    let children = xml_direct_child_ranges(table_fragment, content_start, content_end)?;
    let tbl_grid = children
        .iter()
        .find(|child| child.kind == "tblGrid")
        .ok_or_else(|| CliError::unexpected("table has no tblGrid element"))?;
    let grid_fragment = &table_fragment[tbl_grid.start..tbl_grid.end];
    let (grid_content_start, grid_content_end) = element_content_bounds(grid_fragment)?;
    let grid_cols: Vec<XmlNamedRange> =
        xml_direct_child_ranges(grid_fragment, grid_content_start, grid_content_end)?
            .into_iter()
            .filter(|child| child.kind == "gridCol")
            .collect();
    let col_index = col - 1;
    if col_index >= grid_cols.len() {
        return Err(CliError::target_not_found(
            "target not found: column index out of range",
        ));
    }
    if grid_cols.len() <= 1 {
        return Err(CliError::invalid_args("cannot delete last column"));
    }

    let rows: Vec<XmlNamedRange> = children
        .iter()
        .filter(|child| child.kind == "tr")
        .cloned()
        .collect();
    for row in &rows {
        let row_fragment = &table_fragment[row.start..row.end];
        let cells = row_cells(row_fragment)?;
        if let Some(cell) = cells.get(col_index) {
            reject_unsafe_column_delete_cell(&row_fragment[cell.start..cell.end], col_index)?;
        }
    }

    let mut removals = vec![(
        tbl_grid.start + grid_cols[col_index].start,
        tbl_grid.start + grid_cols[col_index].end,
    )];
    for row in &rows {
        let row_fragment = &table_fragment[row.start..row.end];
        let cells = row_cells(row_fragment)?;
        if let Some(cell) = cells.get(col_index) {
            removals.push((row.start + cell.start, row.start + cell.end));
        }
    }

    let updated_table = apply_table_removals(table_fragment, removals);
    let mut updated =
        String::with_capacity(slide_xml.len() - (table_fragment.len() - updated_table.len()));
    updated.push_str(&slide_xml[..table_span.start]);
    updated.push_str(&updated_table);
    updated.push_str(&slide_xml[table_span.end..]);
    Ok((updated, rows.len()))
}

fn set_table_cell_text_in_slide_xml(
    slide_xml: &str,
    table_span: XmlSpan,
    row: usize,
    col: usize,
    text: &str,
) -> CliResult<(String, String)> {
    let table_fragment = &slide_xml[table_span.start..table_span.end];
    let prefix = drawing_prefix(table_fragment);
    let (content_start, content_end) = element_content_bounds(table_fragment)?;
    let rows: Vec<XmlNamedRange> =
        xml_direct_child_ranges(table_fragment, content_start, content_end)?
            .into_iter()
            .filter(|child| child.kind == "tr")
            .collect();
    let row_range = rows
        .get(row - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: row index out of range"))?;
    let row_fragment = &table_fragment[row_range.start..row_range.end];
    let (row_content_start, row_content_end) = element_content_bounds(row_fragment)?;
    let cells: Vec<XmlNamedRange> =
        xml_direct_child_ranges(row_fragment, row_content_start, row_content_end)?
            .into_iter()
            .filter(|child| child.kind == "tc")
            .collect();
    let cell_range = cells
        .get(col - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: column index out of range"))?;
    let cell_fragment = &row_fragment[cell_range.start..cell_range.end];
    let previous_text = table_cell_text(cell_fragment)?;
    let replacement = replace_table_cell_text(cell_fragment, &prefix, text)?;

    let global_start = table_span.start + row_range.start + cell_range.start;
    let global_end = table_span.start + row_range.start + cell_range.end;
    let mut updated =
        String::with_capacity(slide_xml.len() - (global_end - global_start) + replacement.len());
    updated.push_str(&slide_xml[..global_start]);
    updated.push_str(&replacement);
    updated.push_str(&slide_xml[global_end..]);
    Ok((updated, previous_text))
}

fn set_table_text_matrix_in_slide_xml(
    slide_xml: &str,
    table_span: XmlSpan,
    data: &[Vec<String>],
) -> CliResult<(String, usize, usize)> {
    let (source_rows, source_cols) = validate_text_matrix(data)?;
    let table_fragment = &slide_xml[table_span.start..table_span.end];
    if table_has_merged_cells(table_fragment)? {
        return Err(CliError::invalid_args(
            "cannot update table with merged cells",
        ));
    }
    let prefix = drawing_prefix(table_fragment);
    let (content_start, content_end) = element_content_bounds(table_fragment)?;
    let rows: Vec<XmlNamedRange> =
        xml_direct_child_ranges(table_fragment, content_start, content_end)?
            .into_iter()
            .filter(|child| child.kind == "tr")
            .collect();
    let dest_cols = if let Some(first) = rows.first() {
        count_row_cells(&table_fragment[first.start..first.end])?
    } else {
        0
    };
    if rows.len() != source_rows {
        return Err(CliError::invalid_args(format!(
            "source matrix dimension mismatch: destination table is {}x{}, source is {}x{}",
            rows.len(),
            dest_cols,
            source_rows,
            source_cols
        )));
    }

    let mut replacements = Vec::new();
    let mut changed_cells = 0usize;
    for (row_index, row) in rows.iter().enumerate() {
        let row_fragment = &table_fragment[row.start..row.end];
        let cells = row_cells(row_fragment)?;
        if cells.len() != source_cols {
            return Err(CliError::invalid_args(format!(
                "source matrix dimension mismatch: destination table row {} has {} cells, source has {} columns",
                row_index + 1,
                cells.len(),
                source_cols
            )));
        }
        for (col_index, cell) in cells.iter().enumerate() {
            let cell_fragment = &row_fragment[cell.start..cell.end];
            let next_text = &data[row_index][col_index];
            if table_cell_text(cell_fragment)? != *next_text {
                changed_cells += 1;
            }
            let replacement = replace_table_cell_text(cell_fragment, &prefix, next_text)?;
            replacements.push((row.start + cell.start, row.start + cell.end, replacement));
        }
    }

    let updated_table = apply_table_replacements(table_fragment, replacements);
    let mut updated =
        String::with_capacity(slide_xml.len() + updated_table.len() - table_fragment.len());
    updated.push_str(&slide_xml[..table_span.start]);
    updated.push_str(&updated_table);
    updated.push_str(&slide_xml[table_span.end..]);
    Ok((updated, source_rows * source_cols, changed_cells))
}

fn reject_unsafe_row_delete_cell(cell_fragment: &str, row_index: usize) -> CliResult<()> {
    let attrs = first_element_attrs(cell_fragment)?;
    if let Some(row_span) = attrs
        .get("rowSpan")
        .and_then(|value| value.parse::<u32>().ok())
        && row_span > 1
    {
        return Err(CliError::invalid_args(format!(
            "cannot delete row {row_index}: cell contains vertical merge extending into row(s) below"
        )));
    }
    if attrs.get("vMerge").map(String::as_str) == Some("1") {
        return Err(CliError::invalid_args(format!(
            "cannot delete row {row_index}: cell is part of a vertical merge extending from above"
        )));
    }
    Ok(())
}

fn reject_unsafe_row_insert_target(row_fragment: &str, row_index: usize) -> CliResult<()> {
    let (content_start, content_end) = element_content_bounds(row_fragment)?;
    let cells: Vec<XmlNamedRange> =
        xml_direct_child_ranges(row_fragment, content_start, content_end)?
            .into_iter()
            .filter(|child| child.kind == "tc")
            .collect();
    for cell in cells {
        let attrs = first_element_attrs(&row_fragment[cell.start..cell.end])?;
        if attrs.get("vMerge").map(String::as_str) == Some("1") {
            return Err(CliError::invalid_args(format!(
                "cannot insert row at {row_index}: would split a vertical merge"
            )));
        }
    }
    Ok(())
}

fn reject_unsafe_column_delete_cell(cell_fragment: &str, col_index: usize) -> CliResult<()> {
    let attrs = first_element_attrs(cell_fragment)?;
    if let Some(grid_span) = attrs
        .get("gridSpan")
        .and_then(|value| value.parse::<u32>().ok())
        && grid_span > 1
    {
        return Err(CliError::invalid_args(format!(
            "cannot delete column {col_index}: cell contains horizontal merge extending right"
        )));
    }
    if attrs.get("hMerge").map(String::as_str) == Some("1") {
        return Err(CliError::invalid_args(format!(
            "cannot delete column {col_index}: cell is part of a merge extending from left"
        )));
    }
    Ok(())
}

fn row_cells(row_fragment: &str) -> CliResult<Vec<XmlNamedRange>> {
    let (content_start, content_end) = element_content_bounds(row_fragment)?;
    Ok(
        xml_direct_child_ranges(row_fragment, content_start, content_end)?
            .into_iter()
            .filter(|child| child.kind == "tc")
            .collect(),
    )
}

fn count_row_cells(row_fragment: &str) -> CliResult<usize> {
    Ok(row_cells(row_fragment)?.len())
}

fn count_grid_columns(grid_fragment: &str) -> CliResult<usize> {
    let (content_start, content_end) = element_content_bounds(grid_fragment)?;
    Ok(
        xml_direct_child_ranges(grid_fragment, content_start, content_end)?
            .into_iter()
            .filter(|child| child.kind == "gridCol")
            .count(),
    )
}

fn adjacent_table_row_height(
    table_fragment: &str,
    rows: &[XmlNamedRange],
    at: usize,
) -> CliResult<Option<String>> {
    if rows.is_empty() {
        return Ok(None);
    }
    let adjacent = if at > 1 && at - 2 < rows.len() {
        &rows[at - 2]
    } else {
        &rows[0]
    };
    let attrs = first_element_attrs(&table_fragment[adjacent.start..adjacent.end])?;
    Ok(attrs.get("h").filter(|value| !value.is_empty()).cloned())
}

fn render_empty_table_row(prefix: &str, height: Option<&str>, cell_count: usize) -> String {
    let tr = drawing_tag(prefix, "tr");
    let mut out = String::new();
    out.push('<');
    out.push_str(&tr);
    if let Some(height) = height {
        out.push_str(" h=\"");
        out.push_str(&xml_attr_escape(height));
        out.push('"');
    }
    out.push('>');
    for _ in 0..cell_count {
        out.push_str(&render_empty_table_cell(prefix));
    }
    out.push_str("</");
    out.push_str(&tr);
    out.push('>');
    out
}

fn render_empty_table_cell(prefix: &str) -> String {
    format!(
        "<{tc}><{tx_body}><{body_pr}/><{lst_style}/><{p}/></{tx_body}><{tc_pr}/></{tc}>",
        tc = drawing_tag(prefix, "tc"),
        tx_body = drawing_tag(prefix, "txBody"),
        body_pr = drawing_tag(prefix, "bodyPr"),
        lst_style = drawing_tag(prefix, "lstStyle"),
        p = drawing_tag(prefix, "p"),
        tc_pr = drawing_tag(prefix, "tcPr"),
    )
}

fn render_table_grid_col(prefix: &str, width_emu: i64) -> String {
    format!(
        "<{grid_col} w=\"{}\"/>",
        xml_attr_escape(&width_emu.to_string()),
        grid_col = drawing_tag(prefix, "gridCol"),
    )
}

fn average_grid_width(grid_fragment: &str, grid_cols: &[XmlNamedRange]) -> Option<i64> {
    if grid_cols.is_empty() {
        return None;
    }
    let total = grid_cols
        .iter()
        .filter_map(|col| {
            first_element_attrs(&grid_fragment[col.start..col.end])
                .ok()
                .and_then(|attrs| attrs.get("w").and_then(|value| value.parse::<i64>().ok()))
                .filter(|width| *width > 0)
        })
        .sum::<i64>();
    if total > 0 {
        Some(total / grid_cols.len() as i64)
    } else {
        None
    }
}

fn validate_text_matrix(data: &[Vec<String>]) -> CliResult<(usize, usize)> {
    if data.is_empty() || data.first().is_none_or(Vec::is_empty) {
        return Err(CliError::invalid_args("source matrix is empty"));
    }
    let cols = data[0].len();
    for (row_index, row) in data.iter().enumerate() {
        if row.len() != cols {
            return Err(CliError::invalid_args(format!(
                "source matrix must be rectangular: row {} has {} cells, row 1 has {}",
                row_index + 1,
                row.len(),
                cols
            )));
        }
    }
    Ok((data.len(), cols))
}

fn table_has_merged_cells(table_fragment: &str) -> CliResult<bool> {
    let (content_start, content_end) = element_content_bounds(table_fragment)?;
    let rows: Vec<XmlNamedRange> =
        xml_direct_child_ranges(table_fragment, content_start, content_end)?
            .into_iter()
            .filter(|child| child.kind == "tr")
            .collect();
    for row in rows {
        let row_fragment = &table_fragment[row.start..row.end];
        for cell in row_cells(row_fragment)? {
            if table_cell_has_merge(&row_fragment[cell.start..cell.end])? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn table_cell_has_merge(cell_fragment: &str) -> CliResult<bool> {
    let attrs = first_element_attrs(cell_fragment)?;
    for name in ["hMerge", "vMerge"] {
        if attr_value_is_true(attrs.get(name).map(String::as_str).unwrap_or_default()) {
            return Ok(true);
        }
    }
    for name in ["gridSpan", "rowSpan"] {
        if let Some(span) = attrs.get(name).and_then(|value| value.parse::<u32>().ok())
            && span > 1
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn attr_value_is_true(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true")
}

fn apply_table_insertions(table_fragment: &str, mut insertions: Vec<(usize, String)>) -> String {
    insertions.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    let mut updated = table_fragment.to_string();
    for (pos, text) in insertions {
        updated.insert_str(pos, &text);
    }
    updated
}

fn apply_table_removals(table_fragment: &str, mut removals: Vec<(usize, usize)>) -> String {
    removals.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    let mut updated = table_fragment.to_string();
    for (start, end) in removals {
        updated.replace_range(start..end, "");
    }
    updated
}

fn apply_table_replacements(
    table_fragment: &str,
    mut replacements: Vec<(usize, usize, String)>,
) -> String {
    replacements.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    let mut updated = table_fragment.to_string();
    for (start, end, replacement) in replacements {
        updated.replace_range(start..end, &replacement);
    }
    updated
}

fn table_mutation_output_path(file: &str, options: &PptxTableMutationOptions) -> Option<String> {
    if options.dry_run {
        None
    } else if options.in_place {
        Some(file.to_string())
    } else {
        options
            .out
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    }
}

fn stage_table_mutation(
    file: &str,
    slide_part: &str,
    updated_xml: &str,
    options: &PptxTableMutationOptions,
) -> CliResult<String> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-table")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_override(file, &write_path, slide_part, updated_xml)?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    Ok(write_path)
}

fn finish_table_mutation(
    file: &str,
    staged_path: &str,
    options: &PptxTableMutationOptions,
    output_path: Option<&str>,
) -> CliResult<()> {
    if options.dry_run {
        let _ = fs::remove_file(staged_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(staged_path, file)
            .or_else(|_| {
                fs::copy(staged_path, file)?;
                fs::remove_file(staged_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

fn pptx_slide_refs_for_table_mutation(file: &str) -> CliResult<Vec<PptxSlideRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slide_refs = presentation_slide_refs(&presentation);
    let rels = relationship_entries_from_xml(&zip_text(file, "ppt/_rels/presentation.xml.rels")?);
    slide_refs
        .into_iter()
        .map(|rel_id| {
            let rel = rels
                .iter()
                .find(|candidate| candidate.id == rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            Ok(PptxSlideRef {
                part: package_part_name(&resolve_relationship_target(
                    "/ppt/presentation.xml",
                    &rel.target,
                )),
            })
        })
        .collect()
}

fn presentation_slide_refs(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let Some(rel) = attr_exact(&e, "r:id") {
                    slides.push(rel);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

fn find_table_span_for_shape(xml: &str, table_id: u32) -> CliResult<Option<XmlSpan>> {
    let Some(sp_tree) = find_first_element_span(xml, "spTree")? else {
        return Err(CliError::unexpected("shape tree not found in slide"));
    };
    let (content_start, content_end) = element_content_bounds(&xml[sp_tree.start..sp_tree.end])?;
    let shapes = xml_direct_child_ranges(
        xml,
        sp_tree.start + content_start,
        sp_tree.start + content_end,
    )?;
    for shape in shapes
        .into_iter()
        .filter(|shape| shape.kind == "graphicFrame")
    {
        let fragment = &xml[shape.start..shape.end];
        if first_c_nv_pr_id(fragment) != Some(table_id) {
            continue;
        }
        if let Some(table) = find_first_element_span(fragment, "tbl")? {
            return Ok(Some(XmlSpan {
                start: shape.start + table.start,
                end: shape.start + table.end,
            }));
        }
    }
    Ok(None)
}

fn find_first_element_span(xml: &str, wanted_local: &str) -> CliResult<Option<XmlSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut active: Option<(usize, usize)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if let Some((_, depth)) = active.as_mut() {
                    *depth += 1;
                } else if local_name(e.name().as_ref()) == wanted_local {
                    active = Some((before, 1));
                }
            }
            Ok(Event::Empty(e)) => {
                if active.is_none() && local_name(e.name().as_ref()) == wanted_local {
                    return Ok(Some(XmlSpan {
                        start: before,
                        end: reader.buffer_position() as usize,
                    }));
                }
            }
            Ok(Event::End(e)) => {
                if let Some((start, depth)) = active.as_mut() {
                    if *depth == 1 && local_name(e.name().as_ref()) == wanted_local {
                        return Ok(Some(XmlSpan {
                            start: *start,
                            end: reader.buffer_position() as usize,
                        }));
                    }
                    *depth = depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(None)
}

fn element_content_bounds(fragment: &str) -> CliResult<(usize, usize)> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    if fragment[..=open_end].trim_end().ends_with("/>") {
        return Ok((open_end + 1, open_end + 1));
    }
    let close_start = fragment
        .rfind("</")
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    Ok((open_end + 1, close_start))
}

fn first_c_nv_pr_id(fragment: &str) -> Option<u32> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cNvPr" =>
            {
                return attr(&e, "id").and_then(|value| value.parse().ok());
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn first_element_attrs(fragment: &str) -> CliResult<BTreeMap<String, String>> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let mut attrs = BTreeMap::new();
                for attr in e.attributes().with_checks(false).flatten() {
                    attrs.insert(
                        local_name(attr.key.as_ref()).to_string(),
                        crate::decode_xml_text(attr.value.as_ref()),
                    );
                }
                return Ok(attrs);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(BTreeMap::new())
}

fn drawing_prefix(table_fragment: &str) -> String {
    let Some(open_end) = table_fragment.find('>') else {
        return "a".to_string();
    };
    let tag_name = crate::xml_token_name(&table_fragment[1..open_end]).unwrap_or("a:tbl");
    tag_name
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .filter(|prefix| !prefix.is_empty())
        .unwrap_or_else(|| "a".to_string())
}

fn drawing_tag(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn replace_table_cell_text(cell_fragment: &str, prefix: &str, text: &str) -> CliResult<String> {
    if let Some(tx_body) = direct_child_range(cell_fragment, "txBody")? {
        let tx_body_fragment = &cell_fragment[tx_body.start..tx_body.end];
        let r_pr_template = first_run_properties_fragment(tx_body_fragment)?;
        let paragraph = render_table_cell_paragraph(prefix, text, r_pr_template.as_deref());
        let updated_tx_body = replace_tx_body_paragraphs(tx_body_fragment, &paragraph)?;
        let mut updated = String::with_capacity(
            cell_fragment.len() - (tx_body.end - tx_body.start) + updated_tx_body.len(),
        );
        updated.push_str(&cell_fragment[..tx_body.start]);
        updated.push_str(&updated_tx_body);
        updated.push_str(&cell_fragment[tx_body.end..]);
        return Ok(updated);
    }

    let paragraph = render_table_cell_paragraph(prefix, text, None);
    let tx_body = format!(
        "<{tx_body}><{body_pr}/><{lst_style}/>{paragraph}</{tx_body}>",
        tx_body = drawing_tag(prefix, "txBody"),
        body_pr = drawing_tag(prefix, "bodyPr"),
        lst_style = drawing_tag(prefix, "lstStyle"),
    );
    insert_missing_tx_body(cell_fragment, &tx_body)
}

fn replace_tx_body_paragraphs(tx_body_fragment: &str, paragraph: &str) -> CliResult<String> {
    let (content_start, content_end) = element_content_bounds(tx_body_fragment)?;
    let paragraphs: Vec<XmlNamedRange> =
        xml_direct_child_ranges(tx_body_fragment, content_start, content_end)?
            .into_iter()
            .filter(|child| child.kind == "p")
            .collect();
    let mut content = String::new();
    let mut cursor = content_start;
    for para in paragraphs {
        content.push_str(&tx_body_fragment[cursor..para.start]);
        cursor = para.end;
    }
    content.push_str(&tx_body_fragment[cursor..content_end]);
    content.push_str(paragraph);

    let mut updated = String::with_capacity(tx_body_fragment.len() + paragraph.len());
    updated.push_str(&tx_body_fragment[..content_start]);
    updated.push_str(&content);
    updated.push_str(&tx_body_fragment[content_end..]);
    Ok(updated)
}

fn insert_missing_tx_body(cell_fragment: &str, tx_body: &str) -> CliResult<String> {
    let insert_at = if let Some(tc_pr) = direct_child_range(cell_fragment, "tcPr")? {
        tc_pr.start
    } else {
        let (_content_start, content_end) = element_content_bounds(cell_fragment)?;
        content_end
    };
    let mut updated = String::with_capacity(cell_fragment.len() + tx_body.len());
    updated.push_str(&cell_fragment[..insert_at]);
    updated.push_str(tx_body);
    updated.push_str(&cell_fragment[insert_at..]);
    Ok(updated)
}

fn direct_child_range(fragment: &str, wanted: &str) -> CliResult<Option<XmlNamedRange>> {
    let (content_start, content_end) = element_content_bounds(fragment)?;
    Ok(
        xml_direct_child_ranges(fragment, content_start, content_end)?
            .into_iter()
            .find(|child| child.kind == wanted),
    )
}

fn first_run_properties_fragment(tx_body_fragment: &str) -> CliResult<Option<String>> {
    let Some(span) = find_first_element_span(tx_body_fragment, "rPr")? else {
        return Ok(None);
    };
    Ok(Some(tx_body_fragment[span.start..span.end].to_string()))
}

fn render_table_cell_paragraph(prefix: &str, text: &str, r_pr_template: Option<&str>) -> String {
    let p = drawing_tag(prefix, "p");
    if text.is_empty() {
        return format!("<{p}/>");
    }

    let r = drawing_tag(prefix, "r");
    let t = drawing_tag(prefix, "t");
    let br = drawing_tag(prefix, "br");
    let mut out = String::new();
    out.push('<');
    out.push_str(&p);
    out.push('>');
    let lines: Vec<&str> = text.split('\n').collect();
    for (line_index, line) in lines.iter().enumerate() {
        if !line.is_empty() {
            out.push('<');
            out.push_str(&r);
            out.push('>');
            if let Some(r_pr) = r_pr_template {
                out.push_str(r_pr);
            }
            out.push('<');
            out.push_str(&t);
            if needs_xml_space_preserve(line) {
                out.push_str(" xml:space=\"preserve\"");
            }
            out.push('>');
            out.push_str(&xml_escape(line));
            out.push_str("</");
            out.push_str(&t);
            out.push('>');
            out.push_str("</");
            out.push_str(&r);
            out.push('>');
        }
        if line_index < lines.len() - 1 {
            out.push('<');
            out.push_str(&r);
            out.push_str("><");
            out.push_str(&br);
            out.push_str("/></");
            out.push_str(&r);
            out.push('>');
        }
    }
    out.push_str("</");
    out.push_str(&p);
    out.push('>');
    out
}

fn table_cell_text(cell_fragment: &str) -> CliResult<String> {
    let Some(tx_body) = direct_child_range(cell_fragment, "txBody")? else {
        return Ok(String::new());
    };
    let tx_body_fragment = &cell_fragment[tx_body.start..tx_body.end];
    let (content_start, content_end) = element_content_bounds(tx_body_fragment)?;
    let paragraphs: Vec<XmlNamedRange> =
        xml_direct_child_ranges(tx_body_fragment, content_start, content_end)?
            .into_iter()
            .filter(|child| child.kind == "p")
            .collect();
    let mut out = String::new();
    for (index, paragraph) in paragraphs.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        collect_drawing_text(&tx_body_fragment[paragraph.start..paragraph.end], &mut out)?;
    }
    Ok(out)
}

fn collect_drawing_text(fragment: &str, out: &mut String) -> CliResult<()> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "t" => {
                in_text = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => {
                in_text = false;
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "br" => {
                out.push('\n');
            }
            Ok(event) if in_text && is_xml_text_event(&event) => {
                append_xml_text_event(out, &event);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(())
}

fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}
