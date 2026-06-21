use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CliResult, RelationshipEntry, local_name, relationships_part_for, resolve_relationship_target,
    xml_attrs, xml_attrs_map, zip_text,
};

const REL_TYPE_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const REL_TYPE_VIDEO: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video";
const REL_TYPE_AUDIO: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio";
const REL_TYPE_MEDIA: &str = "http://schemas.microsoft.com/office/2007/relationships/media";

#[derive(Clone)]
struct SlideRef {
    part_uri: String,
    number: usize,
}

#[derive(Default)]
struct XmlPart {
    root: Option<usize>,
    nodes: Vec<XmlNode>,
}

#[derive(Default)]
struct XmlNode {
    local_name: String,
    attrs: BTreeMap<String, String>,
    raw_attrs: BTreeMap<String, String>,
    parent: Option<usize>,
}

pub(crate) fn validate_pptx_semantics(
    file: &str,
    entry_set: &BTreeSet<String>,
) -> CliResult<Vec<Value>> {
    let slides = presentation_slides(file)?;
    let mut diagnostics = Vec::new();
    let mut stale_media_diagnostics = Vec::new();

    diagnostics.extend(validate_presentation_child_order(file)?);

    for slide in slides {
        let rels = slide_relationships(file, &slide.part_uri);
        let rel_by_id = relationship_map(&rels);

        diagnostics.extend(validate_missing_media_relationship_targets(
            &slide, &rels, entry_set,
        ));

        let Some(xml) = read_xml_part(file, &slide.part_uri) else {
            continue;
        };
        diagnostics.extend(validate_slide_xml_relationship_references(
            &slide, &xml, &rel_by_id,
        ));
        stale_media_diagnostics.extend(validate_stale_media_references(
            &slide, &xml, &rel_by_id, entry_set,
        ));
    }

    diagnostics.extend(stale_media_diagnostics);
    Ok(diagnostics)
}

fn presentation_slides(file: &str) -> CliResult<Vec<SlideRef>> {
    let xml = zip_text(file, "ppt/presentation.xml")?;
    let presentation_rels =
        crate::relationship_entries(file, &relationships_part_for("ppt/presentation.xml"))
            .unwrap_or_default();
    let rel_by_id = relationship_map(&presentation_rels);
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                let attrs = xml_attrs_map(&e);
                let rid = attrs
                    .get("r:id")
                    .or_else(|| attrs.get("id"))
                    .cloned()
                    .unwrap_or_default();
                if let Some(rel) = rel_by_id.get(&rid) {
                    slides.push(SlideRef {
                        part_uri: resolve_relationship_target("/ppt/presentation.xml", &rel.target),
                        number: slides.len() + 1,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Ok(Vec::new()),
            _ => {}
        }
    }

    Ok(slides)
}

fn validate_presentation_child_order(file: &str) -> CliResult<Vec<Value>> {
    let xml = zip_text(file, "ppt/presentation.xml")?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    let mut diagnostics = Vec::new();
    let mut depth = 0_u32;
    let mut last_order = 0usize;
    let mut last_name = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if depth == 1 {
                    push_presentation_child_order_diagnostic(
                        &name,
                        &last_name,
                        &mut last_order,
                        &mut diagnostics,
                    );
                    last_name = name;
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if depth == 1 {
                    push_presentation_child_order_diagnostic(
                        &name,
                        &last_name,
                        &mut last_order,
                        &mut diagnostics,
                    );
                    last_name = name;
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                diagnostics.push(validation_diagnostic(
                    "PPTX_PARSE_ERROR",
                    "error",
                    format!("failed to parse presentation XML child order: {err}"),
                ));
                break;
            }
            _ => {}
        }
    }

    Ok(diagnostics)
}

fn push_presentation_child_order_diagnostic(
    name: &str,
    last_name: &str,
    last_order: &mut usize,
    diagnostics: &mut Vec<Value>,
) {
    let current = presentation_child_order(name);
    if current == 0 {
        return;
    }
    if *last_order > current {
        diagnostics.push(validation_diagnostic(
            "PPTX_PRESENTATION_CHILD_ORDER",
            "error",
            format!(
                "/ppt/presentation.xml has <{name}> after <{last_name}>; expected schema child order"
            ),
        ));
        return;
    }
    *last_order = current;
}

fn presentation_child_order(name: &str) -> usize {
    [
        "sldMasterIdLst",
        "notesMasterIdLst",
        "handoutMasterIdLst",
        "sldIdLst",
        "sldSz",
        "notesSz",
        "smartTags",
        "embeddedFontLst",
        "custShowLst",
        "photoAlbum",
        "custDataLst",
        "kinsoku",
        "defaultTextStyle",
        "modifyVerifier",
        "extLst",
    ]
    .iter()
    .position(|candidate| *candidate == name)
    .map(|idx| idx + 1)
    .unwrap_or(0)
}

fn slide_relationships(file: &str, slide_part_uri: &str) -> Vec<RelationshipEntry> {
    crate::relationship_entries(file, &relationships_part_for(slide_part_uri)).unwrap_or_default()
}

