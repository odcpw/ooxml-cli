use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::xlsx_mutation::XlsxMatrixCell;
use crate::xlsx_mutation::{XlsxRangesSetStyleOptions, xlsx_ranges_set_style};
use crate::xlsx_sheet_xml::{
    XlsxWorksheetRootBounds as WorksheetRootBounds,
    xlsx_direct_worksheet_child_range as direct_worksheet_child_range,
    xlsx_worksheet_root_bounds as worksheet_root_bounds,
};
use crate::{
    CellValue, CliError, CliResult, RangeBounds, RelationshipEntry, WorkbookSheet, XlsxTableRef,
    add_xlsx_formula_recalc_package_updates, allocate_relationship_id, append_relationship_xml,
    command_arg, copy_zip_with_part_overrides_and_removals, ensure_content_type_override,
    local_name, normalize_xl_target, parse_range, range_bounds_ref,
    reject_xlsx_merged_cell_intersection, relationship_entries_from_xml,
    relationship_target_from_source_to_target, relationships, relationships_part_for,
    replace_xml_span, resolve_sheet, shared_strings, sheet_cells,
    validate_xlsx_mutation_output_flags, workbook_sheets, xlsx_range_destination_json,
    xlsx_sheet_selectors, xlsx_source_command, xlsx_styles, xlsx_tables, xml_attr_escape,
    xml_direct_child_ranges, xml_open_tag_from_start, xml_tag_prefix, zip_entry_names, zip_text,
};

const OFFICE_R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_TYPE_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/table";
const CONTENT_TYPE_TABLE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml";

pub(crate) struct XlsxTablesCreateOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: Option<&'a str>,
    pub(crate) table: Option<&'a str>,
    pub(crate) style: Option<&'a str>,
    pub(crate) header_style: Option<&'a str>,
    pub(crate) total_row: bool,
    pub(crate) totals: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

struct XlsxTableCreateTarget {
    sheet: WorkbookSheet,
    sheet_part: String,
    sheet_xml: String,
    range: RangeBounds,
    range_ref: String,
    headers: Vec<String>,
    table_name: String,
    style_name: String,
    table_id: u32,
    table_part: String,
    rel_id: String,
    header_style: Option<String>,
    total_row: bool,
    totals: Vec<Option<XlsxTableTotal>>,
}

#[derive(Clone)]
struct XlsxTableTotal {
    function: String,
}

