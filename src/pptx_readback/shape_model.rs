use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::{add_selector, attr, decode_xml_text, local_name, xml_general_ref};
#[derive(Default)]
pub(super) struct Shape {
    pub(super) id: u32,
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) is_placeholder: bool,
    pub(super) has_text_body: bool,
    pub(super) text: String,
    pub(super) paragraphs: Vec<Vec<String>>,
    pub(super) bounds: Option<Bounds>,
    pub(super) placeholder: Option<Placeholder>,
    pub(super) image_rel_id: String,
    pub(super) table: Option<TableInfo>,
}

#[derive(Clone)]
pub(super) struct Placeholder {
    pub(super) literal_type: String,
    pub(super) index: Option<u32>,
}

#[derive(Clone)]
pub(super) struct Bounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

#[derive(Default)]
pub(super) struct TableInfo {
    pub(super) columns: Vec<i64>,
    pub(super) rows: Vec<TableRow>,
}

#[derive(Default)]
pub(super) struct TableRow {
    pub(super) height: Option<i64>,
    pub(super) cells: Vec<TableCell>,
}

#[derive(Clone)]
pub(super) struct TableCell {
    pub(super) text: String,
    pub(super) grid_span: u32,
    pub(super) row_span: u32,
}

impl Default for TableCell {
    fn default() -> Self {
        Self {
            text: String::new(),
            grid_span: 1,
            row_span: 1,
        }
    }
}

