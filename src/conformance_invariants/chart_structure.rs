use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::zip_text;

use super::spec::{
    CHART_NAMESPACE, CONTENT_TYPE_CHART, CONTENT_TYPE_DRAWING, SPREADSHEET_DRAWING_NAMESPACE,
};
use super::types::PartInfo;
use super::util::{diag, xml_element_info};

pub(super) fn check_chart_structure_invariants(file: &str, part: &PartInfo) -> Vec<Value> {
    match part.content_type.as_str() {
        CONTENT_TYPE_DRAWING => {
            let Some(root) = read_expected_root(file, part, "wsDr", SPREADSHEET_DRAWING_NAMESPACE)
            else {
                return Vec::new();
            };
            check_worksheet_drawing(&part.uri, &root)
        }
        CONTENT_TYPE_CHART => {
            let Some(root) = read_expected_root(file, part, "chartSpace", CHART_NAMESPACE) else {
                return Vec::new();
            };
            check_chart_part(&part.uri, &root)
        }
        _ => Vec::new(),
    }
}

#[derive(Clone, Default)]
struct XmlNode {
    local_name: String,
    namespace: String,
    attrs: BTreeMap<String, String>,
    children: Vec<XmlNode>,
}

impl XmlNode {
    fn from_start(element: &BytesStart<'_>) -> Self {
        let info = xml_element_info(element);
        Self {
            local_name: info.local_name,
            namespace: info.namespace,
            attrs: info.attrs,
            children: Vec::new(),
        }
    }
}

fn read_expected_root(
    file: &str,
    part: &PartInfo,
    expected_local: &str,
    expected_namespace: &str,
) -> Option<XmlNode> {
    let xml = zip_text(file, &part.entry_name).ok()?;
    let root = parse_xml_tree(&xml).ok()??;
    if root.local_name == expected_local && root.namespace == expected_namespace {
        Some(root)
    } else {
        None
    }
}

fn parse_xml_tree(xml: &str) -> Result<Option<XmlNode>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut root = None;
    let mut stack = Vec::<XmlNode>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                stack.push(XmlNode::from_start(&element));
            }
            Ok(Event::Empty(element)) => {
                attach_node(&mut root, &mut stack, XmlNode::from_start(&element));
            }
            Ok(Event::End(_)) => {
                let Some(node) = stack.pop() else {
                    continue;
                };
                attach_node(&mut root, &mut stack, node);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(err.to_string()),
            _ => {}
        }
    }

    Ok(root)
}

fn attach_node(root: &mut Option<XmlNode>, stack: &mut [XmlNode], node: XmlNode) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if root.is_none() {
        *root = Some(node);
    }
}

fn check_worksheet_drawing(part_uri: &str, root: &XmlNode) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    for anchor in &root.children {
        match anchor.local_name.as_str() {
            "twoCellAnchor" => diagnostics.extend(check_two_cell_anchor(part_uri, anchor)),
            "oneCellAnchor" => {
                diagnostics.extend(check_ordered_container(
                    part_uri,
                    anchor,
                    one_cell_anchor_order,
                    "XLSX_DRAWING_ANCHOR_ORDER",
                ));
                diagnostics.extend(require_children(
                    part_uri,
                    anchor,
                    "XLSX_DRAWING_ANCHOR_REQUIRED",
                    &["from", "ext", "clientData"],
                ));
            }
            "absoluteAnchor" => {
                diagnostics.extend(check_ordered_container(
                    part_uri,
                    anchor,
                    absolute_anchor_order,
                    "XLSX_DRAWING_ANCHOR_ORDER",
                ));
                diagnostics.extend(require_children(
                    part_uri,
                    anchor,
                    "XLSX_DRAWING_ANCHOR_REQUIRED",
                    &["pos", "ext", "clientData"],
                ));
            }
            _ => {}
        }
    }
    diagnostics
}

fn check_two_cell_anchor(part_uri: &str, anchor: &XmlNode) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(check_ordered_container(
        part_uri,
        anchor,
        two_cell_anchor_order,
        "XLSX_DRAWING_ANCHOR_ORDER",
    ));
    diagnostics.extend(require_children(
        part_uri,
        anchor,
        "XLSX_DRAWING_ANCHOR_REQUIRED",
        &["from", "to", "clientData"],
    ));
    let object_count = anchor
        .children
        .iter()
        .filter(|child| {
            matches!(
                child.local_name.as_str(),
                "sp" | "grpSp" | "graphicFrame" | "cxnSp" | "pic" | "contentPart"
            )
        })
        .count();
    if object_count != 1 {
        diagnostics.push(diag(
            "XLSX_DRAWING_ANCHOR_REQUIRED",
            format!(
                "{part_uri} twoCellAnchor must contain exactly one drawing object before clientData, found {object_count}"
            ),
        ));
    }
    diagnostics
}

