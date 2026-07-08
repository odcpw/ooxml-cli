use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, RelationshipEntry, append_xml_text_event, command_arg, is_xml_text_event,
    local_name, relationship_entries, relationships_part_for, resolve_relationship_target,
    selector_candidates, xml_attrs_map, zip_text,
};

use super::{normalize_ppt_target, pptx_slide_refs};

const REL_CHART: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const REL_PACKAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package";

#[derive(Clone, Debug, Default)]
struct ChartRef {
    number: usize,
    slide: u32,
    slide_part_uri: String,
    shape_id: String,
    shape_name: String,
    relationship_id: String,
    part_uri: String,
    title: String,
    types: Vec<String>,
    series: Vec<SeriesRef>,
    embedded_workbook_part_uri: String,
    embedded_workbook_relationship_id: String,
    primary_selector: String,
    selectors: Vec<String>,
    style: Option<Value>,
}

#[derive(Clone, Debug, Default)]
struct SeriesRef {
    number: usize,
    index: i64,
    order: i64,
    name: Option<DataSourceRef>,
    categories: Option<DataSourceRef>,
    values: Option<DataSourceRef>,
    x_values: Option<DataSourceRef>,
    y_values: Option<DataSourceRef>,
    bubble_size: Option<DataSourceRef>,
    primary_selector: String,
    selectors: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct DataSourceRef {
    formula: String,
    sheet: String,
    range: String,
    ref_kind: String,
    cache_type: String,
    point_count: i64,
    cache_preview: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct XmlNode {
    name: String,
    attrs: BTreeMap<String, String>,
    text: String,
    children: Vec<XmlNode>,
}

pub(crate) fn pptx_charts_list(file: &str, slide: i64) -> CliResult<Value> {
    let slide = positive_slide_filter(slide);
    let charts = list_pptx_charts(file, slide)?;
    pptx_charts_result_json(file, &charts)
}

pub(crate) fn pptx_charts_show(file: &str, slide: i64, selector: Option<&str>) -> CliResult<Value> {
    let slide = positive_slide_filter(slide);
    let charts = list_pptx_charts(file, slide)?;
    let chart = select_chart(&charts, selector.unwrap_or_default())?;
    pptx_charts_result_json(file, &[chart])
}

fn positive_slide_filter(slide: i64) -> u32 {
    if slide > 0 { slide as u32 } else { 0 }
}

fn list_pptx_charts(file: &str, slide_filter: u32) -> CliResult<Vec<ChartRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let presentation_rels = relationship_entries(file, "ppt/_rels/presentation.xml.rels")?;
    let presentation_rel_map = relationship_map(presentation_rels);
    let mut charts = Vec::new();

    for (index, (_, rel_id)) in slides.iter().enumerate() {
        let slide_number = index as u32 + 1;
        if slide_filter > 0 && slide_number != slide_filter {
            continue;
        }
        let target = presentation_rel_map
            .get(rel_id)
            .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
        let slide_part = normalize_ppt_target(&target.target);
        let slide_xml = zip_text(file, &slide_part)?;
        let slide_root = parse_xml_tree(&slide_xml)?;
        let slide_rels =
            relationship_entries(file, &relationships_part_for(&format!("/{slide_part}")))?;
        let slide_rel_map = relationship_map(slide_rels);
        let start_number = charts.len() + 1;
        let slide_charts = list_charts_for_slide(
            file,
            &slide_root,
            slide_number,
            &format!("/{slide_part}"),
            &slide_rel_map,
            start_number,
        )?;
        charts.extend(slide_charts);
    }

    Ok(charts)
}

fn list_charts_for_slide(
    file: &str,
    slide_root: &XmlNode,
    slide_number: u32,
    slide_part_uri: &str,
    slide_rel_map: &BTreeMap<String, RelationshipEntry>,
    start_number: usize,
) -> CliResult<Vec<ChartRef>> {
    let mut charts = Vec::new();
    for frame in slide_root.descendants("graphicFrame") {
        let Some(chart_elem) = frame.first_descendant("chart") else {
            continue;
        };
        let rid = relationship_id(chart_elem);
        if rid.is_empty() {
            return Err(CliError::unexpected(format!(
                "slide {slide_part_uri} chart graphicFrame is missing r:id"
            )));
        }
        let rel = slide_rel_map.get(&rid).ok_or_else(|| {
            CliError::unexpected(format!(
                "slide {slide_part_uri} chart relationship {rid} not found"
            ))
        })?;
        if rel.target_mode == "External" {
            return Err(CliError::unexpected(format!(
                "slide {slide_part_uri} chart relationship {rid} is external"
            )));
        }
        if rel.rel_type != REL_CHART {
            return Err(CliError::unexpected(format!(
                "slide {slide_part_uri} relationship {rid} is {}, expected chart",
                rel.rel_type
            )));
        }

        let chart_uri = resolve_relationship_target(slide_part_uri, &rel.target);
        let chart_xml = zip_text(file, chart_uri.trim_start_matches('/'))?;
        let chart_root = parse_xml_tree(&chart_xml)?;
        if chart_root.name != "chartSpace" {
            return Err(CliError::unexpected(format!(
                "chart part {chart_uri} root element not found"
            )));
        }
        let (shape_id, shape_name) = graphic_frame_id_name(frame);
        let (embedded_uri, embedded_rid) = embedded_workbook(file, &chart_uri, &chart_root)?;
        let mut chart = ChartRef {
            number: start_number + charts.len(),
            slide: slide_number,
            slide_part_uri: slide_part_uri.to_string(),
            shape_id,
            shape_name,
            relationship_id: rid,
            part_uri: chart_uri.clone(),
            title: chart_title(&chart_root),
            types: chart_types(&chart_root),
            series: chart_series(&chart_root),
            embedded_workbook_part_uri: embedded_uri,
            embedded_workbook_relationship_id: embedded_rid,
            style: Some(inspect_style(&chart_root, &chart_uri)),
            ..ChartRef::default()
        };
        chart = with_chart_selectors(chart);
        charts.push(chart);
    }
    Ok(charts)
}

fn embedded_workbook(
    file: &str,
    chart_uri: &str,
    chart_root: &XmlNode,
) -> CliResult<(String, String)> {
    let external_rid = chart_root
        .first_descendant("externalData")
        .map(relationship_id)
        .unwrap_or_default();
    let chart_rels = relationship_entries_optional(file, &relationships_part_for(chart_uri))?;
    for rel in &chart_rels {
        if rel.target_mode == "External" || rel.rel_type != REL_PACKAGE {
            continue;
        }
        if !external_rid.is_empty() && rel.id != external_rid {
            continue;
        }
        return Ok((
            resolve_relationship_target(chart_uri, &rel.target),
            rel.id.clone(),
        ));
    }
    if external_rid.is_empty() {
        for rel in &chart_rels {
            if rel.target_mode != "External" && rel.rel_type == REL_PACKAGE {
                return Ok((
                    resolve_relationship_target(chart_uri, &rel.target),
                    rel.id.clone(),
                ));
            }
        }
    }
    Ok((String::new(), String::new()))
}

fn relationship_entries_optional(file: &str, part: &str) -> CliResult<Vec<RelationshipEntry>> {
    match relationship_entries(file, part) {
        Ok(entries) => Ok(entries),
        Err(err) if err.message.contains("missing zip part") => Ok(Vec::new()),
        Err(err) => Err(err),
    }
}

fn relationship_map(entries: Vec<RelationshipEntry>) -> BTreeMap<String, RelationshipEntry> {
    entries
        .into_iter()
        .filter(|rel| !rel.id.is_empty())
        .map(|rel| (rel.id.clone(), rel))
        .collect()
}

fn relationship_id(node: &XmlNode) -> String {
    node.attr_exact("r:id")
        .or_else(|| node.attr("id"))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn graphic_frame_id_name(frame: &XmlNode) -> (String, String) {
    for c_nv_pr in frame.descendants("cNvPr") {
        let id = c_nv_pr.attr("id").unwrap_or_default().trim().to_string();
        let name = c_nv_pr.attr("name").unwrap_or_default().trim().to_string();
        if !id.is_empty() || !name.is_empty() {
            return (id, name);
        }
    }
    (String::new(), String::new())
}

fn chart_title(root: &XmlNode) -> String {
    let Some(title) = root.first_descendant("title") else {
        return String::new();
    };
    title_text(title)
}

fn title_text(title: &XmlNode) -> String {
    let mut parts = title
        .descendants("t")
        .into_iter()
        .map(|node| node.text.clone())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        parts = title
            .descendants("v")
            .into_iter()
            .map(|node| node.text.clone())
            .filter(|text| !text.is_empty())
            .collect();
    }
    parts.join("").trim().to_string()
}

fn chart_types(root: &XmlNode) -> Vec<String> {
    let Some(plot_area) = root.first_descendant("plotArea") else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for child in &plot_area.children {
        if child.name.ends_with("Chart") && !result.iter().any(|name| name == &child.name) {
            result.push(child.name.clone());
        }
    }
    result
}

fn chart_series(root: &XmlNode) -> Vec<SeriesRef> {
    let mut series = Vec::new();
    for ser in walk_series(root) {
        let mut item = SeriesRef {
            number: series.len() + 1,
            index: parse_idx_val(ser.direct_child("idx")),
            order: parse_idx_val(ser.direct_child("order")),
            name: chart_data_source(ser.direct_child("tx")),
            categories: chart_data_source(ser.direct_child("cat")),
            values: chart_data_source(ser.direct_child("val")),
            x_values: chart_data_source(ser.direct_child("xVal")),
            y_values: chart_data_source(ser.direct_child("yVal")),
            bubble_size: chart_data_source(ser.direct_child("bubbleSize")),
            ..SeriesRef::default()
        };
        item = with_series_selectors(item);
        series.push(item);
    }
    series
}

fn walk_series(root: &XmlNode) -> Vec<&XmlNode> {
    let Some(plot_area) = root.first_descendant("plotArea") else {
        return Vec::new();
    };
    let mut series = Vec::new();
    for chart_type in &plot_area.children {
        if chart_type.name.ends_with("Chart") {
            series.extend(chart_type.direct_children("ser"));
        }
    }
    series
}

fn chart_data_source(elem: Option<&XmlNode>) -> Option<DataSourceRef> {
    let elem = elem?;
    let mut source = None;
    for local in ["strRef", "numRef", "multiLvlStrRef"] {
        if let Some(found) = elem.first_descendant(local) {
            source = Some(found);
            break;
        }
    }
    let Some(source) = source else {
        return elem.direct_child("v").map(|value| DataSourceRef {
            cache_type: "literal".to_string(),
            cache_preview: vec![value.text.clone()],
            ..DataSourceRef::default()
        });
    };

    let mut result = DataSourceRef {
        ref_kind: source.name.clone(),
        ..DataSourceRef::default()
    };
    if let Some(formula) = source.direct_child("f") {
        result.formula = formula.text.trim().to_string();
        if let Some((sheet, range)) = split_sheet_range_formula(&result.formula) {
            result.sheet = sheet;
            result.range = range;
        }
    }
    if let Some(cache) = first_cache_child(source) {
        result.cache_type = cache.name.clone();
        if let Some(pt_count) = cache.direct_child("ptCount") {
            result.point_count = parse_i64_attr(pt_count, "val");
        }
        for pt in cache.descendants("pt") {
            if result.cache_preview.len() >= 5 {
                break;
            }
            if let Some(value) = pt.direct_child("v") {
                result.cache_preview.push(value.text.clone());
            }
        }
    }
    if result.formula.is_empty() && result.point_count == 0 && result.cache_preview.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn first_cache_child(source: &XmlNode) -> Option<&XmlNode> {
    source.children.iter().find(|child| {
        matches!(
            child.name.as_str(),
            "strCache" | "numCache" | "multiLvlStrCache"
        )
    })
}

fn split_sheet_range_formula(formula: &str) -> Option<(String, String)> {
    let formula = formula.trim().trim_start_matches('=');
    if formula.is_empty() {
        return None;
    }
    let mut bang = None;
    let mut in_quote = false;
    let bytes = formula.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => {
                if in_quote && i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 1;
                } else {
                    in_quote = !in_quote;
                }
            }
            b'!' if !in_quote => bang = Some(i),
            _ => {}
        }
        i += 1;
    }
    let bang = bang?;
    let mut sheet = formula[..bang].to_string();
    let range = &formula[bang + 1..];
    if sheet.starts_with('\'') && sheet.ends_with('\'') && sheet.len() >= 2 {
        sheet = sheet[1..sheet.len() - 1].replace("''", "'");
    }
    if sheet.contains(['[', ']']) || range.contains(['[', ']', ',']) {
        return None;
    }
    Some((sheet, normalize_a1_range(range)?))
}

