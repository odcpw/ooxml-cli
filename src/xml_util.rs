use quick_xml::events::BytesStart;
use quick_xml::name::{Namespace, NamespaceResolver, ResolveResult};

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
