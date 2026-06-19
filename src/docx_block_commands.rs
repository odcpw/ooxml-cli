use serde_json::{Map, Value, json};

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, InspectPackageKind,
    detect_inspect_package_type, docx_block_has_section_properties, docx_blocks,
    docx_body_block_ranges, docx_body_prefix, docx_body_tag, docx_rich_block_json,
    docx_rich_block_reports, ensure_docx_package_kind, ensure_docx_word_prefix,
    find_docx_document_part, insert_docx_body_paragraph_xml, package_type, render_docx_paragraph,
    resolve_optional_docx_paragraph_text, validate_xlsx_mutation_output_flags,
    write_docx_mutation_output, zip_entry_names, zip_text,
};

pub(crate) fn docx_text(file: &str) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "docx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }
    let xml = zip_text(file, "word/document.xml")?;
    let blocks = docx_blocks(&xml);
    Ok(json!({"blocks": blocks, "file": file}))
}

pub(crate) fn docx_blocks_show(file: &str, block: usize, include_runs: bool) -> CliResult<Value> {
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
    let xml = zip_text(file, &document_part)?;
    let reports = docx_rich_block_reports(&xml, include_runs).map_err(|err| {
        if err.message == "invalid DOCX XML"
            || err.message.starts_with("failed to extract DOCX blocks:")
        {
            CliError::unexpected(format!(
                "failed to extract DOCX blocks: failed to read document part {document_uri}: failed to parse XML part {document_uri}: etree: invalid XML format"
            ))
        } else {
            CliError::unexpected(format!("failed to extract DOCX blocks: {}", err.message))
        }
    })?;
    let blocks: Vec<Value> = if block > 0 {
        reports
            .into_iter()
            .filter(|report| report.index == block)
            .map(docx_rich_block_json)
            .collect()
    } else {
        reports.into_iter().map(docx_rich_block_json).collect()
    };
    if block > 0 && blocks.is_empty() {
        return Err(CliError::target_not_found(format!(
            "target not found: block {block}"
        )));
    }
    Ok(json!({
        "file": file,
        "documentPartUri": document_uri,
        "blocks": blocks,
    }))
}

pub(crate) fn docx_blocks_insert_after(
    file: &str,
    block: usize,
    expected_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    ensure_docx_package_kind(file, &entries)?;

    let text = resolve_optional_docx_paragraph_text(options.text, options.text_file)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let anchor_hash = if block > 0 {
        let anchor = reports
            .get(block - 1)
            .ok_or_else(|| CliError::target_not_found("target not found: block"))?;
        if anchor.content_hash != expected_hash {
            return Err(CliError::invalid_args(format!(
                "block hash mismatch: block {block} expected {expected_hash} but found {}",
                anchor.content_hash
            )));
        }
        anchor.content_hash.clone()
    } else {
        String::new()
    };

    let style = options.style;
    let (updated_xml, index) = insert_docx_body_paragraph_xml(&xml, block, &text, style)?;
    write_docx_mutation_output(file, &document_part, &updated_xml, options)?;
    let updated_reports = docx_rich_block_reports(&updated_xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let inserted = updated_reports
        .get(index - 1)
        .ok_or_else(|| CliError::unexpected("inserted block readback missing"))?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(index));
    result.insert("blockId".to_string(), json!(format!("body.b{index}")));
    result.insert("contentHash".to_string(), json!(inserted.content_hash));
    if !anchor_hash.is_empty() {
        result.insert("anchorHash".to_string(), json!(anchor_hash));
        result.insert("insertAfter".to_string(), json!(block));
    }
    if !style.is_empty() {
        result.insert("style".to_string(), json!(style));
    }
    result.insert("text".to_string(), json!(text));
    Ok(Value::Object(result))
}

