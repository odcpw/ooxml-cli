mod cells;
mod format;
mod ranges;
pub(crate) use cells::{
    XlsxCellsClearOptions, XlsxCellsSetBatchOptions, XlsxCellsSetOptions, xlsx_cells_clear,
    xlsx_cells_set, xlsx_cells_set_batch,
};
pub(crate) use format::{
    XlsxRangesSetFormatOptions, XlsxRangesSetStyleOptions, default_xlsx_styles_xml,
    xlsx_ranges_set_format, xlsx_ranges_set_style,
};
pub(crate) use ranges::{
    XlsxRangesSetOptions, parse_xlsx_matrix_cell, parse_xlsx_range_set_matrix,
    rectangularize_xlsx_matrix, resolve_xlsx_ranges_set_values, validate_xlsx_null_policy,
    xlsx_ranges_set,
};

use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, RangeBounds, WorkbookSheet, XlsxRangeExportOptions, col_name, command_arg,
    needs_xml_space_preserve, normalize_xl_target, parse_xlsx_row_spans, rebuild_xlsx_sheet_data,
    reject_xlsx_merged_cell_intersection, relationships, render_xlsx_row_with_prefix,
    render_xml_attrs, resolve_sheet, workbook_sheets, xlsx_range_export_with_options,
    xlsx_sheet_data_span, xlsx_sheet_selectors, xlsx_used_range_from_cell_refs, xml_escape,
    zip_text,
};
#[derive(Clone)]
pub(crate) struct XlsxMatrixCell {
    pub(crate) kind: String,
    pub(crate) value: String,
    pub(crate) formula: String,
    pub(crate) null: bool,
}

#[derive(Default)]
pub(crate) struct XlsxRangeSetStats {
    pub(crate) updated: usize,
    pub(crate) created: usize,
    pub(crate) cleared: usize,
    pub(crate) skipped: usize,
    pub(crate) formula_count: usize,
    pub(crate) formula_seen: bool,
    pub(crate) formula_invalidated: bool,
}

pub(crate) fn validate_xlsx_mutation_output_flags(
    out: Option<&str>,
    in_place: bool,
    backup: Option<&str>,
    dry_run: bool,
) -> CliResult<()> {
    let has_out = out.is_some_and(|value| !value.trim().is_empty());
    let has_backup = backup.is_some_and(|value| !value.trim().is_empty());
    if dry_run && (has_out || in_place) {
        return Err(CliError::invalid_args(
            "--dry-run cannot be combined with --out or --in-place",
        ));
    }
    if dry_run && has_backup {
        return Err(CliError::invalid_args(
            "--backup cannot be used with --dry-run",
        ));
    }
    if !dry_run && !has_out && !in_place {
        return Err(CliError::invalid_args(
            "must specify exactly one of --out, --in-place, or --dry-run",
        ));
    }
    if has_out && in_place {
        return Err(CliError::invalid_args(
            "cannot specify both --out and --in-place",
        ));
    }
    if has_backup && !in_place {
        return Err(CliError::invalid_args(
            "--backup can only be used with --in-place",
        ));
    }
    Ok(())
}

fn resolve_xlsx_sheet_context(
    file: &str,
    sheet_selector: &str,
) -> CliResult<(WorkbookSheet, String)> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    Ok((sheet, sheet_part))
}

