use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;
use serde_json::{Value, json};

use super::selectors::DocxHeaderFooterRefInfo;
use crate::{
    CliError, CliResult, DOCX_W_NS, decode_xml_text, docx_word_attr_ns, element_in_ns, local_name,
    xml_general_ref, zip_text,
};

pub(super) fn docx_header_footer_paragraphs(
    file: &str,
    reference: &DocxHeaderFooterRefInfo,
) -> CliResult<Vec<Value>> {
    let xml = zip_text(file, reference.part_uri.trim_start_matches('/')).map_err(|err| {
        CliError::unexpected(format!(
            "failed to read header/footer part {}: {}",
            reference.part_uri, err.message
        ))
    })?;
    let mut reader = NsReader::from_str(&xml);
    let mut stack = Vec::<String>::new();
    let mut paragraphs = Vec::new();
    let mut current = None::<DocxHeaderFooterParagraphBuild>;
    let mut in_t = false;
    let mut skip_text_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    current = Some(DocxHeaderFooterParagraphBuild::default());
                }
                docx_note_header_footer_paragraph_start(
                    &mut current,
                    &e,
                    reader.resolver(),
                    &stack,
                    is_word,
                    skip_text_depth,
                );
                if is_word && name == "t" {
                    in_t = true;
                }
                if is_word && matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth += 1;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    let paragraph = DocxHeaderFooterParagraphBuild::default();
                    paragraphs.push(docx_header_footer_paragraph_json(
                        paragraphs.len() + 1,
                        paragraph,
                        reference,
                    ));
                } else {
                    docx_note_header_footer_paragraph_start(
                        &mut current,
                        &e,
                        reader.resolver(),
                        &stack,
                        is_word,
                        skip_text_depth,
                    );
                }
            }
            Ok(Event::Text(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph.text.push_str(&xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph
                        .text
                        .push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_t = false;
                } else if matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth = skip_text_depth.saturating_sub(1);
                } else if name == "p"
                    && let Some(paragraph) = current.take()
                {
                    paragraphs.push(docx_header_footer_paragraph_json(
                        paragraphs.len() + 1,
                        paragraph,
                        reference,
                    ));
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(paragraphs)
}

#[derive(Default)]
struct DocxHeaderFooterParagraphBuild {
    style: String,
    text: String,
}

fn docx_note_header_footer_paragraph_start(
    current: &mut Option<DocxHeaderFooterParagraphBuild>,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    stack: &[String],
    is_word: bool,
    skip_text_depth: usize,
) {
    let Some(paragraph) = current.as_mut() else {
        return;
    };
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if is_word
        && name == "pStyle"
        && stack.last().is_some_and(|parent| parent == "pPr")
        && let Some(style) = docx_word_attr_ns(element, resolver, b"val")
    {
        paragraph.style = style;
        return;
    }
    if is_word && skip_text_depth == 0 {
        match name {
            "tab" => paragraph.text.push('\t'),
            "br" | "cr" => paragraph.text.push('\n'),
            "noBreakHyphen" => paragraph.text.push('-'),
            _ => {}
        }
    }
}

fn docx_header_footer_paragraph_json(
    index: usize,
    paragraph: DocxHeaderFooterParagraphBuild,
    reference: &DocxHeaderFooterRefInfo,
) -> Value {
    let primary_selector = if reference.primary_selector.is_empty() {
        String::new()
    } else {
        format!("{}/p:{index}", reference.primary_selector)
    };
    let mut selectors = Vec::new();
    for selector in &reference.selectors {
        selectors.push(format!("{selector}/p:{index}"));
        selectors.push(format!("{selector}/paragraph:{index}"));
    }
    json!({
        "index": index,
        "primarySelector": primary_selector,
        "selectors": selectors,
        "style": paragraph.style,
        "text": paragraph.text,
    })
}
