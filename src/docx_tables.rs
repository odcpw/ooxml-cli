use serde_json::{Map, Value, json};
use std::fs;

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, DocxRichBlockReport, InspectPackageKind,
    XmlNamedRange, append_docx_text_children, command_arg, detect_inspect_package_type,
    docx_body_block_ranges, docx_body_content_bounds, docx_body_tag,
    docx_mutation_output_path_for_result, docx_rich_block_reports, ensure_docx_package_kind,
    ensure_docx_table_scaffold_fragment, ensure_docx_word_prefix, find_docx_document_part,
    first_direct_xml_child_by_kind, package_type, validate_xlsx_mutation_output_flags,
    word_xml_tag, write_docx_mutation_output, xml_direct_child_ranges, xml_fragment_bounds,
    xml_open_tag_from_start, xml_tag_prefix, zip_entry_names, zip_text,
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

pub(crate) fn docx_tables_create(
    file: &str,
    values: Option<&str>,
    values_file: Option<&str>,
    values_changed: bool,
    values_file_changed: bool,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let matrix = parse_docx_table_values(values, values_file, values_changed, values_file_changed)?;
    let mutation = docx_table_create_mutation(file, &matrix)?;
    let output_path = docx_mutation_output_path_for_result(file, &options);
    write_docx_mutation_output(file, &mutation.document_part, &mutation.xml, options)?;

    Ok(Value::Object(docx_table_create_result(
        file,
        &mutation,
        output_path,
    )))
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

pub(crate) fn docx_tables_insert_row(
    file: &str,
    table: usize,
    at: usize,
    expected_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let mutation = docx_table_insert_row_mutation(file, table, at, expected_hash)?;
    let output_path = docx_mutation_output_path_for_result(file, &options);
    write_docx_mutation_output(file, &mutation.document_part, &mutation.xml, options)?;

    Ok(Value::Object(docx_table_row_mutation_result(
        file,
        table,
        at,
        &mutation,
        output_path,
    )))
}

pub(crate) fn docx_tables_delete_row(
    file: &str,
    table: usize,
    row: usize,
    expected_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let mutation = docx_table_delete_row_mutation(file, table, row, expected_hash)?;
    let output_path = docx_mutation_output_path_for_result(file, &options);
    write_docx_mutation_output(file, &mutation.document_part, &mutation.xml, options)?;

    Ok(Value::Object(docx_table_row_mutation_result(
        file,
        table,
        row,
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

struct DocxTableRowMutation {
    document_part: String,
    xml: String,
    block: usize,
    rows: usize,
    cols: usize,
    content_hash: String,
    previous_hash: String,
}

struct DocxTableCreateMutation {
    document_part: String,
    xml: String,
    table: usize,
    block: usize,
    rows: usize,
    cols: usize,
    content_hash: String,
}

fn parse_docx_table_values(
    values: Option<&str>,
    values_file: Option<&str>,
    values_changed: bool,
    values_file_changed: bool,
) -> CliResult<Vec<Vec<String>>> {
    if values_changed == values_file_changed {
        return Err(CliError::invalid_args(
            "must specify exactly one of --values or --values-file",
        ));
    }
    let raw = if values_changed {
        values.unwrap_or_default().to_string()
    } else {
        let path = values_file.unwrap_or_default();
        fs::read(path)
            .map(|data| String::from_utf8_lossy(&data).to_string())
            .map_err(|_| CliError::file_not_found(format!("file not found: {path}")))?
    };
    let parsed: Value = serde_json::from_str(&raw)
        .map_err(|err| CliError::invalid_args(format!("invalid --values JSON: {err}")))?;
    let rows = parsed
        .as_array()
        .ok_or_else(|| CliError::invalid_args("--values must be a JSON array of rows"))?;
    if rows.is_empty() {
        return Err(CliError::invalid_args("--values matrix cannot be empty"));
    }
    let mut matrix = Vec::with_capacity(rows.len());
    let mut width = None;
    for (row_idx, row) in rows.iter().enumerate() {
        let cells = row
            .as_array()
            .ok_or_else(|| CliError::invalid_args("--values rows must be JSON arrays"))?;
        if cells.is_empty() {
            return Err(CliError::invalid_args(format!(
                "--values row {} cannot be empty",
                row_idx + 1
            )));
        }
        if let Some(expected) = width {
            if cells.len() != expected {
                return Err(CliError::invalid_args(format!(
                    "--values must be rectangular: row {} has {} cells, expected {expected}",
                    row_idx + 1,
                    cells.len()
                )));
            }
        } else {
            width = Some(cells.len());
        }
        matrix.push(
            cells
                .iter()
                .map(docx_table_cell_value_to_text)
                .collect::<CliResult<Vec<_>>>()?,
        );
    }
    Ok(matrix)
}

fn docx_table_cell_value_to_text(value: &Value) -> CliResult<String> {
    match value {
        Value::Null => Ok(String::new()),
        Value::String(text) => Ok(text.clone()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        _ => Err(CliError::invalid_args(
            "--values cells must be strings, numbers, booleans, or null",
        )),
    }
}

fn docx_table_create_mutation(
    file: &str,
    matrix: &[Vec<String>],
) -> CliResult<DocxTableCreateMutation> {
    let entries = zip_entry_names(file)?;
    ensure_docx_package_kind(file, &entries)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let original_xml = zip_text(file, &document_part)?;
    let mut working = ensure_docx_word_prefix(&original_xml)?;
    let body_tag = docx_body_tag(&working)?;
    let (content_start, content_end) = docx_body_content_bounds(&working, &body_tag)?;
    let body_children = xml_direct_child_ranges(&working, content_start, content_end)?;
    let insert_at = body_children
        .iter()
        .rev()
        .find(|child| child.kind == "sectPr")
        .map(|child| child.start)
        .unwrap_or(content_end);
    let table_xml = render_docx_table("w", matrix);
    working.insert_str(insert_at, &table_xml);

    let reports = docx_rich_block_reports(&working, false).map_err(|err| {
        CliError::unexpected(format!("failed to read updated document: {}", err.message))
    })?;
    let table_reports = reports
        .iter()
        .filter(|report| report.kind == "table")
        .collect::<Vec<_>>();
    let created = table_reports
        .last()
        .ok_or_else(|| CliError::unexpected("created table readback missing"))?;

    Ok(DocxTableCreateMutation {
        document_part,
        xml: working,
        table: table_reports.len(),
        block: created.index,
        rows: matrix.len(),
        cols: matrix.first().map(Vec::len).unwrap_or_default(),
        content_hash: created.content_hash.clone(),
    })
}

fn render_docx_table(prefix: &str, matrix: &[Vec<String>]) -> String {
    let tbl = word_xml_tag(prefix, "tbl");
    let tbl_pr = word_xml_tag(prefix, "tblPr");
    let tbl_w = word_xml_tag(prefix, "tblW");
    let tbl_grid = word_xml_tag(prefix, "tblGrid");
    let grid_col = word_xml_tag(prefix, "gridCol");
    let tr = word_xml_tag(prefix, "tr");
    let tc = word_xml_tag(prefix, "tc");
    let tc_pr = word_xml_tag(prefix, "tcPr");
    let tc_w = word_xml_tag(prefix, "tcW");
    let p = word_xml_tag(prefix, "p");
    let r = word_xml_tag(prefix, "r");
    let cols = matrix.first().map(Vec::len).unwrap_or_default();
    let col_width = if cols == 0 {
        2400
    } else {
        (8640 / cols.max(1)).max(720)
    };

    let mut out = String::new();
    out.push('<');
    out.push_str(&tbl);
    out.push('>');
    out.push('<');
    out.push_str(&tbl_pr);
    out.push('>');
    out.push('<');
    out.push_str(&tbl_w);
    out.push_str(r#" w:w="0" w:type="auto"/>"#);
    out.push_str("</");
    out.push_str(&tbl_pr);
    out.push('>');
    out.push('<');
    out.push_str(&tbl_grid);
    out.push('>');
    for _ in 0..cols {
        out.push('<');
        out.push_str(&grid_col);
        out.push_str(&format!(r#" w:w="{col_width}"/>"#));
    }
    out.push_str("</");
    out.push_str(&tbl_grid);
    out.push('>');
    for row in matrix {
        out.push('<');
        out.push_str(&tr);
        out.push('>');
        for cell in row {
            out.push('<');
            out.push_str(&tc);
            out.push('>');
            out.push('<');
            out.push_str(&tc_pr);
            out.push('>');
            out.push('<');
            out.push_str(&tc_w);
            out.push_str(&format!(r#" w:w="{col_width}" w:type="dxa"/>"#));
            out.push_str("</");
            out.push_str(&tc_pr);
            out.push('>');
            out.push('<');
            out.push_str(&p);
            out.push('>');
            out.push('<');
            out.push_str(&r);
            out.push('>');
            append_docx_text_children(&mut out, prefix, cell);
            out.push_str("</");
            out.push_str(&r);
            out.push('>');
            out.push_str("</");
            out.push_str(&p);
            out.push('>');
            out.push_str("</");
            out.push_str(&tc);
            out.push('>');
        }
        out.push_str("</");
        out.push_str(&tr);
        out.push('>');
    }
    out.push_str("</");
    out.push_str(&tbl);
    out.push('>');
    out
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

fn docx_table_insert_row_mutation(
    file: &str,
    table: usize,
    at: usize,
    expected_hash: &str,
) -> CliResult<DocxTableRowMutation> {
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
        if report.table_merged {
            return Err(CliError::invalid_args("table has merged cells"));
        }
        let row_count = report.table_rows.len();
        if row_count == 0 || at < 1 || at > row_count + 1 {
            return Err(CliError::target_not_found(format!(
                "target not found: table {table} row {at}"
            )));
        }
        break;
    }
    if selected_block == 0 {
        return Err(CliError::target_not_found(format!(
            "target not found: table {table} row {at}"
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
    let updated_table = insert_docx_table_row_fragment(&table_fragment, at)?;

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
    let rows = updated_report.table_rows.len();
    let cols = updated_report
        .table_rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or_default();

    Ok(DocxTableRowMutation {
        document_part,
        xml: updated_xml,
        block: selected_block,
        rows,
        cols,
        content_hash: updated_report.content_hash,
        previous_hash,
    })
}

fn docx_table_delete_row_mutation(
    file: &str,
    table: usize,
    row: usize,
    expected_hash: &str,
) -> CliResult<DocxTableRowMutation> {
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
        if report.table_merged {
            return Err(CliError::invalid_args("table has merged cells"));
        }
        if row < 1 || row > report.table_rows.len() {
            return Err(CliError::target_not_found(format!(
                "target not found: table {table} row {row}"
            )));
        }
        if report.table_rows.len() == 1 {
            return Err(CliError::invalid_args("cannot delete the last table row"));
        }
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
    let updated_table = delete_docx_table_row_fragment(&table_fragment, row)?;

    let mut updated_xml = String::with_capacity(xml.len());
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
    let rows = updated_report.table_rows.len();
    let cols = updated_report
        .table_rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or_default();

    Ok(DocxTableRowMutation {
        document_part,
        xml: updated_xml,
        block: selected_block,
        rows,
        cols,
        content_hash: updated_report.content_hash,
        previous_hash,
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

fn docx_table_row_mutation_result(
    file: &str,
    table: usize,
    row: usize,
    mutation: &DocxTableRowMutation,
    output_path: Option<String>,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("table".to_string(), json!(table));
    result.insert("block".to_string(), json!(mutation.block));
    result.insert("row".to_string(), json!(row));
    result.insert("rows".to_string(), json!(mutation.rows));
    result.insert("cols".to_string(), json!(mutation.cols));
    result.insert("contentHash".to_string(), json!(mutation.content_hash));
    result.insert("previousHash".to_string(), json!(mutation.previous_hash));
    if let Some(output) = output_path.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    add_docx_table_readback_commands(&mut result, output_path.as_deref(), table);
    result
}

fn docx_table_create_result(
    file: &str,
    mutation: &DocxTableCreateMutation,
    output_path: Option<String>,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("table".to_string(), json!(mutation.table));
    result.insert("block".to_string(), json!(mutation.block));
    result.insert("rows".to_string(), json!(mutation.rows));
    result.insert("cols".to_string(), json!(mutation.cols));
    result.insert("contentHash".to_string(), json!(mutation.content_hash));
    if let Some(output) = output_path.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    add_docx_table_readback_commands(&mut result, output_path.as_deref(), mutation.table);
    result
}

fn add_docx_table_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    table: usize,
) {
    let target = output_path.unwrap_or("<out.docx>");
    let validate = format!("ooxml validate --strict {}", command_arg(target));
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

fn delete_docx_table_row_fragment(table_fragment: &str, row: usize) -> CliResult<String> {
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(table_fragment)?;
    if self_closing {
        return Err(CliError::target_not_found(format!(
            "target not found: table row {row}"
        )));
    }
    let rows: Vec<XmlNamedRange> =
        xml_direct_child_ranges(table_fragment, open_end + 1, close_start)?
            .into_iter()
            .filter(|child| child.kind == "tr")
            .collect();
    let row_range = rows
        .get(row - 1)
        .ok_or_else(|| CliError::target_not_found(format!("target not found: table row {row}")))?;

    let mut updated_table = String::with_capacity(table_fragment.len());
    updated_table.push_str(&table_fragment[..row_range.start]);
    updated_table.push_str(&table_fragment[row_range.end..]);
    Ok(updated_table)
}

fn insert_docx_table_row_fragment(table_fragment: &str, at: usize) -> CliResult<String> {
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(table_fragment)?;
    if self_closing {
        return Err(CliError::target_not_found(format!(
            "target not found: table row {at}"
        )));
    }
    let rows: Vec<XmlNamedRange> =
        xml_direct_child_ranges(table_fragment, open_end + 1, close_start)?
            .into_iter()
            .filter(|child| child.kind == "tr")
            .collect();
    if rows.is_empty() || at < 1 || at > rows.len() + 1 {
        return Err(CliError::target_not_found(format!(
            "target not found: table row {at}"
        )));
    }
    let template_range = if at <= rows.len() {
        &rows[at - 1]
    } else {
        rows.last().expect("non-empty rows")
    };
    let inserted_row = clear_docx_table_row_cells_fragment(
        &table_fragment[template_range.start..template_range.end],
    )?;
    let insert_at = if at <= rows.len() {
        rows[at - 1].start
    } else {
        close_start
    };

    let mut updated_table = String::with_capacity(table_fragment.len() + inserted_row.len());
    updated_table.push_str(&table_fragment[..insert_at]);
    updated_table.push_str(&inserted_row);
    updated_table.push_str(&table_fragment[insert_at..]);
    Ok(updated_table)
}

fn clear_docx_table_row_cells_fragment(row_fragment: &str) -> CliResult<String> {
    let (row_open_end, _row_tag_name, row_close_start, row_self_closing) =
        xml_fragment_bounds(row_fragment)?;
    if row_self_closing {
        return Ok(row_fragment.to_string());
    }
    let cells: Vec<XmlNamedRange> =
        xml_direct_child_ranges(row_fragment, row_open_end + 1, row_close_start)?
            .into_iter()
            .filter(|child| child.kind == "tc")
            .collect();
    let mut updated_row = row_fragment.to_string();
    for cell_range in cells.iter().rev() {
        let cell_fragment = &row_fragment[cell_range.start..cell_range.end];
        let (updated_cell, _flattened) = set_docx_table_cell_fragment(cell_fragment, "")?;
        updated_row.replace_range(cell_range.start..cell_range.end, &updated_cell);
    }
    Ok(updated_row)
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
