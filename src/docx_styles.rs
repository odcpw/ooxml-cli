use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, EXIT_INVALID_ARGS, EXIT_TARGET_NOT_FOUND,
    HANDLE_AMBIGUOUS, HANDLE_FORMAT_MISMATCH, HANDLE_MALFORMED, HANDLE_SCOPE_STALE, HANDLE_STALE,
    InspectPackageKind, attr, content_type_for_part, detect_inspect_package_type,
    docx_all_para_ids, docx_body_block_ranges, docx_body_tag, docx_handle_error,
    docx_open_tag_with_para_id, docx_rich_block_reports, ensure_docx_body_table_scaffolds_xml,
    ensure_docx_package_kind, ensure_docx_table_scaffold_fragment, ensure_docx_w14_namespace,
    ensure_docx_word_prefix, find_docx_document_part, is_docx_styles_part, local_name,
    mint_docx_para_id, relationship_entries, relationships_part_for,
    resolve_docx_paragraph_handle_index, resolve_relationship_target,
    validate_xlsx_mutation_output_flags, word_xml_tag, write_docx_mutation_output, xml_attr_escape,
    xml_direct_child_ranges, xml_fragment_bounds, xml_open_tag_from_start, xml_tag_prefix,
    zip_entry_names, zip_text,
};

pub(crate) fn docx_styles_list(file: &str, style_type: Option<&str>) -> CliResult<Value> {
    let style_type = normalize_docx_style_type(style_type)?;
    let (document_part, styles_part) = docx_document_and_styles_parts(file)?;
    let mut styles = Vec::new();
    if let Some(styles_part) = styles_part.as_deref() {
        styles = docx_styles(file, styles_part)?;
        if let Some(style_type) = style_type.as_deref() {
            styles.retain(|style| style.style_type == style_type);
        }
    }
    let counts = docx_style_id_counts(&styles);
    let styles_json: Vec<Value> = styles
        .iter()
        .map(|style| docx_style_json(style, &counts))
        .collect();
    Ok(json!({
        "file": file,
        "documentPartUri": document_part,
        "stylesPartUri": styles_part,
        "count": styles_json.len(),
        "styles": styles_json,
    }))
}

pub(crate) fn docx_styles_show(file: &str, style_id: &str) -> CliResult<Value> {
    let (document_part, styles_part) = docx_document_and_styles_parts(file)?;
    let mut style_json = Value::Null;
    let mut found = false;
    if let Some(styles_part) = styles_part.as_deref() {
        let styles = docx_styles(file, styles_part)?;
        let counts = docx_style_id_counts(&styles);
        if let Some(style) = styles.iter().find(|style| style.style_id == style_id) {
            style_json = docx_style_json(style, &counts);
            found = true;
        }
    }
    Ok(json!({
        "file": file,
        "documentPartUri": document_part,
        "stylesPartUri": styles_part,
        "styleId": style_id,
        "found": found,
        "style": style_json,
    }))
}

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

fn normalize_docx_style_type(value: Option<&str>) -> CliResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_lowercase();
    match normalized.as_str() {
        "paragraph" | "character" | "table" | "numbering" => Ok(Some(normalized)),
        _ => Err(CliError::invalid_args(
            "--type must be one of paragraph, character, table, numbering",
        )),
    }
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

fn docx_document_and_styles_parts(file: &str) -> CliResult<(String, Option<String>)> {
    let entries = zip_entry_names(file)?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        return Err(CliError::unsupported_type(
            "file is not a DOCX document (detected: unknown)",
        ));
    }
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let styles_uri = find_docx_styles_part(file, &entries, &document_part)?;
    Ok((document_uri, styles_uri))
}

fn find_docx_styles_part(
    file: &str,
    entries: &[String],
    document_part: &str,
) -> CliResult<Option<String>> {
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let rels_part = relationships_part_for(document_part);
    for rel in relationship_entries(file, &rels_part).unwrap_or_default() {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
            || rel.rel_type.ends_with("/styles")
        {
            return Ok(Some(resolve_relationship_target(
                &document_uri,
                &rel.target,
            )));
        }
    }
    for entry in entries {
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        let uri = format!("/{}", entry.trim_start_matches('/'));
        if is_docx_styles_part(&uri, &content_type) {
            return Ok(Some(uri));
        }
    }
    Ok(None)
}

