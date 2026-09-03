use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::fs;

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, DocxStyleTarget, InspectPackageKind,
    append_docx_body_paragraph_xml, copy_zip_with_part_override, copy_zip_with_part_overrides,
    detect_inspect_package_type, docx_rich_block_reports, ensure_docx_package_kind,
    find_docx_document_part, package_type, resolve_docx_paragraph_handle_index,
    resolve_optional_docx_paragraph_text, set_or_clear_docx_body_paragraph_xml,
    validate_xlsx_mutation_output_flags, write_docx_mutation_output, zip_entry_names, zip_text,
};

pub(crate) fn docx_paragraphs_append(
    file: &str,
    list: Option<&str>,
    level: u32,
    restart: bool,
    options: DocxParagraphMutationOptions<'_>,
    create_style: bool,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    let text = resolve_optional_docx_paragraph_text(options.text, options.text_file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
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
    let xml = zip_text(file, &document_part)?;
    let list_kind = normalize_docx_list_options(list, level, restart)?;
    let requested_style = list_kind.map(docx_list_style_id).unwrap_or(options.style);
    if list_kind.is_some()
        && !options.style.is_empty()
        && options.style != requested_style
        && !options.style.eq_ignore_ascii_case(requested_style)
    {
        return Err(CliError::invalid_args(format!(
            "--style must be {requested_style} when --list is used"
        )));
    }
    let mut prepared = crate::docx_styles::prepare_docx_style_for_mutation(
        file,
        requested_style,
        DocxStyleTarget::Paragraph,
        create_style,
    )?;
    let block_count = docx_rich_block_reports(&xml, false)
        .map_err(|err| {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        })?
        .len();
    let numbering =
        prepare_docx_list_numbering(file, list_kind, level, restart, &mut prepared.overrides)?;
    let updated_xml = if numbering.is_some() {
        crate::docx_xml::append_docx_body_paragraph_xml_with_numbering(
            &xml,
            &text,
            &prepared.style_id,
            numbering,
        )?
    } else {
        append_docx_body_paragraph_xml(&xml, &text, &prepared.style_id)?
    };

    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let readback_path = crate::mutation_staging_path(file, output_path, "docx-paragraph");
    if prepared.overrides.is_empty() {
        copy_zip_with_part_override(file, &readback_path, &document_part, &updated_xml)?;
    } else {
        let mut overrides = prepared.overrides;
        overrides.insert(document_part, updated_xml);
        copy_zip_with_part_overrides(file, &readback_path, &overrides)?;
    }
    if !options.no_validate {
        crate::validate_owned_mutation_output(&readback_path)?;
    }
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
    result.insert("index".to_string(), json!(block_count + 1));
    if !prepared.style_id.is_empty() {
        result.insert("style".to_string(), json!(prepared.style_id));
    }
    if create_style {
        result.insert("createdStyle".to_string(), json!(prepared.created));
    }
    add_docx_list_readback(&mut result, list_kind, numbering, restart);
    result.insert("text".to_string(), json!(text));
    Ok(Value::Object(result))
}

pub(crate) fn docx_paragraphs_insert(
    file: &str,
    insert_after: i64,
    expected_hash: &str,
    list: Option<&str>,
    level: u32,
    restart: bool,
    options: DocxParagraphMutationOptions<'_>,
    create_style: bool,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    if insert_after < 0 {
        return Err(CliError::invalid_args("--insert-after must be >= 0"));
    }
    let text = resolve_optional_docx_paragraph_text(options.text, options.text_file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
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
    let xml = zip_text(file, &document_part)?;
    if insert_after > 0 {
        let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        })?;
        let anchor = reports
            .get(insert_after as usize - 1)
            .ok_or_else(|| CliError::target_not_found("target not found: block"))?;
        if !expected_hash.is_empty() && expected_hash != anchor.content_hash {
            return Err(CliError::invalid_args(format!(
                "block hash mismatch: block {insert_after} expected {expected_hash} but found {}",
                anchor.content_hash
            )));
        }
    } else if !expected_hash.is_empty() {
        return Err(CliError::invalid_args(
            "--expect-hash cannot be used with --after 0",
        ));
    }
    let list_kind = normalize_docx_list_options(list, level, restart)?;
    let requested_style = list_kind.map(docx_list_style_id).unwrap_or(options.style);
    if list_kind.is_some()
        && !options.style.is_empty()
        && options.style != requested_style
        && !options.style.eq_ignore_ascii_case(requested_style)
    {
        return Err(CliError::invalid_args(format!(
            "--style must be {requested_style} when --list is used"
        )));
    }
    let mut prepared = crate::docx_styles::prepare_docx_style_for_mutation(
        file,
        requested_style,
        DocxStyleTarget::Paragraph,
        create_style,
    )?;
    let numbering =
        prepare_docx_list_numbering(file, list_kind, level, restart, &mut prepared.overrides)?;
    let (updated_xml, index) = crate::docx_xml::insert_docx_body_paragraph_xml_with_numbering(
        &xml,
        insert_after as usize,
        &text,
        &prepared.style_id,
        numbering,
    )?;

    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let readback_path = crate::mutation_staging_path(file, output_path, "docx-paragraph");
    if prepared.overrides.is_empty() {
        copy_zip_with_part_override(file, &readback_path, &document_part, &updated_xml)?;
    } else {
        let mut overrides = prepared.overrides;
        overrides.insert(document_part, updated_xml);
        copy_zip_with_part_overrides(file, &readback_path, &overrides)?;
    }
    if !options.no_validate {
        crate::validate_owned_mutation_output(&readback_path)?;
    }
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
    result.insert("index".to_string(), json!(index));
    result.insert("insertAfter".to_string(), json!(insert_after));
    if !prepared.style_id.is_empty() {
        result.insert("style".to_string(), json!(prepared.style_id));
    }
    if create_style {
        result.insert("createdStyle".to_string(), json!(prepared.created));
    }
    add_docx_list_readback(&mut result, list_kind, numbering, restart);
    result.insert("text".to_string(), json!(text));
    Ok(Value::Object(result))
}

