use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, WorkbookSheet, attr, col_name, command_arg, copy_zip_with_part_override,
    local_name, normalize_xl_target, parse_xlsx_row_spans, relationships, render_xml_attrs,
    replace_xml_span, resolve_sheet, validate, validate_xlsx_mutation_output_flags,
    workbook_sheets, xlsx_ranges_set_temp_path, xlsx_sheet_data_span, xml_attrs,
    xml_direct_child_ranges, xml_fragment_bounds, xml_open_tag_from_start, xml_tag_prefix,
    zip_text,
};

const DEFAULT_COL_WIDTH: f64 = 8.43;
const DEFAULT_ROW_HEIGHT: f64 = 15.0;
const XLSX_MAX_COL: u32 = 16_384;
const MAX_COLUMN_WIDTH: f64 = 255.0;
const MAX_ROW_HEIGHT: f64 = 409.0;
const DIMENSION_TOLERANCE: f64 = 1e-6;

pub(crate) struct XlsxColWidthsSetOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: &'a str,
    pub(crate) width: Option<f64>,
    pub(crate) expect_width: Option<f64>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxRowHeightsSetOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: &'a str,
    pub(crate) height: Option<f64>,
    pub(crate) expect_height: Option<f64>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone, Copy)]
struct ColumnWidthInfo {
    width: f64,
    explicit: bool,
    custom: bool,
    hidden: bool,
}

impl ColumnWidthInfo {
    fn default_with(width: f64) -> Self {
        Self {
            width,
            explicit: false,
            custom: false,
            hidden: false,
        }
    }
}

#[derive(Clone, Copy)]
struct RowHeightInfo {
    height: f64,
    explicit: bool,
    custom: bool,
    hidden: bool,
}

impl RowHeightInfo {
    fn default_with(height: f64) -> Self {
        Self {
            height,
            explicit: false,
            custom: false,
            hidden: false,
        }
    }
}

pub(crate) fn xlsx_colwidths_show(
    file: &str,
    sheet_selector: Option<&str>,
    range: &str,
) -> CliResult<Value> {
    let (min_col, max_col) = parse_column_span(range)?;
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector.unwrap_or("1"))?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (widths, fallback) = read_column_widths(&sheet_xml, min_col, max_col)?;
    let min_column = col_name(min_col);
    let max_column = col_name(max_col);
    let normalized_range = format!("{min_column}:{max_column}");
    let mut columns = BTreeMap::new();
    let mut distinct = Vec::new();
    for col in min_col..=max_col {
        let info = widths
            .get(&col)
            .copied()
            .unwrap_or_else(|| ColumnWidthInfo::default_with(fallback));
        distinct = append_distinct_float(distinct, info.width);
        columns.insert(
            col_name(col),
            json!({
                "width": dimension_json(info.width),
                "explicit": info.explicit,
                "custom": info.custom,
                "hidden": info.hidden,
            }),
        );
    }
    Ok(json!({
        "file": file,
        "sheet": sheet.name,
        "sheetNumber": sheet.position,
        "range": normalized_range,
        "minColumn": min_column,
        "maxColumn": max_column,
        "count": max_col - min_col + 1,
        "defaultWidth": dimension_json(fallback),
        "uniform": distinct.len() <= 1,
        "columns": columns,
        "colwidthsSetCommandTemplate": colwidths_set_command_template(file, &sheet, &normalized_range),
    }))
}

pub(crate) fn xlsx_rowheights_show(
    file: &str,
    sheet_selector: Option<&str>,
    range: &str,
) -> CliResult<Value> {
    let (min_row, max_row) = parse_row_span(range)?;
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector.unwrap_or("1"))?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (heights, fallback) = read_row_heights(&sheet_xml, min_row, max_row)?;
    let normalized_range = format!("{min_row}:{max_row}");
    let mut rows = BTreeMap::new();
    let mut distinct = Vec::new();
    for row in min_row..=max_row {
        let info = heights
            .get(&row)
            .copied()
            .unwrap_or_else(|| RowHeightInfo::default_with(fallback));
        distinct = append_distinct_float(distinct, info.height);
        rows.insert(
            row.to_string(),
            json!({
                "height": dimension_json(info.height),
                "explicit": info.explicit,
                "custom": info.custom,
                "hidden": info.hidden,
            }),
        );
    }
    Ok(json!({
        "file": file,
        "sheet": sheet.name,
        "sheetNumber": sheet.position,
        "range": normalized_range,
        "minRow": min_row,
        "maxRow": max_row,
        "count": max_row - min_row + 1,
        "defaultHeight": dimension_json(fallback),
        "uniform": distinct.len() <= 1,
        "rows": rows,
        "rowheightsSetCommandTemplate": rowheights_set_command_template(file, &sheet, &normalized_range),
    }))
}

