use super::package::*;
use super::style::*;
use super::xml::*;
use super::*;

mod output;

pub(super) use output::{
    ChartCreateResultInput, ChartUpdateDataResultInput, chart_create_result_json,
    chart_update_data_result_json, unique_sorted_warnings,
};

pub(super) fn resolve_chart_create_source(args: &[String]) -> CliResult<ChartCreateSource> {
    let inline_json = parse_string_flag(args, "--values-json")?.unwrap_or_default();
    let inline_file = parse_string_flag(args, "--values-file")?.unwrap_or_default();
    let source_file = parse_string_flag(args, "--source-file")?.unwrap_or_default();
    let inline_count =
        usize::from(!inline_json.trim().is_empty()) + usize::from(!inline_file.trim().is_empty());
    if inline_count > 1 {
        return Err(CliError::invalid_args(
            "specify only one of --values-json or --values-file",
        ));
    }
    if inline_count == 1 && !source_file.trim().is_empty() {
        return Err(CliError::invalid_args(
            "specify either inline values or --source-file, not both",
        ));
    }
    let max_cells = parse_i64_flag(args, "--max-cells")?.unwrap_or(100000);
    if max_cells < 0 {
        return Err(CliError::invalid_args("--max-cells must be >= 0"));
    }

    if !source_file.trim().is_empty() {
        if !Path::new(&source_file).exists() {
            return Err(CliError::file_not_found(format!(
                "file not found: {source_file}"
            )));
        }
        let source_sheet = parse_string_flag(args, "--source-sheet")?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "1".to_string());
        let raw_range = parse_string_flag(args, "--source-range")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| CliError::invalid_args("--source-range is required"))?;
        let bounds = parse_cli_range(&raw_range)?.normalized();
        let source_range = range_bounds_ref_no_abs(bounds);
        let expect_range = parse_string_flag(args, "--expect-source-range")?.unwrap_or_default();
        if !expect_range.trim().is_empty() && !source_range.eq_ignore_ascii_case(&expect_range) {
            return Err(CliError::invalid_args(format!(
                "source range mismatch: expected {} but found {}",
                expect_range.trim(),
                source_range
            )));
        }
        let exported = xlsx_range_export_with_options(
            &source_file,
            &source_sheet,
            &source_range,
            XlsxRangeExportOptions {
                include_types: true,
                include_formulas: false,
                include_formats: false,
                data_out: None,
                max_cells,
            },
        )?;
        let cells = chart_cells_from_xlsx_export(&exported)?;
        let embedded_workbook = if crate::has_flag(args, "--embed-workbook") {
            fs::read(&source_file).map_err(|err| {
                CliError::unexpected(format!(
                    "failed to read source workbook for embedding: {err}"
                ))
            })?
        } else {
            Vec::new()
        };
        let sheet = exported
            .get("sheet")
            .and_then(Value::as_str)
            .unwrap_or(&source_sheet)
            .to_string();
        return Ok(ChartCreateSource {
            mode: "external".to_string(),
            sheet,
            range: source_range,
            bounds,
            cells,
            embedded_workbook,
            source_file,
        });
    }

    let raw = if !inline_file.trim().is_empty() {
        fs::read_to_string(&inline_file)
            .map_err(|err| CliError::invalid_args(format!("failed to read --values-file: {err}")))?
    } else if !inline_json.trim().is_empty() {
        inline_json
    } else {
        return Err(CliError::invalid_args(
            "must specify --values-json, --values-file, or --source-file",
        ));
    };
    let (cells, range) = parse_chart_inline_matrix(&raw, max_cells)?;
    let bounds = parse_cli_range(&range)?.normalized();
    Ok(ChartCreateSource {
        mode: "inline".to_string(),
        sheet: "Sheet1".to_string(),
        range,
        bounds,
        cells,
        embedded_workbook: Vec::new(),
        source_file: String::new(),
    })
}

pub(super) fn parse_chart_inline_matrix(
    raw: &str,
    max_cells: i64,
) -> CliResult<(Vec<Vec<ChartDataCell>>, String)> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|err| CliError::invalid_args(format!("invalid --values JSON matrix: {err}")))?;
    let rows = value.as_array().ok_or_else(|| {
        CliError::invalid_args("invalid --values JSON matrix: expected a JSON array of arrays")
    })?;
    if rows.is_empty() {
        return Err(CliError::invalid_args("inline values matrix is empty"));
    }
    let mut cols = 0usize;
    for row in rows {
        let row = row.as_array().ok_or_else(|| {
            CliError::invalid_args(
                "invalid --values JSON matrix: values must be an array of arrays",
            )
        })?;
        cols = cols.max(row.len());
    }
    if cols == 0 {
        return Err(CliError::invalid_args(
            "inline values matrix has no columns",
        ));
    }
    let cell_count = rows.len() * cols;
    if max_cells > 0 && cell_count > max_cells as usize {
        return Err(CliError::invalid_args(format!(
            "inline matrix has {cell_count} cells, exceeding --max-cells {max_cells}"
        )));
    }
    let mut matrix = Vec::with_capacity(rows.len());
    for row in rows {
        let row_values = row.as_array().expect("checked row array");
        let mut out_row = Vec::with_capacity(cols);
        for col_index in 0..cols {
            let cell = row_values
                .get(col_index)
                .map(chart_cell_from_json)
                .unwrap_or_else(|| ChartDataCell {
                    kind: String::new(),
                    value: String::new(),
                    null: true,
                });
            out_row.push(cell);
        }
        matrix.push(out_row);
    }
    let end_col = col_name(cols as u32);
    Ok((matrix, format!("A1:{end_col}{}", rows.len())))
}

pub(super) fn chart_cell_from_json(value: &Value) -> ChartDataCell {
    if value.is_null() {
        return ChartDataCell {
            kind: String::new(),
            value: String::new(),
            null: true,
        };
    }
    if let Some(number) = value.as_number() {
        return ChartDataCell {
            kind: "number".to_string(),
            value: number.to_string(),
            null: false,
        };
    }
    if let Some(boolean) = value.as_bool() {
        return ChartDataCell {
            kind: "boolean".to_string(),
            value: if boolean { "1" } else { "0" }.to_string(),
            null: false,
        };
    }
    if let Some(text) = value.as_str() {
        return ChartDataCell {
            kind: "string".to_string(),
            value: text.to_string(),
            null: false,
        };
    }
    ChartDataCell {
        kind: "string".to_string(),
        value: value.to_string(),
        null: false,
    }
}

