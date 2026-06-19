use serde_json::{Map, Value, json};

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, DocxRichBlockReport, InspectPackageKind,
    XmlNamedRange, append_docx_text_children, command_arg, detect_inspect_package_type,
    docx_body_block_ranges, docx_body_tag, docx_mutation_output_path_for_result,
    docx_rich_block_reports, ensure_docx_package_kind, ensure_docx_table_scaffold_fragment,
    find_docx_document_part, first_direct_xml_child_by_kind, package_type,
    validate_xlsx_mutation_output_flags, word_xml_tag, write_docx_mutation_output,
    xml_direct_child_ranges, xml_fragment_bounds, xml_open_tag_from_start, xml_tag_prefix,
    zip_entry_names, zip_text,
};

pub(crate) fn docx_tables_show(
    file: &str,
    table: usize,
    include_details: bool,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let xml = zip_text(file, &document_part).map_err(|_| {
        CliError::unexpected(format!(
            "failed to read main document: part {document_uri} not found"
        ))
    })?;
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        if err.message == "invalid DOCX XML"
            || err.message.starts_with("failed to extract DOCX blocks:")
        {
            CliError::unexpected(format!(
                "failed to read main document: failed to read document part {document_uri}: failed to parse XML part {document_uri}: etree: invalid XML format"
            ))
        } else {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        }
    })?;

    let mut table_number = 0usize;
    let mut tables = Vec::new();
    for report in reports.into_iter().filter(|report| report.kind == "table") {
        table_number += 1;
        if table > 0 && table_number != table {
            continue;
        }
        tables.push(docx_table_summary_json(
            file,
            table_number,
            report,
            include_details,
        ));
    }
    if table > 0 && tables.is_empty() {
        return Err(CliError::target_not_found(format!(
            "target not found: table {table}"
        )));
    }

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert(
        "tables".to_string(),
        if tables.is_empty() {
            Value::Null
        } else {
            Value::Array(tables)
        },
    );
    Ok(Value::Object(result))
}

pub(crate) fn docx_tables_set_cell(
    file: &str,
    table: usize,
    row: usize,
    col: usize,
    expected_hash: &str,
    text: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let mutation = docx_table_cell_text_mutation(file, table, row, col, expected_hash, text)?;
    let output_path = docx_mutation_output_path_for_result(file, &options);
    write_docx_mutation_output(file, &mutation.document_part, &mutation.xml, options)?;

    let mut result = docx_table_cell_mutation_result(file, table, row, col, &mutation, output_path);
    result.insert("text".to_string(), json!(text));
    Ok(Value::Object(result))
}

pub(crate) fn docx_tables_clear_cell(
    file: &str,
    table: usize,
    row: usize,
    col: usize,
    expected_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let mutation = docx_table_cell_text_mutation(file, table, row, col, expected_hash, "")?;
    let output_path = docx_mutation_output_path_for_result(file, &options);
    write_docx_mutation_output(file, &mutation.document_part, &mutation.xml, options)?;

    Ok(Value::Object(docx_table_cell_mutation_result(
        file,
        table,
        row,
        col,
        &mutation,
        output_path,
    )))
}

struct DocxTableCellMutation {
    document_part: String,
    xml: String,
    block: usize,
    content_hash: String,
    previous_hash: String,
    previous_text: String,
    flattened: bool,
}

