use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::{CliError, CliResult, attr, attr_exact, local_name, xml_attr_escape, zip_text};

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
                    defaults.insert(extension.to_ascii_lowercase(), content_type);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Override" =>
            {
                if let (Some(part_name), Some(content_type)) =
                    (attr(&e, "PartName"), attr(&e, "ContentType"))
                {
                    overrides.insert(opc_part_lookup_key(&part_name)?, content_type);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    let normalized_lookup_key = opc_part_lookup_key(normalized)?;
    if let Some(content_type) = overrides.get(&normalized_lookup_key) {
        return Ok(content_type.clone());
    }
    let extension = Path::new(normalized)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    Ok(defaults
        .get(&extension.to_ascii_lowercase())
        .cloned()
        .unwrap_or_default())
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

pub(crate) fn ensure_package_root_relationship_xml(
    xml: String,
    rel_type: &str,
    target_uri: &str,
) -> String {
    let rels = relationship_entries_from_xml(&xml);
    if rels.iter().any(|rel| rel.rel_type == rel_type) {
        return xml;
    }
    let next_id = allocate_relationship_id(&rels);
    let rel = format!(
        r#"<Relationship Id="{next_id}" Type="{}" Target="{}"/>"#,
        xml_attr_escape(rel_type),
        xml_attr_escape(target_uri.trim_start_matches('/'))
    );
    if let Some(pos) = xml.rfind("</Relationships>") {
        let mut out = String::with_capacity(xml.len() + rel.len());
        out.push_str(&xml[..pos]);
        out.push_str(&rel);
        out.push_str(&xml[pos..]);
        out
    } else {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rel}</Relationships>"#
        )
    }
}

pub(crate) fn ensure_content_type_override(
    xml: String,
    part_name: &str,
    content_type: &str,
) -> CliResult<String> {
    let normalized = format!("/{}", part_name.trim_start_matches('/'));
    if content_type_override_exists(&xml, &normalized)? {
        return Ok(xml);
    }
    let override_xml = format!(
        r#"<Override PartName="{normalized}" ContentType="{}"/>"#,
        xml_attr_escape(content_type)
    );
    let pos = content_types_root_close_start(&xml)?;
    let mut out = String::with_capacity(xml.len() + override_xml.len());
    out.push_str(&xml[..pos]);
    out.push_str(&override_xml);
    out.push_str(&xml[pos..]);
    Ok(out)
}

pub(crate) fn add_relationship_to_xml(
    xml: String,
    id: &str,
    rel_type: &str,
    target: &str,
) -> String {
    let rel = format!(
        r#"<Relationship Id="{}" Type="{}" Target="{}"/>"#,
        xml_attr_escape(id),
        xml_attr_escape(rel_type),
        xml_attr_escape(target)
    );
    if let Some(pos) = xml.rfind("</Relationships>") {
        let mut out = String::with_capacity(xml.len() + rel.len());
        out.push_str(&xml[..pos]);
        out.push_str(&rel);
        out.push_str(&xml[pos..]);
        out
    } else {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rel}</Relationships>"#
        )
    }
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

pub(crate) fn resolve_relationship_target_part_uri(
    source_uri: &str,
    target: &str,
) -> CliResult<String> {
    percent_decode_package_uri(&resolve_relationship_target(source_uri, target))
}

pub(crate) fn opc_part_lookup_key(uri: &str) -> CliResult<String> {
    percent_decode_package_uri(&normalize_package_uri(&uri.replace('\\', "/")))
        .map(|uri| uri.to_ascii_lowercase())
}

pub(crate) fn opc_part_lookup_set(entries: &[String]) -> BTreeSet<String> {
    entries
        .iter()
        .map(|entry| {
            opc_part_lookup_key(entry).unwrap_or_else(|_| {
                normalize_package_uri(&entry.replace('\\', "/")).to_ascii_lowercase()
            })
        })
        .collect()
}

