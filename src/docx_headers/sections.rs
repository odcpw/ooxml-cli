use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;
use serde_json::{Value, json};
use std::collections::BTreeMap;

use super::selectors::docx_header_footer_ref_json;
use crate::{
    CliError, CliResult, DOCX_W_NS, attr_bound_ns, content_type_for_part, element_in_ns, local_name,
};

#[derive(Default)]
struct DocxHeaderFooterSectionBuild {
    section_index: usize,
    headers: DocxHeaderFooterSetBuild,
    footers: DocxHeaderFooterSetBuild,
}

#[derive(Default)]
struct DocxHeaderFooterSetBuild {
    default: Option<Value>,
    first: Option<Value>,
    even: Option<Value>,
}

pub(super) fn docx_header_footer_sections(
    file: &str,
    document_xml: &str,
    rel_targets: &BTreeMap<String, String>,
) -> CliResult<Vec<Value>> {
    let mut reader = NsReader::from_str(document_xml);
    let mut stack: Vec<String> = Vec::new();
    let mut sections = Vec::new();
    let mut current = None::<DocxHeaderFooterSectionBuild>;

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
                if current.is_none()
                    && is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    current = Some(DocxHeaderFooterSectionBuild {
                        section_index: sections.len() + 1,
                        ..DocxHeaderFooterSectionBuild::default()
                    });
                } else if let Some(section) = current.as_mut()
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                {
                    docx_note_header_footer_ref(
                        file,
                        section,
                        &e,
                        reader.resolver(),
                        &name,
                        rel_targets,
                    );
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
                if current.is_none()
                    && is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    let section = DocxHeaderFooterSectionBuild {
                        section_index: sections.len() + 1,
                        ..DocxHeaderFooterSectionBuild::default()
                    };
                    sections.push(docx_header_footer_section_json(section));
                } else if let Some(section) = current.as_mut()
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                {
                    docx_note_header_footer_ref(
                        file,
                        section,
                        &e,
                        reader.resolver(),
                        &name,
                        rel_targets,
                    );
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "sectPr"
                    && let Some(section) = current.take()
                {
                    sections.push(docx_header_footer_section_json(section));
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(sections)
}

fn docx_note_header_footer_ref(
    file: &str,
    section: &mut DocxHeaderFooterSectionBuild,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    name: &str,
    rel_targets: &BTreeMap<String, String>,
) {
    let kind = if name == "footerReference" {
        "footer"
    } else {
        "header"
    };
    let id = attr_bound_ns(
        element,
        resolver,
        b"http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        b"id",
    )
    .unwrap_or_default();
    let ref_type = normalize_docx_header_footer_type(
        attr_bound_ns(element, resolver, DOCX_W_NS, b"type").unwrap_or_default(),
    );
    let part_uri = rel_targets.get(&id).cloned().unwrap_or_default();
    let content_type = if part_uri.is_empty() {
        String::new()
    } else {
        content_type_for_part(file, &part_uri).unwrap_or_default()
    };
    let value = docx_header_footer_ref_json(
        kind,
        &id,
        &ref_type,
        section.section_index,
        &part_uri,
        &content_type,
    );
    let set = if kind == "footer" {
        &mut section.footers
    } else {
        &mut section.headers
    };
    match ref_type.as_str() {
        "first" => set.first = Some(value),
        "even" => set.even = Some(value),
        _ => set.default = Some(value),
    }
}

pub(super) fn normalize_docx_header_footer_type(value: String) -> String {
    match value.as_str() {
        "first" | "even" => value,
        _ => "default".to_string(),
    }
}

fn docx_header_footer_section_json(section: DocxHeaderFooterSectionBuild) -> Value {
    json!({
        "sectionIndex": section.section_index,
        "headers": docx_header_footer_set_json(section.headers),
        "footers": docx_header_footer_set_json(section.footers),
    })
}

fn docx_header_footer_set_json(set: DocxHeaderFooterSetBuild) -> Value {
    json!({
        "default": set.default.unwrap_or(Value::Null),
        "first": set.first.unwrap_or(Value::Null),
        "even": set.even.unwrap_or(Value::Null),
    })
}