fn normalize_a1_range(value: &str) -> Option<String> {
    let parts = value.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [single] => normalize_a1_cell(single),
        [start, end] => Some(format!(
            "{}:{}",
            normalize_a1_cell(start)?,
            normalize_a1_cell(end)?
        )),
        _ => None,
    }
}

fn normalize_a1_cell(value: &str) -> Option<String> {
    let value = value.trim().replace('$', "");
    let split = value
        .char_indices()
        .find_map(|(idx, ch)| ch.is_ascii_digit().then_some(idx))?;
    let (col, row) = value.split_at(split);
    if col.is_empty()
        || row.is_empty()
        || !col.chars().all(|ch| ch.is_ascii_alphabetic())
        || !row.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    Some(format!("${}${}", col.to_ascii_uppercase(), row))
}

fn parse_idx_val(elem: Option<&XmlNode>) -> i64 {
    elem.map(|node| parse_i64_attr(node, "val"))
        .unwrap_or_default()
}

fn parse_i64_attr(node: &XmlNode, attr: &str) -> i64 {
    node.attr(attr)
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or_default()
}

fn parse_f64_attr(node: Option<&XmlNode>, attr: &str) -> Option<f64> {
    node.and_then(|node| node.attr(attr))
        .and_then(|value| value.trim().parse::<f64>().ok())
}

