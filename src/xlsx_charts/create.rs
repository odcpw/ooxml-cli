use super::*;

pub(super) fn ensure_xlsx_file_exists(file: &str) -> CliResult<()> {
    if Path::new(file).exists() {
        Ok(())
    } else {
        Err(CliError::file_not_found(format!("file not found: {file}")))
    }
}

pub(super) fn parse_create_chart_type(value: Option<&str>) -> CliResult<String> {
    let chart_type = value.unwrap_or_default().trim().to_ascii_lowercase();
    if chart_type.is_empty() {
        return Err(CliError::invalid_args(
            "--type is required (bar, line, area, pie, scatter)",
        ));
    }
    match chart_type.as_str() {
        "bar" | "line" | "area" | "pie" | "scatter" => Ok(chart_type),
        _ => Err(CliError::invalid_args(format!(
            "failed to create chart: invalid chart type {chart_type:?} (bar, line, area, pie, scatter)"
        ))),
    }
}

pub(super) fn resolve_chart_create_source(
    file: &str,
    options: &XlsxChartCreateOptions<'_>,
) -> CliResult<ChartCreateSource> {
    let source_sheet = options.sheet.unwrap_or_default().trim().to_string();
    let source_range = options.range.unwrap_or_default().trim().to_string();
    let source_table = options.table.unwrap_or_default().trim().to_string();
    if !source_range.is_empty() && !source_table.is_empty() {
        return Err(CliError::invalid_args(
            "specify only one of --range or --table",
        ));
    }
    if source_range.is_empty() && source_table.is_empty() {
        return Err(CliError::invalid_args("must specify --range or --table"));
    }
    let (sheet, range) = if !source_table.is_empty() {
        let tables = xlsx_tables(
            file,
            if source_sheet.is_empty() {
                None
            } else {
                Some(source_sheet.as_str())
            },
        )?;
        let table = select_xlsx_table(&tables, &source_table)?;
        (table.sheet, table.range)
    } else {
        if source_sheet.is_empty() {
            return Err(CliError::invalid_args(
                "--sheet is required when using --range",
            ));
        }
        (source_sheet, source_range)
    };
    let bounds = parse_range(&range)
        .map_err(|err| CliError::invalid_args(format!("invalid --range: {}", err.message)))?
        .normalized();
    check_range_max_cells(&range, bounds, options.max_cells)?;
    let exported = xlsx_range_export_with_options(
        file,
        &sheet,
        &range,
        XlsxRangeExportOptions {
            include_types: true,
            include_formulas: true,
            include_formats: false,
            data_out: None,
            max_cells: options.max_cells,
        },
    )?;
    let sheet_number = exported
        .get("sheetNumber")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let canonical_sheet = exported
        .get("sheet")
        .and_then(Value::as_str)
        .unwrap_or(&sheet)
        .to_string();
    let canonical_range = exported
        .get("range")
        .and_then(Value::as_str)
        .unwrap_or(&range)
        .to_string();
    let cells = chart_cells_from_range_export(&exported)?;
    Ok(ChartCreateSource {
        sheet: canonical_sheet,
        sheet_number,
        range: canonical_range,
        bounds,
        cells,
    })
}

pub(super) fn chart_cells_from_range_export(
    exported: &Value,
) -> CliResult<Vec<Vec<ChartSourceCell>>> {
    let values = exported
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::unexpected("range export omitted values"))?;
    let types = exported.get("types").and_then(Value::as_array);
    let number_format_codes = exported.get("numberFormatCodes").and_then(Value::as_array);
    let mut cells = Vec::new();
    for (row_idx, row) in values.iter().enumerate() {
        let row = row
            .as_array()
            .ok_or_else(|| CliError::unexpected("range export value row is not an array"))?;
        let type_row = types
            .and_then(|rows| rows.get(row_idx))
            .and_then(Value::as_array);
        let format_row = number_format_codes
            .and_then(|rows| rows.get(row_idx))
            .and_then(Value::as_array);
        let mut out_row = Vec::new();
        for (col_idx, value) in row.iter().enumerate() {
            out_row.push(ChartSourceCell {
                value: json_value_to_cell_text(value),
                kind: type_row
                    .and_then(|row| row.get(col_idx))
                    .and_then(Value::as_str)
                    .unwrap_or("empty")
                    .to_string(),
                null: value.is_null(),
                number_format_code: format_row
                    .and_then(|row| row.get(col_idx))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            });
        }
        cells.push(out_row);
    }
    Ok(cells)
}

