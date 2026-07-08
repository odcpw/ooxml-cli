use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, DOCX_W_NS, DocxParagraphMutationOptions, InspectPackageKind,
    add_relationship_to_xml, allocate_relationship_id, attr, attr_exact, attr_prefixed_ns,
    content_type_for_part, detect_inspect_package_type, docx_body_block_ranges,
    docx_body_content_bounds, docx_body_prefix, docx_body_tag, docx_rich_block_reports,
    element_in_ns, ensure_content_type_override, ensure_docx_body_table_scaffolds_xml,
    ensure_docx_package_kind, find_docx_document_part, local_name, package_type,
    relationship_entries, relationship_target_from_source_to_target, relationships_part_for,
    replace_xml_span, resolve_relationship_target, validate_xlsx_mutation_output_flags,
    word_xml_tag, write_docx_package_binary_mutation_output, xml_attr_escape, xml_token_name,
    zip_entry_names, zip_text,
};
pub(crate) fn docx_images_list(file: &str) -> CliResult<Value> {
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
    let xml = zip_text(file, &document_part).map_err(|err| {
        CliError::unexpected(format!(
            "failed to extract DOCX images: failed to read document part {document_uri}: {}",
            err.message
        ))
    })?;
    let block_reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to extract DOCX images: {}", err.message))
    })?;
    let block_reports = block_reports
        .into_iter()
        .map(|report| (report.index, report))
        .collect::<BTreeMap<_, _>>();
    let rel_targets = docx_image_relationship_targets(file, &document_part, &document_uri);
    let refs = docx_image_refs_in_document_xml(&xml).map_err(|err| {
        CliError::unexpected(format!("failed to extract DOCX images: {}", err.message))
    })?;

    let mut images = Vec::new();
    for image_ref in refs {
        let Some(block) = block_reports.get(&image_ref.block_index) else {
            continue;
        };
        let index = images.len() + 1;
        let media_uri = rel_targets
            .get(&image_ref.blip_id)
            .cloned()
            .unwrap_or_default();
        let content_type = if media_uri.is_empty() {
            String::new()
        } else {
            content_type_for_part(file, &media_uri).unwrap_or_default()
        };
        let mut image = Map::new();
        image.insert("index".to_string(), json!(index));
        image.insert("id".to_string(), json!(image_ref.blip_id));
        image.insert("primarySelector".to_string(), json!(index.to_string()));
        image.insert("selectors".to_string(), json!([index.to_string()]));
        image.insert("blockIndex".to_string(), json!(image_ref.block_index));
        image.insert(
            "blockId".to_string(),
            json!(format!("body.b{}", image_ref.block_index)),
        );
        image.insert("blockHash".to_string(), json!(block.content_hash));
        image.insert("blipId".to_string(), json!(image_ref.blip_id));
        image.insert("mediaUri".to_string(), json!(media_uri));
        image.insert("contentType".to_string(), json!(content_type));
        image.insert("width".to_string(), json!(image_ref.width));
        image.insert("height".to_string(), json!(image_ref.height));
        images.push(Value::Object(image));
    }

    Ok(json!({
        "file": file,
        "documentPartUri": document_uri,
        "images": images,
    }))
}

const DOCX_IMAGE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const DOCX_REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DOCX_WP_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const DRAWINGML_MAIN_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const DRAWINGML_PIC_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";

