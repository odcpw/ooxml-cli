use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::super::{
    add_xlsx_range_mutation_commands, render_empty_xlsx_cell_with_attrs, replace_xlsx_dimension,
    resolve_xlsx_sheet_context, validate_xlsx_mutation_output_flags, xlsx_range_destination_json,
};
use super::render_xlsx_existing_cell_with_style;
use super::styles_part::{default_xlsx_styles_xml, resolve_or_add_xlsx_styles_part};
use super::styles_xml::{
    element_span_by_local_name, ensure_xlsx_style_defaults, set_collection_count,
};
use crate::{
    CliError, CliResult, RangeBounds, check_range_max_cells, col_name,
    copy_zip_with_part_overrides, ensure_content_type_override, local_name, parse_cli_range,
    parse_xlsx_row_spans, range_bounds_ref, rebuild_xlsx_sheet_data, render_xlsx_row,
    render_xml_attrs, validate, xlsx_ranges_set_temp_path, xlsx_sheet_data_span,
    xlsx_used_range_from_cell_refs, xml_attrs, zip_text,
};

pub(crate) struct XlsxRangesSetStyleOptions<'a> {
    pub(crate) sheet: &'a str,
    pub(crate) range: &'a str,
    pub(crate) font_name: Option<&'a str>,
    pub(crate) font_size: Option<f64>,
    pub(crate) font_bold: Option<bool>,
    pub(crate) font_italic: Option<bool>,
    pub(crate) font_underline: Option<bool>,
    pub(crate) font_color: Option<&'a str>,
    pub(crate) fill_color: Option<&'a str>,
    pub(crate) border_style: Option<&'a str>,
    pub(crate) border_color: Option<&'a str>,
    pub(crate) border_top: Option<bool>,
    pub(crate) border_bottom: Option<bool>,
    pub(crate) border_left: Option<bool>,
    pub(crate) border_right: Option<bool>,
    pub(crate) alignment_horizontal: Option<&'a str>,
    pub(crate) alignment_vertical: Option<&'a str>,
    pub(crate) alignment_wrap_text: Option<bool>,
    pub(crate) max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
struct FontStyleSpec {
    name: Option<String>,
    size: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    color: Option<String>,
}

#[derive(Clone)]
struct FillStyleSpec {
    color: String,
}

#[derive(Clone)]
struct BorderStyleSpec {
    style: String,
    color: Option<String>,
    top: bool,
    bottom: bool,
    left: bool,
    right: bool,
}

#[derive(Clone)]
struct AlignmentStyleSpec {
    horizontal: Option<String>,
    vertical: Option<String>,
    wrap_text: Option<bool>,
}

#[derive(Clone)]
struct CellStyleSpec {
    font: Option<FontStyleSpec>,
    fill: Option<FillStyleSpec>,
    border: Option<BorderStyleSpec>,
    alignment: Option<AlignmentStyleSpec>,
}

#[derive(Clone)]
struct StyleElement {
    name: String,
    attrs: BTreeMap<String, String>,
    inner_xml: String,
}

#[derive(Default)]
struct XlsxRangeStyleStats {
    updated: usize,
    created: usize,
    created_styles: usize,
    style_indexes: BTreeSet<u32>,
}

