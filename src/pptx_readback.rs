mod comments;
mod notes;
mod slide_parts;

pub(crate) use comments::pptx_comments_list;
pub(crate) use notes::{pptx_extract_notes, pptx_notes_show};
use slide_parts::pptx_slide_part_refs;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

use crate::{
    CliError, CliResult, add_selector, attr, attr_exact, content_type_for_part, json_u32,
    local_name, package_type, parse_u32_flags, relationship_entries, relationships,
    relationships_part_for, resolve_relationship_target, zip_text,
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

pub(crate) fn pptx_extract_text_json_args(args: &Value) -> CliResult<Vec<String>> {
    let mut rest = Vec::new();
    if let Some(slide) = json_u32(args, "slide")? {
        rest.push("--slide".to_string());
        rest.push(slide.to_string());
    }
    if let Some(slides) = args.get("slides") {
        let values = slides
            .as_array()
            .ok_or_else(|| CliError::invalid_args("slides must be an array"))?;
        for value in values {
            let slide = if let Some(number) = value.as_u64() {
                u32::try_from(number)
                    .map_err(|_| CliError::invalid_args("slides entries must fit in uint32"))?
            } else if let Some(text) = value.as_str() {
                text.parse::<u32>().map_err(|_| {
                    CliError::invalid_args("slides entries must be integers or integer strings")
                })?
            } else {
                return Err(CliError::invalid_args(
                    "slides entries must be integers or integer strings",
                ));
            };
            rest.push("--slide".to_string());
            rest.push(slide.to_string());
        }
    }
    Ok(rest)
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

pub(crate) fn pptx_extract_text(file: &str, args: &[String]) -> CliResult<Value> {
    if package_type(file)? != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {})",
            package_type(file)?
        )));
    }

    let selected_slides = parse_u32_flags(args, "--slide")?;
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut values = Vec::new();
    for (index, (_slide_id, rel_id)) in slides.iter().enumerate() {
        let slide_number = index as u32 + 1;
        if !selected_slides.is_empty() && !selected_slides.contains(&slide_number) {
            continue;
        }
        let target = rels
            .get(rel_id)
            .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
        let part = normalize_ppt_target(target);
        let xml = zip_text(file, &part)?;
        values.push(json!({
            "slide": slide_number,
            "shapes": pptx_extract_text_shapes(&xml),
        }));
    }
    Ok(json!({
        "file": file,
        "slides": values,
    }))
}

#[derive(Clone)]
struct PptxMasterRef {
    part_uri: String,
    layout_uris: Vec<String>,
    theme_uri: String,
}

#[derive(Clone)]
struct PptxLayoutInfo {
    id: String,
    name: String,
    part_uri: String,
    master_id: String,
    theme_uri: String,
    preserve: bool,
    user_drawn: bool,
    placeholders: Vec<Value>,
}

pub(crate) fn pptx_masters_list(file: &str) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let masters = pptx_presentation_masters(file)?;
    let entries = masters
        .iter()
        .enumerate()
        .map(|(index, master)| {
            let number = index + 1;
            let primary = number.to_string();
            let mut entry = Map::new();
            entry.insert("index".to_string(), json!(number));
            entry.insert("uri".to_string(), json!(master.part_uri));
            entry.insert("primarySelector".to_string(), json!(primary.clone()));
            entry.insert("selectors".to_string(), json!([primary]));
            entry.insert("layouts".to_string(), json!(master.layout_uris.len()));
            if !master.theme_uri.is_empty() {
                entry.insert("theme".to_string(), json!(master.theme_uri));
            }
            Value::Object(entry)
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "masters": entries,
    }))
}

