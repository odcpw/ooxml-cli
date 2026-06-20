use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::{
    CliError, CliResult, DocxRichBlockReport, EXIT_INVALID_ARGS, EXIT_TARGET_NOT_FOUND,
    InspectPackageKind, copy_zip_with_binary_part_overrides_and_removals,
    copy_zip_with_part_override, copy_zip_with_part_overrides, detect_inspect_package_type,
    docx_mutation_temp_path, docx_rich_block_reports, package_type, validate,
};

pub(crate) fn docx_validate_strict_command(file: &str) -> String {
    format!("ooxml validate --strict {file}")
}

pub(crate) fn docx_mutation_output_path_for_result(
    file: &str,
    options: &DocxParagraphMutationOptions<'_>,
) -> Option<String> {
    if options.dry_run {
        None
    } else if options.in_place {
        Some(file.to_string())
    } else {
        options
            .out
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    }
}

pub(crate) fn resolve_required_docx_table_text(
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
        return Ok(text.unwrap_or_default().to_string());
    }
    let path = text_file.unwrap_or_default();
    fs::read(path)
        .map(|data| String::from_utf8_lossy(&data).to_string())
        .map_err(|_| CliError::file_not_found(format!("file not found: {path}")))
}

pub(crate) struct DocxParagraphMutationOptions<'a> {
    pub(crate) text: Option<&'a str>,
    pub(crate) text_file: Option<&'a str>,
    pub(crate) style: &'a str,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) in_place: bool,
    pub(crate) no_validate: bool,
}

pub(crate) fn resolve_optional_docx_paragraph_text(
    text: Option<&str>,
    text_file: Option<&str>,
) -> CliResult<String> {
    match (text, text_file) {
        (Some(_), Some(_)) => Err(CliError::invalid_args(
            "cannot specify both --text and --text-file",
        )),
        (Some(value), None) => Ok(value.to_string()),
        (None, Some(path)) => fs::read(path)
            .map(|data| String::from_utf8_lossy(&data).to_string())
            .map_err(|_| CliError::file_not_found(format!("file not found: {path}"))),
        (None, None) => Ok(String::new()),
    }
}

pub(crate) fn ensure_docx_package_kind(file: &str, entries: &[String]) -> CliResult<()> {
    let package_kind = detect_inspect_package_type(file, entries);
    if package_kind == InspectPackageKind::Docx {
        return Ok(());
    }
    let detected = match package_kind {
        InspectPackageKind::Pptx => "pptx",
        InspectPackageKind::Xlsx => "xlsx",
        InspectPackageKind::Docx => "docx",
        InspectPackageKind::Unknown => package_type(file)?,
    };
    Err(CliError::unsupported_type(format!(
        "file is not a DOCX document (detected: {detected})"
    )))
}

pub(crate) fn write_docx_mutation_output(
    file: &str,
    document_part: &str,
    updated_xml: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<()> {
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
    copy_zip_with_part_override(file, &readback_path, document_part, updated_xml)?;
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
    Ok(())
}

pub(crate) fn write_docx_package_mutation_output(
    file: &str,
    overrides: &BTreeMap<String, String>,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<()> {
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
    copy_zip_with_part_overrides(file, &readback_path, overrides)?;
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
    Ok(())
}

pub(crate) fn write_docx_package_binary_mutation_output(
    file: &str,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<()> {
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
    copy_zip_with_binary_part_overrides_and_removals(
        file,
        &readback_path,
        text_overrides,
        binary_overrides,
        &BTreeSet::new(),
    )?;
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
    Ok(())
}

pub(crate) const HANDLE_MALFORMED: &str = "HANDLE_MALFORMED";
pub(crate) const HANDLE_FORMAT_MISMATCH: &str = "HANDLE_FORMAT_MISMATCH";
pub(crate) const HANDLE_SCOPE_STALE: &str = "HANDLE_SCOPE_STALE";
pub(crate) const HANDLE_STALE: &str = "HANDLE_STALE";
pub(crate) const HANDLE_AMBIGUOUS: &str = "HANDLE_AMBIGUOUS";

pub(crate) fn resolve_docx_paragraph_handle_index(xml: &str, handle: &str) -> CliResult<usize> {
    let para_id = parse_docx_paragraph_handle_para_id(handle)?;
    let reports = docx_rich_block_reports(xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let wanted = para_id.trim().to_ascii_uppercase();
    let matches: Vec<&DocxRichBlockReport> = reports
        .iter()
        .filter(|report| {
            report.kind == "paragraph" && report.para_id.trim().eq_ignore_ascii_case(&wanted)
        })
        .collect();
    match matches.len() {
        0 => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_STALE,
            format!("no paragraph with w14:paraId {para_id:?} in document body"),
            handle,
        )),
        1 => Ok(matches[0].index),
        count => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_AMBIGUOUS,
            format!(
                "w14:paraId {para_id:?} is not unique ({count} paragraphs share it); cannot resolve to a single paragraph"
            ),
            handle,
        )),
    }
}

fn parse_docx_paragraph_handle_para_id(handle: &str) -> CliResult<String> {
    let trimmed = handle.trim();
    let Some(body) = trimmed.strip_prefix("H:") else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "missing handle version prefix \"H:\"",
            handle,
        ));
    };
    let segments: Vec<&str> = body.split('/').collect();
    if segments.len() != 3 {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "handle must be H:docx/<scope>/<class>:<objref>",
            handle,
        ));
    }
    if segments[0].is_empty() {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "empty format tag",
            handle,
        ));
    }
    if segments[0] != "docx" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_FORMAT_MISMATCH,
            format!(
                "handle format tag {:?} does not match package format {:?}",
                segments[0], "docx"
            ),
            handle,
        ));
    }
    let Some((class, objref)) = segments[2].split_once(':') else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("object segment {:?} must be <class>:<objref>", segments[2]),
            handle,
        ));
    };
    if segments[1] != "pt:doc" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!(
                "paragraph handle scope must be {:?}, got {:?}",
                "pt:doc", segments[1]
            ),
            handle,
        ));
    }
    if class != "para" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "expected a paragraph handle",
            handle,
        ));
    }
    let Some((tag, value)) = objref.split_once(':') else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("paragraph objref: objref {objref:?} must be m:<paraId>"),
            handle,
        ));
    };
    if tag != "m" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!(
                "paragraph objref: unsupported objref tag {tag:?} (expected paragraph marker \"m\")"
            ),
            handle,
        ));
    }
    if value.is_empty() {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "paragraph objref: empty paragraph marker",
            handle,
        ));
    }
    Ok(value.to_string())
}

pub(crate) fn docx_handle_error(
    exit_code: i32,
    code: &'static str,
    message: impl Into<String>,
    handle: &str,
) -> CliError {
    CliError {
        code,
        exit_code,
        message: format!("{}: {} (handle {:?})", code, message.into(), handle),
    }
}
