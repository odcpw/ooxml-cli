use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use super::output::{
    ensure_pptx, text_from_xlsx_result_json, text_map_from_xlsx_result_json, write_replace_mutation,
};
use super::{
    PptxReplaceMutationOptions, PptxSlideRef, ShapeTarget, TextNodeReplacement,
    apply_text_node_replacements, pptx_slide_refs_for_replace, slide_targets, text_nodes_in_span,
};
use crate::{
    CliError, CliResult, XlsxRangeExportOptions, check_range_max_cells, parse_cli_range,
    parse_range, range_bounds_ref, select_xlsx_table, xlsx_range_export_with_options, xlsx_tables,
    zip_text,
};
pub(super) struct ReplaceTextFromXlsxRequest {
    pub(super) slide: u32,
    pub(super) target: String,
    pub(super) source: XlsxTextSource,
    pub(super) text: String,
    pub(super) mode: String,
    pub(super) formula_mode: String,
    pub(super) row_sep: String,
    pub(super) col_sep: String,
}

pub(super) struct ReplaceTextMapFromXlsxRequest {
    pub(super) source: XlsxTextSource,
    pub(super) records: Vec<TextMapRecord>,
    pub(super) columns: TextMapColumns,
    pub(super) mode: String,
    pub(super) formula_mode: String,
}

pub(super) struct XlsxTextSource {
    pub(super) source: Value,
    pub(super) data: Vec<Vec<String>>,
    pub(super) rows: usize,
    pub(super) range: String,
}

#[derive(Clone)]
pub(super) struct TextTargetReplacePlan {
    pub(super) slide: u32,
    pub(super) slide_part: String,
    pub(super) slide_xml: String,
    pub(super) target: ShapeTarget,
    pub(super) text: String,
}

#[derive(Clone)]
pub(super) struct TextMapRecord {
    pub(super) source_row: usize,
    pub(super) slide: u32,
    pub(super) target: String,
    pub(super) text: String,
}

pub(super) struct TextMapApplied {
    pub(super) record: TextMapRecord,
    pub(super) plan: TextTargetReplacePlan,
}

pub(super) struct TextMapColumns {
    pub(super) slide: String,
    pub(super) target: String,
    pub(super) text: String,
}

pub(super) fn replace_text_from_xlsx(
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

pub(super) fn replace_text_map_from_xlsx(
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

pub(super) fn load_xlsx_text_range_source(
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

pub(super) fn load_xlsx_text_range_or_table_source(
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

pub(super) fn join_xlsx_text_matrix(
    values: &[Vec<String>],
    row_sep: &str,
    col_sep: &str,
) -> String {
    values
        .iter()
        .map(|row| row.join(col_sep))
        .collect::<Vec<_>>()
        .join(row_sep)
}

pub(super) fn normalize_xlsx_formula_mode(
    value: Option<&str>,
    flag_name: &str,
) -> CliResult<String> {
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

pub(super) fn normalize_replace_text_from_xlsx_mode(value: Option<&str>) -> CliResult<String> {
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

pub(super) fn decode_text_separator_flag(value: &str, flag_name: &str) -> CliResult<String> {
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

pub(super) fn check_expected_xlsx_source_range(
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

pub(super) fn text_map_records_from_values(
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

pub(super) fn build_text_target_replace_plan(
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

pub(super) fn map_text_target_error(err: CliError, target: &str) -> CliError {
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
