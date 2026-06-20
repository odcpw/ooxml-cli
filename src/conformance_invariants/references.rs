use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::xml_util::{attr_bound_ns, attr_exact};
use crate::{element_in_ns, local_name, relationships_part_for, zip_text};

use super::relationships::parse_relationship_part;
use super::spec::{
    CONTENT_TYPE_PPTX_SLIDE_LAYOUT, CONTENT_TYPE_PPTX_SLIDE_MASTER, REL_TYPE_PPTX_SLIDE,
    REL_TYPE_PPTX_SLIDE_LAYOUT, REL_TYPE_PPTX_SLIDE_MASTER, REL_TYPE_XLSX_CHARTSHEET,
    REL_TYPE_XLSX_DIALOGSHEET, REL_TYPE_XLSX_WORKSHEET, is_pptx_presentation_content_type,
    is_xlsx_workbook_content_type,
};
use super::types::{PartInfo, RelationshipRecord};
use super::util::{diag, normalize_uri};

const OFFICE_RELATIONSHIPS_NS: &[u8] =
    b"http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PRESENTATIONML_NS: &[u8] = b"http://schemas.openxmlformats.org/presentationml/2006/main";
const SPREADSHEETML_NS: &[u8] = b"http://schemas.openxmlformats.org/spreadsheetml/2006/main";

pub(super) fn check_reference_list_invariants(
    file: &str,
    entry_set: &BTreeSet<String>,
    parts: &[PartInfo],
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    for part in parts {
        let rels = relationships_for_part(file, entry_set, &part.uri);
        match part.content_type.as_str() {
            ct if is_xlsx_workbook_content_type(ct) => {
                if let Some(lists) = collect_reference_lists(
                    file,
                    part,
                    "workbook",
                    SPREADSHEETML_NS,
                    &[ReferenceListSpec {
                        list_name: "sheets",
                        item_name: "sheet",
                    }],
                ) {
                    diagnostics.extend(check_workbook_sheet_references(
                        &part.uri,
                        lists.items("sheets"),
                        &rels,
                    ));
                }
            }
            ct if is_pptx_presentation_content_type(ct) => {
                if let Some(lists) = collect_reference_lists(
                    file,
                    part,
                    "presentation",
                    PRESENTATIONML_NS,
                    &[
                        ReferenceListSpec {
                            list_name: "sldMasterIdLst",
                            item_name: "sldMasterId",
                        },
                        ReferenceListSpec {
                            list_name: "sldIdLst",
                            item_name: "sldId",
                        },
                    ],
                ) {
                    let rel_map = relationships_by_id(&rels);
                    diagnostics.extend(check_presentation_reference_list(
                        &part.uri,
                        lists.items("sldMasterIdLst"),
                        &rel_map,
                        "sldMasterId",
                        REL_TYPE_PPTX_SLIDE_MASTER,
                        "slide master",
                    ));
                    diagnostics.extend(check_presentation_reference_list(
                        &part.uri,
                        lists.items("sldIdLst"),
                        &rel_map,
                        "sldId",
                        REL_TYPE_PPTX_SLIDE,
                        "slide",
                    ));
                }
            }
            CONTENT_TYPE_PPTX_SLIDE_LAYOUT => {
                if collect_reference_lists(file, part, "sldLayout", PRESENTATIONML_NS, &[])
                    .is_some()
                {
                    diagnostics.extend(check_slide_layout_master_relationship(&part.uri, &rels));
                }
            }
            CONTENT_TYPE_PPTX_SLIDE_MASTER => {
                if let Some(lists) = collect_reference_lists(
                    file,
                    part,
                    "sldMaster",
                    PRESENTATIONML_NS,
                    &[ReferenceListSpec {
                        list_name: "sldLayoutIdLst",
                        item_name: "sldLayoutId",
                    }],
                ) {
                    diagnostics.extend(check_slide_master_layout_references(
                        &part.uri,
                        lists.items("sldLayoutIdLst"),
                        &rels,
                    ));
                }
            }
            _ => {}
        }
    }
    diagnostics
}

