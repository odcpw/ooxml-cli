use super::{
    append_docx_text_children, docx_all_para_ids, docx_body_block_ranges, docx_body_tag,
    docx_open_tag_with_para_id, ensure_docx_w14_namespace, ensure_docx_word_prefix,
    first_direct_xml_child_by_kind, mint_docx_para_id, word_xml_tag, xml_direct_child_ranges,
    xml_token_name,
};
use crate::{CliError, CliResult, docx_rich_block_reports, xml_attr_escape};

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
