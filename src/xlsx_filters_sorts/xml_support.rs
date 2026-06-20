use super::*;

#[derive(Clone)]
pub(super) struct XmlRootBounds {
    pub(super) start: usize,
    pub(super) open_end: usize,
    pub(super) close_start: usize,
    pub(super) end: usize,
    pub(super) tag_name: String,
    pub(super) self_closing: bool,
}

pub(super) fn xml_root_bounds(xml: &str, expected_local: &str) -> CliResult<XmlRootBounds> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == expected_local => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let close_tag = format!("</{tag_name}>");
                let close_start = xml.rfind(&close_tag).ok_or_else(|| {
                    CliError::unexpected(format!("{expected_local} root has no closing tag"))
                })?;
                return Ok(XmlRootBounds {
                    start: before,
                    open_end: reader.buffer_position() as usize,
                    close_start,
                    end: close_start + close_tag.len(),
                    tag_name,
                    self_closing: false,
                });
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == expected_local => {
                let end = reader.buffer_position() as usize;
                return Ok(XmlRootBounds {
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
                    "{expected_local} root is {:?}",
                    local_name(e.name().as_ref())
                )));
            }
            Ok(Event::Eof) => {
                return Err(CliError::unexpected(format!(
                    "{expected_local} root not found"
                )));
            }
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

pub(super) fn direct_child_range(
    xml: &str,
    root: &XmlRootBounds,
    kind: &str,
) -> CliResult<Option<crate::XmlNamedRange>> {
    Ok(
        xml_direct_child_ranges(xml, root.open_end, root.close_start)?
            .into_iter()
            .find(|child| child.kind == kind),
    )
}

pub(super) fn insert_ordered_child(
    xml: &str,
    root: &XmlRootBounds,
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
    let mut updated = String::new();
    updated.push_str(&xml[..insert_at]);
    updated.push_str(child_xml);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
}

pub(super) fn insert_first_child(
    xml: &str,
    root: &XmlRootBounds,
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
    let insert_at = xml_direct_child_ranges(xml, root.open_end, root.close_start)?
        .into_iter()
        .next()
        .map(|child| child.start)
        .unwrap_or(root.close_start);
    let mut updated = String::new();
    updated.push_str(&xml[..insert_at]);
    updated.push_str(child_xml);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
}

pub(super) fn replace_element_ref_attr(fragment: &str, range: &str) -> CliResult<String> {
    let (tag_name, mut attrs, self_closing, open_end) = first_element(fragment)?;
    attrs.insert("ref".to_string(), range.to_string());
    let tag = if self_closing {
        format!("<{}{}{}>", tag_name, render_xml_attrs(&attrs), "/")
    } else {
        format!("<{}{}>", tag_name, render_xml_attrs(&attrs))
    };
    Ok(replace_xml_span(fragment, 0, open_end, &tag))
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

pub(super) fn attr_local_start(element: &BytesStart<'_>, wanted: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attr| {
        if local_name(attr.key.as_ref()) == wanted {
            Some(crate::decode_xml_text(attr.value.as_ref()))
        } else {
            None
        }
    })
}

pub(super) fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

pub(super) fn worksheet_child_order(local_name: &str) -> i32 {
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