pub(crate) fn docx_images_replace(
    file: &str,
    selector: &str,
    image_file: &str,
    expected_hash: &str,
    width: i64,
    height: i64,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let entries = zip_entry_names(file)?;
    ensure_docx_package_kind(file, &entries)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let document_xml = zip_text(file, &document_part).map_err(|err| {
        CliError::unexpected(format!(
            "failed to mutate image: failed to read document part {document_uri}: {}",
            err.message
        ))
    })?;
    let image_bytes = read_docx_image_file(image_file)?;
    let new_content_type = docx_image_content_type_from_path(image_file)?;
    validate_docx_image_payload(&image_bytes, &new_content_type)
        .map_err(|message| CliError::unexpected(format!("failed to mutate image: {message}")))?;

    let block_reports = docx_rich_block_reports(&document_xml, false)
        .map_err(|err| CliError::unexpected(format!("failed to mutate image: {}", err.message)))?;
    let block_reports = block_reports
        .into_iter()
        .map(|report| (report.index, report))
        .collect::<BTreeMap<_, _>>();
    let refs = docx_image_refs_in_document_xml(&document_xml)
        .map_err(|err| CliError::unexpected(format!("failed to mutate image: {}", err.message)))?;
    let Some(target) = select_docx_image_ref(&refs, selector) else {
        return Err(CliError::target_not_found("target not found: image"));
    };
    let Some(block) = block_reports.get(&target.block_index) else {
        return Err(CliError::target_not_found("target not found: image"));
    };
    check_docx_block_hash(target.block_index, expected_hash, &block.content_hash)?;

    let rels_part = relationships_part_for(&document_part);
    let rels_xml = zip_text(file, &rels_part).map_err(|err| {
        CliError::unexpected(format!(
            "failed to mutate image: failed to read relationships part /{rels_part}: {}",
            err.message
        ))
    })?;
    let rels = relationship_entries(file, &rels_part).unwrap_or_default();
    let Some(rel) = rels.iter().find(|rel| rel.id == target.blip_id) else {
        return Err(CliError::target_not_found("target not found: image"));
    };
    if rel.target_mode == "External" {
        return Err(CliError::target_not_found("target not found: image"));
    }
    let previous_uri = resolve_relationship_target(&document_uri, &rel.target);
    let previous_content_type = content_type_for_part(file, &previous_uri).unwrap_or_default();
    let new_uri = replacement_docx_image_uri(
        &entries,
        &previous_uri,
        &previous_content_type,
        &new_content_type,
    );
    let final_width = if width > 0 { width } else { target.width };
    let final_height = if height > 0 { height } else { target.height };
    let inline_xml = &document_xml[target.container_start..target.container_end];
    let updated_inline_xml =
        update_docx_image_extent_fragment(inline_xml, final_width, final_height);
    let mut updated_document_xml = replace_xml_span(
        &document_xml,
        target.container_start,
        target.container_end,
        &updated_inline_xml,
    );
    updated_document_xml = ensure_docx_body_table_scaffolds_xml(&updated_document_xml)
        .map_err(|err| CliError::unexpected(format!("failed to mutate image: {}", err.message)))?;

    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(document_part.clone(), updated_document_xml);
    let mut rels_output_xml = rels_xml;
    if new_uri != previous_uri {
        let new_target = relationship_target_from_source_to_target(&document_uri, &new_uri);
        rels_output_xml =
            rewrite_relationship_target_xml(&rels_output_xml, &target.blip_id, &new_target)?;
        text_overrides.insert(rels_part, rels_output_xml);
    }
    let content_types_xml = zip_text(file, "[Content_Types].xml").map_err(|err| {
        CliError::unexpected(format!(
            "failed to mutate image: failed to read [Content_Types].xml: {}",
            err.message
        ))
    })?;
    text_overrides.insert(
        "[Content_Types].xml".to_string(),
        ensure_content_type_override(content_types_xml, &new_uri, &new_content_type)?,
    );
    let mut binary_overrides = BTreeMap::new();
    binary_overrides.insert(package_part_name(&new_uri), image_bytes);
    write_docx_package_binary_mutation_output(file, &text_overrides, &binary_overrides, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(target.index));
    result.insert("id".to_string(), json!(target.blip_id.clone()));
    result.insert("blockIndex".to_string(), json!(target.block_index));
    result.insert(
        "blockId".to_string(),
        json!(format!("body.b{}", target.block_index)),
    );
    result.insert("blockHash".to_string(), json!(block.content_hash.clone()));
    result.insert("previousUri".to_string(), json!(previous_uri));
    result.insert(
        "previousContentType".to_string(),
        json!(previous_content_type),
    );
    result.insert("newUri".to_string(), json!(new_uri));
    result.insert("newContentType".to_string(), json!(new_content_type));
    result.insert("width".to_string(), json!(final_width));
    result.insert("height".to_string(), json!(final_height));
    Ok(Value::Object(result))
}

