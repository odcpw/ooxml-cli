use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::cli_args::value_flag_present;
use crate::{
    CliError, CliResult, RelationshipEntry, XlsxRangeExportOptions, attr, attr_exact,
    check_range_max_cells, command_arg, content_type_for_part,
    copy_zip_with_binary_part_overrides_and_removals, copy_zip_with_part_overrides,
    decode_xml_text, ensure_content_type_override, local_name, needs_xml_space_preserve,
    package_mutation_temp_path, package_type, parse_cli_range, parse_range, range_bounds_ref,
    relationship_entries_from_xml, relationship_target_from_source_to_target,
    relationships_part_for, replace_xml_span, resolve_relationship_target, select_xlsx_table,
    validate, validate_xlsx_mutation_output_flags, xlsx_range_export_with_options, xlsx_tables,
    xml_attr_escape, xml_direct_child_ranges, xml_escape, zip_text,
};

#[derive(Clone)]
struct PptxReplaceMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

pub(crate) fn pptx_replace_text_occurrences(file: &str, args: &[String]) -> CliResult<Value> {
    let options = parse_replace_mutation_options(args)?;
    let match_text = crate::parse_string_flag(args, "--match-text")?.unwrap_or_default();
    if !value_flag_present(args, "--match-text") || match_text.is_empty() {
        return Err(CliError::invalid_args(
            "--match-text must be specified and cannot be empty",
        ));
    }
    let new_text = resolve_text_occurrences_new_text(args)?;
    let for_slides = crate::parse_string_flag(args, "--for-slides")?.unwrap_or_default();
    let for_shape = crate::parse_string_flag(args, "--for-shape")?.unwrap_or_default();
    if !for_shape.trim().is_empty() && value_flag_present(args, "--for-slides") {
        return Err(CliError::invalid_args(
            "--for-shape and --for-slides are mutually exclusive; --for-shape already encodes its slide scope",
        ));
    }
    let expect_count = if value_flag_present(args, "--expect-count") {
        let count = crate::parse_i64_flag(args, "--expect-count")?.unwrap_or(0);
        if count < 0 {
            return Err(CliError::invalid_args("--expect-count must be >= 0"));
        }
        Some(count as usize)
    } else {
        None
    };
    let expect_plan_hash = crate::parse_string_flag(args, "--expect-plan-hash")?
        .unwrap_or_default()
        .trim()
        .to_string();
    let request = TextOccurrencesRequest {
        match_text,
        new_text,
        for_slides,
        for_shape,
        ignore_case: crate::has_flag(args, "--ignore-case"),
        expect_count,
        expect_plan_hash,
        allow_zero: crate::has_flag(args, "--allow-zero"),
    };
    replace_text_occurrences(file, request, options)
}

pub(crate) fn pptx_replace_images(file: &str, args: &[String]) -> CliResult<Value> {
    let options = parse_replace_mutation_options(args)?;
    let target = crate::parse_string_flag(args, "--target")?.unwrap_or_default();
    if target.is_empty() {
        return Err(CliError::invalid_args("--target must be specified"));
    }
    let image_file = crate::parse_string_flag(args, "--image")?.unwrap_or_default();
    if image_file.is_empty() {
        return Err(CliError::invalid_args("--image must be specified"));
    }
    let image_data = fs::read(&image_file)
        .map_err(|_| CliError::file_not_found(format!("file not found: {image_file}")))?;
    let new_content_type = image_content_type_from_path(&image_file)?;
    validate_image_payload(&image_data, &new_content_type)
        .map_err(|message| CliError::unexpected(format!("failed to replace image: {message}")))?;
    let fit_mode = normalize_fit_mode(
        crate::parse_string_flag(args, "--fit-mode")?
            .unwrap_or_else(|| "contain".to_string())
            .as_str(),
    )?;
    if value_flag_present(args, "--slide") && value_flag_present(args, "--for-slides") {
        return Err(CliError::invalid_args(
            "cannot specify both --slide and --for-slides",
        ));
    }
    if target.starts_with("H:pptx/")
        && (value_flag_present(args, "--slide") || value_flag_present(args, "--for-slides"))
    {
        return Err(CliError::invalid_args(
            "--slide / --for-slides cannot be combined with a handle target",
        ));
    }
    let for_slides = crate::parse_string_flag(args, "--for-slides")?.unwrap_or_default();
    if value_flag_present(args, "--for-slides") {
        return replace_images_for_slides(
            file,
            &target,
            &for_slides,
            &image_data,
            &new_content_type,
            &fit_mode,
            options,
        );
    }
    let slide = if value_flag_present(args, "--slide") {
        let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
        if slide < 1 {
            return Err(CliError::invalid_args("--slide must be >= 1"));
        }
        Some(slide as u32)
    } else {
        None
    };
    replace_image(
        file,
        &target,
        slide,
        &image_data,
        &new_content_type,
        &fit_mode,
        options,
    )
}

pub(crate) fn pptx_replace_text_from_xlsx(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let target = crate::parse_string_flag(args, "--target")?.unwrap_or_default();
    if target.trim().is_empty() {
        return Err(CliError::invalid_args("--target is required"));
    }
    let workbook = crate::parse_string_flag(args, "--workbook")?.unwrap_or_default();
    if workbook.trim().is_empty() {
        return Err(CliError::invalid_args("--workbook is required"));
    }
    if !Path::new(&workbook).exists() {
        return Err(CliError::file_not_found(format!(
            "file not found: {workbook}"
        )));
    }
    let sheet = crate::parse_string_flag(args, "--sheet")?.unwrap_or_default();
    if sheet.trim().is_empty() {
        return Err(CliError::invalid_args("--sheet is required"));
    }
    let range = crate::parse_string_flag(args, "--range")?.unwrap_or_default();
    if range.trim().is_empty() {
        return Err(CliError::invalid_args("--range is required"));
    }
    let max_cells = crate::parse_i64_flag(args, "--max-cells")?.unwrap_or(100000);
    let formula_mode = normalize_xlsx_formula_mode(
        crate::parse_string_flag(args, "--formula-mode")?.as_deref(),
        "--formula-mode",
    )?;
    let mode = normalize_replace_text_from_xlsx_mode(
        crate::parse_string_flag(args, "--mode")?.as_deref(),
    )?;
    let row_sep = decode_text_separator_flag(
        crate::parse_string_flag(args, "--row-sep")?
            .unwrap_or_else(|| "\n".to_string())
            .as_str(),
        "--row-sep",
    )?;
    let col_sep = decode_text_separator_flag(
        crate::parse_string_flag(args, "--col-sep")?
            .unwrap_or_else(|| "\t".to_string())
            .as_str(),
        "--col-sep",
    )?;
    let source = load_xlsx_text_range_source(&workbook, &sheet, &range, max_cells, &formula_mode)?;
    let text = join_xlsx_text_matrix(&source.data, &row_sep, &col_sep);
    let options = parse_replace_mutation_options(args)?;
    replace_text_from_xlsx(
        file,
        ReplaceTextFromXlsxRequest {
            slide: slide as u32,
            target,
            source,
            text,
            mode,
            formula_mode,
            row_sep,
            col_sep,
        },
        options,
    )
}

