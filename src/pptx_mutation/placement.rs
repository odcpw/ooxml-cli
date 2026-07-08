use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::cli_args::value_flag_present;
use crate::{
    CliError, CliResult, XlsxRangeExportOptions, add_relationship_to_xml, allocate_relationship_id,
    attr, check_range_max_cells, copy_zip_with_binary_part_overrides_and_removals,
    copy_zip_with_part_overrides, ensure_content_type_override, local_name,
    needs_xml_space_preserve, package_mutation_temp_path, package_type, parse_cli_range,
    parse_i64_flag, parse_range, parse_string_flag, pptx_slide_show, range_bounds_ref,
    relationship_entries_from_xml, relationship_target_from_source_to_target,
    relationships_part_for, select_xlsx_table, validate, validate_xlsx_mutation_output_flags,
    xlsx_range_export_with_options, xlsx_tables, xml_attr_escape, xml_direct_child_ranges,
    xml_escape, zip_entry_names, zip_text,
};

mod output;

use self::output::{
    add_textbox_result_json, place_image_result_json, place_table_from_xlsx_result_json,
    place_table_result_json, read_shape_destination, read_table_destination,
    table_from_xlsx_destination,
};

const REL_TYPE_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

#[derive(Clone)]
struct PlacementMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

#[derive(Clone, Copy)]
struct Bounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

#[derive(Clone, Copy)]
struct XmlSpan {
    start: usize,
    end: usize,
}

struct PlacementStage {
    staged_path: String,
    output_path: Option<String>,
}

struct TextboxRequest {
    slide: u32,
    text: String,
    bounds: Bounds,
    name: String,
    font_size: f64,
    font_family: String,
    bold: bool,
    italic: bool,
    color: String,
    level: i64,
    align: String,
}

struct ImageRequest {
    slide: u32,
    image_path: String,
    bounds: Bounds,
    name: String,
    fit_mode: String,
}

struct TableRequest {
    slide: u32,
    data: Vec<Vec<String>>,
    bounds: Bounds,
    name: String,
    has_header: bool,
    has_banded_rows: bool,
    header_color: String,
    band1_color: String,
    band2_color: String,
    font_size: i64,
    border_color: String,
    border_width: i64,
}

struct TableFromXlsxRequest {
    table: TableRequest,
    source: XlsxTableSource,
}

struct XlsxTableSource {
    source: Value,
    data: Vec<Vec<String>>,
    range: String,
}

struct TextboxMutation {
    slide: u32,
    slide_part: String,
    shape_id: u32,
    shape_name: String,
    updated_slide_xml: String,
}

struct ImageMutation {
    slide: u32,
    slide_part: String,
    shape_id: u32,
    shape_name: String,
    target_uri: String,
    content_type: String,
    relationship_id: String,
    fit_mode: String,
    updated_slide_xml: String,
    updated_rels_xml: String,
    updated_content_types_xml: String,
    image_data: Vec<u8>,
}

struct TableMutation {
    slide_part: String,
    shape_id: u32,
    shape_name: String,
    width: i64,
    height: i64,
    rows: usize,
    cols: usize,
    updated_slide_xml: String,
}

pub(crate) fn pptx_add_textbox(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let request = parse_add_textbox_request(args)?;
    let options = parse_placement_mutation_options(args)?;
    let mutation = build_textbox_mutation(file, &request)?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(
        mutation.slide_part.clone(),
        mutation.updated_slide_xml.clone(),
    );
    let stage = stage_placement_mutation(
        file,
        &text_overrides,
        &BTreeMap::new(),
        &options,
        "pptx-add-textbox",
    )?;
    let destination = read_shape_destination(
        &stage.staged_path,
        request.slide,
        mutation.shape_id,
        stage.output_path.as_deref(),
        true,
    )?;
    let result = add_textbox_result_json(
        file,
        &mutation,
        &options,
        stage.output_path.as_deref(),
        destination,
    );
    finish_placement_mutation(
        file,
        &stage.staged_path,
        &options,
        stage.output_path.as_deref(),
    )?;
    Ok(result)
}

