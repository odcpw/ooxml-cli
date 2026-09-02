use quick_xml::Reader;
use quick_xml::events::Event;
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CliError, CliResult, RangeBounds, XmlNamedRange, attr, col_name, local_name, parse_cell_ref,
    parse_range, render_xml_attrs, xml_attrs, xml_direct_child_ranges, zip_entry_names, zip_text,
};
#[derive(Clone)]
pub(crate) struct XlsxSheetDataSpan {
    pub(crate) start: usize,
    pub(crate) open_end: usize,
    pub(crate) close_start: usize,
    pub(crate) end: usize,
    pub(crate) empty: bool,
}

#[derive(Clone)]
pub(crate) struct XlsxRowSpan {
    pub(crate) row: u32,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) attrs: BTreeMap<String, String>,
    pub(crate) cells: BTreeMap<u32, XlsxCellSpan>,
}

#[derive(Clone)]
pub(crate) struct XlsxCellSpan {
    pub(crate) xml: String,
    pub(crate) attrs: BTreeMap<String, String>,
    pub(crate) has_formula: bool,
}

#[derive(Clone)]
pub(crate) struct XlsxWorksheetRootBounds {
    pub(crate) start: usize,
    pub(crate) open_end: usize,
    pub(crate) close_start: usize,
    pub(crate) end: usize,
    pub(crate) tag_name: String,
    pub(crate) self_closing: bool,
}

pub(crate) fn xlsx_worksheet_root_bounds(xml: &str) -> CliResult<XlsxWorksheetRootBounds> {
    xlsx_worksheet_root_bounds_impl(xml, true)
}

pub(crate) fn xlsx_worksheet_root_bounds_permissive(
    xml: &str,
) -> CliResult<XlsxWorksheetRootBounds> {
    xlsx_worksheet_root_bounds_impl(xml, false)
}

