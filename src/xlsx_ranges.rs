use serde_json::{Map, Value, json};
use std::fs;

use crate::{
    CliError, CliResult, RangeBounds, col_name, command_arg, normalize_xl_target, parse_cli_range,
    relationships, resolve_sheet, shared_strings, sheet_cells, workbook_sheets, xlsx_styles,
    zip_text,
};
pub(crate) struct XlsxRangeExportOptions<'a> {
    pub(crate) include_types: bool,
    pub(crate) include_formulas: bool,
    pub(crate) include_formats: bool,
    pub(crate) data_out: Option<&'a str>,
    pub(crate) max_cells: i64,
}

pub(crate) fn xlsx_range_export_with_options(
    file: &str,
    sheet_selector: &str,
    range: &str,
    options: XlsxRangeExportOptions<'_>,
) -> CliResult<Value> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let shared_strings = shared_strings(file).unwrap_or_default();
    let styles = xlsx_styles(file).unwrap_or_default();
    let sheet_xml = zip_text(file, &sheet_part)?;
    let cells = sheet_cells(&sheet_xml, &shared_strings, &styles);
    let bounds = parse_cli_range(range)?;
    check_range_max_cells(range, bounds, options.max_cells)?;
    let mut values = Vec::new();
    let mut types = Vec::new();
    let mut formulas = Vec::new();
    let mut style_indexes = Vec::new();
    let mut number_format_ids = Vec::new();
    let mut number_format_codes = Vec::new();
    let mut formula_count = 0;
    let mut has_format_readback = false;
    for row in bounds.min_row()..=bounds.max_row() {
        let mut row_values = Vec::new();
        let mut row_types = Vec::new();
        let mut row_formulas = Vec::new();
        let mut row_style_indexes = Vec::new();
        let mut row_number_format_ids = Vec::new();
        let mut row_number_format_codes = Vec::new();
        for col in bounds.min_col()..=bounds.max_col() {
            let addr = format!("{}{}", col_name(col), row);
            if let Some(cell) = cells.get(&addr) {
                if cell.has_formula {
                    formula_count += 1;
                }
                row_values.push(cell.matrix_value.clone());
                row_types.push(Value::String(cell.kind.clone()));
                if cell.formula.is_empty() {
                    row_formulas.push(Value::Null);
                } else {
                    row_formulas.push(Value::String(cell.formula.clone()));
                }
                let style_index = cell.style_index.unwrap_or(0);
                let number_format_id = cell.number_format_id.unwrap_or(0);
                let number_format_code = cell.number_format_code.clone().unwrap_or_default();
                let has_cell_format =
                    style_index != 0 || number_format_id != 0 || !number_format_code.is_empty();
                if has_cell_format {
                    has_format_readback = true;
                    row_style_indexes.push(json!(style_index));
                    row_number_format_ids.push(json!(number_format_id));
                    if number_format_code.is_empty() {
                        row_number_format_codes.push(Value::Null);
                    } else {
                        row_number_format_codes.push(Value::String(number_format_code));
                    }
                } else {
                    row_style_indexes.push(Value::Null);
                    row_number_format_ids.push(Value::Null);
                    row_number_format_codes.push(Value::Null);
                }
            } else {
                row_values.push(Value::Null);
                row_types.push(Value::String("empty".to_string()));
                row_formulas.push(Value::Null);
                row_style_indexes.push(Value::Null);
                row_number_format_ids.push(Value::Null);
                row_number_format_codes.push(Value::Null);
            }
        }
        values.push(Value::Array(row_values));
        types.push(Value::Array(row_types));
        formulas.push(Value::Array(row_formulas));
        style_indexes.push(Value::Array(row_style_indexes));
        number_format_ids.push(Value::Array(row_number_format_ids));
        number_format_codes.push(Value::Array(row_number_format_codes));
    }
    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let mut output = Map::new();
    output.insert(
        "cellsExtractCommand".to_string(),
        json!(format!(
            "ooxml --json xlsx cells extract {} --sheet {} --range {}",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range)
        )),
    );
    output.insert("cols".to_string(), json!(cols));
    output.insert("dataFormat".to_string(), json!("json"));
    output.insert("file".to_string(), json!(file));
    output.insert("formulaCount".to_string(), json!(formula_count));
    output.insert("majorDimension".to_string(), json!("rows"));
    output.insert(
        "pptxPlaceTableCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json pptx place table-from-xlsx deck.pptx --workbook {} --sheet {} --range {} --expect-source-range {} --slide 1 --x 0 --y 0 --cx 4000000 --out out.pptx",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range),
            command_arg(range)
        )),
    );
    output.insert(
        "pptxReplaceTextCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json pptx replace text-from-xlsx deck.pptx --workbook {} --sheet {} --range {} --slide 1 --target title --out out.pptx",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range)
        )),
    );
    output.insert(
        "pptxUpdateTableCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json pptx tables update-from-xlsx deck.pptx --workbook {} --sheet {} --range {} --expect-source-range {} --slide 1 --target table:1 --out out.pptx",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range),
            command_arg(range)
        )),
    );
    output.insert("primarySelector".to_string(), json!(range));
    output.insert("range".to_string(), json!(range));
    output.insert("rows".to_string(), json!(rows));
    output.insert("selectors".to_string(), json!([range]));
    output.insert("sheet".to_string(), json!(sheet.name));
    output.insert("sheetNumber".to_string(), json!(sheet.position));
    output.insert("truncated".to_string(), json!(false));
    if options.include_types {
        output.insert("types".to_string(), Value::Array(types));
    }
    if options.include_formulas {
        output.insert("formulas".to_string(), Value::Array(formulas));
    }
    if options.include_formats && has_format_readback {
        output.insert("styleIndexes".to_string(), Value::Array(style_indexes));
        output.insert(
            "numberFormatIds".to_string(),
            Value::Array(number_format_ids),
        );
        output.insert(
            "numberFormatCodes".to_string(),
            Value::Array(number_format_codes),
        );
    }
    output.insert(
        "validateCommand".to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(file))),
    );
    output.insert("values".to_string(), Value::Array(values));
    if let Some(data_out) = options.data_out.filter(|data_out| !data_out.is_empty()) {
        output.insert("dataOut".to_string(), json!(data_out));
        let mut data = serde_json::to_vec(&Value::Object(output.clone()))
            .map_err(|err| CliError::unexpected(format!("failed to marshal range JSON: {err}")))?;
        data.push(b'\n');
        fs::write(data_out, data)
            .map_err(|err| CliError::unexpected(format!("failed to write --data-out: {err}")))?;
        output.remove("values");
        output.remove("types");
        output.remove("formulas");
        output.remove("styleIndexes");
        output.remove("numberFormatIds");
        output.remove("numberFormatCodes");
    }
    Ok(Value::Object(output))
}

