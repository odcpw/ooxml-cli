use quick_xml::Reader;
use quick_xml::events::Event;
use sha2::{Digest, Sha256};

use crate::{
    CliError, CliResult, attr, decode_xml_text, local_name, needs_xml_space_preserve,
    normalize_xlsx_cell_ref, parse_cell_ref, xml_attr_escape, xml_escape, xml_general_ref,
    zip_text,
};

use super::{XLSX_NS, XlsxCommentInfo, XlsxCommentsDoc};

pub(super) fn read_comments_doc(file: &str, comments_uri: &str) -> CliResult<XlsxCommentsDoc> {
    let xml = zip_text(file, comments_uri.trim_start_matches('/'))?;
    parse_comments_xml(&xml)
}

fn parse_comments_xml(xml: &str) -> CliResult<XlsxCommentsDoc> {
    let authors = parse_comment_authors(xml)?;
    let comments = parse_comment_entries(xml, &authors)?;
    Ok(XlsxCommentsDoc { authors, comments })
}

fn parse_comment_authors(xml: &str) -> CliResult<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut authors = Vec::new();
    let mut current = String::new();
    let mut in_author = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "author" && stack.last().map(String::as_str) == Some("authors") {
                    in_author = true;
                    current.clear();
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "author" && stack.last().map(String::as_str) == Some("authors") {
                    authors.push(String::new());
                }
            }
            Ok(Event::Text(e)) if in_author => {
                current.push_str(&decode_xml_text(e.as_ref()));
            }
            Ok(Event::GeneralRef(e)) if in_author => {
                current.push_str(&xml_general_ref(e.as_ref()));
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "author" && in_author {
                    authors.push(current.clone());
                    in_author = false;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(authors)
}

fn parse_comment_entries(xml: &str, authors: &[String]) -> CliResult<Vec<XlsxCommentInfo>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut comments = Vec::<XlsxCommentInfo>::new();
    let mut current: Option<XlsxCommentInfo> = None;
    let mut in_comment_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "comment" && stack.last().map(String::as_str) == Some("commentList") {
                    let author_id_attr = attr(&e, "authorId").unwrap_or_default();
                    let author = comment_author(authors, &author_id_attr);
                    let cell = attr(&e, "ref").unwrap_or_default();
                    current = Some(make_comment_info(
                        comments.len() as i64,
                        author_id_attr,
                        author,
                        String::new(),
                        cell,
                    ));
                } else if name == "t"
                    && current.is_some()
                    && stack.iter().any(|item| item == "text")
                {
                    in_comment_text = true;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "comment" && stack.last().map(String::as_str) == Some("commentList") {
                    let author_id_attr = attr(&e, "authorId").unwrap_or_default();
                    let author = comment_author(authors, &author_id_attr);
                    let cell = attr(&e, "ref").unwrap_or_default();
                    comments.push(make_comment_info(
                        comments.len() as i64,
                        author_id_attr,
                        author,
                        String::new(),
                        cell,
                    ));
                }
            }
            Ok(Event::Text(e)) if in_comment_text => {
                if let Some(comment) = current.as_mut() {
                    comment.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) if in_comment_text => {
                if let Some(comment) = current.as_mut() {
                    comment.text.push_str(&xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_comment_text = false;
                }
                if name == "comment"
                    && let Some(mut comment) = current.take()
                {
                    refresh_comment_hash_and_anchor(&mut comment);
                    comments.push(comment);
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    renumber_comments(&mut comments);
    Ok(comments)
}

fn comment_author(authors: &[String], author_id_attr: &str) -> String {
    author_id_attr
        .parse::<usize>()
        .ok()
        .and_then(|index| authors.get(index))
        .cloned()
        .unwrap_or_default()
}

pub(super) fn make_comment_info(
    id: i64,
    author_id_attr: String,
    author: String,
    text: String,
    anchored_to_cell: String,
) -> XlsxCommentInfo {
    let mut comment = XlsxCommentInfo {
        id,
        author_id_attr,
        author,
        text,
        content_hash: String::new(),
        anchored_to_cell,
        anchored_to_cell_row: None,
        anchored_to_cell_column: None,
    };
    refresh_comment_hash_and_anchor(&mut comment);
    comment
}

pub(super) fn refresh_comment_hash_and_anchor(comment: &mut XlsxCommentInfo) {
    comment.content_hash = comment_content_hash(&comment.author, &comment.text);
    match parse_comment_cell(&comment.anchored_to_cell) {
        Ok((col, row)) => {
            comment.anchored_to_cell_column = Some(col);
            comment.anchored_to_cell_row = Some(row);
        }
        Err(_) => {
            comment.anchored_to_cell_column = None;
            comment.anchored_to_cell_row = None;
        }
    }
}

fn comment_content_hash(author: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(author.as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    format!("sha256:{}", lower_hex(&hasher.finalize()))
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub(super) fn sort_comments_by_cell(comments: &mut [XlsxCommentInfo]) {
    comments.sort_by_key(|comment| {
        parse_comment_cell(&comment.anchored_to_cell)
            .map(|(col, row)| (row, col))
            .unwrap_or((u32::MAX, u32::MAX))
    });
}

pub(super) fn renumber_comments(comments: &mut [XlsxCommentInfo]) {
    for (idx, comment) in comments.iter_mut().enumerate() {
        comment.id = idx as i64;
        refresh_comment_hash_and_anchor(comment);
    }
}

pub(super) fn comment_index(comments: &[XlsxCommentInfo], comment_id: i64) -> CliResult<usize> {
    comments
        .iter()
        .position(|comment| comment.id == comment_id)
        .ok_or_else(|| CliError::target_not_found("target not found: comment"))
}

pub(super) fn guard_comment_hash(
    comment_id: i64,
    expected: Option<&str>,
    actual: &str,
) -> CliResult<()> {
    let expected = expected.unwrap_or("").trim();
    if !expected.is_empty() && expected != actual {
        return Err(CliError::invalid_args(format!(
            "comment hash mismatch: comment {comment_id} expected {expected} but found {actual}"
        )));
    }
    Ok(())
}

pub(super) fn render_comments_doc(doc: &XlsxCommentsDoc) -> String {
    let mut out = String::new();
    out.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
    out.push_str(&format!(r#"<comments xmlns="{XLSX_NS}">"#));
    out.push_str("<authors>");
    for author in &doc.authors {
        out.push_str("<author>");
        out.push_str(&xml_escape(author));
        out.push_str("</author>");
    }
    out.push_str("</authors><commentList>");
    for comment in &doc.comments {
        out.push_str(&format!(
            r#"<comment ref="{}" authorId="{}">"#,
            xml_attr_escape(&comment.anchored_to_cell),
            xml_attr_escape(&comment.author_id_attr)
        ));
        out.push_str("<text><t");
        if needs_xml_space_preserve(&comment.text) {
            out.push_str(r#" xml:space="preserve""#);
        }
        out.push('>');
        out.push_str(&xml_escape(&comment.text));
        out.push_str("</t></text></comment>");
    }
    out.push_str("</commentList></comments>");
    out
}

pub(super) fn parse_comment_cell(cell: &str) -> CliResult<(u32, u32)> {
    let normalized = normalize_xlsx_cell_ref(cell, "comment ref")?;
    parse_cell_ref(&normalized)
}