pub(crate) fn pptx_masters_show(file: &str, master_number: i64) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let masters = pptx_presentation_masters(file)?;
    if master_number < 1 || master_number as usize > masters.len() {
        return Err(CliError::invalid_args(format!(
            "master {master_number} not found"
        )));
    }
    let master = &masters[master_number as usize - 1];
    let master_xml = zip_text(file, master.part_uri.trim_start_matches('/')).unwrap_or_default();
    let mut output = Map::new();
    output.insert("uri".to_string(), json!(master.part_uri));
    output.insert("index".to_string(), json!(master_number));
    output.insert("layouts".to_string(), json!(master.layout_uris));
    output.insert("layoutCount".to_string(), json!(master.layout_uris.len()));
    if !master.theme_uri.is_empty() {
        output.insert("themeUri".to_string(), json!(master.theme_uri));
        if let Some(theme) = pptx_theme_info(file, &master.theme_uri) {
            output.insert("theme".to_string(), theme.clone());
            if let Some(defaults) = pptx_default_text_style_info(&theme) {
                output.insert("defaultTextStyleInfo".to_string(), defaults);
            }
        }
    }
    output.insert(
        "shapes".to_string(),
        json!(pptx_master_shape_count(&master_xml)),
    );
    let placeholders = pptx_layout_placeholders(&master_xml);
    if !placeholders.is_empty() {
        output.insert("placeholders".to_string(), Value::Array(placeholders));
    }
    let text_styles = pptx_master_text_styles(&master_xml);
    if !text_styles.is_empty() {
        output.insert("textStyles".to_string(), Value::Object(text_styles));
    }
    Ok(Value::Object(output))
}

fn pptx_master_shape_count(xml: &str) -> usize {
    ["<p:sp", "<p:pic", "<p:graphicFrame", "<p:grpSp"]
        .into_iter()
        .map(|needle| xml.matches(needle).count())
        .sum()
}

fn pptx_master_text_styles(xml: &str) -> Map<String, Value> {
    let mut styles = Map::new();
    if xml.contains("title") || xml.contains("Title") {
        styles.insert(
            "title".to_string(),
            json!({
                "placeholderType": "title",
            }),
        );
    }
    if xml.contains("body") || xml.contains("Body") {
        styles.insert(
            "body".to_string(),
            json!({
                "placeholderType": "body",
            }),
        );
    }
    if xml.contains("ctrTitle") || xml.contains("centerTitle") {
        styles.insert(
            "centerTitle".to_string(),
            json!({
                "placeholderType": "centerTitle",
            }),
        );
    }
    if xml.contains("subTitle") || xml.contains("subtitle") {
        styles.insert(
            "subtitle".to_string(),
            json!({
                "placeholderType": "subtitle",
            }),
        );
    }
    styles
}

pub(crate) fn pptx_layouts_list(file: &str, master: Option<u32>) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let mut layouts = pptx_presentation_layouts(file)?;
    if let Some(master) = master
        && master > 0
    {
        let master_id = format!("master-{master}");
        layouts.retain(|layout| layout.master_id == master_id);
    }
    let entries = layouts
        .iter()
        .enumerate()
        .map(|(index, layout)| {
            let number = index + 1;
            let placeholders = layout
                .placeholders
                .iter()
                .filter_map(|placeholder| placeholder.get("key").and_then(Value::as_str))
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let mut entry = Map::new();
            entry.insert("id".to_string(), json!(layout.id));
            entry.insert("number".to_string(), json!(number));
            entry.insert("name".to_string(), json!(layout.name));
            entry.insert("partUri".to_string(), json!(layout.part_uri));
            if !layout.master_id.is_empty() {
                entry.insert("masterId".to_string(), json!(layout.master_id));
            }
            entry.insert("primarySelector".to_string(), json!(number.to_string()));
            entry.insert(
                "selectors".to_string(),
                json!(pptx_layout_selectors(number, &layout.name)),
            );
            entry.insert("placeholderCount".to_string(), json!(placeholders.len()));
            entry.insert("placeholders".to_string(), json!(placeholders));
            Value::Object(entry)
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "file": file,
        "layouts": entries,
    }))
}

