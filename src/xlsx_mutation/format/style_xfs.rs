use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::BTreeMap;

use super::styles_xml::{
    element_span_by_local_name, ensure_xlsx_style_defaults, set_collection_count,
};
use crate::{CliError, CliResult, local_name, render_xml_attrs, xml_attrs};

#[derive(Clone)]
struct XlsxXfEntry {
    attrs: BTreeMap<String, String>,
    inner_xml: String,
}

pub(super) fn ensure_xlsx_cell_style(
    styles_xml: String,
    base_style_index: u32,
    number_format_id: u32,
) -> CliResult<(String, u32, bool)> {
    let styles_xml = ensure_xlsx_style_defaults(styles_xml);
    let xfs = parse_xlsx_cell_xfs(&styles_xml)?;
    let base_index = if (base_style_index as usize) < xfs.len() {
        base_style_index
    } else {
        0
    };
    let base = xfs
        .get(base_index as usize)
        .cloned()
        .unwrap_or_else(default_xlsx_xf_entry);
    if xlsx_xf_num_fmt_id(&base.attrs) == number_format_id {
        return Ok((styles_xml, base_index, false));
    }
    let mut attrs = base.attrs.clone();
    for (key, value) in [
        ("fontId", "0"),
        ("fillId", "0"),
        ("borderId", "0"),
        ("xfId", "0"),
    ] {
        attrs
            .entry(key.to_string())
            .or_insert_with(|| value.to_string());
    }
    attrs.insert("numFmtId".to_string(), number_format_id.to_string());
    attrs.insert("applyNumberFormat".to_string(), "1".to_string());
    let candidate = XlsxXfEntry {
        attrs,
        inner_xml: base.inner_xml,
    };
    let candidate_sig = render_xlsx_xf(&candidate);
    for (index, xf) in xfs.iter().enumerate() {
        if render_xlsx_xf(xf) == candidate_sig {
            return Ok((styles_xml, index as u32, false));
        }
    }
    let Some(parent) = element_span_by_local_name(&styles_xml, "cellXfs") else {
        return Err(CliError::unexpected("styles cellXfs not found"));
    };
    let mut out = String::with_capacity(styles_xml.len() + candidate_sig.len());
    out.push_str(&styles_xml[..parent.close_start]);
    out.push_str(&candidate_sig);
    out.push_str(&styles_xml[parent.close_start..]);
    let out = set_collection_count(out, "cellXfs", "xf");
    Ok((out, xfs.len() as u32, true))
}

fn parse_xlsx_cell_xfs(styles_xml: &str) -> CliResult<Vec<XlsxXfEntry>> {
    let Some(parent) = element_span_by_local_name(styles_xml, "cellXfs") else {
        return Ok(Vec::new());
    };
    let fragment = &styles_xml[parent.open_end..parent.close_start];
    let base = parent.open_end;
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut entries = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "xf" => {
                let attrs = xml_attrs(&e);
                let open_end = reader.buffer_position() as usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "xf" => {
                            entries.push(XlsxXfEntry {
                                attrs,
                                inner_xml: styles_xml[base + open_end..base + inner_before]
                                    .to_string(),
                            });
                            break;
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("xf has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "xf" => {
                let _ = before;
                entries.push(XlsxXfEntry {
                    attrs: xml_attrs(&e),
                    inner_xml: String::new(),
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(entries)
}

fn default_xlsx_xf_entry() -> XlsxXfEntry {
    let mut attrs = BTreeMap::new();
    attrs.insert("numFmtId".to_string(), "0".to_string());
    attrs.insert("fontId".to_string(), "0".to_string());
    attrs.insert("fillId".to_string(), "0".to_string());
    attrs.insert("borderId".to_string(), "0".to_string());
    attrs.insert("xfId".to_string(), "0".to_string());
    XlsxXfEntry {
        attrs,
        inner_xml: String::new(),
    }
}

fn render_xlsx_xf(xf: &XlsxXfEntry) -> String {
    if xf.inner_xml.is_empty() {
        format!("<xf{}/>", render_xml_attrs(&xf.attrs))
    } else {
        format!("<xf{}>{}</xf>", render_xml_attrs(&xf.attrs), xf.inner_xml)
    }
}

fn xlsx_xf_num_fmt_id(attrs: &BTreeMap<String, String>) -> u32 {
    attrs
        .get("numFmtId")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0)
}
