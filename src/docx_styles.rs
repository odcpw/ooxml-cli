use serde_json::{Map, Value, json};

mod mutation;
mod read;

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, EXIT_INVALID_ARGS, EXIT_TARGET_NOT_FOUND,
    HANDLE_AMBIGUOUS, HANDLE_FORMAT_MISMATCH, HANDLE_MALFORMED, HANDLE_SCOPE_STALE, HANDLE_STALE,
    docx_body_block_ranges, docx_body_tag, docx_handle_error, docx_rich_block_reports,
    ensure_docx_package_kind, resolve_docx_paragraph_handle_index,
    validate_xlsx_mutation_output_flags, write_docx_mutation_output, zip_entry_names, zip_text,
};

use mutation::{apply_docx_style_xml, docx_first_run_style, docx_table_style};
use read::{DocxStyleInfo, docx_document_and_styles_parts, docx_styles};
pub(crate) use read::{docx_styles_list, docx_styles_show};

pub(crate) struct DocxStyleApplyOptions<'a> {
    pub(crate) index: i64,
    pub(crate) handle: Option<&'a str>,
    pub(crate) target: DocxStyleTarget,
    pub(crate) style: &'a str,
    pub(crate) expected_hash: &'a str,
    pub(crate) validate_style: bool,
    pub(crate) mutation: DocxParagraphMutationOptions<'a>,
}

pub(crate) fn docx_styles_apply(
    file: &str,
    request: DocxStyleApplyOptions<'_>,
) -> CliResult<Value> {
    let DocxStyleApplyOptions {
        index,
        handle,
        target,
        style,
        expected_hash,
        validate_style,
        mutation: options,
    } = request;
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    ensure_docx_package_kind(file, &entries)?;

    let (document_uri, styles_part) = docx_document_and_styles_parts(file)?;
    let document_part = document_uri.trim_start_matches('/').to_string();
    let styles = if let Some(styles_part) = styles_part.as_deref() {
        docx_styles(file, styles_part)?
    } else {
        Vec::new()
    };

    let mut style_id = style.trim().to_string();
    let mut style_handle = String::new();
    if style_id.starts_with("H:") {
        style_handle = style_id.clone();
        style_id = resolve_docx_style_handle_id(&styles, styles_part.as_deref(), &style_id)?;
    }
    if validate_style {
        validate_docx_style_for_target(&styles, &style_id, target)?;
    }

    let xml = zip_text(file, &document_part)?;
    let target_index = if let Some(handle_arg) = handle.filter(|value| !value.is_empty()) {
        resolve_docx_paragraph_handle_index(&xml, handle_arg)?
    } else {
        index as usize
    };
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;

    let (result_index, block_index, block_kind, previous_style, previous_hash, para_id) =
        match target {
            DocxStyleTarget::Paragraph | DocxStyleTarget::Run => {
                let report = reports.get(target_index.saturating_sub(1)).ok_or_else(|| {
                    CliError::target_not_found(format!(
                        "target not found: {} block {target_index}",
                        target.as_str()
                    ))
                })?;
                if report.kind != "paragraph" {
                    return Err(CliError::invalid_args(format!(
                        "block {target_index} is a table, not a paragraph"
                    )));
                }
                if !expected_hash.is_empty() && expected_hash != report.content_hash {
                    return Err(CliError::invalid_args(format!(
                        "block hash mismatch: block {} expected {} but found {}",
                        report.index, expected_hash, report.content_hash
                    )));
                }
                let previous_style = if target == DocxStyleTarget::Run {
                    let body_tag = docx_body_tag(&xml)?;
                    let blocks = docx_body_block_ranges(&xml, &body_tag)?;
                    let block = blocks.get(target_index - 1).ok_or_else(|| {
                        CliError::target_not_found(format!(
                            "target not found: {} block {target_index}",
                            target.as_str()
                        ))
                    })?;
                    docx_first_run_style(&xml[block.start..block.end])?
                } else {
                    report.style.clone()
                };
                (
                    report.index,
                    report.index,
                    report.kind.to_string(),
                    previous_style,
                    report.content_hash.clone(),
                    report.para_id.clone(),
                )
            }
            DocxStyleTarget::Table => {
                let mut seen = 0usize;
                let mut selected = None;
                for report in &reports {
                    if report.kind == "table" {
                        seen += 1;
                        if seen == target_index {
                            selected = Some(report);
                            break;
                        }
                    }
                }
                let report = selected.ok_or_else(|| {
                    CliError::target_not_found(format!("target not found: table {target_index}"))
                })?;
                if !expected_hash.is_empty() && expected_hash != report.content_hash {
                    return Err(CliError::invalid_args(format!(
                        "block hash mismatch: block {} expected {} but found {}",
                        report.index, expected_hash, report.content_hash
                    )));
                }
                let body_tag = docx_body_tag(&xml)?;
                let blocks = docx_body_block_ranges(&xml, &body_tag)?;
                let block = blocks.get(report.index - 1).ok_or_else(|| {
                    CliError::target_not_found(format!("target not found: table {target_index}"))
                })?;
                (
                    target_index,
                    report.index,
                    report.kind.to_string(),
                    docx_table_style(&xml[block.start..block.end])?,
                    report.content_hash.clone(),
                    String::new(),
                )
            }
        };

    let updated_xml = apply_docx_style_xml(&xml, target, block_index, &style_id, para_id.trim())?;
    write_docx_mutation_output(file, &document_part, &updated_xml, options)?;

    let updated_reports = docx_rich_block_reports(&updated_xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let updated_report = updated_reports
        .get(block_index - 1)
        .ok_or_else(|| CliError::unexpected("styled block disappeared after mutation"))?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(result_index));
    result.insert("blockIndex".to_string(), json!(block_index));
    result.insert("blockId".to_string(), json!(format!("body.b{block_index}")));
    result.insert("blockKind".to_string(), json!(block_kind));
    result.insert("target".to_string(), json!(target.as_str()));
    if !previous_style.is_empty() {
        result.insert("previousStyle".to_string(), json!(previous_style));
    }
    result.insert("style".to_string(), json!(style_id));
    result.insert(
        "contentHash".to_string(),
        json!(updated_report.content_hash),
    );
    result.insert("previousHash".to_string(), json!(previous_hash));
    if matches!(target, DocxStyleTarget::Paragraph | DocxStyleTarget::Run)
        && !updated_report.para_id.is_empty()
    {
        result.insert(
            "handle".to_string(),
            json!(format!("H:docx/pt:doc/para:m:{}", updated_report.para_id)),
        );
    }
    if !style_handle.is_empty() {
        result.insert("styleHandle".to_string(), json!(style_handle));
    }
    Ok(Value::Object(result))
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum DocxStyleTarget {
    Paragraph,
    Run,
    Table,
}

impl DocxStyleTarget {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            DocxStyleTarget::Paragraph => "paragraph",
            DocxStyleTarget::Run => "run",
            DocxStyleTarget::Table => "table",
        }
    }

    fn required_style_type(self) -> &'static str {
        match self {
            DocxStyleTarget::Paragraph => "paragraph",
            DocxStyleTarget::Run => "character",
            DocxStyleTarget::Table => "table",
        }
    }
}

