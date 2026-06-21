use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, local_name, replace_xml_span, xml_attrs_map, xml_direct_child_ranges,
    xml_open_tag_from_start,
};

#[derive(Clone)]
pub(super) struct WorksheetRootBounds {
    pub(super) start: usize,
    pub(super) open_end: usize,
    pub(super) close_start: usize,
    pub(super) end: usize,
    pub(super) tag_name: String,
    pub(super) self_closing: bool,
}

pub(super) fn worksheet_root_bounds(xml: &str) -> CliResult<WorksheetRootBounds> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "worksheet" => {
                let open_end = reader.buffer_position() as usize;
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let close_tag = format!("</{tag_name}>");
                let close_start = xml
                    .rfind(&close_tag)
                    .ok_or_else(|| CliError::unexpected("worksheet root has no closing tag"))?;
                return Ok(WorksheetRootBounds {
                    start: before,
                    open_end,
                    close_start,
                    end: close_start + close_tag.len(),
                    tag_name,
                    self_closing: false,
                });
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "worksheet" => {
                let end = reader.buffer_position() as usize;
                return Ok(WorksheetRootBounds {
                    start: before,
                    open_end: end,
                    close_start: end,
                    end,
                    tag_name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    self_closing: true,
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                return Err(CliError::unexpected(format!(
                    "worksheet root is {:?}",
                    local_name(e.name().as_ref())
                )));
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("worksheet root not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

pub(super) fn insert_worksheet_child(
    xml: &str,
    root: &WorksheetRootBounds,
    local_name: &str,
    child_xml: &str,
) -> CliResult<String> {
    if root.self_closing {
        let start_tag = xml_open_tag_from_start(&xml[root.start..root.open_end]);
        let mut updated = String::new();
        updated.push_str(&xml[..root.start]);
        updated.push_str(&start_tag);
        updated.push_str(child_xml);
        updated.push_str(&format!("</{}>", root.tag_name));
        updated.push_str(&xml[root.end..]);
        return Ok(updated);
    }
    let target_order = worksheet_child_order(local_name);
    let insert_at = xml_direct_child_ranges(xml, root.open_end, root.close_start)?
        .into_iter()
        .find(|child| worksheet_child_order(&child.kind) > target_order)
        .map(|child| child.start)
        .unwrap_or(root.close_start);
    Ok(replace_xml_span(xml, insert_at, insert_at, child_xml))
}

pub(super) fn first_element(
    fragment: &str,
) -> CliResult<(String, BTreeMap<String, String>, bool, usize)> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let end = reader.buffer_position() as usize;
                return Ok((
                    String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    xml_attrs_map(&e),
                    false,
                    end,
                ));
            }
            Ok(Event::Empty(e)) => {
                let end = reader.buffer_position() as usize;
                return Ok((
                    String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    xml_attrs_map(&e),
                    true,
                    end,
                ));
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("XML element not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

pub(super) fn attr_local(attrs: &BTreeMap<String, String>, wanted: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(key, _)| local_name(key.as_bytes()) == wanted)
        .map(|(_, value)| value.clone())
}

pub(super) fn attr_is_true(attrs: &BTreeMap<String, String>, key: &str) -> bool {
    attr_local(attrs, key)
        .map(|value| {
            let value = value.trim();
            value == "1" || value == "true"
        })
        .unwrap_or(false)
}

pub(super) fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn worksheet_child_order(local_name: &str) -> i32 {
    match local_name {
        "sheetPr" => 10,
        "dimension" => 20,
        "sheetViews" => 30,
        "sheetFormatPr" => 40,
        "cols" => 50,
        "sheetData" => 60,
        "sheetCalcPr" => 70,
        "sheetProtection" => 80,
        "protectedRanges" => 90,
        "scenarios" => 100,
        "autoFilter" => 110,
        "sortState" => 120,
        "dataConsolidate" => 130,
        "customSheetViews" => 140,
        "mergeCells" => 150,
        "phoneticPr" => 160,
        "conditionalFormatting" => 170,
        "dataValidations" => 180,
        "hyperlinks" => 190,
        "printOptions" => 200,
        "pageMargins" => 210,
        "pageSetup" => 220,
        "headerFooter" => 230,
        "rowBreaks" => 240,
        "colBreaks" => 250,
        "customProperties" => 260,
        "cellWatches" => 270,
        "ignoredErrors" => 280,
        "smartTags" => 290,
        "drawing" => 300,
        "legacyDrawing" => 310,
        "legacyDrawingHF" => 320,
        "picture" => 330,
        "oleObjects" => 340,
        "controls" => 350,
        "webPublishItems" => 360,
        "tableParts" => 370,
        "extLst" => 380,
        _ => 1000,
    }
}