fn check_chart_part(part_uri: &str, root: &XmlNode) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(check_ordered_container(
        part_uri,
        root,
        chart_space_child_order,
        "OOXML_CHARTSPACE_CHILD_ORDER",
    ));
    for chart in children_by_local(root, "chart") {
        diagnostics.extend(check_ordered_container(
            part_uri,
            chart,
            chart_child_order,
            "OOXML_CHART_CHILD_ORDER",
        ));
        for plot_area in children_by_local(chart, "plotArea") {
            diagnostics.extend(check_ordered_container(
                part_uri,
                plot_area,
                plot_area_child_order,
                "OOXML_PLOTAREA_CHILD_ORDER",
            ));
            diagnostics.extend(check_chart_type_order(part_uri, plot_area));
            diagnostics.extend(check_chart_axis_references(part_uri, plot_area));
            diagnostics.extend(check_chart_series_caches(part_uri, plot_area));
        }
    }
    diagnostics
}

fn check_chart_type_order(part_uri: &str, plot_area: &XmlNode) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    for child in &plot_area.children {
        match child.local_name.as_str() {
            "barChart" => diagnostics.extend(check_ordered_container(
                part_uri,
                child,
                bar_chart_child_order,
                "OOXML_BARCHART_CHILD_ORDER",
            )),
            "lineChart" => diagnostics.extend(check_ordered_container(
                part_uri,
                child,
                line_chart_child_order,
                "OOXML_LINECHART_CHILD_ORDER",
            )),
            "areaChart" => diagnostics.extend(check_ordered_container(
                part_uri,
                child,
                area_chart_child_order,
                "OOXML_AREACHART_CHILD_ORDER",
            )),
            "pieChart" => diagnostics.extend(check_ordered_container(
                part_uri,
                child,
                pie_chart_child_order,
                "OOXML_PIECHART_CHILD_ORDER",
            )),
            "scatterChart" => diagnostics.extend(check_ordered_container(
                part_uri,
                child,
                scatter_chart_child_order,
                "OOXML_SCATTERCHART_CHILD_ORDER",
            )),
            _ => {}
        }
    }
    diagnostics
}

fn check_chart_axis_references(part_uri: &str, plot_area: &XmlNode) -> Vec<Value> {
    let mut axis_labels = BTreeMap::<String, String>::new();
    let mut diagnostics = Vec::new();

    for axis in &plot_area.children {
        if !is_chart_axis_element(&axis.local_name) {
            continue;
        }
        let label = chart_axis_label(axis);
        let axis_id = chart_child_val(axis, "axId");
        if axis_id.is_empty() {
            diagnostics.push(diag(
                "OOXML_CHART_AXIS_REFERENCE",
                format!("{part_uri} {label} is missing required <c:axId val>"),
            ));
        } else if let Some(first) = axis_labels.get(&axis_id) {
            diagnostics.push(diag(
                "OOXML_CHART_AXIS_REFERENCE",
                format!("{part_uri} {label} duplicates axis id {axis_id} from {first}"),
            ));
        } else {
            axis_labels.insert(axis_id.clone(), label.clone());
        }

        let cross_id = chart_child_val(axis, "crossAx");
        if cross_id.is_empty() {
            diagnostics.push(diag(
                "OOXML_CHART_AXIS_REFERENCE",
                format!("{part_uri} {label} is missing required <c:crossAx val>"),
            ));
            continue;
        }
        if !axis_id.is_empty() && cross_id == axis_id {
            diagnostics.push(diag(
                "OOXML_CHART_AXIS_REFERENCE",
                format!("{part_uri} {label} crossAx references its own axis id {axis_id}"),
            ));
        }
    }

    for axis in &plot_area.children {
        if !is_chart_axis_element(&axis.local_name) {
            continue;
        }
        let cross_id = chart_child_val(axis, "crossAx");
        if cross_id.is_empty() {
            continue;
        }
        if !axis_labels.contains_key(&cross_id) {
            diagnostics.push(diag(
                "OOXML_CHART_AXIS_REFERENCE",
                format!(
                    "{part_uri} {} crossAx references missing axis id {cross_id}",
                    chart_axis_label(axis)
                ),
            ));
        }
    }

    for plot in &plot_area.children {
        let plot_name = plot.local_name.as_str();
        if !chart_type_requires_axes(plot_name) {
            continue;
        }
        let refs = chart_axis_ref_ids(plot);
        if refs.len() < 2 {
            diagnostics.push(diag(
                "OOXML_CHART_AXIS_REFERENCE",
                format!(
                    "{part_uri} <c:{plot_name}> has {} <c:axId> references; expected at least 2 axis references",
                    refs.len()
                ),
            ));
        }
        let mut seen = BTreeSet::<String>::new();
        for (idx, axis_id) in refs.iter().enumerate() {
            let label = format!("<c:{plot_name}>/<c:axId #{}>", idx + 1);
            if axis_id.is_empty() {
                diagnostics.push(diag(
                    "OOXML_CHART_AXIS_REFERENCE",
                    format!("{part_uri} {label} is missing required val"),
                ));
                continue;
            }
            if seen.contains(axis_id) {
                diagnostics.push(diag(
                    "OOXML_CHART_AXIS_REFERENCE",
                    format!(
                        "{part_uri} {label} duplicates axis id {axis_id} inside <c:{plot_name}>"
                    ),
                ));
            }
            seen.insert(axis_id.clone());
            if !axis_labels.contains_key(axis_id) {
                diagnostics.push(diag(
                    "OOXML_CHART_AXIS_REFERENCE",
                    format!("{part_uri} {label} references missing axis id {axis_id}"),
                ));
            }
        }
    }

    diagnostics
}

