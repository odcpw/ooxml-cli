use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use regex::bytes::Regex;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::ops::Range;

use crate::{
    CliError, CliResult, DOCX_W_NS, DocxParagraphMutationOptions, XmlNamedRange, decode_xml_text,
    docx_body_block_ranges, docx_body_tag, docx_paragraph_fragment_text, docx_word_attr_ns,
    element_in_ns, ensure_docx_package_kind, find_docx_document_part, local_name,
    needs_xml_space_preserve, validate_xlsx_mutation_output_flags, write_docx_mutation_output,
    xml_direct_child_ranges, xml_escape, xml_fragment_bounds, xml_token_name, zip_entry_names,
    zip_text,
};

pub(crate) struct DocxReplaceOptions<'a> {
    pub(crate) find: &'a str,
    pub(crate) replace: &'a str,
    pub(crate) regex: bool,
    pub(crate) match_case: bool,
    pub(crate) whole_word: bool,
    pub(crate) expect_count: Option<usize>,
    pub(crate) mutation: DocxParagraphMutationOptions<'a>,
}

#[derive(Clone)]
struct DocxReplaceTarget {
    block_index: usize,
    block_kind: &'static str,
    table_index: usize,
    row_index: usize,
    column_index: usize,
    paragraph_index: usize,
    start: usize,
    end: usize,
}

struct DocxReplaceSummary {
    target: DocxReplaceTarget,
    style: String,
    content_hash: String,
    previous_hash: String,
    replacements: usize,
    previous_text: String,
    text: String,
}

struct TextSegment {
    tag_start: usize,
    content_start: usize,
    content_end: usize,
    text_start: usize,
    text_end: usize,
}

pub(crate) fn docx_replace(file: &str, options: DocxReplaceOptions<'_>) -> CliResult<Value> {
    validate_xlsx_mutation_output_flags(
        options.mutation.out,
        options.mutation.in_place,
        options.mutation.backup,
        options.mutation.dry_run,
    )?;
    let entries = zip_entry_names(file)?;
    ensure_docx_package_kind(file, &entries)?;
    let pattern = build_docx_replace_pattern(
        options.find,
        options.regex,
        options.match_case,
        options.whole_word,
    )?;
    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let targets = docx_replace_targets(&xml)?;

    let mut updated_xml = xml.clone();
    let mut summaries = Vec::new();
    for target in targets.iter().rev() {
        let fragment = &xml[target.start..target.end];
        let previous_text = docx_paragraph_fragment_text(fragment);
        let style = docx_replace_paragraph_style(fragment);
        let (updated_fragment, replacements) =
            replace_in_paragraph_fragment(fragment, &pattern, options.replace)?;
        if replacements == 0 {
            continue;
        }
        let text = docx_paragraph_fragment_text(&updated_fragment);
        let previous_hash = docx_replace_target_hash(target, &style, &previous_text);
        let content_hash = docx_replace_target_hash(target, &style, &text);
        updated_xml.replace_range(target.start..target.end, &updated_fragment);
        summaries.push(DocxReplaceSummary {
            target: target.clone(),
            style,
            content_hash,
            previous_hash,
            replacements,
            previous_text,
            text,
        });
    }
    summaries.reverse();

    let total_replacements = summaries
        .iter()
        .map(|summary| summary.replacements)
        .sum::<usize>();
    if let Some(expected) = options.expect_count
        && expected != total_replacements
    {
        return Err(CliError::invalid_args(format!(
            "replacement count mismatch: expected {expected} replacements, found {total_replacements}"
        )));
    }

    write_docx_mutation_output(file, &document_part, &updated_xml, options.mutation)?;

    let affected_block_indices = affected_docx_replace_block_indices(&summaries);
    Ok(json!({
        "file": file,
        "totalReplacements": total_replacements,
        "affectedBlockCount": affected_block_indices.len(),
        "affectedBlockIndices": affected_block_indices,
        "blockSummaries": summaries.into_iter().map(docx_replace_summary_json).collect::<Vec<_>>(),
    }))
}

fn build_docx_replace_pattern(
    find: &str,
    regex_mode: bool,
    match_case: bool,
    whole_word: bool,
) -> CliResult<Regex> {
    if find.is_empty() {
        return Err(CliError::invalid_args("find pattern is required"));
    }
    let mut expr = if regex_mode {
        find.to_string()
    } else {
        regex::escape(find)
    };
    if whole_word {
        expr = format!(r"\b(?:{expr})\b");
    }
    if !match_case {
        expr = format!("(?i){expr}");
    }
    Regex::new(&expr).map_err(|err| {
        CliError::invalid_args(format!(
            "invalid find pattern: {}",
            go_like_regex_error(&expr, &err.to_string())
        ))
    })
}

