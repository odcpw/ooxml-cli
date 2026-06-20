use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, ResolveResult};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CliResult, attr_bound_ns, attr_exact, local_name, relationships_part_for,
    resolve_relationship_target, xml_attrs, zip_text,
};

use super::embedded_workbook::{
    REL_TYPE_PACKAGE, check_chart_external_data_embedded_workbook_open,
    is_embedded_spreadsheet_package_content_type,
};
use super::relationships::parse_relationship_part;
use super::spec::{
    CHART_NAMESPACE, CONTENT_TYPE_CHART, CONTENT_TYPE_DRAWING, CONTENT_TYPE_PPTX_SLIDE,
    CONTENT_TYPE_PPTX_SLIDE_LAYOUT, CONTENT_TYPE_PPTX_SLIDE_MASTER, CONTENT_TYPE_XLSX_PIVOT_CACHE,
    CONTENT_TYPE_XLSX_WORKSHEET, REL_TYPE_CHART, REL_TYPE_XLSX_DRAWING, REL_TYPE_XLSX_PIVOT_CACHE,
    REL_TYPE_XLSX_PIVOT_RECORDS, REL_TYPE_XLSX_PIVOT_TABLE, REL_TYPE_XLSX_TABLE,
    SPREADSHEET_DRAWING_NAMESPACE, SPREADSHEETML_NAMESPACE, is_xlsx_workbook_content_type,
};
use super::types::{PartInfo, RelationshipRecord};
use super::util::{diag, normalize_uri};

const REL_NS: &[u8] = b"http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DRAWINGML_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const P14_NS: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";

const REL_TYPE_XLSX_VML_DRAWING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/vmlDrawing";
const REL_TYPE_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const REL_TYPE_VIDEO: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video";
const REL_TYPE_AUDIO: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio";
const REL_TYPE_MEDIA: &str = "http://schemas.microsoft.com/office/2007/relationships/media";
#[derive(Clone, Default)]
struct DeepXmlNode {
    local_name: String,
    namespace: String,
    attrs: BTreeMap<String, String>,
    rel_attrs: BTreeMap<String, String>,
    depth: usize,
    parent: Option<String>,
}

#[derive(Default)]
struct DeepXmlPart {
    root: Option<DeepXmlNode>,
    nodes: Vec<DeepXmlNode>,
}

pub(super) fn check_part_deep_relationship_invariants(
    file: &str,
    part: &PartInfo,
    entry_set: &BTreeSet<String>,
    parts: &[PartInfo],
) -> CliResult<Vec<Value>> {
    if !is_deep_relationship_candidate(part) {
        return Ok(Vec::new());
    }

    let Ok(info) = read_deep_xml_part(file, part) else {
        return Ok(Vec::new());
    };
    if !root_matches_expected(part, &info) {
        return Ok(Vec::new());
    }

    let rels = relationships_for_part(file, &part.uri, entry_set);
    let rel_map = relationships_by_id(&rels);
    let content_types = content_types_by_uri(parts);
    let mut diagnostics = Vec::new();

    match part.content_type.as_str() {
        ct if is_xlsx_workbook_content_type(ct) => {
            diagnostics.extend(check_workbook_pivot_cache_references(
                &part.uri, &info, &rel_map,
            ));
        }
        CONTENT_TYPE_XLSX_WORKSHEET => {
            diagnostics.extend(check_worksheet_relationship_references(
                &part.uri, &info, &rel_map, &rels,
            ));
        }
        CONTENT_TYPE_XLSX_PIVOT_CACHE => {
            diagnostics.extend(check_pivot_cache_records_reference(
                &part.uri, &info, &rel_map,
            ));
        }
        CONTENT_TYPE_DRAWING => {
            diagnostics.extend(check_chart_relationship_references(
                &part.uri, &info, &rel_map,
            ));
            diagnostics.extend(check_drawing_media_relationship_references(
                &part.uri,
                &info,
                entry_set,
                &content_types,
                &rel_map,
            ));
        }
        CONTENT_TYPE_PPTX_SLIDE
        | CONTENT_TYPE_PPTX_SLIDE_LAYOUT
        | CONTENT_TYPE_PPTX_SLIDE_MASTER => {
            diagnostics.extend(check_chart_relationship_references(
                &part.uri, &info, &rel_map,
            ));
            diagnostics.extend(check_drawing_media_relationship_references(
                &part.uri,
                &info,
                entry_set,
                &content_types,
                &rel_map,
            ));
        }
        CONTENT_TYPE_CHART => {
            diagnostics.extend(check_chart_external_data_relationship_references(
                file,
                &part.uri,
                &info,
                entry_set,
                &content_types,
                &rel_map,
            ));
        }
        _ => {}
    }

    Ok(diagnostics)
}