pub(crate) fn docx_images_insert(
    file: &str,
    after: usize,
    image_file: &str,
    expected_hash: &str,
    width: i64,
    height: i64,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let entries = zip_entry_names(file)?;
    ensure_docx_package_kind(file, &entries)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let image_bytes = read_docx_image_file(image_file)?;
    let new_content_type = docx_image_content_type_from_path(image_file)?;
    validate_docx_image_payload(&image_bytes, &new_content_type)
        .map_err(|message| CliError::unexpected(format!("failed to mutate image: {message}")))?;
    let document_xml = zip_text(file, &document_part).map_err(|err| {
        CliError::unexpected(format!(
            "failed to mutate image: failed to read document part {document_uri}: {}",
            err.message
        ))
    })?;
    let body_tag = docx_body_tag(&document_xml)?;
    let block_ranges = docx_body_block_ranges(&document_xml, &body_tag)?;
    if after > block_ranges.len() {
        return Err(CliError::target_not_found("target not found: block"));
    }
    let block_reports = docx_rich_block_reports(&document_xml, false)
        .map_err(|err| CliError::unexpected(format!("failed to mutate image: {}", err.message)))?;
    let block_reports = block_reports
        .into_iter()
        .map(|report| (report.index, report))
        .collect::<BTreeMap<_, _>>();
    let anchor_hash = if after == 0 {
        String::new()
    } else {
        let Some(block) = block_reports.get(&after) else {
            return Err(CliError::target_not_found("target not found: block"));
        };
        check_docx_block_hash(after, expected_hash, &block.content_hash)?;
        block.content_hash.clone()
    };

    let media_uri = allocate_docx_media_uri(&entries, &new_content_type);
    let rels_part = relationships_part_for(&document_part);
    let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| {
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"></Relationships>".to_string()
    });
    let rels = relationship_entries(file, &rels_part).unwrap_or_default();
    let rel_id = allocate_relationship_id(&rels);
    let rel_target = relationship_target_from_source_to_target(&document_uri, &media_uri);
    let updated_rels_xml =
        add_relationship_to_xml(rels_xml, &rel_id, DOCX_IMAGE_REL_TYPE, &rel_target);
    let doc_pr_id = next_docx_doc_pr_id(&document_xml);
    let prefix = docx_body_prefix(&body_tag);
    let paragraph_xml =
        render_docx_image_paragraph(&prefix, &rel_id, &media_uri, doc_pr_id, width, height);
    let mut updated_document_xml = insert_docx_image_paragraph_xml(
        &document_xml,
        &body_tag,
        &block_ranges,
        after,
        &paragraph_xml,
    )?;
    updated_document_xml = ensure_docx_image_root_namespaces(&updated_document_xml)?;
    updated_document_xml = ensure_docx_body_table_scaffolds_xml(&updated_document_xml)
        .map_err(|err| CliError::unexpected(format!("failed to mutate image: {}", err.message)))?;
    let content_types_xml = zip_text(file, "[Content_Types].xml").map_err(|err| {
        CliError::unexpected(format!(
            "failed to mutate image: failed to read [Content_Types].xml: {}",
            err.message
        ))
    })?;

    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(document_part, updated_document_xml);
    text_overrides.insert(rels_part, updated_rels_xml);
    text_overrides.insert(
        "[Content_Types].xml".to_string(),
        ensure_content_type_override(content_types_xml, &media_uri, &new_content_type)?,
    );
    let mut binary_overrides = BTreeMap::new();
    binary_overrides.insert(package_part_name(&media_uri), image_bytes);
    write_docx_package_binary_mutation_output(file, &text_overrides, &binary_overrides, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(after + 1));
    result.insert("id".to_string(), json!(rel_id));
    result.insert("insertAfter".to_string(), json!(after));
    if !anchor_hash.is_empty() {
        result.insert("anchorHash".to_string(), json!(anchor_hash));
    }
    result.insert("mediaUri".to_string(), json!(media_uri));
    result.insert("newContentType".to_string(), json!(new_content_type));
    result.insert("width".to_string(), json!(width));
    result.insert("height".to_string(), json!(height));
    Ok(Value::Object(result))
}

#[derive(Default)]
struct DocxImageRef {
    index: usize,
    block_index: usize,
    blip_id: String,
    width: i64,
    height: i64,
    container_start: usize,
    container_end: usize,
}

#[derive(Default)]
struct DocxDrawingScan {
    depth: usize,
    container_depth: Option<usize>,
    container_kind: String,
    container_start: usize,
    container_end: usize,
    blip_id: String,
    width: i64,
    height: i64,
    saw_extent: bool,
}

#[derive(Clone, Copy)]
struct DocxXmlEventSpan {
    start: usize,
    end: usize,
}

fn docx_image_relationship_targets(
    file: &str,
    document_part: &str,
    document_uri: &str,
) -> BTreeMap<String, String> {
    relationship_entries(file, &relationships_part_for(document_part))
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| rel.target_mode != "External")
        .filter(|rel| {
            rel.rel_type
                == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
                || rel.rel_type.ends_with("/image")
        })
        .map(|rel| {
            (
                rel.id,
                resolve_relationship_target(document_uri, &rel.target),
            )
        })
        .collect()
}

