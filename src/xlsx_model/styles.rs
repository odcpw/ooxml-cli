use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::BTreeMap;

use crate::{CliError, CliResult, attr, local_name, zip_text};

#[derive(Clone, Default)]
pub(crate) struct XlsxStyle {
    pub(crate) number_format_id: Option<u32>,
    pub(crate) number_format_code: Option<String>,
    pub(crate) date_style: bool,
}

pub(crate) fn xlsx_styles(file: &str) -> CliResult<Vec<XlsxStyle>> {
    let xml = match zip_text(file, "xl/styles.xml") {
        Ok(xml) => xml,
        Err(_) => return Ok(Vec::new()),
    };
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut custom_formats = BTreeMap::<u32, String>::new();
    let mut styles = Vec::new();
    let mut in_cell_xfs = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "numFmt" =>
            {
                if let (Some(id), Some(code)) = (attr(&e, "numFmtId"), attr(&e, "formatCode"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    custom_formats.insert(id, code);
                }
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "cellXfs" => {
                in_cell_xfs = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "cellXfs" => {
                in_cell_xfs = false;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_cell_xfs && local_name(e.name().as_ref()) == "xf" =>
            {
                let number_format_id = attr(&e, "numFmtId").and_then(|value| value.parse().ok());
                let number_format_code = number_format_id.and_then(|id| {
                    custom_formats
                        .get(&id)
                        .cloned()
                        .or_else(|| builtin_num_format_code(id).map(ToString::to_string))
                });
                let date_style = number_format_id.is_some_and(is_builtin_date_num_fmt)
                    || number_format_code
                        .as_deref()
                        .is_some_and(is_date_format_code);
                styles.push(XlsxStyle {
                    number_format_id,
                    number_format_code,
                    date_style,
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(styles)
}

pub(crate) fn builtin_num_format_code(id: u32) -> Option<&'static str> {
    match id {
        0 => Some("General"),
        1 => Some("0"),
        2 => Some("0.00"),
        3 => Some("#,##0"),
        4 => Some("#,##0.00"),
        9 => Some("0%"),
        10 => Some("0.00%"),
        14 => Some("m/d/yy"),
        15 => Some("d-mmm-yy"),
        16 => Some("d-mmm"),
        17 => Some("mmm-yy"),
        18 => Some("h:mm AM/PM"),
        19 => Some("h:mm:ss AM/PM"),
        20 => Some("h:mm"),
        21 => Some("h:mm:ss"),
        22 => Some("m/d/yy h:mm"),
        45 => Some("mm:ss"),
        46 => Some("[h]:mm:ss"),
        47 => Some("mmss.0"),
        49 => Some("@"),
        _ => None,
    }
}

fn is_builtin_date_num_fmt(id: u32) -> bool {
    matches!(id, 14..=22 | 45..=47)
}

fn is_date_format_code(code: &str) -> bool {
    let mut cleaned = String::new();
    let mut in_quote = false;
    for ch in code.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            _ if !in_quote => cleaned.push(ch.to_ascii_lowercase()),
            _ => {}
        }
    }
    cleaned.contains('y')
        || cleaned.contains('d')
        || cleaned.contains("h:")
        || cleaned.contains("m/")
}