fn docx_table_cell_text_mutation(
    file: &str,
    table: usize,
    row: usize,
    col: usize,
    expected_hash: &str,
    text: &str,
) -> CliResult<DocxTableCellMutation> {
    let entries = zip_entry_names(file)?;
    ensure_docx_package_kind(file, &entries)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;

    let mut table_seen = 0usize;
    let mut selected_block = 0usize;
    let mut previous_hash = String::new();
    let mut previous_text = String::new();
    for report in reports.iter().filter(|report| report.kind == "table") {
        table_seen += 1;
        if table_seen != table {
            continue;
        }
        selected_block = report.index;
        previous_hash = report.content_hash.clone();
        if previous_hash != expected_hash {
            return Err(CliError::invalid_args(format!(
                "block hash mismatch: block {selected_block} expected {expected_hash} but found {previous_hash}"
            )));
        }
        previous_text = report
            .table_rows
            .get(row - 1)
            .and_then(|cells| cells.get(col - 1))
            .cloned()
            .ok_or_else(|| {
                CliError::target_not_found(format!(
                    "target not found: table {table} cell R{row}C{col}"
                ))
            })?;
        break;
    }
    if selected_block == 0 {
        return Err(CliError::target_not_found(format!(
            "target not found: table {table}"
        )));
    }

    let body_tag = docx_body_tag(&xml)?;
    let ranges = docx_body_block_ranges(&xml, &body_tag)?;
    let table_range = ranges
        .get(selected_block - 1)
        .filter(|range| range.kind == "tbl")
        .ok_or_else(|| CliError::unexpected("selected table block readback missing"))?;
    let table_fragment =
        ensure_docx_table_scaffold_fragment(&xml[table_range.start..table_range.end])?;
    let (updated_table, flattened) =
        set_docx_table_cell_text_fragment(&table_fragment, row, col, text)?;

    let mut updated_xml = String::with_capacity(xml.len() + updated_table.len());
    updated_xml.push_str(&xml[..table_range.start]);
    updated_xml.push_str(&updated_table);
    updated_xml.push_str(&xml[table_range.end..]);

    let updated_report = docx_rich_block_reports(&updated_xml, false)
        .map_err(|err| {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        })?
        .into_iter()
        .find(|report| report.index == selected_block && report.kind == "table")
        .ok_or_else(|| CliError::unexpected("updated table readback missing"))?;

    Ok(DocxTableCellMutation {
        document_part,
        xml: updated_xml,
        block: selected_block,
        content_hash: updated_report.content_hash,
        previous_hash,
        previous_text,
        flattened,
    })
}

fn docx_table_cell_mutation_result(
    file: &str,
    table: usize,
    row: usize,
    col: usize,
    mutation: &DocxTableCellMutation,
    output_path: Option<String>,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("table".to_string(), json!(table));
    result.insert("block".to_string(), json!(mutation.block));
    result.insert("row".to_string(), json!(row));
    result.insert("col".to_string(), json!(col));
    result.insert("contentHash".to_string(), json!(mutation.content_hash));
    result.insert("previousHash".to_string(), json!(mutation.previous_hash));
    result.insert("previousText".to_string(), json!(mutation.previous_text));
    result.insert("flattened".to_string(), json!(mutation.flattened));
    if let Some(output) = output_path.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    add_docx_table_readback_commands(&mut result, output_path.as_deref(), table);
    result
}

fn add_docx_table_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    table: usize,
) {
    let target = output_path.unwrap_or("<out.pptx>");
    let validate = format!("ooxml validate --strict {target}");
    let show = format!(
        "ooxml --json docx tables show {} --table {}",
        command_arg(target),
        table
    );
    let list = format!("ooxml --json docx tables show {}", command_arg(target));
    if output_path.is_some() {
        result.insert("validateCommand".to_string(), json!(validate));
        result.insert("tablesShowCommand".to_string(), json!(show));
        result.insert("tablesListCommand".to_string(), json!(list));
    } else {
        result.insert("validateCommandTemplate".to_string(), json!(validate));
        result.insert("tablesShowCommandTemplate".to_string(), json!(show));
        result.insert("tablesListCommandTemplate".to_string(), json!(list));
    }
}

fn set_docx_table_cell_text_fragment(
    table_fragment: &str,
    row: usize,
    col: usize,
    text: &str,
) -> CliResult<(String, bool)> {
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(table_fragment)?;
    if self_closing {
        return Err(CliError::target_not_found(format!(
            "target not found: table cell R{row}C{col}"
        )));
    }
    let rows: Vec<XmlNamedRange> =
        xml_direct_child_ranges(table_fragment, open_end + 1, close_start)?
            .into_iter()
            .filter(|child| child.kind == "tr")
            .collect();
    let row_range = rows.get(row - 1).ok_or_else(|| {
        CliError::target_not_found(format!("target not found: table cell R{row}C{col}"))
    })?;
    let row_fragment = &table_fragment[row_range.start..row_range.end];
    let (row_open_end, _row_tag_name, row_close_start, row_self_closing) =
        xml_fragment_bounds(row_fragment)?;
    if row_self_closing {
        return Err(CliError::target_not_found(format!(
            "target not found: table cell R{row}C{col}"
        )));
    }
    let cells: Vec<XmlNamedRange> =
        xml_direct_child_ranges(row_fragment, row_open_end + 1, row_close_start)?
            .into_iter()
            .filter(|child| child.kind == "tc")
            .collect();
    let cell_range = cells.get(col - 1).ok_or_else(|| {
        CliError::target_not_found(format!("target not found: table cell R{row}C{col}"))
    })?;
    let cell_fragment = &row_fragment[cell_range.start..cell_range.end];
    let (updated_cell, flattened) = set_docx_table_cell_fragment(cell_fragment, text)?;

    let mut updated_row = String::with_capacity(row_fragment.len() + updated_cell.len());
    updated_row.push_str(&row_fragment[..cell_range.start]);
    updated_row.push_str(&updated_cell);
    updated_row.push_str(&row_fragment[cell_range.end..]);

    let mut updated_table = String::with_capacity(table_fragment.len() + updated_row.len());
    updated_table.push_str(&table_fragment[..row_range.start]);
    updated_table.push_str(&updated_row);
    updated_table.push_str(&table_fragment[row_range.end..]);
    Ok((updated_table, flattened))
}