pub(crate) fn pptx_replace_text_map_from_xlsx(file: &str, args: &[String]) -> CliResult<Value> {
    let workbook = crate::parse_string_flag(args, "--workbook")?.unwrap_or_default();
    if workbook.trim().is_empty() {
        return Err(CliError::invalid_args("--workbook is required"));
    }
    if !Path::new(&workbook).exists() {
        return Err(CliError::file_not_found(format!(
            "file not found: {workbook}"
        )));
    }
    let max_cells = crate::parse_i64_flag(args, "--max-cells")?.unwrap_or(100000);
    let formula_mode = normalize_xlsx_formula_mode(
        crate::parse_string_flag(args, "--formula-mode")?.as_deref(),
        "--formula-mode",
    )?;
    let mode = normalize_replace_text_from_xlsx_mode(
        crate::parse_string_flag(args, "--mode")?.as_deref(),
    )?;
    let source = load_xlsx_text_range_or_table_source(
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
    let (records, columns) = text_map_records_from_values(
        &source.data,
        crate::parse_string_flag(args, "--slide-col")?
            .unwrap_or_else(|| "slide".to_string())
            .as_str(),
        crate::parse_string_flag(args, "--target-col")?
            .unwrap_or_else(|| "target".to_string())
            .as_str(),
        crate::parse_string_flag(args, "--text-col")?
            .unwrap_or_else(|| "text".to_string())
            .as_str(),
    )?;
    let options = parse_replace_mutation_options(args)?;
    replace_text_map_from_xlsx(
        file,
        ReplaceTextMapFromXlsxRequest {
            source,
            records,
            columns,
            mode,
            formula_mode,
        },
        options,
    )
}

fn parse_replace_mutation_options(args: &[String]) -> CliResult<PptxReplaceMutationOptions> {
    let out = crate::parse_string_flag(args, "--out")?;
    let backup = crate::parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxReplaceMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

struct ReplaceTextFromXlsxRequest {
    slide: u32,
    target: String,
    source: XlsxTextSource,
    text: String,
    mode: String,
    formula_mode: String,
    row_sep: String,
    col_sep: String,
}

struct ReplaceTextMapFromXlsxRequest {
    source: XlsxTextSource,
    records: Vec<TextMapRecord>,
    columns: TextMapColumns,
    mode: String,
    formula_mode: String,
}

struct XlsxTextSource {
    source: Value,
    data: Vec<Vec<String>>,
    rows: usize,
    range: String,
}

#[derive(Clone)]
struct TextTargetReplacePlan {
    slide: u32,
    slide_part: String,
    slide_xml: String,
    target: ShapeTarget,
    text: String,
}

#[derive(Clone)]
struct TextMapRecord {
    source_row: usize,
    slide: u32,
    target: String,
    text: String,
}

struct TextMapApplied {
    record: TextMapRecord,
    plan: TextTargetReplacePlan,
}

struct TextMapColumns {
    slide: String,
    target: String,
    text: String,
}

fn replace_text_from_xlsx(
    file: &str,
    request: ReplaceTextFromXlsxRequest,
    options: PptxReplaceMutationOptions,
) -> CliResult<Value> {
    ensure_pptx(file)?;
    let slides = pptx_slide_refs_for_replace(file)?;
    let plan = build_text_target_replace_plan(
        file,
        &slides,
        request.slide,
        &request.target,
        &request.text,
    )
    .map_err(|err| map_text_target_error(err, &request.target))?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(plan.slide_part.clone(), plan.slide_xml.clone());
    write_replace_mutation(file, &text_overrides, &BTreeMap::new(), &options)?;
    Ok(text_from_xlsx_result_json(file, &request, &plan, &options))
}

fn replace_text_map_from_xlsx(
    file: &str,
    request: ReplaceTextMapFromXlsxRequest,
    options: PptxReplaceMutationOptions,
) -> CliResult<Value> {
    ensure_pptx(file)?;
    let slides = pptx_slide_refs_for_replace(file)?;
    let mut slide_xml_by_number = BTreeMap::<u32, String>::new();
    let mut overrides = BTreeMap::<String, String>::new();
    let mut applied = Vec::<TextMapApplied>::new();
    for record in &request.records {
        if record.slide as usize > slides.len() {
            return Err(CliError::invalid_args(format!(
                "row {}: slide {} out of range (1-{})",
                record.source_row,
                record.slide,
                slides.len()
            )));
        }
        let slide_ref = slides
            .get(record.slide as usize - 1)
            .ok_or_else(|| CliError::unexpected(format!("slide {} not found", record.slide)))?;
        if let std::collections::btree_map::Entry::Vacant(entry) =
            slide_xml_by_number.entry(record.slide)
        {
            entry.insert(zip_text(file, &slide_ref.part)?);
        }
        let current_xml = slide_xml_by_number
            .get(&record.slide)
            .ok_or_else(|| CliError::unexpected("slide XML cache missing"))?
            .clone();
        let plan = build_text_target_replace_plan_from_xml(
            &current_xml,
            slide_ref,
            &record.target,
            &record.text,
        )
        .map_err(|err| map_text_map_target_error(err, record))?;
        slide_xml_by_number.insert(record.slide, plan.slide_xml.clone());
        overrides.insert(plan.slide_part.clone(), plan.slide_xml.clone());
        applied.push(TextMapApplied {
            record: record.clone(),
            plan,
        });
    }
    write_replace_mutation(file, &overrides, &BTreeMap::new(), &options)?;
    Ok(text_map_from_xlsx_result_json(
        file, &request, &applied, &options,
    ))
}

fn load_xlsx_text_range_source(
    workbook: &str,
    sheet: &str,
    range: &str,
    max_cells: i64,
    formula_mode: &str,
) -> CliResult<XlsxTextSource> {
    load_xlsx_text_source(
        workbook,
        Some(sheet),
        Some(range),
        None,
        max_cells,
        formula_mode,
    )
}

fn load_xlsx_text_range_or_table_source(
    workbook: &str,
    sheet: Option<&str>,
    range: Option<&str>,
    table: Option<&str>,
    max_cells: i64,
    formula_mode: &str,
) -> CliResult<XlsxTextSource> {
    load_xlsx_text_source(workbook, sheet, range, table, max_cells, formula_mode)
}

fn load_xlsx_text_source(
    workbook: &str,
    sheet: Option<&str>,
    range: Option<&str>,
    table: Option<&str>,
    max_cells: i64,
    formula_mode: &str,
) -> CliResult<XlsxTextSource> {
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
    let data = xlsx_text_strings_from_export(values, formulas, formula_mode)?;
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
    Ok(XlsxTextSource {
        source: Value::Object(source),
        data,
        rows,
        range: source_range,
    })
}

fn xlsx_text_strings_from_export(
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
            out_row.push(xlsx_value_to_text(value));
        }
        out.push(out_row);
    }
    Ok(out)
}

fn xlsx_value_to_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        other => other.to_string(),
    }
}

fn join_xlsx_text_matrix(values: &[Vec<String>], row_sep: &str, col_sep: &str) -> String {
    values
        .iter()
        .map(|row| row.join(col_sep))
        .collect::<Vec<_>>()
        .join(row_sep)
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

fn normalize_replace_text_from_xlsx_mode(value: Option<&str>) -> CliResult<String> {
    match value
        .unwrap_or("plain-text")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "plain-text" => Ok("plain-text".to_string()),
        "preserve-format" => Ok("preserve-format".to_string()),
        _ => Err(CliError::invalid_args(
            "--mode must be plain-text or preserve-format",
        )),
    }
}

fn decode_text_separator_flag(value: &str, flag_name: &str) -> CliResult<String> {
    if !value.contains('\\') {
        return Ok(value.to_string());
    }
    let mut decoded = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }
        let Some(escaped) = chars.next() else {
            return Err(CliError::invalid_args(format!(
                "{flag_name} contains invalid escape sequence: trailing backslash"
            )));
        };
        match escaped {
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            '\\' => decoded.push('\\'),
            '"' => decoded.push('"'),
            other => {
                return Err(CliError::invalid_args(format!(
                    "{flag_name} contains invalid escape sequence: \\{other}"
                )));
            }
        }
    }
    Ok(decoded)
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

fn text_map_records_from_values(
    values: &[Vec<String>],
    slide_col: &str,
    target_col: &str,
    text_col: &str,
) -> CliResult<(Vec<TextMapRecord>, TextMapColumns)> {
    if values.len() < 2 {
        return Err(CliError::invalid_args(
            "source map must include a header row and at least one replacement row",
        ));
    }
    let header = &values[0];
    let columns = resolve_text_map_columns(header, slide_col, target_col, text_col)?;
    let mut records = Vec::with_capacity(values.len().saturating_sub(1));
    for (row_index, row) in values.iter().enumerate().skip(1) {
        let source_row = row_index + 1;
        let slide_text = row
            .get(columns.slide_index())
            .map(|value| value.trim())
            .unwrap_or_default();
        if slide_text.is_empty() {
            return Err(CliError::invalid_args(format!(
                "row {source_row}: slide value is required"
            )));
        }
        let slide = slide_text.parse::<u32>().map_err(|_| {
            CliError::invalid_args(format!(
                "row {source_row}: slide must be a positive integer"
            ))
        })?;
        if slide < 1 {
            return Err(CliError::invalid_args(format!(
                "row {source_row}: slide must be a positive integer"
            )));
        }
        let target = row
            .get(columns.target_index())
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        if target.is_empty() {
            return Err(CliError::invalid_args(format!(
                "row {source_row}: target value is required"
            )));
        }
        records.push(TextMapRecord {
            source_row,
            slide,
            target,
            text: row.get(columns.text_index()).cloned().unwrap_or_default(),
        });
    }
    Ok((records, columns.without_indexes()))
}

struct ResolvedTextMapColumns {
    slide: String,
    target: String,
    text: String,
    slide_index: usize,
    target_index: usize,
    text_index: usize,
}

impl ResolvedTextMapColumns {
    fn slide_index(&self) -> usize {
        self.slide_index
    }

    fn target_index(&self) -> usize {
        self.target_index
    }

    fn text_index(&self) -> usize {
        self.text_index
    }

    fn without_indexes(self) -> TextMapColumns {
        TextMapColumns {
            slide: self.slide,
            target: self.target,
            text: self.text,
        }
    }
}

fn resolve_text_map_columns(
    header: &[String],
    slide_col: &str,
    target_col: &str,
    text_col: &str,
) -> CliResult<ResolvedTextMapColumns> {
    if header.is_empty() {
        return Err(CliError::invalid_args("source map header row is empty"));
    }
    let (slide_index, slide) = resolve_text_map_column(header, slide_col, "--slide-col")?;
    let (target_index, target) = resolve_text_map_column(header, target_col, "--target-col")?;
    let (text_index, text) = resolve_text_map_column(header, text_col, "--text-col")?;
    if slide_index == target_index || slide_index == text_index || target_index == text_index {
        return Err(CliError::invalid_args(
            "--slide-col, --target-col, and --text-col must resolve to distinct columns",
        ));
    }
    Ok(ResolvedTextMapColumns {
        slide,
        target,
        text,
        slide_index,
        target_index,
        text_index,
    })
}

fn resolve_text_map_column(
    header: &[String],
    selector: &str,
    flag_name: &str,
) -> CliResult<(usize, String)> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(CliError::invalid_args(format!("{flag_name} is required")));
    }
    if let Ok(index) = selector.parse::<usize>() {
        if index < 1 || index > header.len() {
            return Err(CliError::invalid_args(format!(
                "{flag_name} index {index} out of range (1-{})",
                header.len()
            )));
        }
        let name = header[index - 1].trim();
        return Ok((
            index - 1,
            if name.is_empty() {
                selector.to_string()
            } else {
                name.to_string()
            },
        ));
    }
    let normalized = selector.to_ascii_lowercase();
    let mut matched = None;
    for (index, name) in header.iter().enumerate() {
        if name.trim().to_ascii_lowercase() != normalized {
            continue;
        }
        if matched.is_some() {
            return Err(CliError::invalid_args(format!(
                "{flag_name} header {selector:?} is ambiguous"
            )));
        }
        matched = Some(index);
    }
    let matched = matched.ok_or_else(|| {
        CliError::invalid_args(format!("{flag_name} header {selector:?} not found"))
    })?;
    Ok((matched, header[matched].trim().to_string()))
}

fn build_text_target_replace_plan(
    file: &str,
    slides: &[PptxSlideRef],
    slide: u32,
    target_selector: &str,
    text: &str,
) -> CliResult<TextTargetReplacePlan> {
    let slide_ref = slides.get(slide as usize - 1).ok_or_else(|| {
        CliError::invalid_args(format!("slide {slide} out of range (1-{})", slides.len()))
    })?;
    let slide_xml = zip_text(file, &slide_ref.part)?;
    build_text_target_replace_plan_from_xml(&slide_xml, slide_ref, target_selector, text)
}

