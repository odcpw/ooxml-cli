use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CliResult, attr_exact, local_name, relationship_source_uri, relationships_part_for,
    resolve_relationship_target, zip_text,
};

use super::spec::expected_relationship_target_content_types;
use super::types::{PartInfo, RelationshipRecord};
use super::util::{diag, is_rels_uri, normalize_uri};

pub(super) fn check_package_relationship_closure(
    file: &str,
    entries: &[String],
    entry_set: &BTreeSet<String>,
    parts: &[PartInfo],
) -> CliResult<Vec<Value>> {
    let mut diagnostics = Vec::new();
    let mut relationship_sources = BTreeSet::from(["/".to_string()]);
    let content_types: BTreeMap<String, String> = parts
        .iter()
        .map(|part| (part.uri.clone(), part.content_type.clone()))
        .collect();

    for part in parts {
        if is_rels_uri(&part.uri) {
            let source_uri = relationship_source_uri(&part.entry_name);
            relationship_sources.insert(source_uri.clone());
            if source_uri != "/" && !entry_set.contains(&source_uri) {
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
            if !rel.target_mode.is_empty() && rel.target_mode != "External" {
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
            let target_uri = normalize_uri(&resolve_relationship_target(&source_uri, &rel.target));
            if !entry_set.contains(&target_uri) {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_TARGET_MISSING",
                    format!("{label} points to missing part {target_uri}"),
                ));
                continue;
            }
            let expected =
                expected_relationship_target_content_types(&source_uri, &target_uri, &rel.rel_type);
            if !expected.is_empty() {
                let actual = content_types.get(&target_uri).cloned().unwrap_or_default();
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
) -> Result<Vec<RelationshipRecord>, String> {
    let xml = zip_text(file, entry_name).map_err(|err| err.message)?;
    relationship_records_from_xml(&xml)
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