fn normalize_docx_list_options(
    list: Option<&str>,
    level: u32,
    restart: bool,
) -> CliResult<Option<&'static str>> {
    let Some(list) = list.filter(|value| !value.trim().is_empty()) else {
        if level != 0 || restart {
            return Err(CliError::invalid_args(
                "--level and --restart require --list bullet|number",
            ));
        }
        return Ok(None);
    };
    if level > 2 {
        return Err(CliError::invalid_args("--level must be 0, 1, or 2"));
    }
    match list.trim().to_ascii_lowercase().as_str() {
        "bullet" => Ok(Some("bullet")),
        "number" | "numbered" => Ok(Some("number")),
        _ => Err(CliError::invalid_args("--list must be bullet or number")),
    }
}

fn docx_list_style_id(kind: &str) -> &'static str {
    if kind == "bullet" {
        "ListBullet"
    } else {
        "ListNumber"
    }
}

fn prepare_docx_list_numbering(
    file: &str,
    kind: Option<&str>,
    level: u32,
    restart: bool,
    overrides: &mut std::collections::BTreeMap<String, String>,
) -> CliResult<Option<(u32, u32)>> {
    let Some(kind) = kind else {
        return Ok(None);
    };
    let base_num_id = if kind == "bullet" { 1 } else { 2 };
    if !restart {
        return Ok(Some((base_num_id, level)));
    }

    let part = "word/numbering.xml";
    let xml = overrides
        .get(part)
        .cloned()
        .or_else(|| zip_text(file, part).ok())
        .ok_or_else(|| {
            CliError::invalid_args(
                "--restart requires a numbering part; use a DOCX scaffold or --create-style",
            )
        })?;
    let next_num_id = next_docx_num_id(&xml)?;
    let abstract_id = if kind == "bullet" { 0 } else { 1 };
    let closing = if xml.contains("</w:numbering>") {
        "</w:numbering>"
    } else {
        "</numbering>"
    };
    let insert_at = xml
        .rfind(closing)
        .ok_or_else(|| CliError::unexpected("invalid DOCX numbering XML"))?;
    let prefix = if closing.starts_with("</w:") {
        "w:"
    } else {
        ""
    };
    let instance = format!(
        "<{prefix}num {prefix}numId=\"{next_num_id}\"><{prefix}abstractNumId {prefix}val=\"{abstract_id}\"/><{prefix}lvlOverride {prefix}ilvl=\"{level}\"><{prefix}startOverride {prefix}val=\"1\"/></{prefix}lvlOverride></{prefix}num>"
    );
    let mut updated = String::with_capacity(xml.len() + instance.len());
    updated.push_str(&xml[..insert_at]);
    updated.push_str(&instance);
    updated.push_str(&xml[insert_at..]);
    overrides.insert(part.to_string(), updated);
    Ok(Some((next_num_id, level)))
}