fn build_text_target_replace_plan_from_xml(
    slide_xml: &str,
    slide_ref: &PptxSlideRef,
    target_selector: &str,
    text: &str,
) -> CliResult<TextTargetReplacePlan> {
    let target = resolve_text_shape_target(slide_xml, slide_ref, target_selector)?;
    let slide_xml = apply_shape_text_replacement(slide_xml, &target, text)?;
    Ok(TextTargetReplacePlan {
        slide: slide_ref.number,
        slide_part: slide_ref.part.clone(),
        slide_xml,
        target,
        text: text.to_string(),
    })
}

fn resolve_text_shape_target(
    slide_xml: &str,
    slide_ref: &PptxSlideRef,
    target_selector: &str,
) -> CliResult<ShapeTarget> {
    let matches = slide_targets(slide_xml, slide_ref)
        .into_iter()
        .filter(|target| target.matches_selector(target_selector))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err(CliError::target_not_found(format!(
            "target not found: {target_selector}"
        )));
    }
    if matches.len() > 1 {
        return Err(CliError::invalid_args(format!(
            "ambiguous target: {target_selector}"
        )));
    }
    let target = matches.into_iter().next().expect("matched target");
    if text_nodes_in_span(slide_xml, target.span)?.is_empty() {
        return Err(CliError::invalid_args(format!(
            "target is non-text: {target_selector}"
        )));
    }
    Ok(target)
}

fn apply_shape_text_replacement(
    slide_xml: &str,
    target: &ShapeTarget,
    text: &str,
) -> CliResult<String> {
    let text_nodes = text_nodes_in_span(slide_xml, target.span)?;
    if text_nodes.is_empty() {
        return Err(CliError::invalid_args("target is non-text"));
    }
    let mut replacements = Vec::with_capacity(text_nodes.len());
    for (index, node) in text_nodes.into_iter().enumerate() {
        replacements.push(TextNodeReplacement {
            span: node,
            after: if index == 0 {
                text.to_string()
            } else {
                String::new()
            },
        });
    }
    Ok(apply_text_node_replacements(slide_xml, &mut replacements))
}

fn map_text_target_error(err: CliError, target: &str) -> CliError {
    match err.exit_code {
        crate::EXIT_TARGET_NOT_FOUND => {
            CliError::target_not_found(format!("target not found: {target}"))
        }
        _ => err,
    }
}

fn map_text_map_target_error(err: CliError, record: &TextMapRecord) -> CliError {
    match err.exit_code {
        crate::EXIT_TARGET_NOT_FOUND => CliError::target_not_found(format!(
            "target not found: row {} target {}",
            record.source_row, record.target
        )),
        _ => err,
    }
}

fn text_from_xlsx_result_json(
    file: &str,
    request: &ReplaceTextFromXlsxRequest,
    plan: &TextTargetReplacePlan,
    options: &PptxReplaceMutationOptions,
) -> Value {
    let output = mutation_output_path(file, options);
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    if options.dry_run {
        result.insert("dryRun".to_string(), json!(true));
    }
    result.insert("source".to_string(), request.source.source.clone());
    result.insert(
        "text".to_string(),
        json!({
            "mode": request.mode,
            "formulaMode": request.formula_mode,
            "rowSeparator": request.row_sep,
            "colSeparator": request.col_sep,
            "chars": request.text.chars().count(),
            "value": request.text,
        }),
    );
    result.insert(
        "destination".to_string(),
        text_shape_destination_json(
            &plan.target,
            plan.slide,
            &request.target,
            &plan.text,
            output.as_deref(),
        ),
    );
    add_shape_text_readback_commands(
        &mut result,
        output.as_deref(),
        options.dry_run,
        plan.slide,
        &plan.target.primary_selector,
    );
    Value::Object(result)
}

fn text_map_from_xlsx_result_json(
    file: &str,
    request: &ReplaceTextMapFromXlsxRequest,
    applied: &[TextMapApplied],
    options: &PptxReplaceMutationOptions,
) -> Value {
    let output = mutation_output_path(file, options);
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    if options.dry_run {
        result.insert("dryRun".to_string(), json!(true));
    }
    result.insert("source".to_string(), request.source.source.clone());
    result.insert(
        "map".to_string(),
        json!({
            "mode": request.mode,
            "formulaMode": request.formula_mode,
            "rows": request.source.rows.saturating_sub(1),
            "applied": applied.len(),
            "slideColumn": request.columns.slide,
            "targetColumn": request.columns.target,
            "textColumn": request.columns.text,
        }),
    );
    result.insert(
        "replacements".to_string(),
        Value::Array(
            applied
                .iter()
                .map(|item| text_map_replacement_json(item, output.as_deref(), options.dry_run))
                .collect(),
        ),
    );
    add_output_verification_commands(&mut result, output.as_deref(), options.dry_run);
    Value::Object(result)
}

fn text_map_replacement_json(
    applied: &TextMapApplied,
    output: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("sourceRow".to_string(), json!(applied.record.source_row));
    result.insert("slide".to_string(), json!(applied.record.slide));
    result.insert("target".to_string(), json!(applied.record.target));
    result.insert(
        "chars".to_string(),
        json!(applied.record.text.chars().count()),
    );
    result.insert("text".to_string(), json!(applied.record.text));
    result.insert(
        "destination".to_string(),
        text_shape_destination_json(
            &applied.plan.target,
            applied.record.slide,
            &applied.record.target,
            &applied.record.text,
            output,
        ),
    );
    add_shape_text_readback_commands(
        &mut result,
        output,
        dry_run,
        applied.record.slide,
        &applied.plan.target.primary_selector,
    );
    Value::Object(result)
}

fn text_shape_destination_json(
    target: &ShapeTarget,
    slide: u32,
    requested_target: &str,
    text: &str,
    output: Option<&str>,
) -> Value {
    let mut result = Map::new();
    if let Some(output) = output {
        result.insert("file".to_string(), json!(output));
    }
    result.insert("slide".to_string(), json!(slide));
    result.insert("target".to_string(), json!(requested_target));
    result.insert("shapeId".to_string(), json!(target.shape_id));
    if !target.shape_name.is_empty() {
        result.insert("shapeName".to_string(), json!(target.shape_name));
    }
    result.insert("targetKind".to_string(), json!(target.target_kind));
    result.insert(
        "primarySelector".to_string(),
        json!(target.primary_selector),
    );
    if !target.handle.is_empty() {
        result.insert("handle".to_string(), json!(target.handle));
    }
    result.insert("selectors".to_string(), json!(target.selectors));
    result.insert("textPreview".to_string(), json!(text_preview(text)));
    Value::Object(result)
}

fn text_preview(text: &str) -> String {
    let mut preview = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if preview.len() > 140 {
        preview.truncate(137);
        preview.push_str("...");
    }
    preview
}

fn add_shape_text_readback_commands(
    result: &mut Map<String, Value>,
    output: Option<&str>,
    dry_run: bool,
    slide: u32,
    target: &str,
) {
    let command_target = output.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    result.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx shapes get {} --slide {} --target {} --include-text",
            command_arg(command_target),
            slide,
            command_arg(target)
        )),
    );
    result.insert(
        format!("slideReadbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(command_target),
            slide
        )),
    );
    add_output_verification_commands(result, Some(command_target), dry_run);
}

fn resolve_text_occurrences_new_text(args: &[String]) -> CliResult<String> {
    let has_inline = value_flag_present(args, "--new-text");
    let has_file = value_flag_present(args, "--new-text-file");
    if has_inline == has_file {
        return Err(CliError::invalid_args(
            "must specify exactly one of --new-text or --new-text-file",
        ));
    }
    if has_inline {
        return Ok(crate::parse_string_flag(args, "--new-text")?.unwrap_or_default());
    }
    let path = crate::parse_string_flag(args, "--new-text-file")?.unwrap_or_default();
    fs::read_to_string(&path)
        .map_err(|_| CliError::file_not_found(format!("file not found: {path}")))
}

struct TextOccurrencesRequest {
    match_text: String,
    new_text: String,
    for_slides: String,
    for_shape: String,
    ignore_case: bool,
    expect_count: Option<usize>,
    expect_plan_hash: String,
    allow_zero: bool,
}

struct TextOccurrencePlan {
    selected_slides: Vec<u32>,
    slides_scanned: usize,
    targets_scanned: usize,
    text_nodes_scanned: usize,
    changed_target_count: usize,
    replacement_count: usize,
    plan_hash: String,
    matches: Vec<TextOccurrenceMatch>,
    overrides: BTreeMap<String, String>,
    shape_scoped: bool,
}

#[derive(Clone)]
struct TextOccurrenceMatch {
    slide_number: u32,
    part_uri: String,
    shape_id: u32,
    shape_name: String,
    target_kind: String,
    primary_selector: String,
    selectors: Vec<String>,
    text_node_index: usize,
    match_count: usize,
    before_text: String,
    after_text: String,
    source_hash: String,
}

fn replace_text_occurrences(
    file: &str,
    request: TextOccurrencesRequest,
    options: PptxReplaceMutationOptions,
) -> CliResult<Value> {
    ensure_pptx(file)?;
    let slides = pptx_slide_refs_for_replace(file)?;
    let shape_scope = resolve_shape_scope(&request.for_shape, &slides)?;
    let selected_slides = if let Some((slide_number, _)) = shape_scope {
        vec![slide_number]
    } else {
        resolve_slide_selection(&request.for_slides, &slides)?
    };
    let mut plan =
        build_text_occurrence_plan(file, &slides, &selected_slides, shape_scope, &request)?;
    if let Some(expected) = request.expect_count
        && expected != plan.replacement_count
    {
        return Err(CliError::invalid_args(format!(
            "text occurrences guard mismatch: --expect-count is {expected} but planned replacements are {}",
            plan.replacement_count
        )));
    }
    if !request.expect_plan_hash.is_empty() && request.expect_plan_hash != plan.plan_hash {
        return Err(CliError::invalid_args(format!(
            "text occurrences guard mismatch: --expect-plan-hash is {} but current plan hash is {}",
            request.expect_plan_hash, plan.plan_hash
        )));
    }
    if plan.replacement_count == 0 && !options.dry_run && !request.allow_zero {
        return Err(CliError::invalid_args(
            "text occurrences no matches: no occurrences of match text were found",
        ));
    }
    write_replace_mutation(file, &plan.overrides, &BTreeMap::new(), &options)?;
    Ok(text_occurrences_result_json(
        file, &request, &mut plan, &options,
    ))
}