pub(crate) fn pptx_layouts_show(file: &str, selector: &str) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let layouts = pptx_presentation_layouts(file)?;
    let layout = pptx_find_layout(&layouts, selector)
        .ok_or_else(|| CliError::invalid_args(format!("layout not found: {selector}")))?;
    let mut output = Map::new();
    output.insert("id".to_string(), json!(layout.id));
    output.insert("name".to_string(), json!(layout.name));
    output.insert("partUri".to_string(), json!(layout.part_uri));
    if !layout.master_id.is_empty() {
        output.insert("masterId".to_string(), json!(layout.master_id));
    }
    if !layout.theme_uri.is_empty() {
        output.insert("themeUri".to_string(), json!(layout.theme_uri));
        if let Some(theme) = pptx_theme_info(file, &layout.theme_uri) {
            output.insert("theme".to_string(), theme.clone());
            if let Some(defaults) = pptx_default_text_style_info(&theme) {
                output.insert("defaultTextStyleInfo".to_string(), defaults);
            }
        }
    }
    output.insert("preserve".to_string(), json!(layout.preserve));
    output.insert("userDrawn".to_string(), json!(layout.user_drawn));
    output.insert(
        "placeholders".to_string(),
        Value::Array(layout.placeholders.clone()),
    );
    Ok(Value::Object(output))
}

fn pptx_presentation_layouts(file: &str) -> CliResult<Vec<PptxLayoutInfo>> {
    let masters = pptx_presentation_masters(file)?;
    let mut master_uri_to_id = BTreeMap::<String, String>::new();
    let mut master_uri_to_theme = BTreeMap::<String, String>::new();
    for (index, master) in masters.iter().enumerate() {
        master_uri_to_id.insert(master.part_uri.clone(), format!("master-{}", index + 1));
        master_uri_to_theme.insert(master.part_uri.clone(), master.theme_uri.clone());
    }

    let mut layouts = Vec::new();
    for master in &masters {
        for layout_uri in &master.layout_uris {
            let layout_part = layout_uri.trim_start_matches('/');
            let xml = zip_text(file, layout_part)?;
            let (name, preserve, user_drawn) = pptx_layout_metadata(&xml);
            let master_part_uri = pptx_layout_master_part(file, layout_uri)?;
            let master_id = master_uri_to_id
                .get(&master_part_uri)
                .cloned()
                .unwrap_or_default();
            let theme_uri = master_uri_to_theme
                .get(&master_part_uri)
                .cloned()
                .unwrap_or_default();
            layouts.push(PptxLayoutInfo {
                id: format!("layout-{}", layouts.len() + 1),
                name,
                part_uri: layout_uri.clone(),
                master_id,
                theme_uri,
                preserve,
                user_drawn,
                placeholders: pptx_layout_placeholders(&xml),
            });
        }
    }
    Ok(layouts)
}

fn pptx_presentation_masters(file: &str) -> CliResult<Vec<PptxMasterRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut reader = Reader::from_str(&presentation);
    reader.config_mut().trim_text(true);
    let mut masters = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldMasterId" =>
            {
                let Some(rel_id) = attr_exact(&e, "r:id") else {
                    continue;
                };
                let Some(target) = rels.get(&rel_id) else {
                    return Err(CliError::unexpected(format!(
                        "relationship {rel_id} not found in presentation.xml.rels"
                    )));
                };
                let master_part_uri = resolve_relationship_target("/ppt/presentation.xml", target);
                let (layout_uris, theme_uri) =
                    pptx_master_layouts_and_theme(file, &master_part_uri);
                masters.push(PptxMasterRef {
                    part_uri: master_part_uri,
                    layout_uris,
                    theme_uri,
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(masters)
}

fn pptx_master_layouts_and_theme(file: &str, master_uri: &str) -> (Vec<String>, String) {
    let rels = relationship_entries(file, &relationships_part_for(master_uri)).unwrap_or_default();
    let mut layout_uris = Vec::new();
    let mut theme_uri = String::new();
    for rel in rels {
        match rel.rel_type.as_str() {
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" => {
                layout_uris.push(resolve_relationship_target(master_uri, &rel.target));
            }
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" => {
                theme_uri = resolve_relationship_target(master_uri, &rel.target);
            }
            _ => {}
        }
    }
    (layout_uris, theme_uri)
}