fn next_docx_num_id(xml: &str) -> CliResult<u32> {
    let mut reader = Reader::from_str(xml);
    let mut max_id = 0u32;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if crate::local_name(element.name().as_ref()) == "num" =>
            {
                if let Some(value) =
                    crate::attr(&element, "numId").and_then(|value| value.parse::<u32>().ok())
                {
                    max_id = max_id.max(value);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(CliError::unexpected(error.to_string())),
            _ => {}
        }
    }
    Ok(max_id.saturating_add(1).max(1))
}

fn add_docx_list_readback(
    result: &mut Map<String, Value>,
    kind: Option<&str>,
    numbering: Option<(u32, u32)>,
    restart: bool,
) {
    let Some(kind) = kind else {
        return;
    };
    result.insert("list".to_string(), json!(kind));
    if let Some((num_id, level)) = numbering {
        result.insert("listLevel".to_string(), json!(level));
        result.insert("numId".to_string(), json!(num_id));
    }
    result.insert("restarted".to_string(), json!(restart));
}

pub(crate) fn docx_paragraphs_set(
    file: &str,
    index: i64,
    handle: Option<&str>,
    replacement: &str,
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
    let target_index = if let Some(handle_arg) = handle.filter(|value| !value.is_empty()) {
        resolve_docx_paragraph_handle_index(&xml, handle_arg)?
    } else {
        index as usize
    };
    validate_docx_paragraph_block_hash(&xml, target_index, expected_hash)?;
    let mutation = set_or_clear_docx_body_paragraph_xml(&xml, target_index, Some(replacement))?;
    write_docx_mutation_output(file, &document_part, &mutation.xml, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(mutation.index));
    if !mutation.style.is_empty() {
        result.insert("style".to_string(), json!(mutation.style));
    }
    result.insert("text".to_string(), json!(replacement));
    result.insert("previousText".to_string(), json!(mutation.previous_text));
    result.insert("flattened".to_string(), json!(mutation.flattened));
    if !mutation.handle.is_empty() {
        result.insert("handle".to_string(), json!(mutation.handle));
    }
    Ok(Value::Object(result))
}

pub(crate) fn docx_paragraphs_clear(
    file: &str,
    index: i64,
    handle: Option<&str>,
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
    let target_index = if let Some(handle_arg) = handle.filter(|value| !value.is_empty()) {
        resolve_docx_paragraph_handle_index(&xml, handle_arg)?
    } else {
        index as usize
    };
    validate_docx_paragraph_block_hash(&xml, target_index, expected_hash)?;
    let mutation = set_or_clear_docx_body_paragraph_xml(&xml, target_index, None)?;
    write_docx_mutation_output(file, &document_part, &mutation.xml, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(mutation.index));
    if !mutation.style.is_empty() {
        result.insert("style".to_string(), json!(mutation.style));
    }
    result.insert("previousText".to_string(), json!(mutation.previous_text));
    if !mutation.handle.is_empty() {
        result.insert("handle".to_string(), json!(mutation.handle));
    }
    Ok(Value::Object(result))
}

fn validate_docx_paragraph_block_hash(
    xml: &str,
    target_index: usize,
    expected_hash: &str,
) -> CliResult<()> {
    if expected_hash.is_empty() {
        return Ok(());
    }
    let report = docx_rich_block_reports(xml, false)?
        .into_iter()
        .find(|report| report.index == target_index && report.kind == "paragraph")
        .ok_or_else(|| CliError::target_not_found("target not found: paragraph"))?;
    if report.content_hash != expected_hash {
        return Err(CliError::invalid_args(format!(
            "block hash mismatch: block {target_index} expected {expected_hash} but found {}",
            report.content_hash
        )));
    }
    Ok(())
}

pub(crate) fn resolve_required_docx_paragraph_set_text(
    text: Option<&str>,
    text_file: Option<&str>,
    text_changed: bool,
    text_file_changed: bool,
) -> CliResult<String> {
    if text_changed == text_file_changed {
        return Err(CliError::invalid_args(
            "must specify exactly one of --text or --text-file",
        ));
    }
    if text_changed {
        let value = text.unwrap_or_default();
        if value.is_empty() {
            return Err(CliError::invalid_args(
                "--text cannot be empty; use docx paragraphs clear",
            ));
        }
        return Ok(value.to_string());
    }
    let path = text_file.unwrap_or_default();
    let data =
        fs::read(path).map_err(|_| CliError::file_not_found(format!("file not found: {path}")))?;
    if data.is_empty() {
        return Err(CliError::invalid_args(
            "--text-file cannot be empty; use docx paragraphs clear",
        ));
    }
    Ok(String::from_utf8_lossy(&data).to_string())
}
