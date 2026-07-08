use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::{
    CellValue, CliError, CliResult, EXIT_SUCCESS, RangeBounds, WorkbookSheet, XlsxTableRef,
    add_relationship_to_xml, allocate_relationship_id, command_arg,
    copy_zip_with_part_overrides_and_removals, ensure_content_type_override, local_name,
    normalize_xl_target, parse_range, range_bounds_ref, reject_xlsx_merged_cell_intersection,
    relationship_entries_from_xml, relationship_target_from_source_to_target, relationships,
    relationships_part_for, replace_xml_span, resolve_sheet, shared_strings, sheet_cells, validate,
    validate_exit_code, validate_xlsx_mutation_output_flags, workbook_sheets,
    xlsx_range_destination_json, xlsx_ranges_set_temp_path, xlsx_sheet_selectors,
    xlsx_source_command, xlsx_styles, xlsx_tables, xml_attr_escape, xml_direct_child_ranges,
    xml_open_tag_from_start, xml_tag_prefix, zip_entry_names, zip_text,
};

const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
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
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
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
    let updated_rels = add_relationship_to_xml(
        sheet_rels_xml,
        &target.rel_id,
        REL_TYPE_TABLE,
        &table_target,
    );

    let updated_sheet_xml = add_xlsx_table_part_to_worksheet(&target.sheet_xml, &target.rel_id)?;
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
    overrides.insert(target.sheet_part.clone(), updated_sheet_xml);
    overrides.insert(sheet_rels_part, updated_rels);
    overrides.insert(target.table_part.clone(), table_xml.clone());
    overrides.insert("[Content_Types].xml".to_string(), content_types);
    copy_zip_with_part_overrides_and_removals(file, &readback_path, &overrides, &BTreeSet::new())?;
    if !options.no_validate {
        let report = validate(&readback_path, true)?;
        if validate_exit_code(&report, true) != EXIT_SUCCESS {
            return Err(CliError::validation_failed(
                "created XLSX table package failed strict validation",
            ));
        }
    }

    let table = table_ref_for_created_table(file, &target, &table_xml)?;
    let destination =
        xlsx_table_create_destination_json(&readback_path, commit_path, &target, &table)?;

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
        json!(target.range.row_count().saturating_sub(1)),
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
    let range = parse_range(
        options
            .range
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| CliError::invalid_args("--range is required"))?,
    )
    .map_err(|err| CliError::invalid_args(format!("invalid --range: {}", err.message)))?
    .normalized();
    if range.row_count() < 1 || range.col_count() < 1 {
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
    reject_xlsx_merged_cell_intersection(&sheet_xml, range)?;

    let existing_tables = xlsx_tables(file, None)?;
    reject_duplicate_or_overlapping_table(&existing_tables, &table_name, &sheet_part, range)?;
    let headers = xlsx_table_headers(file, &sheet_xml, range)?;
    let style_name = normalize_table_style(options.style);
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
    })
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
        r#"<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="{}" name="{}" displayName="{}" ref="{}" headerRowCount="1" totalsRowShown="0">"#,
        target.table_id,
        xml_attr_escape(&target.table_name),
        xml_attr_escape(&target.table_name),
        xml_attr_escape(&target.range_ref)
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
        body.push_str(&format!(
            r#"<tableColumn id="{}" name="{}"/>"#,
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
        json!(target.range.row_count().saturating_sub(1)),
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
                let end = reader.buffer_position() as usize;
                return Ok(WorksheetRootBounds {
                    start: before,
                    open_end: end,
                    close_start: end,
                    end,
                    tag_name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    self_closing: true,
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                return Err(CliError::unexpected(format!(
                    "worksheet root is {:?}",
                    local_name(e.name().as_ref())
                )));
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("worksheet root not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
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
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL_NS}"></Relationships>"#
    )
}
