use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::Value;

use crate::{CliResult, zip_text};

use super::spec::{
    CHART_NAMESPACE, CONTENT_TYPE_CHART, CONTENT_TYPE_DOCX_DOCUMENT, CONTENT_TYPE_DOCX_FOOTER,
    CONTENT_TYPE_DOCX_HEADER, CONTENT_TYPE_DRAWING, CONTENT_TYPE_PPTX_SLIDE,
    CONTENT_TYPE_PPTX_SLIDE_LAYOUT, CONTENT_TYPE_PPTX_SLIDE_MASTER, CONTENT_TYPE_XLSX_CALC_CHAIN,
    CONTENT_TYPE_XLSX_PIVOT_CACHE, CONTENT_TYPE_XLSX_PIVOT_RECORDS, CONTENT_TYPE_XLSX_PIVOT_TABLE,
    CONTENT_TYPE_XLSX_SHARED_STRINGS, CONTENT_TYPE_XLSX_STYLES, CONTENT_TYPE_XLSX_TABLE,
    CONTENT_TYPE_XLSX_WORKSHEET, PRESENTATIONML_NAMESPACE, SPREADSHEET_DRAWING_NAMESPACE,
    SPREADSHEETML_NAMESPACE, WORDPROCESSINGML_NAMESPACE, is_pptx_presentation_content_type,
    is_xlsx_workbook_content_type,
};
use super::types::{PartInfo, XmlElementInfo, XmlPartInfo};
use super::util::{diag, xml_element_info};

pub(super) fn check_part_xml_invariants(file: &str, part: &PartInfo) -> CliResult<Vec<Value>> {
    let check = match part.content_type.as_str() {
        ct if is_xlsx_workbook_content_type(ct) => Some((
            "workbook",
            "workbook",
            SPREADSHEETML_NAMESPACE,
            "XLSX_WORKBOOK_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_SHARED_STRINGS => Some((
            "shared strings",
            "sst",
            SPREADSHEETML_NAMESPACE,
            "XLSX_SHARED_STRINGS_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_STYLES => Some((
            "styles",
            "styleSheet",
            SPREADSHEETML_NAMESPACE,
            "XLSX_STYLES_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_CALC_CHAIN => Some((
            "calc chain",
            "calcChain",
            SPREADSHEETML_NAMESPACE,
            "XLSX_CALC_CHAIN_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_TABLE => Some((
            "table",
            "table",
            SPREADSHEETML_NAMESPACE,
            "XLSX_TABLE_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_PIVOT_TABLE => Some((
            "pivot table",
            "pivotTableDefinition",
            SPREADSHEETML_NAMESPACE,
            "XLSX_PIVOT_TABLE_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_PIVOT_CACHE => Some((
            "pivot cache definition",
            "pivotCacheDefinition",
            SPREADSHEETML_NAMESPACE,
            "XLSX_PIVOT_CACHE_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_PIVOT_RECORDS => Some((
            "pivot cache records",
            "pivotCacheRecords",
            SPREADSHEETML_NAMESPACE,
            "XLSX_PIVOT_RECORDS_ROOT",
            None,
        )),
        ct if is_pptx_presentation_content_type(ct) => Some((
            "presentation",
            "presentation",
            PRESENTATIONML_NAMESPACE,
            "PPTX_PRESENTATION_ROOT",
            None,
        )),
        CONTENT_TYPE_DOCX_DOCUMENT => Some((
            "document",
            "document",
            WORDPROCESSINGML_NAMESPACE,
            "DOCX_DOCUMENT_ROOT",
            None,
        )),
        CONTENT_TYPE_DOCX_HEADER => Some((
            "header",
            "hdr",
            WORDPROCESSINGML_NAMESPACE,
            "DOCX_HEADER_ROOT",
            None,
        )),
        CONTENT_TYPE_DOCX_FOOTER => Some((
            "footer",
            "ftr",
            WORDPROCESSINGML_NAMESPACE,
            "DOCX_FOOTER_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_WORKSHEET => Some((
            "worksheet",
            "worksheet",
            SPREADSHEETML_NAMESPACE,
            "XLSX_WORKSHEET_ROOT",
            Some((
                "XLSX_WORKSHEET_CHILD_ORDER",
                worksheet_child_order as fn(&str) -> usize,
            )),
        )),
        CONTENT_TYPE_PPTX_SLIDE => Some((
            "slide",
            "sld",
            PRESENTATIONML_NAMESPACE,
            "PPTX_SLIDE_ROOT",
            Some((
                "PPTX_SLIDE_CHILD_ORDER",
                slide_child_order as fn(&str) -> usize,
            )),
        )),
        CONTENT_TYPE_PPTX_SLIDE_LAYOUT => Some((
            "slide layout",
            "sldLayout",
            PRESENTATIONML_NAMESPACE,
            "PPTX_SLIDE_LAYOUT_ROOT",
            Some((
                "PPTX_SLIDE_LAYOUT_CHILD_ORDER",
                slide_layout_child_order as fn(&str) -> usize,
            )),
        )),
        CONTENT_TYPE_PPTX_SLIDE_MASTER => Some((
            "slide master",
            "sldMaster",
            PRESENTATIONML_NAMESPACE,
            "PPTX_SLIDE_MASTER_ROOT",
            Some((
                "PPTX_SLIDE_MASTER_CHILD_ORDER",
                slide_master_child_order as fn(&str) -> usize,
            )),
        )),
        CONTENT_TYPE_DRAWING => Some((
            "drawing",
            "wsDr",
            SPREADSHEET_DRAWING_NAMESPACE,
            "XLSX_DRAWING_ROOT",
            None,
        )),
        CONTENT_TYPE_CHART => Some((
            "chart",
            "chartSpace",
            CHART_NAMESPACE,
            "OOXML_CHART_ROOT",
            None,
        )),
        _ => None,
    };

    let Some((label, expected_local, expected_ns, root_code, order_check)) = check else {
        return Ok(Vec::new());
    };
    let info = match read_xml_part_info(file, part) {
        Ok(info) => info,
        Err(err) => {
            return Ok(vec![diag(
                "OOXML_XML_PARSE_ERROR",
                format!("failed to read {label} {}: {}", part.uri, err.message),
            )]);
        }
    };
    let root_diags = check_root_name(
        &part.uri,
        &info.root,
        expected_local,
        expected_ns,
        root_code,
    );
    if !root_diags.is_empty() {
        return Ok(root_diags);
    }

    let mut diagnostics = Vec::new();
    if let Some((code, order)) = order_check {
        diagnostics.extend(check_element_order(&part.uri, &info, order, code));
    }
    match part.content_type.as_str() {
        CONTENT_TYPE_XLSX_SHARED_STRINGS => {
            diagnostics.extend(check_shared_string_counts(&part.uri, &info));
        }
        CONTENT_TYPE_XLSX_STYLES => {
            diagnostics.extend(check_styles_counts(&part.uri, &info));
        }
        _ => {}
    }
    Ok(diagnostics)
}

