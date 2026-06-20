use quick_xml::Reader;
use quick_xml::events::Event;

use super::styles_part::default_xlsx_styles_xml;
use crate::local_name;

#[derive(Clone, Copy)]
pub(super) struct XmlElementSpan {
    pub(super) start: usize,
    pub(super) open_end: usize,
    pub(super) close_start: usize,
}

pub(super) fn ensure_xlsx_style_defaults(mut styles_xml: String) -> String {
    if !styles_xml.contains("<styleSheet") {
        return default_xlsx_styles_xml();
    }
    let defaults = [
        ("fonts", r#"<fonts count="1"><font/></fonts>"#),
        (
            "fills",
            r#"<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>"#,
        ),
        ("borders", r#"<borders count="1"><border/></borders>"#),
        (
            "cellStyleXfs",
            r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
        ),
        (
            "cellXfs",
            r#"<cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs>"#,
        ),
        (
            "cellStyles",
            r#"<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>"#,
        ),
    ];
    for (name, block) in defaults {
        if element_span_by_local_name(&styles_xml, name).is_none() {
            styles_xml = insert_xlsx_styles_collection(&styles_xml, name, block);
        }
    }
    styles_xml
}

pub(super) fn insert_xlsx_styles_collection(styles_xml: &str, name: &str, block: &str) -> String {
    let target_order = xlsx_styles_collection_order(name);
    for candidate in [
        "numFmts",
        "fonts",
        "fills",
        "borders",
        "cellStyleXfs",
        "cellXfs",
        "cellStyles",
        "dxfs",
        "tableStyles",
        "colors",
        "extLst",
    ] {
        if xlsx_styles_collection_order(candidate) > target_order
            && let Some(span) = element_span_by_local_name(styles_xml, candidate)
        {
            let mut out = String::with_capacity(styles_xml.len() + block.len());
            out.push_str(&styles_xml[..span.start]);
            out.push_str(block);
            out.push_str(&styles_xml[span.start..]);
            return out;
        }
    }
    if let Some(pos) = styles_xml.rfind("</styleSheet>") {
        let mut out = String::with_capacity(styles_xml.len() + block.len());
        out.push_str(&styles_xml[..pos]);
        out.push_str(block);
        out.push_str(&styles_xml[pos..]);
        out
    } else {
        styles_xml.to_string()
    }
}

pub(super) fn element_span_by_local_name(xml: &str, wanted: &str) -> Option<XmlElementSpan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == wanted => {
                let open_end = reader.buffer_position() as usize;
                let mut depth = 1usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::Start(e)) if local_name(e.name().as_ref()) == wanted => {
                            depth += 1;
                        }
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == wanted => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(XmlElementSpan {
                                    start: before,
                                    open_end,
                                    close_start: inner_before,
                                });
                            }
                        }
                        Ok(Event::Eof) | Err(_) => return None,
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == wanted => {
                let end = reader.buffer_position() as usize;
                return Some(XmlElementSpan {
                    start: before,
                    open_end: end,
                    close_start: before,
                });
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

pub(super) fn set_collection_count(xml: String, parent: &str, child: &str) -> String {
    let count = count_children_in_parent(&xml, parent, child);
    let Some(span) = element_span_by_local_name(&xml, parent) else {
        return xml;
    };
    set_start_tag_count_attr(&xml, span, count)
}

fn xlsx_styles_collection_order(name: &str) -> u32 {
    match name {
        "numFmts" => 10,
        "fonts" => 20,
        "fills" => 30,
        "borders" => 40,
        "cellStyleXfs" => 50,
        "cellXfs" => 60,
        "cellStyles" => 70,
        "dxfs" => 80,
        "tableStyles" => 90,
        "colors" => 100,
        "extLst" => 110,
        _ => 1000,
    }
}

fn count_children_in_parent(xml: &str, parent: &str, child: &str) -> usize {
    let Some(span) = element_span_by_local_name(xml, parent) else {
        return 0;
    };
    let fragment = &xml[span.open_end..span.close_start];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut count = 0usize;
    let mut depth = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if depth == 0 && local_name(e.name().as_ref()) == child {
                    count += 1;
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if depth == 0 && local_name(e.name().as_ref()) == child {
                    count += 1;
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    count
}

fn set_start_tag_count_attr(xml: &str, span: XmlElementSpan, count: usize) -> String {
    let open = &xml[span.start..span.open_end];
    let replacement = if let Some(pos) = open.find("count=\"") {
        let value_start = pos + "count=\"".len();
        if let Some(value_end_rel) = open[value_start..].find('"') {
            let value_end = value_start + value_end_rel;
            let mut tag = String::new();
            tag.push_str(&open[..value_start]);
            tag.push_str(&count.to_string());
            tag.push_str(&open[value_end..]);
            tag
        } else {
            open.to_string()
        }
    } else if let Some(pos) = open.rfind("/>") {
        format!("{} count=\"{}\"/>", &open[..pos].trim_end(), count)
    } else if let Some(pos) = open.rfind('>') {
        format!("{} count=\"{}\">", &open[..pos].trim_end(), count)
    } else {
        open.to_string()
    };
    let mut out = String::with_capacity(xml.len() + replacement.len());
    out.push_str(&xml[..span.start]);
    out.push_str(&replacement);
    out.push_str(&xml[span.open_end..]);
    out
}