pub(super) fn chart_cells_from_xlsx_export(exported: &Value) -> CliResult<Vec<Vec<ChartDataCell>>> {
    let values = exported
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::unexpected("xlsx range export missing values"))?;
    let types = exported.get("types").and_then(Value::as_array);
    let mut rows = Vec::with_capacity(values.len());
    for (row_index, row) in values.iter().enumerate() {
        let row_values = row
            .as_array()
            .ok_or_else(|| CliError::unexpected("xlsx range values must be rows"))?;
        let row_types = types
            .and_then(|items| items.get(row_index))
            .and_then(Value::as_array);
        let mut out_row = Vec::with_capacity(row_values.len());
        for (col_index, value) in row_values.iter().enumerate() {
            let kind = row_types
                .and_then(|items| items.get(col_index))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let mut cell = chart_cell_from_json(value);
            if kind == "number" || kind == "boolean" || kind == "string" {
                cell.kind = kind.to_string();
            }
            out_row.push(cell);
        }
        rows.push(out_row);
    }
    Ok(rows)
}

pub(super) fn resolve_chart_create_geometry(
    file: &str,
    args: &[String],
) -> CliResult<ChartGeometry> {
    let (slide_cx, slide_cy) = presentation_slide_size(file)?;
    let mut cx = parse_i64_flag(args, "--cx")?.unwrap_or(0);
    let mut cy = parse_i64_flag(args, "--cy")?.unwrap_or(0);
    if cx <= 0 {
        cx = slide_cx / 2;
    }
    if cy <= 0 {
        cy = slide_cy / 2;
    }
    let mut x = parse_i64_flag(args, "--x")?.unwrap_or(0);
    let mut y = parse_i64_flag(args, "--y")?.unwrap_or(0);
    if !value_flag_present(args, "--x") {
        x = ((slide_cx - cx) / 2).max(0);
    }
    if !value_flag_present(args, "--y") {
        y = ((slide_cy - cy) / 2).max(0);
    }
    Ok(ChartGeometry { x, y, cx, cy })
}

pub(super) fn presentation_slide_size(file: &str) -> CliResult<(i64, i64)> {
    let xml = zip_text(file, "ppt/presentation.xml")?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldSz" =>
            {
                let cx = e
                    .attributes()
                    .flatten()
                    .find_map(|attr| {
                        (local_name(attr.key.as_ref()) == "cx")
                            .then(|| decode_xml_text(attr.value.as_ref()))
                    })
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(10 * EMU_PER_INCH);
                let cy = e
                    .attributes()
                    .flatten()
                    .find_map(|attr| {
                        (local_name(attr.key.as_ref()) == "cy")
                            .then(|| decode_xml_text(attr.value.as_ref()))
                    })
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(EMU_PER_INCH * 15 / 2);
                return Ok((cx, cy));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok((10 * EMU_PER_INCH, EMU_PER_INCH * 15 / 2))
}

pub(super) fn range_bounds_ref_no_abs(bounds: RangeBounds) -> String {
    let bounds = bounds.normalized();
    let start = format!("{}{}", col_name(bounds.start_col), bounds.start_row);
    let end = format!("{}{}", col_name(bounds.end_col), bounds.end_row);
    if start == end {
        start
    } else {
        format!("{start}:{end}")
    }
}

pub(super) fn create_slide_chart_package_updates(
    file: &str,
    slide: usize,
    chart_type: &str,
    title: &str,
    source: &ChartCreateSource,
    geometry: &ChartGeometry,
    options: &PptxChartMutationOptions,
) -> CliResult<StagedCreateSlideChart> {
    let slide_ref = slide_part_for_number(file, slide)?;
    let chart_part = allocate_numbered_part(file, "/ppt/charts/chart", ".xml")?;
    let chart = build_chart_part(
        chart_type,
        title,
        &source.sheet,
        source.bounds,
        &source.cells,
    )
    .map_err(|err| CliError::invalid_args(format!("failed to create chart: {}", err.message)))?;

    let mut text_overrides = BTreeMap::new();
    let mut binary_overrides = BTreeMap::new();
    let mut content_types = zip_text(file, "[Content_Types].xml")?;
    content_types = ensure_content_type_override(
        content_types,
        chart_part.trim_start_matches('/'),
        CONTENT_TYPE_CHART,
    );
    let mut chart_xml = chart.xml;
    let mut embedded_part = String::new();
    if !source.embedded_workbook.is_empty() {
        embedded_part =
            allocate_numbered_part(file, "/ppt/embeddings/Microsoft_Excel_Sheet", ".xlsx")?;
        content_types = ensure_content_type_override(
            content_types,
            embedded_part.trim_start_matches('/'),
            CONTENT_TYPE_EMBEDDED_XLSX,
        );
        binary_overrides.insert(
            embedded_part.trim_start_matches('/').to_string(),
            source.embedded_workbook.clone(),
        );
        let chart_rels_part = relationships_part_for(&chart_part);
        let chart_rels_xml = zip_text(file, &chart_rels_part).unwrap_or_else(|_| {
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
        });
        let chart_rels = relationship_entries_from_xml(&chart_rels_xml);
        let package_rid = crate::allocate_relationship_id(&chart_rels);
        let target = relationship_target_from_source_to_target(&chart_part, &embedded_part);
        text_overrides.insert(
            chart_rels_part,
            crate::add_relationship_to_xml(chart_rels_xml, &package_rid, REL_PACKAGE, &target),
        );
        chart_xml = add_chart_external_data(&chart_xml, &package_rid)?;
    }
    text_overrides.insert(chart_part.trim_start_matches('/').to_string(), chart_xml);

    let slide_rels_part = relationships_part_for(&slide_ref.part_uri);
    let slide_rels_xml = zip_text(file, &slide_rels_part)?;
    let mut slide_rels = relationship_entries(file, &slide_rels_part)?;
    let chart_rid = crate::allocate_relationship_id(&slide_rels);
    let chart_target = relationship_target_from_source_to_target(&slide_ref.part_uri, &chart_part);
    slide_rels.push(crate::RelationshipEntry {
        id: chart_rid.clone(),
        rel_type: REL_CHART.to_string(),
        target: chart_target.clone(),
        target_mode: String::new(),
    });
    text_overrides.insert(
        slide_rels_part,
        crate::add_relationship_to_xml(slide_rels_xml, &chart_rid, REL_CHART, &chart_target),
    );

    let slide_part_name = slide_ref.part_uri.trim_start_matches('/').to_string();
    let slide_xml = zip_text(file, &slide_part_name)?;
    let (updated_slide_xml, shape_id, shape_name) =
        add_chart_graphic_frame_to_slide(&slide_xml, &chart_rid, geometry)?;
    text_overrides.insert(slide_part_name, updated_slide_xml);
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);

    let staged_path =
        stage_chart_package_mutation(file, &text_overrides, &binary_overrides, options)?;
    Ok(StagedCreateSlideChart {
        staged_path,
        result: CreateSlideChartResult {
            chart_uri: chart_part,
            chart_relationship_id: chart_rid,
            shape_id,
            shape_name,
            chart_type: chart_type.to_string(),
            title: title.to_string(),
            series_count: chart.series_count,
            categories: chart.categories,
            embedded_workbook_part_uri: embedded_part,
            warnings: chart.warnings,
        },
    })
}

pub(super) struct SlidePartRef {
    part_uri: String,
}

