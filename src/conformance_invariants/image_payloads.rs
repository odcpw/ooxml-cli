use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, ResolveResult};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CliResult, attr_bound_ns, attr_exact, local_name, relationships_part_for,
    resolve_relationship_target, zip_bytes, zip_text,
};

use super::relationships::parse_relationship_part;
use super::spec::{
    CONTENT_TYPE_DOCX_DOCUMENT, CONTENT_TYPE_DOCX_FOOTER, CONTENT_TYPE_DOCX_HEADER,
    CONTENT_TYPE_DRAWING, CONTENT_TYPE_PPTX_SLIDE, CONTENT_TYPE_PPTX_SLIDE_LAYOUT,
    CONTENT_TYPE_PPTX_SLIDE_MASTER, PRESENTATIONML_NAMESPACE, SPREADSHEET_DRAWING_NAMESPACE,
    WORDPROCESSINGML_NAMESPACE,
};
use super::types::{PartInfo, RelationshipRecord};
use super::util::{diag, normalize_uri};

const REL_NS: &[u8] = b"http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DRAWINGML_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const REL_TYPE_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

#[derive(Clone, Default)]
struct ImageXmlNode {
    local_name: String,
    namespace: String,
    rel_attrs: BTreeMap<String, String>,
}

#[derive(Default)]
struct ImageXmlPart {
    root: Option<ImageXmlNode>,
    nodes: Vec<ImageXmlNode>,
}

pub(super) fn check_part_image_payload_invariants(
    file: &str,
    part: &PartInfo,
    entry_set: &BTreeSet<String>,
    parts: &[PartInfo],
) -> CliResult<Vec<Value>> {
    if !is_image_payload_candidate(part) {
        return Ok(Vec::new());
    }

    let Ok(info) = read_image_xml_part(file, part) else {
        return Ok(Vec::new());
    };
    if !root_matches_expected(part, &info) {
        return Ok(Vec::new());
    }

    let rels = relationships_for_part(file, &part.uri, entry_set);
    let rel_map = relationships_by_id(&rels);
    let content_types = content_types_by_uri(parts);
    let mut diagnostics = Vec::new();

    let check_reference_targets = is_docx_drawing_image_part(part);
    let blips: Vec<&ImageXmlNode> = info
        .nodes
        .iter()
        .filter(|node| node.local_name == "blip" && node.namespace == DRAWINGML_NS)
        .collect();
    for (idx, node) in blips.iter().enumerate() {
        if let Some(rid) = node.rel_attrs.get("embed") {
            let label = drawing_relationship_label("a:blip", idx + 1, "embed", rid);
            if check_reference_targets {
                diagnostics.extend(check_relationship_reference_target(
                    ReferenceTargetCheck {
                        part_uri: &part.uri,
                        label: &label,
                        rid,
                        attr_name: "embed",
                        rel_map: &rel_map,
                        expected_rel_type: REL_TYPE_IMAGE,
                        code: "OOXML_IMAGE_RELATIONSHIP_REFERENCE",
                        allow_external: false,
                        expected_content: "image/*",
                        content_type_ok: is_image_content_type,
                    },
                    entry_set,
                    &content_types,
                ));
            }
            diagnostics.extend(check_image_relationship_payload(
                file,
                &part.uri,
                &label,
                rid,
                &rel_map,
                entry_set,
                &content_types,
            ));
        }
        if let Some(rid) = node.rel_attrs.get("link") {
            let label = drawing_relationship_label("a:blip", idx + 1, "link", rid);
            if check_reference_targets {
                diagnostics.extend(check_relationship_reference_target(
                    ReferenceTargetCheck {
                        part_uri: &part.uri,
                        label: &label,
                        rid,
                        attr_name: "link",
                        rel_map: &rel_map,
                        expected_rel_type: REL_TYPE_IMAGE,
                        code: "OOXML_IMAGE_RELATIONSHIP_REFERENCE",
                        allow_external: true,
                        expected_content: "image/*",
                        content_type_ok: is_image_content_type,
                    },
                    entry_set,
                    &content_types,
                ));
            }
            diagnostics.extend(check_image_relationship_payload(
                file,
                &part.uri,
                &label,
                rid,
                &rel_map,
                entry_set,
                &content_types,
            ));
        }
    }

    Ok(diagnostics)
}

fn is_image_payload_candidate(part: &PartInfo) -> bool {
    matches!(
        part.content_type.as_str(),
        CONTENT_TYPE_DOCX_DOCUMENT
            | CONTENT_TYPE_DOCX_HEADER
            | CONTENT_TYPE_DOCX_FOOTER
            | CONTENT_TYPE_DRAWING
            | CONTENT_TYPE_PPTX_SLIDE
            | CONTENT_TYPE_PPTX_SLIDE_LAYOUT
            | CONTENT_TYPE_PPTX_SLIDE_MASTER
    )
}