pub(crate) fn pptx_place_image(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let request = parse_place_image_request(args)?;
    let options = parse_placement_mutation_options(args)?;
    let mutation = build_image_mutation(file, &request)?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(
        mutation.slide_part.clone(),
        mutation.updated_slide_xml.clone(),
    );
    text_overrides.insert(
        relationships_part_for(&mutation.slide_part),
        mutation.updated_rels_xml.clone(),
    );
    text_overrides.insert(
        "[Content_Types].xml".to_string(),
        mutation.updated_content_types_xml.clone(),
    );
    let mut binary_overrides = BTreeMap::new();
    binary_overrides.insert(
        mutation.target_uri.trim_start_matches('/').to_string(),
        mutation.image_data.clone(),
    );
    let stage = stage_placement_mutation(
        file,
        &text_overrides,
        &binary_overrides,
        &options,
        "pptx-place-image",
    )?;
    let destination = read_shape_destination(
        &stage.staged_path,
        request.slide,
        mutation.shape_id,
        stage.output_path.as_deref(),
        false,
    )?;
    let result = place_image_result_json(
        file,
        &mutation,
        &request,
        &options,
        stage.output_path.as_deref(),
        destination,
    );
    finish_placement_mutation(
        file,
        &stage.staged_path,
        &options,
        stage.output_path.as_deref(),
    )?;
    Ok(result)
}

pub(crate) fn pptx_place_table(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let request = parse_place_table_request(args)?;
    let options = parse_placement_mutation_options(args)?;
    let mutation = build_table_mutation(file, &request)?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(
        mutation.slide_part.clone(),
        mutation.updated_slide_xml.clone(),
    );
    let stage = stage_placement_mutation(
        file,
        &text_overrides,
        &BTreeMap::new(),
        &options,
        "pptx-place-table",
    )?;
    let result = place_table_result_json(&mutation);
    finish_placement_mutation(
        file,
        &stage.staged_path,
        &options,
        stage.output_path.as_deref(),
    )?;
    Ok(result)
}

pub(crate) fn pptx_place_table_from_xlsx(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let request = parse_place_table_from_xlsx_request(args)?;
    let options = parse_placement_mutation_options(args)?;
    let mutation = build_table_mutation(file, &request.table)?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(
        mutation.slide_part.clone(),
        mutation.updated_slide_xml.clone(),
    );
    let stage = stage_placement_mutation(
        file,
        &text_overrides,
        &BTreeMap::new(),
        &options,
        "pptx-place-table",
    )?;
    let table = read_table_destination(
        &stage.staged_path,
        request.table.slide,
        mutation.shape_id,
        stage.output_path.as_deref(),
    )?;
    let destination = table_from_xlsx_destination(
        &table,
        &request.table,
        &mutation,
        stage.output_path.as_deref(),
    );
    let result = place_table_from_xlsx_result_json(
        file,
        &request,
        &options,
        stage.output_path.as_deref(),
        destination,
    );
    finish_placement_mutation(
        file,
        &stage.staged_path,
        &options,
        stage.output_path.as_deref(),
    )?;
    Ok(result)
}

fn parse_add_textbox_request(args: &[String]) -> CliResult<TextboxRequest> {
    require_value_flags(args, &["--slide", "--text", "--cx", "--cy"])?;
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let text = parse_string_flag(args, "--text")?.unwrap_or_default();
    if text.is_empty() {
        return Err(CliError::invalid_args("--text is required"));
    }
    let bounds = parse_required_bounds(args, true)?;
    let font_size = parse_f64_flag(args, "--font-size")?.unwrap_or(18.0);
    let font_family = parse_string_flag(args, "--font")?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Calibri".to_string());
    Ok(TextboxRequest {
        slide: slide as u32,
        text,
        bounds,
        name: parse_string_flag(args, "--name")?.unwrap_or_default(),
        font_size,
        font_family,
        bold: crate::has_flag(args, "--bold"),
        italic: crate::has_flag(args, "--italic"),
        color: parse_string_flag(args, "--color")?.unwrap_or_default(),
        level: parse_i64_flag(args, "--level")?.unwrap_or(0),
        align: parse_string_flag(args, "--align")?.unwrap_or_default(),
    })
}

fn parse_place_image_request(args: &[String]) -> CliResult<ImageRequest> {
    require_value_flags(args, &["--slide", "--image", "--x", "--y", "--cx", "--cy"])?;
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let image_path = parse_string_flag(args, "--image")?.unwrap_or_default();
    if image_path.trim().is_empty() {
        return Err(CliError::invalid_args("--image must be specified"));
    }
    if !Path::new(&image_path).exists() {
        return Err(CliError::file_not_found(format!(
            "file not found: {image_path}"
        )));
    }
    let bounds = parse_required_bounds(args, false)?;
    let fit_mode = normalize_fit_mode(
        parse_string_flag(args, "--fit-mode")?
            .as_deref()
            .unwrap_or("contain"),
    )?;
    Ok(ImageRequest {
        slide: slide as u32,
        image_path,
        bounds,
        name: parse_string_flag(args, "--name")?.unwrap_or_default(),
        fit_mode,
    })
}

