use serde_json::{Map, Value, json};
use std::fs;

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, DocxStyleTarget, InspectPackageKind,
    append_docx_body_paragraph_xml, copy_zip_with_part_override, copy_zip_with_part_overrides,
    detect_inspect_package_type, docx_rich_block_reports, ensure_docx_package_kind,
    find_docx_document_part, insert_docx_body_paragraph_xml, package_type,
    resolve_docx_paragraph_handle_index, resolve_optional_docx_paragraph_text,
    set_or_clear_docx_body_paragraph_xml, validate_xlsx_mutation_output_flags,
    write_docx_mutation_output, zip_entry_names, zip_text,
};

pub(crate) fn docx_paragraphs_append(
    file: &str,
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
    let prepared = crate::docx_styles::prepare_docx_style_for_mutation(
        file,
        options.style,
        DocxStyleTarget::Paragraph,
        create_style,
    )?;
    let block_count = docx_rich_block_reports(&xml, false)
        .map_err(|err| {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        })?
        .len();
    let updated_xml = append_docx_body_paragraph_xml(&xml, &text, &prepared.style_id)?;

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
    result.insert("text".to_string(), json!(text));
    Ok(Value::Object(result))
}

pub(crate) fn docx_paragraphs_insert(
    file: &str,
    insert_after: i64,
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
    let prepared = crate::docx_styles::prepare_docx_style_for_mutation(
        file,
        options.style,
        DocxStyleTarget::Paragraph,
        create_style,
    )?;
    let (updated_xml, index) =
        insert_docx_body_paragraph_xml(&xml, insert_after as usize, &text, &prepared.style_id)?;

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
    result.insert("text".to_string(), json!(text));
    Ok(Value::Object(result))
}

pub(crate) fn docx_paragraphs_set(
    file: &str,
    index: i64,
    handle: Option<&str>,
    replacement: &str,
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