pub(crate) fn xlsx_tables_create(
    file: &str,
    options: XlsxTablesCreateOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let target = resolve_xlsx_table_create_target(file, &options)?;
    let sheet_rels_part = relationships_part_for(&target.sheet_part);
    let sheet_rels_xml =
        zip_text(file, &sheet_rels_part).unwrap_or_else(|_| relationships_template());
    let table_target =
        relationship_target_from_source_to_target(&target.sheet_part, &target.table_part);
    let updated_rels = append_relationship_xml(
        sheet_rels_xml,
        &RelationshipEntry::new(&target.rel_id, REL_TYPE_TABLE, &table_target),
    );

    let (sheet_with_totals, total_stats) = write_xlsx_table_total_row(&target)?;
    let updated_sheet_xml = add_xlsx_table_part_to_worksheet(&sheet_with_totals, &target.rel_id)?;
    let table_xml = render_xlsx_table_xml(&target)?;
    let content_types = ensure_content_type_override(
        zip_text(file, "[Content_Types].xml")?,
        &target.table_part,
        CONTENT_TYPE_TABLE,
    )?;

    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = crate::mutation_staging_path(file, output_path, "xlsx-table-create");

    let mut overrides = BTreeMap::new();
    let mut removals = BTreeSet::new();
    overrides.insert(target.sheet_part.clone(), updated_sheet_xml);
    overrides.insert(sheet_rels_part, updated_rels);
    overrides.insert(target.table_part.clone(), table_xml.clone());
    add_xlsx_formula_recalc_package_updates(
        file,
        total_stats.formula_seen,
        total_stats.formula_invalidated,
        &mut overrides,
        &mut removals,
    )?;
    let content_types = overrides
        .remove("[Content_Types].xml")
        .unwrap_or(content_types);
    overrides.insert(
        "[Content_Types].xml".to_string(),
        ensure_content_type_override(content_types, &target.table_part, CONTENT_TYPE_TABLE)?,
    );
    copy_zip_with_part_overrides_and_removals(file, &readback_path, &overrides, &removals)?;
    let header_style_result = if let Some(header_style) = target.header_style.as_deref() {
        let header_range = range_bounds_ref(RangeBounds {
            start_col: target.range.start_col,
            end_col: target.range.end_col,
            start_row: target.range.start_row,
            end_row: target.range.start_row,
        });
        Some(xlsx_ranges_set_style(
            &readback_path,
            XlsxRangesSetStyleOptions {
                sheet: &target.sheet.name,
                range: &header_range,
                preset: Some(header_style),
                font_name: None,
                font_size: None,
                font_bold: None,
                font_italic: None,
                font_underline: None,
                font_color: None,
                fill_color: None,
                border_style: None,
                border_color: None,
                border_top: None,
                border_bottom: None,
                border_left: None,
                border_right: None,
                alignment_horizontal: None,
                alignment_vertical: None,
                alignment_wrap_text: None,
                max_cells: i64::from(target.range.col_count()),
                out: None,
                backup: None,
                dry_run: false,
                no_validate: true,
                in_place: true,
            },
        )?)
    } else {
        None
    };
    if !options.no_validate {
        crate::validate_owned_mutation_output(&readback_path)?;
    }

    let table = table_ref_for_created_table(file, &target, &table_xml)?;
    let destination =
        xlsx_table_create_destination_json(&readback_path, commit_path, &target, &table)?;

    crate::finish_mutation_output(
        file,
        &readback_path,
        output_path,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("table".to_string(), json!(target.table_name));
    result.insert("sheet".to_string(), json!(target.sheet.name));
    result.insert("sheetNumber".to_string(), json!(target.sheet.position));
    result.insert("range".to_string(), json!(target.range_ref));
    result.insert("rows".to_string(), json!(target.range.row_count()));
    result.insert("cols".to_string(), json!(target.range.col_count()));
    result.insert(
        "dataRowCount".to_string(),
        json!(
            target
                .range
                .row_count()
                .saturating_sub(if target.total_row { 2 } else { 1 })
        ),
    );
    result.insert("columns".to_string(), json!(target.headers));
    result.insert(
        "tablePartUri".to_string(),
        json!(format!("/{}", target.table_part)),
    );
    result.insert("relationshipId".to_string(), json!(target.rel_id));
    if !target.style_name.is_empty() {
        result.insert("styleName".to_string(), json!(target.style_name));
    }
    result.insert("totalRow".to_string(), json!(target.total_row));
    if let Some(header_style) = target.header_style.as_deref() {
        result.insert("headerStyle".to_string(), json!(header_style));
    }
    if let Some(Value::Object(style_result)) = header_style_result {
        for field in ["createdStyles", "styleIndexes"] {
            if let Some(value) = style_result.get(field) {
                result.insert(field.to_string(), value.clone());
            }
        }
    }
    let totals = target
        .totals
        .iter()
        .enumerate()
        .filter_map(|(index, total)| {
            total.as_ref().map(|total| {
                json!({
                    "column": target.headers[index],
                    "function": total.function,
                })
            })
        })
        .collect::<Vec<_>>();
    result.insert("totals".to_string(), Value::Array(totals));
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_table_create_commands(&mut result, commit_path, &target, &table);
    Ok(Value::Object(result))
}

fn resolve_xlsx_table_create_target(
    file: &str,
    options: &XlsxTablesCreateOptions<'_>,
) -> CliResult<XlsxTableCreateTarget> {
    let table_name = normalize_xlsx_table_name(options.table)?;
    let source_range = parse_range(
        options
            .range
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| CliError::invalid_args("--range is required"))?,
    )
    .map_err(|err| CliError::invalid_args(format!("invalid --range: {}", err.message)))?
    .normalized();
    if source_range.row_count() < 1 || source_range.col_count() < 1 {
        return Err(CliError::invalid_args(
            "--range must include at least one cell",
        ));
    }

    let sheet_selector = options
        .sheet
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| CliError::invalid_args("--sheet is required"))?;
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let sheet_target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(sheet_target);
    if !sheet_part.starts_with("xl/worksheets/") {
        return Err(CliError::invalid_args(format!(
            "sheet {:?} is not a worksheet",
            sheet.name
        )));
    }
    let sheet_xml = zip_text(file, &sheet_part)?;
    reject_xlsx_merged_cell_intersection(&sheet_xml, source_range)?;

    let total_row = options.total_row || options.totals.is_some();
    let range = if total_row {
        if source_range.end_row >= 1_048_576 {
            return Err(CliError::invalid_args(
                "--total-row cannot extend a table beyond worksheet row 1048576",
            ));
        }
        RangeBounds {
            end_row: source_range.end_row + 1,
            ..source_range
        }
    } else {
        source_range
    };

    let existing_tables = xlsx_tables(file, None)?;
    reject_duplicate_or_overlapping_table(&existing_tables, &table_name, &sheet_part, range)?;
    let headers = xlsx_table_headers(file, &sheet_xml, source_range)?;
    let header_style = normalize_header_style(options.header_style)?;
    if options.style.is_some() && header_style.is_some() {
        return Err(CliError::invalid_args(
            "--style and --header-style cannot be used together",
        ));
    }
    let style_name = header_style
        .as_deref()
        .map(header_style_table_style)
        .map(str::to_string)
        .unwrap_or_else(|| normalize_table_style(options.style));
    let totals = parse_table_totals(options.totals, &headers)?;
    let table_id = next_xlsx_table_id(&existing_tables);
    let table_part = next_xlsx_table_part(file)?;
    let sheet_rels_part = relationships_part_for(&sheet_part);
    let rels_xml = zip_text(file, &sheet_rels_part).unwrap_or_else(|_| relationships_template());
    let rel_id = allocate_relationship_id(&relationship_entries_from_xml(&rels_xml));

    Ok(XlsxTableCreateTarget {
        sheet,
        sheet_part,
        sheet_xml,
        range,
        range_ref: range_bounds_ref(range),
        headers,
        table_name,
        style_name,
        table_id,
        table_part,
        rel_id,
        header_style,
        total_row,
        totals,
    })
}