fn docx_image_refs_in_document_xml(xml: &str) -> CliResult<Vec<DocxImageRef>> {
    let mut reader = NsReader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut refs = Vec::new();
    let mut block_index = 0usize;
    let mut current_block = None::<DocxImageBlockKind>;
    let mut body_table_depth = 0usize;
    let mut drawing = None::<DocxDrawingScan>;

    loop {
        let event_start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let event_end = reader.buffer_position() as usize;
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if current_block.is_none() && parent == Some("body") && is_word && name == "p" {
                    block_index += 1;
                    current_block = Some(DocxImageBlockKind::Paragraph { index: block_index });
                } else if current_block.is_none()
                    && parent == Some("body")
                    && is_word
                    && name == "tbl"
                {
                    block_index += 1;
                    current_block = Some(DocxImageBlockKind::Table { index: block_index });
                    body_table_depth = 1;
                } else if matches!(current_block, Some(DocxImageBlockKind::Table { .. }))
                    && is_word
                    && name == "tbl"
                {
                    body_table_depth += 1;
                }

                let current_index = current_block.as_ref().map(DocxImageBlockKind::index);
                docx_image_note_start(
                    &mut drawing,
                    &e,
                    reader.resolver(),
                    is_word,
                    current_index,
                    DocxXmlEventSpan {
                        start: event_start,
                        end: event_end,
                    },
                );
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let event_end = reader.buffer_position() as usize;
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if current_block.is_none()
                    && parent == Some("body")
                    && is_word
                    && matches!(name.as_str(), "p" | "tbl")
                {
                    block_index += 1;
                } else {
                    let current_index = current_block.as_ref().map(DocxImageBlockKind::index);
                    docx_image_note_empty(
                        &mut drawing,
                        &e,
                        reader.resolver(),
                        is_word,
                        current_index,
                        DocxXmlEventSpan {
                            start: event_start,
                            end: event_end,
                        },
                        &mut refs,
                    );
                }
            }
            Ok(Event::End(e)) => {
                let event_end = reader.buffer_position() as usize;
                let name = local_name(e.name().as_ref()).to_string();
                let current_index = current_block.as_ref().map(DocxImageBlockKind::index);
                docx_image_note_end(&mut drawing, &name, current_index, event_end, &mut refs);

                match current_block {
                    Some(DocxImageBlockKind::Paragraph { .. }) if name == "p" => {
                        current_block = None;
                    }
                    Some(DocxImageBlockKind::Table { .. }) if name == "tbl" => {
                        body_table_depth = body_table_depth.saturating_sub(1);
                        if body_table_depth == 0 {
                            current_block = None;
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

    Ok(refs)
}

#[derive(Clone, Copy)]
enum DocxImageBlockKind {
    Paragraph { index: usize },
    Table { index: usize },
}

impl DocxImageBlockKind {
    fn index(&self) -> usize {
        match self {
            DocxImageBlockKind::Paragraph { index } | DocxImageBlockKind::Table { index } => *index,
        }
    }
}

fn docx_image_note_start(
    drawing: &mut Option<DocxDrawingScan>,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    is_word: bool,
    current_block: Option<usize>,
    event_span: DocxXmlEventSpan,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if drawing.is_none() {
        if current_block.is_some() && is_word && name == "drawing" {
            *drawing = Some(DocxDrawingScan {
                depth: 1,
                ..DocxDrawingScan::default()
            });
        }
        return;
    }

    let Some(scan) = drawing.as_mut() else {
        return;
    };
    let event_depth = scan.depth + 1;
    docx_drawing_scan_element(scan, element, resolver, name, event_depth, event_span);
    scan.depth += 1;
}

fn docx_image_note_empty(
    drawing: &mut Option<DocxDrawingScan>,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    is_word: bool,
    current_block: Option<usize>,
    event_span: DocxXmlEventSpan,
    refs: &mut Vec<DocxImageRef>,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if drawing.is_none() {
        if current_block.is_some() && is_word && name == "drawing" {
            // Empty w:drawing cannot contain an inline image.
        }
        return;
    }

    let Some(scan) = drawing.as_mut() else {
        return;
    };
    let event_depth = scan.depth + 1;
    docx_drawing_scan_element(scan, element, resolver, name, event_depth, event_span);
    if matches!(name, "inline" | "anchor") && scan.container_depth == Some(event_depth) {
        scan.container_depth = None;
        scan.container_end = event_span.end;
    }
    if name == "drawing" {
        docx_finish_drawing(drawing, current_block, refs);
    }
}

fn docx_image_note_end(
    drawing: &mut Option<DocxDrawingScan>,
    name: &str,
    current_block: Option<usize>,
    event_end: usize,
    refs: &mut Vec<DocxImageRef>,
) {
    let Some(scan) = drawing.as_mut() else {
        return;
    };
    if name == "drawing" && scan.depth == 1 {
        docx_finish_drawing(drawing, current_block, refs);
        return;
    }
    if scan.container_depth == Some(scan.depth) && name == scan.container_kind {
        scan.container_depth = None;
        scan.container_end = event_end;
    }
    scan.depth = scan.depth.saturating_sub(1);
}

fn docx_drawing_scan_element(
    scan: &mut DocxDrawingScan,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    name: &str,
    event_depth: usize,
    event_span: DocxXmlEventSpan,
) {
    match name {
        "inline" if scan.container_kind != "inline" => {
            scan.container_depth = Some(event_depth);
            scan.container_kind = "inline".to_string();
            scan.container_start = event_span.start;
            scan.container_end = event_span.end;
            scan.blip_id.clear();
            scan.width = 0;
            scan.height = 0;
            scan.saw_extent = false;
        }
        "anchor" if scan.container_kind.is_empty() => {
            scan.container_depth = Some(event_depth);
            scan.container_kind = "anchor".to_string();
            scan.container_start = event_span.start;
            scan.container_end = event_span.end;
        }
        "extent"
            if scan.container_depth == Some(event_depth.saturating_sub(1)) && !scan.saw_extent =>
        {
            scan.width = attr(element, "cx")
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or_default();
            scan.height = attr(element, "cy")
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or_default();
            scan.saw_extent = true;
        }
        "blip" if scan.container_depth.is_some() && scan.blip_id.is_empty() => {
            scan.blip_id = docx_blip_embed_id(element, resolver).unwrap_or_default();
        }
        _ => {}
    }
}

fn docx_finish_drawing(
    drawing: &mut Option<DocxDrawingScan>,
    current_block: Option<usize>,
    refs: &mut Vec<DocxImageRef>,
) {
    let Some(scan) = drawing.take() else {
        return;
    };
    if let Some(block_index) = current_block
        && !scan.blip_id.is_empty()
        && scan.container_end > scan.container_start
    {
        refs.push(DocxImageRef {
            index: refs.len() + 1,
            block_index,
            blip_id: scan.blip_id,
            width: scan.width,
            height: scan.height,
            container_start: scan.container_start,
            container_end: scan.container_end,
        });
    }
}

fn docx_blip_embed_id(element: &BytesStart<'_>, resolver: &NamespaceResolver) -> Option<String> {
    attr_exact(element, "embed")
        .or_else(|| {
            attr_prefixed_ns(
                element,
                resolver,
                b"r",
                b"http://schemas.openxmlformats.org/officeDocument/2006/relationships",
                b"embed",
            )
        })
        .or_else(|| attr_exact(element, "r:embed"))
}

fn read_docx_image_file(path: &str) -> CliResult<Vec<u8>> {
    fs::read(path).map_err(|_| CliError::file_not_found(format!("file not found: {path}")))
}

fn docx_image_content_type_from_path(path: &str) -> CliResult<String> {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "jpg" | "jpeg" => Ok("image/jpeg".to_string()),
        "png" => Ok("image/png".to_string()),
        "gif" => Ok("image/gif".to_string()),
        "bmp" => Ok("image/bmp".to_string()),
        "tif" | "tiff" => Ok("image/tiff".to_string()),
        "webp" => Ok("image/webp".to_string()),
        "svg" => Ok("image/svg+xml".to_string()),
        "emf" => Ok("image/x-emf".to_string()),
        "wmf" => Ok("image/x-wmf".to_string()),
        _ => Err(CliError::unsupported_type(format!(
            "unsupported image extension {path:?}"
        ))),
    }
}

fn validate_docx_image_payload(raw: &[u8], content_type: &str) -> Result<(), String> {
    let normalized = normalized_image_content_type(content_type);
    let ok = match normalized.as_str() {
        "image/png" => raw.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" | "image/jpg" | "image/pjpeg" => {
            raw.len() >= 3 && raw[0] == 0xff && raw[1] == 0xd8 && raw[2] == 0xff
        }
        "image/gif" => raw.starts_with(b"GIF87a") || raw.starts_with(b"GIF89a"),
        "image/bmp" => valid_docx_bmp_header(raw),
        "image/tiff" => valid_docx_tiff_header(raw),
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "image payload does not match content type {normalized}"
        ))
    }
}