pub(super) fn json_value_to_cell_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        other => other.to_string(),
    }
}

pub(super) fn resolve_chart_create_anchor(
    anchor: Option<&str>,
    source_bounds: RangeBounds,
) -> CliResult<((u32, u32), (u32, u32))> {
    let from = if let Some(anchor) = anchor.filter(|value| !value.trim().is_empty()) {
        parse_cell_ref(anchor)
            .map_err(|err| CliError::invalid_args(format!("invalid --anchor: {}", err.message)))?
    } else {
        (source_bounds.max_col() + 2, source_bounds.min_row())
    };
    Ok(((from.0, from.1), (from.0 + 8, from.1 + 15)))
}

pub(super) fn build_chart_create_artifacts(
    file: &str,
    source: &ChartCreateSource,
    chart_type: &str,
    title: &str,
    anchor_from: (u32, u32),
    anchor_to: (u32, u32),
) -> CliResult<ChartCreateArtifacts> {
    let workbook_xml = zip_text(file, "xl/workbook.xml")?;
    let workbook_sheets = workbook_sheets(&workbook_xml)?;
    let workbook_rels = relationship_entries(file, "xl/_rels/workbook.xml.rels")?;
    let sheet = resolve_sheet(&workbook_sheets, &source.sheet)?;
    let sheet_part_uri = sheet_part_uri_for_chart(&sheet, &workbook_rels).ok_or_else(|| {
        CliError::unexpected(format!("sheet {:?} has no worksheet part URI", sheet.name))
    })?;

    let (chart_xml, series_count, categories, warnings) =
        build_chart_part_xml(chart_type, title, source)?;
    let mut entries = zip_entry_names(file)?.into_iter().collect::<BTreeSet<_>>();
    let chart_uri = allocate_numbered_package_part(&mut entries, "/xl/charts/chart", ".xml");
    let mut overrides = BTreeMap::new();
    let mut content_types = zip_text(file, "[Content_Types].xml")?;
    content_types = ensure_content_type_override(content_types, &chart_uri, CONTENT_TYPE_CHART);

    let (drawing_uri, drawing_overrides) = build_or_update_chart_drawing(
        file,
        &sheet_part_uri,
        &chart_uri,
        anchor_from,
        anchor_to,
        &mut entries,
    )?;
    if !zip_entry_names(file)?
        .iter()
        .any(|entry| format!("/{}", entry.trim_start_matches('/')) == drawing_uri)
    {
        content_types =
            ensure_content_type_override(content_types, &drawing_uri, CONTENT_TYPE_DRAWING);
    }

    overrides.insert("[Content_Types].xml".to_string(), content_types);
    overrides.insert(part_name(&chart_uri), chart_xml);
    overrides.extend(drawing_overrides);

    Ok(ChartCreateArtifacts {
        chart_uri,
        drawing_uri,
        chart_type: chart_type.to_string(),
        title: title.trim().to_string(),
        series_count,
        categories,
        anchor: format!(
            "{}{}:{}{}",
            col_name(anchor_from.0),
            anchor_from.1,
            col_name(anchor_to.0),
            anchor_to.1
        ),
        warnings,
        overrides,
    })
}