fn normalize_header_style(value: Option<&str>) -> CliResult<Option<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            let normalized = value.to_ascii_lowercase();
            if matches!(
                normalized.as_str(),
                "header" | "total" | "band" | "input" | "muted"
            ) {
                Ok(normalized)
            } else {
                Err(CliError::invalid_args(
                    "--header-style must be header, total, band, input, or muted",
                ))
            }
        })
        .transpose()
}

fn header_style_table_style(preset: &str) -> &'static str {
    match preset {
        "header" => "TableStyleMedium2",
        "total" => "TableStyleMedium9",
        "band" => "TableStyleMedium4",
        "input" => "TableStyleLight9",
        "muted" => "TableStyleLight1",
        _ => unreachable!("validated table header preset"),
    }
}

fn parse_table_totals(
    value: Option<&str>,
    headers: &[String],
) -> CliResult<Vec<Option<XlsxTableTotal>>> {
    let mut totals = vec![None; headers.len()];
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(totals);
    };
    for assignment in value.split(',') {
        let (column, function) = assignment.split_once(':').ok_or_else(|| {
            CliError::invalid_args(
                "--totals entries must use Column:function (for example Units:sum)",
            )
        })?;
        let column = column.trim();
        let index = headers
            .iter()
            .position(|header| header.eq_ignore_ascii_case(column))
            .ok_or_else(|| CliError::invalid_args(format!("unknown totals column {column:?}")))?;
        let function = normalize_total_function(function)?;
        if totals[index].is_some() {
            return Err(CliError::invalid_args(format!(
                "duplicate totals column {column:?}"
            )));
        }
        totals[index] = Some(XlsxTableTotal { function });
    }
    Ok(totals)
}