fn relationship_map(rels: &[RelationshipEntry]) -> BTreeMap<String, RelationshipEntry> {
    rels.iter()
        .cloned()
        .map(|rel| (rel.id.clone(), rel))
        .collect()
}

fn validate_missing_media_relationship_targets(
    slide: &SlideRef,
    rels: &[RelationshipEntry],
    entry_set: &BTreeSet<String>,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    for rel in rels {
        if rel.target_mode == "External" || !is_media_relationship_type(&rel.rel_type) {
            continue;
        }
        let target_uri = resolve_relationship_target(&slide.part_uri, &rel.target);
        if !entry_set.contains(&target_uri) {
            diagnostics.push(validation_diagnostic(
                "PPTX_MISSING_MEDIA",
                "warning",
                format!(
                    "slide {} references missing media: {}",
                    slide.number, target_uri
                ),
            ));
        }
    }
    diagnostics
}

fn validate_slide_xml_relationship_references(
    slide: &SlideRef,
    xml: &XmlPart,
    rel_by_id: &BTreeMap<String, RelationshipEntry>,
) -> Vec<Value> {
    let Some(root_id) = xml.root else {
        return Vec::new();
    };
    let mut diagnostics = Vec::new();

    for node_id in descendants(xml, root_id, "blip") {
        let node = &xml.nodes[node_id];
        if let Some(rid) = rel_attr(node, "embed") {
            diagnostics.extend(validate_slide_xml_relationship_reference(
                slide,
                rid,
                rel_by_id,
                "image embed",
            ));
        }
        if let Some(rid) = rel_attr(node, "link") {
            diagnostics.extend(validate_slide_xml_relationship_reference(
                slide,
                rid,
                rel_by_id,
                "linked image",
            ));
        }
    }

    for name in ["hlinkClick", "hlinkMouseOver"] {
        for node_id in descendants(xml, root_id, name) {
            if let Some(rid) = rel_attr(&xml.nodes[node_id], "id") {
                diagnostics.extend(validate_slide_xml_relationship_reference(
                    slide,
                    rid,
                    rel_by_id,
                    "hyperlink",
                ));
            }
        }
    }

    for node_id in descendants(xml, root_id, "media") {
        if let Some(rid) = rel_attr(&xml.nodes[node_id], "embed") {
            diagnostics.extend(validate_slide_xml_relationship_reference(
                slide,
                rid,
                rel_by_id,
                "embedded media",
            ));
        }
    }

    for node_id in descendants(xml, root_id, "videoFile") {
        if let Some(rid) = rel_attr(&xml.nodes[node_id], "link") {
            diagnostics.extend(validate_slide_xml_relationship_reference(
                slide,
                rid,
                rel_by_id,
                "video media",
            ));
        }
    }

    for node_id in descendants(xml, root_id, "audioFile") {
        if let Some(rid) = rel_attr(&xml.nodes[node_id], "link") {
            diagnostics.extend(validate_slide_xml_relationship_reference(
                slide,
                rid,
                rel_by_id,
                "audio media",
            ));
        }
    }

    diagnostics
}

fn validate_slide_xml_relationship_reference(
    slide: &SlideRef,
    rid: &str,
    rel_by_id: &BTreeMap<String, RelationshipEntry>,
    description: &str,
) -> Vec<Value> {
    if rel_by_id.contains_key(rid) {
        return Vec::new();
    }
    vec![validation_diagnostic(
        "PPTX_MISSING_SLIDE_RELATIONSHIP",
        "error",
        format!(
            "slide {} {} references missing relationship {}",
            slide.number, description, rid
        ),
    )]
}

fn validate_stale_media_references(
    slide: &SlideRef,
    xml: &XmlPart,
    rel_by_id: &BTreeMap<String, RelationshipEntry>,
    entry_set: &BTreeSet<String>,
) -> Vec<Value> {
    let Some(root_id) = xml.root else {
        return Vec::new();
    };
    let mut diagnostics = Vec::new();
    for pic_id in descendants(xml, root_id, "pic") {
        let Some(stale) = media_stale_reason(slide, xml, pic_id, rel_by_id, entry_set) else {
            continue;
        };
        diagnostics.push(validation_diagnostic(
            "PPTX_STALE_MEDIA_REFERENCE",
            "warning",
            format!(
                "slide {} media shape {} has stale media reference: {}",
                slide.number, stale.spid, stale.reason
            ),
        ));
    }
    diagnostics
}

struct StaleMedia {
    spid: i64,
    reason: String,
}