pub(super) fn slide_part_for_number(file: &str, slide: usize) -> CliResult<SlidePartRef> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = presentation_slide_refs(&presentation);
    if slide == 0 || slide > slides.len() {
        return Err(CliError::target_not_found(format!(
            "slide {slide} not found (presentation has {} slides)",
            slides.len()
        )));
    }
    let rel_id = &slides[slide - 1].1;
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    Ok(SlidePartRef {
        part_uri: format!("/{}", normalize_ppt_target(target)),
    })
}

pub(super) fn presentation_slide_refs(xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let (Some(id), Some(rel)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    slides.push((id, rel));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

pub(super) fn normalize_ppt_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("ppt/") {
        target.to_string()
    } else {
        format!("ppt/{}", target.trim_start_matches("../"))
    }
}

pub(super) fn allocate_numbered_part(file: &str, prefix: &str, suffix: &str) -> CliResult<String> {
    let mut max_number = 0usize;
    for entry in zip_entry_names(file)? {
        let uri = format!("/{}", entry.trim_start_matches('/'));
        if !uri.starts_with(prefix) || !uri.ends_with(suffix) {
            continue;
        }
        let middle = uri
            .trim_start_matches(prefix)
            .trim_end_matches(suffix)
            .trim();
        if let Ok(number) = middle.parse::<usize>() {
            max_number = max_number.max(number);
        }
    }
    Ok(format!("{prefix}{}{suffix}", max_number + 1))
}

pub(super) fn add_chart_external_data(xml: &str, rid: &str) -> CliResult<String> {
    let mut chart_xml = parse_chart_xml(xml)?;
    chart_xml
        .root
        .attrs
        .entry("xmlns:r".to_string())
        .or_insert_with(|| {
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships".to_string()
        });
    let mut external = XmlNode::new(chart_xml.chart_name("externalData"));
    external.set_attr("r:id", rid);
    let mut auto_update = XmlNode::new(chart_xml.chart_name("autoUpdate"));
    auto_update.set_attr("val", "0");
    external.children.push(auto_update);
    chart_xml.root.children.push(external);
    Ok(serialize_xml(&chart_xml.root))
}

pub(super) fn add_chart_graphic_frame_to_slide(
    slide_xml: &str,
    chart_rid: &str,
    geometry: &ChartGeometry,
) -> CliResult<(String, i64, String)> {
    let mut root = parse_xml_tree(slide_xml)?;
    let p_prefix = prefix_for_namespace(&root, PRESENTATION_NS).unwrap_or_else(|| "p".to_string());
    let a_prefix = prefix_for_namespace(&root, DRAWING_NS).unwrap_or_else(|| "a".to_string());
    let shape_id = root
        .first_descendant("spTree")
        .map(next_sp_tree_shape_id)
        .ok_or_else(|| CliError::unexpected("shape tree not found in slide"))?;
    let shape_name = format!("Chart {shape_id}");
    let frame = build_chart_graphic_frame(
        &p_prefix,
        &a_prefix,
        shape_id,
        &shape_name,
        chart_rid,
        geometry,
    );
    let sp_tree = root
        .first_descendant_mut("spTree")
        .ok_or_else(|| CliError::unexpected("shape tree not found in slide"))?;
    let insert_at = sp_tree
        .children
        .iter()
        .position(|child| child.local() == "extLst")
        .unwrap_or(sp_tree.children.len());
    sp_tree.children.insert(insert_at, frame);
    Ok((serialize_xml(&root), shape_id, shape_name))
}

pub(super) fn next_sp_tree_shape_id(sp_tree: &XmlNode) -> i64 {
    sp_tree
        .descendants("cNvPr")
        .into_iter()
        .filter_map(|node| node.attr("id"))
        .filter_map(|id| id.trim().parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

pub(super) fn build_chart_graphic_frame(
    p_prefix: &str,
    a_prefix: &str,
    shape_id: i64,
    shape_name: &str,
    chart_rid: &str,
    geometry: &ChartGeometry,
) -> XmlNode {
    let mut frame = XmlNode::new(qname(p_prefix, "graphicFrame"));
    let mut nv = XmlNode::new(qname(p_prefix, "nvGraphicFramePr"));
    let mut c_nv_pr = XmlNode::new(qname(p_prefix, "cNvPr"));
    c_nv_pr.set_attr("id", &shape_id.to_string());
    c_nv_pr.set_attr("name", shape_name);
    nv.children.push(c_nv_pr);
    nv.children
        .push(XmlNode::new(qname(p_prefix, "cNvGraphicFramePr")));
    nv.children.push(XmlNode::new(qname(p_prefix, "nvPr")));
    frame.children.push(nv);

    let mut xfrm = XmlNode::new(qname(p_prefix, "xfrm"));
    let mut off = XmlNode::new(qname(a_prefix, "off"));
    off.set_attr("x", &geometry.x.to_string());
    off.set_attr("y", &geometry.y.to_string());
    let mut ext = XmlNode::new(qname(a_prefix, "ext"));
    ext.set_attr("cx", &geometry.cx.to_string());
    ext.set_attr("cy", &geometry.cy.to_string());
    xfrm.children.push(off);
    xfrm.children.push(ext);
    frame.children.push(xfrm);

    let mut graphic = XmlNode::new(qname(a_prefix, "graphic"));
    let mut graphic_data = XmlNode::new(qname(a_prefix, "graphicData"));
    graphic_data.set_attr("uri", CHART_NS);
    let mut chart = XmlNode::new("c:chart".to_string());
    chart.set_attr("xmlns:c", CHART_NS);
    chart.set_attr(
        "xmlns:r",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
    );
    chart.set_attr("r:id", chart_rid);
    graphic_data.children.push(chart);
    graphic.children.push(graphic_data);
    frame.children.push(graphic);
    frame
}

pub(super) fn build_chart_part(
    chart_type: &str,
    title: &str,
    source_sheet: &str,
    source_range: RangeBounds,
    cells: &[Vec<ChartDataCell>],
) -> CliResult<CreateChartPartResult> {
    if !matches!(chart_type, "bar" | "line" | "area" | "pie" | "scatter") {
        return Err(CliError::invalid_args(format!(
            "invalid chart type {chart_type:?} (bar, line, area, pie, scatter)"
        )));
    }
    let (mut series, categories, mut warnings) =
        build_chart_series(source_sheet, source_range, cells)?;
    if series.is_empty() {
        return Err(CliError::invalid_args(
            "source range produced no chart series",
        ));
    }
    if chart_type == "pie" && series.len() > 1 {
        series.truncate(1);
        warnings.push("pie chart uses only the first series".to_string());
    }
    let root = build_chart_part_xml(chart_type, title, &series);
    Ok(CreateChartPartResult {
        xml: serialize_xml(&root),
        series_count: series.len(),
        categories,
        warnings,
    })
}

pub(super) fn build_chart_series(
    source_sheet: &str,
    source_range: RangeBounds,
    cells: &[Vec<ChartDataCell>],
) -> CliResult<(Vec<ChartSeriesData>, usize, Vec<String>)> {
    if cells.is_empty() {
        return Err(CliError::invalid_args("source range is empty"));
    }
    let bounds = source_range.normalized();
    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let has_header = rows > 1;
    let data_start_row = if has_header {
        bounds.start_row + 1
    } else {
        bounds.start_row
    };
    if bounds.end_row < data_start_row {
        return Err(CliError::invalid_args("source range has no data rows"));
    }

    let cell_at = |row: u32, col: u32| -> ChartDataCell {
        let row_index = row.saturating_sub(bounds.start_row) as usize;
        let col_index = col.saturating_sub(bounds.start_col) as usize;
        cells
            .get(row_index)
            .and_then(|row| row.get(col_index))
            .cloned()
            .unwrap_or_else(|| ChartDataCell {
                kind: String::new(),
                value: String::new(),
                null: true,
            })
    };
    let text =
        |cell: ChartDataCell| -> String { if cell.null { String::new() } else { cell.value } };

    let has_categories = cols > 1;
    let mut categories = Vec::new();
    if has_categories {
        for row in data_start_row..=bounds.end_row {
            categories.push(text(cell_at(row, bounds.start_col)));
        }
    }
    let category_ref = abs_ref(
        source_sheet,
        bounds.start_col,
        data_start_row,
        bounds.start_col,
        bounds.end_row,
    );

    let first_series_col = if has_categories {
        bounds.start_col + 1
    } else {
        bounds.start_col
    };
    let mut coerced = 0usize;
    let mut series = Vec::new();
    for col in first_series_col..=bounds.end_col {
        let mut item = ChartSeriesData {
            name: String::new(),
            name_ref: String::new(),
            categories: categories.clone(),
            category_ref: category_ref.clone(),
            values: Vec::new(),
            value_ref: String::new(),
        };
        if has_header {
            item.name = text(cell_at(bounds.start_row, col));
            item.name_ref = abs_ref(source_sheet, col, bounds.start_row, col, bounds.start_row);
        }
        for row in data_start_row..=bounds.end_row {
            let (value, was_coerced) = numeric_text_coerced(&cell_at(row, col));
            if was_coerced {
                coerced += 1;
            }
            item.values.push(value);
        }
        item.value_ref = abs_ref(source_sheet, col, data_start_row, col, bounds.end_row);
        series.push(item);
    }

    let mut warnings = Vec::new();
    if !has_categories {
        warnings.push("single-column source: no categories axis".to_string());
    }
    if coerced > 0 {
        warnings.push(format!("{coerced} non-numeric value(s) treated as 0"));
    }
    Ok((series, categories.len(), warnings))
}

pub(super) fn numeric_text_coerced(cell: &ChartDataCell) -> (String, bool) {
    if cell.null || cell.value.is_empty() {
        return ("0".to_string(), false);
    }
    if cell.value.parse::<f64>().is_ok() {
        return (cell.value.clone(), false);
    }
    ("0".to_string(), true)
}

pub(super) fn abs_ref(sheet: &str, col1: u32, row1: u32, col2: u32, row2: u32) -> String {
    let start = format!("${}${row1}", col_name(col1));
    let end = format!("${}${row2}", col_name(col2));
    let quoted = quote_chart_sheet(sheet);
    if col1 == col2 && row1 == row2 {
        format!("{quoted}!{start}")
    } else {
        format!("{quoted}!{start}:{end}")
    }
}

pub(super) fn quote_chart_sheet(sheet: &str) -> String {
    format!("'{}'", sheet.replace('\'', "''"))
}

pub(super) fn chart_cel(local: &str) -> XmlNode {
    XmlNode::new(qname("c", local))
}

pub(super) fn chart_cel_val(local: &str, value: impl ToString) -> XmlNode {
    let mut node = chart_cel(local);
    node.set_attr("val", &value.to_string());
    node
}

pub(super) fn drawing_cel(local: &str) -> XmlNode {
    XmlNode::new(qname("a", local))
}

pub(super) fn build_chart_part_xml(
    chart_type: &str,
    title: &str,
    series: &[ChartSeriesData],
) -> XmlNode {
    let mut root = chart_cel("chartSpace");
    root.set_attr("xmlns:c", CHART_NS);
    root.set_attr("xmlns:a", DRAWING_NS);
    root.set_attr(
        "xmlns:r",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
    );

    let mut chart = chart_cel("chart");
    if !title.trim().is_empty() {
        chart.children.push(build_chart_title_node(title));
        chart.children.push(chart_cel_val("autoTitleDeleted", "0"));
    } else {
        chart.children.push(chart_cel_val("autoTitleDeleted", "1"));
    }

    let mut plot_area = chart_cel("plotArea");
    plot_area.children.push(chart_cel("layout"));
    plot_area.children.push(build_plot_node(chart_type, series));
    if chart_type != "pie" {
        plot_area.children.push(build_cat_axis(chart_type));
        plot_area.children.push(build_val_axis());
    }
    chart.children.push(plot_area);
    chart.children.push(chart_cel_val("plotVisOnly", "1"));
    chart.children.push(chart_cel_val("dispBlanksAs", "gap"));
    root.children.push(chart);
    root
}

pub(super) fn build_chart_title_node(title: &str) -> XmlNode {
    let mut title_node = chart_cel("title");
    let mut tx = chart_cel("tx");
    let mut rich = chart_cel("rich");
    rich.children.push(drawing_cel("bodyPr"));
    rich.children.push(drawing_cel("lstStyle"));
    let mut paragraph = drawing_cel("p");
    let mut run = drawing_cel("r");
    let mut text = drawing_cel("t");
    text.text = title.to_string();
    run.children.push(text);
    paragraph.children.push(run);
    rich.children.push(paragraph);
    tx.children.push(rich);
    title_node.children.push(tx);
    title_node.children.push(chart_cel_val("overlay", "0"));
    title_node
}

pub(super) fn build_plot_node(chart_type: &str, series: &[ChartSeriesData]) -> XmlNode {
    match chart_type {
        "bar" => {
            let mut plot = chart_cel("barChart");
            plot.children.push(chart_cel_val("barDir", "col"));
            plot.children.push(chart_cel_val("grouping", "clustered"));
            plot.children.push(chart_cel_val("varyColors", "0"));
            for (idx, item) in series.iter().enumerate() {
                plot.children.push(build_category_series(idx, item));
            }
            plot.children.push(chart_cel_val("axId", CAT_AXIS_ID));
            plot.children.push(chart_cel_val("axId", VAL_AXIS_ID));
            plot
        }
        "line" => {
            let mut plot = chart_cel("lineChart");
            plot.children.push(chart_cel_val("grouping", "standard"));
            plot.children.push(chart_cel_val("varyColors", "0"));
            for (idx, item) in series.iter().enumerate() {
                plot.children.push(build_category_series(idx, item));
            }
            plot.children.push(chart_cel_val("marker", "1"));
            plot.children.push(chart_cel_val("axId", CAT_AXIS_ID));
            plot.children.push(chart_cel_val("axId", VAL_AXIS_ID));
            plot
        }
        "area" => {
            let mut plot = chart_cel("areaChart");
            plot.children.push(chart_cel_val("grouping", "standard"));
            plot.children.push(chart_cel_val("varyColors", "0"));
            for (idx, item) in series.iter().enumerate() {
                plot.children.push(build_category_series(idx, item));
            }
            plot.children.push(chart_cel_val("axId", CAT_AXIS_ID));
            plot.children.push(chart_cel_val("axId", VAL_AXIS_ID));
            plot
        }
        "pie" => {
            let mut plot = chart_cel("pieChart");
            plot.children.push(chart_cel_val("varyColors", "1"));
            for (idx, item) in series.iter().enumerate() {
                plot.children.push(build_category_series(idx, item));
            }
            plot.children.push(chart_cel_val("firstSliceAng", "0"));
            plot
        }
        "scatter" => {
            let mut plot = chart_cel("scatterChart");
            plot.children
                .push(chart_cel_val("scatterStyle", "lineMarker"));
            plot.children.push(chart_cel_val("varyColors", "0"));
            for (idx, item) in series.iter().enumerate() {
                plot.children.push(build_scatter_series(idx, item));
            }
            plot.children.push(chart_cel_val("axId", CAT_AXIS_ID));
            plot.children.push(chart_cel_val("axId", VAL_AXIS_ID));
            plot
        }
        _ => chart_cel("barChart"),
    }
}

pub(super) fn build_series_header(idx: usize, series: &ChartSeriesData) -> XmlNode {
    let mut ser = chart_cel("ser");
    ser.children.push(chart_cel_val("idx", idx));
    ser.children.push(chart_cel_val("order", idx));
    if !series.name_ref.is_empty() {
        let mut tx = chart_cel("tx");
        tx.children.push(build_str_ref(
            &series.name_ref,
            std::slice::from_ref(&series.name),
        ));
        ser.children.push(tx);
    }
    ser
}

pub(super) fn build_category_series(idx: usize, series: &ChartSeriesData) -> XmlNode {
    let mut ser = build_series_header(idx, series);
    if !series.category_ref.is_empty() && !series.categories.is_empty() {
        let mut cat = chart_cel("cat");
        cat.children
            .push(build_str_ref(&series.category_ref, &series.categories));
        ser.children.push(cat);
    }
    let mut val = chart_cel("val");
    val.children
        .push(build_num_ref(&series.value_ref, &series.values));
    ser.children.push(val);
    ser
}

pub(super) fn build_scatter_series(idx: usize, series: &ChartSeriesData) -> XmlNode {
    let mut ser = build_series_header(idx, series);
    let mut x_val = chart_cel("xVal");
    if !series.category_ref.is_empty() && !series.categories.is_empty() {
        x_val.children.push(build_num_ref(
            &series.category_ref,
            &numeric_axis(&series.categories),
        ));
    } else {
        x_val
            .children
            .push(build_num_ref(&series.value_ref, &series.values));
    }
    ser.children.push(x_val);
    let mut y_val = chart_cel("yVal");
    y_val
        .children
        .push(build_num_ref(&series.value_ref, &series.values));
    ser.children.push(y_val);
    ser
}

pub(super) fn numeric_axis(values: &[String]) -> Vec<String> {
    values
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            if !value.trim().is_empty() && value.trim().parse::<f64>().is_ok() {
                value.clone()
            } else {
                (idx + 1).to_string()
            }
        })
        .collect()
}

pub(super) fn build_str_ref(reference: &str, values: &[String]) -> XmlNode {
    let mut str_ref = chart_cel("strRef");
    let mut formula = chart_cel("f");
    formula.text = reference.to_string();
    str_ref.children.push(formula);
    let mut cache = chart_cel("strCache");
    cache.children.push(chart_cel_val("ptCount", values.len()));
    for (idx, value) in values.iter().enumerate() {
        cache.children.push(build_cache_point(idx, value));
    }
    str_ref.children.push(cache);
    str_ref
}

pub(super) fn build_num_ref(reference: &str, values: &[String]) -> XmlNode {
    let mut num_ref = chart_cel("numRef");
    let mut formula = chart_cel("f");
    formula.text = reference.to_string();
    num_ref.children.push(formula);
    num_ref
        .children
        .push(build_cache_element("numCache", values, Some("General")));
    num_ref
}

pub(super) fn build_cache_element(
    cache_type: &str,
    values: &[String],
    format_code: Option<&str>,
) -> XmlNode {
    let mut cache = chart_cel(cache_type);
    if cache_type == "numCache" {
        let mut format = chart_cel("formatCode");
        format.text = format_code.unwrap_or("General").to_string();
        cache.children.push(format);
    }
    cache.children.push(chart_cel_val("ptCount", values.len()));
    for (idx, value) in values.iter().enumerate() {
        cache.children.push(build_cache_point(idx, value));
    }
    cache
}

pub(super) fn build_cache_point(idx: usize, value: &str) -> XmlNode {
    let mut point = chart_cel("pt");
    point.set_attr("idx", &idx.to_string());
    let mut v = chart_cel("v");
    v.text = value.to_string();
    point.children.push(v);
    point
}

pub(super) fn build_cat_axis(chart_type: &str) -> XmlNode {
    let mut axis = if chart_type == "scatter" {
        chart_cel("valAx")
    } else {
        chart_cel("catAx")
    };
    axis.children.push(chart_cel_val("axId", CAT_AXIS_ID));
    let mut scaling = chart_cel("scaling");
    scaling
        .children
        .push(chart_cel_val("orientation", "minMax"));
    axis.children.push(scaling);
    axis.children.push(chart_cel_val("delete", "0"));
    axis.children.push(chart_cel_val("axPos", "b"));
    axis.children.push(chart_cel_val("crossAx", VAL_AXIS_ID));
    axis
}

pub(super) fn build_val_axis() -> XmlNode {
    let mut axis = chart_cel("valAx");
    axis.children.push(chart_cel_val("axId", VAL_AXIS_ID));
    let mut scaling = chart_cel("scaling");
    scaling
        .children
        .push(chart_cel_val("orientation", "minMax"));
    axis.children.push(scaling);
    axis.children.push(chart_cel_val("delete", "0"));
    axis.children.push(chart_cel_val("axPos", "l"));
    axis.children.push(chart_cel_val("crossAx", CAT_AXIS_ID));
    axis
}

#[derive(Clone, Copy)]
pub(super) struct ChartSourceRole {
    canonical: &'static str,
    element: &'static str,
}

pub(super) fn resolve_update_input_roles(args: &[String]) -> CliResult<Vec<UpdateInputRole>> {
    let (values, values_changed) = resolve_update_input_values(
        parse_string_flag(args, "--values")?,
        parse_string_flag(args, "--values-json")?,
    )?;
    let (categories, categories_changed) = resolve_update_input_values(
        parse_string_flag(args, "--categories")?,
        parse_string_flag(args, "--categories-json")?,
    )?;
    let mut roles = Vec::new();
    if values_changed {
        roles.push(UpdateInputRole {
            role: "values".to_string(),
            values,
        });
    }
    if categories_changed {
        roles.push(UpdateInputRole {
            role: "categories".to_string(),
            values: categories,
        });
    }
    Ok(roles)
}

pub(super) fn resolve_update_input_values(
    csv_values: Option<String>,
    json_values: Option<String>,
) -> CliResult<(Vec<String>, bool)> {
    if let Some(raw) = json_values.filter(|value| !value.trim().is_empty()) {
        let value: Value = serde_json::from_str(&raw)
            .map_err(|err| CliError::invalid_args(format!("invalid JSON values array: {err}")))?;
        let values = value
            .as_array()
            .ok_or_else(|| {
                CliError::invalid_args("invalid JSON values array: values must be an array")
            })?
            .iter()
            .map(|item| {
                item.as_str()
                    .map(|value| value.trim().to_string())
                    .ok_or_else(|| {
                        CliError::invalid_args("invalid JSON values array: values must be strings")
                    })
            })
            .collect::<CliResult<Vec<_>>>()?;
        return Ok((values, true));
    }
    if let Some(raw) = csv_values.filter(|value| !value.trim().is_empty()) {
        let values = parse_single_csv_record(&raw)
            .map_err(|message| {
                CliError::invalid_args(format!("invalid comma-separated values: {message}"))
            })?
            .into_iter()
            .map(|value| value.trim().to_string())
            .collect();
        return Ok((values, true));
    }
    Ok((Vec::new(), false))
}

pub(super) fn parse_single_csv_record(data: &str) -> Result<Vec<String>, String> {
    let mut record = Vec::new();
    let mut field = String::new();
    let mut chars = data.chars().peekable();
    let mut in_quotes = false;
    let mut field_started = false;
    let mut just_closed_quote = false;

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                    just_closed_quote = true;
                }
            } else {
                field.push(ch);
            }
            continue;
        }

        if ch == '"' {
            if !field_started {
                in_quotes = true;
                field_started = true;
                continue;
            }
            return Err("parse error on line 1, column 1: bare \" in non-quoted-field".to_string());
        }
        if ch == ',' {
            record.push(std::mem::take(&mut field));
            field_started = false;
            just_closed_quote = false;
            continue;
        }
        if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            record.push(std::mem::take(&mut field));
            return Ok(record);
        }
        if just_closed_quote {
            return Err(
                "parse error on line 1, column 1: extraneous or missing \" in quoted-field"
                    .to_string(),
            );
        }
        field_started = true;
        field.push(ch);
    }

    if in_quotes {
        return Err(
            "parse error on line 1, column 1: extraneous or missing \" in quoted-field".to_string(),
        );
    }
    record.push(field);
    Ok(record)
}

