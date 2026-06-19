use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use super::{PptxSlidePartRef, pptx_slide_part_refs};
use crate::{
    CliError, CliResult, attr, decode_xml_text, local_name, package_type, relationship_entries,
    relationships_part_for, resolve_relationship_target, selector_candidates, zip_entry_exists,
    zip_entry_names, zip_text,
};

#[derive(Clone, Default)]
struct PptxCommentAuthor {
    name: String,
    initials: String,
}

#[derive(Clone)]
struct PptxCommentInfo {
    id: i64,
    author_id: i64,
    author: String,
    initials: String,
    date: String,
    text: String,
    content_hash: String,
    handle: String,
    primary_selector: String,
    selectors: Vec<String>,
}

#[derive(Default)]
struct PptxCommentBuild {
    id: i64,
    author_id: i64,
    date: String,
    text: String,
}

pub(crate) fn pptx_comments_list(
    file: &str,
    slide_filter: Option<u32>,
    comment_id: Option<i64>,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    if matches!(slide_filter, Some(0)) {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    if comment_id.is_some() && slide_filter.is_none() {
        return Err(CliError::invalid_args("--comment-id requires --slide"));
    }

    let slides = pptx_slide_part_refs(file)?;
    if let Some(slide) = slide_filter
        && slide as usize > slides.len()
    {
        return Err(CliError::invalid_args(format!(
            "--slide {slide} out of range (presentation has {} slides)",
            slides.len()
        )));
    }

    let entries = zip_entry_names(file)?;
    let authors = pptx_comment_authors(file, &entries)?;
    let mut slide_values = Vec::new();
    for slide in &slides {
        if let Some(wanted) = slide_filter
            && slide.number != wanted
        {
            continue;
        }
        let (mut value, comments) = pptx_slide_comments(file, &entries, &authors, slide)?;
        if let Some(comment_id) = comment_id {
            let filtered = comments
                .iter()
                .filter(|comment| comment.id == comment_id)
                .cloned()
                .collect::<Vec<_>>();
            if filtered.is_empty() {
                return Err(pptx_comment_not_found_error(
                    &comments,
                    slide.number,
                    comment_id,
                ));
            }
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "comments".to_string(),
                    Value::Array(filtered.iter().map(pptx_comment_json).collect()),
                );
            }
        }
        slide_values.push(value);
    }

    Ok(json!({
        "file": file,
        "slides": slide_values,
    }))
}

fn pptx_slide_comments(
    file: &str,
    entries: &[String],
    authors: &BTreeMap<i64, PptxCommentAuthor>,
    slide: &PptxSlidePartRef,
) -> CliResult<(Value, Vec<PptxCommentInfo>)> {
    let mut output = Map::new();
    output.insert("slide".to_string(), json!(slide.number));
    output.insert(
        "slidePartUri".to_string(),
        json!(format!("/{}", slide.part.trim_start_matches('/'))),
    );
    let comments_part = pptx_slide_comments_part(file, entries, &slide.part);
    let comments = if let Some(comments_part) = comments_part.as_deref() {
        output.insert("commentsPart".to_string(), json!(comments_part));
        let xml = zip_text(file, comments_part.trim_start_matches('/'))?;
        pptx_comments_from_xml(&xml, authors, slide.slide_id)
    } else {
        Vec::new()
    };
    output.insert(
        "comments".to_string(),
        Value::Array(comments.iter().map(pptx_comment_json).collect()),
    );
    Ok((Value::Object(output), comments))
}

fn pptx_slide_comments_part(file: &str, entries: &[String], slide_part: &str) -> Option<String> {
    let rels = relationship_entries(file, &relationships_part_for(slide_part)).unwrap_or_default();
    let slide_uri = format!("/{}", slide_part.trim_start_matches('/'));
    for rel in rels {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"
        {
            let uri = resolve_relationship_target(&slide_uri, &rel.target);
            return zip_entry_exists(entries, &uri).then_some(uri);
        }
    }
    None
}

