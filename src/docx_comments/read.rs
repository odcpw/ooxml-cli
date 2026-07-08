use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, append_xml_text_event, attr, is_xml_text_event, local_name, zip_text,
};

use super::docx_document_and_comments_parts;

pub(crate) fn docx_comments_list(file: &str, comment_id: Option<i64>) -> CliResult<Value> {
    let (document_part, comments_part) = docx_document_and_comments_parts(file)?;
    let mut comments = Vec::new();
    if let Some(comments_part) = comments_part.as_deref() {
        comments = docx_comments(file, comments_part, &document_part)?;
    }
    if let Some(comment_id) = comment_id {
        comments.retain(|comment| comment.id == comment_id);
        if comments.is_empty() {
            return Err(CliError::target_not_found(format!(
                "target not found: comment {comment_id}"
            )));
        }
    }
    let counts = docx_comment_id_counts(&comments);
    let comment_values = comments
        .iter()
        .map(|comment| docx_comment_json(comment, &counts))
        .collect::<Vec<_>>();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("documentPartUri".to_string(), json!(document_part));
    if let Some(comments_part) = comments_part {
        result.insert("commentsPart".to_string(), json!(comments_part));
    }
    result.insert("comments".to_string(), Value::Array(comment_values));
    Ok(Value::Object(result))
}

#[derive(Clone, Default)]
pub(super) struct DocxCommentInfo {
    pub(super) id: i64,
    pub(super) id_raw: String,
    pub(super) id_valid: bool,
    pub(super) author: String,
    pub(super) date: String,
    pub(super) initials: String,
    pub(super) text: String,
    pub(super) anchored_to_block: usize,
    pub(super) anchored_to_block_kind: String,
}

#[derive(Default)]
struct DocxCommentBuild {
    info: DocxCommentInfo,
    paragraphs: Vec<String>,
    current_paragraph: Option<String>,
    in_t: bool,
    skip_text_depth: usize,
}