fn inspect_style(root: &XmlNode, chart_uri: &str) -> Value {
    let mut style = Map::new();
    style.insert("partUri".to_string(), json!(chart_uri));
    let types = chart_types(root);
    if !types.is_empty() {
        style.insert("types".to_string(), json!(types));
    }
    let chart = root.direct_child("chart");
    style.insert(
        "title".to_string(),
        chart
            .and_then(|chart| chart.direct_child("title"))
            .map(inspect_title)
            .unwrap_or_else(|| json!({"present": false})),
    );
    style.insert(
        "legend".to_string(),
        chart
            .and_then(|chart| chart.direct_child("legend"))
            .map(inspect_legend)
            .unwrap_or_else(|| json!({"present": false})),
    );
    if let Some(plot_area) = root.first_descendant("plotArea") {
        let axes = inspect_axes(plot_area);
        if !axes.is_empty() {
            style.insert("axes".to_string(), Value::Array(axes));
        }
        let fill = inspect_fill(plot_area.direct_child("spPr"));
        if !fill.is_empty() {
            style.insert("plotAreaFill".to_string(), json!(fill));
        }
    }
    let fill = inspect_fill(root.direct_child("spPr"));
    if !fill.is_empty() {
        style.insert("chartSpaceFill".to_string(), json!(fill));
    }
    let series = inspect_series(root);
    if !series.is_empty() {
        style.insert("series".to_string(), Value::Array(series));
    }
    Value::Object(style)
}

