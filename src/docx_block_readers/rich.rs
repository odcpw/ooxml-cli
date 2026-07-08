use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use super::{docx_para_id_ns, docx_word_val_ns, element_in_ns};
use crate::{CliError, CliResult, DOCX_W_NS, append_xml_text_event, is_xml_text_event, local_name};
#[derive(Default)]
struct DocxRichParagraphState {
    text: String,
    style: String,
    para_id: String,
    runs: Vec<DocxRichRunInfo>,
}

enum DocxRichParagraphContext {
    Body,
    TableCell,
}

#[derive(Clone, Default)]
struct DocxRichRunInfo {
    text: String,
    bold: bool,
    italic: bool,
    underline: String,
    color: String,
    size: String,
}

#[derive(Default)]
struct DocxRichRunState {
    info: DocxRichRunInfo,
}

#[derive(Default)]
struct DocxRichTableState {
    rows: Vec<Vec<String>>,
    current_row: Option<Vec<String>>,
    current_cell: Option<Vec<String>>,
    merged: bool,
}

pub(crate) struct DocxRichBlockReport {
    pub(crate) index: usize,
    pub(crate) kind: &'static str,
    pub(crate) text: String,
    pub(crate) style: String,
    pub(crate) para_id: String,
    handle: String,
    pub(crate) content_hash: String,
    runs: Vec<DocxRichRunInfo>,
    pub(crate) table_rows: Vec<Vec<String>>,
    pub(crate) table_merged: bool,
}

