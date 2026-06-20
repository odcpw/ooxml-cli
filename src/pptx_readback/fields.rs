use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};

use crate::{
    CliError, CliResult, attr, attr_exact, decode_xml_text, local_name, package_type,
    relationship_entries_from_xml, resolve_relationship_target, xml_direct_child_ranges,
    xml_fragment_bounds, xml_token_name, zip_text,
};

const SLIDE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const MASTER_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";

pub(crate) fn pptx_fields_inspect(file: &str) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }

    let refs = pptx_presentation_part_refs(file)?;
    let masters = refs
        .masters
        .iter()
        .map(|part| {
            let xml = zip_text(file, part.trim_start_matches('/'))?;
            Ok(master_defaults_json(part, &xml))
        })
        .collect::<CliResult<Vec<_>>>()?;
    let slides = refs
        .slides
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let xml = zip_text(file, part.trim_start_matches('/'))?;
            Ok(slide_fields_json(index + 1, part, &xml))
        })
        .collect::<CliResult<Vec<_>>>()?;

    Ok(json!({
        "masters": masters,
        "slides": slides,
    }))
}

struct PptxPresentationPartRefs {
    slides: Vec<String>,
    masters: Vec<String>,
}

fn pptx_presentation_part_refs(file: &str) -> CliResult<PptxPresentationPartRefs> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let rels_xml = zip_text(file, "ppt/_rels/presentation.xml.rels")?;
    let rels = relationship_entries_from_xml(&rels_xml);
    let mut slides = Vec::new();
    let mut masters = Vec::new();
    let mut reader = Reader::from_str(&presentation);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name != "sldId" && name != "sldMasterId" {
                    continue;
                }
                let Some(rel_id) = attr_exact(&e, "r:id") else {
                    continue;
                };
                let expected = if name == "sldId" {
                    SLIDE_REL_TYPE
                } else {
                    MASTER_REL_TYPE
                };
                let rel = rels.iter().find(|rel| rel.id == rel_id).ok_or_else(|| {
                    CliError::unexpected(format!("missing relationship {rel_id}"))
                })?;
                if rel.rel_type != expected {
                    continue;
                }
                let part = resolve_relationship_target("/ppt/presentation.xml", &rel.target);
                if name == "sldId" {
                    slides.push(part);
                } else {
                    masters.push(part);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(PptxPresentationPartRefs { slides, masters })
}

fn master_defaults_json(part_uri: &str, xml: &str) -> Value {
    let hf_span = root_direct_child_span(xml, "hf");
    let mut out = Map::new();
    out.insert("partUri".to_string(), json!(part_uri));
    out.insert("hasHeaderFooter".to_string(), json!(hf_span.is_some()));
    if let Some((start, end)) = hf_span {
        let start_tag = &xml[start..start_tag_end(xml, start, end)];
        out.insert(
            "showSlideNumber".to_string(),
            json!(bool_attr_default_true(start_tag, "sldNum")),
        );
        out.insert(
            "showFooter".to_string(),
            json!(bool_attr_default_true(start_tag, "ftr")),
        );
        out.insert(
            "showDate".to_string(),
            json!(bool_attr_default_true(start_tag, "dt")),
        );
        out.insert(
            "showHeader".to_string(),
            json!(bool_attr_default_true(start_tag, "hdr")),
        );
    } else {
        out.insert("showSlideNumber".to_string(), json!(true));
        out.insert("showFooter".to_string(), json!(true));
        out.insert("showDate".to_string(), json!(true));
        out.insert("showHeader".to_string(), json!(true));
    }
    Value::Object(out)
}

fn root_direct_child_span(xml: &str, wanted: &str) -> Option<(usize, usize)> {
    let (open_end, _, close_start, self_closing) = document_root_bounds(xml).ok()?;
    if self_closing {
        return None;
    }
    xml_direct_child_ranges(xml, open_end + 1, close_start)
        .ok()?
        .into_iter()
        .find(|child| child.kind == wanted)
        .map(|child| (child.start, child.end))
}

