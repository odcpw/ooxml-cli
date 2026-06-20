use quick_xml::NsReader;
use quick_xml::events::Event;

mod body_paragraphs;
mod paragraph_ids;
mod table_scaffold;
mod text_read;

pub(crate) use body_paragraphs::{
    append_docx_body_paragraph_xml, insert_docx_body_paragraph_xml, render_docx_paragraph,
    set_or_clear_docx_body_paragraph_xml,
};
pub(crate) use paragraph_ids::{
    docx_all_para_ids, docx_open_tag_with_para_id, ensure_docx_w14_namespace, mint_docx_para_id,
};
pub(crate) use table_scaffold::{
    ensure_docx_body_table_scaffolds_xml, ensure_docx_table_scaffold_fragment,
};
pub(crate) use text_read::{
    docx_first_word_attr, docx_paragraph_fragment_text, docx_word_attr_ns,
    docx_word_text_descendants, xml_fragment_text,
};

use crate::{CliError, CliResult, element_in_ns, local_name, xml_escape};

pub(crate) const DOCX_W_NS: &[u8] = b"http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub(crate) const DOCX_W14_NS: &[u8] = b"http://schemas.microsoft.com/office/word/2010/wordml";

pub(crate) fn xml_fragment_bounds(fragment: &str) -> CliResult<(usize, String, usize, bool)> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let tag_name = xml_token_name(&fragment[1..open_end])
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?
        .to_string();
    let self_closing = fragment[..=open_end].trim_end().ends_with("/>");
    let close_start = if self_closing {
        open_end + 1
    } else {
        let close_tag = format!("</{tag_name}>");
        fragment
            .rfind(&close_tag)
            .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?
    };
    Ok((open_end, tag_name, close_start, self_closing))
}

pub(crate) fn xml_open_tag_from_start(start_tag: &str) -> String {
    if !start_tag.trim_end().ends_with("/>") {
        return start_tag.to_string();
    }
    let slash = start_tag
        .rfind('/')
        .unwrap_or_else(|| start_tag.len().saturating_sub(1));
    let mut open = String::new();
    open.push_str(&start_tag[..slash]);
    open.push('>');
    open
}

pub(crate) fn xml_tag_prefix(tag_name: &str) -> String {
    tag_name
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default()
}

#[derive(Clone, Copy)]
pub(crate) struct XmlRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) kind: &'static str,
}

pub(crate) fn docx_body_block_ranges(xml: &str, body_tag: &str) -> CliResult<Vec<XmlRange>> {
    let (content_start, content_end) = docx_body_content_bounds(xml, body_tag)?;
    let mut cursor = content_start;
    let mut depth = 0usize;
    let mut active_block_start: Option<usize> = None;
    let mut active_block_kind: &'static str = "";
    let mut blocks = Vec::new();
    while cursor < content_end {
        let Some(relative_start) = xml[cursor..content_end].find('<') else {
            break;
        };
        let tag_start = cursor + relative_start;
        let Some(relative_end) = xml[tag_start..content_end].find('>') else {
            return Err(CliError::unexpected("invalid DOCX XML"));
        };
        let tag_end = tag_start + relative_end;
        let token = &xml[tag_start + 1..tag_end];
        let trimmed = token.trim_start();
        if trimmed.starts_with("!--") || trimmed.starts_with('?') || trimmed.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        let closing = trimmed.starts_with('/');
        if closing {
            if depth > 0 {
                depth -= 1;
                if depth == 0
                    && let Some(start) = active_block_start.take()
                {
                    blocks.push(XmlRange {
                        start,
                        end: tag_end + 1,
                        kind: active_block_kind,
                    });
                    active_block_kind = "";
                }
            }
            cursor = tag_end + 1;
            continue;
        }

        let self_closing = trimmed.trim_end().ends_with('/');
        let name = xml_token_name(trimmed).unwrap_or_default();
        let kind = match local_name(name.as_bytes()) {
            "p" => "p",
            "tbl" => "tbl",
            _ => "",
        };
        let is_body_block = depth == 0 && !kind.is_empty();
        if is_body_block {
            active_block_start = Some(tag_start);
            active_block_kind = kind;
        }
        if self_closing {
            if is_body_block {
                blocks.push(XmlRange {
                    start: tag_start,
                    end: tag_end + 1,
                    kind,
                });
                active_block_start = None;
                active_block_kind = "";
            }
        } else {
            depth += 1;
        }
        cursor = tag_end + 1;
    }
    Ok(blocks)
}

#[derive(Clone)]
pub(crate) struct XmlNamedRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) kind: String,
}

pub(crate) fn first_direct_xml_child_by_kind(
    fragment: &str,
    wanted: &str,
) -> CliResult<Option<String>> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let start_tag = &fragment[..=open_end];
    if start_tag.trim_end().ends_with("/>") {
        return Ok(None);
    }
    let tag_name = xml_token_name(&fragment[1..open_end])
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let close_tag = format!("</{tag_name}>");
    let close_start = fragment
        .rfind(&close_tag)
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    Ok(
        xml_direct_child_ranges(fragment, open_end + 1, close_start)?
            .into_iter()
            .find(|child| child.kind == wanted)
            .map(|child| fragment[child.start..child.end].to_string()),
    )
}

