use quick_xml::Reader;
use quick_xml::events::Event;

use super::number_format::XlsxNumberFormatSpec;
use super::styles_xml::{
    element_span_by_local_name, ensure_xlsx_style_defaults, insert_xlsx_styles_collection,
    set_collection_count,
};
use crate::{CliResult, attr, local_name, xml_attr_escape};

pub(super) fn ensure_xlsx_number_format(
    styles_xml: String,
    spec: &XlsxNumberFormatSpec,
) -> CliResult<(String, u32)> {
    let styles_xml = ensure_xlsx_style_defaults(styles_xml);
    if spec.builtin {
        return Ok((styles_xml, spec.number_format_id));
    }
    for (id, code) in parse_xlsx_num_formats(&styles_xml) {
        if code == spec.format_code {
            return Ok((styles_xml, id));
        }
    }
    let mut next_id = 164u32;
    for (id, _) in parse_xlsx_num_formats(&styles_xml) {
        if id >= next_id {
            next_id = id + 1;
        }
    }
    let num_fmt = format!(
        r#"<numFmt numFmtId="{next_id}" formatCode="{}"/>"#,
        xml_attr_escape(&spec.format_code)
    );
    let updated = if let Some(span) = element_span_by_local_name(&styles_xml, "numFmts") {
        let mut out = String::with_capacity(styles_xml.len() + num_fmt.len());
        out.push_str(&styles_xml[..span.close_start]);
        out.push_str(&num_fmt);
        out.push_str(&styles_xml[span.close_start..]);
        set_collection_count(out, "numFmts", "numFmt")
    } else {
        insert_xlsx_styles_collection(
            &styles_xml,
            "numFmts",
            &format!(r#"<numFmts count="1">{num_fmt}</numFmts>"#),
        )
    };
    Ok((updated, next_id))
}

fn parse_xlsx_num_formats(styles_xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(styles_xml);
    reader.config_mut().trim_text(false);
    let mut formats = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "numFmt" =>
            {
                if let (Some(id), Some(code)) = (attr(&e, "numFmtId"), attr(&e, "formatCode"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    formats.push((id, code));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    formats
}