pub(crate) fn xlsx_ranges_set_style(
    file: &str,
    options: XlsxRangesSetStyleOptions<'_>,
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
    let spec = build_xlsx_cell_style_spec(&options)?;

    let (sheet, sheet_part) = resolve_xlsx_sheet_context(file, options.sheet)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (styles_part, rels_override) = resolve_or_add_xlsx_styles_part(file)?;
    let styles_xml = zip_text(file, &styles_part).unwrap_or_else(|_| default_xlsx_styles_xml());
    let (updated_sheet_xml, styles_xml, stats) =
        set_xlsx_range_style_xml(&sheet_xml, styles_xml, bounds, &spec)?;
    let content_types_xml = ensure_content_type_override(
        zip_text(file, "[Content_Types].xml")?,
        &format!("/{styles_part}"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
    )?;

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
    result.insert("range".to_string(), json!(range.clone()));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
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

fn build_xlsx_cell_style_spec(options: &XlsxRangesSetStyleOptions<'_>) -> CliResult<CellStyleSpec> {
    let font = if options.font_name.is_some()
        || options.font_size.is_some()
        || options.font_bold.is_some()
        || options.font_italic.is_some()
        || options.font_underline.is_some()
        || options.font_color.is_some()
    {
        Some(FontStyleSpec {
            name: options.font_name.map(ToString::to_string),
            size: options.font_size,
            bold: options.font_bold,
            italic: options.font_italic,
            underline: options.font_underline,
            color: options.font_color.map(ToString::to_string),
        })
    } else {
        None
    };
    let fill = options.fill_color.map(|color| FillStyleSpec {
        color: color.to_string(),
    });
    let border = if options.border_style.is_some()
        || options.border_color.is_some()
        || options.border_top.is_some()
        || options.border_bottom.is_some()
        || options.border_left.is_some()
        || options.border_right.is_some()
    {
        let style = options.border_style.unwrap_or_default().to_string();
        if style.is_empty() {
            return Err(CliError::invalid_args(
                "--border-style is required when setting borders",
            ));
        }
        let any_edge = options.border_top.is_some()
            || options.border_bottom.is_some()
            || options.border_left.is_some()
            || options.border_right.is_some();
        Some(BorderStyleSpec {
            style,
            color: options.border_color.map(ToString::to_string),
            top: if any_edge {
                options.border_top.unwrap_or(false)
            } else {
                true
            },
            bottom: if any_edge {
                options.border_bottom.unwrap_or(false)
            } else {
                true
            },
            left: if any_edge {
                options.border_left.unwrap_or(false)
            } else {
                true
            },
            right: if any_edge {
                options.border_right.unwrap_or(false)
            } else {
                true
            },
        })
    } else {
        None
    };
    let alignment = if options.alignment_horizontal.is_some()
        || options.alignment_vertical.is_some()
        || options.alignment_wrap_text.is_some()
    {
        Some(AlignmentStyleSpec {
            horizontal: options.alignment_horizontal.map(ToString::to_string),
            vertical: options.alignment_vertical.map(ToString::to_string),
            wrap_text: options.alignment_wrap_text,
        })
    } else {
        None
    };
    if font.is_none() && fill.is_none() && border.is_none() && alignment.is_none() {
        return Err(CliError::invalid_args(
            "specify at least one style flag (font/fill/border/alignment)",
        ));
    }
    let spec = CellStyleSpec {
        font,
        fill,
        border,
        alignment,
    };
    validate_xlsx_cell_style_spec(&spec)?;
    Ok(spec)
}

fn validate_xlsx_cell_style_spec(spec: &CellStyleSpec) -> CliResult<()> {
    if let Some(font) = &spec.font {
        if let Some(size) = font.size
            && (!(1.0..=409.0).contains(&size) || !size.is_finite())
        {
            return Err(CliError::invalid_args(format!(
                "font size {size} out of range (1-409)"
            )));
        }
        if let Some(color) = &font.color {
            normalize_xlsx_style_color(color)?;
        }
    }
    if let Some(fill) = &spec.fill {
        normalize_xlsx_style_color(&fill.color)?;
    }
    if let Some(border) = &spec.border {
        if !valid_xlsx_border_style(&border.style) {
            return Err(CliError::invalid_args(format!(
                "invalid border style {:?}",
                border.style
            )));
        }
        if let Some(color) = &border.color {
            normalize_xlsx_style_color(color)?;
        }
    }
    if let Some(alignment) = &spec.alignment {
        if let Some(horizontal) = &alignment.horizontal
            && !valid_xlsx_horizontal_alignment(horizontal)
        {
            return Err(CliError::invalid_args(format!(
                "invalid horizontal alignment {:?}",
                horizontal
            )));
        }
        if let Some(vertical) = &alignment.vertical
            && !valid_xlsx_vertical_alignment(vertical)
        {
            return Err(CliError::invalid_args(format!(
                "invalid vertical alignment {:?}",
                vertical
            )));
        }
    }
    Ok(())
}

fn set_xlsx_range_style_xml(
    sheet_xml: &str,
    mut styles_xml: String,
    bounds: RangeBounds,
    spec: &CellStyleSpec,
) -> CliResult<(String, String, XlsxRangeStyleStats)> {
    let sheet_data = xlsx_sheet_data_span(sheet_xml)?;
    let row_spans = parse_xlsx_row_spans(sheet_xml, sheet_data.as_ref())?;
    let mut stats = XlsxRangeStyleStats::default();
    let mut changed_rows = BTreeMap::<u32, String>::new();
    let mut style_by_base = BTreeMap::<u32, u32>::new();
    let bounds = bounds.normalized();
    for row_num in bounds.start_row..=bounds.end_row {
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
        for col_num in bounds.start_col..=bounds.end_col {
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
                    apply_xlsx_style_to_styles_xml(styles_xml, base_style, spec)?;
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

fn apply_xlsx_style_to_styles_xml(
    styles_xml: String,
    base_style_index: u32,
    spec: &CellStyleSpec,
) -> CliResult<(String, u32, bool)> {
    let mut styles_xml = ensure_xlsx_style_defaults(styles_xml);
    let xfs = collection_entries(&styles_xml, "cellXfs", "xf")?;
    let base_index = if (base_style_index as usize) < xfs.len() {
        base_style_index
    } else {
        0
    };
    let mut candidate = xfs
        .get(base_index as usize)
        .cloned()
        .unwrap_or_else(default_xlsx_xf_entry);
    for (key, value) in [
        ("numFmtId", "0"),
        ("fontId", "0"),
        ("fillId", "0"),
        ("borderId", "0"),
        ("xfId", "0"),
    ] {
        candidate
            .attrs
            .entry(key.to_string())
            .or_insert_with(|| value.to_string());
    }
    if let Some(font) = &spec.font {
        let base_font_id = style_attr_u32(&candidate, "fontId");
        let (updated, font_id) = ensure_xlsx_style_font(styles_xml, base_font_id, font)?;
        styles_xml = updated;
        candidate
            .attrs
            .insert("fontId".to_string(), font_id.to_string());
        candidate
            .attrs
            .insert("applyFont".to_string(), "1".to_string());
    }
    if let Some(fill) = &spec.fill {
        let (updated, fill_id) = ensure_xlsx_style_fill(styles_xml, fill)?;
        styles_xml = updated;
        candidate
            .attrs
            .insert("fillId".to_string(), fill_id.to_string());
        candidate
            .attrs
            .insert("applyFill".to_string(), "1".to_string());
    }
    if let Some(border) = &spec.border {
        let base_border_id = style_attr_u32(&candidate, "borderId");
        let (updated, border_id) = ensure_xlsx_style_border(styles_xml, base_border_id, border)?;
        styles_xml = updated;
        candidate
            .attrs
            .insert("borderId".to_string(), border_id.to_string());
        candidate
            .attrs
            .insert("applyBorder".to_string(), "1".to_string());
    }
    if let Some(alignment) = &spec.alignment {
        apply_xlsx_style_alignment(&mut candidate, alignment);
        candidate
            .attrs
            .insert("applyAlignment".to_string(), "1".to_string());
    }
    let candidate_sig = render_style_element("xf", &candidate);
    for (idx, xf) in xfs.iter().enumerate() {
        if render_style_element("xf", xf) == candidate_sig {
            return Ok((styles_xml, idx as u32, false));
        }
    }
    let styles_xml = append_collection_child(styles_xml, "cellXfs", &candidate_sig)?;
    let styles_xml = set_collection_count(styles_xml, "cellXfs", "xf");
    Ok((styles_xml, xfs.len() as u32, true))
}

fn ensure_xlsx_style_font(
    styles_xml: String,
    base_font_id: u32,
    spec: &FontStyleSpec,
) -> CliResult<(String, u32)> {
    let fonts = collection_entries(&styles_xml, "fonts", "font")?;
    let base = fonts
        .get(base_font_id as usize)
        .cloned()
        .unwrap_or_else(|| StyleElement {
            name: "font".to_string(),
            attrs: BTreeMap::new(),
            inner_xml: String::new(),
        });
    let mut children = direct_style_children(&base.inner_xml)?;
    if let Some(value) = spec.bold {
        set_empty_style_child(&mut children, "b", value, font_child_order);
    }
    if let Some(value) = spec.italic {
        set_empty_style_child(&mut children, "i", value, font_child_order);
    }
    if let Some(value) = spec.underline {
        set_empty_style_child(&mut children, "u", value, font_child_order);
    }
    if let Some(size) = spec.size {
        set_val_style_child(&mut children, "sz", &size.to_string(), font_child_order);
    }
    if let Some(color) = &spec.color {
        let rgb = normalize_xlsx_style_color(color)?;
        let mut attrs = BTreeMap::new();
        attrs.insert("rgb".to_string(), rgb);
        set_style_child(
            &mut children,
            StyleElement {
                name: "color".to_string(),
                attrs,
                inner_xml: String::new(),
            },
            font_child_order,
        );
    }
    if let Some(name) = &spec.name {
        set_val_style_child(&mut children, "name", name, font_child_order);
    }
    let candidate = StyleElement {
        name: "font".to_string(),
        attrs: base.attrs,
        inner_xml: render_style_children(&children),
    };
    let candidate_sig = render_style_element("font", &candidate);
    for (idx, font) in fonts.iter().enumerate() {
        if render_style_element("font", font) == candidate_sig {
            return Ok((styles_xml, idx as u32));
        }
    }
    let styles_xml = append_collection_child(styles_xml, "fonts", &candidate_sig)?;
    let styles_xml = set_collection_count(styles_xml, "fonts", "font");
    Ok((styles_xml, fonts.len() as u32))
}

fn ensure_xlsx_style_fill(styles_xml: String, spec: &FillStyleSpec) -> CliResult<(String, u32)> {
    let fills = collection_entries(&styles_xml, "fills", "fill")?;
    let rgb = normalize_xlsx_style_color(&spec.color)?;
    let candidate = StyleElement {
        name: "fill".to_string(),
        attrs: BTreeMap::new(),
        inner_xml: format!(
            r#"<patternFill patternType="solid"><fgColor rgb="{rgb}"/><bgColor indexed="64"/></patternFill>"#
        ),
    };
    let candidate_sig = render_style_element("fill", &candidate);
    for (idx, fill) in fills.iter().enumerate() {
        if render_style_element("fill", fill) == candidate_sig {
            return Ok((styles_xml, idx as u32));
        }
    }
    let styles_xml = append_collection_child(styles_xml, "fills", &candidate_sig)?;
    let styles_xml = set_collection_count(styles_xml, "fills", "fill");
    Ok((styles_xml, fills.len() as u32))
}

fn ensure_xlsx_style_border(
    styles_xml: String,
    base_border_id: u32,
    spec: &BorderStyleSpec,
) -> CliResult<(String, u32)> {
    let borders = collection_entries(&styles_xml, "borders", "border")?;
    let base = borders
        .get(base_border_id as usize)
        .cloned()
        .unwrap_or_else(|| StyleElement {
            name: "border".to_string(),
            attrs: BTreeMap::new(),
            inner_xml: String::new(),
        });
    let mut children = direct_style_children(&base.inner_xml)?;
    let color = spec
        .color
        .as_deref()
        .map(normalize_xlsx_style_color)
        .transpose()?;
    for (edge, enabled) in [
        ("left", spec.left),
        ("right", spec.right),
        ("top", spec.top),
        ("bottom", spec.bottom),
    ] {
        if enabled {
            set_xlsx_border_edge(&mut children, edge, &spec.style, color.as_deref());
        }
    }
    let candidate = StyleElement {
        name: "border".to_string(),
        attrs: base.attrs,
        inner_xml: render_style_children(&children),
    };
    let candidate_sig = render_style_element("border", &candidate);
    for (idx, border) in borders.iter().enumerate() {
        if render_style_element("border", border) == candidate_sig {
            return Ok((styles_xml, idx as u32));
        }
    }
    let styles_xml = append_collection_child(styles_xml, "borders", &candidate_sig)?;
    let styles_xml = set_collection_count(styles_xml, "borders", "border");
    Ok((styles_xml, borders.len() as u32))
}

fn apply_xlsx_style_alignment(xf: &mut StyleElement, spec: &AlignmentStyleSpec) {
    let mut children = direct_style_children(&xf.inner_xml).unwrap_or_default();
    let mut alignment = children
        .iter()
        .find(|child| child.name == "alignment")
        .cloned()
        .unwrap_or_else(|| StyleElement {
            name: "alignment".to_string(),
            attrs: BTreeMap::new(),
            inner_xml: String::new(),
        });
    if let Some(horizontal) = &spec.horizontal {
        alignment
            .attrs
            .insert("horizontal".to_string(), horizontal.clone());
    }
    if let Some(vertical) = &spec.vertical {
        alignment
            .attrs
            .insert("vertical".to_string(), vertical.clone());
    }
    if let Some(wrap) = spec.wrap_text {
        if wrap {
            alignment
                .attrs
                .insert("wrapText".to_string(), "1".to_string());
        } else {
            alignment.attrs.remove("wrapText");
        }
    }
    set_style_child(&mut children, alignment, xf_child_order);
    xf.inner_xml = render_style_children(&children);
}

fn set_xlsx_border_edge(
    children: &mut Vec<StyleElement>,
    name: &str,
    style: &str,
    color: Option<&str>,
) {
    let mut edge = children
        .iter()
        .find(|child| child.name == name)
        .cloned()
        .unwrap_or_else(|| StyleElement {
            name: name.to_string(),
            attrs: BTreeMap::new(),
            inner_xml: String::new(),
        });
    edge.inner_xml.clear();
    if style.is_empty() || style == "none" {
        edge.attrs.remove("style");
    } else {
        edge.attrs.insert("style".to_string(), style.to_string());
        if let Some(color) = color {
            edge.inner_xml = format!(r#"<color rgb="{color}"/>"#);
        }
    }
    set_style_child(children, edge, border_child_order);
}

fn collection_entries(xml: &str, parent: &str, child: &str) -> CliResult<Vec<StyleElement>> {
    let Some(parent_span) = element_span_by_local_name(xml, parent) else {
        return Ok(Vec::new());
    };
    if parent_span.close_start < parent_span.open_end {
        return Ok(Vec::new());
    }
    let fragment = &xml[parent_span.open_end..parent_span.close_start];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut entries = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == child => {
                let attrs = xml_attrs(&e);
                let open_end = reader.buffer_position() as usize;
                let mut depth = 1usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::Start(_)) => depth += 1,
                        Ok(Event::End(e)) => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 && local_name(e.name().as_ref()) == child {
                                entries.push(StyleElement {
                                    name: child.to_string(),
                                    attrs,
                                    inner_xml: fragment[open_end..inner_before].to_string(),
                                });
                                break;
                            }
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected(format!(
                                "{child} has no closing tag"
                            )));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == child => {
                let _ = before;
                entries.push(StyleElement {
                    name: child.to_string(),
                    attrs: xml_attrs(&e),
                    inner_xml: String::new(),
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(entries)
}

fn direct_style_children(xml: &str) -> CliResult<Vec<StyleElement>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut children = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let attrs = xml_attrs(&e);
                let open_end = reader.buffer_position() as usize;
                let mut depth = 1usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::Start(_)) => depth += 1,
                        Ok(Event::End(e)) => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 && local_name(e.name().as_ref()) == name {
                                children.push(StyleElement {
                                    name,
                                    attrs,
                                    inner_xml: xml[open_end..inner_before].to_string(),
                                });
                                break;
                            }
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("style child has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let _ = before;
                children.push(StyleElement {
                    name: local_name(e.name().as_ref()).to_string(),
                    attrs: xml_attrs(&e),
                    inner_xml: String::new(),
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(children)
}

fn append_collection_child(xml: String, parent: &str, child_xml: &str) -> CliResult<String> {
    let Some(span) = element_span_by_local_name(&xml, parent) else {
        return Err(CliError::unexpected(format!("styles {parent} not found")));
    };
    if span.close_start < span.open_end {
        let open = &xml[span.start..span.open_end];
        let tag = qualified_tag_name(open).unwrap_or(parent);
        let open = open
            .trim_end()
            .strip_suffix("/>")
            .map(|prefix| format!("{}>", prefix.trim_end()))
            .unwrap_or_else(|| open.to_string());
        let replacement = format!("{open}{child_xml}</{tag}>");
        let mut out = String::with_capacity(xml.len() + child_xml.len() + parent.len() + 3);
        out.push_str(&xml[..span.start]);
        out.push_str(&replacement);
        out.push_str(&xml[span.open_end..]);
        return Ok(out);
    }
    let mut out = String::with_capacity(xml.len() + child_xml.len());
    out.push_str(&xml[..span.close_start]);
    out.push_str(child_xml);
    out.push_str(&xml[span.close_start..]);
    Ok(out)
}

fn qualified_tag_name(open_tag: &str) -> Option<&str> {
    let name_start = open_tag.find('<')? + 1;
    let rest = &open_tag[name_start..];
    let name_end = rest
        .find(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>')
        .unwrap_or(rest.len());
    rest.get(..name_end).filter(|value| !value.is_empty())
}

fn render_style_children(children: &[StyleElement]) -> String {
    children
        .iter()
        .map(|child| render_style_element(&child.name, child))
        .collect::<String>()
}

fn render_style_element(name: &str, element: &StyleElement) -> String {
    if element.inner_xml.is_empty() {
        format!("<{name}{}/>", render_xml_attrs(&element.attrs))
    } else {
        format!(
            "<{name}{}>{}</{name}>",
            render_xml_attrs(&element.attrs),
            element.inner_xml
        )
    }
}

fn set_empty_style_child(
    children: &mut Vec<StyleElement>,
    name: &str,
    enabled: bool,
    order: fn(&str) -> i32,
) {
    if !enabled {
        children.retain(|child| child.name != name);
        return;
    }
    if children.iter().any(|child| child.name == name) {
        return;
    }
    set_style_child(
        children,
        StyleElement {
            name: name.to_string(),
            attrs: BTreeMap::new(),
            inner_xml: String::new(),
        },
        order,
    );
}

fn set_val_style_child(
    children: &mut Vec<StyleElement>,
    name: &str,
    value: &str,
    order: fn(&str) -> i32,
) {
    let mut attrs = BTreeMap::new();
    attrs.insert("val".to_string(), value.to_string());
    set_style_child(
        children,
        StyleElement {
            name: name.to_string(),
            attrs,
            inner_xml: String::new(),
        },
        order,
    );
}

fn set_style_child(children: &mut Vec<StyleElement>, child: StyleElement, order: fn(&str) -> i32) {
    children.retain(|existing| existing.name != child.name);
    let child_order = order(&child.name);
    let index = children
        .iter()
        .position(|existing| order(&existing.name) > child_order)
        .unwrap_or(children.len());
    children.insert(index, child);
}

fn default_xlsx_xf_entry() -> StyleElement {
    let mut attrs = BTreeMap::new();
    attrs.insert("numFmtId".to_string(), "0".to_string());
    attrs.insert("fontId".to_string(), "0".to_string());
    attrs.insert("fillId".to_string(), "0".to_string());
    attrs.insert("borderId".to_string(), "0".to_string());
    attrs.insert("xfId".to_string(), "0".to_string());
    StyleElement {
        name: "xf".to_string(),
        attrs,
        inner_xml: String::new(),
    }
}

fn style_attr_u32(element: &StyleElement, key: &str) -> u32 {
    element
        .attrs
        .get(key)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0)
}

fn normalize_xlsx_style_color(value: &str) -> CliResult<String> {
    let mut normalized = value.trim().trim_start_matches('#').to_ascii_uppercase();
    if normalized.chars().any(|ch| !ch.is_ascii_hexdigit()) {
        return Err(CliError::invalid_args(format!(
            "invalid color {value:?} (use hex like #1A2B3C)"
        )));
    }
    match normalized.len() {
        6 => {
            normalized.insert_str(0, "FF");
            Ok(normalized)
        }
        8 => Ok(normalized),
        _ => Err(CliError::invalid_args(format!(
            "invalid color {value:?} (expected 6 or 8 hex digits)"
        ))),
    }
}

fn valid_xlsx_border_style(value: &str) -> bool {
    matches!(
        value,
        "thin"
            | "medium"
            | "thick"
            | "double"
            | "dotted"
            | "dashed"
            | "hair"
            | "dashDot"
            | "dashDotDot"
            | "mediumDashed"
            | "none"
    )
}

fn valid_xlsx_horizontal_alignment(value: &str) -> bool {
    matches!(
        value,
        "left"
            | "center"
            | "right"
            | "fill"
            | "justify"
            | "centerContinuous"
            | "distributed"
            | "general"
    )
}

fn valid_xlsx_vertical_alignment(value: &str) -> bool {
    matches!(
        value,
        "top" | "center" | "bottom" | "justify" | "distributed"
    )
}

fn font_child_order(name: &str) -> i32 {
    match name {
        "b" => 10,
        "i" => 20,
        "u" => 30,
        "strike" => 40,
        "sz" => 50,
        "color" => 60,
        "name" => 70,
        "family" => 80,
        "scheme" => 90,
        _ => 1000,
    }
}

fn border_child_order(name: &str) -> i32 {
    match name {
        "start" | "left" => 10,
        "end" | "right" => 20,
        "top" => 30,
        "bottom" => 40,
        "diagonal" => 50,
        _ => 1000,
    }
}

fn xf_child_order(name: &str) -> i32 {
    match name {
        "alignment" => 10,
        "protection" => 20,
        "extLst" => 30,
        _ => 1000,
    }
}
