use quick_xml::Reader;
use quick_xml::events::Event;

use super::DocxStyleTarget;
use crate::{
    CliError, CliResult, attr, docx_all_para_ids, docx_body_block_ranges, docx_body_tag,
    docx_open_tag_with_para_id, ensure_docx_body_table_scaffolds_xml,
    ensure_docx_table_scaffold_fragment, ensure_docx_w14_namespace, ensure_docx_word_prefix,
    local_name, mint_docx_para_id, word_xml_tag, xml_attr_escape, xml_direct_child_ranges,
    xml_fragment_bounds, xml_open_tag_from_start, xml_tag_prefix,
};

pub(super) fn apply_docx_style_xml(
    xml: &str,
    target: DocxStyleTarget,
    block_index: usize,
    style_id: &str,
    existing_para_id: &str,
) -> CliResult<String> {
    if block_index == 0 {
        return Err(CliError::target_not_found(format!(
            "target not found: {} block 0",
            target.as_str()
        )));
    }
    let mut working = xml.to_string();
    if matches!(target, DocxStyleTarget::Paragraph | DocxStyleTarget::Run)
        && existing_para_id.trim().is_empty()
    {
        working = ensure_docx_w14_namespace(&working)?;
    }
    let body_tag = docx_body_tag(&working)?;
    if !body_tag.contains(':') {
        working = ensure_docx_word_prefix(&working)?;
    }
    let body_tag = docx_body_tag(&working)?;
    let blocks = docx_body_block_ranges(&working, &body_tag)?;
    let block = blocks.get(block_index - 1).ok_or_else(|| {
        CliError::target_not_found(format!(
            "target not found: {} block {block_index}",
            target.as_str()
        ))
    })?;
    let fragment = &working[block.start..block.end];
    let replacement = match target {
        DocxStyleTarget::Paragraph => {
            if block.kind != "p" {
                return Err(CliError::invalid_args(format!(
                    "block {block_index} is a table, not a paragraph"
                )));
            }
            let para_id = docx_style_apply_para_id(&working, existing_para_id)?;
            set_docx_paragraph_style_fragment(fragment, &para_id, style_id)?
        }
        DocxStyleTarget::Run => {
            if block.kind != "p" {
                return Err(CliError::invalid_args(format!(
                    "block {block_index} is a table, not a paragraph"
                )));
            }
            let para_id = docx_style_apply_para_id(&working, existing_para_id)?;
            set_docx_run_style_for_paragraph_fragment(fragment, &para_id, style_id)?
        }
        DocxStyleTarget::Table => {
            if block.kind != "tbl" {
                return Err(CliError::invalid_args(format!(
                    "block {block_index} is a paragraph, not a table"
                )));
            }
            set_docx_table_style_fragment(fragment, style_id)?
        }
    };
    let mut out = String::with_capacity(working.len() + replacement.len());
    out.push_str(&working[..block.start]);
    out.push_str(&replacement);
    out.push_str(&working[block.end..]);
    if matches!(target, DocxStyleTarget::Paragraph | DocxStyleTarget::Run) {
        ensure_docx_body_table_scaffolds_xml(&out)
    } else {
        Ok(out)
    }
}

fn docx_style_apply_para_id(xml: &str, existing_para_id: &str) -> CliResult<String> {
    if !existing_para_id.trim().is_empty() {
        return Ok(existing_para_id.trim().to_string());
    }
    let existing = docx_all_para_ids(xml)?;
    Ok(mint_docx_para_id(&existing))
}

