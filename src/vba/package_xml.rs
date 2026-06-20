use quick_xml::Reader;
use quick_xml::events::Event;

use crate::{
    RelationshipEntry, add_relationship_to_xml, allocate_relationship_id, attr,
    content_type_for_part, local_name, relationship_target_from_source_to_target,
    resolve_relationship_target,
};

use super::model::{VBA_PROJECT_CONTENT_TYPE, VBA_PROJECT_REL_TYPE, VbaInfo};

pub(super) fn upsert_vba_relationship_xml(
    xml: &str,
    file: &str,
    info: &VbaInfo,
    project_part_uri: &str,
) -> String {
    let rels = relationship_entries_from_optional_xml(xml);
    let target = relationship_target_from_source_to_target(&info.main_part_uri, project_part_uri);
    let mut updated = false;
    let out = rewrite_relationships_xml(xml, |rel| {
        let target_uri = resolve_relationship_target(&info.main_part_uri, &rel.target);
        let content_type = content_type_for_part(file, &target_uri).unwrap_or_default();
        if rel.rel_type == VBA_PROJECT_REL_TYPE
            || target_uri == project_part_uri
            || content_type.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE)
        {
            updated = true;
            Some(relationship_xml(&rel.id, VBA_PROJECT_REL_TYPE, &target))
        } else {
            Some(relationship_xml(&rel.id, &rel.rel_type, &rel.target))
        }
    });
    if updated {
        return out;
    }
    add_relationship_to_xml(
        out,
        &allocate_relationship_id(&rels),
        VBA_PROJECT_REL_TYPE,
        &target,
    )
}

pub(super) fn remove_vba_relationships_xml(xml: &str, file: &str, info: &VbaInfo) -> String {
    rewrite_relationships_xml(xml, |rel| {
        let target_uri = resolve_relationship_target(&info.main_part_uri, &rel.target);
        let content_type = content_type_for_part(file, &target_uri).unwrap_or_default();
        if rel.rel_type == VBA_PROJECT_REL_TYPE
            || content_type.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE)
        {
            None
        } else {
            Some(relationship_xml(&rel.id, &rel.rel_type, &rel.target))
        }
    })
}

fn rewrite_relationships_xml<F>(xml: &str, mut mapper: F) -> String
where
    F: FnMut(&RelationshipEntry) -> Option<String>,
{
    let rels = relationship_entries_from_optional_xml(xml);
    let mut body = String::new();
    for rel in &rels {
        if let Some(rendered) = mapper(rel) {
            body.push_str(&rendered);
        }
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{body}</Relationships>"#
    )
}

fn relationship_entries_from_optional_xml(xml: &str) -> Vec<RelationshipEntry> {
    crate::relationship_entries_from_xml(xml)
}

fn relationship_xml(id: &str, rel_type: &str, target: &str) -> String {
    format!(
        r#"<Relationship Id="{}" Type="{}" Target="{}"/>"#,
        crate::xml_attr_escape(id),
        crate::xml_attr_escape(rel_type),
        crate::xml_attr_escape(target)
    )
}

pub(super) fn set_content_type_override(xml: &str, part_name: &str, content_type: &str) -> String {
    let normalized = format!("/{}", part_name.trim_start_matches('/'));
    let replacement = format!(
        r#"<Override PartName="{}" ContentType="{}"/>"#,
        crate::xml_attr_escape(&normalized),
        crate::xml_attr_escape(content_type)
    );
    if let Some((start, end)) = content_type_override_span(xml, &normalized) {
        let mut out = String::with_capacity(xml.len() + replacement.len());
        out.push_str(&xml[..start]);
        out.push_str(&replacement);
        out.push_str(&xml[end..]);
        return out;
    }
    if let Some(pos) = xml.rfind("</Types>") {
        let mut out = String::with_capacity(xml.len() + replacement.len());
        out.push_str(&xml[..pos]);
        out.push_str(&replacement);
        out.push_str(&xml[pos..]);
        return out;
    }
    xml.to_string()
}

pub(super) fn remove_content_type_override(xml: &str, part_name: &str) -> String {
    let normalized = format!("/{}", part_name.trim_start_matches('/'));
    let Some((start, end)) = content_type_override_span(xml, &normalized) else {
        return xml.to_string();
    };
    let mut out = String::with_capacity(xml.len());
    out.push_str(&xml[..start]);
    out.push_str(&xml[end..]);
    out
}

fn content_type_override_span(xml: &str, part_name: &str) -> Option<(usize, usize)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e))
                if local_name(e.name().as_ref()) == "Override"
                    && attr(&e, "PartName").is_some_and(|value| value == part_name) =>
            {
                return Some((before, reader.buffer_position() as usize));
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

pub(super) fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}
