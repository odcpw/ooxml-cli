use quick_xml::NsReader;
use quick_xml::events::Event;

use crate::{
    CliError, CliResult, DOCX_W_NS, XmlNamedRange, append_docx_text_children,
    docx_paragraph_fragment_text, element_in_ns, first_direct_xml_child_by_kind, local_name,
    word_xml_tag, xml_direct_child_ranges, xml_fragment_bounds, xml_open_tag_from_start,
    xml_tag_prefix,
};

pub(super) struct DocxHeaderFooterTextMutation {
    pub(super) xml: String,
    pub(super) index: i64,
    pub(super) previous_text: String,
}

pub(super) fn set_docx_header_footer_text_xml(
    xml: &str,
    part_uri: &str,
    index: i64,
    text: &str,
) -> CliResult<DocxHeaderFooterTextMutation> {
    let root_tag = docx_header_footer_root_tag(xml, part_uri)?;
    let root_start = xml.find(&format!("<{root_tag}")).ok_or_else(|| {
        CliError::unexpected(format!("part {part_uri} is not a header or footer"))
    })?;
    let root_open_end = xml[root_start..]
        .find('>')
        .map(|offset| root_start + offset)
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let root_self_closing = xml[root_start..=root_open_end].trim_end().ends_with("/>");
    let root_close_start = if root_self_closing {
        root_open_end + 1
    } else {
        xml.rfind(&format!("</{root_tag}>"))
            .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?
    };
    let paragraphs: Vec<XmlNamedRange> = if root_self_closing {
        Vec::new()
    } else {
        xml_direct_child_ranges(xml, root_open_end + 1, root_close_start)?
            .into_iter()
            .filter(|child| child.kind == "p")
            .collect()
    };
    let paragraph = paragraphs.get(index as usize - 1).ok_or_else(|| {
        CliError::target_not_found(format!("target not found: header/footer paragraph {index}"))
    })?;
    let fragment = &xml[paragraph.start..paragraph.end];
    let previous_text = docx_paragraph_fragment_text(fragment);
    let updated_paragraph = replace_docx_header_footer_paragraph_fragment(fragment, text)?;
    let mut out = String::with_capacity(xml.len() + updated_paragraph.len());
    out.push_str(&xml[..paragraph.start]);
    out.push_str(&updated_paragraph);
    out.push_str(&xml[paragraph.end..]);
    Ok(DocxHeaderFooterTextMutation {
        xml: out,
        index,
        previous_text,
    })
}

fn replace_docx_header_footer_paragraph_fragment(fragment: &str, text: &str) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let start_tag = &fragment[..=open_end];
    let prefix = xml_tag_prefix(&tag_name);
    let mut paragraph_properties = String::new();
    let mut run_properties = String::new();
    if !self_closing {
        for child in xml_direct_child_ranges(fragment, open_end + 1, close_start)? {
            match child.kind.as_str() {
                "pPr" if paragraph_properties.is_empty() => {
                    paragraph_properties.push_str(&fragment[child.start..child.end]);
                }
                "r" if run_properties.is_empty() => {
                    if let Some(r_pr) =
                        first_direct_xml_child_by_kind(&fragment[child.start..child.end], "rPr")?
                    {
                        run_properties.push_str(&r_pr);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = xml_open_tag_from_start(start_tag);
    out.push_str(&paragraph_properties);
    let r = word_xml_tag(&prefix, "r");
    out.push('<');
    out.push_str(&r);
    out.push('>');
    out.push_str(&run_properties);
    append_docx_text_children(&mut out, &prefix, text);
    out.push_str("</");
    out.push_str(&r);
    out.push('>');
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok(out)
}

pub(crate) fn docx_header_footer_root_tag(xml: &str, part_uri: &str) -> CliResult<String> {
    let mut reader = NsReader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if element_in_ns(reader.resolver(), &e, DOCX_W_NS)
                    && matches!(name.as_str(), "hdr" | "ftr")
                {
                    return Ok(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
                return Err(CliError::unexpected(format!(
                    "part {part_uri} is not a header or footer"
                )));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::unexpected(format!(
        "part {part_uri} is not a header or footer"
    )))
}