fn build_text_occurrence_plan(
    file: &str,
    slides: &[PptxSlideRef],
    selected_slides: &[u32],
    shape_scope: Option<(u32, u32)>,
    request: &TextOccurrencesRequest,
) -> CliResult<TextOccurrencePlan> {
    let mut plan = TextOccurrencePlan {
        selected_slides: selected_slides.to_vec(),
        slides_scanned: 0,
        targets_scanned: 0,
        text_nodes_scanned: 0,
        changed_target_count: 0,
        replacement_count: 0,
        plan_hash: String::new(),
        matches: Vec::new(),
        overrides: BTreeMap::new(),
        shape_scoped: shape_scope.is_some(),
    };
    let mut changed_targets = BTreeSet::<String>::new();

    for slide_number in selected_slides {
        let slide_ref = slides
            .get(*slide_number as usize - 1)
            .ok_or_else(|| CliError::unexpected(format!("slide {slide_number} not found")))?;
        let slide_xml = zip_text(file, &slide_ref.part)?;
        let targets = slide_targets(&slide_xml, slide_ref);
        let mut replacements = Vec::<TextNodeReplacement>::new();
        plan.slides_scanned += 1;
        for target in targets {
            if let Some((scope_slide, scope_shape)) = shape_scope
                && (scope_slide != *slide_number || scope_shape != target.shape_id)
            {
                continue;
            }
            let text_nodes = text_nodes_in_span(&slide_xml, target.span)?;
            if text_nodes.is_empty() {
                continue;
            }
            plan.targets_scanned += 1;
            for (node_index, node) in text_nodes.iter().enumerate() {
                plan.text_nodes_scanned += 1;
                let (after, count) = replace_text_occurrences_in_string(
                    &node.before_text,
                    &request.match_text,
                    &request.new_text,
                    request.ignore_case,
                );
                if count == 0 {
                    continue;
                }
                plan.replacement_count += count;
                changed_targets.insert(format!("{slide_number}:{}", target.shape_id));
                plan.matches.push(TextOccurrenceMatch {
                    slide_number: *slide_number,
                    part_uri: format!("/{}", slide_ref.part),
                    shape_id: target.shape_id,
                    shape_name: target.shape_name.clone(),
                    target_kind: target.target_kind.clone(),
                    primary_selector: target.primary_selector.clone(),
                    selectors: target.selectors.clone(),
                    text_node_index: node_index + 1,
                    match_count: count,
                    before_text: node.before_text.clone(),
                    after_text: after.clone(),
                    source_hash: sha256_string(&node.before_text),
                });
                replacements.push(TextNodeReplacement {
                    span: node.clone(),
                    after,
                });
            }
        }
        if !replacements.is_empty() {
            plan.overrides.insert(
                slide_ref.part.clone(),
                apply_text_node_replacements(&slide_xml, &mut replacements),
            );
        }
    }

    plan.changed_target_count = changed_targets.len();
    plan.plan_hash = compute_text_occurrences_plan_hash(
        &request.match_text,
        &request.new_text,
        request.ignore_case,
        &plan,
    );
    Ok(plan)
}

fn resolve_shape_scope(value: &str, slides: &[PptxSlideRef]) -> CliResult<Option<(u32, u32)>> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let Some(rest) = value.strip_prefix("H:pptx/s:") else {
        return Err(CliError::invalid_args(
            "--for-shape must be a shape handle (H:pptx/s:<sldId>/shape:n:<id>)",
        ));
    };
    let Some((slide_id_raw, shape_raw)) = rest.split_once("/shape:n:") else {
        return Err(CliError::invalid_args(
            "--for-shape must be a shape handle (H:pptx/s:<sldId>/shape:n:<id>)",
        ));
    };
    let slide_id = slide_id_raw
        .parse::<u32>()
        .map_err(|_| CliError::invalid_args("malformed PPTX shape handle"))?;
    let shape_id = shape_raw
        .parse::<u32>()
        .map_err(|_| CliError::invalid_args("malformed PPTX shape handle"))?;
    let slide_number = slides
        .iter()
        .find(|slide| slide.slide_id == slide_id)
        .map(|slide| slide.number)
        .ok_or_else(|| {
            CliError::target_not_found(format!(
                "stale PPTX handle: slide sldId {slide_id} not found"
            ))
        })?;
    Ok(Some((slide_number, shape_id)))
}

fn resolve_slide_selection(value: &str, slides: &[PptxSlideRef]) -> CliResult<Vec<u32>> {
    if value.trim().is_empty() {
        return Ok(slides.iter().map(|slide| slide.number).collect());
    }
    if let Some(raw) = value.strip_prefix("H:pptx/s:") {
        let slide_id = raw
            .parse::<u32>()
            .map_err(|_| CliError::invalid_args("malformed PPTX slide handle"))?;
        let slide = slides
            .iter()
            .find(|slide| slide.slide_id == slide_id)
            .ok_or_else(|| {
                CliError::target_not_found(format!(
                    "stale PPTX handle: slide sldId {slide_id} not found"
                ))
            })?;
        return Ok(vec![slide.number]);
    }
    let parsed = parse_slide_spec(value).map_err(|message| {
        CliError::invalid_args(format!("invalid slide specification: {message}"))
    })?;
    if parsed.is_empty() {
        return Err(CliError::invalid_args(
            "no valid slides specified in --for-slides",
        ));
    }
    let mut out = Vec::new();
    for slide_number in parsed {
        if slide_number < 1 || slide_number as usize > slides.len() {
            return Err(CliError::invalid_args(format!(
                "slide {slide_number} not found (presentation has {} slides)",
                slides.len()
            )));
        }
        if !out.contains(&slide_number) {
            out.push(slide_number);
        }
    }
    Ok(out)
}

fn parse_slide_spec(value: &str) -> Result<Vec<u32>, String> {
    let mut slides = Vec::new();
    for part in value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if let Some((start, end)) = part.split_once('-') {
            let start = start
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("invalid slide number {part:?}"))?;
            let end = end
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("invalid slide number {part:?}"))?;
            if start > end {
                return Err(format!("invalid slide range {part:?}"));
            }
            slides.extend(start..=end);
        } else {
            slides.push(
                part.parse::<u32>()
                    .map_err(|_| format!("invalid slide number {part:?}"))?,
            );
        }
    }
    Ok(slides)
}

fn replace_text_occurrences_in_string(
    text: &str,
    match_text: &str,
    replacement: &str,
    ignore_case: bool,
) -> (String, usize) {
    if text.is_empty() || match_text.is_empty() {
        return (text.to_string(), 0);
    }
    if !ignore_case {
        let count = text.matches(match_text).count();
        if count == 0 {
            return (text.to_string(), 0);
        }
        return (text.replace(match_text, replacement), count);
    }
    let haystack = text.to_ascii_lowercase();
    let needle = match_text.to_ascii_lowercase();
    let mut out = String::new();
    let mut offset = 0;
    let mut count = 0;
    while let Some(found) = haystack[offset..].find(&needle) {
        let start = offset + found;
        let end = start + match_text.len();
        out.push_str(&text[offset..start]);
        out.push_str(replacement);
        offset = end;
        count += 1;
    }
    if count == 0 {
        return (text.to_string(), 0);
    }
    out.push_str(&text[offset..]);
    (out, count)
}

fn apply_text_node_replacements(xml: &str, replacements: &mut [TextNodeReplacement]) -> String {
    replacements.sort_by_key(|replacement| replacement.span.content_start);
    let mut updated = xml.to_string();
    for replacement in replacements.iter().rev() {
        updated = replace_xml_span(
            &updated,
            replacement.span.content_start,
            replacement.span.content_end,
            &xml_escape(&replacement.after),
        );
        if needs_xml_space_preserve(&replacement.after) && !replacement.span.has_xml_space {
            updated = replace_xml_span(
                &updated,
                replacement.span.open_end - 1,
                replacement.span.open_end - 1,
                r#" xml:space="preserve""#,
            );
        }
    }
    updated
}

