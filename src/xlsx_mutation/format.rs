mod num_formats_xml;
mod number_format;
mod style;
mod style_xfs;
mod styles_part;
mod styles_xml;

pub(crate) use style::{XlsxRangesSetStyleOptions, xlsx_ranges_set_style};
pub(crate) use styles_part::default_xlsx_styles_xml;

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::{
    add_xlsx_range_mutation_commands, render_empty_xlsx_cell_with_attrs, replace_xlsx_dimension,
    resolve_xlsx_sheet_context, validate_xlsx_mutation_output_flags, xlsx_range_destination_json,
};
use crate::{
    CliError, CliResult, RangeBounds, XlsxCellSpan, check_range_max_cells, col_name,
    copy_zip_with_part_overrides, ensure_content_type_override, parse_cli_range,
    parse_xlsx_row_spans, range_bounds_ref, rebuild_xlsx_sheet_data, render_xlsx_row,
    render_xml_attrs, validate, xlsx_ranges_set_temp_path, xlsx_sheet_data_span,
    xlsx_used_range_from_cell_refs, zip_text,
};
use num_formats_xml::ensure_xlsx_number_format;
use number_format::resolve_xlsx_number_format;
use style_xfs::ensure_xlsx_cell_style;
use styles_part::resolve_or_add_xlsx_styles_part;

pub(crate) struct XlsxRangesSetFormatOptions<'a> {
    pub(crate) sheet: &'a str,
    pub(crate) range: &'a str,
    pub(crate) preset: Option<&'a str>,
    pub(crate) format_code: Option<&'a str>,
    pub(crate) decimals: i64,
    pub(crate) currency_symbol: Option<&'a str>,
    pub(crate) max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Default)]
struct XlsxRangeFormatStats {
    updated: usize,
    created: usize,
    created_styles: usize,
    style_indexes: BTreeSet<u32>,
}