pub(super) fn pptx_slide_object_counts(xml: &str) -> (usize, usize, usize) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut text_shapes = 0;
    let mut images = 0;
    let mut tables = 0;
    let mut path = Vec::<String>::new();
    let mut current_shape: Option<(String, usize, bool, bool)> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current_shape.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && matches!(name.as_str(), "sp" | "pic" | "graphicFrame")
                {
                    current_shape = Some((name.clone(), path.len() + 1, false, false));
                } else if let Some((kind, _, has_text, has_table)) = current_shape.as_mut() {
                    if kind == "sp" && name == "txBody" {
                        *has_text = true;
                    }
                    if kind == "graphicFrame" && name == "tbl" {
                        *has_table = true;
                    }
                }
                path.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current_shape.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && name == "pic"
                {
                    images += 1;
                } else if let Some((kind, _, has_text, has_table)) = current_shape.as_mut() {
                    if kind == "sp" && name == "txBody" {
                        *has_text = true;
                    }
                    if kind == "graphicFrame" && name == "tbl" {
                        *has_table = true;
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some((kind, depth, has_text, has_table)) = current_shape.take() {
                    if path.len() == depth && name == kind {
                        match kind.as_str() {
                            "sp" if has_text => text_shapes += 1,
                            "pic" => images += 1,
                            "graphicFrame" if has_table => tables += 1,
                            _ => {}
                        }
                    } else {
                        current_shape = Some((kind, depth, has_text, has_table));
                    }
                }
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    (text_shapes, images, tables)
}

pub(super) fn pptx_selector_targets(xml: &str) -> Vec<Value> {
    let shapes = pptx_shape_models(xml);
    pptx_selector_targets_from_shapes(&shapes)
}

pub(super) fn pptx_selector_targets_from_shapes(shapes: &[Shape]) -> Vec<Value> {
    let mut name_counts = BTreeMap::<String, usize>::new();
    let mut index_counts = BTreeMap::<u32, usize>::new();
    for shape in shapes {
        if !shape.name.trim().is_empty() {
            *name_counts.entry(shape.name.clone()).or_default() += 1;
        }
        if let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index) {
            *index_counts.entry(index).or_default() += 1;
        }
    }

    let mut table_index = 0_u32;
    shapes
        .iter()
        .enumerate()
        .map(|(index, shape)| {
            let is_table = shape.kind == "graphicFrame" && shape.table.is_some();
            if is_table {
                table_index += 1;
            }
            let mut placeholder = shape
                .placeholder
                .as_ref()
                .and_then(pptx_selector_placeholder);
            if placeholder.is_none()
                && shape.kind == "sp"
                && shape.has_text_body
                && shape
                    .name
                    .to_ascii_lowercase()
                    .contains("content placeholder")
            {
                let mut inferred = Map::new();
                inferred.insert("key".to_string(), json!("body"));
                inferred.insert("role".to_string(), json!("body"));
                if let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index) {
                    inferred.insert("index".to_string(), json!(index));
                }
                inferred.insert("resolvedType".to_string(), json!("body"));
                inferred.insert("typeSource".to_string(), json!("master"));
                placeholder = Some(inferred);
            }
            let placeholder_key = placeholder
                .as_ref()
                .and_then(|placeholder| placeholder.get("key"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let placeholder_role = placeholder
                .as_ref()
                .and_then(|placeholder| placeholder.get("role"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let mut primary_selector = format!("shape:{}", shape.id);
            if is_table {
                primary_selector = format!("table:{table_index}");
            } else if !placeholder_key.is_empty() {
                primary_selector.clone_from(&placeholder_key);
            }
            let mut selectors = Vec::<String>::new();
            if is_table {
                add_selector(&mut selectors, format!("shape:{}", shape.id));
                add_selector(&mut selectors, format!("table:{table_index}"));
            } else {
                add_selector(&mut selectors, placeholder_key.clone());
                if !placeholder_role.is_empty() {
                    add_selector(&mut selectors, format!("@{placeholder_role}"));
                    add_selector(&mut selectors, placeholder_role.clone());
                    if let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index) {
                        add_selector(&mut selectors, format!("{placeholder_role}:{index}"));
                    }
                }
                if let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index)
                    && index_counts.get(&index).copied().unwrap_or_default() == 1
                {
                    add_selector(&mut selectors, format!("#{index}"));
                }
                add_selector(&mut selectors, format!("shape:{}", shape.id));
            }
            if name_counts.get(&shape.name).copied().unwrap_or_default() == 1 {
                add_selector(&mut selectors, format!("~{}", shape.name));
            }

            let text_preview = normalized_text_preview(&shape.text);
            let mut target = Map::new();
            target.insert("order".to_string(), json!(index + 1));
            target.insert("shapeId".to_string(), json!(shape.id));
            if !shape.name.is_empty() {
                target.insert("shapeName".to_string(), json!(shape.name));
            }
            target.insert("shapeType".to_string(), json!(shape.kind));
            target.insert(
                "targetKind".to_string(),
                json!(if is_table {
                    "table".to_string()
                } else if shape.kind == "pic" {
                    "picture".to_string()
                } else if !placeholder_role.is_empty() {
                    placeholder_role
                } else if shape.has_text_body {
                    "textbox".to_string()
                } else if shape.is_placeholder {
                    "placeholder".to_string()
                } else {
                    "shape".to_string()
                }),
            );
            target.insert(
                "textCapable".to_string(),
                json!(shape.kind == "sp" && shape.has_text_body),
            );
            if !text_preview.is_empty() {
                target.insert("textPreview".to_string(), json!(text_preview));
            }
            target.insert("primarySelector".to_string(), json!(primary_selector));
            target.insert("selectors".to_string(), json!(selectors));
            if let Some(placeholder) = placeholder {
                target.insert("placeholder".to_string(), Value::Object(placeholder));
            }
            Value::Object(target)
        })
        .collect()
}

pub(super) fn bounds_json(bounds: &Bounds) -> Value {
    json!({
        "x": bounds.x,
        "y": bounds.y,
        "cx": bounds.cx,
        "cy": bounds.cy,
    })
}

fn pptx_selector_placeholder(ph: &Placeholder) -> Option<Map<String, Value>> {
    let role = placeholder_role(&ph.literal_type);
    if role.is_empty() {
        return None;
    }
    let key = role.clone();
    let mut placeholder = Map::new();
    placeholder.insert("key".to_string(), json!(key));
    placeholder.insert("role".to_string(), json!(role));
    if let Some(index) = ph.index {
        placeholder.insert("index".to_string(), json!(index));
    }
    if !ph.literal_type.is_empty() {
        placeholder.insert("literalType".to_string(), json!(ph.literal_type));
        placeholder.insert("resolvedType".to_string(), json!(ph.literal_type));
        placeholder.insert("typeSource".to_string(), json!("slide"));
    }
    Some(placeholder)
}

fn placeholder_role(literal_type: &str) -> String {
    match literal_type {
        "ctrTitle" | "title" => "title",
        "subTitle" => "subtitle",
        "body" | "obj" => "body",
        "pic" => "picture",
        other => other,
    }
    .to_string()
}

fn normalized_text_preview(text: &str) -> String {
    let preview = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if preview.len() > 140 {
        format!("{}...", &preview[..137])
    } else {
        preview
    }
}

pub(super) fn pptx_shape_models(xml: &str) -> Vec<Shape> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut shapes = Vec::new();
    let mut current: Option<Shape> = None;
    let mut current_end = String::new();
    let mut current_depth = 0_usize;
    let mut depth = 0_usize;
    let mut sp_tree_depth = None::<usize>;
    let mut in_text = false;
    let mut in_shape_text_body = false;
    let mut in_table = false;
    let mut current_row: Option<TableRow> = None;
    let mut current_cell: Option<TableCell> = None;
    let mut current_paragraph: Option<Vec<String>> = None;
    loop {
        let event = reader.read_event();
        let start_name = match &event {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                Some(local_name(e.name().as_ref()).to_string())
            }
            _ => None,
        };
        let end_name = match &event {
            Ok(Event::End(e)) => Some(local_name(e.name().as_ref()).to_string()),
            _ => None,
        };
        let event_depth = if matches!(event, Ok(Event::Start(_)) | Ok(Event::Empty(_))) {
            depth + 1
        } else {
            depth
        };
        if matches!(event, Ok(Event::Start(_))) && start_name.as_deref() == Some("spTree") {
            sp_tree_depth.get_or_insert(event_depth);
        }
        let in_sp_tree = sp_tree_depth.is_some_and(|sp_depth| event_depth > sp_depth);
        let is_start = matches!(event, Ok(Event::Start(_)));
        let is_end = matches!(event, Ok(Event::End(_)));
        let closes_sp_tree = is_end && sp_tree_depth == Some(depth);

        match event {
            Ok(Event::Start(_))
                if current.is_none()
                    && in_sp_tree
                    && matches!(start_name.as_deref(), Some("sp" | "pic" | "graphicFrame")) =>
            {
                let kind = start_name.clone().unwrap_or_default();
                current_end.clone_from(&kind);
                current_depth = event_depth;
                current = Some(Shape {
                    kind,
                    ..Shape::default()
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && start_name.as_deref() == Some("cNvPr") =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.id = attr(&e, "id")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_default();
                    shape.name = attr(&e, "name").unwrap_or_default();
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && start_name.as_deref() == Some("ph") =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.is_placeholder = true;
                    shape.placeholder = Some(Placeholder {
                        literal_type: attr(&e, "type").unwrap_or_default(),
                        index: attr(&e, "idx").and_then(|idx| idx.parse().ok()),
                    });
                }
            }
            Ok(Event::Start(_))
                if current.as_ref().is_some_and(|shape| shape.kind == "sp")
                    && start_name.as_deref() == Some("txBody") =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.has_text_body = true;
                }
                in_shape_text_body = true;
            }
            Ok(Event::Empty(_))
                if current.as_ref().is_some_and(|shape| shape.kind == "sp")
                    && start_name.as_deref() == Some("txBody") =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.has_text_body = true;
                }
            }
            Ok(Event::Start(_)) if in_shape_text_body && start_name.as_deref() == Some("p") => {
                current_paragraph = Some(Vec::new());
            }
            Ok(Event::Empty(_)) if in_shape_text_body && start_name.as_deref() == Some("p") => {
                if let Some(shape) = current.as_mut() {
                    shape.paragraphs.push(Vec::new());
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && start_name.as_deref() == Some("off") =>
            {
                if let Some(shape) = current.as_mut() {
                    let mut bounds = shape.bounds.clone().unwrap_or(Bounds {
                        x: 0,
                        y: 0,
                        cx: 0,
                        cy: 0,
                    });
                    bounds.x = attr(&e, "x")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.x);
                    bounds.y = attr(&e, "y")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.y);
                    shape.bounds = Some(bounds);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && start_name.as_deref() == Some("ext") =>
            {
                if let Some(shape) = current.as_mut() {
                    let mut bounds = shape.bounds.clone().unwrap_or(Bounds {
                        x: 0,
                        y: 0,
                        cx: 0,
                        cy: 0,
                    });
                    bounds.cx = attr(&e, "cx")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.cx);
                    bounds.cy = attr(&e, "cy")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.cy);
                    shape.bounds = Some(bounds);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.as_ref().is_some_and(|shape| shape.kind == "pic")
                    && start_name.as_deref() == Some("blip") =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.image_rel_id = attr(&e, "embed").unwrap_or_default();
                }
            }
            Ok(Event::Start(_)) if current.is_some() && start_name.as_deref() == Some("tbl") => {
                in_table = true;
                if let Some(shape) = current.as_mut() {
                    shape.table = Some(TableInfo::default());
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_table && start_name.as_deref() == Some("gridCol") =>
            {
                if let Some(table) = current.as_mut().and_then(|shape| shape.table.as_mut())
                    && let Some(width) = attr(&e, "w").and_then(|value| value.parse().ok())
                {
                    table.columns.push(width);
                }
            }
            Ok(Event::Start(e)) if in_table && start_name.as_deref() == Some("tr") => {
                current_row = Some(TableRow {
                    height: attr(&e, "h").and_then(|value| value.parse().ok()),
                    cells: Vec::new(),
                });
            }
            Ok(Event::Start(e)) if in_table && start_name.as_deref() == Some("tc") => {
                current_cell = Some(TableCell {
                    grid_span: attr(&e, "gridSpan")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1),
                    row_span: attr(&e, "rowSpan")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1),
                    ..TableCell::default()
                });
            }
            Ok(Event::Start(_)) if current.is_some() && start_name.as_deref() == Some("t") => {
                in_text = true;
            }
            Ok(Event::Text(e)) if in_text => {
                let text = decode_xml_text(e.as_ref());
                push_pptx_text(
                    &mut current,
                    &mut current_cell,
                    &mut current_paragraph,
                    in_shape_text_body,
                    text,
                );
            }
            Ok(Event::GeneralRef(e)) if in_text => {
                let text = xml_general_ref(e.as_ref());
                push_pptx_text(
                    &mut current,
                    &mut current_cell,
                    &mut current_paragraph,
                    in_shape_text_body,
                    text,
                );
            }
            Ok(Event::End(_)) if end_name.as_deref() == Some("t") => {
                in_text = false;
            }
            Ok(Event::End(_)) if in_shape_text_body && end_name.as_deref() == Some("p") => {
                if let Some(paragraph) = current_paragraph.take()
                    && let Some(shape) = current.as_mut()
                {
                    shape.paragraphs.push(paragraph);
                }
            }
            Ok(Event::End(_)) if in_shape_text_body && end_name.as_deref() == Some("txBody") => {
                in_shape_text_body = false;
            }
            Ok(Event::End(_)) if in_table && end_name.as_deref() == Some("tc") => {
                if let Some(cell) = current_cell.take()
                    && let Some(row) = current_row.as_mut()
                {
                    row.cells.push(cell);
                }
            }
            Ok(Event::End(_)) if in_table && end_name.as_deref() == Some("tr") => {
                if let Some(row) = current_row.take()
                    && let Some(table) = current.as_mut().and_then(|shape| shape.table.as_mut())
                {
                    table.rows.push(row);
                }
            }
            Ok(Event::End(_)) if in_table && end_name.as_deref() == Some("tbl") => {
                in_table = false;
            }
            Ok(Event::End(_))
                if current.is_some()
                    && event_depth == current_depth
                    && end_name.as_deref() == Some(current_end.as_str()) =>
            {
                if let Some(shape) = current.take() {
                    shapes.push(shape);
                }
                current_end.clear();
                current_depth = 0;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        if is_start {
            depth += 1;
        } else if is_end {
            if closes_sp_tree {
                sp_tree_depth = None;
            }
            depth = depth.saturating_sub(1);
        }
    }
    shapes
}

fn push_pptx_text(
    current: &mut Option<Shape>,
    current_cell: &mut Option<TableCell>,
    current_paragraph: &mut Option<Vec<String>>,
    in_shape_text_body: bool,
    text: String,
) {
    if let Some(cell) = current_cell.as_mut() {
        cell.text.push_str(&text);
    } else if let Some(shape) = current.as_mut()
        && shape.kind == "sp"
    {
        shape.text.push_str(&text);
        if in_shape_text_body && let Some(paragraph) = current_paragraph.as_mut() {
            paragraph.push(text);
        }
    }
}