fn parse_place_table_request(args: &[String]) -> CliResult<TableRequest> {
    require_value_flags(args, &["--slide", "--data", "--cx"])?;
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let data_path = parse_string_flag(args, "--data")?.unwrap_or_default();
    if data_path.trim().is_empty() {
        return Err(CliError::invalid_args("--data must be specified"));
    }
    if !Path::new(&data_path).exists() {
        return Err(CliError::file_not_found(format!(
            "file not found: {data_path}"
        )));
    }
    let data_format = normalize_table_data_format(parse_string_flag(args, "--format")?.as_deref())?;
    let data = load_table_data(&data_path, &data_format)?;
    if data.is_empty() {
        return Err(CliError::invalid_args("table data is empty"));
    }
    parse_table_request_from_data(args, slide as u32, data)
}

fn parse_place_table_from_xlsx_request(args: &[String]) -> CliResult<TableFromXlsxRequest> {
    require_value_flags(args, &["--workbook", "--slide", "--cx"])?;
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let workbook = parse_string_flag(args, "--workbook")?.unwrap_or_default();
    if workbook.trim().is_empty() {
        return Err(CliError::invalid_args("--workbook is required"));
    }
    if !Path::new(&workbook).exists() {
        return Err(CliError::file_not_found(format!(
            "file not found: {workbook}"
        )));
    }
    let formula_mode =
        normalize_xlsx_formula_mode(parse_string_flag(args, "--formula-mode")?.as_deref())?;
    let max_cells = parse_i64_flag(args, "--max-cells")?.unwrap_or(100000);
    let source = load_place_table_from_xlsx_source(
        &workbook,
        parse_string_flag(args, "--sheet")?.as_deref(),
        parse_string_flag(args, "--range")?.as_deref(),
        parse_string_flag(args, "--table")?.as_deref(),
        max_cells,
        &formula_mode,
    )?;
    check_expected_xlsx_source_range(
        &source.range,
        parse_string_flag(args, "--expect-source-range")?.as_deref(),
    )?;
    if source.data.is_empty() || source.data.first().is_none_or(Vec::is_empty) {
        return Err(CliError::invalid_args("source range is empty"));
    }
    let table = parse_table_request_from_data(args, slide as u32, source.data.clone())?;
    Ok(TableFromXlsxRequest { table, source })
}

fn parse_table_request_from_data(
    args: &[String],
    slide: u32,
    data: Vec<Vec<String>>,
) -> CliResult<TableRequest> {
    let bounds = parse_required_table_bounds(args)?;
    Ok(TableRequest {
        slide,
        data,
        bounds,
        name: parse_string_flag(args, "--name")?.unwrap_or_default(),
        has_header: crate::has_flag(args, "--header"),
        has_banded_rows: crate::has_flag(args, "--banded-rows"),
        header_color: parse_string_flag(args, "--header-color")?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "4472C4".to_string()),
        band1_color: parse_string_flag(args, "--band1-color")?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "D9E1F2".to_string()),
        band2_color: parse_string_flag(args, "--band2-color")?.unwrap_or_default(),
        font_size: parse_i64_flag(args, "--font-size")?.unwrap_or(18),
        border_color: parse_string_flag(args, "--border-color")?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "000000".to_string()),
        border_width: parse_i64_flag(args, "--border-width")?.unwrap_or(19050),
    })
}

fn parse_required_bounds(args: &[String], textbox: bool) -> CliResult<Bounds> {
    let x = parse_i64_flag(args, "--x")?.unwrap_or(0);
    let y = parse_i64_flag(args, "--y")?.unwrap_or(0);
    let cx = parse_i64_flag(args, "--cx")?.unwrap_or(0);
    let cy = parse_i64_flag(args, "--cy")?.unwrap_or(0);
    if textbox {
        if cx <= 0 || cy <= 0 {
            return Err(CliError::invalid_args("--cx and --cy must be positive"));
        }
    } else if cx <= 0 || cy <= 0 {
        return Err(CliError::invalid_args(format!(
            "dimensions must be positive: cx={cx}, cy={cy}"
        )));
    }
    Ok(Bounds { x, y, cx, cy })
}