fn document_root_bounds(xml: &str) -> CliResult<(usize, String, usize, bool)> {
    let mut cursor = 0usize;
    while cursor < xml.len() {
        let relative_start = xml[cursor..]
            .find('<')
            .ok_or_else(|| CliError::unexpected("invalid XML document"))?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..]
            .find('>')
            .ok_or_else(|| CliError::unexpected("invalid XML document"))?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('?') || token.starts_with('!') || token.starts_with('/') {
            cursor = tag_end + 1;
            continue;
        }
        let end = if token.trim_end().ends_with('/') {
            tag_end + 1
        } else {
            let name = xml_token_name(token)
                .ok_or_else(|| CliError::unexpected("invalid XML document"))?;
            find_matching_element_end(xml, local_name(name.as_bytes()), tag_end + 1, xml.len())
                .ok_or_else(|| CliError::unexpected("invalid XML document"))?
        };
        let (open_end, tag_name, close_start, self_closing) =
            xml_fragment_bounds(&xml[tag_start..end])?;
        return Ok((
            tag_start + open_end,
            tag_name,
            tag_start + close_start,
            self_closing,
        ));
    }
    Err(CliError::unexpected("invalid XML document"))
}

fn bool_attr_default_true(start_tag: &str, name: &str) -> bool {
    !matches!(
        attr_from_start_tag(start_tag, name).as_deref(),
        Some("0" | "false")
    )
}

fn slide_fields_json(slide: usize, part_uri: &str, xml: &str) -> Value {
    let mut out = Map::new();
    out.insert("slide".to_string(), json!(slide));
    out.insert("partUri".to_string(), json!(part_uri));
    if let Some(placeholder) = field_placeholder_json(xml, "ftr") {
        out.insert("footerPlaceholder".to_string(), placeholder);
    }
    if let Some(placeholder) = field_placeholder_json(xml, "dt") {
        out.insert("datePlaceholder".to_string(), placeholder);
    }
    if let Some(placeholder) = field_placeholder_json(xml, "sldNum") {
        out.insert("slideNumberPlaceholder".to_string(), placeholder);
    }
    Value::Object(out)
}

fn field_placeholder_json(xml: &str, placeholder_type: &str) -> Option<Value> {
    let shape_span = find_placeholder_shape_span(xml, placeholder_type)?;
    let shape_xml = &xml[shape_span.0..shape_span.1];
    let (shape_id, shape_name) = shape_identity(shape_xml);
    let (text, field_type) = shape_field_text(shape_xml);
    let mut out = Map::new();
    if shape_id != 0 {
        out.insert("shapeId".to_string(), json!(shape_id));
    }
    if !shape_name.is_empty() {
        out.insert("shapeName".to_string(), json!(shape_name));
    }
    if !text.is_empty() {
        out.insert("text".to_string(), json!(text));
    }
    if !field_type.is_empty() {
        out.insert("fieldType".to_string(), json!(field_type));
    }
    Some(Value::Object(out))
}

fn find_placeholder_shape_span(xml: &str, placeholder_type: &str) -> Option<(usize, usize)> {
    let c_sld = first_element_span(xml, "cSld", 0, xml.len())?;
    let sp_tree = first_element_span(xml, "spTree", c_sld.0, c_sld.1)?;
    let children = xml_direct_child_ranges(
        xml,
        content_start(xml, sp_tree)?,
        content_end(xml, sp_tree)?,
    )
    .ok()?;
    children.into_iter().find_map(|child| {
        if child.kind != "sp" {
            return None;
        }
        let shape_xml = &xml[child.start..child.end];
        if placeholder_type_of_shape(shape_xml).as_deref() == Some(placeholder_type) {
            Some((child.start, child.end))
        } else {
            None
        }
    })
}

fn placeholder_type_of_shape(shape_xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(shape_xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "ph" => {
                return attr(&e, "type");
            }
            Ok(Event::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
    }
}

fn shape_identity(shape_xml: &str) -> (i64, String) {
    let mut reader = Reader::from_str(shape_xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cNvPr" =>
            {
                let id = attr(&e, "id")
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(0);
                let name = attr(&e, "name").unwrap_or_default();
                return (id, name);
            }
            Ok(Event::Eof) => return (0, String::new()),
            Err(_) => return (0, String::new()),
            _ => {}
        }
    }
}