fn inspect_title(title: &XmlNode) -> Value {
    let mut result = Map::new();
    result.insert("present".to_string(), json!(true));
    if title
        .direct_child("tx")
        .and_then(|tx| tx.direct_child("strRef"))
        .is_some()
    {
        result.insert("linked".to_string(), json!(true));
    }
    let text = title_text(title);
    if !text.is_empty() {
        result.insert("text".to_string(), json!(text));
    }
    if let Some(overlay) = title.direct_child("overlay") {
        result.insert("overlay".to_string(), json!(parse_ooxml_bool_attr(overlay)));
    }
    if let Some(font) = inspect_title_font(title) {
        result.insert("font".to_string(), font);
    }
    Value::Object(result)
}

fn inspect_legend(legend: &XmlNode) -> Value {
    let mut result = Map::new();
    result.insert("present".to_string(), json!(true));
    if let Some(pos) = legend
        .direct_child("legendPos")
        .and_then(|pos| pos.attr("val"))
        .filter(|value| !value.trim().is_empty())
    {
        result.insert("position".to_string(), json!(pos.trim()));
    }
    if let Some(overlay) = legend.direct_child("overlay") {
        result.insert("overlay".to_string(), json!(parse_ooxml_bool_attr(overlay)));
    }
    Value::Object(result)
}

fn inspect_axes(plot_area: &XmlNode) -> Vec<Value> {
    let mut axes = Vec::new();
    for child in &plot_area.children {
        if !matches!(child.name.as_str(), "catAx" | "valAx" | "dateAx" | "serAx") {
            continue;
        }
        let mut axis = Map::new();
        axis.insert("element".to_string(), json!(child.name.as_str()));
        axis.insert("kind".to_string(), json!(axis_kind(&child.name)));
        if let Some(id) = child
            .direct_child("axId")
            .and_then(|node| node.attr("val"))
            .filter(|value| !value.trim().is_empty())
        {
            axis.insert("axisId".to_string(), json!(id.trim()));
        }
        if let Some(delete) = child.direct_child("delete") {
            axis.insert("hidden".to_string(), json!(parse_ooxml_bool_attr(delete)));
        }
        if let Some(title) = child.direct_child("title") {
            let text = title_text(title);
            if !text.is_empty() {
                axis.insert("title".to_string(), json!(text));
            }
            if let Some(font) = inspect_title_font(title) {
                axis.insert("titleFont".to_string(), font);
            }
        }
        if let Some(format_code) = child
            .direct_child("numFmt")
            .and_then(|node| node.attr("formatCode"))
            .filter(|value| !value.trim().is_empty())
        {
            axis.insert("numberFormat".to_string(), json!(format_code.trim()));
        }
        if let Some(scaling) = child.direct_child("scaling") {
            if let Some(value) = parse_f64_attr(scaling.direct_child("min"), "val") {
                axis.insert("min".to_string(), json!(value));
            }
            if let Some(value) = parse_f64_attr(scaling.direct_child("max"), "val") {
                axis.insert("max".to_string(), json!(value));
            }
        }
        if let Some(value) = parse_f64_attr(child.direct_child("majorUnit"), "val") {
            axis.insert("majorUnit".to_string(), json!(value));
        }
        axis.insert(
            "majorGridlines".to_string(),
            json!(child.direct_child("majorGridlines").is_some()),
        );
        axis.insert(
            "minorGridlines".to_string(),
            json!(child.direct_child("minorGridlines").is_some()),
        );
        if let Some(font) = inspect_axis_tick_label_font(child) {
            axis.insert("tickLabelFont".to_string(), font);
        }
        axes.push(Value::Object(axis));
    }
    axes
}