pub(super) fn build_or_update_chart_drawing(
    file: &str,
    sheet_part_uri: &str,
    chart_uri: &str,
    anchor_from: (u32, u32),
    anchor_to: (u32, u32),
    entries: &mut BTreeSet<String>,
) -> CliResult<(String, BTreeMap<String, String>)> {
    let mut overrides = BTreeMap::new();
    if let Some((drawing_uri, _drawing_rid)) = worksheet_drawing_part(file, sheet_part_uri)? {
        let drawing_xml = zip_text(file, drawing_uri.trim_start_matches('/'))?;
        let mut drawing_root = parse_xml_node(&drawing_xml)?;
        if drawing_root.name != "wsDr" {
            return Err(CliError::unexpected(format!(
                "drawing part {drawing_uri} root element not found"
            )));
        }
        ensure_drawing_xml_namespaces(&mut drawing_root);
        let drawing_rels_part = relationships_part_for(&drawing_uri);
        let drawing_rels_xml =
            optional_zip_text(file, &drawing_rels_part)?.unwrap_or_else(empty_relationships_xml);
        let drawing_rels = relationship_entries_from_xml(&drawing_rels_xml);
        let chart_rid = allocate_relationship_id(&drawing_rels);
        let drawing_rels_xml = add_relationship_to_xml(
            drawing_rels_xml,
            &chart_rid,
            REL_CHART,
            &relationship_target_from_source_to_target(&drawing_uri, chart_uri),
        );
        let anchor = build_chart_anchor_node(
            drawing_root_prefix(&drawing_root).as_str(),
            anchor_from,
            anchor_to,
            &chart_rid,
            next_drawing_object_id(&drawing_root),
            next_chart_number(&drawing_root),
        );
        add_drawing_anchor(&mut drawing_root, anchor);
        overrides.insert(part_name(&drawing_uri), render_xml_document(&drawing_root));
        overrides.insert(drawing_rels_part, drawing_rels_xml);
        return Ok((drawing_uri, overrides));
    }

    let drawing_uri = allocate_numbered_package_part(entries, "/xl/drawings/drawing", ".xml");
    let chart_rid = "rId1".to_string();
    let drawing_xml = build_drawing_part_xml(anchor_from, anchor_to, &chart_rid);
    let drawing_rels_xml = render_relationships_xml(&[(
        chart_rid.as_str(),
        REL_CHART,
        relationship_target_from_source_to_target(&drawing_uri, chart_uri),
    )]);
    let worksheet_rels_part = relationships_part_for(sheet_part_uri);
    let worksheet_rels_xml =
        optional_zip_text(file, &worksheet_rels_part)?.unwrap_or_else(empty_relationships_xml);
    let worksheet_rels = relationship_entries_from_xml(&worksheet_rels_xml);
    let drawing_rid = allocate_relationship_id(&worksheet_rels);
    let worksheet_rels_xml = add_relationship_to_xml(
        worksheet_rels_xml,
        &drawing_rid,
        REL_DRAWING,
        &relationship_target_from_source_to_target(sheet_part_uri, &drawing_uri),
    );
    let worksheet_xml = zip_text(file, sheet_part_uri.trim_start_matches('/'))?;
    let worksheet_xml = add_worksheet_drawing_ref(&worksheet_xml, sheet_part_uri, &drawing_rid)?;

    overrides.insert(part_name(&drawing_uri), drawing_xml);
    overrides.insert(relationships_part_for(&drawing_uri), drawing_rels_xml);
    overrides.insert(worksheet_rels_part, worksheet_rels_xml);
    overrides.insert(part_name(sheet_part_uri), worksheet_xml);
    Ok((drawing_uri, overrides))
}

