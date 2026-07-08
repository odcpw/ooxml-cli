use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CliResult, attr_exact, local_name, opc_part_lookup_key, opc_part_lookup_set,
    relationship_source_uri, relationships_part_for, resolve_relationship_target_part_uri,
    zip_text,
};

use super::spec::expected_relationship_target_content_types;
use super::types::{PartInfo, RelationshipRecord};
use super::util::{diag, is_rels_uri, normalize_uri};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RelationshipPartError {
    Read(String),
    Parse(String),
}

pub(super) fn check_package_relationship_closure(
    file: &str,
    entries: &[String],
    _entry_set: &BTreeSet<String>,
    parts: &[PartInfo],
) -> CliResult<Vec<Value>> {
    let mut diagnostics = Vec::new();
    let mut relationship_sources = BTreeSet::from(["/".to_string()]);
    let entry_lookup_set = opc_part_lookup_set(entries);
    let content_types: BTreeMap<String, String> = parts
        .iter()
        .map(|part| {
            (
                opc_part_lookup_key(&part.uri).unwrap_or_else(|_| part.uri.to_ascii_lowercase()),
                part.content_type.clone(),
            )
        })
        .collect();

    for part in parts {
        if is_rels_uri(&part.uri) {
            let source_uri = relationship_source_uri(&part.entry_name);
            relationship_sources.insert(source_uri.clone());
            if source_uri != "/"
                && !entry_lookup_set.contains(
                    &opc_part_lookup_key(&source_uri)
                        .unwrap_or_else(|_| source_uri.to_ascii_lowercase()),
                )
            {
                diagnostics.push(diag(
                    "OOXML_RELS_ORPHANED",
                    format!(
                        "{} is a relationships part for missing source part {source_uri}",
                        part.uri
                    ),
                ));
            }
        } else {
            relationship_sources.insert(part.uri.clone());
        }
    }

    for source_uri in relationship_sources {
        let rels_part = rels_part_for_source(&source_uri);
        if !entries.iter().any(|entry| entry == &rels_part) {
            continue;
        }
        let rels = match parse_relationship_part(file, &rels_part) {
            Ok(rels) => rels,
            Err(_) => continue,
        };
        let mut seen_ids = BTreeSet::new();
        for rel in rels {
            let label = relationship_label(&source_uri, &rel);
            let trimmed_id = rel.id.trim();
            if trimmed_id.is_empty() {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_MISSING_ID",
                    format!("{label} is missing Id"),
                ));
            } else if seen_ids.contains(trimmed_id) {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_DUPLICATE_ID",
                    format!("{label} duplicates Id {}", rel.id),
                ));
            }
            seen_ids.insert(rel.id.clone());

            if rel.rel_type.trim().is_empty() {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_MISSING_TYPE",
                    format!("{label} is missing Type"),
                ));
            }
            if rel.target.trim().is_empty() {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_MISSING_TARGET",
                    format!("{label} is missing Target"),
                ));
                continue;
            }
            if !matches!(rel.target_mode.as_str(), "" | "Internal" | "External") {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_TARGET_MODE",
                    format!("{label} has unsupported TargetMode {:?}", rel.target_mode),
                ));
            }
            if rel.target_mode == "External" {
                continue;
            }
            if looks_external_relationship_target(&rel.target) {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_EXTERNAL_MODE_MISSING",
                    format!(
                        "{label} target {:?} looks external but TargetMode is not External",
                        rel.target
                    ),
                ));
                continue;
            }
            let target_uri = match resolve_relationship_target_part_uri(&source_uri, &rel.target) {
                Ok(target_uri) => normalize_uri(&target_uri),
                Err(err) => {
                    diagnostics.push(diag(
                        "OOXML_RELATIONSHIP_TARGET_MALFORMED",
                        format!(
                            "{label} has malformed Target {:?}: {}",
                            rel.target, err.message
                        ),
                    ));
                    continue;
                }
            };
            let target_key = opc_part_lookup_key(&target_uri)?;
            if !entry_lookup_set.contains(&target_key) {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_TARGET_MISSING",
                    format!("{label} points to missing part {target_uri}"),
                ));
                continue;
            }
            let expected =
                expected_relationship_target_content_types(&source_uri, &target_uri, &rel.rel_type);
            if !expected.is_empty() {
                let actual = content_types.get(&target_key).cloned().unwrap_or_default();
                if !expected.contains(&actual.as_str()) {
                    diagnostics.push(diag(
                        "OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE",
                        format!(
                            "{label} has type {:?} but target {target_uri} has content type {:?}; expected one of: {}",
                            rel.rel_type,
                            actual,
                            expected.join(", ")
                        ),
                    ));
                }
            }
        }
    }

    Ok(diagnostics)
}