fn inspect_series(root: &XmlNode) -> Vec<Value> {
    walk_series(root)
        .into_iter()
        .enumerate()
        .map(|(idx, ser)| inspect_series_style(ser, idx + 1))
        .collect()
}

fn inspect_series_style(ser: &XmlNode, number: usize) -> Value {
    let mut style = Map::new();
    style.insert("number".to_string(), json!(number));
    if let Some(name) = ser
        .direct_child("tx")
        .map(series_name_text)
        .filter(|name| !name.is_empty())
    {
        style.insert("name".to_string(), json!(name));
    }
    if let Some(sp_pr) = ser.direct_child("spPr") {
        let fill = inspect_fill(Some(sp_pr));
        if sp_pr.direct_child("noFill").is_some() {
            style.insert("noFill".to_string(), json!(true));
        } else if !fill.is_empty() {
            style.insert("fillColor".to_string(), json!(fill));
        }
        if let Some(ln) = sp_pr.direct_child("ln") {
            if ln.direct_child("noFill").is_some() {
                style.insert("noLine".to_string(), json!(true));
            } else {
                let color = inspect_fill(Some(ln));
                if !color.is_empty() {
                    style.insert("lineColor".to_string(), json!(color));
                }
            }
            if let Some(width) = ln.attr("w").and_then(|v| v.trim().parse::<f64>().ok()) {
                style.insert(
                    "lineWidthPt".to_string(),
                    json_style_number(width / 12700.0),
                );
            }
        }
    }
    if let Some(marker) = ser.direct_child("marker").and_then(inspect_marker) {
        style.insert("marker".to_string(), marker);
    }
    Value::Object(style)
}

fn inspect_marker(marker: &XmlNode) -> Option<Value> {
    let mut result = Map::new();
    if let Some(symbol) = marker
        .direct_child("symbol")
        .and_then(|node| node.attr("val"))
        .filter(|value| !value.trim().is_empty())
    {
        result.insert("symbol".to_string(), json!(symbol.trim()));
    }
    if let Some(size) = marker
        .direct_child("size")
        .and_then(|node| node.attr("val"))
        .and_then(|value| value.trim().parse::<i64>().ok())
    {
        result.insert("size".to_string(), json!(size));
    }
    (!result.is_empty()).then_some(Value::Object(result))
}

fn inspect_title_font(title: &XmlNode) -> Option<Value> {
    let mut candidates = Vec::new();
    if let Some(rich) = title.first_descendant("rich") {
        if let Some(run) = rich.first_descendant("r") {
            candidates.push(run.direct_child("rPr"));
        }
        if let Some(p_pr) = rich.first_descendant("pPr") {
            candidates.push(p_pr.direct_child("defRPr"));
        }
    }
    if let Some(tx_pr) = title.direct_child("txPr")
        && let Some(p_pr) = tx_pr.first_descendant("pPr")
    {
        candidates.push(p_pr.direct_child("defRPr"));
    }
    for candidate in candidates.into_iter().flatten() {
        if let Some(font) = inspect_font(candidate) {
            return Some(font);
        }
    }
    None
}

fn inspect_axis_tick_label_font(axis: &XmlNode) -> Option<Value> {
    let tx_pr = axis.direct_child("txPr")?;
    let mut candidates = Vec::new();
    if let Some(p_pr) = tx_pr.first_descendant("pPr") {
        candidates.push(p_pr.direct_child("defRPr"));
    }
    if let Some(run) = tx_pr.first_descendant("r") {
        candidates.push(run.direct_child("rPr"));
    }
    for candidate in candidates.into_iter().flatten() {
        if let Some(font) = inspect_font(candidate) {
            return Some(font);
        }
    }
    None
}