fn valid_docx_bmp_header(raw: &[u8]) -> bool {
    if raw.len() < 26 || !raw.starts_with(b"BM") {
        return false;
    }
    let file_size = u32::from_le_bytes([raw[2], raw[3], raw[4], raw[5]]) as usize;
    let pixel_offset = u32::from_le_bytes([raw[10], raw[11], raw[12], raw[13]]) as usize;
    let dib_header_size = u32::from_le_bytes([raw[14], raw[15], raw[16], raw[17]]) as usize;
    let header_end = 14usize.saturating_add(dib_header_size);
    dib_header_size >= 12
        && header_end <= raw.len()
        && pixel_offset >= header_end
        && pixel_offset <= raw.len()
        && (file_size == 0 || file_size <= raw.len())
}

fn valid_docx_tiff_header(raw: &[u8]) -> bool {
    if raw.len() < 8 {
        return false;
    }
    let magic = match &raw[..2] {
        b"II" => u16::from_le_bytes([raw[2], raw[3]]),
        b"MM" => u16::from_be_bytes([raw[2], raw[3]]),
        _ => return false,
    };
    if magic != 42 && magic != 43 {
        return false;
    }
    let first_ifd = match &raw[..2] {
        b"II" => u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]),
        b"MM" => u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]),
        _ => 0,
    } as usize;
    first_ifd >= 8 && first_ifd < raw.len()
}

