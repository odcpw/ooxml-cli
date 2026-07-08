use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, NamespaceResolver, ResolveResult};

use super::DOCX_W_NS;
use crate::{append_xml_text_event, attr, attr_prefixed_ns, is_xml_text_event, local_name};

pub(crate) fn docx_first_word_attr(fragment: &str, name: &[u8]) -> Option<String> {
    let mut reader = NsReader::from_str(fragment);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                return docx_word_attr_ns(&e, reader.resolver(), name)
                    .or_else(|| attr(&e, std::str::from_utf8(name).ok()?));
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

pub(crate) fn docx_word_text_descendants(fragment: &str, wanted: &str) -> String {
    let mut reader = NsReader::from_str(fragment);
    let mut text = String::new();
    let mut wanted_depth = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if wanted_depth > 0 {
                    wanted_depth += 1;
                } else if local_name(e.name().as_ref()) == wanted
                    && docx_fragment_word_element(&e, reader.resolver())
                {
                    wanted_depth = 1;
                }
            }
            Ok(event) if wanted_depth > 0 && is_xml_text_event(&event) => {
                append_xml_text_event(&mut text, &event);
            }
            Ok(Event::End(_)) if wanted_depth > 0 => {
                wanted_depth -= 1;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    text
}

fn docx_fragment_word_element(element: &BytesStart<'_>, resolver: &NamespaceResolver) -> bool {
    match resolver.resolve_element(element.name()).0 {
        ResolveResult::Bound(Namespace(uri)) => return uri == DOCX_W_NS,
        ResolveResult::Unbound | ResolveResult::Unknown(_) => {}
    }
    let name = element.name();
    let bytes = name.as_ref();
    if let Some(colon) = bytes.iter().position(|byte| *byte == b':') {
        return &bytes[..colon] == b"w";
    }
    true
}

pub(crate) fn xml_fragment_text(fragment: &str) -> String {
    let mut reader = NsReader::from_str(fragment);
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(event) if is_xml_text_event(&event) => {
                append_xml_text_event(&mut text, &event);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    text
}

pub(crate) fn docx_word_attr_ns(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    wanted_local: &[u8],
) -> Option<String> {
    attr_prefixed_ns(element, resolver, b"w", DOCX_W_NS, wanted_local)
}

pub(crate) fn docx_paragraph_fragment_text(fragment: &str) -> String {
    let mut reader = NsReader::from_str(fragment);
    let mut text = String::new();
    let mut in_t = false;
    let mut skip_text_depth = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_t = true;
                }
                if matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if skip_text_depth == 0 {
                    match name.as_str() {
                        "tab" => text.push('\t'),
                        "br" | "cr" => text.push('\n'),
                        "noBreakHyphen" => text.push('-'),
                        _ => {}
                    }
                }
            }
            Ok(event) if in_t && skip_text_depth == 0 && is_xml_text_event(&event) => {
                append_xml_text_event(&mut text, &event);
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_t = false;
                } else if matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth = skip_text_depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    text
}
