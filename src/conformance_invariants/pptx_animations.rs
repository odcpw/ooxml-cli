use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, ResolveResult};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::{CliResult, local_name, xml_attrs, zip_text};

use super::spec::{CONTENT_TYPE_PPTX_SLIDE, PRESENTATIONML_NAMESPACE};
use super::types::PartInfo;
use super::util::diag;

#[derive(Clone, Default)]
struct AnimationNode {
    local_name: String,
    namespace: String,
    attrs: BTreeMap<String, String>,
    parent: Option<usize>,
}

#[derive(Default)]
struct AnimationXmlPart {
    root: Option<usize>,
    nodes: Vec<AnimationNode>,
}

pub(super) fn check_part_pptx_animation_invariants(
    file: &str,
    part: &PartInfo,
) -> CliResult<Vec<Value>> {
    if part.content_type != CONTENT_TYPE_PPTX_SLIDE {
        return Ok(Vec::new());
    }

    let Ok(info) = read_animation_xml_part(file, part) else {
        return Ok(Vec::new());
    };
    let Some(root_id) = info.root else {
        return Ok(Vec::new());
    };
    let root = &info.nodes[root_id];
    if root.local_name != "sld" || root.namespace != PRESENTATIONML_NAMESPACE {
        return Ok(Vec::new());
    }

    Ok(check_slide_animation_targets(&part.uri, &info, root_id))
}

fn read_animation_xml_part(file: &str, part: &PartInfo) -> CliResult<AnimationXmlPart> {
    let xml = zip_text(file, &part.entry_name)?;
    let mut reader = NsReader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut info = AnimationXmlPart::default();
    let mut stack = Vec::<usize>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let id = push_node(&mut info, &e, &reader, stack.last().copied());
                stack.push(id);
            }
            Ok(Event::Empty(e)) => {
                push_node(&mut info, &e, &reader, stack.last().copied());
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

fn push_node(
    info: &mut AnimationXmlPart,
    element: &BytesStart<'_>,
    reader: &NsReader<&[u8]>,
    parent: Option<usize>,
) -> usize {
    let id = info.nodes.len();
    if parent.is_none() && info.root.is_none() {
        info.root = Some(id);
    }
    info.nodes.push(AnimationNode {
        local_name: local_name(element.name().as_ref()).to_string(),
        namespace: element_namespace(element, reader),
        attrs: xml_attrs(element),
        parent,
    });
    id
}

fn element_namespace(element: &BytesStart<'_>, reader: &NsReader<&[u8]>) -> String {
    match reader.resolver().resolve_element(element.name()) {
        (ResolveResult::Bound(Namespace(uri)), _) => String::from_utf8_lossy(uri).to_string(),
        _ => String::new(),
    }
}

fn check_slide_animation_targets(
    part_uri: &str,
    info: &AnimationXmlPart,
    root_id: usize,
) -> Vec<Value> {
    let Some(timing_id) = presentation_child(info, root_id, "timing") else {
        return Vec::new();
    };

    let shape_ids = collect_slide_shape_ids(info, root_id);
    presentation_descendants(info, timing_id, "spTgt")
        .into_iter()
        .enumerate()
        .filter_map(|(idx, node_id)| {
            let node = &info.nodes[node_id];
            let label = animation_target_label(idx + 1, node);
            let raw = attr_trim(node, "spid");
            if raw.is_empty() {
                return Some(diag(
                    "PPTX_ANIMATION_TARGET_REFERENCE",
                    format!("{part_uri} {label} is missing required spid"),
                ));
            }
            let Ok(id) = raw.parse::<i64>() else {
                return Some(diag(
                    "PPTX_ANIMATION_TARGET_REFERENCE",
                    format!("{part_uri} {label} has invalid spid {raw:?}"),
                ));
            };
            if id < 0 {
                return Some(diag(
                    "PPTX_ANIMATION_TARGET_REFERENCE",
                    format!("{part_uri} {label} has invalid spid {raw:?}"),
                ));
            }
            if !shape_ids.contains(&id) {
                return Some(diag(
                    "PPTX_ANIMATION_TARGET_REFERENCE",
                    format!("{part_uri} {label} references missing slide shape id {id}"),
                ));
            }
            None
        })
        .collect()
}

fn collect_slide_shape_ids(info: &AnimationXmlPart, root_id: usize) -> BTreeSet<i64> {
    let Some(c_sld_id) = presentation_child(info, root_id, "cSld") else {
        return BTreeSet::new();
    };
    let Some(sp_tree_id) = presentation_child(info, c_sld_id, "spTree") else {
        return BTreeSet::new();
    };
    presentation_descendants(info, sp_tree_id, "cNvPr")
        .into_iter()
        .filter_map(|node_id| {
            let raw = attr_trim(&info.nodes[node_id], "id");
            raw.parse::<i64>().ok().filter(|id| *id >= 0)
        })
        .collect()
}

fn presentation_child(info: &AnimationXmlPart, parent_id: usize, name: &str) -> Option<usize> {
    info.nodes.iter().enumerate().find_map(|(id, node)| {
        (node.parent == Some(parent_id)
            && node.local_name == name
            && node.namespace == PRESENTATIONML_NAMESPACE)
            .then_some(id)
    })
}

fn presentation_descendants(info: &AnimationXmlPart, parent_id: usize, name: &str) -> Vec<usize> {
    info.nodes
        .iter()
        .enumerate()
        .filter_map(|(id, node)| {
            (node.local_name == name
                && node.namespace == PRESENTATIONML_NAMESPACE
                && is_descendant_of(info, id, parent_id))
            .then_some(id)
        })
        .collect()
}

fn is_descendant_of(info: &AnimationXmlPart, node_id: usize, ancestor_id: usize) -> bool {
    let mut parent = info.nodes[node_id].parent;
    while let Some(parent_id) = parent {
        if parent_id == ancestor_id {
            return true;
        }
        parent = info.nodes[parent_id].parent;
    }
    false
}

fn attr_trim(node: &AnimationNode, name: &str) -> String {
    node.attrs
        .get(name)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn animation_target_label(position: usize, node: &AnimationNode) -> String {
    let spid = attr_trim(node, "spid");
    if spid.is_empty() {
        format!("<p:spTgt> at position {position}")
    } else {
        format!("<p:spTgt spid={spid:?}> at position {position}")
    }
}
