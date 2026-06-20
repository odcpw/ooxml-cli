use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, EXIT_TARGET_NOT_FOUND, HANDLE_SCOPE_STALE,
    InspectPackageKind, XmlNamedRange, allocate_relationship_id, append_docx_text_children, attr,
    copy_zip_with_part_overrides, decode_xml_text, detect_inspect_package_type,
    docx_body_block_ranges, docx_body_content_bounds, docx_body_tag, docx_handle_error,
    docx_mutation_temp_path, docx_rich_block_reports, ensure_content_type_override,
    ensure_docx_word_prefix, find_docx_document_part, local_name, package_type,
    relationship_entries, relationship_target_from_source_to_target, relationships_part_for,
    resolve_optional_docx_paragraph_text, resolve_relationship_target, validate,
    validate_xlsx_mutation_output_flags, word_xml_tag, xml_attr_escape, xml_direct_child_ranges,
    xml_fragment_bounds, xml_open_tag_from_start, xml_tag_prefix, xml_token_name, zip_entry_exists,
    zip_entry_names, zip_text,
};

mod handles;

use handles::resolve_docx_comment_handle_id;

pub(crate) fn docx_comments_list(file: &str, comment_id: Option<i64>) -> CliResult<Value> {
    let (document_part, comments_part) = docx_document_and_comments_parts(file)?;
    let mut comments = Vec::new();
    if let Some(comments_part) = comments_part.as_deref() {
        comments = docx_comments(file, comments_part, &document_part)?;
    }
    if let Some(comment_id) = comment_id {
        comments.retain(|comment| comment.id == comment_id);
        if comments.is_empty() {
            return Err(CliError::target_not_found(format!(
                "target not found: comment {comment_id}"
            )));
        }
    }
    let counts = docx_comment_id_counts(&comments);
    let comment_values = comments
        .iter()
        .map(|comment| docx_comment_json(comment, &counts))
        .collect::<Vec<_>>();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("documentPartUri".to_string(), json!(document_part));
    if let Some(comments_part) = comments_part {
        result.insert("commentsPart".to_string(), json!(comments_part));
    }
    result.insert("comments".to_string(), Value::Array(comment_values));
    Ok(Value::Object(result))
}

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
    );

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

#[derive(Clone, Default)]
struct DocxCommentInfo {
    id: i64,
    id_raw: String,
    id_valid: bool,
    author: String,
    date: String,
    initials: String,
    text: String,
    anchored_to_block: usize,
    anchored_to_block_kind: String,
}

#[derive(Default)]
struct DocxCommentBuild {
    info: DocxCommentInfo,
    paragraphs: Vec<String>,
    current_paragraph: Option<String>,
    in_t: bool,
    skip_text_depth: usize,
}