fn parse_required_table_bounds(args: &[String]) -> CliResult<Bounds> {
    let x = parse_i64_flag(args, "--x")?.unwrap_or(0);
    let y = parse_i64_flag(args, "--y")?.unwrap_or(0);
    let cx = parse_i64_flag(args, "--cx")?.unwrap_or(0);
    let cy = parse_i64_flag(args, "--cy")?.unwrap_or(0);
    if cx <= 0 {
        return Err(CliError::invalid_args(format!(
            "table width must be positive: cx={cx}"
        )));
    }
    Ok(Bounds { x, y, cx, cy })
}

fn require_value_flags(args: &[String], flags: &[&str]) -> CliResult<()> {
    let missing = flags
        .iter()
        .filter(|flag| !value_flag_present(args, flag))
        .map(|flag| format!(r#""{}""#, flag.trim_start_matches("--")))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "required flag(s) {} not set",
            missing.join(", ")
        )))
    }
}

fn parse_f64_flag(args: &[String], name: &str) -> CliResult<Option<f64>> {
    parse_string_flag(args, name)?
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| CliError::invalid_args(format!("{name} must be a number")))
        })
        .transpose()
}

fn normalize_fit_mode(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "contain" | "fit" => Ok("contain".to_string()),
        "cover" | "crop" => Ok("cover".to_string()),
        other => Err(CliError::invalid_args(format!(
            "invalid fit mode {other:?} (must be 'contain' or 'cover')"
        ))),
    }
}

fn normalize_table_data_format(value: Option<&str>) -> CliResult<String> {
    match value.unwrap_or("csv").trim().to_ascii_lowercase().as_str() {
        "csv" => Ok("csv".to_string()),
        "json" => Ok("json".to_string()),
        _ => Err(CliError::invalid_args("--format must be 'csv' or 'json'")),
    }
}

fn normalize_xlsx_formula_mode(value: Option<&str>) -> CliResult<String> {
    match value
        .unwrap_or("value")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "value" => Ok("value".to_string()),
        "formula" => Ok("formula".to_string()),
        _ => Err(CliError::invalid_args(
            "--formula-mode must be value or formula",
        )),
    }
}

fn load_table_data(path: &str, data_format: &str) -> CliResult<Vec<Vec<String>>> {
    let data = fs::read_to_string(path)
        .map_err(|err| CliError::unexpected(format!("failed to open data file: {err}")))?;
    match data_format {
        "csv" => parse_csv_table_data(&data),
        "json" => parse_json_table_data(&data),
        other => Err(CliError::unexpected(format!("unsupported format: {other}"))),
    }
}

fn parse_json_table_data(data: &str) -> CliResult<Vec<Vec<String>>> {
    let value: Value = serde_json::from_str(data)
        .map_err(|err| CliError::unexpected(format!("failed to decode JSON: {err}")))?;
    let rows = value
        .as_array()
        .ok_or_else(|| CliError::unexpected("failed to decode JSON: expected array of arrays"))?;
    rows.iter()
        .map(|row| {
            let row = row.as_array().ok_or_else(|| {
                CliError::unexpected("failed to decode JSON: expected array of arrays")
            })?;
            Ok(row.iter().map(json_cell_to_table_text).collect())
        })
        .collect()
}

fn json_cell_to_table_text(value: &Value) -> String {
    match value {
        Value::Null => "<nil>".to_string(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        other => other.to_string(),
    }
}

fn parse_csv_table_data(data: &str) -> CliResult<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = data.chars().peekable();
    let mut in_quotes = false;
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
        match ch {
            '"' if field.is_empty() && !just_closed_quote => in_quotes = true,
            ',' => {
                row.push(std::mem::take(&mut field));
                just_closed_quote = false;
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                just_closed_quote = false;
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                just_closed_quote = false;
            }
            _ if just_closed_quote && !ch.is_whitespace() => {
                return Err(CliError::unexpected(
                    "failed to read CSV: extraneous or missing \" in quoted-field",
                ));
            }
            _ => {
                field.push(ch);
                just_closed_quote = false;
            }
        }
    }
    if in_quotes {
        return Err(CliError::unexpected(
            "failed to read CSV: extraneous or missing \" in quoted-field",
        ));
    }
    if !field.is_empty() || !row.is_empty() || data.ends_with(',') {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

fn load_place_table_from_xlsx_source(
    workbook: &str,
    sheet: Option<&str>,
    range: Option<&str>,
    table: Option<&str>,
    max_cells: i64,
    formula_mode: &str,
) -> CliResult<XlsxTableSource> {
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
    let data = xlsx_strings_from_export(values, formulas, formula_mode)?;
    let rows = export_object
        .get("rows")
        .and_then(Value::as_u64)
        .unwrap_or(data.len() as u64);
    let cols = export_object
        .get("cols")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| data.first().map(Vec::len).unwrap_or_default() as u64);
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
    source.insert("range".to_string(), json!(source_range.clone()));
    if !source_table.is_empty() {
        source.insert("table".to_string(), json!(source_table));
    }
    source.insert("rows".to_string(), json!(rows));
    source.insert("cols".to_string(), json!(cols));
    source.insert("formulaCount".to_string(), json!(formula_count));
    Ok(XlsxTableSource {
        source: Value::Object(source),
        data,
        range: source_range,
    })
}