fn pptx_layout_master_part(file: &str, layout_uri: &str) -> CliResult<String> {
    let rels = relationship_entries(file, &relationships_part_for(layout_uri)).unwrap_or_default();
    for rel in rels {
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster"
        {
            return Ok(resolve_relationship_target(layout_uri, &rel.target));
        }
    }
    Ok(String::new())
}

fn pptx_layout_metadata(xml: &str) -> (String, bool, bool) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut name = String::new();
    let mut preserve = false;
    let mut user_drawn = false;
    let mut saw_root = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if !saw_root {
                    preserve = pptx_truthy_attr(&e, "preserve");
                    user_drawn = pptx_truthy_attr(&e, "userDrawn");
                    saw_root = true;
                }
                if local_name(e.name().as_ref()) == "cSld" {
                    name = attr(&e, "name").unwrap_or_default();
                    if saw_root {
                        break;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    (name, preserve, user_drawn)
}

fn pptx_truthy_attr(e: &BytesStart<'_>, key: &str) -> bool {
    matches!(attr(e, key).as_deref(), Some("1" | "true"))
}

fn pptx_layout_placeholders(xml: &str) -> Vec<Value> {
    pptx_shape_models(xml)
        .into_iter()
        .filter_map(|shape| {
            if !shape.is_placeholder {
                return None;
            }
            let placeholder = shape.placeholder.as_ref()?;
            let literal_type = placeholder.literal_type.as_str();
            let role = pptx_layout_placeholder_role(literal_type);
            let index = placeholder.index.unwrap_or(0);
            let key = if literal_type.is_empty() {
                if shape.id != 0 {
                    format!("shape:{}", shape.id)
                } else {
                    "unknown".to_string()
                }
            } else if placeholder.index.is_some() {
                format!("{literal_type}:{index}")
            } else {
                literal_type.to_string()
            };
            let mut value = Map::new();
            value.insert("key".to_string(), json!(key));
            value.insert("role".to_string(), json!(role));
            value.insert("index".to_string(), json!(index));
            value.insert("shapeName".to_string(), json!(shape.name));
            value.insert("literalType".to_string(), json!(""));
            value.insert("resolvedType".to_string(), json!(""));
            if let Some(bounds) = shape.bounds.as_ref() {
                value.insert(
                    "geometry".to_string(),
                    json!({
                        "bounds": bounds_json(bounds),
                    }),
                );
            }
            Some(Value::Object(value))
        })
        .collect()
}

fn pptx_layout_placeholder_role(literal_type: &str) -> String {
    match literal_type {
        "title" | "ctrTitle" => "title",
        "subTitle" => "subtitle",
        "body" => "body",
        "pic" => "pic",
        "tbl" => "table",
        "chart" => "chart",
        "obj" => "object",
        "dt" => "date",
        "ftr" => "footer",
        "sldNum" => "slideNumber",
        other => other,
    }
    .to_string()
}

fn pptx_layout_selectors(number: usize, name: &str) -> Vec<String> {
    let primary = number.to_string();
    let mut selectors = vec![primary.clone()];
    if !name.is_empty() && name != primary {
        selectors.push(name.to_string());
    }
    selectors
}

fn pptx_find_layout<'a>(
    layouts: &'a [PptxLayoutInfo],
    selector: &str,
) -> Option<&'a PptxLayoutInfo> {
    if let Ok(number) = selector.parse::<usize>()
        && number >= 1
    {
        return layouts.get(number - 1);
    }
    layouts.iter().find(|layout| layout.name == selector)
}