#[derive(Clone)]
struct ReferenceListSpec {
    list_name: &'static str,
    item_name: &'static str,
}

struct ReferenceParseConfig<'a> {
    expected_root: &'a str,
    expected_namespace: &'a [u8],
    specs: &'a [ReferenceListSpec],
}

#[derive(Clone, Default)]
struct ReferenceElement {
    id: String,
    name: String,
    rid: String,
    sheet_id: String,
}

#[derive(Default)]
struct ReferenceLists {
    by_list: BTreeMap<&'static str, Vec<ReferenceElement>>,
}

impl ReferenceLists {
    fn push(&mut self, list_name: &'static str, element: ReferenceElement) {
        self.by_list.entry(list_name).or_default().push(element);
    }

    fn items(&self, list_name: &str) -> &[ReferenceElement] {
        self.by_list
            .get(list_name)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

fn collect_reference_lists(
    file: &str,
    part: &PartInfo,
    expected_root: &str,
    expected_namespace: &[u8],
    specs: &[ReferenceListSpec],
) -> Option<ReferenceLists> {
    let xml = zip_text(file, &part.entry_name).ok()?;
    let mut reader = NsReader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut lists = ReferenceLists::default();
    let mut seen_root = false;
    let config = ReferenceParseConfig {
        expected_root,
        expected_namespace,
        specs,
    };

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if !handle_element(&reader, &e, &stack, &mut lists, &mut seen_root, &config)? {
                    return None;
                }
                stack.push(local_name(e.name().as_ref()).to_string());
            }
            Ok(Event::Empty(e)) => {
                if !handle_element(&reader, &e, &stack, &mut lists, &mut seen_root, &config)? {
                    return None;
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    if seen_root { Some(lists) } else { None }
}

fn handle_element(
    reader: &NsReader<&[u8]>,
    element: &BytesStart<'_>,
    stack: &[String],
    lists: &mut ReferenceLists,
    seen_root: &mut bool,
    config: &ReferenceParseConfig<'_>,
) -> Option<bool> {
    let element_name = element.name();
    let name = local_name(element_name.as_ref());
    let in_expected_namespace =
        element_in_ns(reader.resolver(), element, config.expected_namespace);
    if stack.is_empty() && !*seen_root {
        *seen_root = true;
        return Some(name == config.expected_root && in_expected_namespace);
    }
    if !in_expected_namespace || stack.len() != 2 {
        return Some(true);
    }
    let parent = stack.last().map(String::as_str).unwrap_or_default();
    for spec in config.specs {
        if parent == spec.list_name && name == spec.item_name {
            lists.push(
                spec.list_name,
                reference_element(element, reader.resolver()),
            );
            break;
        }
    }
    Some(true)
}

fn reference_element(
    element: &BytesStart<'_>,
    resolver: &quick_xml::name::NamespaceResolver,
) -> ReferenceElement {
    ReferenceElement {
        id: attr_exact(element, "id")
            .unwrap_or_default()
            .trim()
            .to_string(),
        name: attr_exact(element, "name")
            .unwrap_or_default()
            .trim()
            .to_string(),
        rid: relationship_id_attr(element, resolver),
        sheet_id: attr_exact(element, "sheetId")
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

fn relationship_id_attr(
    element: &BytesStart<'_>,
    resolver: &quick_xml::name::NamespaceResolver,
) -> String {
    attr_bound_ns(element, resolver, OFFICE_RELATIONSHIPS_NS, b"id")
        .or_else(|| attr_exact(element, "r:id"))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn relationships_for_part(
    file: &str,
    entry_set: &BTreeSet<String>,
    part_uri: &str,
) -> Vec<RelationshipRecord> {
    let rels_part = relationships_part_for(part_uri.trim_start_matches('/'));
    if !entry_set.contains(&normalize_uri(&rels_part)) {
        return Vec::new();
    }
    parse_relationship_part(file, &rels_part).unwrap_or_default()
}

fn relationships_by_id(rels: &[RelationshipRecord]) -> BTreeMap<String, RelationshipRecord> {
    let mut out = BTreeMap::new();
    for rel in rels {
        let id = rel.id.trim();
        if !id.is_empty() {
            out.insert(id.to_string(), rel.clone());
        }
    }
    out
}

fn check_workbook_sheet_references(
    part_uri: &str,
    sheets: &[ReferenceElement],
    rels: &[RelationshipRecord],
) -> Vec<Value> {
    let rel_map = relationships_by_id(rels);
    let mut diagnostics = Vec::new();
    for (idx, sheet) in sheets.iter().enumerate() {
        let label = workbook_sheet_label(idx + 1, sheet);
        let rid = sheet.rid.as_str();
        if rid.is_empty() {
            diagnostics.push(diag(
                "XLSX_WORKBOOK_SHEET_REFERENCE",
                format!(
                    "{part_uri} {label} is missing required r:id for its worksheet relationship"
                ),
            ));
            continue;
        }
        let Some(rel) = rel_map.get(rid) else {
            diagnostics.push(diag(
                "XLSX_WORKBOOK_SHEET_REFERENCE",
                format!("{part_uri} {label} references missing workbook relationship {rid}"),
            ));
            continue;
        };
        if is_external_relationship(rel) {
            diagnostics.push(diag(
                "XLSX_WORKBOOK_SHEET_REFERENCE",
                format!("{part_uri} {label} relationship {rid} points to an external target; workbook sheets must resolve to internal worksheet parts"),
            ));
            continue;
        }
        if !is_workbook_sheet_relationship_type(&rel.rel_type) {
            diagnostics.push(diag(
                "XLSX_WORKBOOK_SHEET_REFERENCE",
                format!(
                    "{part_uri} {label} relationship {rid} has type {:?}, expected a workbook sheet relationship",
                    rel.rel_type
                ),
            ));
        }
    }
    diagnostics
}

fn check_presentation_reference_list(
    part_uri: &str,
    elements: &[ReferenceElement],
    rel_map: &BTreeMap<String, RelationshipRecord>,
    item_name: &str,
    expected_rel_type: &str,
    target_label: &str,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    for (idx, element) in elements.iter().enumerate() {
        let label = presentation_reference_label(item_name, idx + 1, element);
        let rid = element.rid.as_str();
        if rid.is_empty() {
            diagnostics.push(diag(
                "PPTX_PRESENTATION_REFERENCE",
                format!(
                    "{part_uri} {label} is missing required r:id for its {target_label} relationship"
                ),
            ));
            continue;
        }
        let Some(rel) = rel_map.get(rid) else {
            diagnostics.push(diag(
                "PPTX_PRESENTATION_REFERENCE",
                format!("{part_uri} {label} references missing presentation relationship {rid}"),
            ));
            continue;
        };
        if is_external_relationship(rel) {
            diagnostics.push(diag(
                "PPTX_PRESENTATION_REFERENCE",
                format!("{part_uri} {label} relationship {rid} points to an external target; presentation {target_label} entries must resolve to internal parts"),
            ));
            continue;
        }
        if rel.rel_type != expected_rel_type {
            diagnostics.push(diag(
                "PPTX_PRESENTATION_REFERENCE",
                format!(
                    "{part_uri} {label} relationship {rid} has type {:?}, expected {:?}",
                    rel.rel_type, expected_rel_type
                ),
            ));
        }
    }
    diagnostics
}

fn check_slide_layout_master_relationship(
    part_uri: &str,
    rels: &[RelationshipRecord],
) -> Vec<Value> {
    let mut found_master = false;
    let mut diagnostics = Vec::new();
    for rel in rels {
        if rel.rel_type != REL_TYPE_PPTX_SLIDE_MASTER {
            continue;
        }
        found_master = true;
        if is_external_relationship(rel) {
            diagnostics.push(diag(
                "PPTX_SLIDE_LAYOUT_MASTER_REFERENCE",
                format!(
                    "{part_uri} slide master relationship {} points to an external target",
                    relationship_id_or_placeholder(rel)
                ),
            ));
        }
    }
    if !found_master {
        diagnostics.push(diag(
            "PPTX_SLIDE_LAYOUT_MASTER_REFERENCE",
            format!(
                "{part_uri} has no slideMaster relationship; slide layouts must resolve to a slide master"
            ),
        ));
    }
    diagnostics
}

fn check_slide_master_layout_references(
    part_uri: &str,
    elements: &[ReferenceElement],
    rels: &[RelationshipRecord],
) -> Vec<Value> {
    let rel_map = relationships_by_id(rels);
    let mut diagnostics = Vec::new();
    for (idx, element) in elements.iter().enumerate() {
        let label = presentation_reference_label("sldLayoutId", idx + 1, element);
        diagnostics.extend(check_internal_relationship_reference(
            part_uri,
            &label,
            element,
            &rel_map,
            REL_TYPE_PPTX_SLIDE_LAYOUT,
            "PPTX_SLIDE_MASTER_LAYOUT_REFERENCE",
        ));
    }
    diagnostics
}

fn check_internal_relationship_reference(
    part_uri: &str,
    label: &str,
    element: &ReferenceElement,
    rel_map: &BTreeMap<String, RelationshipRecord>,
    expected_rel_type: &str,
    code: &str,
) -> Vec<Value> {
    let rid = element.rid.as_str();
    if rid.is_empty() {
        return vec![diag(
            code,
            format!("{part_uri} {label} is missing required r:id for its relationship"),
        )];
    }
    let Some(rel) = rel_map.get(rid) else {
        return vec![diag(
            code,
            format!("{part_uri} {label} references missing relationship {rid}"),
        )];
    };
    if is_external_relationship(rel) {
        return vec![diag(
            code,
            format!(
                "{part_uri} {label} relationship {rid} points to an external target; expected an internal relationship of type {:?}",
                expected_rel_type
            ),
        )];
    }
    if rel.rel_type != expected_rel_type {
        return vec![diag(
            code,
            format!(
                "{part_uri} {label} relationship {rid} has type {:?}, expected {:?}",
                rel.rel_type, expected_rel_type
            ),
        )];
    }
    Vec::new()
}

fn is_workbook_sheet_relationship_type(rel_type: &str) -> bool {
    matches!(
        rel_type,
        REL_TYPE_XLSX_WORKSHEET | REL_TYPE_XLSX_CHARTSHEET | REL_TYPE_XLSX_DIALOGSHEET
    )
}

fn is_external_relationship(rel: &RelationshipRecord) -> bool {
    rel.target_mode.trim().eq_ignore_ascii_case("External")
}

fn workbook_sheet_label(position: usize, sheet: &ReferenceElement) -> String {
    match (!sheet.name.is_empty(), !sheet.sheet_id.is_empty()) {
        (true, true) => format!(
            "sheet #{position} name {:?} sheetId {}",
            sheet.name, sheet.sheet_id
        ),
        (true, false) => format!("sheet #{position} name {:?}", sheet.name),
        (false, true) => format!("sheet #{position} sheetId {}", sheet.sheet_id),
        (false, false) => format!("sheet #{position}"),
    }
}

fn presentation_reference_label(
    item_name: &str,
    position: usize,
    element: &ReferenceElement,
) -> String {
    if element.id.is_empty() {
        format!("<p:{item_name}> at position {position}")
    } else {
        format!("<p:{item_name} id={:?}> at position {position}", element.id)
    }
}

fn relationship_id_or_placeholder(rel: &RelationshipRecord) -> &str {
    let id = rel.id.trim();
    if id.is_empty() { "<missing-id>" } else { id }
}
