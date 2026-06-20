use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;
use std::collections::BTreeSet;

mod table_scaffold;

pub(crate) use table_scaffold::{
    ensure_docx_body_table_scaffolds_xml, ensure_docx_table_scaffold_fragment,
};

use crate::{
    CliError, CliResult, attr, attr_prefixed_ns, chrono_like_counter, decode_xml_text,
    docx_para_id_ns, docx_rich_block_reports, element_in_ns, local_name, xml_attr_escape,
    xml_escape, xml_general_ref,
};

pub(crate) const DOCX_W_NS: &[u8] = b"http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub(crate) const DOCX_W14_NS: &[u8] = b"http://schemas.microsoft.com/office/word/2010/wordml";
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

pub(crate) struct DocxParagraphXmlMutation {
    pub(crate) xml: String,
    pub(crate) index: usize,
    pub(crate) style: String,
    pub(crate) previous_text: String,
    pub(crate) flattened: bool,
    pub(crate) handle: String,
}

pub(crate) fn set_or_clear_docx_body_paragraph_xml(
    xml: &str,
    target_index: usize,
    replacement: Option<&str>,
) -> CliResult<DocxParagraphXmlMutation> {
    if target_index == 0 {
        return Err(CliError::target_not_found(
            "target not found: paragraph index 0",
        ));
    }
    let reports = docx_rich_block_reports(xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let report = reports.get(target_index - 1).ok_or_else(|| {
        CliError::target_not_found(format!("target not found: paragraph index {target_index}"))
    })?;
    if report.kind != "paragraph" {
        return Err(CliError::invalid_args(format!(
            "block {target_index} is a table, not a paragraph"
        )));
    }

    let mut working = xml.to_string();
    let mut para_id = report.para_id.trim().to_string();
    if para_id.is_empty() {
        working = ensure_docx_w14_namespace(&working)?;
        let existing = docx_all_para_ids(&working)?;
        para_id = mint_docx_para_id(&existing);
    }

    let body_tag = docx_body_tag(&working)?;
    let blocks = docx_body_block_ranges(&working, &body_tag)?;
    let block = blocks.get(target_index - 1).ok_or_else(|| {
        CliError::target_not_found(format!("target not found: paragraph index {target_index}"))
    })?;
    if block.kind != "p" {
        return Err(CliError::invalid_args(format!(
            "block {target_index} is a table, not a paragraph"
        )));
    }
    let fragment = &working[block.start..block.end];
    let (paragraph, flattened) = replace_docx_paragraph_fragment(fragment, &para_id, replacement)?;
    let mut out = String::with_capacity(working.len() + paragraph.len());
    out.push_str(&working[..block.start]);
    out.push_str(&paragraph);
    out.push_str(&working[block.end..]);

    Ok(DocxParagraphXmlMutation {
        xml: out,
        index: target_index,
        style: report.style.clone(),
        previous_text: report.text.clone(),
        flattened,
        handle: format!("H:docx/pt:doc/para:m:{para_id}"),
    })
}

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

pub(crate) fn append_docx_body_paragraph_xml(
    xml: &str,
    text: &str,
    style: &str,
) -> CliResult<String> {
    let body_tag = docx_body_tag(xml)?;
    let close_tag = format!("</{body_tag}>");
    if !xml.contains(&close_tag) {
        return Err(CliError::unexpected("document body element not found"));
    }
    let prefix = body_tag
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default();
    let mut working = if prefix.is_empty() && !style.is_empty() {
        ensure_docx_word_prefix(xml)?
    } else {
        xml.to_string()
    };
    let body_close = working.rfind(&close_tag).ok_or_else(|| {
        CliError::unexpected("document body element not found after namespace update")
    })?;
    let insert_at = docx_body_sectpr_start(&working[..body_close], &prefix).unwrap_or(body_close);
    let paragraph = render_docx_paragraph(&prefix, text, style);
    working.insert_str(insert_at, &paragraph);
    Ok(working)
}

pub(crate) fn insert_docx_body_paragraph_xml(
    xml: &str,
    insert_after: usize,
    text: &str,
    style: &str,
) -> CliResult<(String, usize)> {
    let body_tag = docx_body_tag(xml)?;
    let close_tag = format!("</{body_tag}>");
    if !xml.contains(&close_tag) {
        return Err(CliError::unexpected("document body element not found"));
    }
    let prefix = body_tag
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default();
    let mut working = if prefix.is_empty() && !style.is_empty() {
        ensure_docx_word_prefix(xml)?
    } else {
        xml.to_string()
    };
    let body_close = working.rfind(&close_tag).ok_or_else(|| {
        CliError::unexpected("document body element not found after namespace update")
    })?;
    let blocks = docx_body_block_ranges(&working, &body_tag)?;
    let (insert_at, index) = if insert_after == 0 {
        (
            blocks.first().map(|block| block.start).unwrap_or_else(|| {
                docx_body_sectpr_start(&working[..body_close], &prefix).unwrap_or(body_close)
            }),
            1,
        )
    } else {
        let block = blocks.get(insert_after - 1).ok_or_else(|| {
            CliError::target_not_found(format!("target not found: block index {insert_after}"))
        })?;
        (block.end, insert_after + 1)
    };
    let paragraph = render_docx_paragraph(&prefix, text, style);
    working.insert_str(insert_at, &paragraph);
    Ok((working, index))
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

fn replace_docx_paragraph_fragment(
    fragment: &str,
    para_id: &str,
    replacement: Option<&str>,
) -> CliResult<(String, bool)> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let start_tag = &fragment[..=open_end];
    let tag_name = xml_token_name(&fragment[1..open_end])
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?
        .to_string();
    let prefix = tag_name
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default();
    let self_closing = start_tag.trim_end().ends_with("/>");

    let mut paragraph_properties = String::new();
    let mut run_properties = String::new();
    let mut flattened = false;
    if !self_closing {
        let close_tag = format!("</{tag_name}>");
        let close_start = fragment
            .rfind(&close_tag)
            .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
        for child in xml_direct_child_ranges(fragment, open_end + 1, close_start)? {
            match child.kind.as_str() {
                "pPr" => {
                    if paragraph_properties.is_empty() {
                        paragraph_properties.push_str(&fragment[child.start..child.end]);
                    }
                }
                "r" => {
                    if run_properties.is_empty()
                        && let Some(r_pr) = first_direct_xml_child_by_kind(
                            &fragment[child.start..child.end],
                            "rPr",
                        )?
                    {
                        run_properties.push_str(&r_pr);
                    }
                }
                _ => flattened = true,
            }
        }
    }

    let mut paragraph = docx_open_tag_with_para_id(start_tag, para_id);
    paragraph.push_str(&paragraph_properties);
    if let Some(text) = replacement {
        let r = word_xml_tag(&prefix, "r");
        paragraph.push('<');
        paragraph.push_str(&r);
        paragraph.push('>');
        paragraph.push_str(&run_properties);
        append_docx_text_children(&mut paragraph, &prefix, text);
        paragraph.push_str("</");
        paragraph.push_str(&r);
        paragraph.push('>');
    }
    paragraph.push_str("</");
    paragraph.push_str(&tag_name);
    paragraph.push('>');
    Ok((paragraph, flattened))
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
        let raw =
            ((chrono_like_counter() as u64) ^ ((std::process::id() as u64) << 17) ^ attempt as u64)
                & 0x7fff_ffff;
        let candidate = format!("{:08X}", raw as u32);
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
    "00000000".to_string()
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

fn docx_body_sectpr_start(body_prefix: &str, prefix: &str) -> Option<usize> {
    let tag = if prefix.is_empty() {
        "<sectPr".to_string()
    } else {
        format!("<{prefix}:sectPr")
    };
    body_prefix.rfind(&tag)
}

pub(crate) fn render_docx_paragraph(prefix: &str, text: &str, style: &str) -> String {
    let p = word_xml_tag(prefix, "p");
    let mut paragraph = String::new();
    paragraph.push('<');
    paragraph.push_str(&p);
    paragraph.push('>');
    if !style.is_empty() {
        let p_pr = word_xml_tag(prefix, "pPr");
        let p_style = word_xml_tag(prefix, "pStyle");
        let val_attr = if prefix.is_empty() {
            "w:val".to_string()
        } else {
            format!("{prefix}:val")
        };
        paragraph.push('<');
        paragraph.push_str(&p_pr);
        paragraph.push('>');
        paragraph.push('<');
        paragraph.push_str(&p_style);
        paragraph.push(' ');
        paragraph.push_str(&val_attr);
        paragraph.push_str("=\"");
        paragraph.push_str(&xml_attr_escape(style));
        paragraph.push_str("\"/>");
        paragraph.push_str("</");
        paragraph.push_str(&p_pr);
        paragraph.push('>');
    }
    if !text.is_empty() {
        let r = word_xml_tag(prefix, "r");
        paragraph.push('<');
        paragraph.push_str(&r);
        paragraph.push('>');
        append_docx_text_children(&mut paragraph, prefix, text);
        paragraph.push_str("</");
        paragraph.push_str(&r);
        paragraph.push('>');
    }
    paragraph.push_str("</");
    paragraph.push_str(&p);
    paragraph.push('>');
    paragraph
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
