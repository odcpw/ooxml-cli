use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Seek};
use zip::ZipArchive;

use crate::{attr_exact, local_name, resolve_relationship_target, zip_bytes};

use super::types::RelationshipRecord;
use super::util::{diag, file_extension, normalize_uri};

pub(super) const REL_TYPE_PACKAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package";

pub(super) fn check_chart_external_data_embedded_workbook_open(
    file: &str,
    part_uri: &str,
    label: &str,
    rid: &str,
    entry_set: &BTreeSet<String>,
    content_types: &BTreeMap<String, String>,
    rel_map: &BTreeMap<String, RelationshipRecord>,
) -> Vec<Value> {
    if rid.is_empty() {
        return Vec::new();
    }
    let Some(rel) = rel_map.get(rid) else {
        return Vec::new();
    };
    if rel.rel_type != REL_TYPE_PACKAGE || is_external(&rel.target_mode) {
        return Vec::new();
    }

    let target_uri = normalize_uri(&resolve_relationship_target(part_uri, &rel.target));
    if !entry_set.contains(&target_uri) {
        return Vec::new();
    }
    let content_type = content_types
        .get(&target_uri)
        .map(|value| value.trim())
        .unwrap_or_default();
    if !is_embedded_spreadsheet_package_content_type(content_type) {
        return Vec::new();
    }

    let entry_name = target_uri.trim_start_matches('/');
    let raw = match zip_bytes(file, entry_name) {
        Ok(raw) => raw,
        Err(err) => {
            return vec![diag(
                "OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN",
                format!(
                    "{part_uri} {label} relationship {rid} points to {target_uri} but embedded workbook could not be read: {}",
                    err.message
                ),
            )];
        }
    };
    let detected = match embedded_ooxml_package_type(&raw) {
        Ok(detected) => detected,
        Err(err) => {
            return vec![diag(
                "OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN",
                format!(
                    "{part_uri} {label} relationship {rid} points to {target_uri} but embedded workbook could not be opened as an OOXML package: {err}",
                ),
            )];
        }
    };
    if detected != "xlsx" {
        return vec![diag(
            "OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN",
            format!(
                "{part_uri} {label} relationship {rid} points to {target_uri} but embedded package type is {detected}, expected xlsx"
            ),
        )];
    }

    Vec::new()
}

fn embedded_ooxml_package_type(raw: &[u8]) -> Result<&'static str, String> {
    let cursor = Cursor::new(raw);
    let mut archive = ZipArchive::new(cursor).map_err(go_like_zip_open_error)?;
    let entries = embedded_zip_entries(&mut archive)?;
    let content_types = embedded_content_types(&mut archive)?;
    let root_relationships = embedded_root_relationships(&mut archive)?;

    for rel in root_relationships {
        let target_uri = resolve_relationship_target("/", &rel.target);
        let target_content_type = content_types.content_type_for_uri(&target_uri);

        if rel.rel_type.contains("presentationml.presentation")
            || target_content_type.contains("presentationml.presentation")
            || target_uri.starts_with("/ppt/")
        {
            return Ok("pptx");
        }
        if rel.rel_type.contains("wordprocessingml.document")
            || target_content_type.contains("wordprocessingml.document")
            || target_uri.starts_with("/word/")
        {
            return Ok("docx");
        }
        if rel.rel_type.contains("spreadsheetml.sheet")
            || target_content_type.contains("spreadsheetml.sheet")
            || target_uri.starts_with("/xl/")
        {
            return Ok("xlsx");
        }
    }

    for entry in entries {
        let content_type = content_types.content_type_for_uri(&entry);
        if content_type.contains("presentationml") {
            return Ok("pptx");
        }
        if content_type.contains("wordprocessingml") {
            return Ok("docx");
        }
        if content_type.contains("spreadsheetml") {
            return Ok("xlsx");
        }
    }

    Ok("unknown")
}

fn go_like_zip_open_error(err: zip::result::ZipError) -> String {
    let message = err.to_string();
    if message.contains("Could not find central directory")
        || message.contains("Invalid archive")
        || message.contains("invalid Zip archive")
    {
        "failed to read zip: zip: not a valid zip file".to_string()
    } else {
        format!("failed to read zip: {message}")
    }
}

