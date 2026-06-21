use quick_xml::Reader;
use quick_xml::events::Event;

use crate::{
    CliError, CliResult, local_name, xlsx_workbook_ordered_insert_position, xml_attr_escape,
    xml_escape,
};

use super::model::XlsxDefinedName;
use super::package::parse_xlsx_defined_name_block;

pub(super) fn rewrite_workbook_defined_names(
    workbook_xml: &str,
    names: &[XlsxDefinedName],
) -> CliResult<String> {
    let rendered = render_defined_names_block(workbook_xml, names);
    if let Some(block) = parse_xlsx_defined_name_block(workbook_xml, &[])? {
        let mut out = String::with_capacity(workbook_xml.len() + rendered.len());
        out.push_str(&workbook_xml[..block.start]);
        out.push_str(&rendered);
        out.push_str(&workbook_xml[block.end..]);
        return Ok(out);
    }
    if rendered.is_empty() {
        return Ok(workbook_xml.to_string());
    }
    let insert_at = workbook_defined_names_insert_position(workbook_xml)
        .ok_or_else(|| CliError::unexpected("could not locate workbook insertion point"))?;
    let mut out = String::with_capacity(workbook_xml.len() + rendered.len());
    out.push_str(&workbook_xml[..insert_at]);
    out.push_str(&rendered);
    out.push_str(&workbook_xml[insert_at..]);
    Ok(out)
}

fn render_defined_names_block(workbook_xml: &str, names: &[XlsxDefinedName]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let prefix = workbook_element_prefix(workbook_xml);
    let wrapper = xml_qualified_name(prefix.as_deref(), "definedNames");
    let mut out = String::new();
    out.push_str(&format!("<{wrapper}>"));
    for name in names {
        out.push_str(&render_defined_name_element(prefix.as_deref(), name));
    }
    out.push_str(&format!("</{wrapper}>"));
    out
}

fn render_defined_name_element(prefix: Option<&str>, name: &XlsxDefinedName) -> String {
    let tag = xml_qualified_name(prefix, "definedName");
    let mut attrs = format!(r#" name="{}""#, xml_attr_escape(&name.name));
    if let Some(local_sheet_id) = name.local_sheet_id {
        attrs.push_str(&format!(r#" localSheetId="{local_sheet_id}""#));
    }
    if name.hidden {
        attrs.push_str(r#" hidden="1""#);
    }
    if !name.comment.trim().is_empty() {
        attrs.push_str(&format!(
            r#" comment="{}""#,
            xml_attr_escape(name.comment.trim())
        ));
    }
    if !name.description.trim().is_empty() {
        attrs.push_str(&format!(
            r#" description="{}""#,
            xml_attr_escape(name.description.trim())
        ));
    }
    format!("<{tag}{attrs}>{}</{tag}>", xml_escape(&name.ref_text))
}

fn workbook_element_prefix(workbook_xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "workbook" =>
            {
                let raw = String::from_utf8_lossy(e.name().as_ref()).to_string();
                return raw.split_once(':').map(|(prefix, _)| prefix.to_string());
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn xml_qualified_name(prefix: Option<&str>, local: &str) -> String {
    match prefix.filter(|value| !value.is_empty()) {
        Some(prefix) => format!("{prefix}:{local}"),
        None => local.to_string(),
    }
}

fn workbook_defined_names_insert_position(workbook_xml: &str) -> Option<usize> {
    xlsx_workbook_ordered_insert_position(workbook_xml, "definedNames")
}
