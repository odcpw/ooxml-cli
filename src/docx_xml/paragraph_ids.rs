use quick_xml::NsReader;
use quick_xml::events::Event;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use super::DOCX_W_NS;
use crate::{CliError, CliResult, docx_para_id_ns, element_in_ns, local_name, xml_attr_escape};

pub(crate) fn docx_open_tag_with_para_id(start_tag: &str, para_id: &str) -> String {
    let mut out = if start_tag.trim_end().ends_with("/>") {
        let slash = start_tag
            .rfind('/')
            .unwrap_or_else(|| start_tag.len().saturating_sub(1));
        let mut open = String::with_capacity(start_tag.len());
        open.push_str(&start_tag[..slash]);
        open.push('>');
        open
    } else {
        start_tag.to_string()
    };
    if !xml_start_tag_has_para_id(&out) {
        insert_xml_start_tag_attr(
            &mut out,
            &format!("w14:paraId=\"{}\"", xml_attr_escape(para_id)),
        );
    }
    out
}

fn xml_start_tag_has_para_id(tag: &str) -> bool {
    tag.contains(":paraId=")
        || tag.contains(" paraId=")
        || tag.contains("\tparaId=")
        || tag.contains("\nparaId=")
}

fn insert_xml_start_tag_attr(tag: &mut String, attr: &str) {
    if let Some(insert_at) = tag.rfind('>') {
        tag.insert_str(insert_at, &format!(" {attr}"));
    }
}

pub(crate) fn ensure_docx_w14_namespace(xml: &str) -> CliResult<String> {
    if xml.contains("xmlns:w14=") {
        return Ok(xml.to_string());
    }
    let document_start = xml
        .find("<w:document")
        .or_else(|| xml.find("<document"))
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let start_end = xml[document_start..]
        .find('>')
        .map(|offset| document_start + offset)
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let mut out = String::with_capacity(xml.len() + 72);
    out.push_str(&xml[..start_end]);
    out.push_str(" xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\"");
    out.push_str(&xml[start_end..]);
    Ok(out)
}

pub(crate) fn docx_all_para_ids(xml: &str) -> CliResult<BTreeSet<String>> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut ids = BTreeSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "p"
                    && element_in_ns(reader.resolver(), &e, DOCX_W_NS) =>
            {
                if let Some(para_id) = docx_para_id_ns(&e, reader.resolver()) {
                    ids.insert(para_id.to_ascii_uppercase());
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(ids)
}

pub(crate) fn mint_docx_para_id(existing: &BTreeSet<String>) -> String {
    for attempt in 0..10_000u32 {
        let mut hasher = Sha256::new();
        hasher.update(b"ooxml-cli/docx/para-id/v1\0");
        for para_id in existing {
            hasher.update(para_id.as_bytes());
            hasher.update([0]);
        }
        hasher.update(attempt.to_le_bytes());
        let digest = hasher.finalize();
        let raw = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) & 0x7fff_ffff;
        let candidate = format!("{raw:08X}");
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
    "00000000".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minted_paragraph_ids_are_deterministic_and_collision_free() {
        let mut existing = BTreeSet::from(["12345678".to_string()]);
        let first = mint_docx_para_id(&existing);
        assert_eq!(first, mint_docx_para_id(&existing));
        assert_eq!(first.len(), 8);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(!existing.contains(&first));

        existing.insert(first.clone());
        let second = mint_docx_para_id(&existing);
        assert_ne!(second, first);
        assert!(!existing.contains(&second));
    }
}
