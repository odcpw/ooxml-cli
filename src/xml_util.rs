use quick_xml::events::BytesStart;
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
    match bytes {
        b"quot" => "\"".to_string(),
        b"apos" => "'".to_string(),
        b"lt" => "<".to_string(),
        b"gt" => ">".to_string(),
        b"amp" => "&".to_string(),
        _ => format!("&{};", String::from_utf8_lossy(bytes)),
    }
}

pub(crate) fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
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
