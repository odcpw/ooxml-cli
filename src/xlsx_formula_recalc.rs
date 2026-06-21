use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CliResult, insert_xlsx_workbook_child_ordered, local_name, relationships_part_for,
    render_xml_attrs, replace_xml_span, resolve_relationship_target, xml_attrs_map, zip_text,
};
pub(crate) fn add_xlsx_formula_recalc_package_updates(
    file: &str,
    formula_seen: bool,
    formula_invalidated: bool,
    overrides: &mut BTreeMap<String, String>,
    removals: &mut BTreeSet<String>,
) -> CliResult<()> {
    if !formula_seen && !formula_invalidated {
        return Ok(());
    }

    let workbook_part = "xl/workbook.xml";
    overrides.insert(
        workbook_part.to_string(),
        ensure_xlsx_full_calc_on_load(zip_text(file, workbook_part)?),
    );

    let content_types_xml = zip_text(file, "[Content_Types].xml")?;
    for part in xlsx_calc_chain_parts_from_content_types(&content_types_xml) {
        removals.insert(part.trim_start_matches('/').to_string());
    }
    overrides.insert(
        "[Content_Types].xml".to_string(),
        remove_xlsx_calc_chain_content_type_overrides(&content_types_xml),
    );

    let rels_part = relationships_part_for(workbook_part);
    if let Ok(rels_xml) = zip_text(file, &rels_part) {
        let (updated_rels, calc_chain_parts) =
            remove_xlsx_calc_chain_relationships(&rels_xml, workbook_part);
        for part in calc_chain_parts {
            removals.insert(part.trim_start_matches('/').to_string());
        }
        if updated_rels != rels_xml {
            overrides.insert(rels_part, updated_rels);
        }
    }

    removals.insert("xl/calcChain.xml".to_string());
    Ok(())
}

pub(crate) fn xlsx_workbook_waiting_for_formula_recalc(file: &str) -> CliResult<bool> {
    let workbook_xml = zip_text(file, "xl/workbook.xml")?;
    Ok(xlsx_workbook_xml_waiting_for_formula_recalc(&workbook_xml))
}

fn xlsx_workbook_xml_waiting_for_formula_recalc(xml: &str) -> bool {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e))
                if local_name(e.name().as_ref()) == "calcPr" =>
            {
                let attrs = xml_attrs_map(&e);
                return xlsx_truthy_xml_bool(attrs.get("fullCalcOnLoad"))
                    || xlsx_truthy_xml_bool(attrs.get("forceFullCalc"));
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    false
}

fn xlsx_truthy_xml_bool(value: Option<&String>) -> bool {
    value.is_some_and(|value| {
        let normalized = value.trim().to_ascii_lowercase();
        normalized == "1" || normalized == "true"
    })
}

fn ensure_xlsx_full_calc_on_load(xml: String) -> String {
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "calcPr" => {
                let end = reader.buffer_position() as usize;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = xml_attrs_map(&e);
                attrs.insert("fullCalcOnLoad".to_string(), "1".to_string());
                attrs.insert("forceFullCalc".to_string(), "1".to_string());
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
                attrs.insert("fullCalcOnLoad".to_string(), "1".to_string());
                attrs.insert("forceFullCalc".to_string(), "1".to_string());
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

    let calc_pr = r#"<calcPr fullCalcOnLoad="1" forceFullCalc="1"/>"#;
    insert_xlsx_workbook_child_ordered(&xml, "calcPr", calc_pr).unwrap_or(xml)
}

fn xlsx_calc_chain_parts_from_content_types(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut parts = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Override" =>
            {
                let attrs = xml_attrs_map(&e);
                if attrs.get("ContentType").is_some_and(|value| {
                    value
                        == "application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml"
                }) && let Some(part_name) = attrs.get("PartName")
                {
                    parts.push(part_name.trim_start_matches('/').to_string());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    parts
}

fn remove_xlsx_calc_chain_content_type_overrides(xml: &str) -> String {
    remove_xml_elements_matching(xml, "Override", |attrs| {
        attrs.get("ContentType").is_some_and(|value| {
            value == "application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml"
        })
    })
}

fn remove_xlsx_calc_chain_relationships(xml: &str, workbook_part: &str) -> (String, Vec<String>) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut parts = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Relationship" =>
            {
                let attrs = xml_attrs_map(&e);
                if attrs.get("Type").is_some_and(|value| {
                    value == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain"
                }) && let Some(target) = attrs.get("Target")
                {
                    parts.push(
                        resolve_relationship_target(workbook_part, target)
                            .trim_start_matches('/')
                            .to_string(),
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    let updated = remove_xml_elements_matching(xml, "Relationship", |attrs| {
        attrs.get("Type").is_some_and(|value| {
            value == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain"
        })
    });
    (updated, parts)
}

fn remove_xml_elements_matching<F>(xml: &str, element_local: &str, predicate: F) -> String
where
    F: Fn(&BTreeMap<String, String>) -> bool,
{
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut spans = Vec::<(usize, usize)>::new();
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == element_local => {
                if predicate(&xml_attrs_map(&e)) {
                    spans.push((start, reader.buffer_position() as usize));
                }
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == element_local => {
                if predicate(&xml_attrs_map(&e)) {
                    let mut depth = 1usize;
                    loop {
                        match reader.read_event() {
                            Ok(Event::Start(inner))
                                if local_name(inner.name().as_ref()) == element_local =>
                            {
                                depth += 1;
                            }
                            Ok(Event::End(inner))
                                if local_name(inner.name().as_ref()) == element_local =>
                            {
                                depth -= 1;
                                if depth == 0 {
                                    spans.push((start, reader.buffer_position() as usize));
                                    break;
                                }
                            }
                            Ok(Event::Eof) | Err(_) => {
                                spans.push((start, reader.buffer_position() as usize));
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    if spans.is_empty() {
        return xml.to_string();
    }
    let mut out = String::with_capacity(xml.len());
    let mut cursor = 0usize;
    for (start, end) in spans {
        if start > cursor {
            out.push_str(&xml[cursor..start]);
        }
        cursor = end;
    }
    out.push_str(&xml[cursor..]);
    out
}
