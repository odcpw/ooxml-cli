mod comments;
mod layouts;
mod notes;
mod slide_parts;
mod tables;
mod text;

pub(crate) use comments::pptx_comments_list;
pub(crate) use layouts::{
    pptx_layouts_list, pptx_layouts_show, pptx_masters_list, pptx_masters_show,
};
pub(crate) use notes::{pptx_extract_notes, pptx_notes_show};
pub(crate) use tables::pptx_tables_show;
use tables::table_info_json;
pub(crate) use text::{pptx_extract_text, pptx_extract_text_json_args};
use text::{pptx_slide_texts, pptx_text_block_from_paragraphs};

use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

use crate::{
    CliError, CliResult, add_selector, attr, attr_exact, content_type_for_part, local_name,
    relationship_entries, relationships, zip_text,
};

pub(crate) fn pptx_slide_show(file: &str, slide: u32) -> CliResult<Value> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    if slide == 0 || slide as usize > slides.len() {
        return Err(CliError::invalid_args(format!(
            "slide number {slide} is out of range (1-{})",
            slides.len()
        )));
    }

    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let (slide_id, rel_id) = &slides[slide as usize - 1];
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    let part = normalize_ppt_target(target);
    let slide_xml = zip_text(file, &part)?;
    let layout_part = slide_layout_part(file, &part)?;
    let layout_name = layout_part
        .as_ref()
        .and_then(|part| zip_text(file, part).ok())
        .and_then(|xml| layout_display_name(&xml))
        .unwrap_or_else(|| "Title Slide".to_string());
    let layout_number = layout_part
        .as_ref()
        .and_then(|part| trailing_number(part, "slideLayout"))
        .unwrap_or(1);
    let shapes = pptx_shapes(&slide_xml);
    let part_uri = format!("/{}", part);
    let layout_part_uri = layout_part
        .as_ref()
        .map(|part| format!("/{part}"))
        .unwrap_or_else(|| "/ppt/slideLayouts/slideLayout1.xml".to_string());

    Ok(json!({
        "file": file,
        "slides": [{
            "id": format!("slide{slide}"),
            "layoutNumber": layout_number,
            "layoutPartUri": layout_part_uri,
            "layoutReadbackCommand": format!("ooxml --json pptx layouts show {file} --layout {layout_number}"),
            "layoutRef": layout_name,
            "partUri": part_uri,
            "primarySelector": slide.to_string(),
            "readbackCommand": format!("ooxml --json pptx slides show {file} --slide {slide} --include-text --include-bounds"),
            "relationshipId": rel_id,
            "selectors": [
                slide.to_string(),
                format!("part:/{}", part),
                format!("slideId:{slide_id}"),
                format!("rId:{rel_id}"),
            ],
            "selectorsCommand": format!("ooxml --json pptx slides selectors {file} --slide {slide}"),
            "shapes": shapes,
            "shapesCommand": format!("ooxml --json pptx shapes show {file} --slide {slide} --include-text --include-bounds"),
            "slide": slide,
            "slideId": slide_id,
        }],
    }))
}