pub(crate) fn xlsx_colwidths_set(
    file: &str,
    options: XlsxColWidthsSetOptions<'_>,
) -> CliResult<Value> {
    let width = options
        .width
        .ok_or_else(|| CliError::invalid_args("--width is required"))?;
    let (min_col, max_col) = parse_column_span(options.range)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let (sheet, sheet_part, sheet_xml) = resolve_dimension_sheet(file, options.sheet)?;
    if let Some(expect) = options.expect_width {
        let (current, _) = read_column_widths(&sheet_xml, min_col, min_col)?;
        let found = current
            .get(&min_col)
            .map(|info| info.width)
            .unwrap_or_else(|| default_column_width(&sheet_xml));
        if (found - expect).abs() > DIMENSION_TOLERANCE {
            return Err(CliError::invalid_args(format!(
                "expected width {} but found {}",
                format_dimension_4g(expect),
                format_dimension_4g(found)
            )));
        }
    }
    let updated_xml =
        set_column_widths_xml(&sheet_xml, min_col, max_col, width).map_err(|err| {
            CliError::invalid_args(format!("failed to set column widths: {}", err.message))
        })?;
    let output_path = write_dimension_mutation_output(
        file,
        &sheet_part,
        &updated_xml,
        DimensionMutationWriteOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )?;
    let min_column = col_name(min_col);
    let max_column = col_name(max_col);
    let normalized_range = format!("{min_column}:{max_column}");
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("range".to_string(), json!(normalized_range));
    result.insert("minColumn".to_string(), json!(min_column));
    result.insert("maxColumn".to_string(), json!(max_column));
    result.insert("columns".to_string(), json!(max_col - min_col + 1));
    result.insert("width".to_string(), dimension_json(width));
    if let Some(output_path) = output_path.as_deref() {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    if let Some(output_path) = output_path.as_deref() {
        let selector = xlsx_sheet_selector(&sheet);
        result.insert(
            "validateCommand".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(output_path)
            )),
        );
        result.insert(
            "colwidthsShowCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx colwidths show {} --sheet {} --range {}",
                command_arg(output_path),
                command_arg(&selector),
                command_arg(&normalized_range)
            )),
        );
    }
    Ok(Value::Object(result))
}

pub(crate) fn xlsx_rowheights_set(
    file: &str,
    options: XlsxRowHeightsSetOptions<'_>,
) -> CliResult<Value> {
    let height = options
        .height
        .ok_or_else(|| CliError::invalid_args("--height is required"))?;
    let (min_row, max_row) = parse_row_span(options.range)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let (sheet, sheet_part, sheet_xml) = resolve_dimension_sheet(file, options.sheet)?;
    if let Some(expect) = options.expect_height {
        let (current, _) = read_row_heights(&sheet_xml, min_row, min_row)?;
        let found = current
            .get(&min_row)
            .map(|info| info.height)
            .unwrap_or_else(|| default_row_height(&sheet_xml));
        if (found - expect).abs() > DIMENSION_TOLERANCE {
            return Err(CliError::invalid_args(format!(
                "expected height {} but found {}",
                format_dimension_4g(expect),
                format_dimension_4g(found)
            )));
        }
    }
    let (updated_xml, created) = set_row_heights_xml(&sheet_xml, min_row, max_row, height)
        .map_err(|err| {
            CliError::invalid_args(format!("failed to set row heights: {}", err.message))
        })?;
    let output_path = write_dimension_mutation_output(
        file,
        &sheet_part,
        &updated_xml,
        DimensionMutationWriteOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )?;
    let normalized_range = format!("{min_row}:{max_row}");
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("range".to_string(), json!(normalized_range));
    result.insert("minRow".to_string(), json!(min_row));
    result.insert("maxRow".to_string(), json!(max_row));
    result.insert("rows".to_string(), json!(max_row - min_row + 1));
    result.insert("created".to_string(), json!(created));
    result.insert("height".to_string(), dimension_json(height));
    if let Some(output_path) = output_path.as_deref() {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    if let Some(output_path) = output_path.as_deref() {
        let selector = xlsx_sheet_selector(&sheet);
        result.insert(
            "validateCommand".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(output_path)
            )),
        );
        result.insert(
            "rowheightsShowCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx rowheights show {} --sheet {} --range {}",
                command_arg(output_path),
                command_arg(&selector),
                command_arg(&normalized_range)
            )),
        );
    }
    Ok(Value::Object(result))
}