fn go_like_regex_error(expr: &str, rust_error: &str) -> String {
    if rust_error.contains("unclosed group") {
        return format!("error parsing regexp: missing closing ): `{expr}`");
    }
    format!("error parsing regexp: {rust_error}")
}

fn docx_replace_targets(xml: &str) -> CliResult<Vec<DocxReplaceTarget>> {
    let body_tag = docx_body_tag(xml)?;
    let blocks = docx_body_block_ranges(xml, &body_tag)?;
    let mut targets = Vec::new();
    let mut table_index = 0usize;
    for (block_offset, block) in blocks.iter().enumerate() {
        let block_index = block_offset + 1;
        match block.kind {
            "p" => targets.push(DocxReplaceTarget {
                block_index,
                block_kind: "paragraph",
                table_index: 0,
                row_index: 0,
                column_index: 0,
                paragraph_index: 0,
                start: block.start,
                end: block.end,
            }),
            "tbl" => {
                table_index += 1;
                targets.extend(docx_table_replace_targets(
                    xml,
                    block.start,
                    block.end,
                    block_index,
                    table_index,
                )?);
            }
            _ => {}
        }
    }
    Ok(targets)
}

fn docx_table_replace_targets(
    xml: &str,
    table_start: usize,
    table_end: usize,
    block_index: usize,
    table_index: usize,
) -> CliResult<Vec<DocxReplaceTarget>> {
    let table_fragment = &xml[table_start..table_end];
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(table_fragment)?;
    if self_closing {
        return Ok(Vec::new());
    }
    let rows: Vec<XmlNamedRange> =
        xml_direct_child_ranges(table_fragment, open_end + 1, close_start)?
            .into_iter()
            .filter(|child| child.kind == "tr")
            .collect();
    let mut targets = Vec::new();
    for (row_offset, row) in rows.iter().enumerate() {
        let row_fragment = &table_fragment[row.start..row.end];
        let (row_open_end, _row_tag_name, row_close_start, row_self_closing) =
            xml_fragment_bounds(row_fragment)?;
        if row_self_closing {
            continue;
        }
        let cells: Vec<XmlNamedRange> =
            xml_direct_child_ranges(row_fragment, row_open_end + 1, row_close_start)?
                .into_iter()
                .filter(|child| child.kind == "tc")
                .collect();
        for (cell_offset, cell) in cells.iter().enumerate() {
            let cell_fragment = &row_fragment[cell.start..cell.end];
            let paragraphs = descendant_paragraph_ranges(cell_fragment)?;
            for (paragraph_offset, paragraph) in paragraphs.iter().enumerate() {
                let start = table_start + row.start + cell.start + paragraph.start;
                let end = table_start + row.start + cell.start + paragraph.end;
                targets.push(DocxReplaceTarget {
                    block_index,
                    block_kind: "table",
                    table_index,
                    row_index: row_offset + 1,
                    column_index: cell_offset + 1,
                    paragraph_index: paragraph_offset + 1,
                    start,
                    end,
                });
            }
        }
    }
    Ok(targets)
}

fn descendant_paragraph_ranges(fragment: &str) -> CliResult<Vec<XmlNamedRange>> {
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(Vec::new());
    }
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    let mut ranges = Vec::new();
    for child in children {
        if child.kind == "p" {
            ranges.push(child);
            continue;
        }
        let nested = descendant_paragraph_ranges(&fragment[child.start..child.end])?;
        for paragraph in nested {
            ranges.push(XmlNamedRange {
                start: child.start + paragraph.start,
                end: child.start + paragraph.end,
                kind: paragraph.kind,
            });
        }
    }
    Ok(ranges)
}

