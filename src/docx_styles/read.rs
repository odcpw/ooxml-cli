use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, InspectPackageKind, attr, content_type_for_part,
    detect_inspect_package_type, find_docx_document_part, is_docx_styles_part, local_name,
    relationship_entries, relationships_part_for, resolve_relationship_target, zip_entry_names,
    zip_text,
};

pub(crate) fn docx_styles_list(file: &str, style_type: Option<&str>) -> CliResult<Value> {
    let style_type = normalize_docx_style_type(style_type)?;
    let (document_part, styles_part) = docx_document_and_styles_parts(file)?;
    let mut styles = Vec::new();
    if let Some(styles_part) = styles_part.as_deref() {
        styles = docx_styles(file, styles_part)?;
        if let Some(style_type) = style_type.as_deref() {
            styles.retain(|style| style.style_type == style_type);
        }
    }
    let counts = docx_style_id_counts(&styles);
    let styles_json: Vec<Value> = styles
        .iter()
        .map(|style| docx_style_json(style, &counts))
        .collect();
    Ok(json!({
        "file": file,
        "documentPartUri": document_part,
        "stylesPartUri": styles_part,
        "count": styles_json.len(),
        "styles": styles_json,
    }))
}

pub(crate) fn docx_styles_show(file: &str, style_id: &str) -> CliResult<Value> {
    let (document_part, styles_part) = docx_document_and_styles_parts(file)?;
    let mut style_json = Value::Null;
    let mut found = false;
    if let Some(styles_part) = styles_part.as_deref() {
        let styles = docx_styles(file, styles_part)?;
        let counts = docx_style_id_counts(&styles);
        if let Some(style) = styles.iter().find(|style| style.style_id == style_id) {
            style_json = docx_style_json(style, &counts);
            found = true;
        }
    }
    Ok(json!({
        "file": file,
        "documentPartUri": document_part,
        "stylesPartUri": styles_part,
        "styleId": style_id,
        "found": found,
        "style": style_json,
    }))
}

fn normalize_docx_style_type(value: Option<&str>) -> CliResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_lowercase();
    match normalized.as_str() {
        "paragraph" | "character" | "table" | "numbering" => Ok(Some(normalized)),
        _ => Err(CliError::invalid_args(
            "--type must be one of paragraph, character, table, numbering",
        )),
    }
}

pub(super) fn docx_document_and_styles_parts(file: &str) -> CliResult<(String, Option<String>)> {
    let entries = zip_entry_names(file)?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        return Err(CliError::unsupported_type(
            "file is not a DOCX document (detected: unknown)",
        ));
    }
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let styles_uri = find_docx_styles_part(file, &entries, &document_part)?;
    Ok((document_uri, styles_uri))
}

fn find_docx_styles_part(
    file: &str,
    entries: &[String],
    document_part: &str,
) -> CliResult<Option<String>> {
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let rels_part = relationships_part_for(document_part);
    for rel in relationship_entries(file, &rels_part).unwrap_or_default() {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
            || rel.rel_type.ends_with("/styles")
        {
            return Ok(Some(resolve_relationship_target(
                &document_uri,
                &rel.target,
            )));
        }
    }
    for entry in entries {
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        let uri = format!("/{}", entry.trim_start_matches('/'));
        if is_docx_styles_part(&uri, &content_type) {
            return Ok(Some(uri));
        }
    }
    Ok(None)
}

#[derive(Clone, Default)]
pub(super) struct DocxStyleInfo {
    pub(super) style_id: String,
    pub(super) name: String,
    pub(super) style_type: String,
    pub(super) default: bool,
    pub(super) builtin: bool,
    pub(super) based_on: String,
    pub(super) next: String,
}

pub(super) fn docx_styles(file: &str, styles_part: &str) -> CliResult<Vec<DocxStyleInfo>> {
    let xml = zip_text(file, styles_part.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut saw_root = false;
    let mut current: Option<DocxStyleInfo> = None;
    let mut styles = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "styles" {
                        return Err(CliError::unexpected(format!(
                            "styles part {styles_part} root is {name:?}, expected styles"
                        )));
                    }
                } else if name == "style" {
                    current = Some(docx_style_from_element(&e));
                } else {
                    docx_note_style_child(&e, &name, &mut current);
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "styles" {
                        return Err(CliError::unexpected(format!(
                            "styles part {styles_part} root is {name:?}, expected styles"
                        )));
                    }
                } else if name == "style" {
                    styles.push(docx_style_from_element(&e));
                } else {
                    docx_note_style_child(&e, &name, &mut current);
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "style" => {
                if let Some(style) = current.take() {
                    styles.push(style);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !saw_root {
        return Err(CliError::unexpected(format!(
            "styles part {styles_part} has no root element"
        )));
    }
    Ok(styles)
}

fn docx_style_from_element(element: &BytesStart<'_>) -> DocxStyleInfo {
    DocxStyleInfo {
        style_id: attr(element, "styleId").unwrap_or_default(),
        style_type: attr(element, "type").unwrap_or_default(),
        default: docx_on_off_attr(element, "default"),
        builtin: !docx_on_off_attr(element, "customStyle"),
        ..DocxStyleInfo::default()
    }
}

fn docx_note_style_child(
    element: &BytesStart<'_>,
    name: &str,
    current: &mut Option<DocxStyleInfo>,
) {
    let Some(style) = current.as_mut() else {
        return;
    };
    let Some(value) = attr(element, "val") else {
        return;
    };
    match name {
        "name" => style.name = value,
        "basedOn" => style.based_on = value,
        "next" => style.next = value,
        _ => {}
    }
}

fn docx_on_off_attr(element: &BytesStart<'_>, name: &str) -> bool {
    match attr(element, name).as_deref() {
        None => false,
        Some("0" | "false" | "off") => false,
        Some(_) => true,
    }
}

fn docx_style_id_counts(styles: &[DocxStyleInfo]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for style in styles {
        if !style.style_id.is_empty() {
            *counts.entry(style.style_id.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn docx_style_json(style: &DocxStyleInfo, counts: &BTreeMap<String, usize>) -> Value {
    let mut object = Map::new();
    object.insert("styleId".to_string(), json!(style.style_id));
    if !style.name.is_empty() {
        object.insert("name".to_string(), json!(style.name));
    }
    if !style.style_type.is_empty() {
        object.insert("type".to_string(), json!(style.style_type));
    }
    object.insert("default".to_string(), json!(style.default));
    object.insert("builtin".to_string(), json!(style.builtin));
    if !style.based_on.is_empty() {
        object.insert("basedOn".to_string(), json!(style.based_on));
    }
    if !style.next.is_empty() {
        object.insert("next".to_string(), json!(style.next));
    }
    if !style.style_id.is_empty() {
        object.insert("primarySelector".to_string(), json!(style.style_id));
        object.insert("selectors".to_string(), json!([style.style_id]));
        if counts.get(&style.style_id).copied().unwrap_or_default() == 1 {
            object.insert(
                "handle".to_string(),
                json!(format!("H:docx/pt:styles/style:n:{}", style.style_id)),
            );
        }
    }
    Value::Object(object)
}