fn colwidths_set_command_template(file: &str, sheet: &WorkbookSheet, range: &str) -> String {
    format!(
        "ooxml xlsx colwidths set {} --sheet {} --range {} --width <width> --in-place",
        command_arg(file),
        command_arg(&xlsx_sheet_selector(sheet)),
        command_arg(range)
    )
}

fn rowheights_set_command_template(file: &str, sheet: &WorkbookSheet, range: &str) -> String {
    format!(
        "ooxml xlsx rowheights set {} --sheet {} --range {} --height <height> --in-place",
        command_arg(file),
        command_arg(&xlsx_sheet_selector(sheet)),
        command_arg(range)
    )
}

fn xlsx_sheet_selector(sheet: &WorkbookSheet) -> String {
    format!("sheetId:{}", sheet.sheet_id)
}

fn resolve_dimension_sheet(
    file: &str,
    sheet_selector: Option<&str>,
) -> CliResult<(WorkbookSheet, String, String)> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector.unwrap_or("1"))?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let sheet_xml = zip_text(file, &sheet_part)?;
    Ok((sheet, sheet_part, sheet_xml))
}

fn read_column_widths(
    xml: &str,
    min_col: u32,
    max_col: u32,
) -> CliResult<(BTreeMap<u32, ColumnWidthInfo>, f64)> {
    let fallback = default_column_width(xml);
    let mut widths = BTreeMap::new();
    for col in min_col..=max_col {
        widths.insert(col, ColumnWidthInfo::default_with(fallback));
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut in_cols = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "cols" => {
                in_cols = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "cols" => {
                in_cols = false;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_cols && local_name(e.name().as_ref()) == "col" =>
            {
                apply_column_width_span(&e, min_col, max_col, fallback, &mut widths);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok((widths, fallback))
}

fn read_row_heights(
    xml: &str,
    min_row: u32,
    max_row: u32,
) -> CliResult<(BTreeMap<u32, RowHeightInfo>, f64)> {
    let fallback = default_row_height(xml);
    let mut heights = BTreeMap::new();
    for row in min_row..=max_row {
        heights.insert(row, RowHeightInfo::default_with(fallback));
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut in_sheet_data = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                in_sheet_data = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                in_sheet_data = false;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_sheet_data && local_name(e.name().as_ref()) == "row" =>
            {
                apply_row_height(&e, min_row, max_row, fallback, &mut heights);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok((heights, fallback))
}

fn set_column_widths_xml(xml: &str, min_col: u32, max_col: u32, width: f64) -> CliResult<String> {
    if min_col < 1 || max_col < min_col {
        return Err(CliError::invalid_args(format!(
            "invalid column span {min_col}:{max_col}"
        )));
    }
    if !(0.0..=MAX_COLUMN_WIDTH).contains(&width) {
        return Err(CliError::invalid_args(format!(
            "width {} out of range 0-{}",
            format_dimension_4g(width),
            format_dimension_4g(MAX_COLUMN_WIDTH)
        )));
    }
    let root = worksheet_root_bounds(xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let cols_range = direct_worksheet_child_range(xml, &root, "cols")?;
    let existing = if let Some(cols_range) = cols_range.as_ref() {
        parse_column_elements(&xml[cols_range.start..cols_range.end])?
    } else {
        Vec::new()
    };
    let rebuilt_cols = rebuild_column_elements(&prefix, &existing, min_col, max_col, width);
    let cols_xml = format!(
        "<{}>{}</{}>",
        element_name(&prefix, "cols"),
        rebuilt_cols,
        element_name(&prefix, "cols")
    );
    if let Some(cols_range) = cols_range {
        return Ok(replace_xml_span(
            xml,
            cols_range.start,
            cols_range.end,
            &cols_xml,
        ));
    }
    insert_worksheet_child(xml, &root, "cols", &cols_xml)
}

fn set_row_heights_xml(
    xml: &str,
    min_row: u32,
    max_row: u32,
    height: f64,
) -> CliResult<(String, u32)> {
    if min_row < 1 || max_row < min_row {
        return Err(CliError::invalid_args(format!(
            "invalid row span {min_row}:{max_row}"
        )));
    }
    if !(0.0..=MAX_ROW_HEIGHT).contains(&height) {
        return Err(CliError::invalid_args(format!(
            "height {} out of range 0-{}",
            format_dimension_4g(height),
            format_dimension_4g(MAX_ROW_HEIGHT)
        )));
    }
    let xml = ensure_sheet_data_xml(xml)?;
    let sheet_data = xlsx_sheet_data_span(&xml)?;
    let row_spans = parse_xlsx_row_spans(&xml, sheet_data.as_ref())?;
    let mut changed_rows = BTreeMap::new();
    let mut created = 0;
    for row in min_row..=max_row {
        let mut attrs = row_spans
            .get(&row)
            .map(|span| span.attrs.clone())
            .unwrap_or_default();
        if !row_spans.contains_key(&row) {
            created += 1;
        }
        attrs.insert("r".to_string(), row.to_string());
        attrs.insert("ht".to_string(), format_dimension(height));
        attrs.insert("customHeight".to_string(), "1".to_string());
        let cells = row_spans
            .get(&row)
            .map(|span| {
                span.cells
                    .values()
                    .map(|cell| cell.xml.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        changed_rows.insert(row, render_row_with_attrs(&attrs, cells));
    }
    let updated =
        crate::rebuild_xlsx_sheet_data(&xml, sheet_data.as_ref(), &row_spans, &changed_rows)?;
    Ok((updated, created))
}

fn write_dimension_mutation_output(
    file: &str,
    sheet_part: &str,
    updated_xml: &str,
    options: DimensionMutationWriteOptions<'_>,
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

#[derive(Clone)]
struct ColumnElement {
    min: Option<u32>,
    max: Option<u32>,
    attrs: BTreeMap<String, String>,
    xml: String,
}

#[derive(Clone)]
struct WidthSegment {
    min: u32,
    max: u32,
    base: Option<ColumnElement>,
}

struct DimensionMutationWriteOptions<'a> {
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
    in_place: bool,
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
    insert_worksheet_child(
        xml,
        &root,
        "sheetData",
        &format!(
            "<{}></{}>",
            element_name(&prefix, "sheetData"),
            element_name(&prefix, "sheetData")
        ),
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

fn parse_column_elements(fragment: &str) -> CliResult<Vec<ColumnElement>> {
    let (open_end, _, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(Vec::new());
    }
    let content = &fragment[open_end + 1..close_start];
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(false);
    let mut columns = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "col" => loop {
                match reader.read_event() {
                    Ok(Event::End(e)) if local_name(e.name().as_ref()) == "col" => {
                        let start = open_end + 1 + before;
                        let end = open_end + 1 + reader.buffer_position() as usize;
                        columns.push(column_element_from_xml(&fragment[start..end])?);
                        break;
                    }
                    Ok(Event::Eof) => return Err(CliError::unexpected("col has no closing tag")),
                    Err(err) => return Err(CliError::unexpected(err.to_string())),
                    _ => {}
                }
            },
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "col" => {
                let start = open_end + 1 + before;
                let end = open_end + 1 + reader.buffer_position() as usize;
                columns.push(column_element_from_xml(&fragment[start..end])?);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(columns)
}

fn column_element_from_xml(xml: &str) -> CliResult<ColumnElement> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "col" => {
                let attrs = xml_attrs(&e);
                let min = attrs.get("min").and_then(|value| value.parse::<u32>().ok());
                let max = attrs.get("max").and_then(|value| value.parse::<u32>().ok());
                let valid = min.zip(max).is_some_and(|(min, max)| max >= min);
                return Ok(ColumnElement {
                    min: valid.then(|| min.unwrap()),
                    max: valid.then(|| max.unwrap()),
                    attrs,
                    xml: xml.to_string(),
                });
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("col element not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn rebuild_column_elements(
    prefix: &str,
    existing: &[ColumnElement],
    min_col: u32,
    max_col: u32,
    width: f64,
) -> String {
    let mut rebuilt: Vec<(u32, String)> = Vec::new();
    let mut malformed = Vec::new();
    for col in existing {
        let Some((span_min, span_max)) = column_element_bounds(col) else {
            malformed.push(col.xml.clone());
            continue;
        };
        if span_min < min_col {
            let mut attrs = col.attrs.clone();
            attrs.insert("min".to_string(), span_min.to_string());
            attrs.insert("max".to_string(), span_max.min(min_col - 1).to_string());
            rebuilt.push((span_min, render_col(prefix, attrs)));
        }
        if span_max > max_col {
            let start = span_min.max(max_col + 1);
            let mut attrs = col.attrs.clone();
            attrs.insert("min".to_string(), start.to_string());
            attrs.insert("max".to_string(), span_max.to_string());
            rebuilt.push((start, render_col(prefix, attrs)));
        }
    }
    for segment in target_width_segments(existing, min_col, max_col) {
        let mut attrs = segment
            .base
            .as_ref()
            .map(|base| base.attrs.clone())
            .unwrap_or_default();
        attrs.insert("min".to_string(), segment.min.to_string());
        attrs.insert("max".to_string(), segment.max.to_string());
        attrs.insert("width".to_string(), format_dimension(width));
        attrs.insert("customWidth".to_string(), "1".to_string());
        rebuilt.push((segment.min, render_col(prefix, attrs)));
    }
    rebuilt.sort_by_key(|(min, _)| *min);
    let mut out = String::new();
    for (_, xml) in rebuilt {
        out.push_str(&xml);
    }
    for xml in malformed {
        out.push_str(&xml);
    }
    out
}

fn target_width_segments(
    existing: &[ColumnElement],
    target_min: u32,
    target_max: u32,
) -> Vec<WidthSegment> {
    let mut spans = existing
        .iter()
        .filter_map(|col| {
            let (min, max) = column_element_bounds(col)?;
            let lo = min.max(target_min);
            let hi = max.min(target_max);
            (lo <= hi).then(|| WidthSegment {
                min: lo,
                max: hi,
                base: Some(col.clone()),
            })
        })
        .collect::<Vec<_>>();
    spans.sort_by_key(|span| span.min);
    let mut segments = Vec::new();
    let mut cursor = target_min;
    for mut span in spans {
        if span.min < cursor {
            span.min = cursor;
        }
        if span.min > span.max {
            continue;
        }
        if span.min > cursor {
            segments.push(WidthSegment {
                min: cursor,
                max: span.min - 1,
                base: None,
            });
        }
        cursor = span.max + 1;
        segments.push(span);
    }
    if cursor <= target_max {
        segments.push(WidthSegment {
            min: cursor,
            max: target_max,
            base: None,
        });
    }
    segments
}

fn column_element_bounds(col: &ColumnElement) -> Option<(u32, u32)> {
    let min = col.min?;
    let max = col.max?;
    (max >= min).then_some((min, max))
}

fn render_col(prefix: &str, attrs: BTreeMap<String, String>) -> String {
    format!(
        "<{}{}/>",
        element_name(prefix, "col"),
        render_xml_attrs(&attrs)
    )
}

fn render_row_with_attrs(attrs: &BTreeMap<String, String>, cells: Vec<String>) -> String {
    let mut out = format!("<row{}>", render_xml_attrs(attrs));
    for cell in cells {
        out.push_str(&cell);
    }
    out.push_str("</row>");
    out
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

fn default_column_width(xml: &str) -> f64 {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sheetFormatPr" =>
            {
                return attr(&e, "defaultColWidth")
                    .and_then(|value| value.parse::<f64>().ok())
                    .unwrap_or(DEFAULT_COL_WIDTH);
            }
            Ok(Event::Eof) => return DEFAULT_COL_WIDTH,
            Err(_) => return DEFAULT_COL_WIDTH,
            _ => {}
        }
    }
}

fn default_row_height(xml: &str) -> f64 {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sheetFormatPr" =>
            {
                return attr(&e, "defaultRowHeight")
                    .and_then(|value| value.parse::<f64>().ok())
                    .unwrap_or(DEFAULT_ROW_HEIGHT);
            }
            Ok(Event::Eof) => return DEFAULT_ROW_HEIGHT,
            Err(_) => return DEFAULT_ROW_HEIGHT,
            _ => {}
        }
    }
}

fn apply_column_width_span(
    element: &BytesStart<'_>,
    min_col: u32,
    max_col: u32,
    fallback: f64,
    widths: &mut BTreeMap<u32, ColumnWidthInfo>,
) {
    let Some((span_min, span_max)) = column_span_bounds(element) else {
        return;
    };
    let width = attr(element, "width").and_then(|value| value.parse::<f64>().ok());
    let custom = attr(element, "customWidth").as_deref() == Some("1");
    let hidden = attr(element, "hidden").as_deref() == Some("1");
    for col in span_min..=span_max {
        if col < min_col || col > max_col {
            continue;
        }
        widths.insert(
            col,
            ColumnWidthInfo {
                width: width.unwrap_or(fallback),
                explicit: width.is_some(),
                custom,
                hidden,
            },
        );
    }
}

fn apply_row_height(
    element: &BytesStart<'_>,
    min_row: u32,
    max_row: u32,
    fallback: f64,
    heights: &mut BTreeMap<u32, RowHeightInfo>,
) {
    let Some(row) = row_number(element) else {
        return;
    };
    if row < min_row || row > max_row {
        return;
    }
    let height = attr(element, "ht").and_then(|value| value.parse::<f64>().ok());
    heights.insert(
        row,
        RowHeightInfo {
            height: height.unwrap_or(fallback),
            explicit: height.is_some(),
            custom: attr(element, "customHeight").as_deref() == Some("1"),
            hidden: attr(element, "hidden").as_deref() == Some("1"),
        },
    );
}

fn column_span_bounds(element: &BytesStart<'_>) -> Option<(u32, u32)> {
    let min_col = attr(element, "min")?.parse::<u32>().ok()?;
    let max_col = attr(element, "max")?.parse::<u32>().ok()?;
    (max_col >= min_col).then_some((min_col, max_col))
}

fn row_number(element: &BytesStart<'_>) -> Option<u32> {
    attr(element, "r")?.parse::<u32>().ok()
}

fn parse_column_span(value: &str) -> CliResult<(u32, u32)> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args(
            "--range is required (e.g. B or B:D)",
        ));
    }
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() > 2 {
        return Err(CliError::invalid_args(format!(
            "invalid column range: {value}"
        )));
    }
    let first = parse_column(parts[0])?;
    let second = if let Some(part) = parts.get(1) {
        parse_column(part)?
    } else {
        first
    };
    Ok((first.min(second), first.max(second)))
}

fn parse_row_span(value: &str) -> CliResult<(u32, u32)> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args(
            "--range is required (e.g. 2 or 2:5)",
        ));
    }
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() > 2 {
        return Err(CliError::invalid_args(format!(
            "invalid row range: {value}"
        )));
    }
    let first = parse_row(parts[0])?;
    let second = if let Some(part) = parts.get(1) {
        parse_row(part)?
    } else {
        first
    };
    Ok((first.min(second), first.max(second)))
}

fn parse_column(value: &str) -> CliResult<u32> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args("invalid column \"\""));
    }
    if !value.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(CliError::invalid_args(format!(
            "invalid column {value:?}: invalid column reference {value:?}"
        )));
    }
    let mut col = 0u32;
    for ch in value.chars() {
        col = col * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        if col > XLSX_MAX_COL {
            return Err(CliError::invalid_args(format!(
                "column {value:?} out of XLSX bounds A-XFD"
            )));
        }
    }
    Ok(col)
}

fn parse_row(value: &str) -> CliResult<u32> {
    let trimmed = value.trim();
    match trimmed.parse::<u32>() {
        Ok(row) if row >= 1 => Ok(row),
        _ => Err(CliError::invalid_args(format!("invalid row {value:?}"))),
    }
}

fn append_distinct_float(mut values: Vec<f64>, value: f64) -> Vec<f64> {
    if values
        .iter()
        .any(|existing| (existing - value).abs() <= DIMENSION_TOLERANCE)
    {
        return values;
    }
    values.push(value);
    values
}

fn format_dimension(value: f64) -> String {
    value.to_string()
}

fn format_dimension_4g(value: f64) -> String {
    format_dimension(value)
}

fn dimension_json(value: f64) -> Value {
    if value.is_finite() && value.fract().abs() <= DIMENSION_TOLERANCE {
        json!(value as i64)
    } else {
        json!(value)
    }
}