pub(crate) fn docx_rich_block_reports(
    xml: &str,
    include_runs: bool,
) -> CliResult<Vec<DocxRichBlockReport>> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let para_id_counts = docx_body_para_id_counts(xml)?;
    let mut stack: Vec<String> = Vec::new();
    let mut word_stack: Vec<bool> = Vec::new();
    let mut blocks = Vec::new();
    let mut current_paragraph: Option<DocxRichParagraphState> = None;
    let mut paragraph_context: Option<DocxRichParagraphContext> = None;
    let mut current_run: Option<DocxRichRunState> = None;
    let mut current_table: Option<DocxRichTableState> = None;
    let mut body_table_depth = 0usize;
    let mut in_t = false;
    let mut skip_text_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let parent_is_word = word_stack.last().copied().unwrap_or(false);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if parent == Some("body") && name == "tbl" {
                    current_table = Some(DocxRichTableState::default());
                    body_table_depth = 1;
                } else if current_table.is_some() && name == "tbl" {
                    body_table_depth += 1;
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tbl")
                    && is_word
                    && name == "tr"
                {
                    if let Some(table) = current_table.as_mut() {
                        table.current_row = Some(Vec::new());
                    }
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tr")
                    && parent_is_word
                    && is_word
                    && name == "tc"
                {
                    if let Some(table) = current_table.as_mut() {
                        table.current_cell = Some(Vec::new());
                    }
                } else if parent == Some("body") && name == "p" {
                    current_paragraph = Some(DocxRichParagraphState {
                        para_id: docx_para_id_ns(&e, reader.resolver()).unwrap_or_default(),
                        ..DocxRichParagraphState::default()
                    });
                    paragraph_context = Some(DocxRichParagraphContext::Body);
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tc")
                    && parent_is_word
                    && is_word
                    && name == "p"
                {
                    current_paragraph = Some(DocxRichParagraphState::default());
                    paragraph_context = Some(DocxRichParagraphContext::TableCell);
                } else if include_runs
                    && matches!(paragraph_context, Some(DocxRichParagraphContext::Body))
                    && parent == Some("p")
                    && is_word
                    && name == "r"
                {
                    current_run = Some(DocxRichRunState::default());
                }
                if current_table.is_some()
                    && is_word
                    && matches!(name.as_str(), "gridSpan" | "vMerge")
                    && let Some(table) = current_table.as_mut()
                {
                    table.merged = true;
                }

                docx_rich_note_empty_or_start(
                    &e,
                    reader.resolver(),
                    &stack,
                    &word_stack,
                    &mut current_paragraph,
                    &mut current_run,
                    skip_text_depth,
                );
                if name == "t" {
                    in_t = true;
                }
                if name == "delText" || name == "instrText" {
                    skip_text_depth += 1;
                }
                stack.push(name);
                word_stack.push(is_word);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let parent_is_word = word_stack.last().copied().unwrap_or(false);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if parent == Some("body") && name == "p" {
                    let paragraph = DocxRichParagraphState {
                        para_id: docx_para_id_ns(&e, reader.resolver()).unwrap_or_default(),
                        ..DocxRichParagraphState::default()
                    };
                    blocks.push(docx_rich_paragraph_report(
                        blocks.len() + 1,
                        paragraph,
                        &para_id_counts,
                    ));
                } else if parent == Some("body") && name == "tbl" {
                    blocks.push(docx_rich_table_report(blocks.len() + 1, Vec::new(), false));
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tr")
                    && parent_is_word
                    && is_word
                    && name == "tc"
                {
                    if let Some(row) = current_table
                        .as_mut()
                        .and_then(|table| table.current_row.as_mut())
                    {
                        row.push(String::new());
                    }
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tbl")
                    && is_word
                    && name == "tr"
                {
                    if let Some(table) = current_table.as_mut() {
                        table.rows.push(Vec::new());
                    }
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tc")
                    && parent_is_word
                    && is_word
                    && name == "p"
                    && let Some(cell) = current_table
                        .as_mut()
                        .and_then(|table| table.current_cell.as_mut())
                {
                    cell.push(String::new());
                }
                if current_table.is_some()
                    && is_word
                    && matches!(name.as_str(), "gridSpan" | "vMerge")
                    && let Some(table) = current_table.as_mut()
                {
                    table.merged = true;
                }
                docx_rich_note_empty_or_start(
                    &e,
                    reader.resolver(),
                    &stack,
                    &word_stack,
                    &mut current_paragraph,
                    &mut current_run,
                    skip_text_depth,
                );
            }
            Ok(event) if in_t && skip_text_depth == 0 && is_xml_text_event(&event) => {
                let mut text = String::new();
                append_xml_text_event(&mut text, &event);
                docx_rich_append_text(&mut current_paragraph, &mut current_run, &text);
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                match name.as_str() {
                    "t" => in_t = false,
                    "delText" | "instrText" => {
                        skip_text_depth = skip_text_depth.saturating_sub(1);
                    }
                    "r" => {
                        if let Some(run) = current_run.take()
                            && let Some(paragraph) = current_paragraph.as_mut()
                            && docx_rich_run_has_content(&run.info)
                        {
                            paragraph.runs.push(run.info);
                        }
                    }
                    "p" => {
                        if let (Some(paragraph), Some(context)) =
                            (current_paragraph.take(), paragraph_context.take())
                        {
                            match context {
                                DocxRichParagraphContext::Body => {
                                    blocks.push(docx_rich_paragraph_report(
                                        blocks.len() + 1,
                                        paragraph,
                                        &para_id_counts,
                                    ));
                                }
                                DocxRichParagraphContext::TableCell => {
                                    if let Some(cell) = current_table
                                        .as_mut()
                                        .and_then(|table| table.current_cell.as_mut())
                                    {
                                        cell.push(paragraph.text);
                                    }
                                }
                            }
                        }
                    }
                    "tc" => {
                        if body_table_depth == 1
                            && let Some(table) = current_table.as_mut()
                            && let Some(cell) = table.current_cell.take()
                            && let Some(row) = table.current_row.as_mut()
                        {
                            row.push(cell.join("\n"));
                        }
                    }
                    "tr" => {
                        if body_table_depth == 1
                            && let Some(table) = current_table.as_mut()
                            && let Some(row) = table.current_row.take()
                        {
                            table.rows.push(row);
                        }
                    }
                    "tbl" => {
                        if body_table_depth == 1 {
                            body_table_depth = 0;
                            if let Some(table) = current_table.take() {
                                blocks.push(docx_rich_table_report(
                                    blocks.len() + 1,
                                    table.rows,
                                    table.merged,
                                ));
                            }
                        } else if body_table_depth > 1 {
                            body_table_depth -= 1;
                        } else if let Some(table) = current_table.take() {
                            blocks.push(docx_rich_table_report(
                                blocks.len() + 1,
                                table.rows,
                                table.merged,
                            ));
                        }
                    }
                    _ => {}
                }
                stack.pop();
                word_stack.pop();
            }
            Ok(Event::Eof) => {
                if !stack.is_empty() {
                    return Err(CliError::unexpected("invalid DOCX XML"));
                }
                break;
            }
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "failed to extract DOCX blocks: {err}"
                )));
            }
            _ => {}
        }
    }

    Ok(blocks)
}