fn normalize_total_function(value: &str) -> CliResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    let canonical = match normalized.as_str() {
        "average" | "avg" => "average",
        "count" => "count",
        "countnums" | "count-numbers" => "countNums",
        "max" => "max",
        "min" => "min",
        "stddev" | "std-dev" => "stdDev",
        "sum" => "sum",
        "var" | "variance" => "var",
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported totals function {value:?}; use average, count, countNums, max, min, stdDev, sum, or var"
            )));
        }
    };
    Ok(canonical.to_string())
}

fn write_xlsx_table_total_row(
    target: &XlsxTableCreateTarget,
) -> CliResult<(String, crate::xlsx_mutation::XlsxRangeSetStats)> {
    if !target.total_row {
        return Ok((
            target.sheet_xml.clone(),
            crate::xlsx_mutation::XlsxRangeSetStats::default(),
        ));
    }
    let mut row = Vec::with_capacity(target.headers.len());
    for (index, total) in target.totals.iter().enumerate() {
        if let Some(total) = total {
            row.push(XlsxMatrixCell {
                kind: "formula".to_string(),
                value: String::new(),
                formula: table_total_formula(
                    &target.table_name,
                    &target.headers[index],
                    &total.function,
                ),
                null: false,
            });
        } else if index == 0 {
            row.push(XlsxMatrixCell {
                kind: "string".to_string(),
                value: "Total".to_string(),
                formula: String::new(),
                null: false,
            });
        } else {
            row.push(XlsxMatrixCell {
                kind: String::new(),
                value: String::new(),
                formula: String::new(),
                null: true,
            });
        }
    }
    let total_bounds = RangeBounds {
        start_col: target.range.start_col,
        end_col: target.range.end_col,
        start_row: target.range.end_row,
        end_row: target.range.end_row,
    };
    crate::xlsx_mutation::set_xlsx_range_in_sheet_xml(
        &target.sheet_xml,
        total_bounds,
        &[row],
        "skip",
        false,
    )
}

fn table_total_formula(table_name: &str, header: &str, function: &str) -> String {
    let code = match function {
        "average" => 101,
        "countNums" => 102,
        "count" => 103,
        "max" => 104,
        "min" => 105,
        "stdDev" => 107,
        "sum" => 109,
        "var" => 110,
        _ => unreachable!("validated table total function"),
    };
    format!(
        "SUBTOTAL({code},{}[{}])",
        table_name,
        header.replace(']', "]]"),
    )
}

fn normalize_xlsx_table_name(value: Option<&str>) -> CliResult<String> {
    let raw = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::invalid_args("--table is required"))?;
    if raw.len() > 255 {
        return Err(CliError::invalid_args(
            "--table must be at most 255 characters",
        ));
    }
    let mut chars = raw.chars();
    let Some(first) = chars.next() else {
        return Err(CliError::invalid_args("--table is required"));
    };
    if !(first.is_ascii_alphabetic() || first == '_' || first == '\\') {
        return Err(CliError::invalid_args(
            "--table must start with a letter, underscore, or backslash",
        ));
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.') {
        return Err(CliError::invalid_args(
            "--table may contain only letters, numbers, underscores, and periods",
        ));
    }
    if parse_range(raw).is_ok() {
        return Err(CliError::invalid_args(
            "--table must not look like an A1 cell or range reference",
        ));
    }
    Ok(raw.to_string())
}

