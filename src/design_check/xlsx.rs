use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use super::{DesignConfig, DesignFinding, finding, fixed_output_path, location};
use crate::{
    CliError, CliResult, attr, col_name, command_arg, local_name, normalize_xl_target,
    relationship_entries, shared_strings, sheet_cells, sorted_xlsx_cells, workbook_sheets,
    xlsx_charts_list, xlsx_styles, xlsx_tables, zip_text,
};

const DEFAULT_COLUMN_WIDTH: f64 = 8.43;

#[derive(Default)]
struct WorksheetPresentation {
    column_widths: BTreeMap<u32, f64>,
    frozen_rows: u32,
}

#[derive(Default)]
struct WorkbookStylePresentation {
    cell_fonts: Vec<u32>,
    font_names: Vec<String>,
}

pub(super) fn analyze(
    file: &str,
    _entries: &[String],
    config: &DesignConfig,
) -> CliResult<Vec<DesignFinding>> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let workbook_relationships = relationship_entries(file, "xl/_rels/workbook.xml.rels")?;
    let strings = shared_strings(file)?;
    let styles = xlsx_styles(file)?;
    let style_presentation = match zip_text(file, "xl/styles.xml") {
        Ok(xml) => scan_styles(&xml)?,
        Err(_) => WorkbookStylePresentation::default(),
    };
    let tables = xlsx_tables(file, None)?;
    let out = fixed_output_path(file, "design-fixed");
    let mut findings = Vec::new();

    for sheet in &sheets {
        let Some(relationship) = workbook_relationships
            .iter()
            .find(|relationship| relationship.id == sheet.rel_id)
        else {
            continue;
        };
        let part = normalize_xl_target(&relationship.target);
        let xml = zip_text(file, &part)?;
        let presentation = scan_worksheet_presentation(&xml)?;
        let cells = sorted_xlsx_cells(&sheet_cells(&xml, &strings, &styles), None);
        let max_row = cells.iter().map(|cell| cell.row).max().unwrap_or(0);

        let average_character_width = config.threshold("xlsx.averageCharacterWidth", 1.0);
        let padding = config.threshold("xlsx.columnPaddingCharacters", 1.0);
        let mut clipped_columns = BTreeSet::new();
        for cell in &cells {
            if !is_numeric_kind(&cell.value.kind) || cell.value.display_value.is_empty() {
                continue;
            }
            let width = presentation
                .column_widths
                .get(&cell.col)
                .copied()
                .unwrap_or(DEFAULT_COLUMN_WIDTH);
            let estimated =
                estimated_display_width(&cell.value.display_value, average_character_width);
            if estimated + padding > width {
                clipped_columns.insert(cell.col);
            }
        }
        for column in clipped_columns {
            let name = col_name(column);
            findings.push(finding(
                "XLSX_NUMBER_CLIPPED",
                format!("Column {name} is too narrow to display at least one numeric value"),
                sheet_location(sheet.name.as_str(), &part, Some(&name)),
                format!(
                    "ooxml --json xlsx colwidths autofit {} --sheet {} --range {} --out {}",
                    command_arg(file),
                    command_arg(&sheet.name),
                    name,
                    command_arg(&out)
                ),
                Some(json!({
                    "measurement": "average-character-width",
                    "averageCharacterWidth": average_character_width,
                })),
            ));
        }

        let long_sheet_rows = config.threshold("xlsx.freezeHeaderMinimumRows", 30.0) as u32;
        if max_row > long_sheet_rows && presentation.frozen_rows == 0 {
            findings.push(finding(
                "XLSX_HEADER_NOT_FROZEN",
                format!(
                    "Sheet {} has {max_row} populated rows but its header is not frozen",
                    sheet.name
                ),
                sheet_location(&sheet.name, &part, None),
                format!(
                    "ooxml --json xlsx freeze set {} --sheet {} --rows 1 --out {}",
                    command_arg(file),
                    command_arg(&sheet.name),
                    command_arg(&out)
                ),
                Some(json!({"rows": max_row})),
            ));
        }

        let mut formats_by_column = BTreeMap::<u32, BTreeSet<String>>::new();
        let mut fonts = BTreeSet::<(u32, String)>::new();
        for cell in &cells {
            if is_numeric_kind(&cell.value.kind) {
                formats_by_column.entry(cell.col).or_default().insert(
                    cell.value
                        .number_format_code
                        .clone()
                        .unwrap_or_else(|| "General".to_string()),
                );
            }
            let style_index = cell.value.style_index.unwrap_or(0) as usize;
            let font_id = style_presentation
                .cell_fonts
                .get(style_index)
                .copied()
                .unwrap_or(0);
            let font_name = style_presentation
                .font_names
                .get(font_id as usize)
                .cloned()
                .unwrap_or_else(|| format!("fontId:{font_id}"));
            fonts.insert((font_id, font_name));
        }
        for (column, formats) in formats_by_column {
            if formats.len() > 1 {
                let name = col_name(column);
                findings.push(finding(
                    "XLSX_INCONSISTENT_NUMBER_FORMAT",
                    format!("Column {name} uses {} number formats", formats.len()),
                    sheet_location(&sheet.name, &part, Some(&name)),
                    format!(
                        "ooxml --json xlsx ranges set-format {} --sheet {} --range {}1:{}{} --preset general --out {}",
                        command_arg(file), command_arg(&sheet.name), name, name, max_row.max(1), command_arg(&out)
                    ),
                    Some(json!({"formats": formats})),
                ));
            }
        }
        if fonts.len() > 1 {
            let used_range = format!(
                "A1:{}{}",
                col_name(cells.iter().map(|cell| cell.col).max().unwrap_or(1)),
                max_row.max(1)
            );
            findings.push(finding(
                "XLSX_MULTIPLE_FONTS",
                format!("Sheet {} uses {} font families", sheet.name, fonts.len()),
                sheet_location(&sheet.name, &part, None),
                format!(
                    "ooxml --json xlsx ranges set-style {} --sheet {} --range {} --font-name Aptos --out {}",
                    command_arg(file), command_arg(&sheet.name), command_arg(&used_range), command_arg(&out)
                ),
                Some(json!({"fonts": fonts.into_iter().map(|(_, name)| name).collect::<Vec<_>>() })),
            ));
        }

        let table_minimum_rows = config.threshold("xlsx.tableMinimumRows", 100.0) as u32;
        if max_row > table_minimum_rows && !tables.iter().any(|table| table.sheet == sheet.name) {
            let max_col = cells.iter().map(|cell| cell.col).max().unwrap_or(1);
            let range = format!("A1:{}{max_row}", col_name(max_col));
            findings.push(finding(
                "XLSX_MISSING_TABLE",
                format!("Sheet {} has a {max_row}-row tabular range but no table", sheet.name),
                sheet_location(&sheet.name, &part, None),
                format!(
                    "ooxml --json xlsx tables create {} --sheet {} --range {} --table DataTable --out {}",
                    command_arg(file), command_arg(&sheet.name), command_arg(&range), command_arg(&out)
                ),
                Some(json!({"range": range, "rows": max_row})),
            ));
        }

        let chart_report = xlsx_charts_list(file, Some(&sheet.name))?;
        for chart in chart_report["charts"].as_array().into_iter().flatten() {
            if chart
                .get("title")
                .and_then(Value::as_str)
                .is_some_and(|title| !title.trim().is_empty())
            {
                continue;
            }
            let selector = chart["primarySelector"].as_str().unwrap_or("#1");
            findings.push(finding(
                "XLSX_CHART_MISSING_TITLE",
                format!("Chart {selector} on {} has no title", sheet.name),
                location(&[
                    ("sheet", json!(sheet.name)),
                    ("part", chart["partUri"].clone()),
                    ("chart", json!(selector)),
                ]),
                format!(
                    "ooxml --json xlsx charts set-title {} --sheet {} --chart {} --title Chart --out {}",
                    command_arg(file), command_arg(&sheet.name), command_arg(selector), command_arg(&out)
                ),
                None,
            ));
        }
    }

    let visible_tabs = sheets
        .iter()
        .filter(|sheet| sheet.state == "visible")
        .count();
    let maximum_tabs = (config.threshold("xlsx.maximumReadableTabs", 12.0) as usize).max(1);
    if visible_tabs > maximum_tabs {
        for sheet in sheets
            .iter()
            .filter(|sheet| sheet.state == "visible")
            .skip(maximum_tabs)
        {
            findings.push(finding(
                "XLSX_UNREADABLE_TAB_COUNT",
                format!(
                    "Workbook has {visible_tabs} visible tabs; remove or consolidate excess tab {}",
                    sheet.name
                ),
                location(&[
                    ("part", json!("/xl/workbook.xml")),
                    ("sheet", json!(sheet.name)),
                ]),
                format!(
                    "ooxml --json xlsx sheets delete {} --sheet {} --out {}",
                    command_arg(file),
                    command_arg(&sheet.name),
                    command_arg(&out)
                ),
                Some(json!({"visibleTabs": visible_tabs, "maximum": maximum_tabs})),
            ));
        }
    }

    Ok(findings)
}