fn docx_rich_note_empty_or_start(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    stack: &[String],
    word_stack: &[bool],
    current_paragraph: &mut Option<DocxRichParagraphState>,
    current_run: &mut Option<DocxRichRunState>,
    skip_text_depth: usize,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if matches!(name, "tab" | "br" | "cr" | "noBreakHyphen") && skip_text_depth == 0 {
        let text = match name {
            "tab" => "\t",
            "br" | "cr" => "\n",
            "noBreakHyphen" => "-",
            _ => "",
        };
        docx_rich_append_text(current_paragraph, current_run, text);
        return;
    }

    if let Some(paragraph) = current_paragraph.as_mut()
        && name == "pStyle"
        && element_in_ns(resolver, element, DOCX_W_NS)
        && stack.last().is_some_and(|parent| parent == "pPr")
        && word_stack.last().copied().unwrap_or(false)
        && let Some(style) = docx_word_val_ns(element, resolver).filter(|style| !style.is_empty())
    {
        paragraph.style = style;
    }

    if stack.last().is_some_and(|parent| parent == "rPr")
        && word_stack.last().copied().unwrap_or(false)
        && element_in_ns(resolver, element, DOCX_W_NS)
        && let Some(run) = current_run.as_mut()
    {
        docx_rich_note_run_prop(element, resolver, name, &mut run.info);
    }
}

fn docx_rich_note_run_prop(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    name: &str,
    run: &mut DocxRichRunInfo,
) {
    match name {
        "b" => run.bold = docx_word_toggle_enabled(element, resolver),
        "i" => run.italic = docx_word_toggle_enabled(element, resolver),
        "u" => {
            let value = docx_word_val_ns(element, resolver).unwrap_or_else(|| "single".to_string());
            if value != "none" && value != "0" {
                run.underline = value;
            }
        }
        "color" => {
            if let Some(value) = docx_word_val_ns(element, resolver) {
                run.color = value;
            }
        }
        "sz" => {
            if let Some(value) = docx_word_val_ns(element, resolver) {
                run.size = value;
            }
        }
        _ => {}
    }
}

fn docx_word_toggle_enabled(element: &BytesStart<'_>, resolver: &NamespaceResolver) -> bool {
    let Some(value) = docx_word_val_ns(element, resolver) else {
        return true;
    };
    match value.to_ascii_lowercase().as_str() {
        "" | "1" | "true" | "on" => true,
        "0" | "false" | "off" => false,
        _ => true,
    }
}

fn docx_rich_append_text(
    current_paragraph: &mut Option<DocxRichParagraphState>,
    current_run: &mut Option<DocxRichRunState>,
    text: &str,
) {
    if let Some(paragraph) = current_paragraph.as_mut() {
        paragraph.text.push_str(text);
    }
    if let Some(run) = current_run.as_mut() {
        run.info.text.push_str(text);
    }
}

fn docx_rich_run_has_content(run: &DocxRichRunInfo) -> bool {
    !run.text.is_empty()
        || run.bold
        || run.italic
        || !run.underline.is_empty()
        || !run.color.is_empty()
        || !run.size.is_empty()
}

fn docx_rich_paragraph_report(
    index: usize,
    paragraph: DocxRichParagraphState,
    para_id_counts: &BTreeMap<String, usize>,
) -> DocxRichBlockReport {
    let normalized_para_id = paragraph.para_id.trim().to_ascii_uppercase();
    let handle = if !paragraph.para_id.is_empty()
        && para_id_counts
            .get(&normalized_para_id)
            .copied()
            .unwrap_or_default()
            == 1
    {
        format!("H:docx/pt:doc/para:m:{}", paragraph.para_id)
    } else {
        String::new()
    };
    let content_hash = docx_rich_block_content_hash("paragraph", &paragraph.style, &paragraph.text);
    DocxRichBlockReport {
        index,
        kind: "paragraph",
        text: paragraph.text,
        style: paragraph.style,
        para_id: paragraph.para_id,
        handle,
        content_hash,
        runs: paragraph.runs,
        table_rows: Vec::new(),
        table_merged: false,
    }
}

fn docx_rich_table_report(
    index: usize,
    rows: Vec<Vec<String>>,
    merged: bool,
) -> DocxRichBlockReport {
    let text = docx_rich_table_text(&rows);
    let content_hash = docx_rich_block_content_hash("table", "", &text);
    DocxRichBlockReport {
        index,
        kind: "table",
        text,
        style: String::new(),
        para_id: String::new(),
        handle: String::new(),
        content_hash,
        runs: Vec::new(),
        table_rows: rows,
        table_merged: merged,
    }
}

