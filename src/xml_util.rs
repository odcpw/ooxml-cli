use quick_xml::escape::unescape as quick_xml_unescape;
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::name::{Namespace, NamespaceResolver, ResolveResult};
use std::collections::BTreeMap;

pub(crate) fn attr(e: &BytesStart<'_>, wanted_local: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local_name(a.key.as_ref()) == wanted_local {
            Some(decode_xml_text(a.value.as_ref()))
        } else {
            None
        }
    })
}

pub(crate) fn attr_prefixed_ns(
    e: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    wanted_prefix: &[u8],
    wanted_ns: &[u8],
    wanted_local: &[u8],
) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        let raw = a.key.as_ref();
        let colon = raw.iter().position(|byte| *byte == b':')?;
        if &raw[..colon] != wanted_prefix || &raw[colon + 1..] != wanted_local {
            return None;
        }
        let (resolved, local) = resolver.resolve_attribute(a.key);
        if matches!(resolved, ResolveResult::Bound(Namespace(uri)) if uri == wanted_ns)
            && local.as_ref() == wanted_local
        {
            Some(decode_xml_text(a.value.as_ref()))
        } else {
            None
        }
    })
}

pub(crate) fn attr_bound_ns(
    e: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    wanted_ns: &[u8],
    wanted_local: &[u8],
) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        let (resolved, local) = resolver.resolve_attribute(a.key);
        if matches!(resolved, ResolveResult::Bound(Namespace(uri)) if uri == wanted_ns)
            && local.as_ref() == wanted_local
        {
            Some(decode_xml_text(a.value.as_ref()))
        } else {
            None
        }
    })
}

pub(crate) fn attr_exact(e: &BytesStart<'_>, wanted: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if String::from_utf8_lossy(a.key.as_ref()) == wanted {
            Some(decode_xml_text(a.value.as_ref()))
        } else {
            None
        }
    })
}

pub(crate) fn local_name(name: &[u8]) -> &str {
    let raw = std::str::from_utf8(name).unwrap_or("");
    raw.rsplit_once(':').map(|(_, local)| local).unwrap_or(raw)
}

pub(crate) fn decode_xml_text(bytes: &[u8]) -> String {
    xml_unescape(&String::from_utf8_lossy(bytes))
}

pub(crate) fn xml_general_ref(bytes: &[u8]) -> String {
    let name = String::from_utf8_lossy(bytes);
    resolve_xml_general_ref_name(&name).unwrap_or_else(|| format!("&{name};"))
}

pub(crate) fn xml_unescape(value: &str) -> String {
    quick_xml_unescape(value)
        .map(|value| value.into_owned())
        .unwrap_or_else(|_| xml_unescape_lossy(value))
}

pub(crate) fn append_xml_text_event(out: &mut String, event: &Event<'_>) -> bool {
    match event {
        Event::Text(e) => {
            out.push_str(&decode_xml_text(e.as_ref()));
            true
        }
        Event::CData(e) => {
            out.push_str(
                &e.xml_content()
                    .map(|value| value.into_owned())
                    .unwrap_or_else(|_| String::from_utf8_lossy(e.as_ref()).into_owned()),
            );
            true
        }
        Event::GeneralRef(e) => {
            out.push_str(&xml_general_ref_event(e));
            true
        }
        _ => false,
    }
}

pub(crate) fn is_xml_text_event(event: &Event<'_>) -> bool {
    matches!(
        event,
        Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)
    )
}

#[derive(Default)]
pub(crate) struct TextAccumulator {
    text: String,
}

impl TextAccumulator {
    pub(crate) fn push_event(&mut self, event: &Event<'_>) -> bool {
        append_xml_text_event(&mut self.text, event)
    }

    pub(crate) fn into_string(self) -> String {
        self.text
    }
}

fn xml_general_ref_event(reference: &BytesRef<'_>) -> String {
    if let Ok(Some(ch)) = reference.resolve_char_ref() {
        return ch.to_string();
    }
    xml_general_ref(reference.as_ref())
}

fn resolve_xml_general_ref_name(name: &str) -> Option<String> {
    match name {
        "quot" => Some("\"".to_string()),
        "apos" => Some("'".to_string()),
        "lt" => Some("<".to_string()),
        "gt" => Some(">".to_string()),
        "amp" => Some("&".to_string()),
        _ => resolve_numeric_xml_ref(name).map(|ch| ch.to_string()),
    }
}

fn resolve_numeric_xml_ref(name: &str) -> Option<char> {
    let number = name.strip_prefix('#')?;
    let value = if let Some(hex) = number
        .strip_prefix('x')
        .or_else(|| number.strip_prefix('X'))
    {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        number.parse::<u32>().ok()?
    };
    char::from_u32(value)
}

fn xml_unescape_lossy(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after_amp = &rest[start + 1..];
        let Some(end) = after_amp.find(';') else {
            out.push('&');
            rest = after_amp;
            continue;
        };
        let name = &after_amp[..end];
        if let Some(decoded) = resolve_xml_general_ref_name(name) {
            out.push_str(&decoded);
        } else {
            out.push('&');
            out.push_str(name);
            out.push(';');
        }
        rest = &after_amp[end + 1..];
    }
    out.push_str(rest);
    out
}

pub(crate) fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn xml_attr_escape(value: &str) -> String {
    xml_escape(value).replace('"', "&quot;")
}

pub(crate) fn decode_xml_attrs(e: &BytesStart<'_>) -> BTreeMap<String, String> {
    e.attributes()
        .flatten()
        .map(|attr| {
            (
                String::from_utf8_lossy(attr.key.as_ref()).to_string(),
                decode_xml_text(attr.value.as_ref()),
            )
        })
        .collect()
}

pub(crate) fn decode_local_xml_attrs(e: &BytesStart<'_>) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    for attr in e.attributes().with_checks(false).flatten() {
        attrs.insert(
            local_name(attr.key.as_ref()).to_string(),
            decode_xml_text(attr.value.as_ref()),
        );
    }
    attrs
}

pub(crate) fn render_xml_attrs(attrs: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    for (key, value) in attrs {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        out.push_str(&xml_attr_escape(value));
        out.push('"');
    }
    out
}

pub(crate) fn remove_xml_span(xml: &str, start: usize, end: usize) -> String {
    let mut out = String::with_capacity(xml.len().saturating_sub(end.saturating_sub(start)));
    out.push_str(&xml[..start]);
    out.push_str(&xml[end..]);
    out
}

pub(crate) fn replace_xml_span(xml: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut out = String::with_capacity(xml.len() - (end - start) + replacement.len());
    out.push_str(&xml[..start]);
    out.push_str(replacement);
    out.push_str(&xml[end..]);
    out
}

pub(crate) fn needs_xml_space_preserve(value: &str) -> bool {
    value != value.trim_matches([' ', '\t', '\r', '\n'])
}