fn is_docx_drawing_image_part(part: &PartInfo) -> bool {
    matches!(
        part.content_type.as_str(),
        CONTENT_TYPE_DOCX_DOCUMENT | CONTENT_TYPE_DOCX_HEADER | CONTENT_TYPE_DOCX_FOOTER
    )
}

fn read_image_xml_part(file: &str, part: &PartInfo) -> CliResult<ImageXmlPart> {
    let xml = zip_text(file, &part.entry_name)?;
    let mut reader = NsReader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut info = ImageXmlPart::default();
    let mut stack = Vec::<String>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let node = image_xml_node(&e, &reader);
                if stack.is_empty() {
                    info.root = Some(node.clone());
                } else {
                    info.nodes.push(node.clone());
                }
                stack.push(node.local_name);
            }
            Ok(Event::Empty(e)) => {
                let node = image_xml_node(&e, &reader);
                if stack.is_empty() {
                    info.root = Some(node);
                } else {
                    info.nodes.push(node);
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(crate::CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(info)
}

fn image_xml_node(element: &BytesStart<'_>, reader: &NsReader<&[u8]>) -> ImageXmlNode {
    ImageXmlNode {
        local_name: local_name(element.name().as_ref()).to_string(),
        namespace: element_namespace(element, reader),
        rel_attrs: relationship_attrs(element, reader),
    }
}

fn element_namespace(element: &BytesStart<'_>, reader: &NsReader<&[u8]>) -> String {
    match reader.resolver().resolve_element(element.name()) {
        (ResolveResult::Bound(Namespace(uri)), _) => String::from_utf8_lossy(uri).to_string(),
        _ => String::new(),
    }
}

fn relationship_attrs(
    element: &BytesStart<'_>,
    reader: &NsReader<&[u8]>,
) -> BTreeMap<String, String> {
    ["embed", "link"]
        .into_iter()
        .filter_map(|name| {
            relationship_attr(element, reader, name).map(|value| (name.to_string(), value))
        })
        .collect()
}

fn relationship_attr(
    element: &BytesStart<'_>,
    reader: &NsReader<&[u8]>,
    local: &str,
) -> Option<String> {
    attr_bound_ns(element, reader.resolver(), REL_NS, local.as_bytes())
        .or_else(|| attr_exact(element, &format!("r:{local}")))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn root_matches_expected(part: &PartInfo, info: &ImageXmlPart) -> bool {
    let Some(root) = &info.root else {
        return false;
    };
    let (local, namespace) = match part.content_type.as_str() {
        CONTENT_TYPE_DOCX_DOCUMENT => ("document", WORDPROCESSINGML_NAMESPACE),
        CONTENT_TYPE_DOCX_HEADER => ("hdr", WORDPROCESSINGML_NAMESPACE),
        CONTENT_TYPE_DOCX_FOOTER => ("ftr", WORDPROCESSINGML_NAMESPACE),
        CONTENT_TYPE_DRAWING => ("wsDr", SPREADSHEET_DRAWING_NAMESPACE),
        CONTENT_TYPE_PPTX_SLIDE => ("sld", PRESENTATIONML_NAMESPACE),
        CONTENT_TYPE_PPTX_SLIDE_LAYOUT => ("sldLayout", PRESENTATIONML_NAMESPACE),
        CONTENT_TYPE_PPTX_SLIDE_MASTER => ("sldMaster", PRESENTATIONML_NAMESPACE),
        _ => return false,
    };
    root.local_name == local && root.namespace == namespace
}

fn relationships_for_part(
    file: &str,
    source_uri: &str,
    entry_set: &BTreeSet<String>,
) -> Vec<RelationshipRecord> {
    let rels_entry = relationships_part_for(source_uri.trim_start_matches('/'));
    if !entry_set.contains(&normalize_uri(&rels_entry)) {
        return Vec::new();
    }
    parse_relationship_part(file, &rels_entry).unwrap_or_default()
}

fn relationships_by_id(rels: &[RelationshipRecord]) -> BTreeMap<String, RelationshipRecord> {
    rels.iter()
        .filter_map(|rel| {
            let id = rel.id.trim();
            (!id.is_empty()).then(|| (id.to_string(), rel.clone()))
        })
        .collect()
}

fn content_types_by_uri(parts: &[PartInfo]) -> BTreeMap<String, String> {
    parts
        .iter()
        .map(|part| (part.uri.clone(), part.content_type.clone()))
        .collect()
}

struct ReferenceTargetCheck<'a> {
    part_uri: &'a str,
    label: &'a str,
    rid: &'a str,
    attr_name: &'a str,
    rel_map: &'a BTreeMap<String, RelationshipRecord>,
    expected_rel_type: &'a str,
    code: &'a str,
    allow_external: bool,
    content_type_ok: fn(&str) -> bool,
    expected_content: &'a str,
}

fn check_relationship_reference_target(
    check: ReferenceTargetCheck<'_>,
    entry_set: &BTreeSet<String>,
    content_types: &BTreeMap<String, String>,
) -> Vec<Value> {
    if check.rid.is_empty() {
        return vec![diag(
            check.code,
            format!(
                "{} {} is missing required r:{} for its relationship",
                check.part_uri, check.label, check.attr_name
            ),
        )];
    }
    let Some(rel) = check.rel_map.get(check.rid) else {
        return vec![diag(
            check.code,
            format!(
                "{} {} references missing relationship {}",
                check.part_uri, check.label, check.rid
            ),
        )];
    };

    let mut diagnostics = Vec::new();
    if rel.rel_type != check.expected_rel_type {
        diagnostics.push(diag(
            check.code,
            format!(
                "{} {} relationship {} has type {:?}, expected {:?}",
                check.part_uri, check.label, check.rid, rel.rel_type, check.expected_rel_type
            ),
        ));
    }
    if is_external(&rel.target_mode) {
        if !check.allow_external {
            diagnostics.push(diag(
                check.code,
                format!(
                    "{} {} relationship {} points to an external target; expected an internal relationship of type {:?}",
                    check.part_uri, check.label, check.rid, check.expected_rel_type
                ),
            ));
        }
        return diagnostics;
    }

    let target_uri = normalize_uri(&resolve_relationship_target(check.part_uri, &rel.target));
    if !entry_set.contains(&target_uri) {
        return diagnostics;
    }
    let content_type = content_types
        .get(&target_uri)
        .map(|value| value.trim())
        .unwrap_or_default();
    if content_type.is_empty() {
        return diagnostics;
    }
    if !(check.content_type_ok)(content_type) {
        diagnostics.push(diag(
            check.code,
            format!(
                "{} {} relationship {} points to {} with content type {:?}, expected {}",
                check.part_uri,
                check.label,
                check.rid,
                target_uri,
                content_type,
                check.expected_content
            ),
        ));
    }
    diagnostics
}

fn check_image_relationship_payload(
    file: &str,
    part_uri: &str,
    label: &str,
    rid: &str,
    rel_map: &BTreeMap<String, RelationshipRecord>,
    entry_set: &BTreeSet<String>,
    content_types: &BTreeMap<String, String>,
) -> Vec<Value> {
    let Some(rel) = rel_map.get(rid) else {
        return Vec::new();
    };
    if rel.rel_type != REL_TYPE_IMAGE || is_external(&rel.target_mode) {
        return Vec::new();
    }
    let target_uri = normalize_uri(&resolve_relationship_target(part_uri, &rel.target));
    let content_type = content_types
        .get(&target_uri)
        .map(|value| value.as_str())
        .unwrap_or_default();
    if !entry_set.contains(&target_uri)
        || !is_image_content_type(content_type)
        || !has_known_signature(content_type)
    {
        return Vec::new();
    }
    let entry_name = target_uri.trim_start_matches('/');
    let raw = match zip_bytes(file, entry_name) {
        Ok(raw) => raw,
        Err(err) => {
            return vec![diag(
                "OOXML_IMAGE_PAYLOAD",
                format!(
                    "{part_uri} {label} relationship {rid} points to {target_uri} but image payload could not be read: {}",
                    err.message
                ),
            )];
        }
    };
    if !payload_matches_content_type(content_type, &raw) {
        return vec![diag(
            "OOXML_IMAGE_PAYLOAD",
            format!(
                "{part_uri} {label} relationship {rid} points to {target_uri} with content type {:?} but payload signature does not match",
                content_type.trim()
            ),
        )];
    }
    Vec::new()
}

fn drawing_relationship_label(
    element_name: &str,
    position: usize,
    attr_name: &str,
    rid: &str,
) -> String {
    if rid.is_empty() {
        format!("<{element_name}> at position {position}")
    } else {
        format!("<{element_name} r:{attr_name}={rid:?}> at position {position}")
    }
}

fn is_external(target_mode: &str) -> bool {
    target_mode.trim().eq_ignore_ascii_case("External")
}

fn is_image_content_type(content_type: &str) -> bool {
    normalized_content_type(content_type).starts_with("image/")
}

fn has_known_signature(content_type: &str) -> bool {
    matches!(
        normalized_content_type(content_type).as_str(),
        "image/png"
            | "image/jpeg"
            | "image/jpg"
            | "image/pjpeg"
            | "image/gif"
            | "image/bmp"
            | "image/tiff"
    )
}

fn payload_matches_content_type(content_type: &str, raw: &[u8]) -> bool {
    match normalized_content_type(content_type).as_str() {
        "image/png" => valid_png_header(raw),
        "image/jpeg" | "image/jpg" | "image/pjpeg" => valid_jpeg_header(raw),
        "image/gif" => valid_gif_header(raw),
        "image/bmp" => valid_bmp_header(raw),
        "image/tiff" => valid_tiff_header(raw),
        _ => true,
    }
}

fn valid_png_header(raw: &[u8]) -> bool {
    if raw.len() < 33 || !raw.starts_with(b"\x89PNG\r\n\x1a\n") {
        return false;
    }
    let ihdr_len = u32::from_be_bytes([raw[8], raw[9], raw[10], raw[11]]);
    if ihdr_len != 13 || &raw[12..16] != b"IHDR" {
        return false;
    }
    let width = u32::from_be_bytes([raw[16], raw[17], raw[18], raw[19]]);
    let height = u32::from_be_bytes([raw[20], raw[21], raw[22], raw[23]]);
    width > 0 && height > 0
}

fn valid_jpeg_header(raw: &[u8]) -> bool {
    if raw.len() < 4 || raw[0] != 0xff || raw[1] != 0xd8 {
        return false;
    }
    let mut pos = 2usize;
    while pos + 4 <= raw.len() {
        if raw[pos] != 0xff {
            return false;
        }
        while pos < raw.len() && raw[pos] == 0xff {
            pos += 1;
        }
        if pos >= raw.len() {
            return false;
        }
        let marker = raw[pos];
        pos += 1;
        if marker == 0xd9 || marker == 0xda {
            return false;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        if pos + 2 > raw.len() {
            return false;
        }
        let segment_len = u16::from_be_bytes([raw[pos], raw[pos + 1]]) as usize;
        if segment_len < 2 || pos + segment_len > raw.len() {
            return false;
        }
        if is_jpeg_start_of_frame(marker) {
            if segment_len < 7 {
                return false;
            }
            let height = u16::from_be_bytes([raw[pos + 3], raw[pos + 4]]);
            let width = u16::from_be_bytes([raw[pos + 5], raw[pos + 6]]);
            return width > 0 && height > 0;
        }
        pos += segment_len;
    }
    false
}

fn is_jpeg_start_of_frame(marker: u8) -> bool {
    matches!(
        marker,
        0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf
    )
}

fn valid_gif_header(raw: &[u8]) -> bool {
    if raw.len() < 10 || !(raw.starts_with(b"GIF87a") || raw.starts_with(b"GIF89a")) {
        return false;
    }
    let width = u16::from_le_bytes([raw[6], raw[7]]);
    let height = u16::from_le_bytes([raw[8], raw[9]]);
    width > 0 && height > 0
}

fn valid_bmp_header(raw: &[u8]) -> bool {
    if raw.len() < 26 || !raw.starts_with(b"BM") {
        return false;
    }
    let file_size = u32::from_le_bytes([raw[2], raw[3], raw[4], raw[5]]);
    let pixel_offset = u32::from_le_bytes([raw[10], raw[11], raw[12], raw[13]]);
    let dib_header_size = u32::from_le_bytes([raw[14], raw[15], raw[16], raw[17]]);
    let header_end = 14_i64 + i64::from(dib_header_size);
    if dib_header_size < 12 || header_end > raw.len() as i64 {
        return false;
    }
    let pixel_offset = i64::from(pixel_offset);
    if pixel_offset < header_end || pixel_offset > raw.len() as i64 {
        return false;
    }
    file_size == 0 || i64::from(file_size) <= raw.len() as i64
}

fn valid_tiff_header(raw: &[u8]) -> bool {
    if raw.len() < 8 {
        return false;
    }
    let little_endian = match &raw[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return false,
    };
    let magic = read_u16_endian(&raw[2..4], little_endian);
    if magic != 42 && magic != 43 {
        return false;
    }
    let first_ifd_offset = read_u32_endian(&raw[4..8], little_endian);
    first_ifd_offset >= 8 && (first_ifd_offset as usize) < raw.len()
}

fn read_u16_endian(raw: &[u8], little_endian: bool) -> u16 {
    if little_endian {
        u16::from_le_bytes([raw[0], raw[1]])
    } else {
        u16::from_be_bytes([raw[0], raw[1]])
    }
}

fn read_u32_endian(raw: &[u8], little_endian: bool) -> u32 {
    if little_endian {
        u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]])
    } else {
        u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]])
    }
}

fn normalized_content_type(content_type: &str) -> String {
    let content_type = content_type.trim().to_ascii_lowercase();
    content_type
        .split_once(';')
        .map(|(base, _)| base.trim().to_string())
        .unwrap_or(content_type)
}
