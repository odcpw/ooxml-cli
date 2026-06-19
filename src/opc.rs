use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::BTreeMap;
use std::path::Path;

use crate::{CliError, CliResult, attr, attr_exact, local_name, zip_text};

#[derive(Clone)]
pub(crate) struct RelationshipEntry {
    pub(crate) id: String,
    pub(crate) rel_type: String,
    pub(crate) target: String,
    pub(crate) target_mode: String,
}

pub(crate) fn relationships_part_for(part: &str) -> String {
    let normalized = part.trim_start_matches('/');
    if let Some((dir, name)) = normalized.rsplit_once('/') {
        format!("{dir}/_rels/{name}.rels")
    } else {
        format!("_rels/{normalized}.rels")
    }
}

pub(crate) fn relationship_source_uri(rels_part: &str) -> String {
    if rels_part == "_rels/.rels" {
        return "/".to_string();
    }
    let normalized = rels_part.trim_start_matches('/');
    if let Some((dir, file_name)) = normalized.rsplit_once("/_rels/")
        && let Some(source_name) = file_name.strip_suffix(".rels")
    {
        return format!("/{dir}/{source_name}");
    }
    "/".to_string()
}

pub(crate) fn relationships(file: &str, part: &str) -> CliResult<BTreeMap<String, String>> {
    Ok(relationship_entries(file, part)?
        .into_iter()
        .map(|rel| (rel.id, rel.target))
        .collect())
}

pub(crate) fn relationship_entries(file: &str, part: &str) -> CliResult<Vec<RelationshipEntry>> {
    let xml = zip_text(file, part)?;
    relationship_entries_from_xml_result(&xml)
}

pub(crate) fn relationship_entries_from_xml(xml: &str) -> Vec<RelationshipEntry> {
    relationship_entries_from_xml_result(xml).unwrap_or_default()
}

fn relationship_entries_from_xml_result(xml: &str) -> CliResult<Vec<RelationshipEntry>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut rels = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Relationship" =>
            {
                if let (Some(id), Some(target)) = (attr_exact(&e, "Id"), attr_exact(&e, "Target")) {
                    rels.push(RelationshipEntry {
                        id,
                        rel_type: attr_exact(&e, "Type").unwrap_or_default(),
                        target,
                        target_mode: attr_exact(&e, "TargetMode").unwrap_or_default(),
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(rels)
}

pub(crate) fn content_type_for_part(file: &str, part_uri: &str) -> CliResult<String> {
    let normalized = part_uri.trim_start_matches('/');
    let xml = zip_text(file, "[Content_Types].xml")?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut defaults = BTreeMap::<String, String>::new();
    let mut overrides = BTreeMap::<String, String>::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Default" =>
            {
                if let (Some(extension), Some(content_type)) =
                    (attr(&e, "Extension"), attr(&e, "ContentType"))
                {
                    defaults.insert(extension, content_type);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Override" =>
            {
                if let (Some(part_name), Some(content_type)) =
                    (attr(&e, "PartName"), attr(&e, "ContentType"))
                {
                    overrides.insert(part_name.trim_start_matches('/').to_string(), content_type);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if let Some(content_type) = overrides.get(normalized) {
        return Ok(content_type.clone());
    }
    let extension = Path::new(normalized)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    Ok(defaults.get(extension).cloned().unwrap_or_default())
}

pub(crate) fn allocate_relationship_id(rels: &[RelationshipEntry]) -> String {
    let mut next = 1u32;
    for rel in rels {
        if let Some(suffix) = rel.id.strip_prefix("rId")
            && let Ok(id) = suffix.parse::<u32>()
            && id >= next
        {
            next = id + 1;
        }
    }
    format!("rId{next}")
}

pub(crate) fn resolve_relationship_target(source_uri: &str, target: &str) -> String {
    if target.starts_with('/') {
        return format!("/{}", target.trim_start_matches('/'));
    }
    let source = source_uri.trim_start_matches('/');
    let base = if source.is_empty() {
        String::new()
    } else if source.ends_with('/') {
        source.to_string()
    } else if let Some((dir, _)) = source.rsplit_once('/') {
        format!("{dir}/")
    } else {
        String::new()
    };
    normalize_package_uri(&format!("{base}{target}"))
}

fn normalize_package_uri(uri: &str) -> String {
    let mut parts = Vec::new();
    for part in uri.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}
