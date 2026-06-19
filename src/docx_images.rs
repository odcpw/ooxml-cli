use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, DOCX_W_NS, InspectPackageKind, attr, attr_exact, attr_prefixed_ns,
    content_type_for_part, detect_inspect_package_type, docx_rich_block_reports, element_in_ns,
    find_docx_document_part, local_name, package_type, relationship_entries,
    relationships_part_for, resolve_relationship_target, zip_entry_names, zip_text,
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

#[derive(Default)]
struct DocxImageRef {
    block_index: usize,
    blip_id: String,
    width: i64,
    height: i64,
}

#[derive(Default)]
struct DocxDrawingScan {
    depth: usize,
    container_depth: Option<usize>,
    container_kind: String,
    blip_id: String,
    width: i64,
    height: i64,
    saw_extent: bool,
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
        match reader.read_event() {
            Ok(Event::Start(e)) => {
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
                docx_image_note_start(&mut drawing, &e, reader.resolver(), is_word, current_index);
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
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
                        &mut refs,
                    );
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let current_index = current_block.as_ref().map(DocxImageBlockKind::index);
                docx_image_note_end(&mut drawing, &name, current_index, &mut refs);

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
    docx_drawing_scan_element(scan, element, resolver, name, event_depth);
    scan.depth += 1;
}

fn docx_image_note_empty(
    drawing: &mut Option<DocxDrawingScan>,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    is_word: bool,
    current_block: Option<usize>,
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
    docx_drawing_scan_element(scan, element, resolver, name, event_depth);
    if matches!(name, "inline" | "anchor") && scan.container_depth == Some(event_depth) {
        scan.container_depth = None;
    }
    if name == "drawing" {
        docx_finish_drawing(drawing, current_block, refs);
    }
}

fn docx_image_note_end(
    drawing: &mut Option<DocxDrawingScan>,
    name: &str,
    current_block: Option<usize>,
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
    }
    scan.depth = scan.depth.saturating_sub(1);
}

fn docx_drawing_scan_element(
    scan: &mut DocxDrawingScan,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    name: &str,
    event_depth: usize,
) {
    match name {
        "inline" if scan.container_kind != "inline" => {
            scan.container_depth = Some(event_depth);
            scan.container_kind = "inline".to_string();
            scan.blip_id.clear();
            scan.width = 0;
            scan.height = 0;
            scan.saw_extent = false;
        }
        "anchor" if scan.container_kind.is_empty() => {
            scan.container_depth = Some(event_depth);
            scan.container_kind = "anchor".to_string();
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
    {
        refs.push(DocxImageRef {
            block_index,
            blip_id: scan.blip_id,
            width: scan.width,
            height: scan.height,
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