pub(crate) fn normalize_docx_style_target(value: &str) -> CliResult<DocxStyleTarget> {
    match value.trim().to_ascii_lowercase().as_str() {
        "paragraph" => Ok(DocxStyleTarget::Paragraph),
        "run" => Ok(DocxStyleTarget::Run),
        "table" => Ok(DocxStyleTarget::Table),
        _ => Err(CliError::invalid_args(
            "--target must be one of paragraph, run, table",
        )),
    }
}

fn validate_docx_style_for_target(
    styles: &[DocxStyleInfo],
    style_id: &str,
    target: DocxStyleTarget,
) -> CliResult<()> {
    let wanted = target.required_style_type();
    if let Some(style) = styles.iter().find(|style| style.style_id == style_id) {
        if style.style_type != wanted {
            return Err(CliError::invalid_args(format!(
                "style type mismatch: {:?} is a {} style but {} target requires a {} style",
                style_id,
                style.style_type,
                target.as_str(),
                wanted
            )));
        }
        return Ok(());
    }
    let mut candidates: Vec<&str> = styles
        .iter()
        .filter(|style| style.style_type == wanted)
        .map(|style| style.style_id.as_str())
        .collect();
    candidates.sort_unstable();
    let detail = if candidates.is_empty() {
        format!(
            "style not found: {:?} ({}); no {} styles defined",
            style_id, wanted, wanted
        )
    } else {
        format!(
            "style not found: {:?} ({}); available {} styles: [{}]",
            style_id,
            wanted,
            wanted,
            candidates.join(" ")
        )
    };
    Err(CliError::target_not_found(detail))
}

fn resolve_docx_style_handle_id(
    styles: &[DocxStyleInfo],
    styles_part: Option<&str>,
    handle: &str,
) -> CliResult<String> {
    let style_id = parse_docx_style_handle_style_id(handle)?;
    if styles_part.is_none() {
        return Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_SCOPE_STALE,
            "document has no styles part",
            handle,
        ));
    }
    let matches = styles
        .iter()
        .filter(|style| style.style_id == style_id)
        .count();
    match matches {
        0 => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_STALE,
            format!("no style with w:styleId {style_id:?} in document"),
            handle,
        )),
        1 => Ok(style_id),
        count => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_AMBIGUOUS,
            format!("w:styleId {style_id:?} is not unique ({count} styles share it)"),
            handle,
        )),
    }
}

fn parse_docx_style_handle_style_id(handle: &str) -> CliResult<String> {
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
    if segments[1] != "pt:styles" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!(
                "style handle scope must be {:?}, got {:?}",
                "pt:styles", segments[1]
            ),
            handle,
        ));
    }
    if class != "style" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "expected a style handle",
            handle,
        ));
    }
    let Some((tag, value)) = objref.split_once(':') else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("style objref: objref {objref:?} must be n:<value>"),
            handle,
        ));
    };
    if tag != "n" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("style objref: unsupported objref tag {tag:?} (expected native id \"n\")"),
            handle,
        ));
    }
    if value.is_empty() {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "style objref: empty native id",
            handle,
        ));
    }
    Ok(value.to_string())
}