pub(crate) fn pptx_slides_list(file: &str) -> CliResult<Value> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let mut slide_id_counts = BTreeMap::<u32, usize>::new();
    for (slide_id, _) in &slides {
        if *slide_id != 0 {
            *slide_id_counts.entry(*slide_id).or_default() += 1;
        }
    }
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let values = slides
        .iter()
        .enumerate()
        .map(|(index, (slide_id, rel_id))| {
            let slide_number = index as u32 + 1;
            let target = rels
                .get(rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            let part = normalize_ppt_target(target);
            let slide_xml = zip_text(file, &part)?;
            let (layout_part, notes_part) = slide_layout_and_notes_parts(file, &part)?;
            let layout_xml = layout_part.as_ref().and_then(|part| zip_text(file, part).ok());
            let layout_name = layout_xml
                .as_deref()
                .and_then(layout_display_name)
                .unwrap_or_default();
            let layout_number = layout_xml
                .as_ref()
                .and(layout_part.as_ref())
                .and_then(|part| trailing_number(part, "slideLayout"))
                .unwrap_or(0);
            let (text_shapes, images, tables) = pptx_slide_object_counts(&slide_xml);
            let part_uri = format!("/{part}");
            let layout_part_uri = layout_xml
                .as_ref()
                .and(layout_part.as_ref())
                .map(|part| format!("/{part}"));
            let notes_part_uri = notes_part.as_ref().map(|part| format!("/{part}"));
            let selectors = vec![
                slide_number.to_string(),
                format!("part:{part_uri}"),
                format!("slideId:{slide_id}"),
                format!("rId:{rel_id}"),
            ];
            let mut item = Map::new();
            item.insert("number".to_string(), json!(slide_number));
            item.insert("slideId".to_string(), json!(slide_id));
            item.insert("relationshipId".to_string(), json!(rel_id));
            item.insert("partUri".to_string(), json!(part_uri));
            item.insert("primarySelector".to_string(), json!(slide_number.to_string()));
            if *slide_id != 0 && slide_id_counts.get(slide_id).copied().unwrap_or_default() == 1 {
                item.insert("handle".to_string(), json!(format!("H:pptx/s:{slide_id}")));
            }
            item.insert("selectors".to_string(), json!(selectors));
            item.insert("layout".to_string(), json!(layout_name));
            if layout_number > 0 {
                item.insert("layoutNumber".to_string(), json!(layout_number));
            }
            if let Some(layout_part_uri) = layout_part_uri {
                item.insert("layoutPartUri".to_string(), json!(layout_part_uri));
            }
            if let Some(notes_part_uri) = notes_part_uri {
                item.insert("notesPartUri".to_string(), json!(notes_part_uri));
            }
            item.insert("textShapes".to_string(), json!(text_shapes));
            item.insert("images".to_string(), json!(images));
            item.insert("tables".to_string(), json!(tables));
            item.insert("notes".to_string(), json!(notes_part.is_some()));
            item.insert(
                "readbackCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx slides show {file} --slide {slide_number} --include-text --include-bounds"
                )),
            );
            item.insert(
                "selectorsCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx slides selectors {file} --slide {slide_number}"
                )),
            );
            item.insert(
                "shapesCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx shapes show {file} --slide {slide_number} --include-text --include-bounds"
                )),
            );
            if tables > 0 {
                item.insert(
                    "tablesCommand".to_string(),
                    json!(format!(
                        "ooxml --json pptx tables show {file} --slide {slide_number}"
                    )),
                );
            }
            if layout_number > 0 {
                item.insert(
                    "layoutReadbackCommand".to_string(),
                    json!(format!(
                        "ooxml --json pptx layouts show {file} --layout {layout_number}"
                    )),
                );
            }
            Ok(Value::Object(item))
        })
        .collect::<CliResult<Vec<_>>>()?;
    Ok(json!({"file": file, "slides": values}))
}

pub(crate) fn pptx_slide_selectors(file: &str, slide: u32) -> CliResult<Value> {
    if slide == 0 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let index = slide as usize - 1;
    let (_, rel_id) = slides
        .get(index)
        .ok_or_else(|| CliError::unexpected(format!("slide {slide} not found")))?;
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    let part = normalize_ppt_target(target);
    let slide_xml = zip_text(file, &part)?;
    let (layout_part, _) = slide_layout_and_notes_parts(file, &part)?;
    let layout_xml = layout_part
        .as_ref()
        .and_then(|part| zip_text(file, part).ok());
    let layout_name = layout_xml.as_deref().and_then(layout_display_name);
    let layout_part_uri = layout_xml
        .as_ref()
        .and(layout_part.as_ref())
        .map(|part| format!("/{part}"));

    let mut output = Map::new();
    output.insert("file".to_string(), json!(file));
    output.insert("slide".to_string(), json!(slide));
    output.insert("partUri".to_string(), json!(format!("/{part}")));
    if let Some(layout_name) = layout_name.filter(|name| !name.is_empty()) {
        output.insert("layoutName".to_string(), json!(layout_name));
    }
    if let Some(layout_part_uri) = layout_part_uri {
        output.insert("layoutPartUri".to_string(), json!(layout_part_uri));
    }
    output.insert(
        "targets".to_string(),
        Value::Array(pptx_selector_targets(&slide_xml)),
    );
    Ok(Value::Object(output))
}

