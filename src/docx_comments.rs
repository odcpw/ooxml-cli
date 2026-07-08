use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, EXIT_TARGET_NOT_FOUND, HANDLE_SCOPE_STALE,
    InspectPackageKind, allocate_relationship_id, attr, copy_zip_with_part_overrides,
    detect_inspect_package_type, docx_handle_error, docx_mutation_temp_path,
    docx_rich_block_reports, ensure_content_type_override, find_docx_document_part, local_name,
    package_type, relationship_entries, relationship_target_from_source_to_target,
    relationships_part_for, resolve_optional_docx_paragraph_text, resolve_relationship_target,
    validate, validate_xlsx_mutation_output_flags, xml_attr_escape, xml_tag_prefix, xml_token_name,
    zip_entry_exists, zip_entry_names, zip_text,
};

mod handles;
mod markers;
mod read;
mod render;

use handles::resolve_docx_comment_handle_id;
use markers::remove_docx_comment_markers_xml;
pub(crate) use read::docx_comments_list;
use read::{DocxCommentInfo, docx_comment_content_hash, docx_comment_info_from_fragment};
use render::{
    append_docx_comment_xml, docx_comments_template, docx_next_comment_id,
    insert_docx_comment_markers_xml, render_docx_comment,
};

pub(crate) fn docx_comments_add(
    file: &str,
    anchor_block: i64,
    author: &str,
    initials: &str,
    date: &str,
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
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        let detected = match detect_inspect_package_type(file, &entries) {
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
    let document_xml = zip_text(file, &document_part)?;
    let anchor_index = if anchor_block == 0 {
        1
    } else {
        anchor_block as usize
    };
    let reports = docx_rich_block_reports(&document_xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    if reports.is_empty() {
        return Err(CliError::unexpected(
            "failed to mutate comments: document has no body blocks to anchor a comment to",
        ));
    }
    let report = reports.get(anchor_index.saturating_sub(1)).ok_or_else(|| {
        CliError::invalid_args(format!("comment anchor block out of range: {anchor_index}"))
    })?;
    if report.kind != "paragraph" {
        return Err(CliError::invalid_args(format!(
            "comment anchor block is not a paragraph: block {anchor_index} is {}",
            report.kind
        )));
    }

    let comments_part = docx_comments_part_uri(file, &entries, &document_part)?;
    let comments_part_key = comments_part.trim_start_matches('/').to_string();
    let created_part = !zip_entry_exists(&entries, &comments_part);
    let comments_xml = if created_part {
        docx_comments_template()
    } else {
        zip_text(file, &comments_part_key)?
    };
    let comment_id = docx_next_comment_id(&comments_xml);
    let updated_document_xml =
        insert_docx_comment_markers_xml(&document_xml, anchor_index, comment_id)?;
    let updated_comments_xml =
        append_docx_comment_xml(&comments_xml, comment_id, author, date, initials, &text)?;
    let (rels_part, rels_xml, created_ref) =
        ensure_docx_comments_relationship_xml(file, &document_part, &document_uri, &comments_part)?;
    let content_types_xml = ensure_content_type_override(
        zip_text(file, "[Content_Types].xml")?,
        &comments_part,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml",
    )?;

    let mut overrides = BTreeMap::new();
    overrides.insert(document_part.clone(), updated_document_xml);
    overrides.insert(comments_part_key, updated_comments_xml);
    overrides.insert("[Content_Types].xml".to_string(), content_types_xml);
    if let Some(rels_xml) = rels_xml {
        overrides.insert(rels_part, rels_xml);
    }
    write_docx_mutation_overrides_output(file, &overrides, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("commentId".to_string(), json!(comment_id));
    result.insert("author".to_string(), json!(author));
    if !date.is_empty() {
        result.insert("date".to_string(), json!(date));
    }
    if !initials.is_empty() {
        result.insert("initials".to_string(), json!(initials));
    }
    result.insert("text".to_string(), json!(text));
    result.insert(
        "contentHash".to_string(),
        json!(docx_comment_content_hash(author, date, &text)),
    );
    result.insert("anchoredToBlock".to_string(), json!(anchor_index));
    result.insert("createdPart".to_string(), json!(created_part));
    result.insert("createdRef".to_string(), json!(created_ref));
    result.insert("operation".to_string(), json!("added"));
    Ok(Value::Object(result))
}

pub(crate) struct DocxCommentEditSpec<'a> {
    pub(crate) expect_hash: &'a str,
    pub(crate) text: &'a str,
    pub(crate) text_set: bool,
    pub(crate) author: &'a str,
    pub(crate) author_set: bool,
    pub(crate) date: &'a str,
    pub(crate) date_set: bool,
}

pub(crate) fn docx_comments_edit(
    file: &str,
    comment_id: i64,
    handle: Option<&str>,
    edit: DocxCommentEditSpec<'_>,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        let detected = match detect_inspect_package_type(file, &entries) {
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
    let comments_part = docx_comments_part_uri(file, &entries, &document_part)?;
    if !zip_entry_exists(&entries, &comments_part) {
        if let Some(handle) = handle.filter(|value| !value.trim().is_empty()) {
            return Err(docx_handle_error(
                EXIT_TARGET_NOT_FOUND,
                HANDLE_SCOPE_STALE,
                "document has no comments part",
                handle,
            ));
        }
        return Err(CliError::target_not_found("target not found: comment"));
    }
    let comments_part_key = comments_part.trim_start_matches('/').to_string();
    let comments_xml = zip_text(file, &comments_part_key)?;
    let target_id = if let Some(handle) = handle.filter(|value| !value.trim().is_empty()) {
        resolve_docx_comment_handle_id(&comments_xml, handle)? as i64
    } else {
        comment_id
    };
    let (updated_comments_xml, before, edited) =
        edit_docx_comment_xml(&comments_xml, target_id, edit)?;

    let mut overrides = BTreeMap::new();
    overrides.insert(comments_part_key, updated_comments_xml);
    write_docx_mutation_overrides_output(file, &overrides, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("commentId".to_string(), json!(target_id));
    result.insert("author".to_string(), json!(edited.author));
    if !edited.date.is_empty() {
        result.insert("date".to_string(), json!(edited.date));
    }
    if !edited.initials.is_empty() {
        result.insert("initials".to_string(), json!(edited.initials));
    }
    result.insert("text".to_string(), json!(edited.text));
    result.insert(
        "contentHash".to_string(),
        json!(docx_comment_content_hash(
            &edited.author,
            &edited.date,
            &edited.text
        )),
    );
    result.insert("previousText".to_string(), json!(before.text));
    result.insert(
        "previousHash".to_string(),
        json!(docx_comment_content_hash(
            &before.author,
            &before.date,
            &before.text
        )),
    );
    result.insert("operation".to_string(), json!("edited"));
    result.insert(
        "handle".to_string(),
        json!(format!("H:docx/pt:doc/comment:n:{target_id}")),
    );
    Ok(Value::Object(result))
}

pub(crate) fn docx_comments_remove(
    file: &str,
    comment_id: i64,
    handle: Option<&str>,
    expect_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        let detected = match detect_inspect_package_type(file, &entries) {
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
    let comments_part = docx_comments_part_uri(file, &entries, &document_part)?;
    if !zip_entry_exists(&entries, &comments_part) {
        if let Some(handle) = handle.filter(|value| !value.trim().is_empty()) {
            return Err(docx_handle_error(
                EXIT_TARGET_NOT_FOUND,
                HANDLE_SCOPE_STALE,
                "document has no comments part",
                handle,
            ));
        }
        return Err(CliError::target_not_found("target not found: comment"));
    }
    let comments_part_key = comments_part.trim_start_matches('/').to_string();
    let comments_xml = zip_text(file, &comments_part_key)?;
    let target_id = if let Some(handle) = handle.filter(|value| !value.trim().is_empty()) {
        resolve_docx_comment_handle_id(&comments_xml, handle)? as i64
    } else {
        comment_id
    };
    let (updated_comments_xml, before) =
        remove_docx_comment_xml(&comments_xml, target_id, expect_hash)?;
    let document_xml = zip_text(file, &document_part)?;
    let (updated_document_xml, range_markers_removed) =
        remove_docx_comment_markers_xml(&document_xml, target_id)?;

    let mut overrides = BTreeMap::new();
    overrides.insert(comments_part_key, updated_comments_xml);
    if range_markers_removed {
        overrides.insert(document_part.clone(), updated_document_xml);
    }
    write_docx_mutation_overrides_output(file, &overrides, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("commentId".to_string(), json!(target_id));
    result.insert("previousAuthor".to_string(), json!(before.author));
    result.insert("previousText".to_string(), json!(before.text));
    result.insert(
        "previousHash".to_string(),
        json!(docx_comment_content_hash(
            &before.author,
            &before.date,
            &before.text
        )),
    );
    result.insert(
        "rangeMarkersRemoved".to_string(),
        json!(range_markers_removed),
    );
    result.insert("operation".to_string(), json!("removed"));
    Ok(Value::Object(result))
}

fn docx_document_and_comments_parts(file: &str) -> CliResult<(String, Option<String>)> {
    let entries = zip_entry_names(file)?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        return Err(CliError::unsupported_type(
            "file is not a DOCX document (detected: unknown)",
        ));
    }
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let comments_part = find_docx_comments_part(file, &entries, &document_part)?;
    Ok((document_uri, comments_part))
}

fn find_docx_comments_part(
    file: &str,
    entries: &[String],
    document_part: &str,
) -> CliResult<Option<String>> {
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    for rel in
        relationship_entries(file, &relationships_part_for(document_part)).unwrap_or_default()
    {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"
        {
            let uri = resolve_relationship_target(&document_uri, &rel.target);
            return Ok(zip_entry_exists(entries, &uri).then_some(uri));
        }
    }
    let conventional = "/word/comments.xml";
    Ok(zip_entry_exists(entries, conventional).then(|| conventional.to_string()))
}

fn docx_comments_part_uri(
    file: &str,
    entries: &[String],
    document_part: &str,
) -> CliResult<String> {
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    for rel in
        relationship_entries(file, &relationships_part_for(document_part)).unwrap_or_default()
    {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"
        {
            return Ok(resolve_relationship_target(&document_uri, &rel.target));
        }
    }
    let conventional = "/word/comments.xml";
    if zip_entry_exists(entries, conventional) {
        return Ok(conventional.to_string());
    }
    Ok(conventional.to_string())
}

fn ensure_docx_comments_relationship_xml(
    file: &str,
    document_part: &str,
    document_uri: &str,
    comments_part: &str,
) -> CliResult<(String, Option<String>, bool)> {
    let rels_part = relationships_part_for(document_part);
    let rels = relationship_entries(file, &rels_part).unwrap_or_default();
    if rels.iter().any(|rel| {
        rel.target_mode != "External"
            && rel.rel_type
                == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"
    }) {
        return Ok((rels_part, None, false));
    }

    let next_id = allocate_relationship_id(&rels);
    let target = relationship_target_from_source_to_target(document_uri, comments_part);
    let rel = format!(
        r#"<Relationship Id="{next_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="{}"/>"#,
        xml_attr_escape(&target)
    );
    let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| {
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#
            .to_string()
    });
    let updated = if let Some(pos) = rels_xml.rfind("</Relationships>") {
        let mut out = String::with_capacity(rels_xml.len() + rel.len());
        out.push_str(&rels_xml[..pos]);
        out.push_str(&rel);
        out.push_str(&rels_xml[pos..]);
        out
    } else {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rel}</Relationships>"#
        )
    };
    Ok((rels_part, Some(updated), true))
}

fn write_docx_mutation_overrides_output(
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

#[derive(Clone, Copy)]
struct XmlFullElementSpan {
    start: usize,
    end: usize,
    open_end: usize,
}

fn edit_docx_comment_xml(
    comments_xml: &str,
    target_id: i64,
    edit: DocxCommentEditSpec<'_>,
) -> CliResult<(String, DocxCommentInfo, DocxCommentInfo)> {
    let spans = docx_comment_element_spans_by_id(comments_xml, target_id)?;
    let Some(span) = spans.first().copied() else {
        return Err(CliError::target_not_found("target not found: comment"));
    };
    let fragment = &comments_xml[span.start..span.end];
    let (before, paragraph_count) = docx_comment_info_from_fragment(fragment)?;
    let before_hash = docx_comment_content_hash(&before.author, &before.date, &before.text);
    if !edit.expect_hash.is_empty() && edit.expect_hash != before_hash {
        return Err(CliError::invalid_args(format!(
            "comment hash mismatch: comment {target_id} expected {} but found {before_hash}",
            edit.expect_hash
        )));
    }
    if edit.text_set && paragraph_count > 1 {
        return Err(CliError::unexpected(format!(
            "failed to mutate comments: comment {target_id} has {paragraph_count} paragraphs; editing its text would discard structure (remove and re-add the comment instead)"
        )));
    }

    let mut edited = before.clone();
    if edit.author_set {
        edited.author = edit.author.to_string();
    }
    if edit.date_set {
        edited.date = edit.date.to_string();
    }
    if edit.text_set {
        edited.text = edit.text.to_string();
    }

    let tag_name = xml_token_name(&fragment[1..span.open_end - span.start - 1])
        .ok_or_else(|| CliError::unexpected("invalid comment XML"))?;
    let prefix = xml_tag_prefix(tag_name);
    let rendered = render_docx_comment(
        &prefix,
        target_id,
        &edited.author,
        &edited.date,
        &edited.initials,
        &edited.text,
    );
    let mut out = String::with_capacity(comments_xml.len() + rendered.len());
    out.push_str(&comments_xml[..span.start]);
    out.push_str(&rendered);
    out.push_str(&comments_xml[span.end..]);
    Ok((out, before, edited))
}

fn remove_docx_comment_xml(
    comments_xml: &str,
    target_id: i64,
    expect_hash: &str,
) -> CliResult<(String, DocxCommentInfo)> {
    let spans = docx_comment_element_spans_by_id(comments_xml, target_id)?;
    let Some(span) = spans.first().copied() else {
        return Err(CliError::target_not_found("target not found: comment"));
    };
    let fragment = &comments_xml[span.start..span.end];
    let (before, _) = docx_comment_info_from_fragment(fragment)?;
    let before_hash = docx_comment_content_hash(&before.author, &before.date, &before.text);
    if !expect_hash.is_empty() && expect_hash != before_hash {
        return Err(CliError::invalid_args(format!(
            "comment hash mismatch: comment {target_id} expected {expect_hash} but found {before_hash}"
        )));
    }

    let mut out = String::with_capacity(comments_xml.len().saturating_sub(span.end - span.start));
    out.push_str(&comments_xml[..span.start]);
    out.push_str(&comments_xml[span.end..]);
    Ok((out, before))
}

fn docx_comment_element_spans_by_id(
    comments_xml: &str,
    target_id: i64,
) -> CliResult<Vec<XmlFullElementSpan>> {
    let target = target_id.to_string();
    let mut reader = Reader::from_str(comments_xml);
    reader.config_mut().trim_text(false);
    let mut spans = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "comment" => {
                let open_end = reader.buffer_position() as usize;
                let matches = attr(&e, "id").is_some_and(|id| id == target);
                let mut depth = 1usize;
                loop {
                    match reader.read_event() {
                        Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "comment" => {
                            depth += 1;
                        }
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "comment" => {
                            depth -= 1;
                            if depth == 0 {
                                if matches {
                                    spans.push(XmlFullElementSpan {
                                        start: before,
                                        end: reader.buffer_position() as usize,
                                        open_end,
                                    });
                                }
                                break;
                            }
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("invalid comments XML"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "comment" => {
                if attr(&e, "id").is_some_and(|id| id == target) {
                    spans.push(XmlFullElementSpan {
                        start: before,
                        end: reader.buffer_position() as usize,
                        open_end: reader.buffer_position() as usize,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(spans)
}
