use quick_xml::events::Event;
use quick_xml::{NsReader, Reader};
use std::collections::BTreeMap;

use super::{XLSX_MAIN_NS, XlsxWorkbookCalcSettings, metadata_ordered_insert_position};
use crate::{attr, element_in_ns, local_name, render_xml_attrs, replace_xml_span, xml_attrs_map};

pub(super) fn xlsx_workbook_calc_settings_from_xml(xml: &str) -> XlsxWorkbookCalcSettings {
    let mut settings = XlsxWorkbookCalcSettings::default();
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "calcPr"
                    && element_in_ns(reader.resolver(), &e, XLSX_MAIN_NS) =>
            {
                if let Some(value) = attr(&e, "calcMode").filter(|value| !value.is_empty()) {
                    settings.calc_mode = value;
                }
                settings.full_calc_on_load = attr(&e, "fullCalcOnLoad").as_deref() == Some("1");
                settings.force_full_calc = attr(&e, "forceFullCalc").as_deref() == Some("1");
                settings.calc_id = attr(&e, "calcId").unwrap_or_default();
                settings.iterate = attr(&e, "iterate").as_deref() == Some("1");
                if let Some(value) = attr(&e, "iterateCount")
                    && let Ok(parsed) = value.parse::<i64>()
                {
                    settings.iterate_count = parsed;
                }
                if let Some(value) = attr(&e, "iterateDelta")
                    && let Ok(parsed) = value.parse::<f64>()
                {
                    settings.iterate_delta = parsed;
                }
                break;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    settings
}

pub(super) fn update_xlsx_workbook_calc_xml(
    xml: String,
    calc_mode: Option<&str>,
    full_calc_on_load: Option<bool>,
) -> String {
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "calcPr" => {
                let end = reader.buffer_position() as usize;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = xml_attrs_map(&e);
                if let Some(calc_mode) = calc_mode {
                    attrs.insert("calcMode".to_string(), calc_mode.to_string());
                }
                if let Some(full_calc_on_load) = full_calc_on_load {
                    if full_calc_on_load {
                        attrs.insert("fullCalcOnLoad".to_string(), "1".to_string());
                        attrs.insert("forceFullCalc".to_string(), "1".to_string());
                    } else {
                        attrs.remove("fullCalcOnLoad");
                        attrs.remove("forceFullCalc");
                    }
                }
                return replace_xml_span(
                    &xml,
                    start,
                    end,
                    &format!("<{name}{}/>", render_xml_attrs(&attrs)),
                );
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "calcPr" => {
                let end = reader.buffer_position() as usize;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = xml_attrs_map(&e);
                if let Some(calc_mode) = calc_mode {
                    attrs.insert("calcMode".to_string(), calc_mode.to_string());
                }
                if let Some(full_calc_on_load) = full_calc_on_load {
                    if full_calc_on_load {
                        attrs.insert("fullCalcOnLoad".to_string(), "1".to_string());
                        attrs.insert("forceFullCalc".to_string(), "1".to_string());
                    } else {
                        attrs.remove("fullCalcOnLoad");
                        attrs.remove("forceFullCalc");
                    }
                }
                return replace_xml_span(
                    &xml,
                    start,
                    end,
                    &format!("<{name}{}>", render_xml_attrs(&attrs)),
                );
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    let mut attrs = BTreeMap::new();
    if let Some(calc_mode) = calc_mode {
        attrs.insert("calcMode".to_string(), calc_mode.to_string());
    }
    if let Some(full_calc_on_load) = full_calc_on_load
        && full_calc_on_load
    {
        attrs.insert("fullCalcOnLoad".to_string(), "1".to_string());
        attrs.insert("forceFullCalc".to_string(), "1".to_string());
    }
    let calc_pr = format!("<calcPr{}/>", render_xml_attrs(&attrs));
    if let Some(pos) =
        metadata_ordered_insert_position(&xml, workbook_child_order("calcPr"), workbook_child_order)
    {
        let mut out = String::with_capacity(xml.len() + calc_pr.len());
        out.push_str(&xml[..pos]);
        out.push_str(&calc_pr);
        out.push_str(&xml[pos..]);
        out
    } else {
        xml
    }
}

fn workbook_child_order(local_name: &str) -> i32 {
    match local_name {
        "fileVersion" => 10,
        "fileSharing" => 20,
        "workbookPr" => 30,
        "workbookProtection" => 40,
        "bookViews" => 50,
        "sheets" => 60,
        "functionGroups" => 70,
        "externalReferences" => 80,
        "definedNames" => 90,
        "calcPr" => 100,
        "oleSize" => 110,
        "customWorkbookViews" => 120,
        "pivotCaches" => 130,
        "smartTagPr" => 140,
        "smartTagTypes" => 150,
        "webPublishing" => 160,
        "fileRecoveryPr" => 170,
        "webPublishObjects" => 180,
        "extLst" => 190,
        _ => 10000,
    }
}