pub(crate) fn require_json_data_format(data_format: Option<&str>) -> CliResult<()> {
    let data_format = data_format.unwrap_or("json").trim().to_ascii_lowercase();
    if data_format.is_empty() || data_format == "json" {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "unsupported Rust-port data format {data_format:?}; only json is implemented"
        )))
    }
}

pub(crate) fn normalize_xlsx_ranges_set_data_format(
    data_format: Option<&str>,
) -> CliResult<String> {
    let normalized = data_format.unwrap_or("json").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" | "json" => Ok("json".to_string()),
        "csv" => Ok("csv".to_string()),
        "tsv" => Ok("tsv".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "invalid data format {data_format:?} (must be json, csv, or tsv)",
        ))),
    }
}

pub(crate) fn check_range_max_cells(
    range: &str,
    bounds: RangeBounds,
    max_cells: i64,
) -> CliResult<()> {
    if max_cells < 0 {
        return Err(CliError::invalid_args("--max-cells must be >= 0"));
    }
    let rows = i64::from(bounds.row_count());
    let cols = i64::from(bounds.col_count());
    let cell_count = rows.saturating_mul(cols);
    if max_cells > 0 && cell_count > max_cells {
        return Err(CliError::invalid_args(format!(
            "range {range} contains {cell_count} cells, above --max-cells {max_cells}"
        )));
    }
    Ok(())
}
