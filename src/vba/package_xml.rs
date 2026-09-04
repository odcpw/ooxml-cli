use crate::{
    RelationshipEntry, allocate_relationship_id, content_type_for_part,
    relationship_target_from_source_to_target, resolve_relationship_target,
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
    let mut out = crate::opc::render_relationships_xml(&rels, false);
    for rel in &rels {
        let target_uri = resolve_relationship_target(&info.main_part_uri, &rel.target);
        let content_type = content_type_for_part(file, &target_uri).unwrap_or_default();
        if rel.rel_type == VBA_PROJECT_REL_TYPE
            || target_uri == project_part_uri
            || content_type.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE)
        {
            updated = true;
            let replacement = RelationshipEntry::new(&rel.id, VBA_PROJECT_REL_TYPE, &target);
            out = crate::opc::replace_relationship_xml(out, &replacement)
                .unwrap_or_else(|_| crate::opc::render_relationships_xml(&rels, false));
        }
    }
    if updated {
        return out;
    }
    crate::opc::append_relationship_if_absent_xml(
        out,
        &RelationshipEntry::new(
            &allocate_relationship_id(&rels),
            VBA_PROJECT_REL_TYPE,
            &target,
        ),
    )
}

pub(super) fn remove_vba_relationships_xml(xml: &str, file: &str, info: &VbaInfo) -> String {
    let relationships = relationship_entries_from_optional_xml(xml)
        .into_iter()
        .filter(|rel| {
            let target_uri = resolve_relationship_target(&info.main_part_uri, &rel.target);
            let content_type = content_type_for_part(file, &target_uri).unwrap_or_default();
            rel.rel_type != VBA_PROJECT_REL_TYPE
                && !content_type.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE)
        })
        .collect::<Vec<_>>();
    crate::opc::render_relationships_xml(&relationships, false)
}

fn relationship_entries_from_optional_xml(xml: &str) -> Vec<RelationshipEntry> {
    crate::relationship_entries_from_xml(xml)
}

pub(super) fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}