pub(super) fn chart_source_role_for(value: &str) -> Option<ChartSourceRole> {
    match value.trim().to_ascii_lowercase().as_str() {
        "name" | "tx" | "series-name" | "seriesname" => Some(ChartSourceRole {
            canonical: "name",
            element: "tx",
        }),
        "categories" | "category" | "cat" | "cats" => Some(ChartSourceRole {
            canonical: "categories",
            element: "cat",
        }),
        "values" | "value" | "val" | "vals" => Some(ChartSourceRole {
            canonical: "values",
            element: "val",
        }),
        "xvalues" | "x" | "xval" | "x-val" | "x-values" => Some(ChartSourceRole {
            canonical: "xValues",
            element: "xVal",
        }),
        "yvalues" | "y" | "yval" | "y-val" | "y-values" => Some(ChartSourceRole {
            canonical: "yValues",
            element: "yVal",
        }),
        "bubblesize" | "bubble" | "bubble-size" => Some(ChartSourceRole {
            canonical: "bubbleSize",
            element: "bubbleSize",
        }),
        _ => None,
    }
}

pub(super) fn read_series_source(
    chart_xml: &ChartXml,
    series_number: usize,
    role_name: &str,
) -> CliResult<SeriesSourceSnapshot> {
    let role = chart_source_role_for(role_name).ok_or_else(|| {
        CliError::invalid_args(format!(
            "invalid chart source role {role_name:?} (must be name, categories, values, xValues, yValues, or bubbleSize)"
        ))
    })?;
    let series = walk_series(&chart_xml.root);
    if series_number == 0 || series_number > series.len() {
        return Err(CliError::invalid_args(format!(
            "series {series_number} is out of range (1-{})",
            series.len()
        )));
    }
    let ser = series[series_number - 1];
    let role_elem = ser.direct_child(role.element).ok_or_else(|| {
        CliError::invalid_args(format!(
            "series {series_number} has no {} source (available roles: {})",
            role.canonical,
            series_roles(ser).join(", ")
        ))
    })?;
    let (source_ref, ref_kind) = source_ref_child(role_elem)?;
    let mut snapshot = SeriesSourceSnapshot {
        role: role.canonical.to_string(),
        ref_kind,
        ..SeriesSourceSnapshot::default()
    };
    if let Some(formula) = source_ref.direct_child("f") {
        snapshot.formula = normalize_formula_text(&formula.text);
        if let Some((sheet, range)) = parse_local_range_formula(&snapshot.formula) {
            snapshot.sheet = sheet;
            snapshot.range = range;
        }
    }
    if let Some(cache) = first_cache_child(source_ref) {
        snapshot.cache_type = cache.local().to_string();
        snapshot.point_count = cache_point_count(cache);
        snapshot.values = cache_values(cache);
    }
    Ok(snapshot)
}