pub(crate) fn docx_rich_block_json(report: DocxRichBlockReport) -> Value {
    let mut block = Map::new();
    block.insert("id".to_string(), json!(format!("body.b{}", report.index)));
    block.insert("index".to_string(), json!(report.index));
    block.insert("kind".to_string(), json!(report.kind));
    block.insert("text".to_string(), json!(report.text));
    block.insert(
        "primarySelector".to_string(),
        json!(report.index.to_string()),
    );
    block.insert("selectors".to_string(), json!([report.index.to_string()]));
    if !report.para_id.is_empty() {
        block.insert("paraId".to_string(), json!(report.para_id));
    }
    if !report.handle.is_empty() {
        block.insert("handle".to_string(), json!(report.handle));
    }
    block.insert("contentHash".to_string(), json!(report.content_hash));
    if report.kind == "paragraph" {
        let mut paragraph = Map::new();
        if !report.style.is_empty() {
            paragraph.insert("style".to_string(), json!(report.style));
        }
        if !report.runs.is_empty() {
            paragraph.insert(
                "runs".to_string(),
                Value::Array(report.runs.into_iter().map(docx_rich_run_json).collect()),
            );
        }
        block.insert("paragraph".to_string(), Value::Object(paragraph));
    } else if report.kind == "table" {
        let rows: Vec<Value> = report
            .table_rows
            .iter()
            .map(|row| {
                let cells: Vec<Value> = row.iter().map(|text| json!({"text": text})).collect();
                json!({"cells": cells})
            })
            .collect();
        block.insert("table".to_string(), json!({"rows": rows}));
    }
    Value::Object(block)
}

fn docx_rich_run_json(run: DocxRichRunInfo) -> Value {
    let mut object = Map::new();
    object.insert("text".to_string(), json!(run.text));
    if run.bold {
        object.insert("bold".to_string(), json!(true));
    }
    if run.italic {
        object.insert("italic".to_string(), json!(true));
    }
    if !run.underline.is_empty() {
        object.insert("underline".to_string(), json!(run.underline));
    }
    if !run.color.is_empty() {
        object.insert("color".to_string(), json!(run.color));
    }
    if !run.size.is_empty() {
        object.insert("size".to_string(), json!(run.size));
    }
    Value::Object(object)
}

fn docx_rich_table_text(rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|row| row.join("\t"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn docx_rich_block_content_hash(kind: &str, style: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.as_bytes());
    hasher.update([0]);
    hasher.update(style.as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

pub(crate) fn docx_body_para_id_counts(xml: &str) -> CliResult<BTreeMap<String, usize>> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<String> = Vec::new();
    let mut word_stack: Vec<bool> = Vec::new();
    let mut counts = BTreeMap::new();
    let mut saw_root = false;
    let mut saw_body = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.is_empty() {
                    if name != "document" || !is_word {
                        return Err(CliError::unexpected("document root element not found"));
                    }
                    saw_root = true;
                }
                if stack.last().is_some_and(|parent| parent == "document")
                    && word_stack.last().copied().unwrap_or(false)
                    && name == "body"
                    && is_word
                {
                    saw_body = true;
                }
                if stack.last().is_some_and(|parent| parent == "body")
                    && name == "p"
                    && let Some(para_id) = docx_para_id_ns(&e, reader.resolver())
                {
                    *counts.entry(para_id.to_ascii_uppercase()).or_insert(0) += 1;
                }
                stack.push(name);
                word_stack.push(is_word);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.is_empty() {
                    if name != "document" || !is_word {
                        return Err(CliError::unexpected("document root element not found"));
                    }
                    saw_root = true;
                }
                if stack.last().is_some_and(|parent| parent == "document")
                    && word_stack.last().copied().unwrap_or(false)
                    && name == "body"
                    && is_word
                {
                    saw_body = true;
                }
                if stack.last().is_some_and(|parent| parent == "body")
                    && name == "p"
                    && let Some(para_id) = docx_para_id_ns(&e, reader.resolver())
                {
                    *counts.entry(para_id.to_ascii_uppercase()).or_insert(0) += 1;
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
                word_stack.pop();
            }
            Ok(Event::Eof) => {
                if !stack.is_empty() {
                    return Err(CliError::unexpected("invalid DOCX XML"));
                }
                break;
            }
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "failed to extract DOCX blocks: {err}"
                )));
            }
            _ => {}
        }
    }
    if !saw_root {
        return Err(CliError::unexpected("document root element not found"));
    }
    if !saw_body {
        return Err(CliError::unexpected("document body element not found"));
    }
    Ok(counts)
}
