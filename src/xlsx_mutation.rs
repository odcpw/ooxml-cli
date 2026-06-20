mod cells;
mod format;
mod ranges;
pub(crate) use cells::{XlsxCellsSetOptions, xlsx_cells_set};
pub(crate) use format::{XlsxRangesSetFormatOptions, xlsx_ranges_set_format};
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
    reject_xlsx_merged_cell_intersection, relationships, render_xlsx_row, render_xml_attrs,
    resolve_sheet, workbook_sheets, xlsx_range_export_with_options, xlsx_sheet_data_span,
    xlsx_sheet_selectors, xlsx_used_range_from_cell_refs, xml_escape, zip_text,
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
                                    render_empty_xlsx_cell_with_attrs(
                                        &addr,
                                        Some(&existing_cell.attrs),
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
            let (rendered, wrote_formula) =
                render_xlsx_cell_with_attrs(&addr, cell, existing_cell.map(|span| &span.attrs))?;
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
                render_xlsx_row(row_number, existing_row, rendered_cells),
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
                format!("<is><t{space_attr}>{}</t></is>", xml_escape(&value)),
                false,
            )
        }
        "number" => (format!("<v>{}</v>", xml_escape(&value)), false),
        "bool" | "boolean" => {
            let value = match cell.value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => "1",
                _ => "0",
            };
            attrs.insert("t".to_string(), "b".to_string());
            (format!("<v>{value}</v>"), false)
        }
        "formula" => (format!("<f>{}</f>", xml_escape(&value)), true),
        _ => unreachable!("cell kind normalized earlier"),
    };
    Ok((
        format!("<c{}>{content}</c>", render_xml_attrs(&attrs)),
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
    let mut attrs = attrs.cloned().unwrap_or_default();
    attrs.insert("r".to_string(), addr.to_string());
    attrs.remove("t");
    format!("<c{}/>", render_xml_attrs(&attrs))
}

fn replace_xlsx_dimension(xml: &str, range: Option<&str>) -> String {
    let dimension = range.map(|range| format!("<dimension ref=\"{range}\"/>"));
    if let Some(start) = xml.find("<dimension")
        && let Some(end) = xml[start..]
            .find("/>")
            .map(|offset| start + offset + "/>".len())
            .or_else(|| xml[start..].find('>').map(|offset| start + offset + 1))
    {
        let mut updated =
            String::with_capacity(xml.len() + dimension.as_ref().map_or(0, String::len));
        updated.push_str(&xml[..start]);
        if let Some(dimension) = dimension.as_deref() {
            updated.push_str(dimension);
        }
        updated.push_str(&xml[end..]);
        return updated;
    }
    if let Some(dimension) = dimension
        && let Some(sheet_data_start) = xml.find("<sheetData")
    {
        let mut updated = String::with_capacity(xml.len() + dimension.len());
        updated.push_str(&xml[..sheet_data_start]);
        updated.push_str(&dimension);
        updated.push_str(&xml[sheet_data_start..]);
        return updated;
    }
    xml.to_string()
}

pub(crate) fn xlsx_range_destination_json(
    readback_file: &str,
    destination_file: Option<&str>,
    sheet: &WorkbookSheet,
    sheet_part: &str,
    range: &str,
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
}