fn is_deep_relationship_candidate(part: &PartInfo) -> bool {
    matches!(
        part.content_type.as_str(),
        CONTENT_TYPE_XLSX_WORKSHEET
            | CONTENT_TYPE_XLSX_PIVOT_CACHE
            | CONTENT_TYPE_DRAWING
            | CONTENT_TYPE_PPTX_SLIDE
            | CONTENT_TYPE_PPTX_SLIDE_LAYOUT
            | CONTENT_TYPE_PPTX_SLIDE_MASTER
            | CONTENT_TYPE_CHART
    ) || is_xlsx_workbook_content_type(&part.content_type)
}

fn read_deep_xml_part(file: &str, part: &PartInfo) -> CliResult<DeepXmlPart> {
    let xml = zip_text(file, &part.entry_name)?;
    let mut reader = NsReader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut info = DeepXmlPart::default();
    let mut stack = Vec::<String>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let node = deep_xml_node(&e, &reader, stack.len(), stack.last().cloned());
                if stack.is_empty() {
                    info.root = Some(node.clone());
                } else {
                    info.nodes.push(node.clone());
                }
                stack.push(node.local_name);
            }
            Ok(Event::Empty(e)) => {
                let node = deep_xml_node(&e, &reader, stack.len(), stack.last().cloned());
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

fn deep_xml_node(
    element: &BytesStart<'_>,
    reader: &NsReader<&[u8]>,
    depth: usize,
    parent: Option<String>,
) -> DeepXmlNode {
    DeepXmlNode {
        local_name: local_name(element.name().as_ref()).to_string(),
        namespace: element_namespace(element, reader),
        attrs: xml_attrs(element),
        rel_attrs: relationship_attrs(element, reader),
        depth,
        parent,
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
    ["id", "embed", "link"]
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

fn root_matches_expected(part: &PartInfo, info: &DeepXmlPart) -> bool {
    let Some(root) = &info.root else {
        return false;
    };
    let (local, namespace) = match part.content_type.as_str() {
        ct if is_xlsx_workbook_content_type(ct) => ("workbook", SPREADSHEETML_NAMESPACE),
        CONTENT_TYPE_XLSX_WORKSHEET => ("worksheet", SPREADSHEETML_NAMESPACE),
        CONTENT_TYPE_XLSX_PIVOT_CACHE => ("pivotCacheDefinition", SPREADSHEETML_NAMESPACE),
        CONTENT_TYPE_DRAWING => ("wsDr", SPREADSHEET_DRAWING_NAMESPACE),
        CONTENT_TYPE_PPTX_SLIDE => (
            "sld",
            "http://schemas.openxmlformats.org/presentationml/2006/main",
        ),
        CONTENT_TYPE_PPTX_SLIDE_LAYOUT => (
            "sldLayout",
            "http://schemas.openxmlformats.org/presentationml/2006/main",
        ),
        CONTENT_TYPE_PPTX_SLIDE_MASTER => (
            "sldMaster",
            "http://schemas.openxmlformats.org/presentationml/2006/main",
        ),
        CONTENT_TYPE_CHART => ("chartSpace", CHART_NAMESPACE),
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

fn check_workbook_pivot_cache_references(
    part_uri: &str,
    info: &DeepXmlPart,
    rel_map: &BTreeMap<String, RelationshipRecord>,
) -> Vec<Value> {
    let pivot_caches: Vec<&DeepXmlNode> = info
        .nodes
        .iter()
        .filter(|node| node.parent.as_deref() == Some("pivotCaches"))
        .filter(|node| node.local_name == "pivotCache" && node.namespace == SPREADSHEETML_NAMESPACE)
        .collect();
    let mut diagnostics = Vec::new();
    let mut seen_cache_ids = BTreeMap::<i64, String>::new();
    for (idx, node) in pivot_caches.iter().enumerate() {
        let label = workbook_pivot_cache_label(idx + 1, node);
        let raw_cache_id = attr_trim(node, "cacheId");
        if raw_cache_id.is_empty() {
            diagnostics.push(diag(
                "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE",
                format!("{part_uri} {label} is missing required cacheId"),
            ));
        } else if let Ok(cache_id) = raw_cache_id.parse::<i64>() {
            if cache_id <= 0 {
                diagnostics.push(diag(
                    "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE",
                    format!("{part_uri} {label} has invalid cacheId {raw_cache_id:?}"),
                ));
            } else if let Some(first) = seen_cache_ids.get(&cache_id) {
                diagnostics.push(diag(
                    "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE",
                    format!("{part_uri} {label} duplicates {first} cacheId {cache_id}"),
                ));
            } else {
                seen_cache_ids.insert(cache_id, label.clone());
            }
        } else {
            diagnostics.push(diag(
                "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE",
                format!("{part_uri} {label} has invalid cacheId {raw_cache_id:?}"),
            ));
        }
        diagnostics.extend(check_internal_relationship_reference(
            part_uri,
            &label,
            node,
            rel_map,
            REL_TYPE_XLSX_PIVOT_CACHE,
            "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE",
        ));
    }
    diagnostics
}

fn check_worksheet_relationship_references(
    part_uri: &str,
    info: &DeepXmlPart,
    rel_map: &BTreeMap<String, RelationshipRecord>,
    rels: &[RelationshipRecord],
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    for (idx, node) in worksheet_direct_nodes(info, "drawing").iter().enumerate() {
        let label = worksheet_reference_label("drawing", idx + 1, node);
        diagnostics.extend(check_internal_relationship_reference(
            part_uri,
            &label,
            node,
            rel_map,
            REL_TYPE_XLSX_DRAWING,
            "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE",
        ));
    }
    for (idx, node) in worksheet_direct_nodes(info, "legacyDrawing")
        .iter()
        .enumerate()
    {
        let label = worksheet_reference_label("legacyDrawing", idx + 1, node);
        diagnostics.extend(check_internal_relationship_reference(
            part_uri,
            &label,
            node,
            rel_map,
            REL_TYPE_XLSX_VML_DRAWING,
            "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE",
        ));
    }
    for (idx, node) in worksheet_direct_nodes(info, "legacyDrawingHF")
        .iter()
        .enumerate()
    {
        let label = worksheet_reference_label("legacyDrawingHF", idx + 1, node);
        diagnostics.extend(check_internal_relationship_reference(
            part_uri,
            &label,
            node,
            rel_map,
            REL_TYPE_XLSX_VML_DRAWING,
            "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE",
        ));
    }

    if let Some(table_parts) = worksheet_direct_nodes(info, "tableParts").first() {
        let table_part_nodes: Vec<&DeepXmlNode> = info
            .nodes
            .iter()
            .filter(|node| node.parent.as_deref() == Some("tableParts"))
            .filter(|node| {
                node.local_name == "tablePart" && node.namespace == SPREADSHEETML_NAMESPACE
            })
            .collect();
        diagnostics.extend(check_table_parts_count(
            part_uri,
            table_parts,
            table_part_nodes.len(),
        ));
        for (idx, node) in table_part_nodes.iter().enumerate() {
            let label = worksheet_reference_label("tablePart", idx + 1, node);
            diagnostics.extend(check_internal_relationship_reference(
                part_uri,
                &label,
                node,
                rel_map,
                REL_TYPE_XLSX_TABLE,
                "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE",
            ));
        }
    }

    for (idx, node) in worksheet_direct_nodes(info, "pivotTableDefinition")
        .iter()
        .enumerate()
    {
        let label = worksheet_reference_label("pivotTableDefinition", idx + 1, node);
        diagnostics.push(diag(
            "XLSX_WORKSHEET_PIVOT_REFERENCE",
            format!(
                "{part_uri} {label} is not a valid worksheet child; pivotTableDefinition must be the root of a pivot table part"
            ),
        ));
    }
    for rel in rels {
        if rel.rel_type != REL_TYPE_XLSX_PIVOT_TABLE {
            continue;
        }
        if is_external(&rel.target_mode) {
            diagnostics.push(diag(
                "XLSX_WORKSHEET_PIVOT_REFERENCE",
                format!(
                    "{part_uri} pivot table relationship {} points to an external target; worksheet pivot tables must resolve to internal pivot table parts",
                    relationship_id_or_placeholder(rel)
                ),
            ));
        }
    }
    diagnostics
}

fn worksheet_direct_nodes<'a>(info: &'a DeepXmlPart, name: &str) -> Vec<&'a DeepXmlNode> {
    info.nodes
        .iter()
        .filter(|node| node.depth == 1)
        .filter(|node| node.local_name == name && node.namespace == SPREADSHEETML_NAMESPACE)
        .collect()
}

fn check_table_parts_count(part_uri: &str, table_parts: &DeepXmlNode, actual: usize) -> Vec<Value> {
    let raw = attr_trim(table_parts, "count");
    if raw.is_empty() {
        return Vec::new();
    }
    match raw.parse::<usize>() {
        Ok(count) if count == actual => Vec::new(),
        Ok(count) => vec![diag(
            "XLSX_WORKSHEET_TABLEPARTS_COUNT",
            format!(
                "{part_uri} <tableParts> count is {count} but contains {actual} <tablePart> entries"
            ),
        )],
        Err(_) => vec![diag(
            "XLSX_WORKSHEET_TABLEPARTS_COUNT",
            format!("{part_uri} <tableParts> count {raw:?} is not a valid integer"),
        )],
    }
}

fn check_pivot_cache_records_reference(
    part_uri: &str,
    info: &DeepXmlPart,
    rel_map: &BTreeMap<String, RelationshipRecord>,
) -> Vec<Value> {
    let Some(root) = &info.root else {
        return Vec::new();
    };
    if !root.rel_attrs.contains_key("id") {
        return Vec::new();
    }
    check_internal_relationship_reference(
        part_uri,
        "<pivotCacheDefinition>",
        root,
        rel_map,
        REL_TYPE_XLSX_PIVOT_RECORDS,
        "XLSX_PIVOT_CACHE_RECORDS_REFERENCE",
    )
}

fn check_chart_relationship_references(
    part_uri: &str,
    info: &DeepXmlPart,
    rel_map: &BTreeMap<String, RelationshipRecord>,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    let charts: Vec<&DeepXmlNode> = info
        .nodes
        .iter()
        .filter(|node| node.local_name == "chart" && node.namespace == CHART_NAMESPACE)
        .collect();
    for (idx, node) in charts.iter().enumerate() {
        let label = chart_reference_label(idx + 1, node);
        diagnostics.extend(check_internal_relationship_reference(
            part_uri,
            &label,
            node,
            rel_map,
            REL_TYPE_CHART,
            "OOXML_CHART_RELATIONSHIP_REFERENCE",
        ));
    }
    diagnostics
}

fn check_chart_external_data_relationship_references(
    file: &str,
    part_uri: &str,
    info: &DeepXmlPart,
    entry_set: &BTreeSet<String>,
    content_types: &BTreeMap<String, String>,
    rel_map: &BTreeMap<String, RelationshipRecord>,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    let external_data: Vec<&DeepXmlNode> = info
        .nodes
        .iter()
        .filter(|node| node.depth == 1)
        .filter(|node| node.local_name == "externalData" && node.namespace == CHART_NAMESPACE)
        .collect();
    for (idx, node) in external_data.iter().enumerate() {
        let rid = rel_attr_trim(node, "id");
        let label = chart_external_data_label(idx + 1, node);
        diagnostics.extend(check_relationship_reference_target(
            ReferenceTargetCheck {
                part_uri,
                label: &label,
                rid: &rid,
                attr_name: "id",
                rel_map,
                expected_rel_type: REL_TYPE_PACKAGE,
                code: "OOXML_CHART_EXTERNAL_DATA_REFERENCE",
                allow_external: true,
                expected_content: "embedded spreadsheet package",
                content_type_ok: is_embedded_spreadsheet_package_content_type,
            },
            entry_set,
            content_types,
        ));
        diagnostics.extend(check_chart_external_data_embedded_workbook_open(
            file,
            part_uri,
            &label,
            &rid,
            entry_set,
            content_types,
            rel_map,
        ));
    }
    diagnostics
}

fn check_drawing_media_relationship_references(
    part_uri: &str,
    info: &DeepXmlPart,
    entry_set: &BTreeSet<String>,
    content_types: &BTreeMap<String, String>,
    rel_map: &BTreeMap<String, RelationshipRecord>,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    let blips: Vec<&DeepXmlNode> = info
        .nodes
        .iter()
        .filter(|node| node.local_name == "blip" && node.namespace == DRAWINGML_NS)
        .collect();
    for (idx, node) in blips.iter().enumerate() {
        if let Some(rid) = node.rel_attrs.get("embed") {
            let label = drawing_relationship_label("a:blip", idx + 1, "embed", rid);
            diagnostics.extend(check_relationship_reference_target(
                ReferenceTargetCheck {
                    part_uri,
                    label: &label,
                    rid,
                    attr_name: "embed",
                    rel_map,
                    expected_rel_type: REL_TYPE_IMAGE,
                    code: "OOXML_IMAGE_RELATIONSHIP_REFERENCE",
                    allow_external: false,
                    expected_content: "image/*",
                    content_type_ok: is_image_content_type,
                },
                entry_set,
                content_types,
            ));
        }
        if let Some(rid) = node.rel_attrs.get("link") {
            let label = drawing_relationship_label("a:blip", idx + 1, "link", rid);
            diagnostics.extend(check_relationship_reference_target(
                ReferenceTargetCheck {
                    part_uri,
                    label: &label,
                    rid,
                    attr_name: "link",
                    rel_map,
                    expected_rel_type: REL_TYPE_IMAGE,
                    code: "OOXML_IMAGE_RELATIONSHIP_REFERENCE",
                    allow_external: true,
                    expected_content: "image/*",
                    content_type_ok: is_image_content_type,
                },
                entry_set,
                content_types,
            ));
        }
    }

    let video_files: Vec<&DeepXmlNode> = info
        .nodes
        .iter()
        .filter(|node| node.local_name == "videoFile" && node.namespace == DRAWINGML_NS)
        .collect();
    for (idx, node) in video_files.iter().enumerate() {
        let rid = rel_attr_trim(node, "link");
        let label = drawing_relationship_label("a:videoFile", idx + 1, "link", &rid);
        diagnostics.extend(check_relationship_reference_target(
            ReferenceTargetCheck {
                part_uri,
                label: &label,
                rid: &rid,
                attr_name: "link",
                rel_map,
                expected_rel_type: REL_TYPE_VIDEO,
                code: "PPTX_MEDIA_RELATIONSHIP_REFERENCE",
                allow_external: true,
                expected_content: "video/*",
                content_type_ok: is_video_content_type,
            },
            entry_set,
            content_types,
        ));
    }

    let audio_files: Vec<&DeepXmlNode> = info
        .nodes
        .iter()
        .filter(|node| node.local_name == "audioFile" && node.namespace == DRAWINGML_NS)
        .collect();
    for (idx, node) in audio_files.iter().enumerate() {
        let rid = rel_attr_trim(node, "link");
        let label = drawing_relationship_label("a:audioFile", idx + 1, "link", &rid);
        diagnostics.extend(check_relationship_reference_target(
            ReferenceTargetCheck {
                part_uri,
                label: &label,
                rid: &rid,
                attr_name: "link",
                rel_map,
                expected_rel_type: REL_TYPE_AUDIO,
                code: "PPTX_MEDIA_RELATIONSHIP_REFERENCE",
                allow_external: true,
                expected_content: "audio/*",
                content_type_ok: is_audio_content_type,
            },
            entry_set,
            content_types,
        ));
    }

    let media_nodes: Vec<&DeepXmlNode> = info
        .nodes
        .iter()
        .filter(|node| node.local_name == "media" && node.namespace == P14_NS)
        .collect();
    for (idx, node) in media_nodes.iter().enumerate() {
        let rid = rel_attr_trim(node, "embed");
        let label = drawing_relationship_label("p14:media", idx + 1, "embed", &rid);
        diagnostics.extend(check_relationship_reference_target(
            ReferenceTargetCheck {
                part_uri,
                label: &label,
                rid: &rid,
                attr_name: "embed",
                rel_map,
                expected_rel_type: REL_TYPE_MEDIA,
                code: "PPTX_MEDIA_RELATIONSHIP_REFERENCE",
                allow_external: false,
                expected_content: "audio/* or video/*",
                content_type_ok: is_audio_video_content_type,
            },
            entry_set,
            content_types,
        ));
    }

    diagnostics
}

fn check_internal_relationship_reference(
    part_uri: &str,
    label: &str,
    node: &DeepXmlNode,
    rel_map: &BTreeMap<String, RelationshipRecord>,
    expected_rel_type: &str,
    code: &str,
) -> Vec<Value> {
    let rid = rel_attr_trim(node, "id");
    if rid.is_empty() {
        return vec![diag(
            code,
            format!("{part_uri} {label} is missing required r:id for its relationship"),
        )];
    }
    let Some(rel) = rel_map.get(&rid) else {
        return vec![diag(
            code,
            format!("{part_uri} {label} references missing relationship {rid}"),
        )];
    };
    if is_external(&rel.target_mode) {
        return vec![diag(
            code,
            format!(
                "{part_uri} {label} relationship {rid} points to an external target; expected an internal relationship of type {expected_rel_type:?}"
            ),
        )];
    }
    if rel.rel_type != expected_rel_type {
        return vec![diag(
            code,
            format!(
                "{part_uri} {label} relationship {rid} has type {:?}, expected {expected_rel_type:?}",
                rel.rel_type
            ),
        )];
    }
    Vec::new()
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

fn workbook_pivot_cache_label(position: usize, node: &DeepXmlNode) -> String {
    let cache_id = attr_trim(node, "cacheId");
    let rid = rel_attr_trim(node, "id");
    let mut attrs = Vec::new();
    if !cache_id.is_empty() {
        attrs.push(format!("cacheId={cache_id:?}"));
    }
    if !rid.is_empty() {
        attrs.push(format!("r:id={rid:?}"));
    }
    if attrs.is_empty() {
        format!("<pivotCache> at position {position}")
    } else {
        format!("<pivotCache {}> at position {position}", attrs.join(" "))
    }
}

fn worksheet_reference_label(item_name: &str, position: usize, node: &DeepXmlNode) -> String {
    let rid = rel_attr_trim(node, "id");
    if rid.is_empty() {
        format!("<{item_name}> at position {position}")
    } else {
        format!("<{item_name} r:id={rid:?}> at position {position}")
    }
}

fn chart_reference_label(position: usize, node: &DeepXmlNode) -> String {
    let rid = rel_attr_trim(node, "id");
    if rid.is_empty() {
        format!("<c:chart> at position {position}")
    } else {
        format!("<c:chart r:id={rid:?}> at position {position}")
    }
}

fn chart_external_data_label(position: usize, node: &DeepXmlNode) -> String {
    let rid = rel_attr_trim(node, "id");
    if rid.is_empty() {
        format!("<c:externalData> at position {position}")
    } else {
        format!("<c:externalData r:id={rid:?}> at position {position}")
    }
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

fn relationship_id_or_placeholder(rel: &RelationshipRecord) -> String {
    let id = rel.id.trim();
    if id.is_empty() {
        "<missing-id>".to_string()
    } else {
        id.to_string()
    }
}

fn attr_trim(node: &DeepXmlNode, name: &str) -> String {
    node.attrs
        .get(name)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn rel_attr_trim(node: &DeepXmlNode, name: &str) -> String {
    node.rel_attrs
        .get(name)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn is_external(target_mode: &str) -> bool {
    target_mode.trim().eq_ignore_ascii_case("External")
}

fn is_image_content_type(content_type: &str) -> bool {
    normalized_content_type(content_type).starts_with("image/")
}

fn is_video_content_type(content_type: &str) -> bool {
    normalized_content_type(content_type).starts_with("video/")
}

fn is_audio_content_type(content_type: &str) -> bool {
    normalized_content_type(content_type).starts_with("audio/")
}

fn is_audio_video_content_type(content_type: &str) -> bool {
    is_audio_content_type(content_type) || is_video_content_type(content_type)
}

fn normalized_content_type(content_type: &str) -> String {
    let content_type = content_type.trim().to_ascii_lowercase();
    content_type
        .split_once(';')
        .map(|(base, _)| base.trim().to_string())
        .unwrap_or(content_type)
}
