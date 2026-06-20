use quick_xml::Reader;
use quick_xml::events::Event;

use super::{
    docx_body_block_ranges, docx_body_tag, word_xml_tag, xml_direct_child_ranges,
    xml_fragment_bounds, xml_tag_prefix,
};
use crate::{CliError, CliResult, local_name};

pub(crate) fn ensure_docx_body_table_scaffolds_xml(xml: &str) -> CliResult<String> {
    let body_tag = docx_body_tag(xml)?;
    let blocks = docx_body_block_ranges(xml, &body_tag)?;
    let mut out = String::with_capacity(xml.len());
    let mut cursor = 0usize;
    for block in blocks {
        if block.kind != "tbl" {
            continue;
        }
        out.push_str(&xml[cursor..block.start]);
        out.push_str(&ensure_docx_table_scaffold_fragment(
            &xml[block.start..block.end],
        )?);
        cursor = block.end;
    }
    out.push_str(&xml[cursor..]);
    Ok(out)
}

pub(crate) fn ensure_docx_table_scaffold_fragment(fragment: &str) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(fragment.to_string());
    }
    let prefix = xml_tag_prefix(&tag_name);
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    let has_tbl_pr = children.iter().any(|child| child.kind == "tblPr");
    let has_tbl_grid = children.iter().any(|child| child.kind == "tblGrid");
    if has_tbl_pr && has_tbl_grid {
        return Ok(fragment.to_string());
    }
    let first_row_start = children
        .iter()
        .find(|child| child.kind == "tr")
        .map(|child| child.start)
        .unwrap_or(open_end + 1);
    let mut scaffold = String::new();
    if !has_tbl_pr {
        scaffold.push_str(&format!("<{}/>", word_xml_tag(&prefix, "tblPr")));
    }
    if !has_tbl_grid {
        scaffold.push_str(&render_docx_tbl_grid(
            &prefix,
            docx_table_max_cols(fragment)?,
        ));
    }
    let mut out = String::new();
    out.push_str(&fragment[..first_row_start]);
    out.push_str(&scaffold);
    out.push_str(&fragment[first_row_start..]);
    Ok(out)
}

fn render_docx_tbl_grid(prefix: &str, cols: usize) -> String {
    let tbl_grid = word_xml_tag(prefix, "tblGrid");
    let grid_col = word_xml_tag(prefix, "gridCol");
    let width_attr = if prefix.is_empty() {
        "w:w".to_string()
    } else {
        format!("{prefix}:w")
    };
    let mut out = format!("<{tbl_grid}>");
    for _ in 0..cols {
        out.push_str(&format!("<{grid_col} {width_attr}=\"0\"/>"));
    }
    out.push_str("</");
    out.push_str(&tbl_grid);
    out.push('>');
    out
}

fn docx_table_max_cols(fragment: &str) -> CliResult<usize> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<String> = Vec::new();
    let mut table_depth = 0usize;
    let mut current_cols = 0usize;
    let mut max_cols = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                if name == "tbl" {
                    table_depth += 1;
                } else if table_depth == 1 && parent == Some("tr") && name == "tc" {
                    current_cols += 1;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                if table_depth == 1 && parent == Some("tr") && name == "tc" {
                    current_cols += 1;
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if table_depth == 1 && name == "tr" {
                    max_cols = max_cols.max(current_cols);
                    current_cols = 0;
                }
                if name == "tbl" && table_depth > 0 {
                    table_depth -= 1;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(max_cols)
}