fn check_chart_series_caches(part_uri: &str, plot_area: &XmlNode) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    for plot in &plot_area.children {
        let series = children_by_local(plot, "ser");
        if series.is_empty() {
            continue;
        }
        let mut seen_idx = BTreeMap::<i64, String>::new();
        let mut seen_order = BTreeMap::<i64, String>::new();
        for (idx, ser) in series.iter().enumerate() {
            let label = format!("<c:{}>/<c:ser #{}>", plot.local_name, idx + 1);
            diagnostics.extend(check_chart_series_ordinal(
                part_uri,
                &label,
                ser,
                "idx",
                &mut seen_idx,
            ));
            diagnostics.extend(check_chart_series_ordinal(
                part_uri,
                &label,
                ser,
                "order",
                &mut seen_order,
            ));
            diagnostics.extend(check_chart_series_source_caches(part_uri, &label, ser));
        }
    }
    diagnostics
}

fn check_chart_series_ordinal(
    part_uri: &str,
    label: &str,
    ser: &XmlNode,
    child_name: &str,
    seen: &mut BTreeMap<i64, String>,
) -> Vec<Value> {
    let Some((value, raw)) = chart_required_int_child_val(ser, child_name) else {
        return vec![diag(
            "OOXML_CHART_SERIES_CACHE",
            format!("{part_uri} {label} is missing required <c:{child_name} val>"),
        )];
    };
    if value < 0 {
        return vec![diag(
            "OOXML_CHART_SERIES_CACHE",
            format!(
                "{part_uri} {label} has invalid <c:{child_name} val={raw:?}>; expected a non-negative integer"
            ),
        )];
    }
    if let Some(first) = seen.get(&value) {
        vec![diag(
            "OOXML_CHART_SERIES_CACHE",
            format!("{part_uri} {label} duplicates series {child_name} {value} from {first}"),
        )]
    } else {
        seen.insert(value, label.to_string());
        Vec::new()
    }
}

fn check_chart_series_source_caches(
    part_uri: &str,
    series_label: &str,
    ser: &XmlNode,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    for role_name in ["tx", "cat", "val", "xVal", "yVal", "bubbleSize"] {
        let Some(role) = first_child_by_local(ser, role_name) else {
            continue;
        };
        for ref_name in ["strRef", "numRef"] {
            let Some(reference) = first_child_by_local(role, ref_name) else {
                continue;
            };
            let ref_label = format!("{series_label}/<c:{role_name}>/<c:{ref_name}>");
            if first_child_by_local(reference, "f").is_none() {
                diagnostics.push(diag(
                    "OOXML_CHART_SERIES_CACHE",
                    format!("{part_uri} {ref_label} is missing required <c:f>"),
                ));
            }
            let expected_cache = chart_expected_cache_for_ref(ref_name);
            for cache in chart_direct_caches(reference) {
                let cache_name = cache.local_name.as_str();
                if !expected_cache.is_empty() && cache_name != expected_cache {
                    diagnostics.push(diag(
                        "OOXML_CHART_SERIES_CACHE",
                        format!(
                            "{part_uri} {ref_label} has <c:{cache_name}>; expected <c:{expected_cache}>"
                        ),
                    ));
                }
                diagnostics.extend(check_chart_series_cache(part_uri, &ref_label, cache));
            }
        }
    }
    diagnostics
}