fn shape_field_text(shape_xml: &str) -> (String, String) {
    let Some(tx_body) = first_element_span(shape_xml, "txBody", 0, shape_xml.len()) else {
        return (String::new(), String::new());
    };
    let Ok(paragraphs) = xml_direct_child_ranges(
        shape_xml,
        content_start(shape_xml, tx_body).unwrap_or(tx_body.0),
        content_end(shape_xml, tx_body).unwrap_or(tx_body.1),
    ) else {
        return (String::new(), String::new());
    };
    for paragraph in paragraphs.into_iter().filter(|child| child.kind == "p") {
        if let Some((text, field_type)) =
            paragraph_field_text(shape_xml, paragraph.start, paragraph.end)
        {
            return (text, field_type);
        }
        let text = paragraph_run_text(shape_xml, paragraph.start, paragraph.end);
        if !text.is_empty() {
            return (text, String::new());
        }
    }
    (String::new(), String::new())
}

fn paragraph_field_text(xml: &str, start: usize, end: usize) -> Option<(String, String)> {
    let children = xml_direct_child_ranges(
        xml,
        content_start(xml, (start, end))?,
        content_end(xml, (start, end))?,
    )
    .ok()?;
    let field = children.into_iter().find(|child| child.kind == "fld")?;
    let field_xml = &xml[field.start..field.end];
    let field_type = attr_from_start_tag(
        &field_xml[..start_tag_end(field_xml, 0, field_xml.len())],
        "type",
    )
    .unwrap_or_default();
    Some((text_descendants(field_xml), field_type))
}

fn paragraph_run_text(xml: &str, start: usize, end: usize) -> String {
    let Some(content_start) = content_start(xml, (start, end)) else {
        return String::new();
    };
    let Some(content_end) = content_end(xml, (start, end)) else {
        return String::new();
    };
    let Ok(children) = xml_direct_child_ranges(xml, content_start, content_end) else {
        return String::new();
    };
    children
        .into_iter()
        .filter(|child| child.kind == "r")
        .map(|child| text_descendants(&xml[child.start..child.end]))
        .collect::<Vec<_>>()
        .join("")
}

fn text_descendants(fragment: &str) -> String {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut in_text = false;
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "t" => in_text = true,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => in_text = false,
            Ok(Event::Text(e)) if in_text => text.push_str(&decode_xml_text(e.as_ref())),
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    text
}

fn first_element_span(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Option<(usize, usize)> {
    let mut cursor = range_start;
    while cursor < range_end {
        let relative_start = xml[cursor..range_end].find('<')?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..range_end].find('>')?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('/') || token.starts_with('?') || token.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        let name = xml_token_name(token)?;
        let self_closing = token.trim_end().ends_with('/');
        if local_name(name.as_bytes()) == wanted {
            if self_closing {
                return Some((tag_start, tag_end + 1));
            }
            return find_matching_element_end(xml, wanted, tag_end + 1, range_end)
                .map(|end| (tag_start, end));
        }
        cursor = tag_end + 1;
    }
    None
}

fn find_matching_element_end(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Option<usize> {
    let mut depth = 1usize;
    let mut cursor = range_start;
    while cursor < range_end {
        let relative_start = xml[cursor..range_end].find('<')?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..range_end].find('>')?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('?') || token.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        if let Some(name) = xml_token_name(token)
            && local_name(name.as_bytes()) == wanted
        {
            if token.starts_with('/') {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(tag_end + 1);
                }
            } else if !token.trim_end().ends_with('/') {
                depth += 1;
            }
        }
        cursor = tag_end + 1;
    }
    None
}

fn content_start(xml: &str, span: (usize, usize)) -> Option<usize> {
    let tag_end = xml[span.0..span.1].find('>')?;
    Some(span.0 + tag_end + 1)
}

fn content_end(xml: &str, span: (usize, usize)) -> Option<usize> {
    let fragment = &xml[span.0..span.1];
    if fragment[..fragment.find('>')?].trim_end().ends_with('/') {
        return Some(span.0 + fragment.find('>')?);
    }
    let close_start = fragment.rfind("</")?;
    Some(span.0 + close_start)
}

fn start_tag_end(xml: &str, start: usize, end: usize) -> usize {
    xml[start..end]
        .find('>')
        .map(|offset| start + offset + 1)
        .unwrap_or(end)
}

fn attr_from_start_tag(start_tag: &str, name: &str) -> Option<String> {
    let mut reader = Reader::from_str(start_tag);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => return attr(&e, name),
            Ok(Event::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
    }
}