pub(crate) fn set_xlsx_range_in_sheet_xml(
    xml: &str,
    bounds: RangeBounds,
    rows: &[Vec<XlsxMatrixCell>],
    null_policy: &str,
    overwrite_formulas: bool,
) -> CliResult<(String, XlsxRangeSetStats)> {
    reject_xlsx_merged_cell_intersection(xml, bounds)?;
    let sheet_data = xlsx_sheet_data_span(xml)?;
    let row_spans = parse_xlsx_row_spans(xml, sheet_data.as_ref())?;
    let prefix = find_xlsx_element_start(xml, "sheetData")
        .or_else(|| find_xlsx_element_start(xml, "worksheet"))
        .map(|(_, prefix)| prefix)
        .unwrap_or_default();

    let mut stats = XlsxRangeSetStats::default();
    let mut changed_rows = BTreeMap::<u32, String>::new();
    let write_bounds = bounds.normalized();
    for (row_offset, row) in rows.iter().enumerate() {
        let row_number = write_bounds.start_row + row_offset as u32;
        let existing_row = row_spans.get(&row_number);
        let mut rendered_cells = existing_row
            .map(|span| {
                span.cells
                    .iter()
                    .map(|(col, cell)| (*col, cell.xml.clone()))
                    .collect::<BTreeMap<u32, String>>()
            })
            .unwrap_or_default();
        let mut row_changed = false;
        for (col_offset, cell) in row.iter().enumerate() {
            let col_number = write_bounds.start_col + col_offset as u32;
            let addr = format!("{}{}", col_name(col_number), row_number);
            let existing_cell = existing_row.and_then(|span| span.cells.get(&col_number));
            if !overwrite_formulas
                && existing_cell.is_some_and(|span| span.has_formula)
                && xlsx_range_cell_touches_existing(cell, null_policy)
            {
                return Err(CliError::invalid_args(format!(
                    "range write would overwrite existing formula: {addr}"
                )));
            }
            if cell.null {
                match null_policy.trim().to_ascii_lowercase().as_str() {
                    "skip" => {
                        stats.skipped += 1;
                    }
                    "clear" => {
                        if let Some(existing_cell) = existing_cell {
                            stats.cleared += 1;
                            row_changed = true;
                            if existing_cell.has_formula {
                                stats.formula_invalidated = true;
                            }
                            if existing_cell
                                .attrs
                                .get("s")
                                .is_some_and(|value| !value.is_empty())
                            {
                                rendered_cells.insert(
                                    col_number,
                                    render_empty_xlsx_cell_with_attrs_prefixed(
                                        &addr,
                                        Some(&existing_cell.attrs),
                                        &prefix,
                                    ),
                                );
                            } else {
                                rendered_cells.remove(&col_number);
                            }
                        } else {
                            rendered_cells.remove(&col_number);
                        }
                    }
                    "empty-string" => {
                        let empty = XlsxMatrixCell {
                            kind: "string".to_string(),
                            value: String::new(),
                            formula: String::new(),
                            null: false,
                        };
                        let (rendered, wrote_formula) = render_xlsx_cell_with_attrs(
                            &addr,
                            &empty,
                            existing_cell.map(|span| &span.attrs),
                            &prefix,
                        )?;
                        rendered_cells.insert(col_number, rendered);
                        row_changed = true;
                        stats.updated += 1;
                        if existing_cell.is_none() {
                            stats.created += 1;
                        }
                        if wrote_formula {
                            stats.formula_count += 1;
                            stats.formula_seen = true;
                        }
                    }
                    _ => unreachable!("null policy validated earlier"),
                }
                continue;
            }
            let (rendered, wrote_formula) = render_xlsx_cell_with_attrs(
                &addr,
                cell,
                existing_cell.map(|span| &span.attrs),
                &prefix,
            )?;
            rendered_cells.insert(col_number, rendered);
            row_changed = true;
            if existing_cell.is_some_and(|span| span.has_formula) {
                stats.formula_invalidated = true;
            }
            if existing_cell.is_none() {
                stats.created += 1;
            }
            if wrote_formula {
                stats.formula_count += 1;
                stats.formula_seen = true;
            }
            stats.updated += 1;
        }
        if row_changed {
            changed_rows.insert(
                row_number,
                render_xlsx_row_with_prefix(row_number, existing_row, rendered_cells, &prefix),
            );
        }
    }
    let updated = rebuild_xlsx_sheet_data(xml, sheet_data.as_ref(), &row_spans, &changed_rows)?;
    let used_range = xlsx_used_range_from_cell_refs(&updated);
    Ok((
        replace_xlsx_dimension(&updated, used_range.as_deref()),
        stats,
    ))
}

fn xlsx_range_cell_touches_existing(cell: &XlsxMatrixCell, null_policy: &str) -> bool {
    !(cell.null && null_policy.trim().eq_ignore_ascii_case("skip"))
}