fn set_docx_table_cell_fragment(cell_fragment: &str, text: &str) -> CliResult<(String, bool)> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(cell_fragment)?;
    let start_tag = &cell_fragment[..=open_end];
    let prefix = xml_tag_prefix(&tag_name);
    let children = if self_closing {
        Vec::new()
    } else {
        xml_direct_child_ranges(cell_fragment, open_end + 1, close_start)?
    };
    let paragraphs: Vec<&XmlNamedRange> =
        children.iter().filter(|child| child.kind == "p").collect();
    let mut flattened = paragraphs.len() > 1;
    for child in &children {
        if child.kind != "tcPr" && (child.kind != "p" || paragraphs.len() > 1) {
            flattened = true;
        }
    }

    let mut paragraph_properties = String::new();
    let mut run_properties = String::new();
    if let Some(first_paragraph) = paragraphs.first() {
        let paragraph_fragment = &cell_fragment[first_paragraph.start..first_paragraph.end];
        if let Some(p_pr) = first_direct_xml_child_by_kind(paragraph_fragment, "pPr")? {
            paragraph_properties = p_pr;
        }
        run_properties = first_docx_run_properties_in_paragraph_fragment(paragraph_fragment)?;
    }

    let mut out = xml_open_tag_from_start(start_tag);
    for child in children.iter().filter(|child| child.kind == "tcPr") {
        out.push_str(&cell_fragment[child.start..child.end]);
    }
    out.push_str(&render_docx_cell_paragraph(
        &prefix,
        text,
        &paragraph_properties,
        &run_properties,
    ));
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok((out, flattened))
}

fn first_docx_run_properties_in_paragraph_fragment(fragment: &str) -> CliResult<String> {
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(String::new());
    }
    for child in xml_direct_child_ranges(fragment, open_end + 1, close_start)? {
        if child.kind == "r" {
            return first_direct_xml_child_by_kind(&fragment[child.start..child.end], "rPr")
                .map(|value| value.unwrap_or_default());
        }
    }
    Ok(String::new())
}

fn render_docx_cell_paragraph(
    prefix: &str,
    text: &str,
    paragraph_properties: &str,
    run_properties: &str,
) -> String {
    let p = word_xml_tag(prefix, "p");
    let mut paragraph = String::new();
    paragraph.push('<');
    paragraph.push_str(&p);
    paragraph.push('>');
    paragraph.push_str(paragraph_properties);
    if !text.is_empty() {
        let r = word_xml_tag(prefix, "r");
        paragraph.push('<');
        paragraph.push_str(&r);
        paragraph.push('>');
        paragraph.push_str(run_properties);
        append_docx_text_children(&mut paragraph, prefix, text);
        paragraph.push_str("</");
        paragraph.push_str(&r);
        paragraph.push('>');
    }
    paragraph.push_str("</");
    paragraph.push_str(&p);
    paragraph.push('>');
    paragraph
}

fn docx_table_summary_json(
    file: &str,
    table_number: usize,
    report: DocxRichBlockReport,
    include_details: bool,
) -> Value {
    let rows = report.table_rows;
    let row_count = rows.len();
    let col_count = rows.iter().map(Vec::len).max().unwrap_or_default();
    let mut table = Map::new();
    table.insert("file".to_string(), json!(file));
    table.insert("table".to_string(), json!(table_number));
    table.insert("block".to_string(), json!(report.index));
    table.insert(
        "primarySelector".to_string(),
        json!(table_number.to_string()),
    );
    table.insert("selectors".to_string(), json!([table_number.to_string()]));
    table.insert("contentHash".to_string(), json!(report.content_hash));
    table.insert("rows".to_string(), json!(row_count));
    table.insert("cols".to_string(), json!(col_count));
    table.insert("merged".to_string(), json!(report.table_merged));
    if include_details {
        let detail_rows: Vec<Value> = rows.iter().map(|row| json!({"cells": row})).collect();
        table.insert("tableInfo".to_string(), json!({"rows": detail_rows}));
    } else {
        table.insert("cells".to_string(), json!(rows));
    }
    Value::Object(table)
}