fn sheet_location(sheet: &str, part: &str, column: Option<&str>) -> Value {
    let mut fields = vec![("sheet", json!(sheet)), ("part", json!(format!("/{part}")))];
    if let Some(column) = column {
        fields.push(("column", json!(column)));
    }
    location(&fields)
}

fn is_numeric_kind(kind: &str) -> bool {
    matches!(kind, "number" | "date")
}

fn estimated_display_width(value: &str, average_character_width: f64) -> f64 {
    value
        .chars()
        .map(|character| {
            if matches!(character, '1' | 'i' | 'l' | '.' | ',' | ':' | ' ') {
                average_character_width * 0.55
            } else if character.is_ascii_digit() {
                average_character_width
            } else {
                average_character_width * 1.1
            }
        })
        .sum()
}

fn scan_worksheet_presentation(xml: &str) -> CliResult<WorksheetPresentation> {
    let mut reader = Reader::from_str(xml);
    let mut presentation = WorksheetPresentation::default();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                match local_name(element.name().as_ref()) {
                    "col" => {
                        let min = attr(&element, "min")
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or(1);
                        let max = attr(&element, "max")
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or(min);
                        if let Some(width) =
                            attr(&element, "width").and_then(|value| value.parse::<f64>().ok())
                        {
                            for column in min..=max.min(16_384) {
                                presentation.column_widths.insert(column, width);
                            }
                        }
                    }
                    "pane" if attr(&element, "state").as_deref() == Some("frozen") => {
                        presentation.frozen_rows = attr(&element, "ySplit")
                            .and_then(|value| value.parse::<f64>().ok())
                            .unwrap_or(0.0)
                            as u32;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse worksheet presentation: {error}"
                )));
            }
            _ => {}
        }
    }
    Ok(presentation)
}