pub(crate) fn docx_blocks_replace(
    file: &str,
    block: usize,
    expected_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    ensure_docx_package_kind(file, &entries)?;

    let text = resolve_optional_docx_paragraph_text(options.text, options.text_file)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let previous = reports
        .get(block - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: block"))?;
    if previous.content_hash != expected_hash {
        return Err(CliError::invalid_args(format!(
            "block hash mismatch: block {block} expected {expected_hash} but found {}",
            previous.content_hash
        )));
    }

    let style = if options.style.is_empty() && previous.kind == "paragraph" {
        previous.style.clone()
    } else {
        options.style.to_string()
    };
    let original_body_tag = docx_body_tag(&xml)?;
    let original_prefix = docx_body_prefix(&original_body_tag);
    let working = if original_prefix.is_empty() && !style.is_empty() {
        ensure_docx_word_prefix(&xml)?
    } else {
        xml
    };
    let body_tag = docx_body_tag(&working)?;
    let prefix = docx_body_prefix(&body_tag);
    let ranges = docx_body_block_ranges(&working, &body_tag)?;
    let target_range = ranges
        .get(block - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: block"))?;
    let target_fragment = &working[target_range.start..target_range.end];
    if docx_block_has_section_properties(target_fragment) {
        return Err(CliError::invalid_args(format!(
            "block contains section properties: block {block}"
        )));
    }

    let replacement = render_docx_paragraph(&prefix, &text, &style);
    let mut updated_xml = String::with_capacity(working.len() + replacement.len());
    updated_xml.push_str(&working[..target_range.start]);
    updated_xml.push_str(&replacement);
    updated_xml.push_str(&working[target_range.end..]);

    write_docx_mutation_output(file, &document_part, &updated_xml, options)?;
    let updated_report = docx_rich_block_reports(&updated_xml, true)
        .map_err(|err| {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        })?
        .into_iter()
        .nth(block - 1)
        .ok_or_else(|| CliError::unexpected("replaced block readback missing"))?;
    let content_hash = updated_report.content_hash.clone();
    let destination = docx_rich_block_json(updated_report);

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(block));
    result.insert("blockId".to_string(), json!(format!("body.b{block}")));
    result.insert("contentHash".to_string(), json!(content_hash));
    result.insert("previousKind".to_string(), json!(previous.kind));
    result.insert("previousHash".to_string(), json!(previous.content_hash));
    result.insert("previousText".to_string(), json!(previous.text));
    if !style.is_empty() {
        result.insert("style".to_string(), json!(style));
    }
    result.insert("text".to_string(), json!(text));
    result.insert("destination".to_string(), destination);
    Ok(Value::Object(result))
}

pub(crate) fn docx_blocks_delete(
    file: &str,
    block: usize,
    expected_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    ensure_docx_package_kind(file, &entries)?;

    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let previous = reports
        .get(block - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: block"))?;
    if reports.len() <= 1 {
        return Err(CliError::invalid_args("cannot delete the last body block"));
    }

    let body_tag = docx_body_tag(&xml)?;
    let ranges = docx_body_block_ranges(&xml, &body_tag)?;
    let target_range = ranges
        .get(block - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: block"))?;
    let target_fragment = &xml[target_range.start..target_range.end];
    if docx_block_has_section_properties(target_fragment) {
        return Err(CliError::invalid_args(format!(
            "block contains section properties: block {block}"
        )));
    }
    if previous.content_hash != expected_hash {
        return Err(CliError::invalid_args(format!(
            "block hash mismatch: block {block} expected {expected_hash} but found {}",
            previous.content_hash
        )));
    }

    let mut updated_xml = String::with_capacity(xml.len());
    updated_xml.push_str(&xml[..target_range.start]);
    updated_xml.push_str(&xml[target_range.end..]);

    write_docx_mutation_output(file, &document_part, &updated_xml, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(block));
    result.insert("blockId".to_string(), json!(format!("body.b{block}")));
    result.insert("previousKind".to_string(), json!(previous.kind));
    result.insert("previousHash".to_string(), json!(previous.content_hash));
    result.insert("previousText".to_string(), json!(previous.text));
    Ok(Value::Object(result))
}