fn replace_in_paragraph_fragment(
    fragment: &str,
    pattern: &Regex,
    replacement: &str,
) -> CliResult<(String, usize)> {
    let (segments, full) = collect_docx_text_segments(fragment)?;
    if segments.is_empty() {
        return Ok((fragment.to_string(), 0));
    }
    let matches: Vec<(usize, usize)> = pattern
        .find_iter(&full)
        .filter(|matched| matched.start() != matched.end())
        .map(|matched| (matched.start(), matched.end()))
        .collect();
    if matches.is_empty() {
        return Ok((fragment.to_string(), 0));
    }

    let mut out = vec![Vec::<u8>::new(); segments.len()];
    let mut applied = 0usize;
    let mut cursor = 0usize;
    for (match_start, match_end) in matches {
        emit_docx_replace_range(&segments, &full, cursor, match_start, &mut out);
        if let Some(segment_index) = segment_index_at(&segments, match_start) {
            out[segment_index].extend_from_slice(replacement.as_bytes());
            cursor = match_end;
            applied += 1;
        }
    }
    emit_docx_replace_range(&segments, &full, cursor, full.len(), &mut out);
    if applied == 0 {
        return Ok((fragment.to_string(), 0));
    }

    let mut updated = fragment.to_string();
    for (index, segment) in segments.iter().enumerate().rev() {
        let value = String::from_utf8_lossy(&out[index]).to_string();
        let open_tag = &fragment[segment.tag_start..segment.content_start];
        let open_tag = apply_docx_text_space_preserve(open_tag, &value);
        let replacement = format!("{}{}", open_tag, xml_escape(&value));
        updated.replace_range(segment.tag_start..segment.content_end, &replacement);
    }
    Ok((updated, applied))
}

fn collect_docx_text_segments(fragment: &str) -> CliResult<(Vec<TextSegment>, Vec<u8>)> {
    let mut segments = Vec::new();
    let mut full = Vec::new();
    let mut cursor = 0usize;
    let mut skip_text_depth = 0usize;
    while cursor < fragment.len() {
        let Some(relative_start) = fragment[cursor..].find('<') else {
            break;
        };
        let tag_start = cursor + relative_start;
        let Some(relative_end) = fragment[tag_start..].find('>') else {
            return Err(CliError::unexpected("invalid DOCX XML"));
        };
        let tag_end = tag_start + relative_end;
        let token = &fragment[tag_start + 1..tag_end];
        let trimmed = token.trim_start();
        if trimmed.starts_with("!--") || trimmed.starts_with('?') || trimmed.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        let closing = trimmed.starts_with('/');
        let self_closing = trimmed.trim_end().ends_with('/');
        let Some(tag_name) = xml_token_name(trimmed) else {
            cursor = tag_end + 1;
            continue;
        };
        let name = local_name(tag_name.as_bytes());
        if closing {
            if matches!(name, "delText" | "instrText") {
                skip_text_depth = skip_text_depth.saturating_sub(1);
            }
            cursor = tag_end + 1;
            continue;
        }
        if name == "t" && skip_text_depth == 0 && !self_closing {
            let content_start = tag_end + 1;
            let close_tag = format!("</{tag_name}>");
            let close_start = fragment[content_start..]
                .find(&close_tag)
                .map(|offset| content_start + offset)
                .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
            let text_start = full.len();
            full.extend_from_slice(
                decode_xml_text(&fragment.as_bytes()[content_start..close_start]).as_bytes(),
            );
            let text_end = full.len();
            segments.push(TextSegment {
                tag_start,
                content_start,
                content_end: close_start,
                text_start,
                text_end,
            });
            cursor = close_start + close_tag.len();
            continue;
        }
        if matches!(name, "delText" | "instrText") && !self_closing {
            skip_text_depth += 1;
        }
        cursor = tag_end + 1;
    }
    Ok((segments, full))
}

fn emit_docx_replace_range(
    segments: &[TextSegment],
    full: &[u8],
    start: usize,
    end: usize,
    out: &mut [Vec<u8>],
) {
    for (index, segment) in segments.iter().enumerate() {
        let lo = start.max(segment.text_start);
        let hi = end.min(segment.text_end);
        if lo < hi {
            out[index].extend_from_slice(&full[lo..hi]);
        }
    }
}

fn segment_index_at(segments: &[TextSegment], offset: usize) -> Option<usize> {
    segments
        .iter()
        .position(|segment| offset >= segment.text_start && offset < segment.text_end)
}