fn docx_comments(
    file: &str,
    comments_part: &str,
    document_part: &str,
) -> CliResult<Vec<DocxCommentInfo>> {
    let xml = zip_text(file, comments_part.trim_start_matches('/'))?;
    let anchors = docx_comment_anchors(file, document_part)?;
    let mut reader = Reader::from_str(&xml);
    let mut saw_root = false;
    let mut stack = Vec::<String>::new();
    let mut current: Option<DocxCommentBuild> = None;
    let mut comments = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "comments" {
                        return Ok(Vec::new());
                    }
                } else if name == "comment"
                    && stack.last().is_some_and(|parent| parent == "comments")
                {
                    current = Some(docx_comment_from_element(&e));
                } else if let Some(comment) = current.as_mut() {
                    docx_note_comment_start(&e, &name, &stack, comment);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "comments" {
                        return Ok(Vec::new());
                    }
                } else if name == "comment"
                    && stack.last().is_some_and(|parent| parent == "comments")
                {
                    let mut comment = docx_comment_from_element(&e);
                    docx_finish_comment(&mut comment, &anchors);
                    comments.push(comment.info);
                } else if let Some(comment) = current.as_mut() {
                    docx_note_comment_empty(&e, &name, &stack, comment);
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(comment) = current.as_mut()
                    && comment.in_t
                    && comment.skip_text_depth == 0
                    && let Some(paragraph) = comment.current_paragraph.as_mut()
                {
                    paragraph.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(comment) = current.as_mut()
                    && comment.in_t
                    && comment.skip_text_depth == 0
                    && let Some(paragraph) = comment.current_paragraph.as_mut()
                {
                    paragraph.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(comment) = current.as_mut() {
                    match name.as_str() {
                        "t" => comment.in_t = false,
                        "delText" | "instrText" => {
                            comment.skip_text_depth = comment.skip_text_depth.saturating_sub(1);
                        }
                        "p" => {
                            if let Some(paragraph) = comment.current_paragraph.take() {
                                comment.paragraphs.push(paragraph);
                            }
                        }
                        "comment" => {
                            if let Some(mut comment) = current.take() {
                                docx_finish_comment(&mut comment, &anchors);
                                comments.push(comment.info);
                            }
                        }
                        _ => {}
                    }
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(comments)
}

fn docx_comment_from_element(element: &BytesStart<'_>) -> DocxCommentBuild {
    let id_raw = attr(element, "id").unwrap_or_default();
    let (id, id_valid) = parse_docx_comment_id(&id_raw);
    DocxCommentBuild {
        info: DocxCommentInfo {
            id,
            id_raw,
            id_valid,
            author: attr(element, "author").unwrap_or_default(),
            date: attr(element, "date").unwrap_or_default(),
            initials: attr(element, "initials").unwrap_or_default(),
            ..DocxCommentInfo::default()
        },
        ..DocxCommentBuild::default()
    }
}

fn docx_note_comment_start(
    element: &BytesStart<'_>,
    name: &str,
    stack: &[String],
    comment: &mut DocxCommentBuild,
) {
    if name == "p" && stack.last().is_some_and(|parent| parent == "comment") {
        comment.current_paragraph = Some(String::new());
    }
    docx_note_comment_empty(element, name, stack, comment);
    if name == "t" {
        comment.in_t = true;
    }
    if name == "delText" || name == "instrText" {
        comment.skip_text_depth += 1;
    }
}

fn docx_note_comment_empty(
    _element: &BytesStart<'_>,
    name: &str,
    _stack: &[String],
    comment: &mut DocxCommentBuild,
) {
    let Some(paragraph) = comment.current_paragraph.as_mut() else {
        return;
    };
    match name {
        "tab" => paragraph.push('\t'),
        "br" | "cr" => paragraph.push('\n'),
        "noBreakHyphen" => paragraph.push('-'),
        _ => {}
    }
}

fn docx_finish_comment(
    comment: &mut DocxCommentBuild,
    anchors: &BTreeMap<String, DocxCommentAnchor>,
) {
    comment.info.text = comment.paragraphs.join("\n");
    if let Some(anchor) = anchors.get(&comment.info.id_raw) {
        comment.info.anchored_to_block = anchor.index;
        comment.info.anchored_to_block_kind = anchor.kind.clone();
    }
}

fn parse_docx_comment_id(value: &str) -> (i64, bool) {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return (0, false);
    }
    value
        .parse::<i64>()
        .map(|id| (id, true))
        .unwrap_or((0, false))
}

#[derive(Clone)]
struct DocxCommentAnchor {
    index: usize,
    kind: String,
    tag: String,
    depth: usize,
}

fn docx_comment_anchors(
    file: &str,
    document_part: &str,
) -> CliResult<BTreeMap<String, DocxCommentAnchor>> {
    let xml = zip_text(file, document_part.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut anchors = BTreeMap::<String, DocxCommentAnchor>::new();
    let mut current_block: Option<DocxCommentAnchor> = None;
    let mut block_index = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.last().is_some_and(|parent| parent == "body")
                    && matches!(name.as_str(), "p" | "tbl")
                {
                    block_index += 1;
                    current_block = Some(DocxCommentAnchor {
                        index: block_index,
                        kind: if name == "p" { "paragraph" } else { "table" }.to_string(),
                        tag: name.clone(),
                        depth: stack.len() + 1,
                    });
                }
                if name == "commentRangeStart" {
                    docx_note_comment_anchor(&mut anchors, current_block.as_ref(), &e);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.last().is_some_and(|parent| parent == "body")
                    && matches!(name.as_str(), "p" | "tbl")
                {
                    block_index += 1;
                }
                if name == "commentRangeStart" {
                    docx_note_comment_anchor(&mut anchors, current_block.as_ref(), &e);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current_block
                    .as_ref()
                    .is_some_and(|block| block.depth == stack.len() && block.tag == name)
                {
                    current_block = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(anchors)
}

fn docx_note_comment_anchor(
    anchors: &mut BTreeMap<String, DocxCommentAnchor>,
    current_block: Option<&DocxCommentAnchor>,
    element: &BytesStart<'_>,
) {
    let Some(block) = current_block else {
        return;
    };
    if let Some(id) = attr(element, "id") {
        anchors.entry(id).or_insert_with(|| block.clone());
    }
}

fn docx_comment_id_counts(comments: &[DocxCommentInfo]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for comment in comments {
        if !comment.id_raw.is_empty() {
            *counts.entry(comment.id_raw.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn docx_comment_json(comment: &DocxCommentInfo, counts: &BTreeMap<String, usize>) -> Value {
    let mut object = Map::new();
    object.insert("id".to_string(), json!(comment.id));
    object.insert("author".to_string(), json!(comment.author));
    if !comment.date.is_empty() {
        object.insert("date".to_string(), json!(comment.date));
    }
    if !comment.initials.is_empty() {
        object.insert("initials".to_string(), json!(comment.initials));
    }
    object.insert("text".to_string(), json!(comment.text));
    object.insert(
        "contentHash".to_string(),
        json!(docx_comment_content_hash(
            &comment.author,
            &comment.date,
            &comment.text
        )),
    );
    if comment.anchored_to_block > 0 {
        object.insert(
            "anchoredToBlock".to_string(),
            json!(comment.anchored_to_block),
        );
    }
    if !comment.anchored_to_block_kind.is_empty() {
        object.insert(
            "anchoredToBlockKind".to_string(),
            json!(comment.anchored_to_block_kind),
        );
    }
    if comment.id_valid {
        let selector = comment.id.to_string();
        object.insert("primarySelector".to_string(), json!(selector));
        object.insert("selectors".to_string(), json!([selector]));
        if counts.get(&comment.id_raw).copied().unwrap_or_default() == 1 {
            object.insert(
                "handle".to_string(),
                json!(format!("H:docx/pt:doc/comment:n:{}", comment.id)),
            );
        }
    }
    Value::Object(object)
}

fn docx_comment_content_hash(author: &str, date: &str, text: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(author.as_bytes());
    hash.update([0]);
    hash.update(date.as_bytes());
    hash.update([0]);
    hash.update(text.as_bytes());
    format!("sha256:{:x}", hash.finalize())
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

struct OpenXmlDeleteElement {
    name: String,
    start: usize,
    delete_self: bool,
    contains_target_comment_reference: bool,
}

fn remove_docx_comment_markers_xml(
    document_xml: &str,
    target_id: i64,
) -> CliResult<(String, bool)> {
    let body_tag = docx_body_tag(document_xml)?;
    let (content_start, content_end) = docx_body_content_bounds(document_xml, &body_tag)?;
    let body_xml = &document_xml[content_start..content_end];
    let target = target_id.to_string();
    let mut reader = Reader::from_str(body_xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<OpenXmlDeleteElement>::new();
    let mut ranges = Vec::<(usize, usize)>::new();

    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_target_marker =
                    matches!(name.as_str(), "commentRangeStart" | "commentRangeEnd")
                        && attr(&e, "id").is_some_and(|id| id == target);
                let is_target_reference =
                    name == "commentReference" && attr(&e, "id").is_some_and(|id| id == target);
                let reference_has_run_parent =
                    is_target_reference && mark_nearest_open_word_run(&mut stack);
                stack.push(OpenXmlDeleteElement {
                    name,
                    start: content_start + before,
                    delete_self: is_target_marker
                        || (is_target_reference && !reference_has_run_parent),
                    contains_target_comment_reference: false,
                });
            }
            Ok(Event::Empty(e)) => {
                let after = reader.buffer_position() as usize;
                let name = local_name(e.name().as_ref()).to_string();
                let is_target_marker =
                    matches!(name.as_str(), "commentRangeStart" | "commentRangeEnd")
                        && attr(&e, "id").is_some_and(|id| id == target);
                if is_target_marker {
                    ranges.push((content_start + before, content_start + after));
                    continue;
                }
                let is_target_reference =
                    name == "commentReference" && attr(&e, "id").is_some_and(|id| id == target);
                if is_target_reference && !mark_nearest_open_word_run(&mut stack) {
                    ranges.push((content_start + before, content_start + after));
                }
            }
            Ok(Event::End(_)) => {
                let after = reader.buffer_position() as usize;
                let Some(element) = stack.pop() else {
                    continue;
                };
                if element.delete_self || element.contains_target_comment_reference {
                    ranges.push((element.start, content_start + after));
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    if ranges.is_empty() {
        return Ok((document_xml.to_string(), false));
    }
    Ok((delete_xml_ranges(document_xml, ranges)?, true))
}

fn mark_nearest_open_word_run(stack: &mut [OpenXmlDeleteElement]) -> bool {
    if let Some(run) = stack.iter_mut().rev().find(|element| element.name == "r") {
        run.contains_target_comment_reference = true;
        true
    } else {
        false
    }
}

fn delete_xml_ranges(xml: &str, mut ranges: Vec<(usize, usize)>) -> CliResult<String> {
    ranges.retain(|(start, end)| start < end && *end <= xml.len());
    if ranges.is_empty() {
        return Ok(xml.to_string());
    }
    ranges.sort_by_key(|(start, end)| (*start, std::cmp::Reverse(*end)));
    let mut merged = Vec::<(usize, usize)>::new();
    for (start, end) in ranges {
        if let Some((_, current_end)) = merged.last_mut()
            && start <= *current_end
        {
            *current_end = (*current_end).max(end);
            continue;
        }
        merged.push((start, end));
    }
    let mut out = xml.to_string();
    for (start, end) in merged.into_iter().rev() {
        out.replace_range(start..end, "");
    }
    Ok(out)
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

fn docx_comment_info_from_fragment(fragment: &str) -> CliResult<(DocxCommentInfo, usize)> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut info = DocxCommentInfo::default();
    let mut paragraphs = Vec::<String>::new();
    let mut current_paragraph: Option<String> = None;
    let mut paragraph_count = 0usize;
    let mut in_t = false;
    let mut skip_text_depth = 0usize;
    let mut saw_comment = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_comment {
                    if name != "comment" {
                        return Err(CliError::unexpected("invalid comment XML"));
                    }
                    saw_comment = true;
                    info = docx_comment_from_element(&e).info;
                } else {
                    if name == "p" && stack.last().is_some_and(|parent| parent == "comment") {
                        current_paragraph = Some(String::new());
                        paragraph_count += 1;
                    }
                    if name == "br"
                        && let Some(paragraph) = current_paragraph.as_mut()
                    {
                        paragraph.push('\n');
                    }
                    if name == "tab"
                        && let Some(paragraph) = current_paragraph.as_mut()
                    {
                        paragraph.push('\t');
                    }
                    if name == "t" {
                        in_t = true;
                    }
                    if matches!(name.as_str(), "delText" | "instrText") {
                        skip_text_depth += 1;
                    }
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_comment {
                    if name != "comment" {
                        return Err(CliError::unexpected("invalid comment XML"));
                    }
                    saw_comment = true;
                    info = docx_comment_from_element(&e).info;
                } else if name == "p" && stack.last().is_some_and(|parent| parent == "comment") {
                    paragraphs.push(String::new());
                    paragraph_count += 1;
                } else if name == "br" {
                    if let Some(paragraph) = current_paragraph.as_mut() {
                        paragraph.push('\n');
                    }
                } else if name == "tab"
                    && let Some(paragraph) = current_paragraph.as_mut()
                {
                    paragraph.push('\t');
                }
            }
            Ok(Event::Text(e)) => {
                if in_t
                    && skip_text_depth == 0
                    && let Some(paragraph) = current_paragraph.as_mut()
                {
                    paragraph.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if in_t
                    && skip_text_depth == 0
                    && let Some(paragraph) = current_paragraph.as_mut()
                {
                    paragraph.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                match name.as_str() {
                    "t" => in_t = false,
                    "delText" | "instrText" => {
                        skip_text_depth = skip_text_depth.saturating_sub(1);
                    }
                    "p" => {
                        if let Some(paragraph) = current_paragraph.take() {
                            paragraphs.push(paragraph);
                        }
                    }
                    _ => {}
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !saw_comment {
        return Err(CliError::unexpected("invalid comment XML"));
    }
    info.text = paragraphs.join("\n");
    Ok((info, paragraph_count))
}

fn docx_comments_template() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"></w:comments>"#
        .to_string()
}

fn docx_next_comment_id(comments_xml: &str) -> i64 {
    let mut reader = Reader::from_str(comments_xml);
    let mut max_id = -1_i64;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "comment" =>
            {
                if let Some(id) = attr(&e, "id").and_then(|value| value.parse::<i64>().ok())
                    && id > max_id
                {
                    max_id = id;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    max_id + 1
}

fn append_docx_comment_xml(
    comments_xml: &str,
    comment_id: i64,
    author: &str,
    date: &str,
    initials: &str,
    text: &str,
) -> CliResult<String> {
    let root_tag = docx_comments_root_tag(comments_xml)?;
    let prefix = xml_tag_prefix(&root_tag);
    let comment = render_docx_comment(&prefix, comment_id, author, date, initials, text);
    let close_tag = format!("</{root_tag}>");
    if let Some(pos) = comments_xml.rfind(&close_tag) {
        let mut out = String::with_capacity(comments_xml.len() + comment.len());
        out.push_str(&comments_xml[..pos]);
        out.push_str(&comment);
        out.push_str(&comments_xml[pos..]);
        return Ok(out);
    }

    let start = comments_xml
        .find(&format!("<{root_tag}"))
        .ok_or_else(|| CliError::unexpected("comments part has no w:comments root"))?;
    let open_end = comments_xml[start..]
        .find('>')
        .map(|offset| start + offset)
        .ok_or_else(|| CliError::unexpected("comments part has no w:comments root"))?;
    let start_tag = &comments_xml[start..=open_end];
    if !start_tag.trim_end().ends_with("/>") {
        return Err(CliError::unexpected(
            "comments part has no closing w:comments tag",
        ));
    }
    let mut out = String::with_capacity(comments_xml.len() + comment.len() + close_tag.len());
    out.push_str(&comments_xml[..start]);
    out.push_str(&xml_open_tag_from_start(start_tag));
    out.push_str(&comment);
    out.push_str(&close_tag);
    out.push_str(&comments_xml[open_end + 1..]);
    Ok(out)
}

fn docx_comments_root_tag(comments_xml: &str) -> CliResult<String> {
    let mut reader = Reader::from_str(comments_xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if local_name(e.name().as_ref()) == "comments" {
                    return Ok(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
                return Err(CliError::unexpected("comments part has no w:comments root"));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::unexpected("comments part has no w:comments root"))
}

fn render_docx_comment(
    prefix: &str,
    comment_id: i64,
    author: &str,
    date: &str,
    initials: &str,
    text: &str,
) -> String {
    let comment = word_xml_tag(prefix, "comment");
    let p = word_xml_tag(prefix, "p");
    let r = word_xml_tag(prefix, "r");
    let mut out = String::new();
    out.push('<');
    out.push_str(&comment);
    out.push(' ');
    out.push_str(&word_attr_name(prefix, "id"));
    out.push_str("=\"");
    out.push_str(&comment_id.to_string());
    out.push_str("\" ");
    out.push_str(&word_attr_name(prefix, "author"));
    out.push_str("=\"");
    out.push_str(&xml_attr_escape(author));
    out.push('"');
    if !date.is_empty() {
        out.push(' ');
        out.push_str(&word_attr_name(prefix, "date"));
        out.push_str("=\"");
        out.push_str(&xml_attr_escape(date));
        out.push('"');
    }
    if !initials.is_empty() {
        out.push(' ');
        out.push_str(&word_attr_name(prefix, "initials"));
        out.push_str("=\"");
        out.push_str(&xml_attr_escape(initials));
        out.push('"');
    }
    out.push('>');
    out.push('<');
    out.push_str(&p);
    out.push('>');
    if !text.is_empty() {
        out.push('<');
        out.push_str(&r);
        out.push('>');
        append_docx_text_children(&mut out, prefix, text);
        out.push_str("</");
        out.push_str(&r);
        out.push('>');
    }
    out.push_str("</");
    out.push_str(&p);
    out.push('>');
    out.push_str("</");
    out.push_str(&comment);
    out.push('>');
    out
}

fn insert_docx_comment_markers_xml(
    document_xml: &str,
    anchor_index: usize,
    comment_id: i64,
) -> CliResult<String> {
    let body_tag = docx_body_tag(document_xml)?;
    let prefix = xml_tag_prefix(&body_tag);
    let working = if prefix.is_empty() {
        ensure_docx_word_prefix(document_xml)?
    } else {
        document_xml.to_string()
    };
    let body_tag = docx_body_tag(&working)?;
    let prefix = xml_tag_prefix(&body_tag);
    let blocks = docx_body_block_ranges(&working, &body_tag)?;
    let block = blocks.get(anchor_index - 1).ok_or_else(|| {
        CliError::invalid_args(format!("comment anchor block out of range: {anchor_index}"))
    })?;
    if block.kind != "p" {
        return Err(CliError::invalid_args(format!(
            "comment anchor block is not a paragraph: block {anchor_index} is table"
        )));
    }
    let fragment = &working[block.start..block.end];
    let updated = insert_docx_comment_markers_in_paragraph(fragment, &prefix, comment_id)?;
    let mut out = String::with_capacity(working.len() + updated.len());
    out.push_str(&working[..block.start]);
    out.push_str(&updated);
    out.push_str(&working[block.end..]);
    Ok(out)
}

fn insert_docx_comment_markers_in_paragraph(
    paragraph: &str,
    prefix: &str,
    comment_id: i64,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(paragraph)?;
    let start_tag = &paragraph[..=open_end];
    let open_tag = xml_open_tag_from_start(start_tag);
    let close_tag = format!("</{tag_name}>");
    let content_start = open_tag.len();
    let normalized = if self_closing {
        format!("{open_tag}{close_tag}")
    } else {
        paragraph.to_string()
    };
    let content_end = if self_closing {
        content_start
    } else {
        close_start
    };
    let children = xml_direct_child_ranges(&normalized, content_start, content_end)?;
    let start_marker = render_docx_comment_range_marker(prefix, "commentRangeStart", comment_id);
    let end_marker = render_docx_comment_range_marker(prefix, "commentRangeEnd", comment_id);
    let reference = render_docx_comment_reference_run(prefix, comment_id);
    let run_children: Vec<&XmlNamedRange> =
        children.iter().filter(|child| child.kind == "r").collect();
    if let (Some(first_run), Some(last_run)) = (run_children.first(), run_children.last()) {
        let mut out = String::with_capacity(
            normalized.len() + start_marker.len() + end_marker.len() + reference.len(),
        );
        out.push_str(&normalized[..first_run.start]);
        out.push_str(&start_marker);
        out.push_str(&normalized[first_run.start..last_run.end]);
        out.push_str(&end_marker);
        out.push_str(&reference);
        out.push_str(&normalized[last_run.end..]);
        return Ok(out);
    }

    let insert_at = children
        .iter()
        .find(|child| child.kind == "pPr")
        .map(|child| child.end)
        .unwrap_or(content_start);
    let mut out = String::with_capacity(
        normalized.len() + start_marker.len() + end_marker.len() + reference.len(),
    );
    out.push_str(&normalized[..insert_at]);
    out.push_str(&start_marker);
    out.push_str(&end_marker);
    out.push_str(&reference);
    out.push_str(&normalized[insert_at..]);
    Ok(out)
}

fn render_docx_comment_range_marker(prefix: &str, local: &str, comment_id: i64) -> String {
    let tag = word_xml_tag(prefix, local);
    format!(
        r#"<{tag} {}="{}"/>"#,
        word_attr_name(prefix, "id"),
        comment_id
    )
}

fn render_docx_comment_reference_run(prefix: &str, comment_id: i64) -> String {
    let r = word_xml_tag(prefix, "r");
    let reference = word_xml_tag(prefix, "commentReference");
    format!(
        r#"<{r}><{reference} {}="{}"/></{r}>"#,
        word_attr_name(prefix, "id"),
        comment_id
    )
}

fn word_attr_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        format!("w:{local}")
    } else {
        format!("{prefix}:{local}")
    }
}