fn check_chart_series_cache(part_uri: &str, ref_label: &str, cache: &XmlNode) -> Vec<Value> {
    let cache_name = cache.local_name.as_str();
    let points = children_by_local(cache, "pt");
    let mut diagnostics = Vec::new();
    match chart_required_int_child_val(cache, "ptCount") {
        None => diagnostics.push(diag(
            "OOXML_CHART_SERIES_CACHE",
            format!("{part_uri} {ref_label}/<c:{cache_name}> is missing required <c:ptCount val>"),
        )),
        Some((count, raw_count)) if count < 0 => diagnostics.push(diag(
            "OOXML_CHART_SERIES_CACHE",
            format!(
                "{part_uri} {ref_label}/<c:{cache_name}> has invalid ptCount {raw_count:?}; expected a non-negative integer"
            ),
        )),
        Some((count, _)) if count as usize != points.len() => diagnostics.push(diag(
            "OOXML_CHART_SERIES_CACHE",
            format!(
                "{part_uri} {ref_label}/<c:{cache_name}> ptCount={count} but contains {} <c:pt> elements",
                points.len()
            ),
        )),
        _ => {}
    }

    let mut seen_point_idx = BTreeSet::<i64>::new();
    for (point_number, point) in points.iter().enumerate() {
        let point_label = format!("{ref_label}/<c:{cache_name}>/<c:pt #{}>", point_number + 1);
        let Some((point_idx, raw_idx)) = chart_required_int_attr(point, "idx") else {
            diagnostics.push(diag(
                "OOXML_CHART_SERIES_CACHE",
                format!("{part_uri} {point_label} is missing required idx"),
            ));
            continue;
        };
        if point_idx < 0 {
            diagnostics.push(diag(
                "OOXML_CHART_SERIES_CACHE",
                format!(
                    "{part_uri} {point_label} has invalid idx {raw_idx:?}; expected a non-negative integer"
                ),
            ));
            continue;
        }
        if seen_point_idx.contains(&point_idx) {
            diagnostics.push(diag(
                "OOXML_CHART_SERIES_CACHE",
                format!("{part_uri} {point_label} duplicates point idx {point_idx}"),
            ));
        }
        seen_point_idx.insert(point_idx);
    }
    diagnostics
}

fn check_ordered_container(
    part_uri: &str,
    parent: &XmlNode,
    order: fn(&str) -> usize,
    code: &str,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    let mut last_order = 0usize;
    let mut last_name = "";
    for child in &parent.children {
        let current = order(&child.local_name);
        if current == 0 {
            continue;
        }
        if last_order > current {
            diagnostics.push(diag(
                code,
                format!(
                    "{part_uri} <{}> has <{}> after <{last_name}>; expected schema child order",
                    parent.local_name, child.local_name
                ),
            ));
            continue;
        }
        last_order = current;
        last_name = &child.local_name;
    }
    diagnostics
}

fn require_children(part_uri: &str, parent: &XmlNode, code: &str, names: &[&str]) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    for name in names {
        if first_child_by_local(parent, name).is_none() {
            diagnostics.push(diag(
                code,
                format!(
                    "{part_uri} <{}> is missing required <{name}>",
                    parent.local_name
                ),
            ));
        }
    }
    diagnostics
}

fn first_child_by_local<'a>(parent: &'a XmlNode, name: &str) -> Option<&'a XmlNode> {
    parent
        .children
        .iter()
        .find(|child| child.local_name == name)
}

fn children_by_local<'a>(parent: &'a XmlNode, name: &str) -> Vec<&'a XmlNode> {
    parent
        .children
        .iter()
        .filter(|child| child.local_name == name)
        .collect()
}

fn is_chart_axis_element(name: &str) -> bool {
    matches!(name, "catAx" | "dateAx" | "valAx" | "serAx")
}

fn chart_type_requires_axes(name: &str) -> bool {
    matches!(
        name,
        "areaChart"
            | "area3DChart"
            | "barChart"
            | "bar3DChart"
            | "bubbleChart"
            | "lineChart"
            | "line3DChart"
            | "radarChart"
            | "scatterChart"
            | "stockChart"
            | "surfaceChart"
            | "surface3DChart"
    )
}

fn chart_axis_ref_ids(plot: &XmlNode) -> Vec<String> {
    children_by_local(plot, "axId")
        .into_iter()
        .map(|child| attr_trimmed(child, "val"))
        .collect()
}