fn normalized_image_content_type(content_type: &str) -> String {
    content_type
        .split_once(';')
        .map(|(head, _)| head)
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase()
}

fn select_docx_image_ref<'a>(refs: &'a [DocxImageRef], selector: &str) -> Option<&'a DocxImageRef> {
    let selector = selector.trim();
    if let Ok(index) = selector.parse::<usize>() {
        return refs.iter().find(|image_ref| image_ref.index == index);
    }
    refs.iter().find(|image_ref| image_ref.blip_id == selector)
}

fn check_docx_block_hash(index: usize, expected: &str, actual: &str) -> CliResult<()> {
    if expected.is_empty() || expected == actual {
        return Ok(());
    }
    Err(CliError::invalid_args(format!(
        "block hash mismatch: block {index} expected {expected} but found {actual}"
    )))
}

fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}

fn replacement_docx_image_uri(
    entries: &[String],
    previous_uri: &str,
    previous_content_type: &str,
    new_content_type: &str,
) -> String {
    if normalized_image_content_type(previous_content_type)
        == normalized_image_content_type(new_content_type)
    {
        return previous_uri.to_string();
    }
    let new_extension = extension_for_docx_image_content_type(new_content_type);
    let Some((stem, old_extension)) = previous_uri.rsplit_once('.') else {
        return unique_docx_media_uri(entries, &format!("{previous_uri}{new_extension}"));
    };
    let old_extension = format!(".{old_extension}");
    if old_extension.eq_ignore_ascii_case(&new_extension) {
        previous_uri.to_string()
    } else {
        unique_docx_media_uri(entries, &format!("{stem}{new_extension}"))
    }
}

fn extension_for_docx_image_content_type(content_type: &str) -> String {
    match normalized_image_content_type(content_type).as_str() {
        "image/png" => ".png",
        "image/jpeg" | "image/jpg" | "image/pjpeg" => ".jpeg",
        "image/gif" => ".gif",
        "image/bmp" => ".bmp",
        "image/tiff" => ".tiff",
        "image/webp" => ".webp",
        "image/svg+xml" => ".svg",
        "image/x-emf" | "image/emf" => ".emf",
        "image/x-wmf" | "image/wmf" => ".wmf",
        _ => ".bin",
    }
    .to_string()
}

fn allocate_docx_media_uri(entries: &[String], content_type: &str) -> String {
    let extension = extension_for_docx_image_content_type(content_type);
    let used = entries
        .iter()
        .filter_map(|entry| {
            let entry = format!("/{}", entry.trim_start_matches('/'));
            let name = entry.strip_prefix("/word/media/image")?;
            let number = name
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            number.parse::<u32>().ok()
        })
        .collect::<BTreeSet<_>>();
    let mut next = 1u32;
    while used.contains(&next) {
        next += 1;
    }
    format!("/word/media/image{next}{extension}")
}