fn docx_comments(
    file: &str,
    comments_part: &str,
    document_part: &str,
) -> CliResult<Vec<DocxCommentInfo>> {
    let xml = zip_text(file, comments_part.trim_start_matches('/'))?;
    let anchors = docx_comment_anchors(file, document_part)?;
    let mut reader = Reader::from_str(&xml);
    let mut saw_root = false;
    let mut stack = Vec::<String>::new();
    let mut current: Option<DocxCommentBuild> = None;
    let mut comments = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "comments" {
                        return Ok(Vec::new());
                    }
                } else if name == "comment"
                    && stack.last().is_some_and(|parent| parent == "comments")
                {
                    current = Some(docx_comment_from_element(&e));
                } else if let Some(comment) = current.as_mut() {
                    docx_note_comment_start(&e, &name, &stack, comment);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "comments" {
                        return Ok(Vec::new());
                    }
                } else if name == "comment"
                    && stack.last().is_some_and(|parent| parent == "comments")
                {
                    let mut comment = docx_comment_from_element(&e);
                    docx_finish_comment(&mut comment, &anchors);
                    comments.push(comment.info);
                } else if let Some(comment) = current.as_mut() {
                    docx_note_comment_empty(&e, &name, &stack, comment);
                }
            }
            Ok(event) if is_xml_text_event(&event) => {
                if let Some(comment) = current.as_mut()
                    && comment.in_t
                    && comment.skip_text_depth == 0
                    && let Some(paragraph) = comment.current_paragraph.as_mut()
                {
                    append_xml_text_event(paragraph, &event);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(comment) = current.as_mut() {
                    match name.as_str() {
                        "t" => comment.in_t = false,
                        "delText" | "instrText" => {
                            comment.skip_text_depth = comment.skip_text_depth.saturating_sub(1);
                        }
                        "p" => {
                            if let Some(paragraph) = comment.current_paragraph.take() {
                                comment.paragraphs.push(paragraph);
                            }
                        }
                        "comment" => {
                            if let Some(mut comment) = current.take() {
                                docx_finish_comment(&mut comment, &anchors);
                                comments.push(comment.info);
                            }
                        }
                        _ => {}
                    }
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(comments)
}

fn docx_comment_from_element(element: &BytesStart<'_>) -> DocxCommentBuild {
    let id_raw = attr(element, "id").unwrap_or_default();
    let (id, id_valid) = parse_docx_comment_id(&id_raw);
    DocxCommentBuild {
        info: DocxCommentInfo {
            id,
            id_raw,
            id_valid,
            author: attr(element, "author").unwrap_or_default(),
            date: attr(element, "date").unwrap_or_default(),
            initials: attr(element, "initials").unwrap_or_default(),
            ..DocxCommentInfo::default()
        },
        ..DocxCommentBuild::default()
    }
}

fn docx_note_comment_start(
    element: &BytesStart<'_>,
    name: &str,
    stack: &[String],
    comment: &mut DocxCommentBuild,
) {
    if name == "p" && stack.last().is_some_and(|parent| parent == "comment") {
        comment.current_paragraph = Some(String::new());
    }
    docx_note_comment_empty(element, name, stack, comment);
    if name == "t" {
        comment.in_t = true;
    }
    if name == "delText" || name == "instrText" {
        comment.skip_text_depth += 1;
    }
}

fn docx_note_comment_empty(
    _element: &BytesStart<'_>,
    name: &str,
    _stack: &[String],
    comment: &mut DocxCommentBuild,
) {
    let Some(paragraph) = comment.current_paragraph.as_mut() else {
        return;
    };
    match name {
        "tab" => paragraph.push('\t'),
        "br" | "cr" => paragraph.push('\n'),
        "noBreakHyphen" => paragraph.push('-'),
        _ => {}
    }
}

fn docx_finish_comment(
    comment: &mut DocxCommentBuild,
    anchors: &BTreeMap<String, DocxCommentAnchor>,
) {
    comment.info.text = comment.paragraphs.join("\n");
    if let Some(anchor) = anchors.get(&comment.info.id_raw) {
        comment.info.anchored_to_block = anchor.index;
        comment.info.anchored_to_block_kind = anchor.kind.clone();
    }
}

fn parse_docx_comment_id(value: &str) -> (i64, bool) {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return (0, false);
    }
    value
        .parse::<i64>()
        .map(|id| (id, true))
        .unwrap_or((0, false))
}

#[derive(Clone)]
struct DocxCommentAnchor {
    index: usize,
    kind: String,
    tag: String,
    depth: usize,
}

fn docx_comment_anchors(
    file: &str,
    document_part: &str,
) -> CliResult<BTreeMap<String, DocxCommentAnchor>> {
    let xml = zip_text(file, document_part.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut anchors = BTreeMap::<String, DocxCommentAnchor>::new();
    let mut current_block: Option<DocxCommentAnchor> = None;
    let mut block_index = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.last().is_some_and(|parent| parent == "body")
                    && matches!(name.as_str(), "p" | "tbl")
                {
                    block_index += 1;
                    current_block = Some(DocxCommentAnchor {
                        index: block_index,
                        kind: if name == "p" { "paragraph" } else { "table" }.to_string(),
                        tag: name.clone(),
                        depth: stack.len() + 1,
                    });
                }
                if name == "commentRangeStart" {
                    docx_note_comment_anchor(&mut anchors, current_block.as_ref(), &e);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.last().is_some_and(|parent| parent == "body")
                    && matches!(name.as_str(), "p" | "tbl")
                {
                    block_index += 1;
                }
                if name == "commentRangeStart" {
                    docx_note_comment_anchor(&mut anchors, current_block.as_ref(), &e);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current_block
                    .as_ref()
                    .is_some_and(|block| block.depth == stack.len() && block.tag == name)
                {
                    current_block = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(anchors)
}

fn docx_note_comment_anchor(
    anchors: &mut BTreeMap<String, DocxCommentAnchor>,
    current_block: Option<&DocxCommentAnchor>,
    element: &BytesStart<'_>,
) {
    let Some(block) = current_block else {
        return;
    };
    if let Some(id) = attr(element, "id") {
        anchors.entry(id).or_insert_with(|| block.clone());
    }
}

fn docx_comment_id_counts(comments: &[DocxCommentInfo]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for comment in comments {
        if !comment.id_raw.is_empty() {
            *counts.entry(comment.id_raw.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn docx_comment_json(comment: &DocxCommentInfo, counts: &BTreeMap<String, usize>) -> Value {
    let mut object = Map::new();
    object.insert("id".to_string(), json!(comment.id));
    object.insert("author".to_string(), json!(comment.author));
    if !comment.date.is_empty() {
        object.insert("date".to_string(), json!(comment.date));
    }
    if !comment.initials.is_empty() {
        object.insert("initials".to_string(), json!(comment.initials));
    }
    object.insert("text".to_string(), json!(comment.text));
    object.insert(
        "contentHash".to_string(),
        json!(docx_comment_content_hash(
            &comment.author,
            &comment.date,
            &comment.text
        )),
    );
    if comment.anchored_to_block > 0 {
        object.insert(
            "anchoredToBlock".to_string(),
            json!(comment.anchored_to_block),
        );
    }
    if !comment.anchored_to_block_kind.is_empty() {
        object.insert(
            "anchoredToBlockKind".to_string(),
            json!(comment.anchored_to_block_kind),
        );
    }
    if comment.id_valid {
        let selector = comment.id.to_string();
        object.insert("primarySelector".to_string(), json!(selector));
        object.insert("selectors".to_string(), json!([selector]));
        if counts.get(&comment.id_raw).copied().unwrap_or_default() == 1 {
            object.insert(
                "handle".to_string(),
                json!(format!("H:docx/pt:doc/comment:n:{}", comment.id)),
            );
        }
    }
    Value::Object(object)
}

pub(super) fn docx_comment_content_hash(author: &str, date: &str, text: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(author.as_bytes());
    hash.update([0]);
    hash.update(date.as_bytes());
    hash.update([0]);
    hash.update(text.as_bytes());
    format!("sha256:{:x}", hash.finalize())
}

pub(super) fn docx_comment_info_from_fragment(
    fragment: &str,
) -> CliResult<(DocxCommentInfo, usize)> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut info = DocxCommentInfo::default();
    let mut paragraphs = Vec::<String>::new();
    let mut current_paragraph: Option<String> = None;
    let mut paragraph_count = 0usize;
    let mut in_t = false;
    let mut skip_text_depth = 0usize;
    let mut saw_comment = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_comment {
                    if name != "comment" {
                        return Err(CliError::unexpected("invalid comment XML"));
                    }
                    saw_comment = true;
                    info = docx_comment_from_element(&e).info;
                } else {
                    if name == "p" && stack.last().is_some_and(|parent| parent == "comment") {
                        current_paragraph = Some(String::new());
                        paragraph_count += 1;
                    }
                    if name == "br"
                        && let Some(paragraph) = current_paragraph.as_mut()
                    {
                        paragraph.push('\n');
                    }
                    if name == "tab"
                        && let Some(paragraph) = current_paragraph.as_mut()
                    {
                        paragraph.push('\t');
                    }
                    if name == "t" {
                        in_t = true;
                    }
                    if matches!(name.as_str(), "delText" | "instrText") {
                        skip_text_depth += 1;
                    }
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_comment {
                    if name != "comment" {
                        return Err(CliError::unexpected("invalid comment XML"));
                    }
                    saw_comment = true;
                    info = docx_comment_from_element(&e).info;
                } else if name == "p" && stack.last().is_some_and(|parent| parent == "comment") {
                    paragraphs.push(String::new());
                    paragraph_count += 1;
                } else if name == "br" {
                    if let Some(paragraph) = current_paragraph.as_mut() {
                        paragraph.push('\n');
                    }
                } else if name == "tab"
                    && let Some(paragraph) = current_paragraph.as_mut()
                {
                    paragraph.push('\t');
                }
            }
            Ok(event) if is_xml_text_event(&event) => {
                if in_t
                    && skip_text_depth == 0
                    && let Some(paragraph) = current_paragraph.as_mut()
                {
                    append_xml_text_event(paragraph, &event);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                match name.as_str() {
                    "t" => in_t = false,
                    "delText" | "instrText" => {
                        skip_text_depth = skip_text_depth.saturating_sub(1);
                    }
                    "p" => {
                        if let Some(paragraph) = current_paragraph.take() {
                            paragraphs.push(paragraph);
                        }
                    }
                    _ => {}
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !saw_comment {
        return Err(CliError::unexpected("invalid comment XML"));
    }
    info.text = paragraphs.join("\n");
    Ok((info, paragraph_count))
}