fn normalize_table_style(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| !value.eq_ignore_ascii_case("none"))
        .unwrap_or("TableStyleMedium2")
        .to_string()
}

fn reject_duplicate_or_overlapping_table(
    tables: &[XlsxTableRef],
    table_name: &str,
    sheet_part: &str,
    range: RangeBounds,
) -> CliResult<()> {
    for table in tables {
        if table.name.eq_ignore_ascii_case(table_name)
            || table.display_name.eq_ignore_ascii_case(table_name)
        {
            return Err(CliError::invalid_args(format!(
                "table {table_name:?} already exists"
            )));
        }
        if table.sheet_part_uri.trim_start_matches('/') == sheet_part
            && let Ok(existing) = parse_range(&table.range)
            && ranges_intersect(existing.normalized(), range)
        {
            return Err(CliError::invalid_args(format!(
                "table range {} overlaps existing table {} at {}",
                range_bounds_ref(range),
                table.display_name,
                table.range
            )));
        }
    }
    Ok(())
}

fn ranges_intersect(left: RangeBounds, right: RangeBounds) -> bool {
    left.min_col() <= right.max_col()
        && left.max_col() >= right.min_col()
        && left.min_row() <= right.max_row()
        && left.max_row() >= right.min_row()
}

fn xlsx_table_headers(file: &str, sheet_xml: &str, range: RangeBounds) -> CliResult<Vec<String>> {
    let shared = shared_strings(file).unwrap_or_default();
    let styles = xlsx_styles(file).unwrap_or_default();
    let cells = sheet_cells(sheet_xml, &shared, &styles);
    let mut seen = BTreeSet::new();
    let mut headers = Vec::new();
    for col in range.start_col..=range.end_col {
        let cell_ref = format!("{}{}", crate::col_name(col), range.start_row);
        let header = cells
            .get(&cell_ref)
            .map(header_text_from_cell)
            .unwrap_or_default();
        if header.trim().is_empty() {
            return Err(CliError::invalid_args(format!(
                "table header cell {cell_ref} is blank; write header values before creating a table"
            )));
        }
        let key = header.to_ascii_lowercase();
        if !seen.insert(key) {
            return Err(CliError::invalid_args(format!(
                "duplicate table header {header:?}"
            )));
        }
        headers.push(header);
    }
    Ok(headers)
}

fn header_text_from_cell(cell: &CellValue) -> String {
    if !cell.display_value.is_empty() {
        cell.display_value.clone()
    } else if !cell.raw_value.is_empty() {
        cell.raw_value.clone()
    } else {
        String::new()
    }
}