pub(crate) fn xml_direct_child_ranges(
    xml: &str,
    content_start: usize,
    content_end: usize,
) -> CliResult<Vec<XmlNamedRange>> {
    let mut cursor = content_start;
    let mut depth = 0usize;
    let mut active_start: Option<usize> = None;
    let mut active_kind = String::new();
    let mut ranges = Vec::new();
    while cursor < content_end {
        let Some(relative_start) = xml[cursor..content_end].find('<') else {
            break;
        };
        let tag_start = cursor + relative_start;
        let Some(relative_end) = xml[tag_start..content_end].find('>') else {
            return Err(CliError::unexpected("invalid DOCX XML"));
        };
        let tag_end = tag_start + relative_end;
        let token = &xml[tag_start + 1..tag_end];
        let trimmed = token.trim_start();
        if trimmed.starts_with("!--") || trimmed.starts_with('?') || trimmed.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        if trimmed.starts_with('/') {
            if depth > 0 {
                depth -= 1;
                if depth == 0
                    && let Some(start) = active_start.take()
                {
                    ranges.push(XmlNamedRange {
                        start,
                        end: tag_end + 1,
                        kind: active_kind.clone(),
                    });
                    active_kind.clear();
                }
            }
            cursor = tag_end + 1;
            continue;
        }

        let self_closing = trimmed.trim_end().ends_with('/');
        let name = xml_token_name(trimmed).unwrap_or_default();
        if depth == 0 {
            active_start = Some(tag_start);
            active_kind = local_name(name.as_bytes()).to_string();
        }
        if self_closing {
            if depth == 0 {
                ranges.push(XmlNamedRange {
                    start: tag_start,
                    end: tag_end + 1,
                    kind: active_kind.clone(),
                });
                active_start = None;
                active_kind.clear();
            }
        } else {
            depth += 1;
        }
        cursor = tag_end + 1;
    }
    Ok(ranges)
}

pub(crate) fn docx_body_content_bounds(xml: &str, body_tag: &str) -> CliResult<(usize, usize)> {
    let body_open = xml
        .find(&format!("<{body_tag}"))
        .ok_or_else(|| CliError::unexpected("document body element not found"))?;
    let content_start = xml[body_open..]
        .find('>')
        .map(|offset| body_open + offset + 1)
        .ok_or_else(|| CliError::unexpected("document body element not found"))?;
    let content_end = xml
        .rfind(&format!("</{body_tag}>"))
        .ok_or_else(|| CliError::unexpected("document body element not found"))?;
    Ok((content_start, content_end))
}

pub(crate) fn xml_token_name(token: &str) -> Option<&str> {
    let token = token.trim_start().trim_start_matches('/');
    if token.is_empty() || token.starts_with('?') || token.starts_with('!') {
        return None;
    }
    let end = token
        .find(|ch: char| ch.is_whitespace() || ch == '/')
        .unwrap_or(token.len());
    Some(&token[..end])
}

pub(crate) fn docx_body_prefix(body_tag: &str) -> String {
    body_tag
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default()
}

pub(crate) fn docx_block_has_section_properties(fragment: &str) -> bool {
    let mut cursor = 0usize;
    while cursor < fragment.len() {
        let Some(relative_start) = fragment[cursor..].find('<') else {
            break;
        };
        let tag_start = cursor + relative_start;
        let Some(relative_end) = fragment[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_end;
        let token = &fragment[tag_start + 1..tag_end];
        if let Some(name) = xml_token_name(token)
            && local_name(name.as_bytes()) == "sectPr"
        {
            return true;
        }
        cursor = tag_end + 1;
    }
    false
}

pub(crate) fn docx_body_tag(xml: &str) -> CliResult<String> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<String> = Vec::new();
    let mut word_stack: Vec<bool> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.last().is_some_and(|parent| parent == "document")
                    && word_stack.last().copied().unwrap_or(false)
                    && name == "body"
                    && is_word
                {
                    return Ok(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
                stack.push(name);
                word_stack.push(is_word);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.last().is_some_and(|parent| parent == "document")
                    && word_stack.last().copied().unwrap_or(false)
                    && name == "body"
                    && is_word
                {
                    return Ok(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
                word_stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "failed to read main document: {err}"
                )));
            }
            _ => {}
        }
    }
    Err(CliError::unexpected("document body element not found"))
}

pub(crate) fn ensure_docx_word_prefix(xml: &str) -> CliResult<String> {
    if xml.contains("xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"") {
        return Ok(xml.to_string());
    }
    let document_start = xml
        .find("<document")
        .or_else(|| xml.find("<w:document"))
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let start_end = xml[document_start..]
        .find('>')
        .map(|offset| document_start + offset)
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let mut out = String::with_capacity(xml.len() + 83);
    out.push_str(&xml[..start_end]);
    out.push_str(" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"");
    out.push_str(&xml[start_end..]);
    Ok(out)
}

pub(crate) fn append_docx_text_children(out: &mut String, prefix: &str, text: &str) {
    for (line_index, line) in text.split('\n').enumerate() {
        if line_index > 0 {
            let br = word_xml_tag(prefix, "br");
            out.push('<');
            out.push_str(&br);
            out.push_str("/>");
        }
        for (segment_index, segment) in line.split('\t').enumerate() {
            if segment_index > 0 {
                let tab = word_xml_tag(prefix, "tab");
                out.push('<');
                out.push_str(&tab);
                out.push_str("/>");
            }
            if segment.is_empty() {
                continue;
            }
            let t = word_xml_tag(prefix, "t");
            out.push('<');
            out.push_str(&t);
            if needs_docx_space_preserve(segment) {
                out.push_str(" xml:space=\"preserve\"");
            }
            out.push('>');
            out.push_str(&xml_escape(segment));
            out.push_str("</");
            out.push_str(&t);
            out.push('>');
        }
    }
}

fn needs_docx_space_preserve(value: &str) -> bool {
    value != value.trim_matches(|ch| matches!(ch, ' ' | '\t' | '\r' | '\n'))
}

pub(crate) fn word_xml_tag(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}
