use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, local_name, relationships, relationships_part_for, render_xml_attrs,
    replace_xml_span, resolve_relationship_target, workbook_sheets, xml_attrs_map,
    xml_direct_child_ranges, xml_open_tag_from_start, xml_tag_prefix, zip_text,
};

use super::cfb::CfbFile;
use super::model::VbaInfo;
use super::package_xml::package_part_name;

#[derive(Default)]
struct XlsxVbaDocumentCodeNames {
    workbook: Option<String>,
    sheets: Vec<String>,
}

pub(super) fn xlsx_vba_document_code_name_overrides(
    file: &str,
    info: &VbaInfo,
    data: &[u8],
) -> CliResult<BTreeMap<String, String>> {
    let Some(code_names) = xlsx_document_code_names_from_vba_project(data)? else {
        return Ok(BTreeMap::new());
    };

    let mut text_overrides = BTreeMap::new();
    let workbook_part = package_part_name(&info.main_part_uri);
    let mut workbook_xml = zip_text(file, &workbook_part)?;
    if let Some(workbook_code_name) = &code_names.workbook {
        workbook_xml = set_workbook_code_name(&workbook_xml, workbook_code_name)?;
    }

    let sheets = workbook_sheets(&workbook_xml)?;
    if !code_names.sheets.is_empty() && !sheets.is_empty() {
        let workbook_rels_part = relationships_part_for(&info.main_part_uri);
        let rels = relationships(file, &workbook_rels_part)?;
        for sheet_code_name in &code_names.sheets {
            let Some(position) = sheet_code_name
                .strip_prefix("Sheet")
                .and_then(|suffix| suffix.parse::<u32>().ok())
            else {
                continue;
            };
            let Some(sheet) = sheets.iter().find(|sheet| sheet.position == position) else {
                continue;
            };
            let Some(target) = rels.get(&sheet.rel_id) else {
                continue;
            };
            let sheet_part =
                package_part_name(&resolve_relationship_target(&info.main_part_uri, target));
            let sheet_xml = text_overrides
                .get(&sheet_part)
                .cloned()
                .unwrap_or_else(|| zip_text(file, &sheet_part).unwrap_or_default());
            if sheet_xml.is_empty() {
                continue;
            }
            text_overrides.insert(
                sheet_part,
                set_worksheet_code_name(&sheet_xml, sheet_code_name)?,
            );
        }
    }
    text_overrides.insert(workbook_part, workbook_xml);
    Ok(text_overrides)
}

fn xlsx_document_code_names_from_vba_project(
    data: &[u8],
) -> CliResult<Option<XlsxVbaDocumentCodeNames>> {
    let cfb = match CfbFile::open(data) {
        Ok(cfb) => cfb,
        Err(_) => return Ok(None),
    };
    let project_stream = match cfb.stream("PROJECT") {
        Ok(stream) => stream,
        Err(_) => return Ok(None),
    };
    let project_text = String::from_utf8_lossy(&project_stream)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut code_names = XlsxVbaDocumentCodeNames::default();
    for line in project_text.lines() {
        let trimmed = line.trim();
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("document") {
            continue;
        }
        let name = value
            .split_once('/')
            .map(|(before, _)| before)
            .unwrap_or(value)
            .trim()
            .trim_matches('"')
            .to_string();
        if name.eq_ignore_ascii_case("ThisWorkbook") {
            code_names.workbook = Some(name);
        } else if !name.is_empty() {
            code_names.sheets.push(name);
        }
    }
    if code_names.workbook.is_none() && code_names.sheets.is_empty() {
        Ok(None)
    } else {
        Ok(Some(code_names))
    }
}

fn set_workbook_code_name(xml: &str, code_name: &str) -> CliResult<String> {
    if let Some(span) = first_direct_child_open_span(xml, "workbook", "workbookPr")? {
        return Ok(replace_element_start_tag_attr(
            xml, &span, "codeName", code_name,
        ));
    }
    let root = root_open_span(xml, "workbook")?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let child = format!(
        "<{} codeName=\"{}\"/>",
        element_name(&prefix, "workbookPr"),
        crate::xml_attr_escape(code_name)
    );
    crate::insert_xlsx_workbook_child_ordered(xml, "workbookPr", &child)
        .ok_or_else(|| CliError::unexpected("failed to insert workbookPr codeName"))
}

fn set_worksheet_code_name(xml: &str, code_name: &str) -> CliResult<String> {
    if let Some(span) = first_direct_child_open_span(xml, "worksheet", "sheetPr")? {
        return Ok(replace_element_start_tag_attr(
            xml, &span, "codeName", code_name,
        ));
    }
    let root = root_open_span(xml, "worksheet")?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let child = format!(
        "<{} codeName=\"{}\"/>",
        element_name(&prefix, "sheetPr"),
        crate::xml_attr_escape(code_name)
    );
    insert_worksheet_child_ordered(xml, &root, "sheetPr", &child)
}

