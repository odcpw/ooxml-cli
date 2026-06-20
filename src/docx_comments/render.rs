use quick_xml::Reader;
use quick_xml::events::Event;

use crate::{
    CliError, CliResult, XmlNamedRange, append_docx_text_children, attr, docx_body_block_ranges,
    docx_body_tag, ensure_docx_word_prefix, local_name, word_xml_tag, xml_attr_escape,
    xml_direct_child_ranges, xml_fragment_bounds, xml_open_tag_from_start, xml_tag_prefix,
};
pub(super) fn docx_comments_template() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"></w:comments>"#
        .to_string()
}

pub(super) fn docx_next_comment_id(comments_xml: &str) -> i64 {
    let mut reader = Reader::from_str(comments_xml);
    let mut max_id = -1_i64;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "comment" =>
            {
                if let Some(id) = attr(&e, "id").and_then(|value| value.parse::<i64>().ok())
                    && id > max_id
                {
                    max_id = id;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    max_id + 1
}

pub(super) fn append_docx_comment_xml(
    comments_xml: &str,
    comment_id: i64,
    author: &str,
    date: &str,
    initials: &str,
    text: &str,
) -> CliResult<String> {
    let root_tag = docx_comments_root_tag(comments_xml)?;
    let prefix = xml_tag_prefix(&root_tag);
    let comment = render_docx_comment(&prefix, comment_id, author, date, initials, text);
    let close_tag = format!("</{root_tag}>");
    if let Some(pos) = comments_xml.rfind(&close_tag) {
        let mut out = String::with_capacity(comments_xml.len() + comment.len());
        out.push_str(&comments_xml[..pos]);
        out.push_str(&comment);
        out.push_str(&comments_xml[pos..]);
        return Ok(out);
    }

    let start = comments_xml
        .find(&format!("<{root_tag}"))
        .ok_or_else(|| CliError::unexpected("comments part has no w:comments root"))?;
    let open_end = comments_xml[start..]
        .find('>')
        .map(|offset| start + offset)
        .ok_or_else(|| CliError::unexpected("comments part has no w:comments root"))?;
    let start_tag = &comments_xml[start..=open_end];
    if !start_tag.trim_end().ends_with("/>") {
        return Err(CliError::unexpected(
            "comments part has no closing w:comments tag",
        ));
    }
    let mut out = String::with_capacity(comments_xml.len() + comment.len() + close_tag.len());
    out.push_str(&comments_xml[..start]);
    out.push_str(&xml_open_tag_from_start(start_tag));
    out.push_str(&comment);
    out.push_str(&close_tag);
    out.push_str(&comments_xml[open_end + 1..]);
    Ok(out)
}

fn docx_comments_root_tag(comments_xml: &str) -> CliResult<String> {
    let mut reader = Reader::from_str(comments_xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if local_name(e.name().as_ref()) == "comments" {
                    return Ok(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
                return Err(CliError::unexpected("comments part has no w:comments root"));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::unexpected("comments part has no w:comments root"))
}

pub(super) fn render_docx_comment(
    prefix: &str,
    comment_id: i64,
    author: &str,
    date: &str,
    initials: &str,
    text: &str,
) -> String {
    let comment = word_xml_tag(prefix, "comment");
    let p = word_xml_tag(prefix, "p");
    let r = word_xml_tag(prefix, "r");
    let mut out = String::new();
    out.push('<');
    out.push_str(&comment);
    out.push(' ');
    out.push_str(&word_attr_name(prefix, "id"));
    out.push_str("=\"");
    out.push_str(&comment_id.to_string());
    out.push_str("\" ");
    out.push_str(&word_attr_name(prefix, "author"));
    out.push_str("=\"");
    out.push_str(&xml_attr_escape(author));
    out.push('"');
    if !date.is_empty() {
        out.push(' ');
        out.push_str(&word_attr_name(prefix, "date"));
        out.push_str("=\"");
        out.push_str(&xml_attr_escape(date));
        out.push('"');
    }
    if !initials.is_empty() {
        out.push(' ');
        out.push_str(&word_attr_name(prefix, "initials"));
        out.push_str("=\"");
        out.push_str(&xml_attr_escape(initials));
        out.push('"');
    }
    out.push('>');
    out.push('<');
    out.push_str(&p);
    out.push('>');
    if !text.is_empty() {
        out.push('<');
        out.push_str(&r);
        out.push('>');
        append_docx_text_children(&mut out, prefix, text);
        out.push_str("</");
        out.push_str(&r);
        out.push('>');
    }
    out.push_str("</");
    out.push_str(&p);
    out.push('>');
    out.push_str("</");
    out.push_str(&comment);
    out.push('>');
    out
}

pub(super) fn insert_docx_comment_markers_xml(
    document_xml: &str,
    anchor_index: usize,
    comment_id: i64,
) -> CliResult<String> {
    let body_tag = docx_body_tag(document_xml)?;
    let prefix = xml_tag_prefix(&body_tag);
    let working = if prefix.is_empty() {
        ensure_docx_word_prefix(document_xml)?
    } else {
        document_xml.to_string()
    };
    let body_tag = docx_body_tag(&working)?;
    let prefix = xml_tag_prefix(&body_tag);
    let blocks = docx_body_block_ranges(&working, &body_tag)?;
    let block = blocks.get(anchor_index - 1).ok_or_else(|| {
        CliError::invalid_args(format!("comment anchor block out of range: {anchor_index}"))
    })?;
    if block.kind != "p" {
        return Err(CliError::invalid_args(format!(
            "comment anchor block is not a paragraph: block {anchor_index} is table"
        )));
    }
    let fragment = &working[block.start..block.end];
    let updated = insert_docx_comment_markers_in_paragraph(fragment, &prefix, comment_id)?;
    let mut out = String::with_capacity(working.len() + updated.len());
    out.push_str(&working[..block.start]);
    out.push_str(&updated);
    out.push_str(&working[block.end..]);
    Ok(out)
}

fn insert_docx_comment_markers_in_paragraph(
    paragraph: &str,
    prefix: &str,
    comment_id: i64,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(paragraph)?;
    let start_tag = &paragraph[..=open_end];
    let open_tag = xml_open_tag_from_start(start_tag);
    let close_tag = format!("</{tag_name}>");
    let content_start = open_tag.len();
    let normalized = if self_closing {
        format!("{open_tag}{close_tag}")
    } else {
        paragraph.to_string()
    };
    let content_end = if self_closing {
        content_start
    } else {
        close_start
    };
    let children = xml_direct_child_ranges(&normalized, content_start, content_end)?;
    let start_marker = render_docx_comment_range_marker(prefix, "commentRangeStart", comment_id);
    let end_marker = render_docx_comment_range_marker(prefix, "commentRangeEnd", comment_id);
    let reference = render_docx_comment_reference_run(prefix, comment_id);
    let run_children: Vec<&XmlNamedRange> =
        children.iter().filter(|child| child.kind == "r").collect();
    if let (Some(first_run), Some(last_run)) = (run_children.first(), run_children.last()) {
        let mut out = String::with_capacity(
            normalized.len() + start_marker.len() + end_marker.len() + reference.len(),
        );
        out.push_str(&normalized[..first_run.start]);
        out.push_str(&start_marker);
        out.push_str(&normalized[first_run.start..last_run.end]);
        out.push_str(&end_marker);
        out.push_str(&reference);
        out.push_str(&normalized[last_run.end..]);
        return Ok(out);
    }

    let insert_at = children
        .iter()
        .find(|child| child.kind == "pPr")
        .map(|child| child.end)
        .unwrap_or(content_start);
    let mut out = String::with_capacity(
        normalized.len() + start_marker.len() + end_marker.len() + reference.len(),
    );
    out.push_str(&normalized[..insert_at]);
    out.push_str(&start_marker);
    out.push_str(&end_marker);
    out.push_str(&reference);
    out.push_str(&normalized[insert_at..]);
    Ok(out)
}

fn render_docx_comment_range_marker(prefix: &str, local: &str, comment_id: i64) -> String {
    let tag = word_xml_tag(prefix, local);
    format!(
        r#"<{tag} {}="{}"/>"#,
        word_attr_name(prefix, "id"),
        comment_id
    )
}

fn render_docx_comment_reference_run(prefix: &str, comment_id: i64) -> String {
    let r = word_xml_tag(prefix, "r");
    let reference = word_xml_tag(prefix, "commentReference");
    format!(
        r#"<{r}><{reference} {}="{}"/></{r}>"#,
        word_attr_name(prefix, "id"),
        comment_id
    )
}

fn word_attr_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        format!("w:{local}")
    } else {
        format!("{prefix}:{local}")
    }
}