pub(crate) fn relationship_target_from_source_to_target(
    source_uri: &str,
    target_uri: &str,
) -> String {
    let source = source_uri.trim_start_matches('/');
    let target = target_uri.trim_start_matches('/');
    let source_dirs: Vec<&str> = source
        .rsplit_once('/')
        .map(|(dir, _)| dir.split('/').filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    let target_parts: Vec<&str> = target.split('/').filter(|part| !part.is_empty()).collect();
    let common = source_dirs
        .iter()
        .zip(target_parts.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let mut parts = Vec::new();
    for _ in common..source_dirs.len() {
        parts.push("..".to_string());
    }
    for part in target_parts.iter().skip(common) {
        parts.push((*part).to_string());
    }
    if parts.is_empty() {
        target.rsplit('/').next().unwrap_or(target).to_string()
    } else {
        parts.join("/")
    }
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

fn content_type_override_exists(xml: &str, normalized_part_name: &str) -> CliResult<bool> {
    let wanted = opc_part_lookup_key(normalized_part_name)?;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Override" =>
            {
                if let Some(part_name) = attr(&e, "PartName")
                    && opc_part_lookup_key(&part_name)? == wanted
                {
                    return Ok(true);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(false)
}

fn content_types_root_close_start(xml: &str) -> CliResult<usize> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let root_name = loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "Types" => {
                break String::from_utf8_lossy(e.name().as_ref()).into_owned();
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "Types" => {
                return Err(CliError::unexpected(
                    "[Content_Types].xml Types root is self-closing; cannot insert Override",
                ));
            }
            Ok(Event::Eof) => {
                return Err(CliError::unexpected(
                    "[Content_Types].xml Types root element not found",
                ));
            }
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    };
    let mut depth = 1usize;
    loop {
        let event_start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(e)) => {
                if depth == 1 && e.name().as_ref() == root_name.as_bytes() {
                    return Ok(event_start);
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::unexpected(
        "[Content_Types].xml Types root closing tag not found",
    ))
}

fn percent_decode_package_uri(uri: &str) -> CliResult<String> {
    let bytes = uri.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let Some(&hi) = bytes.get(index + 1) else {
                return Err(invalid_percent_escape(uri));
            };
            let Some(&lo) = bytes.get(index + 2) else {
                return Err(invalid_percent_escape(uri));
            };
            let Some(hi) = hex_value(hi) else {
                return Err(invalid_percent_escape(uri));
            };
            let Some(lo) = hex_value(lo) else {
                return Err(invalid_percent_escape(uri));
            };
            decoded.push((hi << 4) | lo);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| {
        CliError::unexpected(format!(
            "invalid UTF-8 percent-encoded OPC part URI {uri:?}"
        ))
    })
}

fn invalid_percent_escape(uri: &str) -> CliError {
    CliError::unexpected(format!("invalid percent escape in OPC part URI {uri:?}"))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_content_type_override_detects_legal_existing_override_serializations() {
        let xml = r#"<?xml version='1.0' encoding='UTF-8'?>
<Types xmlns='http://schemas.openxmlformats.org/package/2006/content-types'>
  <Override ContentType='application/xml' PartName='/word/styles.xml'/>
</Types>"#;

        let updated = ensure_content_type_override(
            xml.to_string(),
            "/WORD/STYLES.XML",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml",
        )
        .expect("existing override detection");

        assert_eq!(
            updated.matches("<Override").count(),
            1,
            "existing single-quoted, reordered override should not be duplicated: {updated}"
        );
        assert!(!updated.contains("wordprocessingml.styles+xml"));
    }

    #[test]
    fn ensure_content_type_override_refuses_self_closing_types_root() {
        let err = ensure_content_type_override(
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#
                .to_string(),
            "/word/styles.xml",
            "application/xml",
        )
        .expect_err("self-closing Types root cannot be spliced safely");

        assert!(
            err.message.contains("self-closing"),
            "error should explain unsafe content-types XML shape: {}",
            err.message
        );
    }

    #[test]
    fn ensure_content_type_override_inserts_after_spaced_empty_child() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ct:Types xmlns:ct="http://schemas.openxmlformats.org/package/2006/content-types"><ct:Override PartName="/xl/tables/table1.xml" ContentType="application/xml" /></ct:Types>"#;
        let updated = ensure_content_type_override(
            xml.to_string(),
            "/xl/pivotTables/pivotTable1.xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml",
        )
        .expect("insert override after spaced empty child");
        assert!(
            updated.contains(
                r#"ContentType="application/xml" /><Override PartName="/xl/pivotTables/pivotTable1.xml""#
            ),
            "override was inserted inside the prior empty element: {updated}"
        );
        assert!(updated.ends_with("</ct:Types>"));
        let mut reader = Reader::from_str(&updated);
        loop {
            match reader.read_event() {
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(err) => panic!("updated content types must remain well-formed: {err}"),
            }
        }
    }

    #[test]
    fn ensure_content_type_override_ignores_fake_root_close_in_trailing_comment() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ct:Types xmlns:ct="http://schemas.openxmlformats.org/package/2006/content-types"><ct:Default Extension="xml" ContentType="application/xml"/></ct:Types><!-- fake </ct:Types> -->"#;
        let updated = ensure_content_type_override(
            xml.to_string(),
            "/xl/pivotTables/pivotTable1.xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml",
        )
        .expect("insert override before parsed root close");
        let override_pos = updated.find("<Override").expect("inserted Override");
        let root_close_pos = updated.find("</ct:Types>").expect("real root close");
        let comment_pos = updated.find("<!-- fake").expect("trailing comment");
        assert!(
            override_pos < root_close_pos,
            "Override must be inside Types: {updated}"
        );
        assert!(
            root_close_pos < comment_pos,
            "trailing comment must remain outside Types: {updated}"
        );
        let mut reader = Reader::from_str(&updated);
        loop {
            match reader.read_event() {
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(err) => panic!("updated content types must remain well-formed: {err}"),
            }
        }
    }

    #[test]
    fn opc_part_lookup_key_percent_decodes_and_ascii_folds() {
        assert_eq!(
            opc_part_lookup_key("/PPT/media/caf%C3%A9.PNG").expect("lookup key"),
            "/ppt/media/café.png"
        );
    }
}