fn inspect_font(r_pr: &XmlNode) -> Option<Value> {
    let mut font = Map::new();
    if let Some(size) = r_pr
        .attr("sz")
        .and_then(|value| value.trim().parse::<f64>().ok())
    {
        font.insert("sizePt".to_string(), json_style_number(size / 100.0));
    }
    if let Some(bold) = r_pr.attr("b") {
        font.insert("bold".to_string(), json!(parse_ooxml_bool(bold)));
    }
    if let Some(italic) = r_pr.attr("i") {
        font.insert("italic".to_string(), json!(parse_ooxml_bool(italic)));
    }
    if let Some(family) = r_pr
        .direct_child("latin")
        .and_then(|node| node.attr("typeface"))
        .filter(|value| !value.trim().is_empty())
    {
        font.insert("family".to_string(), json!(family.trim()));
    }
    let color = inspect_fill(Some(r_pr));
    if !color.is_empty() {
        font.insert("color".to_string(), json!(color));
    }
    (!font.is_empty()).then_some(Value::Object(font))
}

fn inspect_fill(holder: Option<&XmlNode>) -> String {
    let Some(holder) = holder else {
        return String::new();
    };
    let Some(solid) = holder.direct_child("solidFill") else {
        return String::new();
    };
    if let Some(srgb) = solid.direct_child("srgbClr")
        && let Some(value) = srgb.attr("val")
    {
        return value.trim().to_ascii_uppercase();
    }
    if let Some(scheme) = solid.direct_child("schemeClr")
        && let Some(value) = scheme.attr("val")
    {
        return format!("scheme:{}", value.trim());
    }
    String::new()
}

fn series_name_text(tx: &XmlNode) -> String {
    let mut parts = tx
        .descendants("v")
        .into_iter()
        .map(|node| node.text.clone())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        parts = tx
            .descendants("t")
            .into_iter()
            .map(|node| node.text.clone())
            .filter(|text| !text.is_empty())
            .collect();
    }
    parts.join("").trim().to_string()
}

fn axis_kind(element: &str) -> &'static str {
    match element {
        "valAx" => "value",
        "dateAx" => "date",
        "serAx" => "series",
        _ => "category",
    }
}

fn json_style_number(value: f64) -> Value {
    if value.is_finite() && value.fract() == 0.0 {
        json!(value as i64)
    } else {
        json!(value)
    }
}

fn parse_ooxml_bool_attr(node: &XmlNode) -> bool {
    node.attr("val").map(parse_ooxml_bool).unwrap_or(false)
}

fn parse_ooxml_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on"
    )
}

fn with_chart_selectors(mut chart: ChartRef) -> ChartRef {
    let mut selectors = Vec::new();
    chart.primary_selector = format!("chart:{}", chart.number);
    add_selector_ci(&mut selectors, chart.primary_selector.clone());
    add_selector_ci(&mut selectors, format!("chart:{}", chart.number));
    add_selector_ci(&mut selectors, format!("#{}", chart.number));
    if chart.slide > 0 {
        add_selector_ci(
            &mut selectors,
            format!("slide:{}/chart:{}", chart.slide, chart.number),
        );
    }
    if !chart.shape_id.is_empty() {
        add_selector_ci(&mut selectors, format!("shape:{}", chart.shape_id));
        add_selector_ci(&mut selectors, format!("id:{}", chart.shape_id));
    }
    if !chart.shape_name.is_empty() {
        add_selector_ci(&mut selectors, format!("shape:{}", chart.shape_name));
        add_selector_ci(&mut selectors, format!("name:{}", chart.shape_name));
        add_selector_ci(&mut selectors, format!("~{}", chart.shape_name));
        add_selector_ci(&mut selectors, chart.shape_name.clone());
    }
    if !chart.relationship_id.is_empty() {
        add_selector_ci(
            &mut selectors,
            format!("rid:{}", chart.relationship_id.clone()),
        );
        add_selector_ci(
            &mut selectors,
            format!("rId:{}", chart.relationship_id.clone()),
        );
    }
    if !chart.part_uri.is_empty() {
        add_selector_ci(&mut selectors, format!("part:{}", chart.part_uri));
    }
    chart.selectors = selectors;
    chart.series = chart
        .series
        .into_iter()
        .map(with_series_selectors)
        .collect();
    chart
}

fn with_series_selectors(mut series: SeriesRef) -> SeriesRef {
    let mut selectors = Vec::new();
    series.primary_selector = format!("series:{}", series.number);
    add_selector_ci(&mut selectors, series.primary_selector.clone());
    add_selector_ci(&mut selectors, format!("series:{}", series.number));
    add_selector_ci(&mut selectors, format!("#{}", series.number));
    let name = series_display_name(&series);
    if !name.is_empty() {
        add_selector_ci(&mut selectors, format!("name:{name}"));
        add_selector_ci(&mut selectors, format!("~{name}"));
        add_selector_ci(&mut selectors, name);
    }
    series.selectors = selectors;
    series
}