fn chart_axis_label(axis: &XmlNode) -> String {
    let axis_id = chart_child_val(axis, "axId");
    if axis_id.is_empty() {
        format!("<c:{}>", axis.local_name)
    } else {
        format!("<c:{} axId={axis_id:?}>", axis.local_name)
    }
}

fn chart_child_val(parent: &XmlNode, child_name: &str) -> String {
    first_child_by_local(parent, child_name)
        .map(|child| attr_trimmed(child, "val"))
        .unwrap_or_default()
}

fn chart_expected_cache_for_ref(ref_name: &str) -> &str {
    match ref_name {
        "numRef" => "numCache",
        "strRef" => "strCache",
        _ => "",
    }
}

fn chart_direct_caches(parent: &XmlNode) -> Vec<&XmlNode> {
    parent
        .children
        .iter()
        .filter(|child| matches!(child.local_name.as_str(), "numCache" | "strCache"))
        .collect()
}

fn chart_required_int_child_val(parent: &XmlNode, child_name: &str) -> Option<(i64, String)> {
    let child = first_child_by_local(parent, child_name)?;
    chart_required_int_attr(child, "val")
}

fn chart_required_int_attr(element: &XmlNode, attr_name: &str) -> Option<(i64, String)> {
    let raw = attr_trimmed(element, attr_name);
    if raw.is_empty() {
        return None;
    }
    let value = raw.parse::<i64>().unwrap_or(-1);
    Some((value, raw))
}

fn attr_trimmed(element: &XmlNode, attr_name: &str) -> String {
    element
        .attrs
        .get(attr_name)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn two_cell_anchor_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "from",
            "to",
            "sp",
            "grpSp",
            "graphicFrame",
            "cxnSp",
            "pic",
            "contentPart",
            "clientData",
        ],
    )
}

fn one_cell_anchor_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "from",
            "ext",
            "sp",
            "grpSp",
            "graphicFrame",
            "cxnSp",
            "pic",
            "contentPart",
            "clientData",
        ],
    )
}

fn absolute_anchor_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "pos",
            "ext",
            "sp",
            "grpSp",
            "graphicFrame",
            "cxnSp",
            "pic",
            "contentPart",
            "clientData",
        ],
    )
}

fn chart_space_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "date1904",
            "lang",
            "roundedCorners",
            "style",
            "clrMapOvr",
            "pivotSource",
            "protection",
            "chart",
            "spPr",
            "txPr",
            "externalData",
            "printSettings",
            "userShapes",
            "extLst",
        ],
    )
}

fn chart_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "title",
            "autoTitleDeleted",
            "pivotFmts",
            "view3D",
            "floor",
            "sideWall",
            "backWall",
            "plotArea",
            "legend",
            "plotVisOnly",
            "dispBlanksAs",
            "showDLblsOverMax",
            "extLst",
        ],
    )
}

fn plot_area_child_order(name: &str) -> usize {
    match name {
        "layout" => 1,
        "areaChart" | "area3DChart" | "lineChart" | "line3DChart" | "stockChart" | "radarChart"
        | "scatterChart" | "pieChart" | "pie3DChart" | "doughnutChart" | "barChart"
        | "bar3DChart" | "ofPieChart" | "surfaceChart" | "surface3DChart" | "bubbleChart" => 2,
        "valAx" | "catAx" | "dateAx" | "serAx" => 3,
        "dTable" => 4,
        "spPr" => 5,
        "extLst" => 6,
        _ => 0,
    }
}

fn bar_chart_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "barDir",
            "grouping",
            "varyColors",
            "ser",
            "dLbls",
            "gapWidth",
            "overlap",
            "serLines",
            "axId",
            "extLst",
        ],
    )
}

fn line_chart_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "grouping",
            "varyColors",
            "ser",
            "dLbls",
            "dropLines",
            "hiLowLines",
            "upDownBars",
            "marker",
            "smooth",
            "axId",
            "extLst",
        ],
    )
}

fn area_chart_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "grouping",
            "varyColors",
            "ser",
            "dLbls",
            "dropLines",
            "axId",
            "extLst",
        ],
    )
}

fn pie_chart_child_order(name: &str) -> usize {
    order_index(
        name,
        &["varyColors", "ser", "dLbls", "firstSliceAng", "extLst"],
    )
}

fn scatter_chart_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "scatterStyle",
            "varyColors",
            "ser",
            "dLbls",
            "axId",
            "extLst",
        ],
    )
}

fn order_index(name: &str, ordered_names: &[&str]) -> usize {
    ordered_names
        .iter()
        .position(|candidate| *candidate == name)
        .map(|idx| idx + 1)
        .unwrap_or(0)
}