fn render_xlsx_cell_with_attrs(
    addr: &str,
    cell: &XlsxMatrixCell,
    attrs: Option<&BTreeMap<String, String>>,
    prefix: &str,
) -> CliResult<(String, bool)> {
    let mut attrs = attrs.cloned().unwrap_or_default();
    attrs.insert("r".to_string(), addr.to_string());
    attrs.remove("t");
    let (kind, value) = normalize_xlsx_write_cell(cell)?;
    let (content, wrote_formula) = match kind.as_str() {
        "string" => {
            attrs.insert("t".to_string(), "inlineStr".to_string());
            let space_attr = if needs_xml_space_preserve(&value) {
                " xml:space=\"preserve\""
            } else {
                ""
            };
            (
                format!(
                    "<{prefix}is><{prefix}t{space_attr}>{}</{prefix}t></{prefix}is>",
                    xml_escape(&value)
                ),
                false,
            )
        }
        "number" => (
            format!("<{prefix}v>{}</{prefix}v>", xml_escape(&value)),
            false,
        ),
        "bool" | "boolean" => {
            let value = match cell.value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => "1",
                _ => "0",
            };
            attrs.insert("t".to_string(), "b".to_string());
            (format!("<{prefix}v>{value}</{prefix}v>"), false)
        }
        "formula" => (
            format!("<{prefix}f>{}</{prefix}f>", xml_escape(&value)),
            true,
        ),
        _ => unreachable!("cell kind normalized earlier"),
    };
    Ok((
        format!(
            "<{prefix}c{}>{content}</{prefix}c>",
            render_xml_attrs(&attrs)
        ),
        wrote_formula,
    ))
}

fn normalize_xlsx_write_cell(cell: &XlsxMatrixCell) -> CliResult<(String, String)> {
    let kind = if !cell.formula.is_empty() {
        "formula".to_string()
    } else {
        cell.kind.trim().to_ascii_lowercase()
    };
    match kind.as_str() {
        "" | "string" => Ok(("string".to_string(), cell.value.clone())),
        "number" => {
            let literal = cell.value.trim();
            let parsed = literal.parse::<f64>().map_err(|_| {
                CliError::invalid_args(format!("invalid number value {:?}", cell.value))
            })?;
            if !parsed.is_finite() || literal.is_empty() {
                return Err(CliError::invalid_args(format!(
                    "invalid number value {:?}",
                    cell.value
                )));
            }
            Ok(("number".to_string(), literal.to_string()))
        }
        "bool" | "boolean" => match cell.value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(("bool".to_string(), "1".to_string())),
            "false" | "0" => Ok(("bool".to_string(), "0".to_string())),
            _ => Err(CliError::invalid_args(format!(
                "invalid bool value {:?}",
                cell.value
            ))),
        },
        "formula" => {
            let formula = if cell.formula.is_empty() {
                &cell.value
            } else {
                &cell.formula
            };
            let formula = formula.trim().trim_start_matches('=').to_string();
            if formula.is_empty() {
                return Err(CliError::invalid_args("formula value cannot be empty"));
            }
            Ok(("formula".to_string(), formula))
        }
        "auto" => {
            let trimmed = cell.value.trim();
            if trimmed.starts_with('=') {
                return normalize_xlsx_write_cell(&XlsxMatrixCell {
                    kind: "formula".to_string(),
                    value: trimmed.to_string(),
                    formula: trimmed.to_string(),
                    null: false,
                });
            }
            if matches!(trimmed.to_ascii_lowercase().as_str(), "true" | "false") {
                return normalize_xlsx_write_cell(&XlsxMatrixCell {
                    kind: "bool".to_string(),
                    value: trimmed.to_string(),
                    formula: String::new(),
                    null: false,
                });
            }
            if let Ok(parsed) = trimmed.parse::<f64>()
                && parsed.is_finite()
            {
                return normalize_xlsx_write_cell(&XlsxMatrixCell {
                    kind: "number".to_string(),
                    value: trimmed.to_string(),
                    formula: String::new(),
                    null: false,
                });
            }
            Ok(("string".to_string(), cell.value.clone()))
        }
        _ => Err(CliError::invalid_args(format!(
            "invalid cell value type {:?} (must be string, number, bool, formula, or auto)",
            cell.kind
        ))),
    }
}