pub(super) fn set_series_source(
    chart_xml: &mut ChartXml,
    series_number: usize,
    role_name: &str,
    formula: &str,
    cache_points: &[CachePoint],
) -> CliResult<SetSeriesSourceResult> {
    let role = chart_source_role_for(role_name).ok_or_else(|| {
        CliError::invalid_args(format!(
            "invalid chart source role {role_name:?} (must be name, categories, values, xValues, yValues, or bubbleSize)"
        ))
    })?;
    let total_series = series_count(&chart_xml.root);
    let ser = nth_series_mut(&mut chart_xml.root, series_number).ok_or_else(|| {
        CliError::invalid_args(format!(
            "series {series_number} is out of range (1-{})",
            total_series
        ))
    })?;
    let available_roles = series_roles(ser).join(", ");
    let role_elem = ser.direct_child_mut(role.element).ok_or_else(|| {
        CliError::invalid_args(format!(
            "series {series_number} has no {} source (available roles: {available_roles})",
            role.canonical
        ))
    })?;

    let (ref_kind, cache_type, cache_preview, cache_point_count) = {
        let (ref_kind, source_ref) = source_ref_child_mut(role_elem)?;
        let formula = normalize_formula_text(formula);
        let prefix = prefix_from_name(&source_ref.name).unwrap_or_else(|| "c".to_string());
        let formula_index = match source_ref.direct_child_index("f") {
            Some(index) => index,
            None => {
                source_ref
                    .children
                    .insert(0, XmlNode::new(qname(&prefix, "f")));
                0
            }
        };
        source_ref.children[formula_index].text = formula;
        source_ref.children.retain(|child| !is_cache_child(child));
        let formula_index = source_ref.direct_child_index("f").unwrap_or(0);
        let cache_type = cache_type_for_ref_kind(&ref_kind);
        let cache_values = cache_points
            .iter()
            .map(|point| point.value.clone())
            .collect::<Vec<_>>();
        let cache = build_cache_element_for_prefix(&prefix, &cache_type, cache_points, "General");
        source_ref.children.insert(formula_index + 1, cache);
        (
            ref_kind,
            cache_type,
            preview_strings(&cache_values, 5),
            cache_points.len(),
        )
    };

    let counts = sibling_point_counts(ser);
    let mut warnings = Vec::new();
    if let Some(edited_count) = counts.get(role.canonical).copied()
        && edited_count > 0
        && comparable_point_role(role.canonical)
    {
        for (sibling_role, count) in counts {
            if sibling_role == role.canonical
                || !comparable_point_role(&sibling_role)
                || count == 0
                || count == edited_count
            {
                continue;
            }
            warnings.push(format!(
                "{} now has {} point(s) but {} has {}; chart may misrender until related sources are updated",
                role.canonical, edited_count, sibling_role, count
            ));
        }
    }

    let _ = ref_kind;
    Ok(SetSeriesSourceResult {
        cache_type,
        cache_point_count,
        cache_preview,
        warnings,
    })
}