fn pptx_theme_info(file: &str, theme_uri: &str) -> Option<Value> {
    let xml = zip_text(file, theme_uri.trim_start_matches('/')).ok()?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut theme_name = String::new();
    let mut color_scheme = Map::new();
    let mut font_scheme = Map::new();
    let mut in_theme_elements = false;
    let mut in_color_scheme = false;
    let mut in_font_scheme = false;
    let mut current_color = String::new();
    let mut current_font = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                match name.as_str() {
                    "theme" => theme_name = attr(&e, "name").unwrap_or_default(),
                    "themeElements" => in_theme_elements = true,
                    "clrScheme" if in_theme_elements => {
                        in_color_scheme = true;
                        if let Some(value) = attr(&e, "name") {
                            color_scheme.insert("name".to_string(), json!(value));
                        }
                    }
                    "fontScheme" if in_theme_elements => {
                        in_font_scheme = true;
                        if let Some(value) = attr(&e, "name") {
                            font_scheme.insert("name".to_string(), json!(value));
                        }
                    }
                    "dk1" | "lt1" | "dk2" | "lt2" | "accent1" | "accent2" | "accent3"
                    | "accent4" | "accent5" | "accent6" | "hlink" | "folHlink"
                        if in_color_scheme =>
                    {
                        current_color = name;
                    }
                    "majorFont" | "minorFont" if in_font_scheme => current_font = name,
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if in_color_scheme && !current_color.is_empty() {
                    if name == "srgbClr" {
                        if let Some(value) = attr(&e, "val") {
                            pptx_insert_theme_color(&mut color_scheme, &current_color, value);
                        }
                    } else if name == "sysClr"
                        && let Some(value) = attr(&e, "lastClr")
                    {
                        pptx_insert_theme_color(&mut color_scheme, &current_color, value);
                    }
                }
                if in_font_scheme && !current_font.is_empty() {
                    match (current_font.as_str(), name.as_str()) {
                        ("majorFont", "latin") => {
                            if let Some(value) = attr(&e, "typeface") {
                                font_scheme.insert("majorFont".to_string(), json!(value));
                            }
                        }
                        ("minorFont", "latin") => {
                            if let Some(value) = attr(&e, "typeface") {
                                font_scheme.insert("minorFont".to_string(), json!(value));
                            }
                        }
                        ("majorFont", "ea") => {
                            if let Some(value) = attr(&e, "typeface")
                                && !value.is_empty()
                            {
                                font_scheme.insert("eastAsianMajorFont".to_string(), json!(value));
                            }
                        }
                        ("minorFont", "ea") => {
                            if let Some(value) = attr(&e, "typeface")
                                && !value.is_empty()
                            {
                                font_scheme.insert("eastAsianMinorFont".to_string(), json!(value));
                            }
                        }
                        ("majorFont", "cs") => {
                            if let Some(value) = attr(&e, "typeface")
                                && !value.is_empty()
                            {
                                font_scheme
                                    .insert("complexScriptMajorFont".to_string(), json!(value));
                            }
                        }
                        ("minorFont", "cs") => {
                            if let Some(value) = attr(&e, "typeface")
                                && !value.is_empty()
                            {
                                font_scheme
                                    .insert("complexScriptMinorFont".to_string(), json!(value));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => match local_name(e.name().as_ref()) {
                "themeElements" => in_theme_elements = false,
                "clrScheme" => in_color_scheme = false,
                "fontScheme" => in_font_scheme = false,
                "dk1" | "lt1" | "dk2" | "lt2" | "accent1" | "accent2" | "accent3" | "accent4"
                | "accent5" | "accent6" | "hlink" | "folHlink" => {
                    current_color.clear();
                }
                "majorFont" | "minorFont" => current_font.clear(),
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    let mut theme = Map::new();
    if !theme_name.is_empty() {
        theme.insert("name".to_string(), json!(theme_name));
    }
    if !color_scheme.is_empty() {
        theme.insert("colorScheme".to_string(), Value::Object(color_scheme));
    }
    if !font_scheme.is_empty() {
        theme.insert("fontScheme".to_string(), Value::Object(font_scheme));
    }
    Some(Value::Object(theme))
}

fn pptx_insert_theme_color(color_scheme: &mut Map<String, Value>, key: &str, value: String) {
    let json_key = match key {
        "dk1" => "dark1",
        "lt1" => "light1",
        "dk2" => "dark2",
        "lt2" => "light2",
        "hlink" => "hypLink",
        "folHlink" => "folLink",
        other => other,
    };
    color_scheme.insert(json_key.to_string(), json!(value));
}

fn pptx_default_text_style_info(theme: &Value) -> Option<Value> {
    let theme_object = theme.as_object()?;
    let mut info = Map::new();
    if let Some(name) = theme_object.get("name").and_then(Value::as_str)
        && !name.is_empty()
    {
        info.insert("themeName".to_string(), json!(name));
    }
    if let Some(font_scheme) = theme_object.get("fontScheme").and_then(Value::as_object) {
        if let Some(major_font) = font_scheme.get("majorFont").and_then(Value::as_str)
            && !major_font.is_empty()
        {
            info.insert("majorFont".to_string(), json!(major_font));
        }
        if let Some(minor_font) = font_scheme.get("minorFont").and_then(Value::as_str)
            && !minor_font.is_empty()
        {
            info.insert("minorFont".to_string(), json!(minor_font));
        }
    }
    let accents = theme_object
        .get("colorScheme")
        .and_then(Value::as_object)
        .map(|color_scheme| {
            [
                "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
            ]
            .into_iter()
            .filter_map(|key| color_scheme.get(key).and_then(Value::as_str))
            .filter(|value| !value.is_empty())
            .map(|value| json!(value))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !accents.is_empty() {
        info.insert("accentColors".to_string(), Value::Array(accents));
    }
    if info.is_empty() {
        None
    } else {
        Some(Value::Object(info))
    }
}

pub(crate) fn pptx_tables_show(
    file: &str,
    slide: u32,
    table_id: u32,
    target: Option<&str>,
    include_details: bool,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let slides = pptx_slide_part_refs(file)?;
    if slide == 0 || slide as usize > slides.len() {
        return Err(CliError::invalid_args(format!(
            "slide number {slide} out of range (1-{})",
            slides.len()
        )));
    }
    let slide_ref = &slides[slide as usize - 1];
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let shapes = pptx_shape_models(&slide_xml);
    let targets = pptx_selector_targets_from_shapes(&shapes);
    let resolved_table_id = pptx_resolve_table_target(&shapes, &targets, target)?;
    let wanted_table_id = if table_id > 0 {
        Some(table_id)
    } else {
        resolved_table_id
    };
    let tables = pptx_table_summaries(slide, &shapes, &targets, wanted_table_id, include_details);
    if let Some(wanted_table_id) = wanted_table_id
        && tables.is_empty()
    {
        return Err(CliError::target_not_found(format!(
            "target not found: table shape ID {wanted_table_id} on slide {slide}"
        )));
    }
    Ok(json!({
        "file": file,
        "slide": slide,
        "tables": tables,
    }))
}

fn pptx_resolve_table_target(
    shapes: &[Shape],
    targets: &[Value],
    target: Option<&str>,
) -> CliResult<Option<u32>> {
    let target = target.map(str::trim).unwrap_or_default();
    if target.is_empty() || target == "@all-tables" {
        return Ok(None);
    }
    for (shape, target_value) in shapes.iter().zip(targets) {
        let primary = target_value
            .get("primarySelector")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let selectors = target_value
            .get("selectors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str);
        if primary == target || selectors.clone().any(|selector| selector == target) {
            if shape.kind == "graphicFrame" && shape.table.is_some() {
                return Ok(Some(shape.id));
            }
            return Err(CliError::invalid_args(format!(
                "target {target:?} resolves to {primary}, not a table"
            )));
        }
    }
    Err(CliError::target_not_found(format!(
        "target not found: target not found: {target} (available selectors: {})",
        pptx_available_shape_selectors(targets).join(", ")
    )))
}

fn pptx_available_shape_selectors(targets: &[Value]) -> Vec<String> {
    let mut selectors = Vec::new();
    add_selector(&mut selectors, "@all-shapes".to_string());
    add_selector(&mut selectors, "@all-shapes-nonph".to_string());
    add_selector(&mut selectors, "@all-tables".to_string());
    for target in targets {
        if let Some(items) = target.get("selectors").and_then(Value::as_array) {
            for item in items {
                if let Some(selector) = item.as_str() {
                    add_selector(&mut selectors, selector.to_string());
                }
            }
        }
    }
    selectors
}

fn pptx_table_summaries(
    slide: u32,
    shapes: &[Shape],
    targets: &[Value],
    table_id: Option<u32>,
    include_details: bool,
) -> Vec<Value> {
    shapes
        .iter()
        .zip(targets)
        .filter(|(shape, _target)| shape.kind == "graphicFrame" && shape.table.is_some())
        .filter(|(shape, _target)| table_id.is_none_or(|table_id| shape.id == table_id))
        .map(|(shape, target)| pptx_table_summary(slide, shape, target, include_details))
        .collect()
}

fn pptx_table_summary(slide: u32, shape: &Shape, target: &Value, include_details: bool) -> Value {
    let table = shape.table.as_ref().expect("table summary requires table");
    let cells = table
        .rows
        .iter()
        .map(|row| {
            Value::Array(
                row.cells
                    .iter()
                    .map(|cell| Value::String(cell.text.clone()))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let mut summary = Map::new();
    summary.insert("slide".to_string(), json!(slide));
    summary.insert("shapeId".to_string(), json!(shape.id));
    summary.insert("shapeName".to_string(), json!(shape.name));
    summary.insert(
        "targetKind".to_string(),
        target
            .get("targetKind")
            .cloned()
            .unwrap_or_else(|| json!("table")),
    );
    summary.insert(
        "primarySelector".to_string(),
        target
            .get("primarySelector")
            .cloned()
            .unwrap_or_else(|| json!(format!("shape:{}", shape.id))),
    );
    summary.insert(
        "selectors".to_string(),
        target
            .get("selectors")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    summary.insert("rows".to_string(), json!(table.rows.len()));
    summary.insert("cols".to_string(), json!(table_column_count(table)));
    summary.insert("cells".to_string(), Value::Array(cells));
    if let Some(bounds) = shape.bounds.as_ref() {
        summary.insert("bounds".to_string(), bounds_json(bounds));
    }
    if include_details {
        summary.insert("tableInfo".to_string(), table_info_json(table));
    }
    Value::Object(summary)
}

fn pptx_extract_text_shapes(xml: &str) -> Vec<Value> {
    let shapes = pptx_shape_models(xml);
    let targets = pptx_selector_targets_from_shapes(&shapes);
    shapes
        .iter()
        .zip(targets)
        .filter(|(shape, _target)| shape.kind == "sp" && shape.has_text_body)
        .map(|(shape, target)| {
            let key = pptx_extract_text_shape_key(shape, &target);
            json!({
                "id": shape.id,
                "name": shape.name,
                "type": shape.kind,
                "key": key,
                "text": pptx_extract_text_body(shape),
            })
        })
        .collect()
}

fn pptx_extract_text_shape_key(shape: &Shape, target: &Value) -> String {
    let lower_name = shape.name.to_ascii_lowercase();
    if lower_name.contains("content placeholder")
        && let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index)
    {
        return format!("body:{index}");
    }
    target
        .get("primarySelector")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn pptx_extract_text_body(shape: &Shape) -> Value {
    pptx_text_block_from_paragraphs(&shape.paragraphs, true, true)
}

fn pptx_text_block_from_paragraphs(
    paragraphs: &[Vec<String>],
    include_body_properties: bool,
    synthesize_empty_paragraph: bool,
) -> Value {
    let paragraphs = if paragraphs.is_empty() && synthesize_empty_paragraph {
        vec![Vec::<String>::new()]
    } else {
        paragraphs.to_vec()
    };
    let paragraph_values = paragraphs
        .iter()
        .map(|runs| {
            let text = runs.join("");
            let mut paragraph = Map::new();
            if !runs.is_empty() {
                paragraph.insert(
                    "runs".to_string(),
                    Value::Array(runs.iter().map(|run| json!({"text": run})).collect()),
                );
            }
            paragraph.insert("text".to_string(), json!(text));
            Value::Object(paragraph)
        })
        .collect::<Vec<_>>();
    let plain_text = paragraphs
        .iter()
        .map(|runs| runs.join(""))
        .collect::<Vec<_>>()
        .join("\n");
    let mut block = Map::new();
    block.insert("paragraphs".to_string(), Value::Array(paragraph_values));
    block.insert("plainText".to_string(), json!(plain_text));
    if include_body_properties {
        block.insert("bodyProperties".to_string(), json!({}));
    }
    Value::Object(block)
}

#[derive(Clone, Default)]
struct ShapeText {
    key: String,
    name: String,
    text: String,
}

fn pptx_slide_texts(file: &str) -> CliResult<Vec<Vec<ShapeText>>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut out = Vec::new();
    for (_, rel_id) in slides {
        let target = rels
            .get(&rel_id)
            .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
        let part = normalize_ppt_target(target);
        let xml = zip_text(file, &part)?;
        out.push(
            pptx_shape_models(&xml)
                .into_iter()
                .filter(|shape| !shape.text.is_empty())
                .map(|shape| ShapeText {
                    key: shape_key(&shape),
                    name: shape.name,
                    text: shape.text,
                })
                .collect(),
        );
    }
    Ok(out)
}

fn shape_key(shape: &Shape) -> String {
    if shape.is_placeholder && shape.name.to_ascii_lowercase().contains("title") {
        "title".to_string()
    } else if !shape.name.is_empty() {
        shape.name.clone()
    } else {
        format!("shape:{}", shape.id)
    }
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
struct Shape {
    id: u32,
    name: String,
    kind: String,
    is_placeholder: bool,
    has_text_body: bool,
    text: String,
    paragraphs: Vec<Vec<String>>,
    bounds: Option<Bounds>,
    placeholder: Option<Placeholder>,
    image_rel_id: String,
    table: Option<TableInfo>,
}

#[derive(Clone)]
struct Placeholder {
    literal_type: String,
    index: Option<u32>,
}

#[derive(Clone)]
struct Bounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

#[derive(Default)]
struct TableInfo {
    columns: Vec<i64>,
    rows: Vec<TableRow>,
}

#[derive(Default)]
struct TableRow {
    height: Option<i64>,
    cells: Vec<TableCell>,
}

#[derive(Clone)]
struct TableCell {
    text: String,
    grid_span: u32,
    row_span: u32,
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

fn pptx_selector_targets_from_shapes(shapes: &[Shape]) -> Vec<Value> {
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

fn bounds_json(bounds: &Bounds) -> Value {
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

fn table_info_json(table: &TableInfo) -> Value {
    let cells = table
        .rows
        .iter()
        .map(|row| {
            Value::Array(
                row.cells
                    .iter()
                    .map(|cell| Value::String(cell.text.clone()))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let row_defs = table
        .rows
        .iter()
        .map(|row| {
            let mut row_def = Map::new();
            if let Some(height) = row.height {
                row_def.insert("height".to_string(), json!(height));
            }
            row_def.insert("cells".to_string(), table_cells_json(&row.cells));
            Value::Object(row_def)
        })
        .collect::<Vec<_>>();
    let column_defs = table
        .columns
        .iter()
        .map(|width| json!({"width": width}))
        .collect::<Vec<_>>();
    let cell_defs = table
        .rows
        .iter()
        .map(|row| table_cells_json(&row.cells))
        .collect::<Vec<_>>();
    json!({
        "rows": table.rows.len(),
        "cols": table_column_count(table),
        "cells": cells,
        "rowDefs": row_defs,
        "columnDefs": column_defs,
        "cellDefs": cell_defs,
    })
}

fn table_cells_json(cells: &[TableCell]) -> Value {
    Value::Array(
        cells
            .iter()
            .map(|cell| {
                json!({
                    "text": cell.text.clone(),
                    "gridSpan": cell.grid_span,
                    "rowSpan": cell.row_span,
                })
            })
            .collect(),
    )
}

fn table_column_count(table: &TableInfo) -> usize {
    table.columns.len().max(
        table
            .rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(0),
    )
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

fn pptx_shape_models(xml: &str) -> Vec<Shape> {
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