#[derive(Clone)]
struct ElementOpenSpan {
    start: usize,
    open_end: usize,
    end: usize,
    tag_name: String,
    attrs: BTreeMap<String, String>,
    self_closing: bool,
}

fn root_open_span(xml: &str, expected_local: &str) -> CliResult<ElementOpenSpan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == expected_local => {
                let open_end = reader.buffer_position() as usize;
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let close_tag = format!("</{tag_name}>");
                let close_start = xml.rfind(&close_tag).ok_or_else(|| {
                    CliError::unexpected(format!("{expected_local} root has no closing tag"))
                })?;
                return Ok(ElementOpenSpan {
                    start: before,
                    open_end,
                    end: close_start + close_tag.len(),
                    tag_name,
                    attrs: xml_attrs_map(&e),
                    self_closing: false,
                });
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == expected_local => {
                return Ok(ElementOpenSpan {
                    start: before,
                    open_end: reader.buffer_position() as usize,
                    end: reader.buffer_position() as usize,
                    tag_name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    attrs: xml_attrs_map(&e),
                    self_closing: true,
                });
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

fn first_direct_child_open_span(
    xml: &str,
    root_local: &str,
    child_local: &str,
) -> CliResult<Option<ElementOpenSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut depth = 0usize;
    let mut saw_root = false;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let local = local_name(e.name().as_ref()).to_string();
                if depth == 0 {
                    if local != root_local {
                        return Err(CliError::unexpected(format!(
                            "root is {local:?}, expected {root_local}"
                        )));
                    }
                    saw_root = true;
                } else if depth == 1 && local == child_local {
                    return Ok(Some(open_span_from_start(xml, before, &e, false)?));
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                let local = local_name(e.name().as_ref()).to_string();
                if depth == 0 {
                    if local != root_local {
                        return Err(CliError::unexpected(format!(
                            "root is {local:?}, expected {root_local}"
                        )));
                    }
                    saw_root = true;
                } else if depth == 1 && local == child_local {
                    return Ok(Some(open_span_from_start(xml, before, &e, true)?));
                }
            }
            Ok(Event::End(_)) => {
                if depth == 1 {
                    return Ok(None);
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => {
                return if saw_root {
                    Ok(None)
                } else {
                    Err(CliError::unexpected(format!("{root_local} root not found")))
                };
            }
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn open_span_from_start(
    xml: &str,
    start: usize,
    element: &BytesStart<'_>,
    self_closing: bool,
) -> CliResult<ElementOpenSpan> {
    let tag_name = String::from_utf8_lossy(element.name().as_ref()).to_string();
    let open_end = xml[start..]
        .find('>')
        .map(|offset| start + offset + 1)
        .ok_or_else(|| CliError::unexpected("invalid XML start tag"))?;
    Ok(ElementOpenSpan {
        start,
        open_end,
        end: open_end,
        tag_name,
        attrs: xml_attrs_map(element),
        self_closing,
    })
}

fn replace_element_start_tag_attr(
    xml: &str,
    span: &ElementOpenSpan,
    attr_name: &str,
    attr_value: &str,
) -> String {
    let mut attrs = span.attrs.clone();
    attrs.insert(attr_name.to_string(), attr_value.to_string());
    let replacement = if span.self_closing {
        format!("<{}{}/>", span.tag_name, render_xml_attrs(&attrs))
    } else {
        format!("<{}{}>", span.tag_name, render_xml_attrs(&attrs))
    };
    replace_xml_span(xml, span.start, span.open_end, &replacement)
}

fn insert_worksheet_child_ordered(
    xml: &str,
    root: &ElementOpenSpan,
    child_local: &str,
    child_xml: &str,
) -> CliResult<String> {
    if root.self_closing {
        let start_tag = xml_open_tag_from_start(&xml[root.start..root.open_end]);
        let mut out = String::with_capacity(xml.len() + child_xml.len() + root.tag_name.len() + 3);
        out.push_str(&xml[..root.start]);
        out.push_str(&start_tag);
        out.push_str(child_xml);
        out.push_str(&format!("</{}>", root.tag_name));
        out.push_str(&xml[root.end..]);
        return Ok(out);
    }
    let insert_at =
        xml_direct_child_ranges(xml, root.open_end, root.end - root.tag_name.len() - 3)?
            .into_iter()
            .find(|child| worksheet_child_order(&child.kind) > worksheet_child_order(child_local))
            .map(|child| child.start)
            .unwrap_or(root.end - root.tag_name.len() - 3);
    let mut out = String::with_capacity(xml.len() + child_xml.len());
    out.push_str(&xml[..insert_at]);
    out.push_str(child_xml);
    out.push_str(&xml[insert_at..]);
    Ok(out)
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

fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}