fn compute_text_occurrences_plan_hash(
    match_text: &str,
    new_text: &str,
    ignore_case: bool,
    plan: &TextOccurrencePlan,
) -> String {
    let mut hasher = Sha256::new();
    let mut write_field = |value: &str| {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    };
    write_field("pptx.replace.text-occurrences");
    write_field(match_text);
    write_field(new_text);
    write_field(if ignore_case { "true" } else { "false" });
    for slide in &plan.selected_slides {
        write_field(&slide.to_string());
    }
    for item in &plan.matches {
        write_field(&item.slide_number.to_string());
        write_field(&item.part_uri);
        write_field(&item.shape_id.to_string());
        write_field(&item.primary_selector);
        write_field(&item.text_node_index.to_string());
        write_field(&item.match_count.to_string());
        write_field(&item.before_text);
        write_field(&item.source_hash);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn text_occurrences_result_json(
    file: &str,
    request: &TextOccurrencesRequest,
    plan: &mut TextOccurrencePlan,
    options: &PptxReplaceMutationOptions,
) -> Value {
    let output = mutation_output_path(file, options);
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert(
        "operation".to_string(),
        json!("pptx.replace.text-occurrences"),
    );
    result.insert("matchText".to_string(), json!(request.match_text));
    result.insert("newText".to_string(), json!(request.new_text));
    result.insert("ignoreCase".to_string(), json!(request.ignore_case));
    if !request.for_slides.is_empty() {
        result.insert("forSlides".to_string(), json!(request.for_slides));
    }
    let mut guard = Map::new();
    if let Some(expected) = request.expect_count {
        guard.insert("expectedCount".to_string(), json!(expected));
    }
    guard.insert("actualCount".to_string(), json!(plan.replacement_count));
    if !request.expect_plan_hash.is_empty() {
        guard.insert(
            "expectedPlanHash".to_string(),
            json!(request.expect_plan_hash),
        );
    }
    guard.insert("actualPlanHash".to_string(), json!(plan.plan_hash));
    guard.insert("allowZero".to_string(), json!(request.allow_zero));
    result.insert("staleGuard".to_string(), Value::Object(guard));
    result.insert(
        "summary".to_string(),
        json!({
            "slidesScanned": plan.slides_scanned,
            "targetsScanned": plan.targets_scanned,
            "textNodesScanned": plan.text_nodes_scanned,
            "changedTargetCount": plan.changed_target_count,
            "replacementCount": plan.replacement_count,
        }),
    );
    result.insert(
        "scope".to_string(),
        json!({
            "slides": plan.selected_slides,
            "text": if plan.shape_scoped {
                "slide-visible text nodes under a single shape target (shape-scoped)"
            } else {
                "slide-visible text nodes under published slide targets"
            },
            "splitRunMatches": "not matched; only occurrences contained within one XML text node are replaced",
            "excludedContent": "notes, layouts, masters, comments, charts, and non-slide parts",
            "tableCellsIncluded": true,
            "slideShapesIncluded": true,
        }),
    );
    result.insert(
        "matches".to_string(),
        Value::Array(
            plan.matches
                .iter()
                .map(|item| text_occurrence_match_json(item, output.as_deref(), options.dry_run))
                .collect(),
        ),
    );
    add_output_verification_commands(&mut result, output.as_deref(), options.dry_run);
    Value::Object(result)
}

fn text_occurrence_match_json(
    item: &TextOccurrenceMatch,
    output: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("slideNumber".to_string(), json!(item.slide_number));
    result.insert("partUri".to_string(), json!(item.part_uri));
    result.insert("shapeId".to_string(), json!(item.shape_id));
    if !item.shape_name.is_empty() {
        result.insert("shapeName".to_string(), json!(item.shape_name));
    }
    result.insert("targetKind".to_string(), json!(item.target_kind));
    result.insert("primarySelector".to_string(), json!(item.primary_selector));
    result.insert("selectors".to_string(), json!(item.selectors));
    result.insert("textNodeIndex".to_string(), json!(item.text_node_index));
    result.insert("matchCount".to_string(), json!(item.match_count));
    result.insert("beforeText".to_string(), json!(item.before_text));
    result.insert("afterText".to_string(), json!(item.after_text));
    result.insert("sourceHash".to_string(), json!(item.source_hash));
    let command_target = output.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    let readback = if item.target_kind == "table" {
        format!(
            "ooxml --json pptx tables show {} --slide {} --target {}",
            command_arg(command_target),
            item.slide_number,
            command_arg(&item.primary_selector)
        )
    } else {
        format!(
            "ooxml --json pptx shapes get {} --slide {} --target {} --include-text --include-bounds",
            command_arg(command_target),
            item.slide_number,
            command_arg(&item.primary_selector)
        )
    };
    result.insert(format!("readbackCommand{suffix}"), json!(readback));
    add_slide_validate_render_commands(&mut result, command_target, item.slide_number, dry_run);
    Value::Object(result)
}

struct ImageReplacePlan {
    slide: u32,
    target: ShapeTarget,
    slide_xml: String,
    rels_part: String,
    rels_xml: String,
    old_target_uri: String,
    old_content_type: String,
    new_target_uri: String,
    new_content_type: String,
    relationship_id: String,
}

struct ImageBatchReplacePlan {
    slides: Vec<ImageBatchSlideResult>,
    text_overrides: BTreeMap<String, String>,
    binary_overrides: BTreeMap<String, Vec<u8>>,
    success_count: usize,
    not_found_count: usize,
    error_count: usize,
}

struct ImageBatchSlideResult {
    slide: u32,
    success: bool,
    not_found: bool,
    error: String,
    plan: Option<ImageReplacePlan>,
}

struct ImageBatchSelector {
    normalized: String,
    unsupported_error: Option<String>,
}

fn replace_image(
    file: &str,
    target_selector: &str,
    slide: Option<u32>,
    image_data: &[u8],
    new_content_type: &str,
    fit_mode: &str,
    options: PptxReplaceMutationOptions,
) -> CliResult<Value> {
    ensure_pptx(file)?;
    let slides = pptx_slide_refs_for_replace(file)?;
    if let Some(slide) = slide
        && (slide < 1 || slide as usize > slides.len())
    {
        return Err(CliError::unexpected(format!(
            "failed to replace image: invalid slide number {slide} (presentation has {} slides)",
            slides.len()
        )));
    }
    let plan = plan_image_replace(
        file,
        &slides,
        target_selector,
        slide,
        new_content_type,
        fit_mode,
    )?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(
        slides[plan.slide as usize - 1].part.clone(),
        plan.slide_xml.clone(),
    );
    text_overrides.insert(plan.rels_part.clone(), plan.rels_xml.clone());
    let mut content_types = zip_text(file, "[Content_Types].xml")?;
    content_types =
        ensure_content_type_override(content_types, &plan.new_target_uri, &plan.new_content_type);
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);
    let mut binary_overrides = BTreeMap::new();
    binary_overrides.insert(
        plan.new_target_uri.trim_start_matches('/').to_string(),
        image_data.to_vec(),
    );
    write_replace_mutation(file, &text_overrides, &binary_overrides, &options)?;
    Ok(image_replace_result_json(
        file,
        target_selector,
        fit_mode,
        &plan,
        &options,
    ))
}

fn plan_image_replace(
    file: &str,
    slides: &[PptxSlideRef],
    target_selector: &str,
    requested_slide: Option<u32>,
    new_content_type: &str,
    fit_mode: &str,
) -> CliResult<ImageReplacePlan> {
    for slide_ref in slides {
        if let Some(requested_slide) = requested_slide
            && requested_slide != slide_ref.number
        {
            continue;
        }
        let slide_xml = zip_text(file, &slide_ref.part)?;
        let targets = slide_targets(&slide_xml, slide_ref);
        let Some(target) = targets
            .iter()
            .find(|target| {
                target.target_kind == "picture" && target.matches_selector(target_selector)
            })
            .cloned()
        else {
            continue;
        };
        let relationship_id = target.image_rel_id.clone();
        if relationship_id.is_empty() {
            return Err(CliError::unexpected(
                "failed to replace image: no relationship ID found in blip element",
            ));
        }
        let rels_part = relationships_part_for(&slide_ref.part);
        let rels_xml = zip_text(file, &rels_part)?;
        let mut rels = relationship_entries_from_xml(&rels_xml);
        let Some(rel_index) = rels.iter().position(|rel| rel.id == relationship_id) else {
            return Err(CliError::unexpected(format!(
                "failed to replace image: could not resolve relationship {relationship_id}",
            )));
        };
        if rels[rel_index].target_mode == "External" {
            return Err(CliError::target_not_found(format!(
                "picture shape not found: {target_selector}"
            )));
        }
        let old_target_uri =
            resolve_relationship_target(&format!("/{}", slide_ref.part), &rels[rel_index].target);
        let old_content_type = content_type_for_part(file, &old_target_uri).unwrap_or_default();
        let new_target_uri =
            replacement_image_uri(&old_target_uri, &old_content_type, new_content_type)?;
        if new_target_uri != old_target_uri {
            rels[rel_index].target = relationship_target_from_source_to_target(
                &format!("/{}", slide_ref.part),
                &new_target_uri,
            );
        }
        let rels_xml = render_relationships_xml(&rels);
        let slide_xml = update_picture_fit_mode(&slide_xml, target.span, fit_mode)?;
        return Ok(ImageReplacePlan {
            slide: slide_ref.number,
            target,
            slide_xml,
            rels_part,
            rels_xml,
            old_target_uri,
            old_content_type,
            new_target_uri,
            new_content_type: new_content_type.to_string(),
            relationship_id,
        });
    }
    if let Some(slide) = requested_slide {
        return Err(image_target_not_found(target_selector, slide, file));
    }
    Err(CliError::target_not_found(format!(
        "picture shape not found: {target_selector}; discover with `ooxml --json pptx slides show <file> --include-bounds`"
    )))
}

fn replace_images_for_slides(
    file: &str,
    target_selector: &str,
    for_slides: &str,
    image_data: &[u8],
    new_content_type: &str,
    fit_mode: &str,
    options: PptxReplaceMutationOptions,
) -> CliResult<Value> {
    ensure_pptx(file)?;
    let selector = parse_image_batch_selector(target_selector)?;
    let selected_slides = parse_image_batch_slide_spec(for_slides).map_err(|message| {
        CliError::invalid_args(format!("invalid slide specification: {message}"))
    })?;
    if selected_slides.is_empty() {
        return Err(CliError::invalid_args(
            "no valid slides specified in --for-slides",
        ));
    }
    let slides = pptx_slide_refs_for_replace(file)?;
    let plan = plan_image_batch_replace(
        file,
        &slides,
        &selected_slides,
        &selector,
        image_data,
        new_content_type,
        fit_mode,
    );
    write_replace_mutation(file, &plan.text_overrides, &plan.binary_overrides, &options)?;
    Ok(image_batch_replace_result_json(&selector.normalized, &plan))
}

fn plan_image_batch_replace(
    file: &str,
    slides: &[PptxSlideRef],
    selected_slides: &[u32],
    selector: &ImageBatchSelector,
    image_data: &[u8],
    new_content_type: &str,
    fit_mode: &str,
) -> ImageBatchReplacePlan {
    let mut plan = ImageBatchReplacePlan {
        slides: Vec::with_capacity(selected_slides.len()),
        text_overrides: BTreeMap::new(),
        binary_overrides: BTreeMap::new(),
        success_count: 0,
        not_found_count: 0,
        error_count: 0,
    };

    for &slide in selected_slides {
        if slide < 1 || slide as usize > slides.len() {
            plan.not_found_count += 1;
            plan.slides.push(ImageBatchSlideResult {
                slide,
                success: false,
                not_found: true,
                error: format!(
                    "slide {slide} not found (presentation has {} slides)",
                    slides.len()
                ),
                plan: None,
            });
            continue;
        }
        if let Some(message) = selector.unsupported_error.as_deref() {
            plan.error_count += 1;
            plan.slides.push(ImageBatchSlideResult {
                slide,
                success: false,
                not_found: false,
                error: message.to_string(),
                plan: None,
            });
            continue;
        }
        match plan_image_replace(
            file,
            slides,
            &selector.normalized,
            Some(slide),
            new_content_type,
            fit_mode,
        ) {
            Ok(replace_plan) => {
                plan.success_count += 1;
                plan.text_overrides.insert(
                    slides[replace_plan.slide as usize - 1].part.clone(),
                    replace_plan.slide_xml.clone(),
                );
                plan.text_overrides.insert(
                    replace_plan.rels_part.clone(),
                    replace_plan.rels_xml.clone(),
                );
                plan.binary_overrides.insert(
                    replace_plan
                        .new_target_uri
                        .trim_start_matches('/')
                        .to_string(),
                    image_data.to_vec(),
                );
                plan.slides.push(ImageBatchSlideResult {
                    slide,
                    success: true,
                    not_found: false,
                    error: String::new(),
                    plan: Some(replace_plan),
                });
            }
            Err(err) if err.code == "target_not_found" => {
                plan.not_found_count += 1;
                plan.slides.push(ImageBatchSlideResult {
                    slide,
                    success: false,
                    not_found: true,
                    error: String::new(),
                    plan: None,
                });
            }
            Err(err) => {
                plan.error_count += 1;
                plan.slides.push(ImageBatchSlideResult {
                    slide,
                    success: false,
                    not_found: false,
                    error: err.message,
                    plan: None,
                });
            }
        }
    }

    if plan.success_count > 0
        && let Ok(content_types) = zip_text(file, "[Content_Types].xml")
    {
        let content_types = plan
            .slides
            .iter()
            .filter_map(|item| item.plan.as_ref())
            .fold(content_types, |content_types, item| {
                ensure_content_type_override(
                    content_types,
                    &item.new_target_uri,
                    &item.new_content_type,
                )
            });
        plan.text_overrides
            .insert("[Content_Types].xml".to_string(), content_types);
    }

    plan
}

fn image_batch_replace_result_json(target_selector: &str, plan: &ImageBatchReplacePlan) -> Value {
    json!({
        "target": target_selector,
        "totalSlides": plan.slides.len(),
        "successCount": plan.success_count,
        "notFoundCount": plan.not_found_count,
        "errorCount": plan.error_count,
        "results": plan.slides.iter().map(image_batch_slide_result_json).collect::<Vec<_>>(),
    })
}

fn image_batch_slide_result_json(item: &ImageBatchSlideResult) -> Value {
    json!({
        "SlideNumber": item.slide,
        "Success": item.success,
        "NotFound": item.not_found,
        "Error": item.error,
        "Result": item.plan.as_ref().map(|plan| {
            json!({
                "ShapeID": plan.target.shape_id,
                "ShapeName": plan.target.shape_name,
                "OldTargetURI": plan.old_target_uri,
                "OldContentType": plan.old_content_type,
                "NewTargetURI": plan.new_target_uri,
                "NewContentType": plan.new_content_type,
                "RelationshipID": plan.relationship_id,
            })
        }),
    })
}

fn parse_image_batch_selector(target_selector: &str) -> CliResult<ImageBatchSelector> {
    let trimmed = target_selector.trim();
    if trimmed.is_empty() {
        return Err(CliError::invalid_args(
            "invalid target selector: selector cannot be empty",
        ));
    }
    if let Some(raw) = trimmed.strip_prefix("shape:") {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(CliError::invalid_args(
                "invalid target selector: shape ID selector cannot be empty after 'shape:'",
            ));
        }
        let id = raw.parse::<i64>().map_err(|err| {
            CliError::invalid_args(format!("invalid target selector: invalid shape ID: {err}"))
        })?;
        if id < 0 {
            return Err(CliError::invalid_args(format!(
                "invalid target selector: shape ID must be non-negative, got {id}"
            )));
        }
        return Ok(ImageBatchSelector {
            normalized: format!("shape:{id}"),
            unsupported_error: None,
        });
    }
    if let Some(name) = trimmed.strip_prefix('~') {
        if name.is_empty() {
            return Err(CliError::invalid_args(
                "invalid target selector: shape name selector cannot be empty after ~",
            ));
        }
        return Ok(ImageBatchSelector {
            normalized: format!("~{name}"),
            unsupported_error: None,
        });
    }
    let unsupported_type = if trimmed.starts_with('@') {
        match trimmed.trim_start_matches('@').trim() {
            "*" | "all-placeholders" => "*selectors.WildcardAllPlaceholdersSelector",
            "all-shapes" | "all-shapes-nonph" => "*selectors.WildcardAllShapesSelector",
            "all-pictures" => "*selectors.WildcardAllPicturesSelector",
            "all-tables" => "*selectors.WildcardAllTablesSelector",
            _ => "*selectors.PlaceholderTypeSelector",
        }
    } else if trimmed.starts_with('#') {
        "*selectors.PlaceholderIndexSelector"
    } else if is_image_batch_slide_selector(trimmed) {
        if trimmed.contains(',') || trimmed.contains('-') {
            "*selectors.SlideRangeSelector"
        } else {
            "*selectors.SlideNumberSelector"
        }
    } else {
        "*selectors.PlaceholderKeySelector"
    };
    Ok(ImageBatchSelector {
        normalized: trimmed.to_string(),
        unsupported_error: Some(format!(
            "selector type {unsupported_type} is not supported for image replacement (use shape ID or name)"
        )),
    })
}

