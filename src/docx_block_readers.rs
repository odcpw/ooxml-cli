mod rich;

pub(crate) use rich::{DocxRichBlockReport, docx_rich_block_json, docx_rich_block_reports};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, NamespaceResolver, ResolveResult};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::{
    DOCX_W_NS, DOCX_W14_NS, append_xml_text_event, attr, attr_prefixed_ns, is_xml_text_event,
    local_name,
};
#[derive(Default)]
struct DocxParagraphState {
    text: String,
    style: Option<String>,
    para_id: Option<String>,
}

enum DocxParagraphContext {
    Body,
    TableCell,
}

#[derive(Default)]
struct DocxTableState {
    rows: Vec<Vec<String>>,
    current_row: Option<Vec<String>>,
    current_cell: Option<Vec<String>>,
}

pub(crate) fn docx_blocks(xml: &str) -> Vec<Value> {
    let mut reader = Reader::from_str(xml);
    let para_id_counts = docx_para_id_counts(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut blocks = Vec::new();
    let mut current_paragraph: Option<DocxParagraphState> = None;
    let mut paragraph_context: Option<DocxParagraphContext> = None;
    let mut current_table: Option<DocxTableState> = None;
    let mut in_t = false;
    let mut skip_text_depth = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.last().is_some_and(|parent| parent == "body") && name == "tbl" {
                    current_table = Some(DocxTableState::default());
                } else if current_table.is_some() && name == "tr" {
                    if let Some(table) = current_table.as_mut() {
                        table.current_row = Some(Vec::new());
                    }
                } else if current_table.is_some() && name == "tc" {
                    if let Some(table) = current_table.as_mut() {
                        table.current_cell = Some(Vec::new());
                    }
                } else if stack.last().is_some_and(|parent| parent == "body") && name == "p" {
                    current_paragraph = Some(DocxParagraphState {
                        para_id: docx_para_id(&e),
                        ..DocxParagraphState::default()
                    });
                    paragraph_context = Some(DocxParagraphContext::Body);
                } else if current_table.is_some() && name == "p" && stack_contains(&stack, "tc") {
                    current_paragraph = Some(DocxParagraphState {
                        para_id: docx_para_id(&e),
                        ..DocxParagraphState::default()
                    });
                    paragraph_context = Some(DocxParagraphContext::TableCell);
                }

                docx_note_empty_or_start(&e, &name, &mut current_paragraph);
                if name == "t" {
                    in_t = true;
                }
                if name == "delText" || name == "instrText" {
                    skip_text_depth += 1;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                docx_note_empty_or_start(&e, &name, &mut current_paragraph);
            }
            Ok(event) if in_t && skip_text_depth == 0 && is_xml_text_event(&event) => {
                if let Some(paragraph) = current_paragraph.as_mut() {
                    append_xml_text_event(&mut paragraph.text, &event);
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
                        if let (Some(paragraph), Some(context)) =
                            (current_paragraph.take(), paragraph_context.take())
                        {
                            match context {
                                DocxParagraphContext::Body => {
                                    blocks.push(docx_paragraph_block(
                                        blocks.len() + 1,
                                        paragraph,
                                        &para_id_counts,
                                    ));
                                }
                                DocxParagraphContext::TableCell => {
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
                        if let Some(table) = current_table.as_mut()
                            && let Some(cell) = table.current_cell.take()
                            && let Some(row) = table.current_row.as_mut()
                        {
                            row.push(cell.join("\n"));
                        }
                    }
                    "tr" => {
                        if let Some(table) = current_table.as_mut()
                            && let Some(row) = table.current_row.take()
                        {
                            table.rows.push(row);
                        }
                    }
                    "tbl" => {
                        if let Some(table) = current_table.take() {
                            blocks.push(docx_table_block(blocks.len() + 1, table.rows));
                        }
                    }
                    _ => {}
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    blocks
}

fn docx_note_empty_or_start(
    element: &BytesStart<'_>,
    name: &str,
    current_paragraph: &mut Option<DocxParagraphState>,
) {
    let Some(paragraph) = current_paragraph.as_mut() else {
        return;
    };
    match name {
        "pStyle" => {
            if let Some(style) = attr(element, "val").filter(|style| !style.is_empty()) {
                paragraph.style = Some(style);
            }
        }
        "tab" => paragraph.text.push('\t'),
        "br" | "cr" => paragraph.text.push('\n'),
        "noBreakHyphen" => paragraph.text.push('-'),
        _ => {}
    }
}

fn docx_paragraph_block(
    index: usize,
    paragraph: DocxParagraphState,
    para_id_counts: &BTreeMap<String, usize>,
) -> Value {
    let mut block = Map::new();
    block.insert("index".to_string(), json!(index));
    block.insert("kind".to_string(), json!("paragraph"));
    if let Some(style) = paragraph.style {
        block.insert("style".to_string(), json!(style));
    }
    block.insert("text".to_string(), json!(paragraph.text));
    if let Some(para_id) = paragraph.para_id.filter(|para_id| !para_id.is_empty()) {
        let normalized = para_id.trim().to_ascii_uppercase();
        block.insert("paraId".to_string(), json!(para_id));
        if para_id_counts.get(&normalized).copied().unwrap_or_default() == 1 {
            block.insert(
                "handle".to_string(),
                json!(format!("H:docx/pt:doc/para:m:{para_id}")),
            );
        }
    }
    Value::Object(block)
}

fn docx_table_block(index: usize, rows: Vec<Vec<String>>) -> Value {
    let table_rows: Vec<Value> = rows.iter().map(|row| json!({"cells": row})).collect();
    let text = rows
        .iter()
        .map(|row| row.join("\t"))
        .collect::<Vec<_>>()
        .join("\n");
    json!({
        "index": index,
        "kind": "table",
        "table": {"rows": table_rows},
        "text": text,
    })
}

pub(crate) fn stack_contains(stack: &[String], name: &str) -> bool {
    stack.iter().any(|item| item == name)
}

fn docx_para_id_counts(xml: &str) -> BTreeMap<String, usize> {
    let mut reader = Reader::from_str(xml);
    let mut counts = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "p" => {
                if let Some(para_id) = docx_para_id(&e) {
                    *counts.entry(para_id.to_ascii_uppercase()).or_insert(0) += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    counts
}

fn docx_para_id(element: &BytesStart<'_>) -> Option<String> {
    attr(element, "paraId")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn docx_para_id_ns(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
) -> Option<String> {
    attr_prefixed_ns(element, resolver, b"w14", DOCX_W14_NS, b"paraId")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn docx_word_val_ns(element: &BytesStart<'_>, resolver: &NamespaceResolver) -> Option<String> {
    attr_prefixed_ns(element, resolver, b"w", DOCX_W_NS, b"val")
}

pub(crate) fn element_in_ns(
    resolver: &NamespaceResolver,
    element: &BytesStart<'_>,
    ns: &[u8],
) -> bool {
    matches!(
        resolver.resolve_element(element.name()),
        (ResolveResult::Bound(Namespace(uri)), _) if uri == ns
    )
}