fn unique_docx_media_uri(entries: &[String], candidate: &str) -> String {
    let existing = entries
        .iter()
        .map(|entry| format!("/{}", entry.trim_start_matches('/')))
        .collect::<Vec<_>>();
    if !existing.iter().any(|entry| entry == candidate) {
        return candidate.to_string();
    }
    let extension = Path::new(candidate)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_default();
    let stem = candidate
        .strip_suffix(&extension)
        .unwrap_or(candidate)
        .to_string();
    for index in 1.. {
        let next = format!("{stem}_{index}{extension}");
        if !existing.iter().any(|entry| entry == &next) {
            return next;
        }
    }
    candidate.to_string()
}

fn update_docx_image_extent_fragment(fragment: &str, width: i64, height: i64) -> String {
    let mut out = String::with_capacity(fragment.len());
    let mut cursor = 0usize;
    while cursor < fragment.len() {
        let Some(relative_start) = fragment[cursor..].find('<') else {
            out.push_str(&fragment[cursor..]);
            break;
        };
        let tag_start = cursor + relative_start;
        let Some(relative_end) = fragment[tag_start..].find('>') else {
            out.push_str(&fragment[cursor..]);
            break;
        };
        let tag_end = tag_start + relative_end + 1;
        out.push_str(&fragment[cursor..tag_start]);
        let tag = &fragment[tag_start..tag_end];
        let token = &tag[1..tag.len().saturating_sub(1)];
        let name = xml_token_name(token).unwrap_or_default();
        if matches!(local_name(name.as_bytes()), "extent" | "ext")
            && !token.trim_start().starts_with('/')
        {
            let updated = replace_xml_tag_attr(tag, "cx", &width.to_string());
            out.push_str(&replace_xml_tag_attr(&updated, "cy", &height.to_string()));
        } else {
            out.push_str(tag);
        }
        cursor = tag_end;
    }
    out
}

fn replace_xml_tag_attr(tag: &str, name: &str, value: &str) -> String {
    if let Some((value_start, value_end)) = xml_tag_attr_value_span(tag, name) {
        let mut out = String::with_capacity(tag.len() + value.len());
        out.push_str(&tag[..value_start]);
        out.push_str(&xml_attr_escape(value));
        out.push_str(&tag[value_end..]);
        return out;
    }
    let insert_at = tag
        .rfind("/>")
        .or_else(|| tag.rfind('>'))
        .unwrap_or(tag.len());
    let mut out = String::with_capacity(tag.len() + name.len() + value.len() + 4);
    out.push_str(&tag[..insert_at]);
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    out.push_str(&xml_attr_escape(value));
    out.push('"');
    out.push_str(&tag[insert_at..]);
    out
}

fn xml_tag_attr_value(tag: &str, name: &str) -> Option<String> {
    xml_tag_attr_value_span(tag, name).map(|(start, end)| tag[start..end].to_string())
}

fn xml_tag_attr_value_span(tag: &str, name: &str) -> Option<(usize, usize)> {
    let bytes = tag.as_bytes();
    let mut cursor = 0usize;
    while let Some(relative) = tag[cursor..].find(name) {
        let start = cursor + relative;
        let before_ok = start > 0 && bytes[start - 1].is_ascii_whitespace();
        let after = start + name.len();
        if before_ok
            && after < bytes.len()
            && (bytes[after] == b'=' || bytes[after].is_ascii_whitespace())
        {
            let mut eq = after;
            while eq < bytes.len() && bytes[eq].is_ascii_whitespace() {
                eq += 1;
            }
            if eq < bytes.len() && bytes[eq] == b'=' {
                let mut quote = eq + 1;
                while quote < bytes.len() && bytes[quote].is_ascii_whitespace() {
                    quote += 1;
                }
                if quote < bytes.len() && matches!(bytes[quote], b'"' | b'\'') {
                    let quote_byte = bytes[quote];
                    let value_start = quote + 1;
                    let mut value_end = value_start;
                    while value_end < bytes.len() && bytes[value_end] != quote_byte {
                        value_end += 1;
                    }
                    if value_end < bytes.len() {
                        return Some((value_start, value_end));
                    }
                }
            }
        }
        cursor = after;
    }
    None
}