fn is_image_batch_slide_selector(value: &str) -> bool {
    value.contains(',')
        || (!value.starts_with('-') && value.contains('-'))
        || value.chars().all(|ch| ch.is_ascii_digit())
}

fn parse_image_batch_slide_spec(value: &str) -> Result<Vec<u32>, String> {
    let spec = value.trim();
    if spec.is_empty() {
        return Err("empty specification".to_string());
    }
    let mut slides = Vec::new();
    let mut seen = BTreeSet::<u32>::new();
    for part in spec
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if part.contains('-') {
            let range_parts = part.split('-').collect::<Vec<_>>();
            if range_parts.len() != 2 {
                return Err(format!("invalid range format: {part}"));
            }
            let start_raw = range_parts[0].trim();
            let end_raw = range_parts[1].trim();
            let start = start_raw
                .parse::<i64>()
                .map_err(|_| format!("invalid range start: {start_raw}"))?;
            if start <= 0 {
                return Err(format!("invalid range start: {start_raw}"));
            }
            let end = end_raw
                .parse::<i64>()
                .map_err(|_| format!("invalid range end: {end_raw}"))?;
            if end <= 0 {
                return Err(format!("invalid range end: {end_raw}"));
            }
            if start > end {
                return Err(format!(
                    "range start ({start}) cannot be greater than end ({end})"
                ));
            }
            for slide in start as u32..=end as u32 {
                if seen.insert(slide) {
                    slides.push(slide);
                }
            }
        } else {
            let slide = part
                .parse::<i64>()
                .map_err(|_| format!("invalid slide number: {part}"))?;
            if slide <= 0 {
                return Err(format!("invalid slide number: {part}"));
            }
            let slide = slide as u32;
            if seen.insert(slide) {
                slides.push(slide);
            }
        }
    }
    slides.sort_unstable();
    Ok(slides)
}

fn image_target_not_found(target_selector: &str, slide: u32, file: &str) -> CliError {
    let candidates = pptx_slide_refs_for_replace(file)
        .ok()
        .and_then(|slides| slides.get(slide as usize - 1).cloned())
        .and_then(|slide_ref| {
            zip_text(file, &slide_ref.part)
                .ok()
                .map(|xml| (slide_ref, xml))
        })
        .map(|(slide_ref, xml)| {
            slide_targets(&xml, &slide_ref)
                .into_iter()
                .filter(|target| target.target_kind == "picture")
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let hint = candidates
        .first()
        .map(|candidate| format!("; did you mean: {}", candidate.primary_selector))
        .unwrap_or_default();
    CliError::target_not_found(format!(
        "picture shape not found: {target_selector}{hint}; discover with `ooxml --json pptx shapes show <file> --slide {slide}`"
    ))
}

fn image_replace_result_json(
    file: &str,
    target_selector: &str,
    fit_mode: &str,
    plan: &ImageReplacePlan,
    options: &PptxReplaceMutationOptions,
) -> Value {
    let output = mutation_output_path(file, options);
    let destination = image_destination_json(
        &plan.target,
        plan.slide,
        target_selector,
        output.as_deref(),
        &plan.relationship_id,
        &plan.new_target_uri,
        &plan.new_content_type,
    );
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("target".to_string(), json!(target_selector));
    result.insert("fitMode".to_string(), json!(fit_mode));
    result.insert("slideNumber".to_string(), json!(plan.slide));
    result.insert("shapeId".to_string(), json!(plan.target.shape_id));
    result.insert("shapeName".to_string(), json!(plan.target.shape_name));
    result.insert("oldTargetUri".to_string(), json!(plan.old_target_uri));
    result.insert("oldContentType".to_string(), json!(plan.old_content_type));
    result.insert("newTargetUri".to_string(), json!(plan.new_target_uri));
    result.insert("newContentType".to_string(), json!(plan.new_content_type));
    result.insert("relationshipId".to_string(), json!(plan.relationship_id));
    result.insert("destination".to_string(), destination);
    let command_target = output.as_deref().unwrap_or("<out.pptx>");
    let suffix = if options.dry_run { "Template" } else { "" };
    result.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx shapes get {} --slide {} --target {} --include-bounds",
            command_arg(command_target),
            plan.slide,
            command_arg(&plan.target.primary_selector)
        )),
    );
    add_slide_validate_render_commands(&mut result, command_target, plan.slide, options.dry_run);
    Value::Object(result)
}

fn image_destination_json(
    target: &ShapeTarget,
    slide: u32,
    requested_target: &str,
    output: Option<&str>,
    rel_id: &str,
    target_uri: &str,
    content_type: &str,
) -> Value {
    let mut result = Map::new();
    if let Some(output) = output {
        result.insert("file".to_string(), json!(output));
    }
    result.insert("slide".to_string(), json!(slide));
    result.insert("target".to_string(), json!(requested_target));
    result.insert("shapeId".to_string(), json!(target.shape_id));
    if !target.shape_name.is_empty() {
        result.insert("shapeName".to_string(), json!(target.shape_name));
    }
    result.insert("targetKind".to_string(), json!(target.target_kind));
    result.insert(
        "primarySelector".to_string(),
        json!(target.primary_selector),
    );
    if !target.handle.is_empty() {
        result.insert("handle".to_string(), json!(target.handle));
    }
    result.insert("selectors".to_string(), json!(target.selectors));
    if let Some(bounds) = target.bounds {
        result.insert(
            "bounds".to_string(),
            json!({"x": bounds.x, "y": bounds.y, "cx": bounds.cx, "cy": bounds.cy}),
        );
    }
    result.insert(
        "imageRef".to_string(),
        json!({
            "relId": rel_id,
            "targetUri": target_uri,
            "contentType": content_type,
        }),
    );
    Value::Object(result)
}