fn apply_docx_text_space_preserve(open_tag: &str, value: &str) -> String {
    let needs_preserve = needs_xml_space_preserve(value);
    let existing = xml_space_attr_range(open_tag);
    match (needs_preserve, existing) {
        (true, Some(_)) => open_tag.to_string(),
        (true, None) => {
            let insert_at = open_tag.rfind('>').unwrap_or(open_tag.len());
            let mut out = String::with_capacity(open_tag.len() + 21);
            out.push_str(&open_tag[..insert_at]);
            out.push_str(" xml:space=\"preserve\"");
            out.push_str(&open_tag[insert_at..]);
            out
        }
        (false, Some(range)) => {
            let mut out = String::with_capacity(open_tag.len() - (range.end - range.start));
            out.push_str(&open_tag[..range.start]);
            out.push_str(&open_tag[range.end..]);
            out
        }
        (false, None) => open_tag.to_string(),
    }
}

fn xml_space_attr_range(tag: &str) -> Option<Range<usize>> {
    let bytes = tag.as_bytes();
    let mut index = 1usize;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || matches!(bytes[index], b'>' | b'/') {
            return None;
        }
        let leading_start = index;
        let name_start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && !matches!(bytes[index], b'=' | b'>' | b'/')
        {
            index += 1;
        }
        let name = &tag[name_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || !matches!(bytes[index], b'"' | b'\'') {
            continue;
        }
        let quote = bytes[index];
        index += 1;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index < bytes.len() {
            index += 1;
        }
        if name == "xml:space" {
            return Some(leading_start..index);
        }
    }
    None
}

fn docx_replace_paragraph_style(fragment: &str) -> String {
    let mut reader = NsReader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<String> = Vec::new();
    let mut word_stack: Vec<bool> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if let Some(style) =
                    docx_replace_style_from_element(&e, reader.resolver(), &stack, &word_stack)
                {
                    return style;
                }
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                stack.push(name);
                word_stack.push(is_word);
            }
            Ok(Event::Empty(e)) => {
                if let Some(style) =
                    docx_replace_style_from_element(&e, reader.resolver(), &stack, &word_stack)
                {
                    return style;
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
                word_stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    String::new()
}

fn docx_replace_style_from_element(
    element: &BytesStart<'_>,
    resolver: &quick_xml::name::NamespaceResolver,
    stack: &[String],
    word_stack: &[bool],
) -> Option<String> {
    if local_name(element.name().as_ref()) == "pStyle"
        && element_in_ns(resolver, element, DOCX_W_NS)
        && stack.last().is_some_and(|parent| parent == "pPr")
        && word_stack.last().copied().unwrap_or(false)
    {
        docx_word_attr_ns(element, resolver, b"val").filter(|style| !style.is_empty())
    } else {
        None
    }
}

fn docx_replace_target_hash(target: &DocxReplaceTarget, style: &str, text: &str) -> String {
    if target.block_kind == "table" {
        docx_replace_content_hash(
            "table",
            "",
            &format!(
                "{}:{}:{}:{}",
                target.row_index, target.column_index, target.paragraph_index, text
            ),
        )
    } else {
        docx_replace_content_hash("paragraph", style, text)
    }
}

fn docx_replace_content_hash(kind: &str, style: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.as_bytes());
    hasher.update([0]);
    hasher.update(style.as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn affected_docx_replace_block_indices(summaries: &[DocxReplaceSummary]) -> Vec<usize> {
    let mut indices = Vec::new();
    for summary in summaries {
        if !indices.contains(&summary.target.block_index) {
            indices.push(summary.target.block_index);
        }
    }
    indices
}

fn docx_replace_summary_json(summary: DocxReplaceSummary) -> Value {
    let mut object = Map::new();
    object.insert("index".to_string(), json!(summary.target.block_index));
    object.insert(
        "kind".to_string(),
        json!(if summary.target.block_kind == "table" {
            "tableCell"
        } else {
            summary.target.block_kind
        }),
    );
    if !summary.style.is_empty() {
        object.insert("style".to_string(), json!(summary.style));
    }
    if summary.target.table_index > 0 {
        object.insert("tableIndex".to_string(), json!(summary.target.table_index));
        object.insert("rowIndex".to_string(), json!(summary.target.row_index));
        object.insert(
            "columnIndex".to_string(),
            json!(summary.target.column_index),
        );
        object.insert(
            "paragraphIndex".to_string(),
            json!(summary.target.paragraph_index),
        );
    }
    object.insert("contentHash".to_string(), json!(summary.content_hash));
    object.insert("previousHash".to_string(), json!(summary.previous_hash));
    object.insert(
        "replacementsInBlock".to_string(),
        json!(summary.replacements),
    );
    object.insert("previousText".to_string(), json!(summary.previous_text));
    object.insert("text".to_string(), json!(summary.text));
    Value::Object(object)
}