pub(super) fn parse_relationship_part(
    file: &str,
    entry_name: &str,
) -> Result<Vec<RelationshipRecord>, RelationshipPartError> {
    let xml = zip_text(file, entry_name).map_err(|err| RelationshipPartError::Read(err.message))?;
    relationship_records_from_xml(&xml).map_err(RelationshipPartError::Parse)
}

fn relationship_records_from_xml(xml: &str) -> Result<Vec<RelationshipRecord>, String> {
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
            Err(err) => return Err(err.to_string()),
            _ => {}
        }
    }
    Ok(rels)
}

fn rels_part_for_source(source_uri: &str) -> String {
    if normalize_uri(source_uri) == "/" {
        "_rels/.rels".to_string()
    } else {
        relationships_part_for(source_uri.trim_start_matches('/'))
    }
}

fn relationship_label(source_uri: &str, rel: &RelationshipRecord) -> String {
    if rel.id.is_empty() {
        format!("{source_uri} relationship")
    } else {
        format!("{source_uri} relationship {}", rel.id)
    }
}

fn looks_external_relationship_target(target: &str) -> bool {
    let lowered = target.trim().to_ascii_lowercase();
    lowered.contains("://")
        || lowered.starts_with("mailto:")
        || lowered.starts_with("file:")
        || lowered.starts_with("urn:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    #[test]
    fn parse_relationship_part_classifies_zip_read_errors() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ooxml-relationship-read-error-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let package = temp_dir.join("bad-rels.xlsx");
        write_relationship_package(&package);
        corrupt_zip_entry_payload(&package, "xl/_rels/workbook.xml.rels");

        let err = match parse_relationship_part(
            package.to_str().expect("package path"),
            "xl/_rels/workbook.xml.rels",
        ) {
            Ok(_) => panic!("corrupt compressed relationship stream should be a read error"),
            Err(err) => err,
        };

        match err {
            RelationshipPartError::Read(message) => {
                assert!(
                    message.contains("failed to read zip entry xl/_rels/workbook.xml.rels"),
                    "read-error message should preserve the zip entry name: {message}"
                );
            }
            RelationshipPartError::Parse(message) => {
                panic!("expected read error, got parse error: {message}");
            }
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    fn write_relationship_package(path: &Path) {
        let output = File::create(path).expect("create package");
        let mut writer = ZipWriter::new(output);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer
            .start_file("xl/_rels/workbook.xml.rels", options)
            .expect("start rels");
        writer
            .write_all(
                br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
            )
            .expect("write rels");
        writer.finish().expect("finish package");
    }

    fn corrupt_zip_entry_payload(path: &Path, target_name: &str) {
        let mut data = fs::read(path).expect("read zip");
        let target = target_name.as_bytes();
        let mut offset = 0;
        while offset + 30 <= data.len() {
            if data[offset..].starts_with(&[0x50, 0x4b, 0x03, 0x04]) {
                let name_len = read_u16_le(&data, offset + 26) as usize;
                let extra_len = read_u16_le(&data, offset + 28) as usize;
                let name_start = offset + 30;
                let name_end = name_start.saturating_add(name_len);
                let data_start = name_end.saturating_add(extra_len);
                if data_start <= data.len() && &data[name_start..name_end] == target {
                    data[data_start] ^= 0xff;
                    fs::write(path, data).expect("write corrupt zip");
                    return;
                }
                offset = data_start.max(offset + 1);
            } else {
                offset += 1;
            }
        }
        panic!("missing local zip entry {target_name}");
    }

    fn read_u16_le(data: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([data[offset], data[offset + 1]])
    }
}