fn read_xml_part_info(file: &str, part: &PartInfo) -> CliResult<XmlPartInfo> {
    let xml = zip_text(file, &part.entry_name)?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut info = XmlPartInfo::default();
    let mut stack = Vec::<String>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let element = xml_element_info(&e);
                if stack.is_empty() {
                    info.root = Some(element.clone());
                } else {
                    if stack.len() == 1 {
                        info.children.push(element.clone());
                    }
                    if let Some(parent) = stack.last() {
                        *info
                            .direct_child_counts
                            .entry((parent.clone(), element.local_name.clone()))
                            .or_insert(0) += 1;
                    }
                }
                stack.push(element.local_name);
            }
            Ok(Event::Empty(e)) => {
                let element = xml_element_info(&e);
                if stack.is_empty() {
                    info.root = Some(element);
                } else {
                    if stack.len() == 1 {
                        info.children.push(element.clone());
                    }
                    if let Some(parent) = stack.last() {
                        *info
                            .direct_child_counts
                            .entry((parent.clone(), element.local_name))
                            .or_insert(0) += 1;
                    }
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(crate::CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(info)
}

fn check_root_name(
    part_uri: &str,
    root: &Option<XmlElementInfo>,
    expected_local_name: &str,
    expected_namespace: &str,
    code: &str,
) -> Vec<Value> {
    let Some(root) = root else {
        return vec![diag(code, format!("{part_uri} has no XML root"))];
    };
    if root.local_name != expected_local_name || root.namespace != expected_namespace {
        return vec![diag(
            code,
            format!(
                "{part_uri} root is {{{}}}{}, expected {{{}}}{}",
                root.namespace, root.local_name, expected_namespace, expected_local_name
            ),
        )];
    }
    Vec::new()
}

fn check_element_order(
    part_uri: &str,
    info: &XmlPartInfo,
    order: fn(&str) -> usize,
    code: &str,
) -> Vec<Value> {
    if info.root.is_none() {
        return vec![diag(code, format!("{part_uri} has no XML root"))];
    }
    let mut diagnostics = Vec::new();
    let mut last_order = 0usize;
    let mut last_name = "";
    for child in &info.children {
        let current = order(&child.local_name);
        if current == 0 {
            continue;
        }
        if last_order > current {
            diagnostics.push(diag(
                code,
                format!(
                    "{part_uri} has <{}> after <{last_name}>; expected schema child order",
                    child.local_name
                ),
            ));
            continue;
        }
        last_order = current;
        last_name = &child.local_name;
    }
    diagnostics
}

fn check_shared_string_counts(part_uri: &str, info: &XmlPartInfo) -> Vec<Value> {
    let items = info
        .children
        .iter()
        .filter(|child| child.local_name == "si")
        .count();
    let Some(root) = &info.root else {
        return Vec::new();
    };
    let (count, count_present, count_ok) = optional_unsigned_int_attr(root, "count");
    let (unique_count, unique_present, unique_ok) = optional_unsigned_int_attr(root, "uniqueCount");

    let mut diagnostics = Vec::new();
    if count_present && !count_ok {
        diagnostics.push(diag(
            "XLSX_SHARED_STRINGS_COUNTS",
            format!(
                "{} <sst> count {:?} is not a valid unsigned integer",
                part_uri,
                root.attrs.get("count").cloned().unwrap_or_default()
            ),
        ));
    }
    if unique_present && !unique_ok {
        diagnostics.push(diag(
            "XLSX_SHARED_STRINGS_COUNTS",
            format!(
                "{} <sst> uniqueCount {:?} is not a valid unsigned integer",
                part_uri,
                root.attrs.get("uniqueCount").cloned().unwrap_or_default()
            ),
        ));
    }
    if count_present && !unique_present {
        diagnostics.push(diag(
            "XLSX_SHARED_STRINGS_COUNTS",
            format!("{part_uri} <sst> uses count without required uniqueCount"),
        ));
    }
    if unique_present && !count_present {
        diagnostics.push(diag(
            "XLSX_SHARED_STRINGS_COUNTS",
            format!("{part_uri} <sst> uses uniqueCount without required count"),
        ));
    }
    if unique_present && unique_ok && unique_count != items {
        diagnostics.push(diag(
            "XLSX_SHARED_STRINGS_COUNTS",
            format!(
                "{part_uri} <sst> uniqueCount is {unique_count} but contains {items} <si> entries"
            ),
        ));
    }
    let _ = count;
    diagnostics
}

fn check_styles_counts(part_uri: &str, info: &XmlPartInfo) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(check_style_collection_count(
        part_uri, info, "numFmts", "numFmt",
    ));
    diagnostics.extend(check_style_collection_count(
        part_uri, info, "cellXfs", "xf",
    ));
    diagnostics
}

fn check_style_collection_count(
    part_uri: &str,
    info: &XmlPartInfo,
    collection_name: &str,
    child_name: &str,
) -> Vec<Value> {
    let Some(collection) = info
        .children
        .iter()
        .find(|child| child.local_name == collection_name)
    else {
        return Vec::new();
    };
    let (declared, present, ok) = optional_unsigned_int_attr(collection, "count");
    if !present {
        return Vec::new();
    }
    if !ok {
        return vec![diag(
            "XLSX_STYLES_COUNT_MISMATCH",
            format!(
                "{} <{}> count {:?} is not a valid unsigned integer",
                part_uri,
                collection_name,
                collection.attrs.get("count").cloned().unwrap_or_default()
            ),
        )];
    }
    let actual = info
        .direct_child_counts
        .get(&(collection_name.to_string(), child_name.to_string()))
        .copied()
        .unwrap_or(0);
    if declared != actual {
        return vec![diag(
            "XLSX_STYLES_COUNT_MISMATCH",
            format!(
                "{part_uri} <{collection_name}> count is {declared} but contains {actual} <{child_name}> entries"
            ),
        )];
    }
    Vec::new()
}

fn optional_unsigned_int_attr(elem: &XmlElementInfo, name: &str) -> (usize, bool, bool) {
    let Some(raw) = elem.attrs.get(name) else {
        return (0, false, false);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return (0, true, false);
    }
    match raw.parse::<usize>() {
        Ok(value) => (value, true, true),
        Err(_) => (0, true, false),
    }
}

fn worksheet_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "sheetPr",
            "dimension",
            "sheetViews",
            "sheetFormatPr",
            "cols",
            "sheetData",
            "sheetCalcPr",
            "sheetProtection",
            "protectedRanges",
            "scenarios",
            "autoFilter",
            "sortState",
            "dataConsolidate",
            "customSheetViews",
            "mergeCells",
            "phoneticPr",
            "conditionalFormatting",
            "dataValidations",
            "hyperlinks",
            "printOptions",
            "pageMargins",
            "pageSetup",
            "headerFooter",
            "rowBreaks",
            "colBreaks",
            "customProperties",
            "cellWatches",
            "ignoredErrors",
            "smartTags",
            "drawing",
            "legacyDrawing",
            "legacyDrawingHF",
            "drawingHF",
            "picture",
            "oleObjects",
            "controls",
            "webPublishItems",
            "tableParts",
            "extLst",
        ],
    )
}

fn slide_child_order(name: &str) -> usize {
    order_index(
        name,
        &["cSld", "clrMapOvr", "transition", "timing", "extLst"],
    )
}

fn slide_layout_child_order(name: &str) -> usize {
    order_index(
        name,
        &["cSld", "clrMapOvr", "transition", "timing", "hf", "extLst"],
    )
}

fn slide_master_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "cSld",
            "clrMap",
            "sldLayoutIdLst",
            "transition",
            "timing",
            "hf",
            "txStyles",
            "extLst",
        ],
    )
}

fn order_index(name: &str, ordered_names: &[&str]) -> usize {
    ordered_names
        .iter()
        .position(|candidate| *candidate == name)
        .map(|idx| idx + 1)
        .unwrap_or(0)
}