pub(crate) fn xlsx_ranges_set_format(
    file: &str,
    options: XlsxRangesSetFormatOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let bounds = parse_cli_range(options.range)?;
    let range = range_bounds_ref(bounds);
    check_range_max_cells(&range, bounds, options.max_cells)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let spec = resolve_xlsx_number_format(
        options.preset,
        options.format_code,
        options.decimals,
        options.currency_symbol,
    )?;

    let (sheet, sheet_part) = resolve_xlsx_sheet_context(file, options.sheet)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (styles_part, rels_override) = resolve_or_add_xlsx_styles_part(file)?;
    let styles_xml = zip_text(file, &styles_part).unwrap_or_else(|_| default_xlsx_styles_xml());
    let (styles_xml, number_format_id) = ensure_xlsx_number_format(styles_xml, &spec)?;
    let (updated_sheet_xml, styles_xml, stats) =
        set_xlsx_range_number_format_xml(&sheet_xml, styles_xml, bounds, number_format_id)?;
    let content_types_xml = ensure_content_type_override(
        zip_text(file, "[Content_Types].xml")?,
        &format!("/{styles_part}"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
    );

    let output_path = options.out.filter(|value| !value.is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        xlsx_ranges_set_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };

    let mut overrides = BTreeMap::new();
    overrides.insert(sheet_part.clone(), updated_sheet_xml);
    overrides.insert(styles_part.clone(), styles_xml);
    overrides.insert("[Content_Types].xml".to_string(), content_types_xml);
    if let Some(rels_xml) = rels_override {
        overrides.insert("xl/_rels/workbook.xml.rels".to_string(), rels_xml);
    }
    copy_zip_with_part_overrides(file, &readback_path, &overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let destination =
        xlsx_range_destination_json(&readback_path, commit_path, &sheet, &sheet_part, &range)?;
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }

    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("range".to_string(), json!(range));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
    if !spec.preset.is_empty() {
        result.insert("preset".to_string(), json!(spec.preset));
    }
    result.insert("formatCode".to_string(), json!(spec.format_code));
    result.insert("numberFormatId".to_string(), json!(number_format_id));
    result.insert("builtin".to_string(), json!(spec.builtin));
    result.insert("updated".to_string(), json!(stats.updated));
    result.insert("created".to_string(), json!(stats.created));
    result.insert("createdStyles".to_string(), json!(stats.created_styles));
    if !stats.style_indexes.is_empty() {
        result.insert(
            "styleIndexes".to_string(),
            json!(stats.style_indexes.into_iter().collect::<Vec<_>>()),
        );
    }
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_range_mutation_commands(
        &mut result,
        commit_path,
        &format!("sheetId:{}", sheet.sheet_id),
        &range,
    );
    Ok(Value::Object(result))
}

fn set_xlsx_range_number_format_xml(
    sheet_xml: &str,
    mut styles_xml: String,
    bounds: RangeBounds,
    number_format_id: u32,
) -> CliResult<(String, String, XlsxRangeFormatStats)> {
    let sheet_data = xlsx_sheet_data_span(sheet_xml)?;
    let row_spans = parse_xlsx_row_spans(sheet_xml, sheet_data.as_ref())?;
    let mut stats = XlsxRangeFormatStats::default();
    let mut changed_rows = BTreeMap::<u32, String>::new();
    let mut style_by_base = BTreeMap::<u32, u32>::new();
    let write_bounds = bounds.normalized();
    for row_num in write_bounds.start_row..=write_bounds.end_row {
        let existing_row = row_spans.get(&row_num);
        let mut rendered_cells = existing_row
            .map(|span| {
                span.cells
                    .iter()
                    .map(|(col, cell)| (*col, cell.xml.clone()))
                    .collect::<BTreeMap<u32, String>>()
            })
            .unwrap_or_default();
        let mut row_changed = false;
        for col_num in write_bounds.start_col..=write_bounds.end_col {
            let addr = format!("{}{}", col_name(col_num), row_num);
            let existing_cell = existing_row.and_then(|span| span.cells.get(&col_num));
            let base_style = existing_cell
                .and_then(|cell| cell.attrs.get("s"))
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0);
            let style_index = if let Some(style_index) = style_by_base.get(&base_style).copied() {
                style_index
            } else {
                let (new_styles_xml, style_index, created) =
                    ensure_xlsx_cell_style(styles_xml, base_style, number_format_id)?;
                styles_xml = new_styles_xml;
                if created {
                    stats.created_styles += 1;
                }
                style_by_base.insert(base_style, style_index);
                style_index
            };
            let cell_xml = if let Some(existing_cell) = existing_cell {
                render_xlsx_existing_cell_with_style(&addr, existing_cell, style_index)
            } else {
                let mut attrs = BTreeMap::new();
                attrs.insert("r".to_string(), addr.clone());
                attrs.insert("s".to_string(), style_index.to_string());
                stats.created += 1;
                render_empty_xlsx_cell_with_attrs(&addr, Some(&attrs))
            };
            rendered_cells.insert(col_num, cell_xml);
            stats.updated += 1;
            stats.style_indexes.insert(style_index);
            row_changed = true;
        }
        if row_changed {
            changed_rows.insert(
                row_num,
                render_xlsx_row(row_num, existing_row, rendered_cells),
            );
        }
    }
    let updated =
        rebuild_xlsx_sheet_data(sheet_xml, sheet_data.as_ref(), &row_spans, &changed_rows)?;
    let used_range = xlsx_used_range_from_cell_refs(&updated);
    Ok((
        replace_xlsx_dimension(&updated, used_range.as_deref()),
        styles_xml,
        stats,
    ))
}

fn render_xlsx_existing_cell_with_style(
    addr: &str,
    cell: &XlsxCellSpan,
    style_index: u32,
) -> String {
    let mut attrs = cell.attrs.clone();
    attrs.insert("r".to_string(), addr.to_string());
    attrs.insert("s".to_string(), style_index.to_string());
    if cell.xml.trim_end().ends_with("/>") {
        return render_empty_xlsx_cell_with_attrs(addr, Some(&attrs));
    }
    if let Some(open_end) = cell.xml.find('>') {
        let mut out = format!("<c{}>", render_xml_attrs(&attrs));
        out.push_str(&cell.xml[open_end + 1..]);
        out
    } else {
        render_empty_xlsx_cell_with_attrs(addr, Some(&attrs))
    }
}