fn xlsx_worksheet_root_bounds_impl(
    xml: &str,
    reject_other_root: bool,
) -> CliResult<XlsxWorksheetRootBounds> {
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
                return Ok(XlsxWorksheetRootBounds {
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
                return Ok(XlsxWorksheetRootBounds {
                    start: before,
                    open_end: end,
                    close_start: end,
                    end,
                    tag_name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    self_closing: true,
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if reject_other_root => {
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

pub(crate) fn xlsx_direct_worksheet_child_range(
    xml: &str,
    root: &XlsxWorksheetRootBounds,
    kind: &str,
) -> CliResult<Option<XmlNamedRange>> {
    if root.self_closing || root.open_end >= root.close_start {
        return Ok(None);
    }
    Ok(
        xml_direct_child_ranges(xml, root.open_end, root.close_start)?
            .into_iter()
            .find(|child| child.kind == kind),
    )
}

/// Remove the legacy `pivotTableParts` worksheet child emitted by old
/// ooxml-cli releases.
///
/// SpreadsheetML links pivot tables exclusively through the worksheet
/// relationships part. A `pivotTableParts` child is never valid under
/// `CT_Worksheet`; `tableParts`, however, is valid and must remain untouched.
pub(crate) fn strip_legacy_xlsx_pivot_table_parts(xml: &str) -> CliResult<(String, usize)> {
    let root = xlsx_worksheet_root_bounds(xml)?;
    if root.self_closing || root.open_end >= root.close_start {
        return Ok((xml.to_string(), 0));
    }

    let mut ranges = xml_direct_child_ranges(xml, root.open_end, root.close_start)?
        .into_iter()
        .filter(|child| child.kind == "pivotTableParts")
        .collect::<Vec<_>>();
    if ranges.is_empty() {
        return Ok((xml.to_string(), 0));
    }

    let count = ranges.len();
    ranges.sort_by_key(|child| child.start);
    let mut stripped = xml.to_string();
    for child in ranges.into_iter().rev() {
        stripped.replace_range(child.start..child.end, "");
    }
    Ok((stripped, count))
}

pub(crate) fn xlsx_package_has_legacy_pivot_table_parts(file: &str) -> CliResult<bool> {
    for part in zip_entry_names(file)?.into_iter().filter(|entry| {
        entry.starts_with("xl/worksheets/") && entry.ends_with(".xml") && !entry.contains("/_rels/")
    }) {
        let (_, removed) = strip_legacy_xlsx_pivot_table_parts(&zip_text(file, &part)?)?;
        if removed > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn xlsx_sheet_data_span(xml: &str) -> CliResult<Option<XlsxSheetDataSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                let open_end = reader.buffer_position() as usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                            return Ok(Some(XlsxSheetDataSpan {
                                start: before,
                                open_end,
                                close_start: inner_before,
                                end: reader.buffer_position() as usize,
                                empty: false,
                            }));
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("sheetData has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                let end = reader.buffer_position() as usize;
                return Ok(Some(XlsxSheetDataSpan {
                    start: before,
                    open_end: end,
                    close_start: end,
                    end,
                    empty: true,
                }));
            }
            Ok(Event::Eof) => return Ok(None),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

pub(crate) fn parse_xlsx_row_spans(
    xml: &str,
    sheet_data: Option<&XlsxSheetDataSpan>,
) -> CliResult<BTreeMap<u32, XlsxRowSpan>> {
    let Some(sheet_data) = sheet_data.filter(|span| !span.empty) else {
        return Ok(BTreeMap::new());
    };
    let fragment = &xml[sheet_data.open_end..sheet_data.close_start];
    let base = sheet_data.open_end;
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut rows = BTreeMap::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "row" => {
                let Some(row) = attr(&e, "r").and_then(|value| value.parse::<u32>().ok()) else {
                    continue;
                };
                let attrs = xml_attrs(&e);
                loop {
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "row" => {
                            let start = base + before;
                            let end = base + reader.buffer_position() as usize;
                            let row_xml = &xml[start..end];
                            rows.insert(
                                row,
                                XlsxRowSpan {
                                    row,
                                    start,
                                    end,
                                    attrs,
                                    cells: parse_xlsx_cell_spans(row_xml, start)?,
                                },
                            );
                            break;
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("row has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "row" => {
                if let Some(row) = attr(&e, "r").and_then(|value| value.parse::<u32>().ok()) {
                    let start = base + before;
                    let end = base + reader.buffer_position() as usize;
                    rows.insert(
                        row,
                        XlsxRowSpan {
                            row,
                            start,
                            end,
                            attrs: xml_attrs(&e),
                            cells: BTreeMap::new(),
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(rows)
}

fn parse_xlsx_cell_spans(row_xml: &str, base: usize) -> CliResult<BTreeMap<u32, XlsxCellSpan>> {
    let mut reader = Reader::from_str(row_xml);
    reader.config_mut().trim_text(false);
    let mut cells = BTreeMap::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "c" => {
                let Some(addr) = attr(&e, "r") else {
                    continue;
                };
                let (col, _) = parse_cell_ref(&addr)?;
                let attrs = xml_attrs(&e);
                loop {
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "c" => {
                            let end = reader.buffer_position() as usize;
                            let xml = row_xml[before..end].to_string();
                            cells.insert(
                                col,
                                XlsxCellSpan {
                                    has_formula: xlsx_cell_xml_has_formula(&xml),
                                    xml,
                                    attrs,
                                },
                            );
                            break;
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("cell has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "c" => {
                if let Some(addr) = attr(&e, "r") {
                    let (col, _) = parse_cell_ref(&addr)?;
                    let end = reader.buffer_position() as usize;
                    let xml = row_xml[before..end].to_string();
                    cells.insert(
                        col,
                        XlsxCellSpan {
                            has_formula: false,
                            xml,
                            attrs: xml_attrs(&e),
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    let _ = base;
    Ok(cells)
}

fn xlsx_cell_xml_has_formula(cell_xml: &str) -> bool {
    let mut reader = Reader::from_str(cell_xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "f" => {
                return true;
            }
            Ok(Event::Eof) => return false,
            Err(_) => return false,
            _ => {}
        }
    }
}

pub(crate) fn render_xlsx_row(
    row_number: u32,
    row_span: Option<&XlsxRowSpan>,
    cells: BTreeMap<u32, String>,
) -> String {
    render_xlsx_row_with_prefix(row_number, row_span, cells, "")
}

pub(crate) fn render_xlsx_row_with_prefix(
    row_number: u32,
    row_span: Option<&XlsxRowSpan>,
    cells: BTreeMap<u32, String>,
    prefix: &str,
) -> String {
    let mut attrs = row_span.map(|span| span.attrs.clone()).unwrap_or_default();
    attrs.insert("r".to_string(), row_number.to_string());
    attrs.remove("spans");
    let mut out = format!("<{prefix}row{}>", render_xml_attrs(&attrs));
    for cell_xml in cells.into_values() {
        out.push_str(&cell_xml);
    }
    out.push_str(&format!("</{prefix}row>"));
    out
}

pub(crate) fn rebuild_xlsx_sheet_data(
    xml: &str,
    sheet_data: Option<&XlsxSheetDataSpan>,
    row_spans: &BTreeMap<u32, XlsxRowSpan>,
    changed_rows: &BTreeMap<u32, String>,
) -> CliResult<String> {
    if changed_rows.is_empty() {
        return Ok(xml.to_string());
    }
    let new_sheet_data = if let Some(sheet_data) = sheet_data.filter(|span| !span.empty) {
        let mut out = String::new();
        out.push_str(&xml[sheet_data.start..sheet_data.open_end]);
        let mut last = sheet_data.open_end;
        let mut emitted = BTreeSet::new();
        let mut rows_by_start = row_spans.values().collect::<Vec<_>>();
        rows_by_start.sort_by_key(|span| span.start);
        for row_span in rows_by_start {
            for (row, row_xml) in changed_rows.range(..row_span.row) {
                if !row_spans.contains_key(row) && emitted.insert(*row) {
                    out.push_str(row_xml);
                }
            }
            out.push_str(&xml[last..row_span.start]);
            if let Some(row_xml) = changed_rows.get(&row_span.row) {
                out.push_str(row_xml);
                emitted.insert(row_span.row);
            } else {
                out.push_str(&xml[row_span.start..row_span.end]);
            }
            last = row_span.end;
        }
        out.push_str(&xml[last..sheet_data.close_start]);
        for (row, row_xml) in changed_rows {
            if emitted.insert(*row) {
                out.push_str(row_xml);
            }
        }
        out.push_str(&xml[sheet_data.close_start..sheet_data.end]);
        out
    } else {
        let mut out = String::from("<sheetData>");
        for row_xml in changed_rows.values() {
            out.push_str(row_xml);
        }
        out.push_str("</sheetData>");
        out
    };
    if let Some(sheet_data) = sheet_data {
        let mut updated = String::with_capacity(xml.len() + new_sheet_data.len());
        updated.push_str(&xml[..sheet_data.start]);
        updated.push_str(&new_sheet_data);
        updated.push_str(&xml[sheet_data.end..]);
        return Ok(updated);
    }
    let insert_at = xml
        .find("</worksheet>")
        .ok_or_else(|| CliError::unexpected("worksheet has no closing tag"))?;
    let mut updated = String::with_capacity(xml.len() + new_sheet_data.len());
    updated.push_str(&xml[..insert_at]);
    updated.push_str(&new_sheet_data);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
}

pub(crate) fn reject_xlsx_merged_cell_intersection(
    xml: &str,
    bounds: RangeBounds,
) -> CliResult<()> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "mergeCell" =>
            {
                if let Some(merge_ref) = attr(&e, "ref") {
                    let merged = parse_range(&merge_ref)?;
                    if ranges_intersect(bounds, merged) {
                        return Err(CliError::invalid_args(format!(
                            "range write intersects merged cells: {} intersects {}",
                            range_bounds_ref(bounds),
                            range_bounds_ref(merged)
                        )));
                    }
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn ranges_intersect(a: RangeBounds, b: RangeBounds) -> bool {
    a.min_col() <= b.max_col()
        && a.max_col() >= b.min_col()
        && a.min_row() <= b.max_row()
        && a.max_row() >= b.min_row()
}

pub(crate) fn range_bounds_ref(bounds: RangeBounds) -> String {
    let start = format!("{}{}", col_name(bounds.start_col), bounds.start_row);
    let end = format!("{}{}", col_name(bounds.end_col), bounds.end_row);
    if start == end {
        start
    } else {
        format!("{start}:{end}")
    }
}

pub(crate) fn xlsx_used_range_from_cell_refs(xml: &str) -> Option<String> {
    let mut min_row = u32::MAX;
    let mut max_row = 0;
    let mut min_col = u32::MAX;
    let mut max_col = 0;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "c" => {
                if let Some(addr) = attr(&e, "r")
                    && let Ok((col, row)) = parse_cell_ref(&addr)
                {
                    min_row = min_row.min(row);
                    max_row = max_row.max(row);
                    min_col = min_col.min(col);
                    max_col = max_col.max(col);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    if max_row == 0 {
        None
    } else {
        Some(format!(
            "{}{}:{}{}",
            col_name(min_col),
            min_row,
            col_name(max_col),
            max_row
        ))
    }
}

#[cfg(test)]
mod worksheet_root_tests {
    use super::{
        strip_legacy_xlsx_pivot_table_parts, xlsx_worksheet_root_bounds,
        xlsx_worksheet_root_bounds_permissive,
    };

    #[test]
    fn strict_root_parser_rejects_a_nested_worksheet() {
        let error = xlsx_worksheet_root_bounds("<root><worksheet/></root>")
            .err()
            .expect("strict callers reject a non-worksheet document root");
        assert!(error.message.contains("root"));
    }

    #[test]
    fn permissive_root_parser_preserves_nested_worksheet_discovery() {
        let bounds = xlsx_worksheet_root_bounds_permissive("<root><worksheet/></root>")
            .expect("legacy permissive callers find a nested worksheet");
        assert!(bounds.self_closing);
        assert_eq!(bounds.start, 6);
    }

    #[test]
    fn strips_only_legacy_pivot_table_parts_from_worksheet_children() {
        let xml = concat!(
            r#"<x:worksheet xmlns:x="urn:x" xmlns:r="urn:r">"#,
            r#"<x:sheetData/>"#,
            r#"<x:tableParts count="1"><x:tablePart r:id="rIdTable"/></x:tableParts>"#,
            r#"<x:pivotTableParts count="1"><x:pivotTablePart r:id="rIdPivot"/></x:pivotTableParts>"#,
            r#"<x:extLst/>"#,
            r#"</x:worksheet>"#,
        );

        let (stripped, count) =
            strip_legacy_xlsx_pivot_table_parts(xml).expect("strip legacy pivot child");

        assert_eq!(count, 1);
        assert!(!stripped.contains("pivotTableParts"));
        assert!(stripped.contains("<x:tableParts count=\"1\">"));
        assert!(stripped.contains("rIdTable"));
        assert!(stripped.contains("<x:extLst/>"));
    }

    #[test]
    fn strip_is_a_noop_for_valid_worksheet_xml() {
        let xml = r#"<worksheet><sheetData/><tableParts count="1"><tablePart r:id="rId1"/></tableParts></worksheet>"#;
        let (stripped, count) = strip_legacy_xlsx_pivot_table_parts(xml).expect("valid worksheet");

        assert_eq!(count, 0);
        assert_eq!(stripped, xml);
    }
}
