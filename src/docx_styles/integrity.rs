use quick_xml::NsReader;
use quick_xml::events::Event;
use serde_json::{Value, json};
use std::collections::BTreeSet;

use super::read::docx_document_and_styles_parts;
use crate::{
    CliError, CliResult, DOCX_W_NS, attr, content_type_for_part, element_in_ns,
    is_docx_numbering_part, local_name, relationship_entries, relationships_part_for,
    resolve_relationship_target, zip_text,
};

const NUMBERING_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";

pub(crate) fn validate_docx_style_integrity(
    file: &str,
    entries: &[String],
) -> CliResult<Vec<Value>> {
    let (document_uri, styles_part) = docx_document_and_styles_parts(file)?;
    let style_ids = match styles_part.as_deref() {
        Some(part) => defined_style_ids(&zip_text(file, part.trim_start_matches('/'))?)?,
        None => BTreeSet::new(),
    };
    let numbering_part = find_numbering_part(file, entries, &document_uri)?;
    let numbering_ids = match numbering_part.as_deref() {
        Some(part) => defined_numbering_ids(&zip_text(file, part.trim_start_matches('/'))?)?,
        None => BTreeSet::new(),
    };

    let mut dangling_styles = BTreeSet::<(String, String, String)>::new();
    let mut dangling_numbering = BTreeSet::<(String, u32)>::new();
    for entry in entries
        .iter()
        .filter(|entry| entry.ends_with(".xml") && !entry.contains("/_rels/"))
    {
        let xml = zip_text(file, entry)?;
        collect_references(
            &xml,
            entry,
            &style_ids,
            &numbering_ids,
            &mut dangling_styles,
            &mut dangling_numbering,
        )?;
    }

    let mut diagnostics = Vec::with_capacity(dangling_styles.len() + dangling_numbering.len());
    for (part, element, style_id) in dangling_styles {
        diagnostics.push(json!({
            "code": "DOCX_DANGLING_STYLE",
            "severity": "error",
            "message": format!("{element} references undefined DOCX style {style_id:?} in {part}"),
            "part": part,
            "element": element,
            "styleId": style_id,
            "check": "style-integrity",
        }));
    }
    for (part, num_id) in dangling_numbering {
        diagnostics.push(json!({
            "code": "DOCX_DANGLING_NUMBERING",
            "severity": "error",
            "message": format!("numId references undefined DOCX numbering instance {num_id} in {part}"),
            "part": part,
            "element": "numId",
            "numId": num_id,
            "check": "style-integrity",
        }));
    }
    Ok(diagnostics)
}

fn defined_style_ids(xml: &str) -> CliResult<BTreeSet<String>> {
    let mut reader = NsReader::from_str(xml);
    let mut ids = BTreeSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if local_name(element.name().as_ref()) == "style"
                    && element_in_ns(reader.resolver(), &element, DOCX_W_NS) =>
            {
                if let Some(style_id) = attr(&element, "styleId").filter(|id| !id.is_empty()) {
                    ids.insert(style_id);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse DOCX styles part: {error}"
                )));
            }
            _ => {}
        }
    }
    Ok(ids)
}

fn defined_numbering_ids(xml: &str) -> CliResult<BTreeSet<u32>> {
    let mut reader = NsReader::from_str(xml);
    let mut ids = BTreeSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if local_name(element.name().as_ref()) == "num"
                    && element_in_ns(reader.resolver(), &element, DOCX_W_NS) =>
            {
                if let Some(num_id) = attr(&element, "numId").and_then(|id| id.parse().ok()) {
                    ids.insert(num_id);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse DOCX numbering part: {error}"
                )));
            }
            _ => {}
        }
    }
    Ok(ids)
}

fn collect_references(
    xml: &str,
    entry: &str,
    style_ids: &BTreeSet<String>,
    numbering_ids: &BTreeSet<u32>,
    dangling_styles: &mut BTreeSet<(String, String, String)>,
    dangling_numbering: &mut BTreeSet<(String, u32)>,
) -> CliResult<()> {
    let part = format!("/{}", entry.trim_start_matches('/'));
    let mut reader = NsReader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if element_in_ns(reader.resolver(), &element, DOCX_W_NS) =>
            {
                let qualified_name = element.name();
                let element_name = local_name(qualified_name.as_ref());
                if matches!(element_name, "pStyle" | "rStyle" | "tblStyle") {
                    if let Some(style_id) = attr(&element, "val")
                        && !style_id.is_empty()
                        && !style_ids.contains(&style_id)
                    {
                        dangling_styles.insert((part.clone(), element_name.to_string(), style_id));
                    }
                } else if element_name == "numId"
                    && let Some(num_id) = attr(&element, "val").and_then(|id| id.parse().ok())
                    && num_id != 0
                    && !numbering_ids.contains(&num_id)
                {
                    dangling_numbering.insert((part.clone(), num_id));
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse DOCX XML part {part}: {error}"
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

fn find_numbering_part(
    file: &str,
    entries: &[String],
    document_uri: &str,
) -> CliResult<Option<String>> {
    let document_part = document_uri.trim_start_matches('/');
    let rels_part = relationships_part_for(document_part);
    for relationship in relationship_entries(file, &rels_part).unwrap_or_default() {
        if relationship.target_mode != "External"
            && (relationship.rel_type == NUMBERING_REL
                || relationship.rel_type.ends_with("/numbering"))
        {
            return Ok(Some(resolve_relationship_target(
                document_uri,
                &relationship.target,
            )));
        }
    }
    for entry in entries {
        let uri = format!("/{}", entry.trim_start_matches('/'));
        let content_type = content_type_for_part(file, &uri).unwrap_or_default();
        if is_docx_numbering_part(&uri, &content_type) {
            return Ok(Some(uri));
        }
    }
    Ok(None)
}
