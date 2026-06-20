use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CliError, CliResult, DOCX_W_NS, attr_prefixed_ns, element_in_ns, local_name,
    relationship_entries, relationships_part_for, resolve_relationship_target,
};

pub(crate) fn docx_header_footer_part_uris(
    file: &str,
    document_part: &str,
    document_uri: &str,
    document_xml: &str,
) -> CliResult<Vec<String>> {
    let rels_part = relationships_part_for(document_part);
    let rel_targets = relationship_entries(file, &rels_part)
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| rel.target_mode != "External")
        .map(|rel| {
            (
                rel.id,
                resolve_relationship_target(document_uri, &rel.target),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut reader = NsReader::from_str(document_xml);
    let mut stack: Vec<String> = Vec::new();
    let mut section_uris = Vec::new();
    let mut seen = BTreeSet::new();
    let mut in_direct_section = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    in_direct_section = true;
                } else if in_direct_section
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                    && let Some(part_uri) =
                        docx_header_footer_ref_part_uri(&e, reader.resolver(), &rel_targets)
                    && seen.insert(part_uri.clone())
                {
                    section_uris.push(part_uri);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    // Empty section properties have no references.
                } else if in_direct_section
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                    && let Some(part_uri) =
                        docx_header_footer_ref_part_uri(&e, reader.resolver(), &rel_targets)
                    && seen.insert(part_uri.clone())
                {
                    section_uris.push(part_uri);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "sectPr" {
                    in_direct_section = false;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(section_uris)
}

fn docx_header_footer_ref_part_uri(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    rel_targets: &BTreeMap<String, String>,
) -> Option<String> {
    let id = attr_prefixed_ns(
        element,
        resolver,
        b"r",
        b"http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        b"id",
    )?;
    rel_targets.get(&id).cloned()
}
