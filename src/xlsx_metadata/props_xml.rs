use quick_xml::events::Event;
use quick_xml::{NsReader, Reader};
use std::collections::BTreeMap;

use super::{
    XLSX_CORE_PROPS_NS, XLSX_DUBLIN_CORE_NS, XLSX_EXTENDED_PROPS_NS, XlsxWorkbookMetadataFields,
    XlsxWorkbookMetadataUpdateOptions, metadata_ordered_insert_position,
};
use crate::{
    append_xml_text_event, element_in_ns, is_xml_text_event, local_name, remove_xml_span,
    render_xml_attrs, replace_xml_span, xml_attrs_map, xml_escape,
};
struct MetadataXmlElementSpan {
    start: usize,
    end: usize,
    name: String,
    attrs: BTreeMap<String, String>,
}

pub(super) fn xml_direct_child_text_by_ns(xml: &str, ns: &[u8], local: &str) -> String {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut depth = 0usize;
    let mut active_depth = None::<usize>;
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let matched = depth == 1
                    && local_name(e.name().as_ref()) == local
                    && element_in_ns(reader.resolver(), &e, ns);
                depth += 1;
                if matched {
                    active_depth = Some(depth);
                    text.clear();
                }
            }
            Ok(Event::Empty(e)) => {
                if depth == 1
                    && local_name(e.name().as_ref()) == local
                    && element_in_ns(reader.resolver(), &e, ns)
                {
                    return String::new();
                }
            }
            Ok(event) if active_depth.is_some() && is_xml_text_event(&event) => {
                append_xml_text_event(&mut text, &event);
            }
            Ok(Event::End(_)) => {
                if active_depth == Some(depth) {
                    return text;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    String::new()
}

pub(super) fn render_xlsx_core_props_xml(fields: &XlsxWorkbookMetadataFields) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">"#.to_string();
    push_metadata_element(&mut xml, "dc", "title", &fields.title);
    push_metadata_element(&mut xml, "dc", "subject", &fields.subject);
    push_metadata_element(&mut xml, "dc", "creator", &fields.creator);
    push_metadata_element(&mut xml, "dc", "description", &fields.description);
    push_metadata_element(&mut xml, "cp", "keywords", &fields.keywords);
    push_metadata_element(&mut xml, "cp", "lastModifiedBy", &fields.last_modified_by);
    push_metadata_element(&mut xml, "cp", "category", &fields.category);
    xml.push_str("</cp:coreProperties>");
    xml
}

pub(super) fn render_xlsx_app_props_xml(fields: &XlsxWorkbookMetadataFields) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">"#.to_string();
    push_metadata_element(&mut xml, "", "Manager", &fields.manager);
    push_metadata_element(&mut xml, "", "Company", &fields.company);
    xml.push_str("</Properties>");
    xml
}

fn push_metadata_element(xml: &mut String, prefix: &str, local: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    let name = qualified_xml_name(prefix, local);
    xml.push('<');
    xml.push_str(&name);
    xml.push('>');
    xml.push_str(&xml_escape(value));
    xml.push_str("</");
    xml.push_str(&name);
    xml.push('>');
}

pub(super) fn update_xlsx_core_props_xml(
    xml: &str,
    options: &XlsxWorkbookMetadataUpdateOptions<'_>,
    fields: &XlsxWorkbookMetadataFields,
) -> String {
    let mut xml = ensure_xmlns_attr(
        xml.to_string(),
        "cp",
        std::str::from_utf8(XLSX_CORE_PROPS_NS).unwrap_or(""),
    );
    xml = ensure_xmlns_attr(
        xml,
        "dc",
        std::str::from_utf8(XLSX_DUBLIN_CORE_NS).unwrap_or(""),
    );
    xml = ensure_xmlns_attr(xml, "dcterms", "http://purl.org/dc/terms/");
    xml = ensure_xmlns_attr(xml, "dcmitype", "http://purl.org/dc/dcmitype/");
    xml = ensure_xmlns_attr(xml, "xsi", "http://www.w3.org/2001/XMLSchema-instance");
    if options.title.is_some() {
        xml = set_metadata_direct_child_xml(
            &xml,
            XLSX_DUBLIN_CORE_NS,
            "title",
            "dc",
            &fields.title,
            None,
        );
    }
    if options.subject.is_some() {
        xml = set_metadata_direct_child_xml(
            &xml,
            XLSX_DUBLIN_CORE_NS,
            "subject",
            "dc",
            &fields.subject,
            None,
        );
    }
    if options.creator.is_some() {
        xml = set_metadata_direct_child_xml(
            &xml,
            XLSX_DUBLIN_CORE_NS,
            "creator",
            "dc",
            &fields.creator,
            None,
        );
    }
    if options.description.is_some() {
        xml = set_metadata_direct_child_xml(
            &xml,
            XLSX_DUBLIN_CORE_NS,
            "description",
            "dc",
            &fields.description,
            None,
        );
    }
    if options.keywords.is_some() {
        xml = set_metadata_direct_child_xml(
            &xml,
            XLSX_CORE_PROPS_NS,
            "keywords",
            "cp",
            &fields.keywords,
            None,
        );
    }
    if options.last_modified_by.is_some() {
        xml = set_metadata_direct_child_xml(
            &xml,
            XLSX_CORE_PROPS_NS,
            "lastModifiedBy",
            "cp",
            &fields.last_modified_by,
            None,
        );
    }
    if options.category.is_some() {
        xml = set_metadata_direct_child_xml(
            &xml,
            XLSX_CORE_PROPS_NS,
            "category",
            "cp",
            &fields.category,
            None,
        );
    }
    xml
}