fn rewrite_relationship_target_xml(xml: &str, id: &str, target: &str) -> CliResult<String> {
    let mut out = String::with_capacity(xml.len());
    let mut cursor = 0usize;
    let mut rewritten = false;
    while cursor < xml.len() {
        let Some(relative_start) = xml[cursor..].find('<') else {
            out.push_str(&xml[cursor..]);
            break;
        };
        let tag_start = cursor + relative_start;
        let Some(relative_end) = xml[tag_start..].find('>') else {
            return Err(CliError::unexpected(
                "failed to mutate image: invalid relationships XML",
            ));
        };
        let tag_end = tag_start + relative_end + 1;
        out.push_str(&xml[cursor..tag_start]);
        let tag = &xml[tag_start..tag_end];
        let token = &tag[1..tag.len().saturating_sub(1)];
        let name = xml_token_name(token).unwrap_or_default();
        if local_name(name.as_bytes()) == "Relationship"
            && xml_tag_attr_value(tag, "Id").as_deref() == Some(id)
        {
            out.push_str(&replace_xml_tag_attr(tag, "Target", target));
            rewritten = true;
        } else {
            out.push_str(tag);
        }
        cursor = tag_end;
    }
    if rewritten {
        Ok(out)
    } else {
        Err(CliError::target_not_found("target not found: image"))
    }
}

fn next_docx_doc_pr_id(xml: &str) -> i64 {
    let mut reader = NsReader::from_str(xml);
    let mut max_id = 0i64;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "docPr" =>
            {
                if let Some(id) = attr(&e, "id").and_then(|value| value.parse::<i64>().ok()) {
                    max_id = max_id.max(id);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    max_id + 1
}

fn render_docx_image_paragraph(
    prefix: &str,
    rel_id: &str,
    media_uri: &str,
    doc_pr_id: i64,
    width: i64,
    height: i64,
) -> String {
    let p = word_xml_tag(prefix, "p");
    let r = word_xml_tag(prefix, "r");
    let drawing = word_xml_tag(prefix, "drawing");
    let picture_name = format!("Picture {doc_pr_id}");
    let escaped_picture_name = xml_attr_escape(&picture_name);
    let escaped_rel_id = xml_attr_escape(rel_id);
    let media_name = Path::new(media_uri)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image")
        .to_string();
    let escaped_media_name = xml_attr_escape(&media_name);
    format!(
        r#"<{p}><{r}><{drawing}><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{width}" cy="{height}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="{doc_pr_id}" name="{escaped_picture_name}"/><wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a="{DRAWINGML_MAIN_NS}" noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic xmlns:a="{DRAWINGML_MAIN_NS}"><a:graphicData uri="{DRAWINGML_PIC_NS}"><pic:pic xmlns:pic="{DRAWINGML_PIC_NS}"><pic:nvPicPr><pic:cNvPr id="0" name="{escaped_media_name}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{escaped_rel_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{width}" cy="{height}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></{drawing}></{r}></{p}>"#
    )
}

fn insert_docx_image_paragraph_xml(
    xml: &str,
    body_tag: &str,
    blocks: &[crate::XmlRange],
    after: usize,
    paragraph_xml: &str,
) -> CliResult<String> {
    let insert_at = if after == 0 {
        blocks
            .first()
            .map(|range| range.start)
            .unwrap_or(docx_body_content_bounds(xml, body_tag)?.0)
    } else {
        blocks
            .get(after - 1)
            .map(|range| range.end)
            .ok_or_else(|| CliError::target_not_found("target not found: block"))?
    };
    Ok(replace_xml_span(xml, insert_at, insert_at, paragraph_xml))
}

fn ensure_docx_image_root_namespaces(xml: &str) -> CliResult<String> {
    let xml = ensure_docx_root_namespace(xml, "wp", DOCX_WP_NS)?;
    ensure_docx_root_namespace(&xml, "r", DOCX_REL_NS)
}

fn ensure_docx_root_namespace(xml: &str, prefix: &str, uri: &str) -> CliResult<String> {
    if xml.contains(&format!("xmlns:{prefix}=")) {
        return Ok(xml.to_string());
    }
    let document_start = xml
        .find("<w:document")
        .or_else(|| xml.find("<document"))
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let start_end = xml[document_start..]
        .find('>')
        .map(|offset| document_start + offset)
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let attr = format!(" xmlns:{prefix}=\"{}\"", xml_attr_escape(uri));
    let mut out = String::with_capacity(xml.len() + attr.len());
    out.push_str(&xml[..start_end]);
    out.push_str(&attr);
    out.push_str(&xml[start_end..]);
    Ok(out)
}
