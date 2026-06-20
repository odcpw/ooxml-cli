use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::Path;

use super::{
    XlsxMatrixCell, add_xlsx_range_mutation_commands, resolve_xlsx_sheet_context,
    set_xlsx_range_in_sheet_xml, validate_xlsx_mutation_output_flags, xlsx_range_destination_json,
};
use crate::{
    CliError, CliResult, RangeBounds, add_xlsx_formula_recalc_package_updates,
    check_range_max_cells, col_name, copy_zip_with_part_overrides_and_removals,
    normalize_xlsx_ranges_set_data_format, parse_cell_ref, parse_cli_range, range_bounds_ref,
    validate, xlsx_ranges_set_temp_path, zip_text,
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

pub(crate) struct XlsxRangeSetMatrix {
    pub(crate) range: Option<String>,
    pub(crate) null_policy: Option<String>,
    pub(crate) major_dimension: String,
    pub(crate) rows: Vec<Vec<XlsxMatrixCell>>,
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

pub(crate) fn resolve_xlsx_ranges_set_values(
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

pub(crate) fn parse_xlsx_range_set_matrix(
    data: &str,
    data_format: &str,
) -> CliResult<XlsxRangeSetMatrix> {
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

pub(crate) fn rectangularize_xlsx_matrix(
    rows: &mut Vec<Vec<XlsxMatrixCell>>,
    ragged: &str,
) -> CliResult<()> {
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

pub(crate) fn validate_xlsx_null_policy(policy: &str) -> CliResult<()> {
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