pub(super) fn build_chart_part_xml(
    chart_type: &str,
    title: &str,
    source: &ChartCreateSource,
) -> CliResult<(String, usize, usize, Vec<String>)> {
    let (mut series, categories, mut warnings) = build_chart_series(source)?;
    if series.is_empty() {
        return Err(CliError::invalid_args(
            "source range produced no chart series",
        ));
    }
    if chart_type == "pie" && series.len() > 1 {
        series.truncate(1);
        warnings.push("pie chart uses only the first series".to_string());
    }
    let mut xml = format!(
        r#"<c:chartSpace xmlns:c="{NS_CHART}" xmlns:a="{NS_DRAWING_MAIN}" xmlns:r="{NS_RELATIONSHIPS}"><c:chart>"#
    );
    if !title.trim().is_empty() {
        xml.push_str(&build_chart_title_xml(title));
        xml.push_str(r#"<c:autoTitleDeleted val="0"/>"#);
    } else {
        xml.push_str(r#"<c:autoTitleDeleted val="1"/>"#);
    }
    xml.push_str(r#"<c:plotArea><c:layout/>"#);
    xml.push_str(&build_plot_xml(chart_type, &series));
    if chart_type != "pie" {
        xml.push_str(&build_cat_axis_xml(chart_type));
        xml.push_str(&build_val_axis_xml());
    }
    xml.push_str(r#"</c:plotArea><c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/></c:chart></c:chartSpace>"#);
    Ok((xml, series.len(), categories, warnings))
}

pub(super) fn build_chart_series(
    source: &ChartCreateSource,
) -> CliResult<(Vec<BuiltChartSeries>, usize, Vec<String>)> {
    if source.cells.is_empty() {
        return Err(CliError::invalid_args("source range is empty"));
    }
    let bounds = source.bounds.normalized();
    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let has_header = rows > 1;
    let data_start_row = if has_header {
        bounds.min_row() + 1
    } else {
        bounds.min_row()
    };
    if data_start_row > bounds.max_row() {
        return Err(CliError::invalid_args("source range has no data rows"));
    }
    let has_categories = cols > 1;
    let cat_col = bounds.min_col();
    let mut cats = Vec::new();
    if has_categories {
        for row in data_start_row..=bounds.max_row() {
            cats.push(chart_cell_at(source, row, cat_col).value);
        }
    }
    let cat_ref = absolute_chart_ref(
        &source.sheet,
        cat_col,
        data_start_row,
        cat_col,
        bounds.max_row(),
    );
    let first_series_col = if has_categories {
        bounds.min_col() + 1
    } else {
        bounds.min_col()
    };
    let mut coerced = 0;
    let mut series = Vec::new();
    for col in first_series_col..=bounds.max_col() {
        let mut item = BuiltChartSeries {
            name: String::new(),
            name_ref: String::new(),
            cats: cats.clone(),
            cat_ref: cat_ref.clone(),
            values: Vec::new(),
            val_ref: absolute_chart_ref(&source.sheet, col, data_start_row, col, bounds.max_row()),
        };
        if has_header {
            item.name = chart_cell_at(source, bounds.min_row(), col).value;
            item.name_ref =
                absolute_chart_ref(&source.sheet, col, bounds.min_row(), col, bounds.min_row());
        }
        for row in data_start_row..=bounds.max_row() {
            let (value, was_coerced) = numeric_text_coerced(&chart_cell_at(source, row, col));
            if was_coerced {
                coerced += 1;
            }
            item.values.push(value);
        }
        series.push(item);
    }
    let mut warnings = Vec::new();
    if !has_categories {
        warnings.push("single-column source: no categories axis".to_string());
    }
    if coerced > 0 {
        warnings.push(format!("{coerced} non-numeric value(s) treated as 0"));
    }
    Ok((series, cats.len(), warnings))
}

pub(super) fn chart_cell_at(source: &ChartCreateSource, row: u32, col: u32) -> ChartSourceCell {
    let bounds = source.bounds.normalized();
    let row_idx = row.saturating_sub(bounds.min_row()) as usize;
    let col_idx = col.saturating_sub(bounds.min_col()) as usize;
    source
        .cells
        .get(row_idx)
        .and_then(|row| row.get(col_idx))
        .cloned()
        .unwrap_or(ChartSourceCell {
            value: String::new(),
            kind: "empty".to_string(),
            null: true,
            number_format_code: String::new(),
        })
}

pub(super) fn numeric_text_coerced(cell: &ChartSourceCell) -> (String, bool) {
    if cell.null || cell.value.is_empty() {
        return ("0".to_string(), false);
    }
    if cell.value.trim().parse::<f64>().is_ok() {
        return (cell.value.clone(), false);
    }
    ("0".to_string(), true)
}

pub(super) fn build_chart_title_xml(title: &str) -> String {
    format!(
        "<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val=\"0\"/></c:title>",
        xml_escape(title)
    )
}

pub(super) fn build_plot_xml(chart_type: &str, series: &[BuiltChartSeries]) -> String {
    let mut xml = String::new();
    match chart_type {
        "bar" => {
            xml.push_str(r#"<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>"#);
            for (idx, item) in series.iter().enumerate() {
                xml.push_str(&build_category_series_xml(idx, item));
            }
            xml.push_str(r#"<c:axId val="111111111"/><c:axId val="222222222"/></c:barChart>"#);
        }
        "line" => {
            xml.push_str(r#"<c:lineChart><c:grouping val="standard"/><c:varyColors val="0"/>"#);
            for (idx, item) in series.iter().enumerate() {
                xml.push_str(&build_category_series_xml(idx, item));
            }
            xml.push_str(r#"<c:marker val="1"/><c:axId val="111111111"/><c:axId val="222222222"/></c:lineChart>"#);
        }
        "area" => {
            xml.push_str(r#"<c:areaChart><c:grouping val="standard"/><c:varyColors val="0"/>"#);
            for (idx, item) in series.iter().enumerate() {
                xml.push_str(&build_category_series_xml(idx, item));
            }
            xml.push_str(r#"<c:axId val="111111111"/><c:axId val="222222222"/></c:areaChart>"#);
        }
        "pie" => {
            xml.push_str(r#"<c:pieChart><c:varyColors val="1"/>"#);
            for (idx, item) in series.iter().enumerate() {
                xml.push_str(&build_category_series_xml(idx, item));
            }
            xml.push_str(r#"<c:firstSliceAng val="0"/></c:pieChart>"#);
        }
        "scatter" => {
            xml.push_str(
                r#"<c:scatterChart><c:scatterStyle val="lineMarker"/><c:varyColors val="0"/>"#,
            );
            for (idx, item) in series.iter().enumerate() {
                xml.push_str(&build_scatter_series_xml(idx, item));
            }
            xml.push_str(r#"<c:axId val="111111111"/><c:axId val="222222222"/></c:scatterChart>"#);
        }
        _ => {}
    }
    xml
}

pub(super) fn build_series_header_xml(idx: usize, series: &BuiltChartSeries) -> String {
    let mut xml = format!(r#"<c:ser><c:idx val="{idx}"/><c:order val="{idx}"/>"#);
    if !series.name_ref.is_empty() {
        xml.push_str("<c:tx>");
        xml.push_str(&build_str_ref_xml(
            &series.name_ref,
            std::slice::from_ref(&series.name),
        ));
        xml.push_str("</c:tx>");
    }
    xml
}

pub(super) fn build_category_series_xml(idx: usize, series: &BuiltChartSeries) -> String {
    let mut xml = build_series_header_xml(idx, series);
    if !series.cat_ref.is_empty() && !series.cats.is_empty() {
        xml.push_str("<c:cat>");
        xml.push_str(&build_str_ref_xml(&series.cat_ref, &series.cats));
        xml.push_str("</c:cat>");
    }
    xml.push_str("<c:val>");
    xml.push_str(&build_num_ref_xml(
        &series.val_ref,
        &series.values,
        "General",
    ));
    xml.push_str("</c:val></c:ser>");
    xml
}

pub(super) fn build_scatter_series_xml(idx: usize, series: &BuiltChartSeries) -> String {
    let mut xml = build_series_header_xml(idx, series);
    xml.push_str("<c:xVal>");
    if !series.cat_ref.is_empty() && !series.cats.is_empty() {
        xml.push_str(&build_num_ref_xml(
            &series.cat_ref,
            &numeric_axis(&series.cats),
            "General",
        ));
    } else {
        xml.push_str(&build_num_ref_xml(
            &series.val_ref,
            &series.values,
            "General",
        ));
    }
    xml.push_str("</c:xVal><c:yVal>");
    xml.push_str(&build_num_ref_xml(
        &series.val_ref,
        &series.values,
        "General",
    ));
    xml.push_str("</c:yVal></c:ser>");
    xml
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

pub(super) fn build_str_ref_xml(reference: &str, values: &[String]) -> String {
    let mut xml = format!(
        "<c:strRef><c:f>{}</c:f><c:strCache><c:ptCount val=\"{}\"/>",
        xml_escape(reference),
        values.len()
    );
    for (idx, value) in values.iter().enumerate() {
        xml.push_str(&format!(
            "<c:pt idx=\"{idx}\"><c:v>{}</c:v></c:pt>",
            xml_escape(value)
        ));
    }
    xml.push_str("</c:strCache></c:strRef>");
    xml
}

pub(super) fn build_num_ref_xml(reference: &str, values: &[String], format_code: &str) -> String {
    let mut xml = format!(
        "<c:numRef><c:f>{}</c:f><c:numCache><c:formatCode>{}</c:formatCode><c:ptCount val=\"{}\"/>",
        xml_escape(reference),
        xml_escape(format_code),
        values.len()
    );
    for (idx, value) in values.iter().enumerate() {
        xml.push_str(&format!(
            "<c:pt idx=\"{idx}\"><c:v>{}</c:v></c:pt>",
            xml_escape(value)
        ));
    }
    xml.push_str("</c:numCache></c:numRef>");
    xml
}

pub(super) fn build_cat_axis_xml(chart_type: &str) -> String {
    let axis = if chart_type == "scatter" {
        "valAx"
    } else {
        "catAx"
    };
    format!(
        r#"<c:{axis}><c:axId val="111111111"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:crossAx val="222222222"/></c:{axis}>"#
    )
}

pub(super) fn build_val_axis_xml() -> String {
    r#"<c:valAx><c:axId val="222222222"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:crossAx val="111111111"/></c:valAx>"#.to_string()
}

pub(super) fn absolute_chart_ref(
    sheet: &str,
    col1: u32,
    row1: u32,
    col2: u32,
    row2: u32,
) -> String {
    let sheet = quote_chart_sheet_always(sheet);
    if col1 == col2 && row1 == row2 {
        format!("{sheet}!${}${row1}", col_name(col1))
    } else {
        format!(
            "{sheet}!${}${row1}:${}${row2}",
            col_name(col1),
            col_name(col2)
        )
    }
}

pub(super) fn quote_chart_sheet_always(sheet: &str) -> String {
    format!("'{}'", sheet.replace('\'', "''"))
}

pub(super) fn local_update_formula(sheet: &str, range: &str) -> String {
    format!(
        "{}!{}",
        quote_formula_sheet_if_needed(sheet),
        absolute_formula_range(range)
    )
}

pub(super) fn quote_formula_sheet_if_needed(sheet: &str) -> String {
    if is_simple_formula_sheet_name(sheet) {
        sheet.to_string()
    } else {
        format!("'{}'", sheet.replace('\'', "''"))
    }
}

pub(super) fn is_simple_formula_sheet_name(sheet: &str) -> bool {
    if sheet.is_empty() {
        return false;
    }
    for (idx, ch) in sheet.chars().enumerate() {
        if ch.is_ascii_alphabetic() || ch == '_' {
            continue;
        }
        if idx > 0 && ch.is_ascii_digit() {
            continue;
        }
        return false;
    }
    true
}

pub(super) fn absolute_formula_range(range: &str) -> String {
    let Some(normalized) = normalize_formula_range(range) else {
        return range.to_string();
    };
    normalized
        .split(':')
        .map(|cell| {
            parse_formula_cell(cell)
                .map(|mut parsed| {
                    parsed.abs_column = true;
                    parsed.abs_row = true;
                    format_formula_cell(parsed)
                })
                .unwrap_or_else(|| cell.to_string())
        })
        .collect::<Vec<_>>()
        .join(":")
}

pub(super) fn worksheet_drawing_part(
    file: &str,
    sheet_part_uri: &str,
) -> CliResult<Option<(String, String)>> {
    let sheet_xml = zip_text(file, sheet_part_uri.trim_start_matches('/'))?;
    let ids = worksheet_drawing_relationship_ids(&sheet_xml, sheet_part_uri)?;
    let Some(drawing_rid) = ids.into_iter().next() else {
        return Ok(None);
    };
    let sheet_rels = relationship_entries(file, &relationships_part_for(sheet_part_uri))?;
    let rel = sheet_rels
        .iter()
        .find(|rel| rel.id == drawing_rid)
        .ok_or_else(|| {
            CliError::unexpected(format!(
                "worksheet {sheet_part_uri} drawing relationship {drawing_rid} not found"
            ))
        })?;
    if rel.target_mode == "External" {
        return Err(CliError::unexpected(format!(
            "worksheet {sheet_part_uri} drawing relationship {drawing_rid} is external"
        )));
    }
    if rel.rel_type != REL_DRAWING {
        return Err(CliError::unexpected(format!(
            "worksheet {sheet_part_uri} relationship {drawing_rid} is {}, expected drawing",
            rel.rel_type
        )));
    }
    Ok(Some((
        resolve_relationship_target(sheet_part_uri, &rel.target),
        drawing_rid,
    )))
}

pub(super) fn ensure_drawing_xml_namespaces(root: &mut XmlNode) {
    let prefix = drawing_root_prefix(root);
    root.ensure_namespace(&prefix, NS_SPREADSHEET_DRAWING);
    root.ensure_namespace("a", NS_DRAWING_MAIN);
    root.ensure_namespace("r", NS_RELATIONSHIPS);
    root.ensure_namespace("c", NS_CHART);
}

pub(super) fn drawing_root_prefix(root: &XmlNode) -> String {
    root.namespace_prefix_for(NS_SPREADSHEET_DRAWING)
        .unwrap_or_else(|| prefix_from_qname(&root.qname).unwrap_or("xdr").to_string())
}

pub(super) fn build_drawing_part_xml(from: (u32, u32), to: (u32, u32), chart_rid: &str) -> String {
    let mut root = XmlNode::new("xdr:wsDr".to_string());
    root.set_attr("xmlns:xdr", NS_SPREADSHEET_DRAWING);
    root.set_attr("xmlns:a", NS_DRAWING_MAIN);
    root.set_attr("xmlns:r", NS_RELATIONSHIPS);
    root.set_attr("xmlns:c", NS_CHART);
    root.children
        .push(build_chart_anchor_node("xdr", from, to, chart_rid, 2, 1));
    render_xml_document(&root)
}

pub(super) fn build_chart_anchor_node(
    prefix: &str,
    from: (u32, u32),
    to: (u32, u32),
    chart_rid: &str,
    object_id: i64,
    chart_number: i64,
) -> XmlNode {
    let prefix = if prefix.trim().is_empty() {
        "xdr"
    } else {
        prefix
    };
    let mut anchor = XmlNode::new(prefixed_qname(prefix, "twoCellAnchor"));
    anchor.set_attr("editAs", "oneCell");
    anchor.children.push(build_anchor_marker_node(
        prefix,
        "from",
        from.0 - 1,
        from.1 - 1,
    ));
    anchor
        .children
        .push(build_anchor_marker_node(prefix, "to", to.0 - 1, to.1 - 1));

    let mut frame = XmlNode::new(prefixed_qname(prefix, "graphicFrame"));
    frame.set_attr("macro", "");
    let mut nv = XmlNode::new(prefixed_qname(prefix, "nvGraphicFramePr"));
    let mut c_nv_pr = XmlNode::new(prefixed_qname(prefix, "cNvPr"));
    c_nv_pr.set_attr("id", &object_id.max(1).to_string());
    c_nv_pr.set_attr("name", &format!("Chart {}", chart_number.max(1)));
    nv.children.push(c_nv_pr);
    nv.children
        .push(XmlNode::new(prefixed_qname(prefix, "cNvGraphicFramePr")));
    frame.children.push(nv);

    let mut xfrm = XmlNode::new(prefixed_qname(prefix, "xfrm"));
    let mut off = XmlNode::new("a:off".to_string());
    off.set_attr("x", "0");
    off.set_attr("y", "0");
    let mut ext = XmlNode::new("a:ext".to_string());
    ext.set_attr("cx", "0");
    ext.set_attr("cy", "0");
    xfrm.children.push(off);
    xfrm.children.push(ext);
    frame.children.push(xfrm);

    let mut graphic = XmlNode::new("a:graphic".to_string());
    let mut graphic_data = XmlNode::new("a:graphicData".to_string());
    graphic_data.set_attr("uri", NS_CHART);
    let mut chart = XmlNode::new("c:chart".to_string());
    chart.set_attr("r:id", chart_rid);
    graphic_data.children.push(chart);
    graphic.children.push(graphic_data);
    frame.children.push(graphic);
    anchor.children.push(frame);
    anchor
        .children
        .push(XmlNode::new(prefixed_qname(prefix, "clientData")));
    anchor
}

pub(super) fn build_anchor_marker_node(prefix: &str, name: &str, col0: u32, row0: u32) -> XmlNode {
    let mut marker = XmlNode::new(prefixed_qname(prefix, name));
    let mut col = XmlNode::new(prefixed_qname(prefix, "col"));
    col.text = col0.to_string();
    let mut col_off = XmlNode::new(prefixed_qname(prefix, "colOff"));
    col_off.text = "0".to_string();
    let mut row = XmlNode::new(prefixed_qname(prefix, "row"));
    row.text = row0.to_string();
    let mut row_off = XmlNode::new(prefixed_qname(prefix, "rowOff"));
    row_off.text = "0".to_string();
    marker.children.extend([col, col_off, row, row_off]);
    marker
}

pub(super) fn add_drawing_anchor(root: &mut XmlNode, anchor: XmlNode) {
    if let Some(index) = root
        .children
        .iter()
        .position(|child| child.name == "extLst")
    {
        root.children.insert(index, anchor);
    } else {
        root.children.push(anchor);
    }
}

pub(super) fn next_drawing_object_id(root: &XmlNode) -> i64 {
    descendants(root, "cNvPr")
        .into_iter()
        .filter_map(|node| {
            node.attr("id")
                .and_then(|value| value.trim().parse::<i64>().ok())
        })
        .max()
        .unwrap_or(1)
        + 1
}

pub(super) fn next_chart_number(root: &XmlNode) -> i64 {
    let mut count = 0;
    for anchor in &root.children {
        if matches!(
            anchor.name.as_str(),
            "twoCellAnchor" | "oneCellAnchor" | "absoluteAnchor"
        ) && first_descendant(anchor, "chart").is_some()
        {
            count += 1;
        }
    }
    count + 1
}

pub(super) fn add_worksheet_drawing_ref(
    xml: &str,
    sheet_part_uri: &str,
    rid: &str,
) -> CliResult<String> {
    let mut root = parse_xml_node(xml)?;
    if root.name != "worksheet" {
        return Err(CliError::unexpected(format!(
            "worksheet part {sheet_part_uri} root element not found"
        )));
    }
    if let Some(existing) = direct_child(&root, "drawing") {
        if existing.attr("id") == Some(rid) {
            return Ok(render_xml_document(&root));
        }
        return Err(CliError::unexpected(format!(
            "worksheet already has drawing relationship {}",
            existing.attr("id").unwrap_or_default()
        )));
    }
    root.ensure_namespace("r", NS_RELATIONSHIPS);
    let prefix = prefix_from_qname(&root.qname)
        .unwrap_or_default()
        .to_string();
    let mut drawing = XmlNode::new(prefixed_qname(&prefix, "drawing"));
    drawing.set_attr("r:id", rid);
    insert_child_in_order(&mut root, drawing, WORKSHEET_CHILD_ORDER);
    Ok(render_xml_document(&root))
}

pub(super) fn render_relationships_xml(relationships: &[(&str, &str, String)]) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#.to_string();
    for (id, rel_type, target) in relationships {
        xml.push_str(&format!(
            r#"<Relationship Id="{}" Type="{}" Target="{}"/>"#,
            xml_attr_escape(id),
            xml_attr_escape(rel_type),
            xml_attr_escape(target)
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

pub(super) fn xlsx_chart_create_result(
    file: &str,
    source: &ChartCreateSource,
    artifacts: &ChartCreateArtifacts,
    output: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(source.sheet));
    result.insert("sheetNumber".to_string(), json!(source.sheet_number));
    result.insert("chartType".to_string(), json!(artifacts.chart_type));
    insert_nonempty_string(&mut result, "title", &artifacts.title);
    result.insert("chartPartUri".to_string(), json!(artifacts.chart_uri));
    result.insert("drawingPartUri".to_string(), json!(artifacts.drawing_uri));
    result.insert("seriesCount".to_string(), json!(artifacts.series_count));
    result.insert("categories".to_string(), json!(artifacts.categories));
    result.insert("anchor".to_string(), json!(artifacts.anchor));
    result.insert("sourceSheet".to_string(), json!(source.sheet));
    result.insert("sourceRange".to_string(), json!(source.range));
    if !artifacts.warnings.is_empty() {
        result.insert("warnings".to_string(), json!(artifacts.warnings));
    }
    if let Some(output) = output {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    if let Some(output) = output {
        result.insert(
            "validateCommand".to_string(),
            json!(format!("ooxml validate --strict {}", command_arg(output))),
        );
        result.insert(
            "chartsListCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx charts list {}",
                command_arg(output)
            )),
        );
    }
    Value::Object(result)
}