fn normalize_fit_mode(mode: &str) -> CliResult<String> {
    match mode.to_ascii_lowercase().as_str() {
        "contain" | "fit" => Ok("contain".to_string()),
        "cover" | "crop" => Ok("cover".to_string()),
        other => Err(CliError::invalid_args(format!(
            "invalid fit mode {other:?} (must be 'contain' or 'cover')"
        ))),
    }
}

fn replacement_image_uri(
    old_uri: &str,
    old_content_type: &str,
    new_content_type: &str,
) -> CliResult<String> {
    if normalized_image_content_type(old_content_type)
        == normalized_image_content_type(new_content_type)
    {
        return Ok(old_uri.to_string());
    }
    let new_ext = image_extension_for_content_type(new_content_type)?;
    let old_ext = Path::new(old_uri)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if old_ext.eq_ignore_ascii_case(new_ext.trim_start_matches('.')) {
        return Ok(old_uri.to_string());
    }
    let Some((base, _)) = old_uri.rsplit_once('.') else {
        return Ok(format!("{old_uri}{new_ext}"));
    };
    Ok(format!("{base}{new_ext}"))
}

fn image_content_type_from_path(path: &str) -> CliResult<String> {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" => Ok("image/png".to_string()),
        "jpg" | "jpeg" => Ok("image/jpeg".to_string()),
        "gif" => Ok("image/gif".to_string()),
        "bmp" => Ok("image/bmp".to_string()),
        "tif" | "tiff" => Ok("image/tiff".to_string()),
        "webp" => Ok("image/webp".to_string()),
        "svg" => Ok("image/svg+xml".to_string()),
        _ => Err(CliError::unsupported_type(format!(
            "unsupported image type for {path}; supported extensions are .png, .jpg, .jpeg, .gif, .bmp, .tif, .tiff, .webp, and .svg"
        ))),
    }
}

fn image_extension_for_content_type(content_type: &str) -> CliResult<&'static str> {
    match normalized_image_content_type(content_type).as_str() {
        "image/png" => Ok(".png"),
        "image/jpeg" | "image/jpg" | "image/pjpeg" => Ok(".jpg"),
        "image/gif" => Ok(".gif"),
        "image/bmp" => Ok(".bmp"),
        "image/tiff" => Ok(".tiff"),
        "image/webp" => Ok(".webp"),
        "image/svg+xml" => Ok(".svg"),
        other => Err(CliError::unsupported_type(format!(
            "unsupported image content type {other}"
        ))),
    }
}

fn validate_image_payload(raw: &[u8], content_type: &str) -> Result<(), String> {
    let normalized = normalized_image_content_type(content_type);
    let ok = match normalized.as_str() {
        "image/png" => raw.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" | "image/jpg" | "image/pjpeg" => {
            raw.len() >= 3 && raw[0] == 0xff && raw[1] == 0xd8 && raw[2] == 0xff
        }
        "image/gif" => raw.starts_with(b"GIF87a") || raw.starts_with(b"GIF89a"),
        "image/bmp" => raw.starts_with(b"BM"),
        "image/tiff" => raw.starts_with(b"II*\0") || raw.starts_with(b"MM\0*"),
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "image payload does not match content type {normalized}"
        ))
    }
}

fn normalized_image_content_type(content_type: &str) -> String {
    content_type
        .split_once(';')
        .map(|(head, _)| head)
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase()
}

#[derive(Clone)]
struct PptxSlideRef {
    number: u32,
    slide_id: u32,
    part: String,
    slide_id_unique: bool,
}

fn pptx_slide_refs_for_replace(file: &str) -> CliResult<Vec<PptxSlideRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slide_ids = presentation_slide_refs(&presentation);
    let rels = relationship_entries_from_xml(&zip_text(file, "ppt/_rels/presentation.xml.rels")?);
    let mut id_counts = BTreeMap::<u32, usize>::new();
    for (slide_id, _) in &slide_ids {
        *id_counts.entry(*slide_id).or_default() += 1;
    }
    slide_ids
        .into_iter()
        .enumerate()
        .map(|(index, (slide_id, rel_id))| {
            let rel = rels
                .iter()
                .find(|candidate| candidate.id == rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            let part = resolve_relationship_target("/ppt/presentation.xml", &rel.target)
                .trim_start_matches('/')
                .to_string();
            Ok(PptxSlideRef {
                number: index as u32 + 1,
                slide_id,
                part,
                slide_id_unique: id_counts.get(&slide_id).copied().unwrap_or_default() == 1,
            })
        })
        .collect()
}