fn embedded_zip_entries<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<Vec<String>, String> {
    let mut entries = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|err| format!("failed to read zip entry {index}: {err}"))?;
        if !entry.is_dir() {
            entries.push(normalize_uri(entry.name()));
        }
    }
    Ok(entries)
}

fn embedded_root_relationships<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<Vec<RelationshipRecord>, String> {
    let Some(xml) = embedded_zip_text(archive, "_rels/.rels")? else {
        return Ok(Vec::new());
    };
    embedded_relationship_records_from_xml(&xml)
}

#[derive(Default)]
struct EmbeddedContentTypes {
    defaults: BTreeMap<String, String>,
    overrides: BTreeMap<String, String>,
}

impl EmbeddedContentTypes {
    fn go_default() -> Self {
        Self {
            defaults: BTreeMap::from([
                (
                    "rels".to_string(),
                    "application/vnd.openxmlformats-package.relationships+xml".to_string(),
                ),
                ("xml".to_string(), "application/xml".to_string()),
            ]),
            overrides: BTreeMap::new(),
        }
    }

    fn content_type_for_uri(&self, uri: &str) -> String {
        let normalized = normalize_uri(uri);
        if let Some(content_type) = self.overrides.get(&normalized) {
            return content_type.clone();
        }
        self.defaults
            .get(file_extension(&normalized))
            .cloned()
            .unwrap_or_else(|| "application/octet-stream".to_string())
    }
}

fn embedded_content_types<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<EmbeddedContentTypes, String> {
    let Some(xml) = embedded_zip_text(archive, "[Content_Types].xml")? else {
        return Ok(EmbeddedContentTypes::go_default());
    };
    embedded_content_types_from_xml(&xml)
}

fn embedded_content_types_from_xml(xml: &str) -> Result<EmbeddedContentTypes, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut content_types = EmbeddedContentTypes::default();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local_name(e.name().as_ref()) {
                "Default" => {
                    let extension = attr_exact(&e, "Extension").unwrap_or_default();
                    let content_type = attr_exact(&e, "ContentType").unwrap_or_default();
                    content_types.defaults.insert(extension, content_type);
                }
                "Override" => {
                    let part_name = attr_exact(&e, "PartName").unwrap_or_default();
                    let content_type = attr_exact(&e, "ContentType").unwrap_or_default();
                    content_types
                        .overrides
                        .insert(normalize_uri(&part_name), content_type);
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(format!("failed to parse [Content_Types].xml: {err}"));
            }
            _ => {}
        }
    }

    Ok(content_types)
}

fn embedded_relationship_records_from_xml(xml: &str) -> Result<Vec<RelationshipRecord>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut rels = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Relationship" =>
            {
                rels.push(RelationshipRecord {
                    id: attr_exact(&e, "Id").unwrap_or_default(),
                    rel_type: attr_exact(&e, "Type").unwrap_or_default(),
                    target: attr_exact(&e, "Target").unwrap_or_default(),
                    target_mode: attr_exact(&e, "TargetMode").unwrap_or_default(),
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(format!("failed to parse relationships for /: {err}")),
            _ => {}
        }
    }
    Ok(rels)
}

fn embedded_zip_text<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Option<String>, String> {
    let mut entry = match archive.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(err) => return Err(format!("failed to open zip entry {name}: {err}")),
    };
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .map_err(|err| format!("failed to read zip entry {name}: {err}"))?;
    Ok(Some(text))
}

pub(super) fn is_embedded_spreadsheet_package_content_type(content_type: &str) -> bool {
    matches!(
        normalized_content_type(content_type).as_str(),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.template"
            | "application/vnd.ms-excel.sheet.macroenabled.12"
            | "application/vnd.ms-excel.template.macroenabled.12"
            | "application/vnd.ms-excel.sheet.binary.macroenabled.12"
    )
}

fn normalized_content_type(content_type: &str) -> String {
    let content_type = content_type.trim().to_ascii_lowercase();
    content_type
        .split_once(';')
        .map(|(base, _)| base.trim().to_string())
        .unwrap_or(content_type)
}

fn is_external(target_mode: &str) -> bool {
    target_mode.trim().eq_ignore_ascii_case("External")
}