pub(super) fn nth_series_mut(root: &mut XmlNode, series_number: usize) -> Option<&mut XmlNode> {
    if series_number == 0 {
        return None;
    }
    let plot_area = root.first_descendant_mut("plotArea")?;
    let mut count = 0usize;
    for chart_type in &mut plot_area.children {
        if !chart_type.local().ends_with("Chart") {
            continue;
        }
        for ser in &mut chart_type.children {
            if ser.local() != "ser" {
                continue;
            }
            count += 1;
            if count == series_number {
                return Some(ser);
            }
        }
    }
    None
}

pub(super) fn source_ref_child(role_elem: &XmlNode) -> CliResult<(&XmlNode, String)> {
    for local in ["numRef", "strRef", "multiLvlStrRef"] {
        if let Some(child) = role_elem.direct_child(local) {
            if local == "multiLvlStrRef" {
                return Err(CliError::invalid_args(
                    "multi-level category sources are not supported",
                ));
            }
            return Ok((child, local.to_string()));
        }
    }
    if role_elem.direct_child("v").is_some() {
        return Err(CliError::invalid_args(
            "series source is a literal value, not a cell reference; setting literal chart sources is not supported",
        ));
    }
    Err(CliError::invalid_args(
        "series source has no supported reference",
    ))
}