pub(super) fn update_xlsx_app_props_xml(
    xml: &str,
    options: &XlsxWorkbookMetadataUpdateOptions<'_>,
    fields: &XlsxWorkbookMetadataFields,
) -> String {
    let mut xml = ensure_xmlns_attr(
        xml.to_string(),
        "",
        std::str::from_utf8(XLSX_EXTENDED_PROPS_NS).unwrap_or(""),
    );
    if options.manager.is_some() {
        xml = set_metadata_direct_child_xml(
            &xml,
            XLSX_EXTENDED_PROPS_NS,
            "Manager",
            "",
            &fields.manager,
            Some(app_property_order),
        );
    }
    if options.company.is_some() {
        xml = set_metadata_direct_child_xml(
            &xml,
            XLSX_EXTENDED_PROPS_NS,
            "Company",
            "",
            &fields.company,
            Some(app_property_order),
        );
    }
    xml
}

fn set_metadata_direct_child_xml(
    xml: &str,
    ns: &[u8],
    local: &str,
    prefix: &str,
    value: &str,
    order: Option<fn(&str) -> i32>,
) -> String {
    if let Some(span) = find_direct_child_span_by_ns(xml, ns, local) {
        if value.is_empty() {
            return remove_xml_span(xml, span.start, span.end);
        }
        let name = span.name;
        return replace_xml_span(
            xml,
            span.start,
            span.end,
            &format!(
                "<{name}{}>{}</{name}>",
                render_xml_attrs(&span.attrs),
                xml_escape(value)
            ),
        );
    }
    if value.is_empty() {
        return xml.to_string();
    }
    let insert_pos = if let Some(order) = order {
        metadata_ordered_insert_position(xml, order(local), order)
    } else {
        xml_root_end_position(xml)
    };
    let Some(insert_pos) = insert_pos else {
        return xml.to_string();
    };
    let name = qualified_xml_name(prefix, local);
    let child = format!("<{name}>{}</{name}>", xml_escape(value));
    let mut out = String::with_capacity(xml.len() + child.len());
    out.push_str(&xml[..insert_pos]);
    out.push_str(&child);
    out.push_str(&xml[insert_pos..]);
    out
}

fn find_direct_child_span_by_ns(
    xml: &str,
    ns: &[u8],
    local: &str,
) -> Option<MetadataXmlElementSpan> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut depth = 0usize;
    let mut active = None::<(usize, usize, String, BTreeMap<String, String>)>;
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let matched = depth == 1
                    && local_name(e.name().as_ref()) == local
                    && element_in_ns(reader.resolver(), &e, ns);
                depth += 1;
                if matched {
                    active = Some((
                        start,
                        depth,
                        String::from_utf8_lossy(e.name().as_ref()).to_string(),
                        xml_attrs_map(&e),
                    ));
                }
            }
            Ok(Event::Empty(e)) => {
                if depth == 1
                    && local_name(e.name().as_ref()) == local
                    && element_in_ns(reader.resolver(), &e, ns)
                {
                    return Some(MetadataXmlElementSpan {
                        start,
                        end: reader.buffer_position() as usize,
                        name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
                        attrs: xml_attrs_map(&e),
                    });
                }
            }
            Ok(Event::End(_)) => {
                if let Some((span_start, span_depth, name, attrs)) = active.as_ref()
                    && *span_depth == depth
                {
                    return Some(MetadataXmlElementSpan {
                        start: *span_start,
                        end: reader.buffer_position() as usize,
                        name: name.clone(),
                        attrs: attrs.clone(),
                    });
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn ensure_xmlns_attr(xml: String, prefix: &str, ns: &str) -> String {
    if ns.is_empty() {
        return xml;
    }
    let attr_name = if prefix.is_empty() {
        "xmlns".to_string()
    } else {
        format!("xmlns:{prefix}")
    };
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let end = reader.buffer_position() as usize;
                let mut attrs = xml_attrs_map(&e);
                if attrs.contains_key(&attr_name) {
                    return xml;
                }
                attrs.insert(attr_name, ns.to_string());
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let suffix = if xml[start..end].trim_end().ends_with("/>") {
                    "/>"
                } else {
                    ">"
                };
                return replace_xml_span(
                    &xml,
                    start,
                    end,
                    &format!("<{name}{}{suffix}", render_xml_attrs(&attrs)),
                );
            }
            Ok(Event::Decl(_)) | Ok(Event::PI(_)) | Ok(Event::DocType(_)) => {}
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    xml
}

fn xml_root_end_position(xml: &str) -> Option<usize> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut depth = 0usize;
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::Empty(_)) if depth == 0 => return Some(start),
            Ok(Event::End(_)) => {
                if depth == 1 {
                    return Some(start);
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn app_property_order(local_name: &str) -> i32 {
    match local_name {
        "Template" => 10,
        "Manager" => 20,
        "Company" => 30,
        "Pages" => 40,
        "Words" => 50,
        "Characters" => 60,
        "PresentationFormat" => 70,
        "Lines" => 80,
        "Paragraphs" => 90,
        "Slides" => 100,
        "Notes" => 110,
        "TotalTime" => 120,
        "HiddenSlides" => 130,
        "MMClips" => 140,
        "ScaleCrop" => 150,
        "HeadingPairs" => 160,
        "TitlesOfParts" => 170,
        "LinksUpToDate" => 180,
        "CharactersWithSpaces" => 190,
        "SharedDoc" => 200,
        "HyperlinkBase" => 210,
        "HLinks" => 220,
        "HyperlinksChanged" => 230,
        "DigSig" => 240,
        "Application" => 250,
        "AppVersion" => 260,
        "DocSecurity" => 270,
        _ => 10000,
    }
}

fn qualified_xml_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}