fn next_xlsx_table_id(tables: &[XlsxTableRef]) -> u32 {
    tables
        .iter()
        .map(|table| table.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn next_xlsx_table_part(file: &str) -> CliResult<String> {
    let entries = zip_entry_names(file)?;
    let existing = entries
        .iter()
        .map(|entry| format!("/{}", entry.trim_start_matches('/')))
        .collect::<BTreeSet<_>>();
    for number in 1..=100_000_u32 {
        let part = format!("/xl/tables/table{number}.xml");
        if !existing.contains(&part) {
            return Ok(part.trim_start_matches('/').to_string());
        }
    }
    Err(CliError::unexpected("could not allocate table part name"))
}

fn render_xlsx_table_xml(target: &XlsxTableCreateTarget) -> CliResult<String> {
    let mut body = String::new();
    body.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    body.push_str(&format!(
        r#"<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="{}" name="{}" displayName="{}" ref="{}" headerRowCount="1" totalsRowCount="{}" totalsRowShown="{}">"#,
        target.table_id,
        xml_attr_escape(&target.table_name),
        xml_attr_escape(&target.table_name),
        xml_attr_escape(&target.range_ref),
        u8::from(target.total_row),
        u8::from(target.total_row)
    ));
    body.push_str(&format!(
        r#"<autoFilter ref="{}"/>"#,
        xml_attr_escape(&target.range_ref)
    ));
    body.push_str(&format!(
        r#"<tableColumns count="{}">"#,
        target.headers.len()
    ));
    for (idx, header) in target.headers.iter().enumerate() {
        let total_attr = target.totals[idx]
            .as_ref()
            .map(|total| format!(r#" totalsRowFunction="{}""#, total.function))
            .unwrap_or_else(|| {
                if target.total_row && idx == 0 {
                    r#" totalsRowLabel="Total""#.to_string()
                } else {
                    String::new()
                }
            });
        body.push_str(&format!(
            r#"<tableColumn id="{}" name="{}"{total_attr}/>"#,
            idx + 1,
            xml_attr_escape(header)
        ));
    }
    body.push_str("</tableColumns>");
    if !target.style_name.is_empty() {
        body.push_str(&format!(
            r#"<tableStyleInfo name="{}" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/>"#,
            xml_attr_escape(&target.style_name)
        ));
    }
    body.push_str("</table>");
    Ok(body)
}

fn add_xlsx_table_part_to_worksheet(xml: &str, rel_id: &str) -> CliResult<String> {
    let xml = ensure_relationships_namespace(xml)?;
    let root = worksheet_root_bounds(&xml)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let mut rel_ids = Vec::new();
    if let Some(range) = direct_worksheet_child_range(&xml, &root, "tableParts")? {
        rel_ids = table_part_rel_ids(&xml[range.start..range.end])?;
        if rel_ids.iter().any(|id| id == rel_id) {
            return Ok(xml);
        }
        rel_ids.push(rel_id.to_string());
        let container = render_table_parts_container(&prefix, &rel_ids);
        return Ok(replace_xml_span(&xml, range.start, range.end, &container));
    }
    rel_ids.push(rel_id.to_string());
    insert_worksheet_child(
        &xml,
        &root,
        "tableParts",
        &render_table_parts_container(&prefix, &rel_ids),
    )
}

fn table_part_rel_ids(fragment: &str) -> CliResult<Vec<String>> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut ids = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "tablePart" =>
            {
                if let Some(id) = e.attributes().flatten().find_map(|attr| {
                    if attr.key.as_ref() == b"r:id" || local_name(attr.key.as_ref()) == "id" {
                        Some(crate::decode_xml_text(attr.value.as_ref()))
                    } else {
                        None
                    }
                }) {
                    ids.push(id);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(ids)
}

fn render_table_parts_container(prefix: &str, ids: &[String]) -> String {
    let table_parts = element_name(prefix, "tableParts");
    let table_part = element_name(prefix, "tablePart");
    let mut out = format!(r#"<{table_parts} count="{}">"#, ids.len());
    for id in ids {
        out.push_str(&format!(
            r#"<{table_part} r:id="{}"/>"#,
            xml_attr_escape(id)
        ));
    }
    out.push_str(&format!("</{table_parts}>"));
    out
}

fn table_ref_for_created_table(
    file: &str,
    target: &XlsxTableCreateTarget,
    table_xml: &str,
) -> CliResult<XlsxTableRef> {
    let mut table =
        crate::xlsx_tables::parse_xlsx_table_part(table_xml, &format!("/{}", target.table_part))?;
    table.number = xlsx_tables(file, None)?.len() as u32 + 1;
    table.sheet = target.sheet.name.clone();
    table.sheet_number = target.sheet.position;
    table.sheet_part_uri = format!("/{}", target.sheet_part);
    table.relationship_id = target.rel_id.clone();
    table.part_uri = format!("/{}", target.table_part);
    table.apply_selectors();
    Ok(table)
}

fn xlsx_table_create_destination_json(
    readback_file: &str,
    destination_file: Option<&str>,
    target: &XlsxTableCreateTarget,
    table: &XlsxTableRef,
) -> CliResult<Value> {
    let range = xlsx_range_destination_json(
        readback_file,
        destination_file,
        &target.sheet,
        &target.sheet_part,
        &target.range_ref,
    )?;
    let mut destination = Map::new();
    if let Some(file) = destination_file {
        destination.insert("file".to_string(), json!(file));
    }
    destination.insert("table".to_string(), json!(table.display_name));
    destination.insert(
        "tablePrimarySelector".to_string(),
        json!(table.primary_selector),
    );
    destination.insert("tableSelectors".to_string(), json!(table.selectors));
    destination.insert("tablePartUri".to_string(), json!(table.part_uri));
    destination.insert("relationshipId".to_string(), json!(table.relationship_id));
    destination.insert("sheet".to_string(), json!(target.sheet.name));
    destination.insert("sheetNumber".to_string(), json!(target.sheet.position));
    destination.insert(
        "sheetPrimarySelector".to_string(),
        json!(format!("sheetId:{}", target.sheet.sheet_id)),
    );
    destination.insert(
        "sheetSelectors".to_string(),
        json!(xlsx_sheet_selectors(
            &target.sheet.name,
            target.sheet.sheet_id,
            target.sheet.position,
            &target.sheet.rel_id,
            &format!("/{}", target.sheet_part),
        )),
    );
    destination.insert("range".to_string(), json!(target.range_ref));
    destination.insert("rows".to_string(), json!(target.range.row_count()));
    destination.insert("cols".to_string(), json!(target.range.col_count()));
    destination.insert(
        "dataRows".to_string(),
        json!(
            target
                .range
                .row_count()
                .saturating_sub(if target.total_row { 2 } else { 1 })
        ),
    );
    destination.insert("columns".to_string(), json!(target.headers));
    destination.insert("rangeData".to_string(), range);
    Ok(Value::Object(destination))
}

fn add_xlsx_table_create_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    target: &XlsxTableCreateTarget,
    table: &XlsxTableRef,
) {
    let file = output_path.unwrap_or("<out.xlsx>");
    let validate_key = if output_path.is_some() {
        "validateCommand"
    } else {
        "validateCommandTemplate"
    };
    let show_key = if output_path.is_some() {
        "tableShowCommand"
    } else {
        "tableShowCommandTemplate"
    };
    let export_key = if output_path.is_some() {
        "tableExportCommand"
    } else {
        "tableExportCommandTemplate"
    };
    result.insert(
        validate_key.to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(file))),
    );
    result.insert(
        show_key.to_string(),
        json!(xlsx_source_command(
            vec!["ooxml", "--json", "xlsx", "tables", "show", file],
            &[
                ("--sheet", &format!("sheetId:{}", target.sheet.sheet_id)),
                ("--table", &table.primary_selector),
            ],
        )),
    );
    let mut export = xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "tables", "export", file],
        &[
            ("--sheet", &format!("sheetId:{}", target.sheet.sheet_id)),
            ("--table", &table.primary_selector),
        ],
    );
    export.push_str(" --include-types --include-formulas");
    result.insert(export_key.to_string(), json!(export));
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
    Ok(replace_xml_span(xml, insert_at, insert_at, child_xml))
}

fn ensure_relationships_namespace(xml: &str) -> CliResult<String> {
    let root = worksheet_root_bounds(xml)?;
    let start_tag = &xml[root.start..root.open_end];
    if start_tag.contains("xmlns:r=") {
        return Ok(xml.to_string());
    }
    let relative_insert = start_tag
        .rfind("/>")
        .unwrap_or_else(|| start_tag.len().saturating_sub(1));
    let insert_at = root.start + relative_insert;
    let attr = format!(r#" xmlns:r="{OFFICE_R_NS}""#);
    let mut updated = String::with_capacity(xml.len() + attr.len());
    updated.push_str(&xml[..insert_at]);
    updated.push_str(&attr);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
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

fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn relationships_template() -> String {
    crate::opc::empty_relationships_xml(false)
}