pub(crate) fn pptx_shapes_show(
    file: &str,
    slide: u32,
    include_text: bool,
    include_bounds: bool,
) -> CliResult<Value> {
    if slide == 0 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let mut slide_id_counts = BTreeMap::<u32, usize>::new();
    for (slide_id, _) in &slides {
        if *slide_id != 0 {
            *slide_id_counts.entry(*slide_id).or_default() += 1;
        }
    }
    let index = slide as usize - 1;
    let (slide_id, rel_id) = slides.get(index).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide {slide} not found (presentation has {} slides)",
            slides.len()
        ))
    })?;
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    let part = normalize_ppt_target(target);
    let slide_xml = zip_text(file, &part)?;
    let (layout_part, _) = slide_layout_and_notes_parts(file, &part)?;
    let layout_xml = layout_part
        .as_ref()
        .and_then(|part| zip_text(file, part).ok());
    let layout_name = layout_xml.as_deref().and_then(layout_display_name);
    let layout_part_uri = layout_xml
        .as_ref()
        .and(layout_part.as_ref())
        .map(|part| format!("/{part}"));
    let slide_id_unique =
        *slide_id != 0 && slide_id_counts.get(slide_id).copied().unwrap_or_default() == 1;

    let mut output = Map::new();
    output.insert("file".to_string(), json!(file));
    output.insert("slide".to_string(), json!(slide));
    output.insert("partUri".to_string(), json!(format!("/{part}")));
    if let Some(layout_name) = layout_name.filter(|name| !name.is_empty()) {
        output.insert("layoutName".to_string(), json!(layout_name));
    }
    if let Some(layout_part_uri) = layout_part_uri {
        output.insert("layoutPartUri".to_string(), json!(layout_part_uri));
    }
    output.insert(
        "shapes".to_string(),
        Value::Array(pptx_shape_show_entries(
            file,
            &part,
            &slide_xml,
            *slide_id,
            slide_id_unique,
            include_text,
            include_bounds,
        )),
    );
    Ok(Value::Object(output))
}

pub(crate) fn pptx_all_slides(file: &str) -> Vec<u32> {
    zip_text(file, "ppt/presentation.xml")
        .map(|xml| (1..=pptx_slide_refs(&xml).len() as u32).collect())
        .unwrap_or_else(|_| vec![1])
}

pub(crate) fn pptx_diff(baseline: &str, file: &str) -> CliResult<Value> {
    let before = pptx_slide_texts(baseline)?;
    let after = pptx_slide_texts(file)?;
    let slide_count_a = before.len();
    let slide_count_b = after.len();
    let mut changed_slides = Vec::new();
    let mut text_diffs = Vec::new();
    for slide_idx in 0..slide_count_a.max(slide_count_b) {
        let before_shapes = before.get(slide_idx).cloned().unwrap_or_default();
        let after_shapes = after.get(slide_idx).cloned().unwrap_or_default();
        let mut changed = false;
        for before_shape in before_shapes {
            let Some(after_shape) = after_shapes
                .iter()
                .find(|candidate| candidate.key == before_shape.key)
            else {
                continue;
            };
            if before_shape.text != after_shape.text {
                changed = true;
                text_diffs.push(json!({
                    "after": after_shape.text,
                    "before": before_shape.text,
                    "shapeKey": before_shape.key,
                    "shapeName": before_shape.name,
                    "slide": slide_idx + 1,
                }));
            }
        }
        if changed {
            changed_slides.push(Value::from(slide_idx + 1));
        }
    }
    Ok(json!({
        "schemaVersion": "1.0",
        "semantic": {
            "changedSlides": changed_slides,
            "imageDiffs": [],
            "layoutDiffs": [],
            "slideCountA": slide_count_a,
            "slideCountB": slide_count_b,
            "slideCountEqual": slide_count_a == slide_count_b,
            "textDiffs": text_diffs,
        },
        "type": "pptx",
        "visual": {
            "enabled": false,
            "status": "disabled",
        },
    }))
}

fn pptx_slide_refs(xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let (Some(id), Some(rel)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    slides.push((id, rel));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

fn slide_part_relationships(file: &str, slide_part: &str) -> CliResult<BTreeMap<String, String>> {
    let name = Path::new(slide_part)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::unexpected(format!("invalid slide part {slide_part}")))?;
    relationships(file, &format!("ppt/slides/_rels/{name}.rels"))
}

fn normalize_ppt_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("ppt/") {
        target.to_string()
    } else {
        format!("ppt/{}", target.trim_start_matches("../"))
    }
}

fn slide_layout_part(file: &str, slide_part: &str) -> CliResult<Option<String>> {
    slide_layout_and_notes_parts(file, slide_part).map(|(layout, _)| layout)
}