fn set_docx_paragraph_style_fragment(
    fragment: &str,
    para_id: &str,
    style_id: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let start_tag = &fragment[..=open_end];
    let prefix = xml_tag_prefix(&tag_name);
    let open_tag = docx_open_tag_with_para_id(start_tag, para_id);
    let props = render_docx_style_props(&prefix, "pPr", "pStyle", style_id);
    if self_closing {
        return Ok(format!("{open_tag}{props}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    if let Some(child) = children.into_iter().find(|child| child.kind == "pPr") {
        let updated_props =
            set_docx_style_child_in_props(&fragment[child.start..child.end], "pStyle", style_id)?;
        let mut out = String::new();
        out.push_str(&open_tag);
        out.push_str(&fragment[open_end + 1..child.start]);
        out.push_str(&updated_props);
        out.push_str(&fragment[child.end..close_start]);
        out.push_str("</");
        out.push_str(&tag_name);
        out.push('>');
        return Ok(out);
    }
    let mut out = String::new();
    out.push_str(&open_tag);
    out.push_str(&props);
    out.push_str(&fragment[open_end + 1..close_start]);
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok(out)
}

fn set_docx_run_style_for_paragraph_fragment(
    fragment: &str,
    para_id: &str,
    style_id: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let start_tag = &fragment[..=open_end];
    let open_tag = docx_open_tag_with_para_id(start_tag, para_id);
    if self_closing {
        return Ok(format!("{open_tag}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    let mut out = String::new();
    out.push_str(&open_tag);
    let mut cursor = open_end + 1;
    for child in children {
        if child.kind != "r" {
            continue;
        }
        out.push_str(&fragment[cursor..child.start]);
        out.push_str(&set_docx_run_style_fragment(
            &fragment[child.start..child.end],
            style_id,
        )?);
        cursor = child.end;
    }
    out.push_str(&fragment[cursor..close_start]);
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok(out)
}

fn set_docx_run_style_fragment(fragment: &str, style_id: &str) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let prefix = xml_tag_prefix(&tag_name);
    let props = render_docx_style_props(&prefix, "rPr", "rStyle", style_id);
    if self_closing {
        let open = xml_open_tag_from_start(&fragment[..=open_end]);
        return Ok(format!("{open}{props}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    if let Some(child) = children.into_iter().find(|child| child.kind == "rPr") {
        let updated_props =
            set_docx_style_child_in_props(&fragment[child.start..child.end], "rStyle", style_id)?;
        let mut out = String::new();
        out.push_str(&fragment[..child.start]);
        out.push_str(&updated_props);
        out.push_str(&fragment[child.end..]);
        return Ok(out);
    }
    let mut out = String::new();
    out.push_str(&fragment[..open_end + 1]);
    out.push_str(&props);
    out.push_str(&fragment[open_end + 1..]);
    Ok(out)
}

fn set_docx_table_style_fragment(fragment: &str, style_id: &str) -> CliResult<String> {
    let scaffolded = ensure_docx_table_scaffold_fragment(fragment)?;
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(&scaffolded)?;
    if self_closing {
        return Ok(scaffolded);
    }
    let children = xml_direct_child_ranges(&scaffolded, open_end + 1, close_start)?;
    let Some(child) = children.into_iter().find(|child| child.kind == "tblPr") else {
        return Ok(scaffolded);
    };
    let updated_props =
        set_docx_style_child_in_props(&scaffolded[child.start..child.end], "tblStyle", style_id)?;
    let mut out = String::new();
    out.push_str(&scaffolded[..child.start]);
    out.push_str(&updated_props);
    out.push_str(&scaffolded[child.end..]);
    Ok(out)
}

fn set_docx_style_child_in_props(
    props_fragment: &str,
    style_local: &str,
    style_id: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(props_fragment)?;
    let prefix = xml_tag_prefix(&tag_name);
    let style_child = render_docx_style_child(&prefix, style_local, style_id);
    if self_closing {
        let open = xml_open_tag_from_start(&props_fragment[..=open_end]);
        return Ok(format!("{open}{style_child}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(props_fragment, open_end + 1, close_start)?;
    if let Some(child) = children.into_iter().find(|child| child.kind == style_local) {
        let mut out = String::new();
        out.push_str(&props_fragment[..child.start]);
        out.push_str(&style_child);
        out.push_str(&props_fragment[child.end..]);
        return Ok(out);
    }
    let mut out = String::new();
    out.push_str(&props_fragment[..open_end + 1]);
    out.push_str(&style_child);
    out.push_str(&props_fragment[open_end + 1..]);
    Ok(out)
}

fn render_docx_style_props(
    prefix: &str,
    props_local: &str,
    style_local: &str,
    style_id: &str,
) -> String {
    let props = word_xml_tag(prefix, props_local);
    let mut out = String::new();
    out.push('<');
    out.push_str(&props);
    out.push('>');
    out.push_str(&render_docx_style_child(prefix, style_local, style_id));
    out.push_str("</");
    out.push_str(&props);
    out.push('>');
    out
}

fn render_docx_style_child(prefix: &str, style_local: &str, style_id: &str) -> String {
    let style_tag = word_xml_tag(prefix, style_local);
    let val_attr = if prefix.is_empty() {
        "w:val".to_string()
    } else {
        format!("{prefix}:val")
    };
    format!(
        "<{} {}=\"{}\"/>",
        style_tag,
        val_attr,
        xml_attr_escape(style_id)
    )
}

pub(super) fn docx_first_run_style(fragment: &str) -> CliResult<String> {
    docx_style_in_fragment(fragment, "rPr", "rStyle")
}

pub(super) fn docx_table_style(fragment: &str) -> CliResult<String> {
    docx_style_in_fragment(fragment, "tblPr", "tblStyle")
}

fn docx_style_in_fragment(
    fragment: &str,
    property_parent: &str,
    style_local: &str,
) -> CliResult<String> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<String> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                if parent == Some(property_parent)
                    && name == style_local
                    && let Some(style) = attr(&e, "val")
                {
                    return Ok(style);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                if parent == Some(property_parent)
                    && name == style_local
                    && let Some(style) = attr(&e, "val")
                {
                    return Ok(style);
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(String::new())
}