fn media_stale_reason(
    slide: &SlideRef,
    xml: &XmlPart,
    pic_id: usize,
    rel_by_id: &BTreeMap<String, RelationshipEntry>,
    entry_set: &BTreeSet<String>,
) -> Option<StaleMedia> {
    let nv_pic_pr = child(xml, pic_id, "nvPicPr")?;
    let nv_pr = child(xml, nv_pic_pr, "nvPr")?;
    let video_file = child(xml, nv_pr, "videoFile");
    let audio_file = child(xml, nv_pr, "audioFile");
    let p14_media = p14_media_child(xml, nv_pr);
    if video_file.is_none() && audio_file.is_none() && p14_media.is_none() {
        return None;
    }

    let spid = child(xml, nv_pic_pr, "cNvPr")
        .and_then(|id| attr_trim(&xml.nodes[id], "id").parse::<i64>().ok())
        .unwrap_or_default();

    let media_rid = p14_media
        .and_then(|id| rel_attr(&xml.nodes[id], "embed").map(str::to_string))
        .or_else(|| video_file.and_then(|id| rel_attr(&xml.nodes[id], "link").map(str::to_string)))
        .or_else(|| audio_file.and_then(|id| rel_attr(&xml.nodes[id], "link").map(str::to_string)));
    let mut reason = media_rid
        .as_deref()
        .and_then(|rid| stale_reason(slide, rid, rel_by_id, entry_set));

    if reason.is_none()
        && let Some(blip) = child(xml, pic_id, "blipFill").and_then(|id| child(xml, id, "blip"))
        && let Some(poster_rid) = rel_attr(&xml.nodes[blip], "embed")
    {
        reason = stale_reason(slide, poster_rid, rel_by_id, entry_set);
    }

    reason.map(|reason| StaleMedia { spid, reason })
}

fn stale_reason(
    slide: &SlideRef,
    rid: &str,
    rel_by_id: &BTreeMap<String, RelationshipEntry>,
    entry_set: &BTreeSet<String>,
) -> Option<String> {
    let Some(rel) = rel_by_id.get(rid) else {
        return Some(format!("dangling-rel:{rid}"));
    };
    if rel.target_mode == "External" {
        return None;
    }
    let target_uri = resolve_relationship_target(&slide.part_uri, &rel.target);
    if !entry_set.contains(&target_uri) {
        return Some(format!("missing-part:{target_uri}"));
    }
    None
}

fn read_xml_part(file: &str, part_uri: &str) -> Option<XmlPart> {
    let xml = zip_text(file, part_uri.trim_start_matches('/')).ok()?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut part = XmlPart::default();
    let mut stack = Vec::<usize>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let id = push_node(&mut part, &e, stack.last().copied());
                stack.push(id);
            }
            Ok(Event::Empty(e)) => {
                push_node(&mut part, &e, stack.last().copied());
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    Some(part)
}

fn push_node(part: &mut XmlPart, element: &BytesStart<'_>, parent: Option<usize>) -> usize {
    let id = part.nodes.len();
    if parent.is_none() && part.root.is_none() {
        part.root = Some(id);
    }
    part.nodes.push(XmlNode {
        local_name: local_name(element.name().as_ref()).to_string(),
        attrs: xml_attrs(element),
        raw_attrs: xml_attrs_map(element),
        parent,
    });
    id
}

fn child(xml: &XmlPart, parent_id: usize, name: &str) -> Option<usize> {
    xml.nodes.iter().enumerate().find_map(|(id, node)| {
        (node.parent == Some(parent_id) && node.local_name == name).then_some(id)
    })
}

fn p14_media_child(xml: &XmlPart, nv_pr_id: usize) -> Option<usize> {
    let ext_lst = child(xml, nv_pr_id, "extLst")?;
    children(xml, ext_lst, "ext")
        .into_iter()
        .find_map(|ext_id| child(xml, ext_id, "media"))
}

fn children(xml: &XmlPart, parent_id: usize, name: &str) -> Vec<usize> {
    xml.nodes
        .iter()
        .enumerate()
        .filter_map(|(id, node)| {
            (node.parent == Some(parent_id) && node.local_name == name).then_some(id)
        })
        .collect()
}

fn descendants(xml: &XmlPart, parent_id: usize, name: &str) -> Vec<usize> {
    xml.nodes
        .iter()
        .enumerate()
        .filter_map(|(id, node)| {
            (node.local_name == name && is_descendant_of(xml, id, parent_id)).then_some(id)
        })
        .collect()
}

fn is_descendant_of(xml: &XmlPart, node_id: usize, ancestor_id: usize) -> bool {
    let mut parent = xml.nodes[node_id].parent;
    while let Some(parent_id) = parent {
        if parent_id == ancestor_id {
            return true;
        }
        parent = xml.nodes[parent_id].parent;
    }
    false
}

fn rel_attr<'a>(node: &'a XmlNode, local: &str) -> Option<&'a str> {
    node.raw_attrs
        .get(&format!("r:{local}"))
        .or_else(|| node.attrs.get(local))
        .map(String::as_str)
        .filter(|value| !value.is_empty())
}

fn attr_trim(node: &XmlNode, local: &str) -> String {
    node.attrs
        .get(local)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn is_media_relationship_type(rel_type: &str) -> bool {
    matches!(
        rel_type,
        REL_TYPE_IMAGE | REL_TYPE_VIDEO | REL_TYPE_AUDIO | REL_TYPE_MEDIA
    )
}

fn validation_diagnostic(code: &str, severity: &str, message: impl Into<String>) -> Value {
    json!({
        "code": code,
        "severity": severity,
        "message": message.into(),
    })
}