fn slide_layout_and_notes_parts(
    file: &str,
    slide_part: &str,
) -> CliResult<(Option<String>, Option<String>)> {
    let name = Path::new(slide_part)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::unexpected(format!("invalid slide part {slide_part}")))?;
    let rels_part = format!("ppt/slides/_rels/{name}.rels");
    let rels = relationship_entries(file, &rels_part)?;
    let mut layout_part = None;
    let mut notes_part = None;
    for rel in rels {
        match rel.rel_type.as_str() {
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" => {
                layout_part = Some(normalize_ppt_target(&rel.target));
            }
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" => {
                notes_part = Some(normalize_ppt_target(&rel.target));
            }
            _ => {}
        }
    }
    Ok((layout_part, notes_part))
}

fn layout_display_name(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cSld" =>
            {
                return attr(&e, "name");
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn trailing_number(path: &str, stem: &str) -> Option<u32> {
    let file_name = Path::new(path).file_stem()?.to_str()?;
    file_name.strip_prefix(stem)?.parse::<u32>().ok()
}

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
    literal_type: String,
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

fn pptx_shapes(xml: &str) -> Vec<Value> {
    pptx_shape_models(xml)
        .into_iter()
        .map(|shape| {
            let mut map = Map::new();
            map.insert("id".to_string(), json!(shape.id));
            map.insert("shapeName".to_string(), json!(shape.name));
            map.insert("type".to_string(), json!(shape.kind));
            if let Some(bounds) = shape.bounds.as_ref() {
                map.insert("bounds".to_string(), bounds_json(bounds));
            }
            map.insert("isPlaceholder".to_string(), json!(shape.is_placeholder));
            if !shape.text.is_empty() {
                map.insert("textContent".to_string(), json!(shape.text));
            }
            if let Some(table) = shape.table.as_ref() {
                map.insert("tableInfo".to_string(), table_info_json(table));
            }
            if !shape.image_rel_id.is_empty() {
                map.insert(
                    "imageRef".to_string(),
                    image_ref_json(&shape.image_rel_id, "", ""),
                );
            }
            Value::Object(map)
        })
        .collect()
}

fn pptx_slide_object_counts(xml: &str) -> (usize, usize, usize) {
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

fn pptx_selector_targets(xml: &str) -> Vec<Value> {
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
            let placeholder = shape
                .placeholder
                .as_ref()
                .and_then(pptx_selector_placeholder);
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

fn pptx_shape_show_entries(
    file: &str,
    slide_part: &str,
    xml: &str,
    slide_id: u32,
    slide_id_unique: bool,
    include_text: bool,
    include_bounds: bool,
) -> Vec<Value> {
    let shapes = pptx_shape_models(xml);
    let mut id_counts = BTreeMap::<u32, usize>::new();
    for shape in &shapes {
        if shape.id != 0 {
            *id_counts.entry(shape.id).or_default() += 1;
        }
    }
    let targets = pptx_selector_targets_from_shapes(&shapes);
    let slide_relationships = slide_part_relationships(file, slide_part).unwrap_or_default();
    shapes
        .iter()
        .zip(targets)
        .map(|(shape, target)| {
            let mut entry = target.as_object().cloned().unwrap_or_default();
            if slide_id_unique && id_counts.get(&shape.id).copied().unwrap_or_default() == 1 {
                entry.insert(
                    "handle".to_string(),
                    json!(format!("H:pptx/s:{slide_id}/shape:n:{}", shape.id)),
                );
            }
            if !include_text {
                entry.remove("textPreview");
            }
            if include_bounds && let Some(bounds) = shape.bounds.as_ref() {
                entry.insert("bounds".to_string(), bounds_json(bounds));
            }
            if let Some(table) = shape.table.as_ref() {
                entry.insert("tableInfo".to_string(), table_info_json(table));
            }
            if !shape.image_rel_id.is_empty() {
                let target_uri = slide_relationships
                    .get(&shape.image_rel_id)
                    .map(|target| format!("/{}", normalize_ppt_target(target)))
                    .unwrap_or_default();
                let content_type = if target_uri.is_empty() {
                    String::new()
                } else {
                    content_type_for_part(file, &target_uri).unwrap_or_default()
                };
                entry.insert(
                    "imageRef".to_string(),
                    image_ref_json(&shape.image_rel_id, &target_uri, &content_type),
                );
            }
            Value::Object(entry)
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

fn image_ref_json(rel_id: &str, target_uri: &str, content_type: &str) -> Value {
    json!({
        "relId": rel_id,
        "targetUri": target_uri,
        "contentType": content_type,
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
    reader.config_mut().trim_text(true);
    let mut shapes = Vec::new();
    let mut current: Option<Shape> = None;
    let mut current_end = String::new();
    let mut in_text = false;
    let mut in_shape_text_body = false;
    let mut in_table = false;
    let mut current_row: Option<TableRow> = None;
    let mut current_cell: Option<TableCell> = None;
    let mut current_paragraph: Option<Vec<String>> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e))
                if current.is_none()
                    && matches!(local_name(e.name().as_ref()), "sp" | "pic" | "graphicFrame") =>
            {
                let kind = local_name(e.name().as_ref()).to_string();
                current_end.clone_from(&kind);
                current = Some(Shape {
                    kind,
                    ..Shape::default()
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "cNvPr" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.id = attr(&e, "id")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_default();
                    shape.name = attr(&e, "name").unwrap_or_default();
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "ph" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.is_placeholder = true;
                    shape.placeholder = Some(Placeholder {
                        literal_type: attr(&e, "type").unwrap_or_default(),
                        index: attr(&e, "idx").and_then(|idx| idx.parse().ok()),
                    });
                }
            }
            Ok(Event::Start(e))
                if current.as_ref().is_some_and(|shape| shape.kind == "sp")
                    && local_name(e.name().as_ref()) == "txBody" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.has_text_body = true;
                }
                in_shape_text_body = true;
            }
            Ok(Event::Empty(e))
                if current.as_ref().is_some_and(|shape| shape.kind == "sp")
                    && local_name(e.name().as_ref()) == "txBody" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.has_text_body = true;
                }
            }
            Ok(Event::Start(e)) if in_shape_text_body && local_name(e.name().as_ref()) == "p" => {
                current_paragraph = Some(Vec::new());
            }
            Ok(Event::Empty(e)) if in_shape_text_body && local_name(e.name().as_ref()) == "p" => {
                if let Some(shape) = current.as_mut() {
                    shape.paragraphs.push(Vec::new());
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "off" =>
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
                if current.is_some() && local_name(e.name().as_ref()) == "ext" =>
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
                    && local_name(e.name().as_ref()) == "blip" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.image_rel_id = attr(&e, "embed").unwrap_or_default();
                }
            }
            Ok(Event::Start(e)) if current.is_some() && local_name(e.name().as_ref()) == "tbl" => {
                in_table = true;
                if let Some(shape) = current.as_mut() {
                    shape.table = Some(TableInfo::default());
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_table && local_name(e.name().as_ref()) == "gridCol" =>
            {
                if let Some(table) = current.as_mut().and_then(|shape| shape.table.as_mut())
                    && let Some(width) = attr(&e, "w").and_then(|value| value.parse().ok())
                {
                    table.columns.push(width);
                }
            }
            Ok(Event::Start(e)) if in_table && local_name(e.name().as_ref()) == "tr" => {
                current_row = Some(TableRow {
                    height: attr(&e, "h").and_then(|value| value.parse().ok()),
                    cells: Vec::new(),
                });
            }
            Ok(Event::Start(e)) if in_table && local_name(e.name().as_ref()) == "tc" => {
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
            Ok(Event::Start(e)) if current.is_some() && local_name(e.name().as_ref()) == "t" => {
                in_text = true;
            }
            Ok(Event::Text(e)) if in_text => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
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
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => {
                in_text = false;
            }
            Ok(Event::End(e)) if in_shape_text_body && local_name(e.name().as_ref()) == "p" => {
                if let Some(paragraph) = current_paragraph.take()
                    && let Some(shape) = current.as_mut()
                {
                    shape.paragraphs.push(paragraph);
                }
            }
            Ok(Event::End(e))
                if in_shape_text_body && local_name(e.name().as_ref()) == "txBody" =>
            {
                in_shape_text_body = false;
            }
            Ok(Event::End(e)) if in_table && local_name(e.name().as_ref()) == "tc" => {
                if let Some(cell) = current_cell.take()
                    && let Some(row) = current_row.as_mut()
                {
                    row.cells.push(cell);
                }
            }
            Ok(Event::End(e)) if in_table && local_name(e.name().as_ref()) == "tr" => {
                if let Some(row) = current_row.take()
                    && let Some(table) = current.as_mut().and_then(|shape| shape.table.as_mut())
                {
                    table.rows.push(row);
                }
            }
            Ok(Event::End(e)) if in_table && local_name(e.name().as_ref()) == "tbl" => {
                in_table = false;
            }
            Ok(Event::End(e))
                if current.is_some() && local_name(e.name().as_ref()) == current_end =>
            {
                if let Some(shape) = current.take() {
                    shapes.push(shape);
                }
                current_end.clear();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    shapes
}