fn series_display_name(series: &SeriesRef) -> String {
    let Some(name) = series.name.as_ref() else {
        return String::new();
    };
    if let Some(first) = name.cache_preview.first() {
        return first.trim().to_string();
    }
    name.formula.trim().to_string()
}

fn add_selector_ci(selectors: &mut Vec<String>, value: String) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    if selectors
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(value))
    {
        return;
    }
    selectors.push(value.to_string());
}

fn select_chart(charts: &[ChartRef], selector: &str) -> CliResult<ChartRef> {
    if charts.is_empty() {
        return Err(CliError::target_not_found(
            "target not found: presentation has no charts; discover charts with `ooxml --json pptx charts list <file>`",
        ));
    }
    let selector = selector.trim();
    if selector.is_empty() {
        if charts.len() == 1 {
            return Ok(charts[0].clone());
        }
        return Err(CliError::invalid_args(
            "chart selector is required when presentation has multiple charts",
        ));
    }
    let mut matches = Vec::new();
    for chart in charts {
        if chart
            .selectors
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(selector))
        {
            matches.push(chart.clone());
        }
    }
    if matches.len() == 1 {
        return Ok(matches.remove(0));
    }
    if matches.len() > 1 {
        let mut selectors = matches
            .iter()
            .map(|chart| chart.primary_selector.clone())
            .collect::<Vec<_>>();
        selectors.sort();
        return Err(CliError::invalid_args(format!(
            "chart selector {selector:?} matched multiple charts ({}); use a more specific selector",
            selectors.join(", ")
        )));
    }
    if let Ok(number) = selector.parse::<usize>() {
        if (1..=charts.len()).contains(&number) {
            return Ok(charts[number - 1].clone());
        }
        return chart_target_not_found_with_candidates(
            charts,
            selector,
            &format!("chart {number} is out of range (1-{})", charts.len()),
        );
    }
    chart_target_not_found_with_candidates(
        charts,
        selector,
        &format!("chart not found: {selector}"),
    )
}

fn chart_target_not_found_with_candidates(
    charts: &[ChartRef],
    selector: &str,
    message: &str,
) -> CliResult<ChartRef> {
    let owned = charts
        .iter()
        .map(|chart| (chart.primary_selector.as_str(), chart.selectors.as_slice()))
        .collect::<Vec<_>>();
    let candidates = selector_candidates(&owned, selector, 5);
    let discovery = "ooxml --json pptx charts list <file>";
    if candidates.is_empty() {
        return Err(CliError::target_not_found(format!(
            "{message}; discover charts with `{discovery}`"
        )));
    }
    Err(CliError::target_not_found(format!(
        "{message}; did you mean: {}; discover with `{discovery}`",
        candidates.join(", ")
    )))
}

fn pptx_charts_result_json(file: &str, charts: &[ChartRef]) -> CliResult<Value> {
    let values = charts
        .iter()
        .map(|chart| chart_json(file, chart))
        .collect::<Vec<_>>();
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "charts": values,
    }))
}

fn chart_json(file: &str, chart: &ChartRef) -> Value {
    let mut item = Map::new();
    item.insert("number".to_string(), json!(chart.number));
    item.insert("slide".to_string(), json!(chart.slide));
    item.insert("slidePartUri".to_string(), json!(&chart.slide_part_uri));
    insert_non_empty(&mut item, "shapeId", &chart.shape_id);
    insert_non_empty(&mut item, "shapeName", &chart.shape_name);
    insert_non_empty(&mut item, "relationshipId", &chart.relationship_id);
    item.insert("partUri".to_string(), json!(&chart.part_uri));
    insert_non_empty(&mut item, "title", &chart.title);
    if !chart.types.is_empty() {
        item.insert("types".to_string(), json!(&chart.types));
    }
    if !chart.series.is_empty() {
        item.insert(
            "series".to_string(),
            Value::Array(chart.series.iter().map(series_json).collect()),
        );
    }
    insert_non_empty(
        &mut item,
        "embeddedWorkbookPartUri",
        &chart.embedded_workbook_part_uri,
    );
    insert_non_empty(
        &mut item,
        "embeddedWorkbookRelationshipId",
        &chart.embedded_workbook_relationship_id,
    );
    insert_non_empty(&mut item, "primarySelector", &chart.primary_selector);
    if !chart.selectors.is_empty() {
        item.insert("selectors".to_string(), json!(&chart.selectors));
    }
    item.insert(
        "showCommand".to_string(),
        json!(pptx_chart_show_command(
            file,
            chart.slide,
            &format!("part:{}", chart.part_uri)
        )),
    );
    if let Some(style) = chart.style.clone() {
        item.insert("style".to_string(), style);
    }
    Value::Object(item)
}