fn xlsx_strings_from_export(
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

fn parse_placement_mutation_options(args: &[String]) -> CliResult<PlacementMutationOptions> {
    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PlacementMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn build_textbox_mutation(file: &str, request: &TextboxRequest) -> CliResult<TextboxMutation> {
    let slide_part = slide_part_for_number(file, request.slide)?;
    let slide_xml = zip_text(file, &slide_part)?;
    let shape_id = next_shape_id(&slide_xml)?;
    let shape_name = if request.name.is_empty() {
        format!("TextBox {shape_id}")
    } else {
        request.name.clone()
    };
    let shape_xml = textbox_shape_xml(shape_id, &shape_name, request);
    let updated_slide_xml = insert_shape_into_sp_tree(&slide_xml, &shape_xml)?;
    Ok(TextboxMutation {
        slide: request.slide,
        slide_part,
        shape_id,
        shape_name,
        updated_slide_xml,
    })
}

fn build_image_mutation(file: &str, request: &ImageRequest) -> CliResult<ImageMutation> {
    let slide_part = slide_part_for_number(file, request.slide)?;
    let slide_xml = zip_text(file, &slide_part)?;
    let shape_id = next_shape_id(&slide_xml)?;
    let image_data = fs::read(&request.image_path)
        .map_err(|err| CliError::unexpected(format!("failed to read image file: {err}")))?;
    let content_type = image_content_type(&request.image_path)?;
    validate_image_payload(&content_type, &image_data)?;
    let extension = image_extension_for_content_type(&content_type)?;
    let target_uri = allocate_image_part(file, shape_id, extension)?;
    let rels_part = relationships_part_for(&slide_part);
    let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| {
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#
            .to_string()
    });
    let rels = relationship_entries_from_xml(&rels_xml);
    let relationship_id = allocate_relationship_id(&rels);
    let rel_target =
        relationship_target_from_source_to_target(&format!("/{slide_part}"), &target_uri);
    let updated_rels_xml =
        add_relationship_to_xml(rels_xml, &relationship_id, REL_TYPE_IMAGE, &rel_target);
    let content_types = zip_text(file, "[Content_Types].xml")?;
    let updated_content_types_xml =
        ensure_content_type_override(content_types, &target_uri, &content_type)?;
    let shape_name = if request.name.is_empty() {
        Path::new(&target_uri)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("image")
            .to_string()
    } else {
        request.name.clone()
    };
    let pic_xml = picture_shape_xml(shape_id, &shape_name, &relationship_id, request);
    let slide_xml = ensure_root_namespace(slide_xml, "r", R_NS)?;
    let updated_slide_xml = insert_shape_into_sp_tree(&slide_xml, &pic_xml)?;
    Ok(ImageMutation {
        slide: request.slide,
        slide_part,
        shape_id,
        shape_name,
        target_uri,
        content_type,
        relationship_id,
        fit_mode: request.fit_mode.clone(),
        updated_slide_xml,
        updated_rels_xml,
        updated_content_types_xml,
        image_data,
    })
}

fn build_table_mutation(file: &str, request: &TableRequest) -> CliResult<TableMutation> {
    let slide_part = slide_part_for_number(file, request.slide)?;
    let slide_xml = zip_text(file, &slide_part)?;
    let shape_id = next_shape_id(&slide_xml)?;
    let shape_name = if request.name.is_empty() {
        format!("Table {shape_id}")
    } else {
        request.name.clone()
    };
    let (table_xml, width, height, rows, cols) = table_xml_from_data(request)?;
    let frame_xml =
        table_graphic_frame_xml(shape_id, &shape_name, &table_xml, request, width, height);
    let slide_xml = ensure_root_namespace(slide_xml, "a", A_NS)?;
    let updated_slide_xml = insert_shape_into_sp_tree(&slide_xml, &frame_xml)?;
    Ok(TableMutation {
        slide_part,
        shape_id,
        shape_name,
        width,
        height,
        rows,
        cols,
        updated_slide_xml,
    })
}

fn slide_part_for_number(file: &str, slide: u32) -> CliResult<String> {
    let show = pptx_slide_show(file, slide)?;
    show.get("slides")
        .and_then(Value::as_array)
        .and_then(|slides| slides.first())
        .and_then(|entry| entry.get("partUri"))
        .and_then(Value::as_str)
        .map(|part| part.trim_start_matches('/').to_string())
        .ok_or_else(|| CliError::unexpected("slide readback missing partUri"))
}

fn next_shape_id(slide_xml: &str) -> CliResult<u32> {
    let sp_tree = find_first_element_span(slide_xml, "spTree")?
        .ok_or_else(|| CliError::unexpected("shape tree not found in slide"))?;
    let fragment = &slide_xml[sp_tree.start..sp_tree.end];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    let mut max_id = 0_u32;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cNvPr" =>
            {
                if let Some(id) = attr(&e, "id").and_then(|value| value.trim().parse::<u32>().ok())
                {
                    max_id = max_id.max(id);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(max_id + 1)
}

fn insert_shape_into_sp_tree(slide_xml: &str, shape_xml: &str) -> CliResult<String> {
    let sp_tree = find_first_element_span(slide_xml, "spTree")?
        .ok_or_else(|| CliError::unexpected("shape tree not found in slide"))?;
    let (content_start, content_end) =
        element_content_bounds(&slide_xml[sp_tree.start..sp_tree.end])?;
    let children = xml_direct_child_ranges(
        slide_xml,
        sp_tree.start + content_start,
        sp_tree.start + content_end,
    )?;
    let insert_at = children
        .iter()
        .find(|child| child.kind == "extLst")
        .map(|child| child.start)
        .unwrap_or(sp_tree.start + content_end);
    Ok(insert_xml_at(slide_xml, insert_at, shape_xml))
}

fn textbox_shape_xml(shape_id: u32, shape_name: &str, request: &TextboxRequest) -> String {
    let mut paragraph = String::new();
    paragraph.push_str("<a:p>");
    if request.level > 0 || !request.align.is_empty() {
        paragraph.push_str("<a:pPr");
        if !request.align.is_empty() {
            paragraph.push_str(&format!(r#" algn="{}""#, xml_attr_escape(&request.align)));
        }
        if request.level > 0 {
            paragraph.push_str(&format!(r#" lvl="{}""#, request.level));
        }
        paragraph.push_str("/>");
    }
    paragraph.push_str("<a:r>");
    paragraph.push_str(&run_properties_xml(
        request.font_size,
        &request.font_family,
        request.bold,
        request.italic,
        &request.color,
    ));
    paragraph.push_str(&text_element_xml(&request.text));
    paragraph.push_str("</a:r><a:endParaRPr lang=\"en-US\" sz=\"1800\"/></a:p>");
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{shape_id}" name="{}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr anchor="t" anchorCtr="false" wrap="square" rtlCol="false"/><a:lstStyle/>{paragraph}</p:txBody></p:sp>"#,
        xml_attr_escape(shape_name),
        request.bounds.x,
        request.bounds.y,
        request.bounds.cx,
        request.bounds.cy
    )
}

fn run_properties_xml(
    font_size: f64,
    font_family: &str,
    bold: bool,
    italic: bool,
    color: &str,
) -> String {
    let size = (font_size * 100.0) as i64;
    let mut xml = format!(r#"<a:rPr lang="en-US" sz="{size}""#);
    if bold {
        xml.push_str(r#" b="1""#);
    }
    if italic {
        xml.push_str(r#" i="1""#);
    }
    xml.push('>');
    if !color.is_empty() {
        xml.push_str(&format!(
            r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
            xml_attr_escape(color)
        ));
    }
    if !font_family.is_empty() {
        xml.push_str(&format!(
            r#"<a:latin typeface="{}"/>"#,
            xml_attr_escape(font_family)
        ));
    }
    xml.push_str("</a:rPr>");
    xml
}

fn text_element_xml(text: &str) -> String {
    if needs_xml_space_preserve(text) {
        format!(r#"<a:t xml:space="preserve">{}</a:t>"#, xml_escape(text))
    } else {
        format!("<a:t>{}</a:t>", xml_escape(text))
    }
}

fn picture_shape_xml(
    shape_id: u32,
    shape_name: &str,
    rel_id: &str,
    request: &ImageRequest,
) -> String {
    let fit = if request.fit_mode == "cover" {
        r#"<a:tile tx="0" ty="0" sx="100000" sy="100000" flip="none" algn="ctr"/>"#
    } else {
        "<a:stretch><a:fillRect/></a:stretch>"
    };
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{shape_id}" name="{}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{}"/>{fit}</p:blipFill><p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
        xml_attr_escape(shape_name),
        xml_attr_escape(rel_id),
        request.bounds.x,
        request.bounds.y,
        request.bounds.cx,
        request.bounds.cy
    )
}

fn table_graphic_frame_xml(
    shape_id: u32,
    shape_name: &str,
    table_xml: &str,
    request: &TableRequest,
    width: i64,
    height: i64,
) -> String {
    format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{shape_id}" name="{}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="{}" y="{}"/><a:ext cx="{width}" cy="{height}"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table">{table_xml}</a:graphicData></a:graphic></p:graphicFrame>"#,
        xml_attr_escape(shape_name),
        request.bounds.x,
        request.bounds.y,
    )
}

fn table_xml_from_data(request: &TableRequest) -> CliResult<(String, i64, i64, usize, usize)> {
    if request.data.is_empty() {
        return Err(CliError::unexpected(
            "failed to create table data: table data cannot be empty",
        ));
    }
    let cols = request.data[0].len();
    if cols == 0 {
        return Err(CliError::unexpected(
            "failed to create table data: first row must contain at least one column",
        ));
    }
    for (index, row) in request.data.iter().enumerate() {
        if row.len() != cols {
            return Err(CliError::unexpected(format!(
                "failed to create table data: row {index} has {} columns, expected {cols}",
                row.len()
            )));
        }
    }
    let rows = request.data.len();
    let font_size = if request.font_size <= 0 {
        18
    } else {
        request.font_size
    };
    let border_color = if request.border_color.is_empty() {
        "000000"
    } else {
        request.border_color.as_str()
    };
    let border_width = if request.border_width <= 0 {
        19050
    } else {
        request.border_width
    };
    let col_width = request.bounds.cx / cols as i64;
    let row_height = if request.bounds.cy > 0 {
        request.bounds.cy / rows as i64
    } else {
        457200
    };
    let height = row_height * rows as i64;
    let mut xml = String::new();
    xml.push_str(r#"<a:tbl xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">"#);
    xml.push_str("<a:tblPr/>");
    xml.push_str("<a:tblGrid>");
    for _ in 0..cols {
        xml.push_str(&format!(r#"<a:gridCol w="{col_width}"/>"#));
    }
    xml.push_str("</a:tblGrid>");
    for (row_index, row) in request.data.iter().enumerate() {
        let is_header = request.has_header && row_index == 0;
        let is_band = request.has_banded_rows && !is_header;
        let band_index = if is_band && row_index > 0 {
            (row_index - 1) % 2
        } else {
            0
        };
        xml.push_str(&format!(r#"<a:tr h="{row_height}">"#));
        for cell in row {
            xml.push_str("<a:tc>");
            xml.push_str(&table_cell_text_body_xml(cell, font_size, is_header));
            xml.push_str(&table_cell_properties_xml(
                request,
                is_header,
                is_band,
                band_index,
                border_color,
                border_width,
            ));
            xml.push_str("</a:tc>");
        }
        xml.push_str("</a:tr>");
    }
    xml.push_str("</a:tbl>");
    Ok((xml, request.bounds.cx, height, rows, cols))
}

fn table_cell_properties_xml(
    request: &TableRequest,
    is_header: bool,
    is_band: bool,
    band_index: usize,
    border_color: &str,
    border_width: i64,
) -> String {
    let mut xml = String::from("<a:tcPr>");
    let fill = if is_header && !request.header_color.is_empty() {
        Some(request.header_color.as_str())
    } else if is_band && band_index == 0 && !request.band1_color.is_empty() {
        Some(request.band1_color.as_str())
    } else if is_band && band_index == 1 && !request.band2_color.is_empty() {
        Some(request.band2_color.as_str())
    } else {
        None
    };
    for side in ["lnL", "lnR", "lnT", "lnB"] {
        xml.push_str(&format!(
            r#"<a:{side} w="{border_width}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:{side}>"#,
            xml_attr_escape(border_color)
        ));
    }
    if let Some(fill) = fill {
        xml.push_str(&format!(
            r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
            xml_attr_escape(fill)
        ));
    }
    xml.push_str("</a:tcPr>");
    xml
}

fn table_cell_text_body_xml(text: &str, font_size: i64, is_header: bool) -> String {
    let size = font_size * 100;
    let bold = if is_header { r#" b="1""# } else { "" };
    format!(
        r#"<a:txBody><a:bodyPr wrap="none" rtlCol="0"/><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="en-US" sz="{size}"{bold}/>{}</a:r><a:endParaRPr lang="en-US" sz="{size}"/></a:p></a:txBody>"#,
        text_element_xml(text)
    )
}

fn image_content_type(path: &str) -> CliResult<String> {
    let ext = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => Ok("image/png".to_string()),
        "jpg" | "jpeg" => Ok("image/jpeg".to_string()),
        "gif" => Ok("image/gif".to_string()),
        "bmp" => Ok("image/bmp".to_string()),
        "tif" | "tiff" => Ok("image/tiff".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "unsupported image content type for {path:?}"
        ))),
    }
}

fn image_extension_for_content_type(content_type: &str) -> CliResult<&'static str> {
    match content_type {
        "image/png" => Ok(".png"),
        "image/jpeg" => Ok(".jpeg"),
        "image/gif" => Ok(".gif"),
        "image/bmp" => Ok(".bmp"),
        "image/tiff" => Ok(".tiff"),
        _ => Err(CliError::invalid_args(format!(
            "unsupported image content type {content_type:?}"
        ))),
    }
}

fn validate_image_payload(content_type: &str, data: &[u8]) -> CliResult<()> {
    let ok = match content_type {
        "image/png" => data.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => data.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a"),
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "image payload does not match content type {content_type}"
        )))
    }
}

fn allocate_image_part(file: &str, shape_id: u32, extension: &str) -> CliResult<String> {
    let entries = zip_entry_names(file)?;
    let base = format!("/ppt/media/image{shape_id}");
    let mut candidate = format!("{base}{extension}");
    let mut counter = 1_u32;
    while entries
        .iter()
        .any(|entry| format!("/{}", entry.trim_start_matches('/')) == candidate)
    {
        candidate = format!("{base}_{counter}{extension}");
        counter += 1;
    }
    Ok(candidate)
}

fn ensure_root_namespace(xml: String, prefix: &str, uri: &str) -> CliResult<String> {
    if xml.contains(&format!("xmlns:{prefix}=")) {
        return Ok(xml);
    }
    let root = find_first_element_span(&xml, "sld")?
        .ok_or_else(|| CliError::unexpected("slide root not found"))?;
    let open_end = xml[root.start..root.end]
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid slide root XML"))?
        + root.start;
    let insert = format!(r#" xmlns:{prefix}="{}""#, xml_attr_escape(uri));
    Ok(insert_xml_at(&xml, open_end, &insert))
}

fn stage_placement_mutation(
    file: &str,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    options: &PlacementMutationOptions,
    label: &str,
) -> CliResult<PlacementStage> {
    let output_path = placement_output_path(file, options);
    let requested_out = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let staged_path = if options.dry_run || options.in_place || requested_out == Some(file) {
        package_mutation_temp_path(file, label)
    } else {
        requested_out
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    if binary_overrides.is_empty() {
        copy_zip_with_part_overrides(file, &staged_path, text_overrides)?;
    } else {
        copy_zip_with_binary_part_overrides_and_removals(
            file,
            &staged_path,
            text_overrides,
            binary_overrides,
            &BTreeSet::new(),
        )?;
    }
    if !options.no_validate {
        validate(&staged_path, true)?;
    }
    Ok(PlacementStage {
        staged_path,
        output_path,
    })
}

fn finish_placement_mutation(
    file: &str,
    staged_path: &str,
    options: &PlacementMutationOptions,
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

fn placement_output_path(file: &str, options: &PlacementMutationOptions) -> Option<String> {
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

fn ensure_pptx(file: &str) -> CliResult<()> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    Ok(())
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

fn insert_xml_at(xml: &str, index: usize, insert: &str) -> String {
    let mut out = String::with_capacity(xml.len() + insert.len());
    out.push_str(&xml[..index]);
    out.push_str(insert);
    out.push_str(&xml[index..]);
    out
}