fn pptx_comment_authors(
    file: &str,
    entries: &[String],
) -> CliResult<BTreeMap<i64, PptxCommentAuthor>> {
    let mut authors = BTreeMap::new();
    let Some(part) = pptx_comment_authors_part(file, entries) else {
        return Ok(authors);
    };
    let xml = zip_text(file, part.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cmAuthor" =>
            {
                let id = attr(&e, "id")
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or_default();
                authors.insert(
                    id,
                    PptxCommentAuthor {
                        name: attr(&e, "name").unwrap_or_default(),
                        initials: attr(&e, "initials").unwrap_or_default(),
                    },
                );
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(authors)
}

fn pptx_comment_authors_part(file: &str, entries: &[String]) -> Option<String> {
    let rels = relationship_entries(file, "ppt/_rels/presentation.xml.rels").unwrap_or_default();
    for rel in rels {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/commentAuthors"
        {
            let uri = resolve_relationship_target("/ppt/presentation.xml", &rel.target);
            if zip_entry_exists(entries, &uri) {
                return Some(uri);
            }
            return None;
        }
    }
    let conventional = "/ppt/commentAuthors.xml";
    zip_entry_exists(entries, conventional).then(|| conventional.to_string())
}

fn pptx_comments_from_xml(
    xml: &str,
    authors: &BTreeMap<i64, PptxCommentAuthor>,
    slide_id: u32,
) -> Vec<PptxCommentInfo> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut comments = Vec::new();
    let mut current: Option<PptxCommentBuild> = None;
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "cm" => {
                current = Some(PptxCommentBuild {
                    id: attr(&e, "idx")
                        .and_then(|value| value.parse::<i64>().ok())
                        .unwrap_or_default(),
                    author_id: attr(&e, "authorId")
                        .and_then(|value| value.parse::<i64>().ok())
                        .unwrap_or_default(),
                    date: attr(&e, "dt").unwrap_or_default(),
                    text: String::new(),
                });
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "cm" => {
                comments.push(pptx_comment_from_build(
                    PptxCommentBuild {
                        id: attr(&e, "idx")
                            .and_then(|value| value.parse::<i64>().ok())
                            .unwrap_or_default(),
                        author_id: attr(&e, "authorId")
                            .and_then(|value| value.parse::<i64>().ok())
                            .unwrap_or_default(),
                        date: attr(&e, "dt").unwrap_or_default(),
                        text: String::new(),
                    },
                    authors,
                    slide_id,
                ));
            }
            Ok(Event::Start(e)) if current.is_some() && local_name(e.name().as_ref()) == "text" => {
                in_text = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "text" => {
                in_text = false;
            }
            Ok(Event::Text(e)) if in_text => {
                if let Some(comment) = current.as_mut() {
                    comment.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) if in_text => {
                if let Some(comment) = current.as_mut() {
                    comment.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "cm" => {
                if let Some(comment) = current.take() {
                    comments.push(pptx_comment_from_build(comment, authors, slide_id));
                }
                in_text = false;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    comments
}

fn pptx_comment_from_build(
    comment: PptxCommentBuild,
    authors: &BTreeMap<i64, PptxCommentAuthor>,
    slide_id: u32,
) -> PptxCommentInfo {
    let author = authors.get(&comment.author_id).cloned().unwrap_or_default();
    let content_hash = pptx_comment_content_hash(&author.name, &comment.date, &comment.text);
    let handle = pptx_comment_handle(slide_id, comment.id, comment.author_id);
    let primary_selector = pptx_comment_primary_selector(&handle, comment.id, comment.author_id);
    let selectors = pptx_comment_selectors(&handle, comment.id, comment.author_id);
    PptxCommentInfo {
        id: comment.id,
        author_id: comment.author_id,
        author: author.name,
        initials: author.initials,
        date: comment.date,
        text: comment.text,
        content_hash,
        handle,
        primary_selector,
        selectors,
    }
}

fn pptx_comment_json(comment: &PptxCommentInfo) -> Value {
    let mut output = Map::new();
    output.insert("id".to_string(), json!(comment.id));
    output.insert("authorId".to_string(), json!(comment.author_id));
    if !comment.handle.is_empty() {
        output.insert("handle".to_string(), json!(comment.handle));
    }
    if !comment.primary_selector.is_empty() {
        output.insert(
            "primarySelector".to_string(),
            json!(comment.primary_selector),
        );
    }
    if !comment.selectors.is_empty() {
        output.insert("selectors".to_string(), json!(comment.selectors));
    }
    output.insert("author".to_string(), json!(comment.author));
    if !comment.initials.is_empty() {
        output.insert("initials".to_string(), json!(comment.initials));
    }
    if !comment.date.is_empty() {
        output.insert("date".to_string(), json!(comment.date));
    }
    output.insert("text".to_string(), json!(comment.text));
    output.insert("contentHash".to_string(), json!(comment.content_hash));
    Value::Object(output)
}

fn pptx_comment_content_hash(author: &str, date: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(author.as_bytes());
    hasher.update([0]);
    hasher.update(date.as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn pptx_comment_handle(slide_id: u32, comment_id: i64, author_id: i64) -> String {
    if slide_id == 0 || comment_id < 0 || author_id < 0 {
        return String::new();
    }
    format!("H:pptx/s:{slide_id}/comment:idx:{comment_id}:authorId:{author_id}")
}

fn pptx_comment_primary_selector(handle: &str, comment_id: i64, author_id: i64) -> String {
    if !handle.trim().is_empty() {
        handle.to_string()
    } else {
        format!("comment:{comment_id}:authorId:{author_id}")
    }
}

fn pptx_comment_selectors(handle: &str, comment_id: i64, author_id: i64) -> Vec<String> {
    let mut selectors = Vec::new();
    if !handle.trim().is_empty() {
        selectors.push(handle.to_string());
    }
    selectors.push(format!("comment:{comment_id}:authorId:{author_id}"));
    selectors.push(format!("comment:{comment_id}"));
    selectors.push(comment_id.to_string());
    selectors.push(format!("authorId:{author_id}"));
    selectors
}

fn pptx_comment_not_found_error(
    comments: &[PptxCommentInfo],
    slide: u32,
    comment_id: i64,
) -> CliError {
    let selector = format!("comment:{comment_id}");
    let selector_items = comments
        .iter()
        .map(|comment| {
            (
                comment.primary_selector.as_str(),
                comment.selectors.as_slice(),
            )
        })
        .collect::<Vec<_>>();
    let candidates = selector_candidates(&selector_items, &selector, 3);
    let mut message = format!("comment not found: {selector}");
    if !candidates.is_empty() {
        message.push_str(&format!("; did you mean: {}", candidates.join(", ")));
    }
    message.push_str(&format!(
        "; discover with `ooxml --json pptx comments list <file> --slide {slide}`"
    ));
    CliError::target_not_found(message)
}