#[derive(Clone, Default)]
struct DocxStyleInfo {
    style_id: String,
    name: String,
    style_type: String,
    default: bool,
    builtin: bool,
    based_on: String,
    next: String,
}

fn docx_styles(file: &str, styles_part: &str) -> CliResult<Vec<DocxStyleInfo>> {
    let xml = zip_text(file, styles_part.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut saw_root = false;
    let mut current: Option<DocxStyleInfo> = None;
    let mut styles = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "styles" {
                        return Err(CliError::unexpected(format!(
                            "styles part {styles_part} root is {name:?}, expected styles"
                        )));
                    }
                } else if name == "style" {
                    current = Some(docx_style_from_element(&e));
                } else {
                    docx_note_style_child(&e, &name, &mut current);
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "styles" {
                        return Err(CliError::unexpected(format!(
                            "styles part {styles_part} root is {name:?}, expected styles"
                        )));
                    }
                } else if name == "style" {
                    styles.push(docx_style_from_element(&e));
                } else {
                    docx_note_style_child(&e, &name, &mut current);
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "style" => {
                if let Some(style) = current.take() {
                    styles.push(style);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !saw_root {
        return Err(CliError::unexpected(format!(
            "styles part {styles_part} has no root element"
        )));
    }
    Ok(styles)
}

fn docx_style_from_element(element: &BytesStart<'_>) -> DocxStyleInfo {
    DocxStyleInfo {
        style_id: attr(element, "styleId").unwrap_or_default(),
        style_type: attr(element, "type").unwrap_or_default(),
        default: docx_on_off_attr(element, "default"),
        builtin: !docx_on_off_attr(element, "customStyle"),
        ..DocxStyleInfo::default()
    }
}

fn docx_note_style_child(
    element: &BytesStart<'_>,
    name: &str,
    current: &mut Option<DocxStyleInfo>,
) {
    let Some(style) = current.as_mut() else {
        return;
    };
    let Some(value) = attr(element, "val") else {
        return;
    };
    match name {
        "name" => style.name = value,
        "basedOn" => style.based_on = value,
        "next" => style.next = value,
        _ => {}
    }
}

fn docx_on_off_attr(element: &BytesStart<'_>, name: &str) -> bool {
    match attr(element, name).as_deref() {
        None => false,
        Some("0" | "false" | "off") => false,
        Some(_) => true,
    }
}

fn docx_style_id_counts(styles: &[DocxStyleInfo]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for style in styles {
        if !style.style_id.is_empty() {
            *counts.entry(style.style_id.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn docx_style_json(style: &DocxStyleInfo, counts: &BTreeMap<String, usize>) -> Value {
    let mut object = Map::new();
    object.insert("styleId".to_string(), json!(style.style_id));
    if !style.name.is_empty() {
        object.insert("name".to_string(), json!(style.name));
    }
    if !style.style_type.is_empty() {
        object.insert("type".to_string(), json!(style.style_type));
    }
    object.insert("default".to_string(), json!(style.default));
    object.insert("builtin".to_string(), json!(style.builtin));
    if !style.based_on.is_empty() {
        object.insert("basedOn".to_string(), json!(style.based_on));
    }
    if !style.next.is_empty() {
        object.insert("next".to_string(), json!(style.next));
    }
    if !style.style_id.is_empty() {
        object.insert("primarySelector".to_string(), json!(style.style_id));
        object.insert("selectors".to_string(), json!([style.style_id]));
        if counts.get(&style.style_id).copied().unwrap_or_default() == 1 {
            object.insert(
                "handle".to_string(),
                json!(format!("H:docx/pt:styles/style:n:{}", style.style_id)),
            );
        }
    }
    Value::Object(object)
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

fn apply_docx_style_xml(
    xml: &str,
    target: DocxStyleTarget,
    block_index: usize,
    style_id: &str,
    existing_para_id: &str,
) -> CliResult<String> {
    if block_index == 0 {
        return Err(CliError::target_not_found(format!(
            "target not found: {} block 0",
            target.as_str()
        )));
    }
    let mut working = xml.to_string();
    if matches!(target, DocxStyleTarget::Paragraph | DocxStyleTarget::Run)
        && existing_para_id.trim().is_empty()
    {
        working = ensure_docx_w14_namespace(&working)?;
    }
    let body_tag = docx_body_tag(&working)?;
    if !body_tag.contains(':') {
        working = ensure_docx_word_prefix(&working)?;
    }
    let body_tag = docx_body_tag(&working)?;
    let blocks = docx_body_block_ranges(&working, &body_tag)?;
    let block = blocks.get(block_index - 1).ok_or_else(|| {
        CliError::target_not_found(format!(
            "target not found: {} block {block_index}",
            target.as_str()
        ))
    })?;
    let fragment = &working[block.start..block.end];
    let replacement = match target {
        DocxStyleTarget::Paragraph => {
            if block.kind != "p" {
                return Err(CliError::invalid_args(format!(
                    "block {block_index} is a table, not a paragraph"
                )));
            }
            let para_id = docx_style_apply_para_id(&working, existing_para_id)?;
            set_docx_paragraph_style_fragment(fragment, &para_id, style_id)?
        }
        DocxStyleTarget::Run => {
            if block.kind != "p" {
                return Err(CliError::invalid_args(format!(
                    "block {block_index} is a table, not a paragraph"
                )));
            }
            let para_id = docx_style_apply_para_id(&working, existing_para_id)?;
            set_docx_run_style_for_paragraph_fragment(fragment, &para_id, style_id)?
        }
        DocxStyleTarget::Table => {
            if block.kind != "tbl" {
                return Err(CliError::invalid_args(format!(
                    "block {block_index} is a paragraph, not a table"
                )));
            }
            set_docx_table_style_fragment(fragment, style_id)?
        }
    };
    let mut out = String::with_capacity(working.len() + replacement.len());
    out.push_str(&working[..block.start]);
    out.push_str(&replacement);
    out.push_str(&working[block.end..]);
    if matches!(target, DocxStyleTarget::Paragraph | DocxStyleTarget::Run) {
        ensure_docx_body_table_scaffolds_xml(&out)
    } else {
        Ok(out)
    }
}

fn docx_style_apply_para_id(xml: &str, existing_para_id: &str) -> CliResult<String> {
    if !existing_para_id.trim().is_empty() {
        return Ok(existing_para_id.trim().to_string());
    }
    let existing = docx_all_para_ids(xml)?;
    Ok(mint_docx_para_id(&existing))
}

fn set_docx_paragraph_style_fragment(
    fragment: &str,
    para_id: &str,
    style_id: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let start_tag = &fragment[..=open_end];
    let prefix = xml_tag_prefix(&tag_name);
    let open_tag = docx_open_tag_with_para_id(start_tag, para_id);
    let props = render_docx_style_props(&prefix, "pPr", "pStyle", style_id);
    if self_closing {
        return Ok(format!("{open_tag}{props}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    if let Some(child) = children.into_iter().find(|child| child.kind == "pPr") {
        let updated_props =
            set_docx_style_child_in_props(&fragment[child.start..child.end], "pStyle", style_id)?;
        let mut out = String::new();
        out.push_str(&open_tag);
        out.push_str(&fragment[open_end + 1..child.start]);
        out.push_str(&updated_props);
        out.push_str(&fragment[child.end..close_start]);
        out.push_str("</");
        out.push_str(&tag_name);
        out.push('>');
        return Ok(out);
    }
    let mut out = String::new();
    out.push_str(&open_tag);
    out.push_str(&props);
    out.push_str(&fragment[open_end + 1..close_start]);
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok(out)
}

fn set_docx_run_style_for_paragraph_fragment(
    fragment: &str,
    para_id: &str,
    style_id: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let start_tag = &fragment[..=open_end];
    let open_tag = docx_open_tag_with_para_id(start_tag, para_id);
    if self_closing {
        return Ok(format!("{open_tag}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    let mut out = String::new();
    out.push_str(&open_tag);
    let mut cursor = open_end + 1;
    for child in children {
        if child.kind != "r" {
            continue;
        }
        out.push_str(&fragment[cursor..child.start]);
        out.push_str(&set_docx_run_style_fragment(
            &fragment[child.start..child.end],
            style_id,
        )?);
        cursor = child.end;
    }
    out.push_str(&fragment[cursor..close_start]);
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok(out)
}

fn set_docx_run_style_fragment(fragment: &str, style_id: &str) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let prefix = xml_tag_prefix(&tag_name);
    let props = render_docx_style_props(&prefix, "rPr", "rStyle", style_id);
    if self_closing {
        let open = xml_open_tag_from_start(&fragment[..=open_end]);
        return Ok(format!("{open}{props}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    if let Some(child) = children.into_iter().find(|child| child.kind == "rPr") {
        let updated_props =
            set_docx_style_child_in_props(&fragment[child.start..child.end], "rStyle", style_id)?;
        let mut out = String::new();
        out.push_str(&fragment[..child.start]);
        out.push_str(&updated_props);
        out.push_str(&fragment[child.end..]);
        return Ok(out);
    }
    let mut out = String::new();
    out.push_str(&fragment[..open_end + 1]);
    out.push_str(&props);
    out.push_str(&fragment[open_end + 1..]);
    Ok(out)
}

fn set_docx_table_style_fragment(fragment: &str, style_id: &str) -> CliResult<String> {
    let scaffolded = ensure_docx_table_scaffold_fragment(fragment)?;
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(&scaffolded)?;
    if self_closing {
        return Ok(scaffolded);
    }
    let children = xml_direct_child_ranges(&scaffolded, open_end + 1, close_start)?;
    let Some(child) = children.into_iter().find(|child| child.kind == "tblPr") else {
        return Ok(scaffolded);
    };
    let updated_props =
        set_docx_style_child_in_props(&scaffolded[child.start..child.end], "tblStyle", style_id)?;
    let mut out = String::new();
    out.push_str(&scaffolded[..child.start]);
    out.push_str(&updated_props);
    out.push_str(&scaffolded[child.end..]);
    Ok(out)
}

fn set_docx_style_child_in_props(
    props_fragment: &str,
    style_local: &str,
    style_id: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(props_fragment)?;
    let prefix = xml_tag_prefix(&tag_name);
    let style_child = render_docx_style_child(&prefix, style_local, style_id);
    if self_closing {
        let open = xml_open_tag_from_start(&props_fragment[..=open_end]);
        return Ok(format!("{open}{style_child}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(props_fragment, open_end + 1, close_start)?;
    if let Some(child) = children.into_iter().find(|child| child.kind == style_local) {
        let mut out = String::new();
        out.push_str(&props_fragment[..child.start]);
        out.push_str(&style_child);
        out.push_str(&props_fragment[child.end..]);
        return Ok(out);
    }
    let mut out = String::new();
    out.push_str(&props_fragment[..open_end + 1]);
    out.push_str(&style_child);
    out.push_str(&props_fragment[open_end + 1..]);
    Ok(out)
}

fn render_docx_style_props(
    prefix: &str,
    props_local: &str,
    style_local: &str,
    style_id: &str,
) -> String {
    let props = word_xml_tag(prefix, props_local);
    let mut out = String::new();
    out.push('<');
    out.push_str(&props);
    out.push('>');
    out.push_str(&render_docx_style_child(prefix, style_local, style_id));
    out.push_str("</");
    out.push_str(&props);
    out.push('>');
    out
}

fn render_docx_style_child(prefix: &str, style_local: &str, style_id: &str) -> String {
    let style_tag = word_xml_tag(prefix, style_local);
    let val_attr = if prefix.is_empty() {
        "w:val".to_string()
    } else {
        format!("{prefix}:val")
    };
    format!(
        "<{} {}=\"{}\"/>",
        style_tag,
        val_attr,
        xml_attr_escape(style_id)
    )
}

fn docx_first_run_style(fragment: &str) -> CliResult<String> {
    docx_style_in_fragment(fragment, "rPr", "rStyle")
}

fn docx_table_style(fragment: &str) -> CliResult<String> {
    docx_style_in_fragment(fragment, "tblPr", "tblStyle")
}

fn docx_style_in_fragment(
    fragment: &str,
    property_parent: &str,
    style_local: &str,
) -> CliResult<String> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<String> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                if parent == Some(property_parent)
                    && name == style_local
                    && let Some(style) = attr(&e, "val")
                {
                    return Ok(style);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                if parent == Some(property_parent)
                    && name == style_local
                    && let Some(style) = attr(&e, "val")
                {
                    return Ok(style);
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(String::new())
}
