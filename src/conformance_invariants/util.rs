use quick_xml::events::BytesStart;
use serde_json::{Value, json};

use crate::xml_util::decode_local_xml_attrs;
use crate::{decode_xml_text, local_name};

use super::types::XmlElementInfo;
pub(super) fn xml_element_info(e: &BytesStart<'_>) -> XmlElementInfo {
    XmlElementInfo {
        local_name: local_name(e.name().as_ref()).to_string(),
        namespace: element_namespace(e),
        attrs: decode_local_xml_attrs(e),
    }
}

pub(super) fn element_namespace(e: &BytesStart<'_>) -> String {
    let name = e.name();
    let raw = std::str::from_utf8(name.as_ref()).unwrap_or_default();
    let prefix = raw.rsplit_once(':').map(|(prefix, _)| prefix);
    let wanted = match prefix {
        Some(prefix) => format!("xmlns:{prefix}"),
        None => "xmlns".to_string(),
    };
    e.attributes()
        .with_checks(false)
        .flatten()
        .find_map(|attr| {
            if String::from_utf8_lossy(attr.key.as_ref()) == wanted {
                Some(decode_xml_text(attr.value.as_ref()))
            } else {
                None
            }
        })
        .unwrap_or_default()
}

pub(super) fn is_rels_uri(uri: &str) -> bool {
    let normalized = normalize_uri(uri);
    normalized == "/_rels/.rels"
        || (normalized.ends_with(".rels") && normalized.contains("/_rels/"))
}

pub(super) fn normalize_uri(uri: &str) -> String {
    let mut parts = Vec::new();
    let normalized = uri.replace('\\', "/");
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}

pub(super) fn file_extension(uri: &str) -> &str {
    base_name(uri)
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or_default()
}

pub(super) fn base_name(uri: &str) -> &str {
    uri.rsplit('/').next().unwrap_or(uri)
}

pub(super) fn diag(code: &str, message: impl Into<String>) -> Value {
    json!({
        "code": code,
        "severity": "error",
        "message": message.into(),
    })
}
