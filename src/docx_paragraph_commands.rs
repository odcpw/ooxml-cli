use serde_json::{Map, Value, json};
use std::fs;

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, InspectPackageKind,
    append_docx_body_paragraph_xml, copy_zip_with_part_override, detect_inspect_package_type,
    docx_mutation_temp_path, docx_rich_block_reports, ensure_docx_package_kind,
    find_docx_document_part, insert_docx_body_paragraph_xml, package_type,
    resolve_docx_paragraph_handle_index, resolve_optional_docx_paragraph_text,
    set_or_clear_docx_body_paragraph_xml, validate, validate_xlsx_mutation_output_flags,
    write_docx_mutation_output, zip_entry_names, zip_text,
};

pub(crate) fn docx_paragraphs_append(
    file: &str,
    options: DocxParagraphMutationOptions<'_>,
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
    let block_count = docx_rich_block_reports(&xml, false)
        .map_err(|err| {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        })?
        .len();
    let updated_xml = append_docx_body_paragraph_xml(&xml, &text, options.style)?;

    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        docx_mutation_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_override(file, &readback_path, &document_part, &updated_xml)?;
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

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(block_count + 1));
    if !options.style.is_empty() {
        result.insert("style".to_string(), json!(options.style));
    }
    result.insert("text".to_string(), json!(text));
    Ok(Value::Object(result))
}

pub(crate) fn docx_paragraphs_insert(
    file: &str,
    insert_after: i64,
    options: DocxParagraphMutationOptions<'_>,
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
    let (updated_xml, index) =
        insert_docx_body_paragraph_xml(&xml, insert_after as usize, &text, options.style)?;

    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        docx_mutation_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_override(file, &readback_path, &document_part, &updated_xml)?;
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

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(index));
    result.insert("insertAfter".to_string(), json!(insert_after));
    if !options.style.is_empty() {
        result.insert("style".to_string(), json!(options.style));
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