fn series_json(series: &SeriesRef) -> Value {
    let mut item = Map::new();
    item.insert("number".to_string(), json!(series.number));
    if series.index != 0 {
        item.insert("index".to_string(), json!(series.index));
    }
    if series.order != 0 {
        item.insert("order".to_string(), json!(series.order));
    }
    insert_data_source(&mut item, "name", series.name.as_ref());
    insert_data_source(&mut item, "categories", series.categories.as_ref());
    insert_data_source(&mut item, "values", series.values.as_ref());
    insert_data_source(&mut item, "xValues", series.x_values.as_ref());
    insert_data_source(&mut item, "yValues", series.y_values.as_ref());
    insert_data_source(&mut item, "bubbleSize", series.bubble_size.as_ref());
    insert_non_empty(&mut item, "primarySelector", &series.primary_selector);
    if !series.selectors.is_empty() {
        item.insert("selectors".to_string(), json!(&series.selectors));
    }
    Value::Object(item)
}

fn data_source_json(source: &DataSourceRef) -> Value {
    let mut item = Map::new();
    insert_non_empty(&mut item, "formula", &source.formula);
    insert_non_empty(&mut item, "sheet", &source.sheet);
    insert_non_empty(&mut item, "range", &source.range);
    insert_non_empty(&mut item, "refKind", &source.ref_kind);
    insert_non_empty(&mut item, "cacheType", &source.cache_type);
    if source.point_count != 0 {
        item.insert("pointCount".to_string(), json!(source.point_count));
    }
    if !source.cache_preview.is_empty() {
        item.insert("cachePreview".to_string(), json!(&source.cache_preview));
    }
    Value::Object(item)
}

fn insert_data_source(map: &mut Map<String, Value>, key: &str, source: Option<&DataSourceRef>) {
    if let Some(source) = source {
        map.insert(key.to_string(), data_source_json(source));
    }
}

fn insert_non_empty(map: &mut Map<String, Value>, key: &str, value: &str) {
    if !value.is_empty() {
        map.insert(key.to_string(), json!(value));
    }
}

fn pptx_chart_show_command(file: &str, slide: u32, chart_selector: &str) -> String {
    let mut command = format!("ooxml --json pptx charts show {}", command_arg(file));
    if slide > 0 {
        command.push_str(&format!(" --slide {slide}"));
    }
    if !chart_selector.trim().is_empty() {
        command.push_str(&format!(" --chart {}", command_arg(chart_selector)));
    }
    command
}

fn parse_xml_tree(xml: &str) -> CliResult<XmlNode> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => stack.push(node_from_start(&e)),
            Ok(Event::Empty(e)) => {
                let node = node_from_start(&e);
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root = Some(node);
                }
            }
            Ok(event) if is_xml_text_event(&event) => {
                if let Some(current) = stack.last_mut() {
                    append_xml_text_event(&mut current.text, &event);
                }
            }
            Ok(Event::End(_)) => {
                let Some(node) = stack.pop() else {
                    continue;
                };
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root = Some(node);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    root.ok_or_else(|| CliError::unexpected("XML root element not found"))
}

fn node_from_start(e: &BytesStart<'_>) -> XmlNode {
    XmlNode {
        name: local_name(e.name().as_ref()).to_string(),
        attrs: xml_attrs_map(e),
        text: String::new(),
        children: Vec::new(),
    }
}

impl XmlNode {
    fn attr_exact(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(String::as_str)
    }

    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(String::as_str).or_else(|| {
            self.attrs.iter().find_map(|(candidate, value)| {
                (local_name(candidate.as_bytes()) == key).then_some(value.as_str())
            })
        })
    }

    fn direct_child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|child| child.name == name)
    }

    fn direct_children(&self, name: &str) -> Vec<&XmlNode> {
        self.children
            .iter()
            .filter(|child| child.name == name)
            .collect()
    }

    fn first_descendant(&self, name: &str) -> Option<&XmlNode> {
        if self.name == name {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.first_descendant(name) {
                return Some(found);
            }
        }
        None
    }

    fn descendants(&self, name: &str) -> Vec<&XmlNode> {
        let mut result = Vec::new();
        self.collect_descendants(name, &mut result);
        result
    }

    fn collect_descendants<'a>(&'a self, name: &str, result: &mut Vec<&'a XmlNode>) {
        if self.name == name {
            result.push(self);
        }
        for child in &self.children {
            child.collect_descendants(name, result);
        }
    }
}
