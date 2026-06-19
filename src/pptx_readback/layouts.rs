use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use super::{bounds_json, pptx_shape_models};
use crate::{
    CliError, CliResult, attr, attr_exact, local_name, package_type, relationship_entries,
    relationships, relationships_part_for, resolve_relationship_target, zip_text,
};

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