fn render_empty_xlsx_cell_with_attrs(
    addr: &str,
    attrs: Option<&BTreeMap<String, String>>,
) -> String {
    render_empty_xlsx_cell_with_attrs_prefixed(addr, attrs, "")
}

fn render_empty_xlsx_cell_with_attrs_prefixed(
    addr: &str,
    attrs: Option<&BTreeMap<String, String>>,
    prefix: &str,
) -> String {
    let mut attrs = attrs.cloned().unwrap_or_default();
    attrs.insert("r".to_string(), addr.to_string());
    attrs.remove("t");
    format!("<{prefix}c{}/>", render_xml_attrs(&attrs))
}

fn replace_xlsx_dimension(xml: &str, range: Option<&str>) -> String {
    let existing_dimension = find_xlsx_element_start(xml, "dimension");
    if let Some((start, prefix)) = existing_dimension
        && let Some(end) = xml[start..]
            .find("/>")
            .map(|offset| start + offset + "/>".len())
            .or_else(|| xml[start..].find('>').map(|offset| start + offset + 1))
    {
        let dimension = range.map(|range| format!("<{prefix}dimension ref=\"{range}\"/>"));
        let mut updated =
            String::with_capacity(xml.len() + dimension.as_ref().map_or(0, String::len));
        updated.push_str(&xml[..start]);
        if let Some(dimension) = dimension.as_deref() {
            updated.push_str(dimension);
        }
        updated.push_str(&xml[end..]);
        return updated;
    }
    if let Some(range) = range
        && let Some((sheet_data_start, prefix)) = find_xlsx_element_start(xml, "sheetData")
    {
        let dimension = format!("<{prefix}dimension ref=\"{range}\"/>");
        let mut updated = String::with_capacity(xml.len() + dimension.len());
        updated.push_str(&xml[..sheet_data_start]);
        updated.push_str(&dimension);
        updated.push_str(&xml[sheet_data_start..]);
        return updated;
    }
    xml.to_string()
}

fn find_xlsx_element_start(xml: &str, local_name: &str) -> Option<(usize, String)> {
    let mut cursor = 0usize;
    while let Some(offset) = xml[cursor..].find('<') {
        let start = cursor + offset;
        let after = &xml[start + 1..];
        let first = after.as_bytes().first().copied()?;
        if matches!(first, b'/' | b'?' | b'!') {
            cursor = start + 1;
            continue;
        }
        let name_end = after
            .find(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '>' | '/'))
            .unwrap_or(after.len());
        let qualified_name = &after[..name_end];
        if qualified_name == local_name {
            return Some((start, String::new()));
        }
        if let Some(prefix) = qualified_name.strip_suffix(local_name)
            && prefix.ends_with(':')
        {
            return Some((start, prefix.to_string()));
        }
        cursor = start + 1;
    }
    None
}

pub(crate) fn xlsx_range_destination_json(
    readback_file: &str,
    destination_file: Option<&str>,
    sheet: &WorkbookSheet,
    sheet_part: &str,
    range: &str,
) -> CliResult<Value> {
    xlsx_range_destination_json_with_max(
        readback_file,
        destination_file,
        sheet,
        sheet_part,
        range,
        0,
    )
}

pub(crate) fn xlsx_range_destination_json_with_max(
    readback_file: &str,
    destination_file: Option<&str>,
    sheet: &WorkbookSheet,
    sheet_part: &str,
    range: &str,
    readback_max_cells: i64,
) -> CliResult<Value> {
    let exported = xlsx_range_export_with_options(
        readback_file,
        &sheet.name,
        range,
        XlsxRangeExportOptions {
            include_types: true,
            include_formulas: true,
            include_formats: true,
            data_out: None,
            max_cells: 0,
        },
    )?;
    let exported = truncate_xlsx_destination_export(exported, readback_max_cells);
    let mut destination = Map::new();
    if let Some(file) = destination_file {
        destination.insert("file".to_string(), json!(file));
    }
    destination.insert("sheet".to_string(), json!(sheet.name));
    destination.insert("sheetNumber".to_string(), json!(sheet.position));
    destination.insert(
        "sheetPrimarySelector".to_string(),
        json!(format!("sheetId:{}", sheet.sheet_id)),
    );
    destination.insert(
        "sheetSelectors".to_string(),
        json!(xlsx_sheet_selectors(
            &sheet.name,
            sheet.sheet_id,
            sheet.position,
            &sheet.rel_id,
            &format!("/{sheet_part}")
        )),
    );
    for key in [
        "range",
        "rows",
        "cols",
        "values",
        "types",
        "formulas",
        "styleIndexes",
        "numberFormatIds",
        "numberFormatCodes",
        "formulaCount",
        "truncated",
    ] {
        if let Some(value) = exported.get(key) {
            destination.insert(key.to_string(), value.clone());
        }
    }
    Ok(Value::Object(destination))
}