fn scan_styles(xml: &str) -> CliResult<WorkbookStylePresentation> {
    let mut reader = Reader::from_str(xml);
    let mut result = WorkbookStylePresentation::default();
    let mut section = String::new();
    let mut current_font = None::<String>;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if matches!(name.as_str(), "fonts" | "cellXfs") {
                    section.clone_from(&name);
                } else if section == "fonts" && name == "font" {
                    current_font = Some(String::new());
                } else if section == "fonts"
                    && name == "name"
                    && let Some(value) = attr(&element, "val")
                    && let Some(font) = current_font.as_mut()
                {
                    *font = value;
                } else if section == "cellXfs" && name == "xf" {
                    result.cell_fonts.push(
                        attr(&element, "fontId")
                            .and_then(|value| value.parse().ok())
                            .unwrap_or(0),
                    );
                }
            }
            Ok(Event::Empty(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if section == "fonts"
                    && name == "name"
                    && let Some(value) = attr(&element, "val")
                    && let Some(font) = current_font.as_mut()
                {
                    *font = value;
                } else if section == "cellXfs" && name == "xf" {
                    result.cell_fonts.push(
                        attr(&element, "fontId")
                            .and_then(|value| value.parse().ok())
                            .unwrap_or(0),
                    );
                }
            }
            Ok(Event::End(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if name == "font" && section == "fonts" {
                    result
                        .font_names
                        .push(current_font.take().unwrap_or_default());
                } else if name == section {
                    section.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse XLSX styles: {error}"
                )));
            }
            _ => {}
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_character_metric_and_sheet_presentation_are_deterministic() {
        assert!(
            estimated_display_width("123456789", 1.0) > estimated_display_width("111111111", 1.0)
        );
        let presentation = scan_worksheet_presentation(
            r#"<worksheet><cols><col min="2" max="3" width="4.5"/></cols><sheetViews><sheetView><pane ySplit="1" state="frozen"/></sheetView></sheetViews></worksheet>"#,
        ).unwrap();
        assert_eq!(presentation.column_widths[&2], 4.5);
        assert_eq!(presentation.column_widths[&3], 4.5);
        assert_eq!(presentation.frozen_rows, 1);
    }

    #[test]
    fn style_scanner_maps_cell_formats_to_named_fonts() {
        let styles = scan_styles(r#"<styleSheet><fonts><font><name val="Aptos"/></font><font><name val="Arial"/></font></fonts><cellXfs><xf fontId="0"/><xf fontId="1"/></cellXfs></styleSheet>"#).unwrap();
        assert_eq!(styles.font_names, ["Aptos", "Arial"]);
        assert_eq!(styles.cell_fonts, [0, 1]);
    }
}