fn presentation_slide_refs(xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let (Some(id), Some(rel)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    slides.push((id, rel));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

#[derive(Clone, Copy)]
struct XmlSpan {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy)]
struct Bounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

#[derive(Clone)]
struct ShapeTarget {
    shape_id: u32,
    shape_name: String,
    target_kind: String,
    primary_selector: String,
    selectors: Vec<String>,
    handle: String,
    bounds: Option<Bounds>,
    image_rel_id: String,
    span: XmlSpan,
}

impl ShapeTarget {
    fn matches_selector(&self, selector: &str) -> bool {
        self.primary_selector == selector
            || self.handle == selector
            || self.selectors.iter().any(|item| item == selector)
    }
}

#[derive(Clone)]
struct ShapeScan {
    shape_id: u32,
    shape_name: String,
    shape_type: String,
    is_placeholder: bool,
    placeholder_type: String,
    placeholder_index: Option<u32>,
    has_text_body: bool,
    has_table: bool,
    image_rel_id: String,
    bounds: Option<Bounds>,
    start: usize,
    depth_or_end: usize,
}

fn slide_targets(xml: &str, slide_ref: &PptxSlideRef) -> Vec<ShapeTarget> {
    let scans = shape_scans(xml);
    let mut name_counts = BTreeMap::<String, usize>::new();
    let mut index_counts = BTreeMap::<u32, usize>::new();
    let mut shape_id_counts = BTreeMap::<u32, usize>::new();
    for scan in &scans {
        if !scan.shape_name.trim().is_empty() {
            *name_counts.entry(scan.shape_name.clone()).or_default() += 1;
        }
        if let Some(index) = scan.placeholder_index {
            *index_counts.entry(index).or_default() += 1;
        }
        *shape_id_counts.entry(scan.shape_id).or_default() += 1;
    }
    let mut table_index = 0_u32;
    scans
        .into_iter()
        .map(|scan| {
            let is_table = scan.shape_type == "graphicFrame" && scan.has_table;
            if is_table {
                table_index += 1;
            }
            let mut placeholder_role = placeholder_role(&scan.placeholder_type);
            let mut placeholder_key = placeholder_role.clone();
            if placeholder_role.is_empty()
                && scan.shape_type == "sp"
                && scan.has_text_body
                && scan
                    .shape_name
                    .to_ascii_lowercase()
                    .contains("content placeholder")
            {
                placeholder_role = "body".to_string();
                placeholder_key = "body".to_string();
            }
            let primary_selector = if is_table {
                format!("table:{table_index}")
            } else if !placeholder_key.is_empty() {
                placeholder_key.clone()
            } else {
                format!("shape:{}", scan.shape_id)
            };
            let mut selectors = Vec::new();
            if is_table {
                add_selector(&mut selectors, format!("shape:{}", scan.shape_id));
                add_selector(&mut selectors, format!("table:{table_index}"));
            } else {
                add_selector(&mut selectors, placeholder_key.clone());
                if !placeholder_role.is_empty() {
                    add_selector(&mut selectors, format!("@{placeholder_role}"));
                    add_selector(&mut selectors, placeholder_role.clone());
                    if let Some(index) = scan.placeholder_index {
                        add_selector(&mut selectors, format!("{placeholder_role}:{index}"));
                    }
                }
                if let Some(index) = scan.placeholder_index
                    && index_counts.get(&index).copied().unwrap_or_default() == 1
                {
                    add_selector(&mut selectors, format!("#{index}"));
                }
                add_selector(&mut selectors, format!("shape:{}", scan.shape_id));
            }
            if name_counts
                .get(&scan.shape_name)
                .copied()
                .unwrap_or_default()
                == 1
            {
                add_selector(&mut selectors, format!("~{}", scan.shape_name));
            }
            let target_kind = if is_table {
                "table".to_string()
            } else if scan.shape_type == "pic" {
                "picture".to_string()
            } else if !placeholder_role.is_empty() {
                placeholder_role
            } else if scan.has_text_body {
                "textbox".to_string()
            } else if scan.is_placeholder {
                "placeholder".to_string()
            } else {
                "shape".to_string()
            };
            let handle = if slide_ref.slide_id != 0
                && slide_ref.slide_id_unique
                && shape_id_counts
                    .get(&scan.shape_id)
                    .copied()
                    .unwrap_or_default()
                    == 1
            {
                format!("H:pptx/s:{}/shape:n:{}", slide_ref.slide_id, scan.shape_id)
            } else {
                String::new()
            };
            ShapeTarget {
                shape_id: scan.shape_id,
                shape_name: scan.shape_name,
                target_kind,
                primary_selector,
                selectors,
                handle,
                bounds: scan.bounds,
                image_rel_id: scan.image_rel_id,
                span: XmlSpan {
                    start: scan.start,
                    end: scan.depth_or_end,
                },
            }
        })
        .collect()
}

fn shape_scans(xml: &str) -> Vec<ShapeScan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path = Vec::<String>::new();
    let mut current: Option<ShapeScan> = None;
    let mut shapes = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && matches!(name.as_str(), "sp" | "pic" | "graphicFrame")
                {
                    current = Some(ShapeScan {
                        shape_id: 0,
                        shape_name: String::new(),
                        shape_type: name.clone(),
                        is_placeholder: false,
                        placeholder_type: String::new(),
                        placeholder_index: None,
                        has_text_body: false,
                        has_table: false,
                        image_rel_id: String::new(),
                        bounds: None,
                        start: before,
                        depth_or_end: path.len() + 1,
                    });
                } else if let Some(scan) = current.as_mut() {
                    note_shape_element(scan, &e, &name);
                }
                path.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(scan) = current.as_mut() {
                    note_shape_element(scan, &e, &name);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(scan) = current.take() {
                    if path.len() == scan.depth_or_end && name == scan.shape_type {
                        let mut finished = scan;
                        finished.depth_or_end = reader.buffer_position() as usize;
                        shapes.push(finished);
                    } else {
                        current = Some(scan);
                    }
                }
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    shapes
}

fn note_shape_element(scan: &mut ShapeScan, e: &BytesStart<'_>, name: &str) {
    match name {
        "cNvPr" => {
            scan.shape_id = attr(e, "id")
                .and_then(|value| value.parse().ok())
                .unwrap_or(scan.shape_id);
            scan.shape_name = attr(e, "name").unwrap_or_else(|| scan.shape_name.clone());
        }
        "ph" => {
            scan.is_placeholder = true;
            scan.placeholder_type = attr(e, "type").unwrap_or_default();
            scan.placeholder_index = attr(e, "idx").and_then(|value| value.parse().ok());
        }
        "txBody" if scan.shape_type == "sp" => {
            scan.has_text_body = true;
        }
        "tbl" if scan.shape_type == "graphicFrame" => {
            scan.has_table = true;
        }
        "blip" if scan.shape_type == "pic" => {
            scan.image_rel_id = attr(e, "embed").unwrap_or_default();
        }
        "off" => {
            let mut bounds = scan.bounds.unwrap_or(Bounds {
                x: 0,
                y: 0,
                cx: 0,
                cy: 0,
            });
            bounds.x = attr(e, "x")
                .and_then(|value| value.parse().ok())
                .unwrap_or(bounds.x);
            bounds.y = attr(e, "y")
                .and_then(|value| value.parse().ok())
                .unwrap_or(bounds.y);
            scan.bounds = Some(bounds);
        }
        "ext" => {
            let mut bounds = scan.bounds.unwrap_or(Bounds {
                x: 0,
                y: 0,
                cx: 0,
                cy: 0,
            });
            bounds.cx = attr(e, "cx")
                .and_then(|value| value.parse().ok())
                .unwrap_or(bounds.cx);
            bounds.cy = attr(e, "cy")
                .and_then(|value| value.parse().ok())
                .unwrap_or(bounds.cy);
            scan.bounds = Some(bounds);
        }
        _ => {}
    }
}

fn placeholder_role(literal_type: &str) -> String {
    match literal_type {
        "ctrTitle" | "title" => "title",
        "subTitle" => "subtitle",
        "body" | "obj" => "body",
        "pic" => "picture",
        other => other,
    }
    .to_string()
}

fn add_selector(selectors: &mut Vec<String>, value: String) {
    if !value.is_empty() && !selectors.iter().any(|selector| selector == &value) {
        selectors.push(value);
    }
}

#[derive(Clone)]
struct TextNodeSpan {
    open_end: usize,
    content_start: usize,
    content_end: usize,
    before_text: String,
    has_xml_space: bool,
}

struct TextNodeReplacement {
    span: TextNodeSpan,
    after: String,
}

fn text_nodes_in_span(xml: &str, span: XmlSpan) -> CliResult<Vec<TextNodeSpan>> {
    let fragment = &xml[span.start..span.end];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut nodes = Vec::new();
    let mut current: Option<TextNodeScan> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current.is_none() && name == "t" {
                    let open_end = span.start + reader.buffer_position() as usize;
                    current = Some(TextNodeScan {
                        open_end,
                        content_start: open_end,
                        content_end: open_end,
                        text: String::new(),
                        depth: 1,
                        has_xml_space: attr_exact(&e, "xml:space")
                            .or_else(|| attr(&e, "space"))
                            .is_some(),
                    });
                } else if let Some(scan) = current.as_mut() {
                    scan.depth += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                if current.is_none() && local_name(e.name().as_ref()) == "t" {
                    let open_end = span.start + reader.buffer_position() as usize;
                    nodes.push(TextNodeSpan {
                        open_end,
                        content_start: open_end,
                        content_end: open_end,
                        before_text: String::new(),
                        has_xml_space: attr_exact(&e, "xml:space")
                            .or_else(|| attr(&e, "space"))
                            .is_some(),
                    });
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(scan) = current.as_mut() {
                    scan.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(scan) = current.as_mut() {
                    scan.text.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                if let Some(scan) = current.as_mut() {
                    if scan.depth == 1 && local_name(e.name().as_ref()) == "t" {
                        scan.content_end = span.start + before;
                        let finished = current.take().expect("text scan");
                        nodes.push(TextNodeSpan {
                            open_end: finished.open_end,
                            content_start: finished.content_start,
                            content_end: finished.content_end,
                            before_text: finished.text,
                            has_xml_space: finished.has_xml_space,
                        });
                    } else {
                        scan.depth = scan.depth.saturating_sub(1);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(nodes)
}

struct TextNodeScan {
    open_end: usize,
    content_start: usize,
    content_end: usize,
    text: String,
    depth: usize,
    has_xml_space: bool,
}

fn update_picture_fit_mode(xml: &str, picture_span: XmlSpan, fit_mode: &str) -> CliResult<String> {
    let Some(blip_fill) = find_child_element_span(xml, picture_span, "blipFill")? else {
        return Ok(xml.to_string());
    };
    let Some((content_start, content_end)) = element_content_span(xml, blip_fill, "blipFill")?
    else {
        return Ok(xml.to_string());
    };
    let children = xml_direct_child_ranges(xml, content_start, content_end)?;
    let mut replacement = String::new();
    let mut cursor = content_start;
    for child in children {
        if child.kind == "stretch" || child.kind == "tile" {
            replacement.push_str(&xml[cursor..child.start]);
            cursor = child.end;
        }
    }
    replacement.push_str(&xml[cursor..content_end]);
    if fit_mode == "cover" {
        replacement
            .push_str(r#"<a:tile tx="0" ty="0" sx="100000" sy="100000" flip="none" algn="ctr"/>"#);
    } else {
        replacement.push_str("<a:stretch><a:fillRect/></a:stretch>");
    }
    Ok(replace_xml_span(
        xml,
        content_start,
        content_end,
        &replacement,
    ))
}

fn find_child_element_span(
    xml: &str,
    outer: XmlSpan,
    wanted_local: &str,
) -> CliResult<Option<XmlSpan>> {
    let fragment = &xml[outer.start..outer.end];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    let mut depth = 0_usize;
    let mut found: Option<(usize, usize)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if found.is_none() && name == wanted_local {
                    found = Some((outer.start + before, depth + 1));
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if found.is_none() && local_name(e.name().as_ref()) == wanted_local {
                    return Ok(Some(XmlSpan {
                        start: outer.start + before,
                        end: outer.start + reader.buffer_position() as usize,
                    }));
                }
            }
            Ok(Event::End(e)) => {
                if let Some((start, wanted_depth)) = found
                    && depth == wanted_depth
                    && local_name(e.name().as_ref()) == wanted_local
                {
                    return Ok(Some(XmlSpan {
                        start,
                        end: outer.start + reader.buffer_position() as usize,
                    }));
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(None)
}

fn element_content_span(
    xml: &str,
    outer: XmlSpan,
    wanted_local: &str,
) -> CliResult<Option<(usize, usize)>> {
    let fragment = &xml[outer.start..outer.end];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    let mut depth = 0_usize;
    let mut open_end = 0_usize;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if depth == 0 && local_name(e.name().as_ref()) == wanted_local => {
                open_end = reader.buffer_position() as usize;
                depth = 1;
            }
            Ok(Event::Start(_)) if depth > 0 => depth += 1,
            Ok(Event::End(e)) if depth > 0 => {
                if depth == 1 && local_name(e.name().as_ref()) == wanted_local {
                    return Ok(Some((outer.start + open_end, outer.start + before)));
                }
                depth -= 1;
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(None)
}

fn render_relationships_xml(rels: &[RelationshipEntry]) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for rel in rels {
        out.push_str(&format!(
            r#"<Relationship Id="{}" Type="{}" Target="{}""#,
            xml_attr_escape(&rel.id),
            xml_attr_escape(&rel.rel_type),
            xml_attr_escape(&rel.target)
        ));
        if !rel.target_mode.is_empty() {
            out.push_str(&format!(
                r#" TargetMode="{}""#,
                xml_attr_escape(&rel.target_mode)
            ));
        }
        out.push_str("/>");
    }
    out.push_str("</Relationships>");
    out
}

fn write_replace_mutation(
    file: &str,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    options: &PptxReplaceMutationOptions,
) -> CliResult<()> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-replace")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    if binary_overrides.is_empty() {
        copy_zip_with_part_overrides(file, &write_path, text_overrides)?;
    } else {
        copy_zip_with_binary_part_overrides_and_removals(
            file,
            &write_path,
            text_overrides,
            binary_overrides,
            &BTreeSet::new(),
        )?;
    }
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&write_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&write_path, file)
            .or_else(|_| {
                fs::copy(&write_path, file)?;
                fs::remove_file(&write_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

fn mutation_output_path(file: &str, options: &PptxReplaceMutationOptions) -> Option<String> {
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

fn add_output_verification_commands(
    result: &mut Map<String, Value>,
    output: Option<&str>,
    dry_run: bool,
) {
    let command_target = output.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    result.insert(
        format!("validateCommand{suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
}

fn add_slide_validate_render_commands(
    result: &mut Map<String, Value>,
    command_target: &str,
    slide: u32,
    dry_run: bool,
) {
    let suffix = if dry_run { "Template" } else { "" };
    result.insert(
        format!("slideReadbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(command_target),
            slide
        )),
    );
    add_output_verification_commands(result, Some(command_target), dry_run);
}

fn sha256_string(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
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