pub(super) fn source_ref_child_mut(role_elem: &mut XmlNode) -> CliResult<(String, &mut XmlNode)> {
    if let Some(index) = role_elem
        .children
        .iter()
        .position(|child| matches!(child.local(), "numRef" | "strRef" | "multiLvlStrRef"))
    {
        let ref_kind = role_elem.children[index].local().to_string();
        if ref_kind == "multiLvlStrRef" {
            return Err(CliError::invalid_args(
                "multi-level category sources are not supported",
            ));
        }
        return Ok((ref_kind, &mut role_elem.children[index]));
    }
    if role_elem.direct_child("v").is_some() {
        return Err(CliError::invalid_args(
            "series source is a literal value, not a cell reference; setting literal chart sources is not supported",
        ));
    }
    Err(CliError::invalid_args(
        "series source has no supported reference",
    ))
}

pub(super) fn first_cache_child(node: &XmlNode) -> Option<&XmlNode> {
    node.children.iter().find(|child| is_cache_child(child))
}

pub(super) fn is_cache_child(node: &XmlNode) -> bool {
    matches!(node.local(), "strCache" | "numCache" | "multiLvlStrCache")
}

pub(super) fn cache_point_count(cache: &XmlNode) -> usize {
    cache
        .direct_child("ptCount")
        .and_then(|node| node.attr("val"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or_else(|| cache.descendants("pt").len())
}

pub(super) fn cache_values(cache: &XmlNode) -> Vec<String> {
    cache
        .descendants("pt")
        .into_iter()
        .filter_map(|point| point.direct_child("v"))
        .map(|value| value.text.clone())
        .collect()
}

pub(super) fn series_roles(ser: &XmlNode) -> Vec<String> {
    let mut roles = Vec::new();
    for role_name in [
        "name",
        "categories",
        "values",
        "xValues",
        "yValues",
        "bubbleSize",
    ] {
        if let Some(role) = chart_source_role_for(role_name)
            && ser.direct_child(role.element).is_some()
        {
            roles.push(role.canonical.to_string());
        }
    }
    if roles.is_empty() {
        roles.push("none".to_string());
    }
    roles
}

pub(super) fn sibling_point_counts(ser: &XmlNode) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for role_name in [
        "name",
        "categories",
        "values",
        "xValues",
        "yValues",
        "bubbleSize",
    ] {
        let Some(role) = chart_source_role_for(role_name) else {
            continue;
        };
        let Some(role_elem) = ser.direct_child(role.element) else {
            continue;
        };
        let Ok((source_ref, _)) = source_ref_child(role_elem) else {
            continue;
        };
        let Some(cache) = first_cache_child(source_ref) else {
            continue;
        };
        counts.insert(role.canonical.to_string(), cache_point_count(cache));
    }
    counts
}

pub(super) fn cache_type_for_ref_kind(ref_kind: &str) -> String {
    if ref_kind == "numRef" {
        "numCache".to_string()
    } else {
        "strCache".to_string()
    }
}

pub(super) fn comparable_point_role(role: &str) -> bool {
    role != "name"
}

pub(super) fn build_cache_element_for_prefix(
    prefix: &str,
    cache_type: &str,
    points: &[CachePoint],
    format_code: &str,
) -> XmlNode {
    let mut cache = XmlNode::new(qname(prefix, cache_type));
    if cache_type == "numCache" {
        let mut format = XmlNode::new(qname(prefix, "formatCode"));
        format.text = if format_code.trim().is_empty() {
            "General".to_string()
        } else {
            format_code.to_string()
        };
        cache.children.push(format);
    }
    let mut pt_count = XmlNode::new(qname(prefix, "ptCount"));
    pt_count.set_attr("val", &points.len().to_string());
    cache.children.push(pt_count);
    for (idx, point) in points.iter().enumerate() {
        let point_index = point.index;
        let mut pt = XmlNode::new(qname(prefix, "pt"));
        pt.set_attr("idx", &point_index.max(idx).to_string());
        let mut value = XmlNode::new(qname(prefix, "v"));
        value.text = point.value.clone();
        pt.children.push(value);
        cache.children.push(pt);
    }
    cache
}

pub(super) fn normalize_formula_text(value: &str) -> String {
    value.trim().trim_start_matches('=').trim().to_string()
}

pub(super) fn parse_local_range_formula(formula: &str) -> Option<(String, String)> {
    let formula = normalize_formula_text(formula);
    if formula.is_empty() {
        return None;
    }
    let mut bang = None;
    let mut in_quote = false;
    let bytes = formula.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                if in_quote && index + 1 < bytes.len() && bytes[index + 1] == b'\'' {
                    index += 2;
                    continue;
                }
                in_quote = !in_quote;
            }
            b'!' if !in_quote => bang = Some(index),
            _ => {}
        }
        index += 1;
    }
    let bang = bang?;
    let mut sheet = formula[..bang].to_string();
    let reference = &formula[bang + 1..];
    if sheet.starts_with('\'') && sheet.ends_with('\'') && sheet.len() >= 2 {
        sheet = sheet[1..sheet.len() - 1].replace("''", "'");
    }
    if sheet.contains(['[', ']']) || reference.contains(['[', ']', ',']) {
        return None;
    }
    let normalized = normalize_chart_range_ref(reference)?;
    Some((sheet, normalized))
}

