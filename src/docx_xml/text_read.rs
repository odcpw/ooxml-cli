use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;

use super::DOCX_W_NS;
use crate::{attr, attr_prefixed_ns, decode_xml_text, local_name, xml_general_ref};

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
    let mut in_wanted = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if local_name(e.name().as_ref()) == wanted {
                    in_wanted = true;
                }
            }
            Ok(Event::Text(e)) if in_wanted => text.push_str(&decode_xml_text(e.as_ref())),
            Ok(Event::GeneralRef(e)) if in_wanted => text.push_str(&xml_general_ref(e.as_ref())),
            Ok(Event::CData(e)) if in_wanted => text.push_str(&String::from_utf8_lossy(e.as_ref())),
            Ok(Event::End(e)) => {
                if local_name(e.name().as_ref()) == wanted {
                    in_wanted = false;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    text
}

pub(crate) fn xml_fragment_text(fragment: &str) -> String {
    let mut reader = NsReader::from_str(fragment);
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(e)) => text.push_str(&decode_xml_text(e.as_ref())),
            Ok(Event::GeneralRef(e)) => text.push_str(&xml_general_ref(e.as_ref())),
            Ok(Event::CData(e)) => text.push_str(&String::from_utf8_lossy(e.as_ref())),
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
            Ok(Event::Text(e)) if in_t && skip_text_depth == 0 => {
                text.push_str(&decode_xml_text(e.as_ref()));
            }
            Ok(Event::GeneralRef(e)) if in_t && skip_text_depth == 0 => {
                text.push_str(&xml_general_ref(e.as_ref()));
            }
            Ok(Event::CData(e)) if in_t && skip_text_depth == 0 => {
                text.push_str(&String::from_utf8_lossy(e.as_ref()));
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