fn truncate_xlsx_destination_export(mut exported: Value, max_cells: i64) -> Value {
    if max_cells <= 0 {
        return exported;
    }
    let Some(object) = exported.as_object_mut() else {
        return exported;
    };
    let rows = object.get("rows").and_then(Value::as_u64).unwrap_or(0);
    let cols = object.get("cols").and_then(Value::as_u64).unwrap_or(0);
    let total_cells = rows.saturating_mul(cols);
    if total_cells <= max_cells as u64 {
        return exported;
    }
    for key in [
        "values",
        "types",
        "formulas",
        "styleIndexes",
        "numberFormatIds",
        "numberFormatCodes",
    ] {
        if let Some(value) = object.get_mut(key) {
            truncate_xlsx_matrix_value(value, max_cells as usize);
        }
    }
    if let Some(formulas) = object.get("formulas") {
        object.insert(
            "formulaCount".to_string(),
            json!(count_non_null_xlsx_matrix_cells(formulas)),
        );
    }
    object.insert("truncated".to_string(), json!(true));
    exported
}

fn truncate_xlsx_matrix_value(value: &mut Value, max_cells: usize) {
    let Some(rows) = value.as_array_mut() else {
        return;
    };
    let mut remaining = max_cells;
    let mut keep_rows = 0usize;
    for row in rows.iter_mut() {
        if remaining == 0 {
            break;
        }
        let Some(cells) = row.as_array_mut() else {
            keep_rows += 1;
            continue;
        };
        if cells.len() > remaining {
            cells.truncate(remaining);
            remaining = 0;
        } else {
            remaining -= cells.len();
        }
        keep_rows += 1;
    }
    rows.truncate(keep_rows);
}

fn count_non_null_xlsx_matrix_cells(value: &Value) -> usize {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_array)
        .flatten()
        .filter(|cell| !cell.is_null())
        .count()
}

pub(crate) fn add_xlsx_range_mutation_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    sheet_selector: &str,
    range: &str,
) {
    let target = output_path.unwrap_or("<out.xlsx>");
    let validate_key = if output_path.is_some() {
        "validateCommand"
    } else {
        "validateCommandTemplate"
    };
    let cells_key = if output_path.is_some() {
        "cellsExtractCommand"
    } else {
        "cellsExtractCommandTemplate"
    };
    let ranges_key = if output_path.is_some() {
        "rangesExportCommand"
    } else {
        "rangesExportCommandTemplate"
    };
    let sheet_key = if output_path.is_some() {
        "sheetShowCommand"
    } else {
        "sheetShowCommandTemplate"
    };
    result.insert(
        validate_key.to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(target))),
    );
    result.insert(
        cells_key.to_string(),
        json!(format!(
            "ooxml --json xlsx cells extract {} --sheet {} --range {} --include-empty",
            command_arg(target),
            command_arg(sheet_selector),
            command_arg(range)
        )),
    );
    result.insert(
        ranges_key.to_string(),
        json!(format!(
            "ooxml --json xlsx ranges export {} --sheet {} --range {} --include-types --include-formulas --include-formats",
            command_arg(target),
            command_arg(sheet_selector),
            command_arg(range)
        )),
    );
    result.insert(
        sheet_key.to_string(),
        json!(format!(
            "ooxml --json xlsx sheets show {} --sheet {}",
            command_arg(target),
            command_arg(sheet_selector)
        )),
    );
}