pub(super) fn normalize_chart_range_ref(reference: &str) -> Option<String> {
    let cleaned = reference.replace('$', "");
    parse_range(&cleaned).ok().map(|bounds| {
        let bounds = bounds.normalized();
        if reference.contains('$') {
            absolute_range_bounds_ref(bounds)
        } else {
            range_bounds_ref(bounds)
        }
    })
}

pub(super) fn absolute_range_bounds_ref(bounds: RangeBounds) -> String {
    let start = format!("${}${}", col_name(bounds.start_col), bounds.start_row);
    let end = format!("${}${}", col_name(bounds.end_col), bounds.end_row);
    if start == end {
        start
    } else {
        format!("{start}:{end}")
    }
}

pub(super) fn chart_cache_points_for_values(
    values: &[String],
    ref_kind: &str,
) -> Result<Vec<CachePoint>, String> {
    if values.is_empty() {
        return Err("at least one point is required".to_string());
    }
    let mut points = Vec::with_capacity(values.len());
    for (idx, value) in values.iter().enumerate() {
        if ref_kind == "numRef" {
            if value.trim().is_empty() {
                return Err(format!(
                    "point {} is empty but numeric chart sources require numbers",
                    idx + 1
                ));
            }
            if value.parse::<f64>().is_err() {
                return Err(format!(
                    "point {} value {:?} is not numeric",
                    idx + 1,
                    value
                ));
            }
        }
        points.push(CachePoint {
            index: idx,
            value: value.clone(),
        });
    }
    Ok(points)
}

pub(super) fn update_embedded_workbook_chart_range(
    file: &str,
    bytes: &[u8],
    snapshot: &SeriesSourceSnapshot,
    values: &[String],
) -> CliResult<Option<Vec<u8>>> {
    if snapshot.sheet.is_empty() || snapshot.range.is_empty() {
        return Ok(None);
    }
    let bounds = parse_range(&snapshot.range).map_err(|err| {
        CliError::invalid_args(format!(
            "invalid embedded workbook source range {:?}: {}",
            snapshot.range, err.message
        ))
    })?;
    let bounds = bounds.normalized();
    let rows = bounds.row_count() as usize;
    let cols = bounds.col_count() as usize;
    if rows != 1 && cols != 1 {
        return Err(CliError::invalid_args(format!(
            "embedded workbook source range {} is {}x{}; update-data currently requires a one-row or one-column series range",
            range_bounds_ref(bounds),
            rows,
            cols
        )));
    }
    if rows * cols != values.len() {
        return Err(CliError::invalid_args(format!(
            "embedded workbook source range {} has {} cell(s) but {} input has {} point(s)",
            range_bounds_ref(bounds),
            rows * cols,
            snapshot.role,
            values.len()
        )));
    }
    let matrix = chart_values_to_range_matrix(values, rows, cols, &snapshot.ref_kind);
    let values_json = serde_json::to_string(&matrix).map_err(|err| {
        CliError::unexpected(format!(
            "failed to encode embedded workbook update values: {err}"
        ))
    })?;
    let parent = Path::new(file)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let nonce = chrono_like_counter();
    let input_path = parent.join(format!(
        ".ooxml-rust-pptx-chart-embedded-{}-{nonce}.xlsx",
        std::process::id()
    ));
    let output_path = parent.join(format!(
        ".ooxml-rust-pptx-chart-embedded-out-{}-{nonce}.xlsx",
        std::process::id()
    ));
    let input_string = input_path.to_string_lossy().to_string();
    let output_string = output_path.to_string_lossy().to_string();
    fs::write(&input_path, bytes)
        .map_err(|err| CliError::unexpected(format!("failed to stage embedded workbook: {err}")))?;
    let set_result = xlsx_ranges_set(
        &input_string,
        XlsxRangesSetOptions {
            sheet: &snapshot.sheet,
            range: Some(&range_bounds_ref(bounds)),
            anchor: None,
            values: Some(&values_json),
            values_file: None,
            data_format: Some("json"),
            null_policy: Some("empty-string"),
            ragged: Some("reject"),
            max_cells: 0,
            out: Some(&output_string),
            backup: None,
            dry_run: false,
            no_validate: true,
            in_place: false,
            overwrite_formulas: true,
        },
    );
    let result = match set_result {
        Ok(_) => fs::read(&output_path).map(Some).map_err(|err| {
            CliError::unexpected(format!("failed to read updated embedded workbook: {err}"))
        }),
        Err(err) => Err(CliError::invalid_args(format!(
            "failed to update embedded workbook range {}!{}: {}",
            snapshot.sheet, snapshot.range, err.message
        ))),
    };
    let _ = fs::remove_file(&input_path);
    let _ = fs::remove_file(&output_path);
    result
}

pub(super) fn chart_values_to_range_matrix(
    values: &[String],
    rows: usize,
    cols: usize,
    ref_kind: &str,
) -> Vec<Vec<Value>> {
    let value_type = if ref_kind == "numRef" {
        "number"
    } else {
        "string"
    };
    let mut matrix = Vec::with_capacity(rows);
    let mut index = 0usize;
    for _ in 0..rows {
        let mut row = Vec::with_capacity(cols);
        for _ in 0..cols {
            row.push(json!({
                "type": value_type,
                "value": values.get(index).cloned().unwrap_or_default(),
            }));
            index += 1;
        }
        matrix.push(row);
    }
    matrix
}

pub(super) fn chart_values_hash(values: &[String]) -> String {
    let data = serde_json::to_vec(values).unwrap_or_else(|_| b"[]".to_vec());
    let digest = Sha256::digest(data);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("sha256:{hex}")
}

pub(super) fn chart_hash_matches(current: &str, expected: &str) -> bool {
    let expected = expected.trim();
    expected.is_empty()
        || current == expected
        || (!expected.starts_with("sha256:") && current.trim_start_matches("sha256:") == expected)
}

pub(super) fn preview_strings(values: &[String], limit: usize) -> Vec<String> {
    values.iter().take(limit).cloned().collect()
}
