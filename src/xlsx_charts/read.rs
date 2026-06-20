use super::*;

pub(super) fn load_xlsx_charts(
    file: &str,
    sheet_selector: Option<&str>,
) -> CliResult<Vec<ChartRef>> {
    let workbook_xml = zip_text(file, "xl/workbook.xml")?;
    let workbook_sheets = workbook_sheets(&workbook_xml)?;
    let workbook_rels = relationship_entries(file, "xl/_rels/workbook.xml.rels")?;
    let selected_sheets = if let Some(selector) = sheet_selector.filter(|value| !value.is_empty()) {
        vec![resolve_sheet_for_chart_cli(&workbook_sheets, selector)?]
    } else {
        workbook_sheets.clone()
    };

    let mut charts = Vec::new();
    for sheet in selected_sheets {
        let Some(sheet_rel) = workbook_rels.iter().find(|rel| rel.id == sheet.rel_id) else {
            return Err(CliError::unexpected(format!(
                "missing relationship {}",
                sheet.rel_id
            )));
        };
        if sheet_rel.rel_type != REL_WORKSHEET {
            continue;
        }
        let sheet_part_uri = resolve_workbook_target_uri(&sheet_rel.target);
        let sheet_charts = list_charts_for_sheet(file, &sheet, &sheet_part_uri, charts.len() + 1)?;
        charts.extend(sheet_charts);
    }
    Ok(charts)
}

pub(super) fn resolve_sheet_for_chart_cli(
    sheets: &[WorkbookSheet],
    selector: &str,
) -> CliResult<WorkbookSheet> {
    match resolve_sheet(sheets, selector) {
        Ok(sheet) => Ok(sheet),
        Err(err) if err.message == format!("sheet not found: {selector}") => {
            let candidates = sheets
                .iter()
                .map(|sheet| {
                    let primary = format!("sheetId:{}", sheet.sheet_id);
                    let part_uri = String::new();
                    let selectors = xlsx_sheet_selectors(
                        &sheet.name,
                        sheet.sheet_id,
                        sheet.position,
                        &sheet.rel_id,
                        &part_uri,
                    );
                    (primary, selectors)
                })
                .collect::<Vec<_>>();
            let candidate_refs = candidates
                .iter()
                .map(|(primary, selectors)| (primary.as_str(), selectors.as_slice()))
                .collect::<Vec<_>>();
            let suggestions = selector_candidates(&candidate_refs, selector, 5);
            let hint = if suggestions.is_empty() {
                String::new()
            } else {
                format!("; did you mean: {}", suggestions.join(", "))
            };
            Err(CliError::target_not_found(format!(
                "sheet not found: {selector}{hint}; discover with `ooxml --json xlsx sheets list <file>`"
            )))
        }
        Err(err) => Err(err),
    }
}

pub(super) fn list_charts_for_sheet(
    file: &str,
    sheet: &WorkbookSheet,
    sheet_part_uri: &str,
    start_number: usize,
) -> CliResult<Vec<ChartRef>> {
    let sheet_xml = zip_text(file, sheet_part_uri.trim_start_matches('/'))?;
    let drawing_relationship_ids = worksheet_drawing_relationship_ids(&sheet_xml, sheet_part_uri)?;
    if drawing_relationship_ids.is_empty() {
        return Ok(Vec::new());
    }
    let sheet_rels = relationship_entries(file, &relationships_part_for(sheet_part_uri))?;
    let mut charts = Vec::new();
    for drawing_rid in drawing_relationship_ids {
        let Some(drawing_rel) = sheet_rels.iter().find(|rel| rel.id == drawing_rid) else {
            return Err(CliError::unexpected(format!(
                "worksheet {sheet_part_uri} drawing relationship {drawing_rid} not found"
            )));
        };
        if drawing_rel.target_mode == "External" {
            return Err(CliError::unexpected(format!(
                "worksheet {sheet_part_uri} drawing relationship {drawing_rid} is external"
            )));
        }
        if drawing_rel.rel_type != REL_DRAWING {
            return Err(CliError::unexpected(format!(
                "worksheet {sheet_part_uri} relationship {drawing_rid} is {}, expected drawing",
                drawing_rel.rel_type
            )));
        }
        let drawing_uri = resolve_relationship_target(sheet_part_uri, &drawing_rel.target);
        let drawing_charts = list_charts_for_drawing(
            file,
            sheet,
            sheet_part_uri,
            &drawing_rid,
            &drawing_uri,
            start_number + charts.len(),
        )?;
        charts.extend(drawing_charts);
    }
    Ok(charts)
}

pub(super) fn list_charts_for_drawing(
    file: &str,
    sheet: &WorkbookSheet,
    sheet_part_uri: &str,
    drawing_rid: &str,
    drawing_uri: &str,
    start_number: usize,
) -> CliResult<Vec<ChartRef>> {
    let drawing_xml = zip_text(file, drawing_uri.trim_start_matches('/'))?;
    let root = parse_xml_node(&drawing_xml)?;
    if root.name != "wsDr" {
        return Err(CliError::unexpected(format!(
            "drawing part {drawing_uri} root element not found"
        )));
    }
    let drawing_rels = relationship_entries(file, &relationships_part_for(drawing_uri))?;
    let mut charts = Vec::new();
    for anchor in root.children.iter().filter(|child| {
        matches!(
            child.name.as_str(),
            "twoCellAnchor" | "oneCellAnchor" | "absoluteAnchor"
        )
    }) {
        let Some(chart_elem) = first_descendant(anchor, "chart") else {
            continue;
        };
        let chart_rid = chart_elem.attr("id").ok_or_else(|| {
            CliError::unexpected(format!("drawing {drawing_uri} chart is missing r:id"))
        })?;
        let Some(chart_rel) = drawing_rels.iter().find(|rel| rel.id == chart_rid) else {
            return Err(CliError::unexpected(format!(
                "drawing {drawing_uri} chart relationship {chart_rid} not found"
            )));
        };
        if chart_rel.target_mode == "External" {
            return Err(CliError::unexpected(format!(
                "drawing {drawing_uri} chart relationship {chart_rid} is external"
            )));
        }
        if chart_rel.rel_type != REL_CHART {
            return Err(CliError::unexpected(format!(
                "drawing {drawing_uri} relationship {chart_rid} is {}, expected chart",
                chart_rel.rel_type
            )));
        }
        let chart_uri = resolve_relationship_target(drawing_uri, &chart_rel.target);
        let chart_xml = zip_text(file, chart_uri.trim_start_matches('/'))?;
        let chart_root = parse_xml_node(&chart_xml)?;
        let mut chart = read_chart_part(&chart_root, &chart_uri)?;
        chart.number = (start_number + charts.len()) as u32;
        chart.sheet = sheet.name.clone();
        chart.sheet_number = sheet.position;
        chart.sheet_part_uri = sheet_part_uri.to_string();
        chart.drawing_relationship_id = drawing_rid.to_string();
        chart.drawing_part_uri = drawing_uri.to_string();
        chart.relationship_id = chart_rid.to_string();
        chart.part_uri = chart_uri;
        chart.name = chart_name(anchor);
        chart.anchor = Some(parse_anchor(anchor));
        add_chart_selectors(&mut chart);
        chart.style = Some(inspect_chart_style(&chart_root, &chart.part_uri));
        charts.push(chart);
    }
    Ok(charts)
}

pub(super) fn worksheet_drawing_relationship_ids(
    xml: &str,
    sheet_part_uri: &str,
) -> CliResult<Vec<String>> {
    let root = parse_xml_node(xml)?;
    if root.name != "worksheet" {
        return Err(CliError::unexpected(format!(
            "worksheet part {sheet_part_uri} root element not found"
        )));
    }
    let mut ids = Vec::new();
    for drawing in root.children.iter().filter(|child| child.name == "drawing") {
        let rid = drawing.attr("id").ok_or_else(|| {
            CliError::unexpected(format!(
                "worksheet {sheet_part_uri} drawing is missing r:id"
            ))
        })?;
        ids.push(rid.to_string());
    }
    Ok(ids)
}

pub(super) fn read_chart_part(root: &XmlNode, chart_uri: &str) -> CliResult<ChartRef> {
    if root.name != "chartSpace" {
        return Err(CliError::unexpected(format!(
            "chart part {chart_uri} root element not found"
        )));
    }
    Ok(ChartRef {
        number: 0,
        sheet: String::new(),
        sheet_number: 0,
        sheet_part_uri: String::new(),
        drawing_relationship_id: String::new(),
        drawing_part_uri: String::new(),
        relationship_id: String::new(),
        part_uri: String::new(),
        name: String::new(),
        title: chart_title(root),
        types: chart_types(root),
        anchor: None,
        primary_selector: String::new(),
        selectors: Vec::new(),
        series: chart_series(root),
        style: None,
    })
}

pub(super) fn select_xlsx_chart(charts: &[ChartRef], selector: &str) -> CliResult<ChartRef> {
    if charts.is_empty() {
        return Err(CliError::invalid_args("workbook has no charts"));
    }
    let selector = selector.trim();
    if selector.is_empty() {
        if charts.len() == 1 {
            return Ok(charts[0].clone());
        }
        return Err(CliError::invalid_args(
            "--chart is required when workbook has multiple charts",
        ));
    }
    let matches = charts
        .iter()
        .filter(|chart| {
            chart
                .selectors
                .iter()
                .any(|candidate| candidate == selector)
        })
        .cloned()
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [chart] => return Ok(chart.clone()),
        [] => {}
        many => {
            let selectors = many
                .iter()
                .map(|chart| chart.primary_selector.clone())
                .collect::<Vec<_>>();
            return Err(CliError::invalid_args(format!(
                "chart selector {selector:?} matched multiple charts ({}); use a more specific selector",
                selectors.join(", ")
            )));
        }
    }
    if let Ok(number) = selector.parse::<usize>() {
        if (1..=charts.len()).contains(&number) {
            return Ok(charts[number - 1].clone());
        }
        return Err(CliError::target_not_found(format!(
            "chart {number} is out of range (1-{})",
            charts.len()
        )));
    }
    let candidates = charts
        .iter()
        .map(|chart| (chart.primary_selector.as_str(), chart.selectors.as_slice()))
        .collect::<Vec<_>>();
    let suggestions = selector_candidates(&candidates, selector, 5);
    let hint = if suggestions.is_empty() {
        String::new()
    } else {
        format!("; did you mean: {}", suggestions.join(", "))
    };
    Err(CliError::target_not_found(format!(
        "chart not found: {selector}{hint}; discover with `ooxml --json xlsx charts list <file>`"
    )))
}

pub(super) fn add_chart_selectors(chart: &mut ChartRef) {
    chart.primary_selector = if chart.number > 0 {
        format!("chart:{}", chart.number)
    } else if !chart.name.trim().is_empty() {
        format!("chart:{}", chart.name)
    } else {
        String::new()
    };
    let mut selectors = Vec::new();
    add_selector(&mut selectors, chart.primary_selector.clone());
    if chart.number > 0 {
        add_selector(&mut selectors, format!("chart:{}", chart.number));
        add_selector(&mut selectors, format!("#{}", chart.number));
    }
    if !chart.name.trim().is_empty() {
        add_selector(&mut selectors, format!("chart:{}", chart.name));
        add_selector(&mut selectors, format!("name:{}", chart.name));
        add_selector(&mut selectors, format!("~{}", chart.name));
        add_selector(&mut selectors, chart.name.clone());
    }
    if !chart.relationship_id.trim().is_empty() {
        add_selector(&mut selectors, format!("rId:{}", chart.relationship_id));
        add_selector(&mut selectors, format!("rid:{}", chart.relationship_id));
    }
    if !chart.drawing_relationship_id.trim().is_empty() {
        add_selector(
            &mut selectors,
            format!("drawingRid:{}", chart.drawing_relationship_id),
        );
    }
    if !chart.part_uri.trim().is_empty() {
        add_selector(&mut selectors, format!("part:{}", chart.part_uri));
    }
    chart.selectors = selectors;
}

pub(super) fn xlsx_chart_item(file: &str, chart: &ChartRef) -> Value {
    let mut item = Map::new();
    item.insert("number".to_string(), json!(chart.number));
    item.insert("sheet".to_string(), json!(chart.sheet));
    item.insert("sheetNumber".to_string(), json!(chart.sheet_number));
    item.insert("sheetPartUri".to_string(), json!(chart.sheet_part_uri));
    item.insert(
        "drawingRelationshipId".to_string(),
        json!(chart.drawing_relationship_id),
    );
    item.insert("drawingPartUri".to_string(), json!(chart.drawing_part_uri));
    item.insert("relationshipId".to_string(), json!(chart.relationship_id));
    item.insert("partUri".to_string(), json!(chart.part_uri));
    insert_nonempty_string(&mut item, "name", &chart.name);
    insert_nonempty_string(&mut item, "title", &chart.title);
    insert_nonempty_array(
        &mut item,
        "types",
        chart.types.iter().map(|v| json!(v)).collect(),
    );
    if let Some(anchor) = &chart.anchor {
        item.insert("anchor".to_string(), anchor_json(anchor));
    }
    insert_nonempty_string(&mut item, "primarySelector", &chart.primary_selector);
    insert_nonempty_array(
        &mut item,
        "selectors",
        chart.selectors.iter().map(|v| json!(v)).collect(),
    );
    insert_nonempty_array(
        &mut item,
        "series",
        chart.series.iter().map(series_json).collect(),
    );
    item.insert(
        "showCommand".to_string(),
        json!(xlsx_chart_show_command(file, chart)),
    );
    insert_nonempty_array(
        &mut item,
        "sourceExportCommands",
        xlsx_chart_source_export_commands(file, chart),
    );
    if let Some(style) = &chart.style {
        item.insert("style".to_string(), style.clone());
    }
    Value::Object(item)
}

pub(super) fn xlsx_chart_item_for_update(file: Option<&str>, chart: &ChartRef) -> Value {
    let mut item = xlsx_chart_item(file.unwrap_or_default(), chart);
    if file.is_none()
        && let Some(object) = item.as_object_mut()
    {
        object.remove("showCommand");
        object.remove("sourceExportCommands");
    }
    item
}

pub(super) fn xlsx_chart_source_export_commands(file: &str, chart: &ChartRef) -> Vec<Value> {
    let mut commands = Vec::new();
    for series in &chart.series {
        for (role, source) in chart_series_sources(series) {
            let Some(source) = source else {
                continue;
            };
            if source.sheet.is_empty() || source.range.is_empty() {
                continue;
            }
            commands.push(json!({
                "series": series.number,
                "role": role,
                "formula": source.formula,
                "sheet": source.sheet,
                "range": source.range,
                "rangesExportCommand": xlsx_ranges_export_command(file, &source.sheet, &source.range),
            }));
        }
    }
    commands
}

pub(super) fn chart_series_sources(
    series: &ChartSeries,
) -> Vec<(&'static str, Option<&ChartDataSource>)> {
    vec![
        ("name", series.name.as_ref()),
        ("categories", series.categories.as_ref()),
        ("values", series.values.as_ref()),
        ("xValues", series.x_values.as_ref()),
        ("yValues", series.y_values.as_ref()),
        ("bubbleSize", series.bubble_size.as_ref()),
    ]
}

pub(super) fn xlsx_chart_show_command(file: &str, chart: &ChartRef) -> String {
    let mut args = vec![
        "ooxml".to_string(),
        "--json".to_string(),
        "xlsx".to_string(),
        "charts".to_string(),
        "show".to_string(),
        command_arg(file),
    ];
    if !chart.sheet.trim().is_empty() {
        args.push("--sheet".to_string());
        args.push(command_arg(&chart.sheet));
    }
    let selector = if !chart.primary_selector.trim().is_empty() {
        chart.primary_selector.as_str()
    } else if !chart.name.trim().is_empty() {
        chart.name.as_str()
    } else {
        "1"
    };
    args.push("--chart".to_string());
    args.push(command_arg(selector));
    args.join(" ")
}

pub(super) fn xlsx_ranges_export_command(file: &str, sheet: &str, range: &str) -> String {
    format!(
        "ooxml --json xlsx ranges export {} --sheet {} --range {} --include-types",
        command_arg(file),
        command_arg(sheet),
        command_arg(range)
    )
}

pub(super) fn anchor_json(anchor: &ChartAnchor) -> Value {
    let mut object = Map::new();
    object.insert("type".to_string(), json!(anchor.kind));
    if let Some(marker) = &anchor.from {
        object.insert("from".to_string(), marker_json(marker));
    }
    if let Some(marker) = &anchor.to {
        object.insert("to".to_string(), marker_json(marker));
    }
    Value::Object(object)
}

pub(super) fn marker_json(marker: &ChartMarker) -> Value {
    let mut object = Map::new();
    object.insert("column".to_string(), json!(marker.column));
    insert_nonzero_i64(&mut object, "columnOffset", marker.column_offset);
    object.insert("row".to_string(), json!(marker.row));
    insert_nonzero_i64(&mut object, "rowOffset", marker.row_offset);
    Value::Object(object)
}

pub(super) fn series_json(series: &ChartSeries) -> Value {
    let mut object = Map::new();
    object.insert("number".to_string(), json!(series.number));
    insert_nonzero_i64(&mut object, "index", series.index);
    insert_nonzero_i64(&mut object, "order", series.order);
    if let Some(source) = &series.name {
        object.insert("name".to_string(), data_source_json(source));
    }
    if let Some(source) = &series.categories {
        object.insert("categories".to_string(), data_source_json(source));
    }
    if let Some(source) = &series.values {
        object.insert("values".to_string(), data_source_json(source));
    }
    if let Some(source) = &series.x_values {
        object.insert("xValues".to_string(), data_source_json(source));
    }
    if let Some(source) = &series.y_values {
        object.insert("yValues".to_string(), data_source_json(source));
    }
    if let Some(source) = &series.bubble_size {
        object.insert("bubbleSize".to_string(), data_source_json(source));
    }
    Value::Object(object)
}

pub(super) fn data_source_json(source: &ChartDataSource) -> Value {
    let mut object = Map::new();
    insert_nonempty_string(&mut object, "formula", &source.formula);
    insert_nonempty_string(&mut object, "sheet", &source.sheet);
    insert_nonempty_string(&mut object, "range", &source.range);
    insert_nonempty_string(&mut object, "refKind", &source.ref_kind);
    insert_nonempty_string(&mut object, "cacheType", &source.cache_type);
    insert_nonzero_i64(&mut object, "pointCount", source.point_count);
    insert_nonempty_array(
        &mut object,
        "cachePreview",
        source.cache_preview.iter().map(|v| json!(v)).collect(),
    );
    Value::Object(object)
}

pub(super) fn chart_name(anchor: &XmlNode) -> String {
    let Some(frame) = first_descendant(anchor, "graphicFrame") else {
        return String::new();
    };
    descendants(frame, "cNvPr")
        .into_iter()
        .find_map(|node| node.attr("name").map(str::trim).filter(|v| !v.is_empty()))
        .unwrap_or_default()
        .to_string()
}

pub(super) fn parse_anchor(anchor: &XmlNode) -> ChartAnchor {
    ChartAnchor {
        kind: anchor.name.clone(),
        from: direct_child(anchor, "from").map(parse_marker),
        to: direct_child(anchor, "to").map(parse_marker),
    }
}

pub(super) fn parse_marker(marker: &XmlNode) -> ChartMarker {
    ChartMarker {
        column: parse_child_i64(marker, "col"),
        column_offset: parse_child_i64(marker, "colOff"),
        row: parse_child_i64(marker, "row"),
        row_offset: parse_child_i64(marker, "rowOff"),
    }
}

pub(super) fn chart_title(root: &XmlNode) -> String {
    first_descendant(root, "title")
        .map(title_text)
        .unwrap_or_default()
}

pub(super) fn chart_types(root: &XmlNode) -> Vec<String> {
    let Some(plot_area) = first_descendant(root, "plotArea") else {
        return Vec::new();
    };
    let mut seen = Vec::<String>::new();
    for child in &plot_area.children {
        if child.name.ends_with("Chart") && !seen.iter().any(|name| name == &child.name) {
            seen.push(child.name.clone());
        }
    }
    seen
}

pub(super) fn chart_series(root: &XmlNode) -> Vec<ChartSeries> {
    walk_series(root)
        .into_iter()
        .enumerate()
        .map(|(idx, ser)| ChartSeries {
            number: idx as u32 + 1,
            index: direct_child(ser, "idx").and_then(attr_val_i64).unwrap_or(0),
            order: direct_child(ser, "order")
                .and_then(attr_val_i64)
                .unwrap_or(0),
            name: chart_data_source(direct_child(ser, "tx")),
            categories: chart_data_source(direct_child(ser, "cat")),
            values: chart_data_source(direct_child(ser, "val")),
            x_values: chart_data_source(direct_child(ser, "xVal")),
            y_values: chart_data_source(direct_child(ser, "yVal")),
            bubble_size: chart_data_source(direct_child(ser, "bubbleSize")),
        })
        .collect()
}

pub(super) fn walk_series(root: &XmlNode) -> Vec<&XmlNode> {
    let Some(plot_area) = first_descendant(root, "plotArea") else {
        return Vec::new();
    };
    let mut series = Vec::new();
    for chart_type in &plot_area.children {
        if !chart_type.name.ends_with("Chart") {
            continue;
        }
        series.extend(
            chart_type
                .children
                .iter()
                .filter(|child| child.name == "ser"),
        );
    }
    series
}

pub(super) fn chart_data_source(elem: Option<&XmlNode>) -> Option<ChartDataSource> {
    let elem = elem?;
    let source = ["strRef", "numRef", "multiLvlStrRef"]
        .iter()
        .find_map(|name| first_descendant(elem, name));
    if let Some(source) = source {
        let mut result = ChartDataSource {
            formula: String::new(),
            sheet: String::new(),
            range: String::new(),
            ref_kind: source.name.clone(),
            cache_type: String::new(),
            point_count: 0,
            cache_preview: Vec::new(),
        };
        if let Some(formula) = direct_child(source, "f").map(node_text_trimmed) {
            result.formula = formula;
            let (sheet, range) = split_sheet_range_formula(&result.formula);
            result.sheet = sheet;
            result.range = range;
        }
        if let Some(cache) = first_cache_child(source) {
            result.cache_type = cache.name.clone();
            result.point_count = direct_child(cache, "ptCount")
                .and_then(attr_val_i64)
                .unwrap_or(0);
            for point in descendants(cache, "pt") {
                if result.cache_preview.len() >= 5 {
                    break;
                }
                if let Some(value) = direct_child(point, "v").map(node_text) {
                    result.cache_preview.push(value);
                }
            }
        }
        if result.formula.is_empty() && result.point_count == 0 && result.cache_preview.is_empty() {
            None
        } else {
            Some(result)
        }
    } else {
        direct_child(elem, "v").map(|value| ChartDataSource {
            formula: String::new(),
            sheet: String::new(),
            range: String::new(),
            ref_kind: String::new(),
            cache_type: "literal".to_string(),
            point_count: 0,
            cache_preview: vec![node_text(value)],
        })
    }
}

pub(super) fn first_cache_child(elem: &XmlNode) -> Option<&XmlNode> {
    elem.children.iter().find(|child| {
        matches!(
            child.name.as_str(),
            "strCache" | "numCache" | "multiLvlStrCache"
        )
    })
}

pub(super) fn inspect_chart_style(root: &XmlNode, chart_uri: &str) -> Value {
    let mut style = Map::new();
    style.insert("partUri".to_string(), json!(chart_uri));
    insert_nonempty_array(
        &mut style,
        "types",
        chart_types(root).into_iter().map(Value::String).collect(),
    );
    let chart = direct_child(root, "chart");
    style.insert(
        "title".to_string(),
        chart
            .and_then(|node| direct_child(node, "title"))
            .map(inspect_title)
            .unwrap_or_else(|| json!({"present": false})),
    );
    style.insert(
        "legend".to_string(),
        chart
            .and_then(|node| direct_child(node, "legend"))
            .map(inspect_legend)
            .unwrap_or_else(|| json!({"present": false})),
    );
    if let Some(plot_area) = first_descendant(root, "plotArea") {
        insert_nonempty_array(
            &mut style,
            "axes",
            inspect_axes(plot_area).into_iter().collect(),
        );
        insert_nonempty_string_value(
            &mut style,
            "plotAreaFill",
            direct_child(plot_area, "spPr")
                .map(inspect_fill)
                .unwrap_or_default(),
        );
    }
    insert_nonempty_string_value(
        &mut style,
        "chartSpaceFill",
        direct_child(root, "spPr")
            .map(inspect_fill)
            .unwrap_or_default(),
    );
    insert_nonempty_array(
        &mut style,
        "series",
        walk_series(root)
            .into_iter()
            .enumerate()
            .map(|(index, series)| inspect_series_style(series, index + 1))
            .collect(),
    );
    Value::Object(style)
}

pub(super) fn inspect_title(title: &XmlNode) -> Value {
    let mut object = Map::new();
    object.insert("present".to_string(), json!(true));
    if direct_child(title, "tx")
        .and_then(|tx| direct_child(tx, "strRef"))
        .is_some()
    {
        object.insert("linked".to_string(), json!(true));
    }
    insert_nonempty_string_value(&mut object, "text", title_text(title));
    if let Some(overlay) = direct_child(title, "overlay") {
        object.insert(
            "overlay".to_string(),
            json!(parse_ooxml_bool(overlay.attr("val").unwrap_or_default())),
        );
    }
    if let Some(font) = inspect_title_font(title) {
        object.insert("font".to_string(), font);
    }
    Value::Object(object)
}

pub(super) fn title_text(title: &XmlNode) -> String {
    let mut parts = descendants(title, "t")
        .into_iter()
        .map(node_text)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        parts = descendants(title, "v")
            .into_iter()
            .map(node_text)
            .collect::<Vec<_>>();
    }
    parts.join("").trim().to_string()
}

pub(super) fn inspect_title_font(title: &XmlNode) -> Option<Value> {
    let mut candidates = Vec::new();
    if let Some(rich) = first_descendant(title, "rich") {
        if let Some(run) = first_descendant(rich, "r") {
            candidates.push(direct_child(run, "rPr"));
        }
        if let Some(p_pr) = first_descendant(rich, "pPr") {
            candidates.push(direct_child(p_pr, "defRPr"));
        }
    }
    if let Some(tx_pr) = direct_child(title, "txPr")
        && let Some(p_pr) = first_descendant(tx_pr, "pPr")
    {
        candidates.push(direct_child(p_pr, "defRPr"));
    }
    candidates.into_iter().flatten().find_map(inspect_font)
}

pub(super) fn inspect_axis_tick_label_font(axis: &XmlNode) -> Option<Value> {
    let tx_pr = direct_child(axis, "txPr")?;
    let mut candidates = Vec::new();
    if let Some(p_pr) = first_descendant(tx_pr, "pPr") {
        candidates.push(direct_child(p_pr, "defRPr"));
    }
    if let Some(run) = first_descendant(tx_pr, "r") {
        candidates.push(direct_child(run, "rPr"));
    }
    candidates.into_iter().flatten().find_map(inspect_font)
}

pub(super) fn inspect_font(r_pr: &XmlNode) -> Option<Value> {
    let mut object = Map::new();
    if let Some(size) = r_pr.attr("sz").and_then(|value| value.parse::<f64>().ok()) {
        object.insert("sizePt".to_string(), json_f64(size / 100.0));
    }
    if let Some(value) = r_pr.attr("b") {
        object.insert("bold".to_string(), json!(parse_ooxml_bool(value)));
    }
    if let Some(value) = r_pr.attr("i") {
        object.insert("italic".to_string(), json!(parse_ooxml_bool(value)));
    }
    if let Some(latin) = direct_child(r_pr, "latin")
        && let Some(family) = latin.attr("typeface")
        && !family.trim().is_empty()
    {
        object.insert("family".to_string(), json!(family.trim()));
    }
    let color = inspect_fill(r_pr);
    insert_nonempty_string_value(&mut object, "color", color);
    if object.is_empty() {
        None
    } else {
        Some(Value::Object(object))
    }
}

pub(super) fn inspect_legend(legend: &XmlNode) -> Value {
    let mut object = Map::new();
    object.insert("present".to_string(), json!(true));
    if let Some(pos) = direct_child(legend, "legendPos").and_then(|node| node.attr("val")) {
        insert_nonempty_string(&mut object, "position", pos.trim());
    }
    if let Some(overlay) = direct_child(legend, "overlay") {
        object.insert(
            "overlay".to_string(),
            json!(parse_ooxml_bool(overlay.attr("val").unwrap_or_default())),
        );
    }
    Value::Object(object)
}

pub(super) fn inspect_axes(plot_area: &XmlNode) -> Vec<Value> {
    let mut axes = Vec::new();
    for child in &plot_area.children {
        if !matches!(child.name.as_str(), "catAx" | "valAx" | "dateAx" | "serAx") {
            continue;
        }
        let mut axis = Map::new();
        axis.insert("element".to_string(), json!(child.name));
        axis.insert("kind".to_string(), json!(axis_kind(&child.name)));
        if let Some(id) = direct_child(child, "axId").and_then(|node| node.attr("val")) {
            insert_nonempty_string(&mut axis, "axisId", id.trim());
        }
        if let Some(delete) = direct_child(child, "delete") {
            axis.insert(
                "hidden".to_string(),
                json!(parse_ooxml_bool(delete.attr("val").unwrap_or_default())),
            );
        }
        if let Some(title) = direct_child(child, "title") {
            insert_nonempty_string_value(&mut axis, "title", title_text(title));
            if let Some(font) = inspect_title_font(title) {
                axis.insert("titleFont".to_string(), font);
            }
        }
        if let Some(format) = direct_child(child, "numFmt").and_then(|node| node.attr("formatCode"))
        {
            insert_nonempty_string(&mut axis, "numberFormat", format.trim());
        }
        if let Some(scaling) = direct_child(child, "scaling") {
            if let Some(min) = direct_child(scaling, "min").and_then(attr_val_f64) {
                axis.insert("min".to_string(), json_f64(min));
            }
            if let Some(max) = direct_child(scaling, "max").and_then(attr_val_f64) {
                axis.insert("max".to_string(), json_f64(max));
            }
        }
        if let Some(unit) = direct_child(child, "majorUnit").and_then(attr_val_f64) {
            axis.insert("majorUnit".to_string(), json_f64(unit));
        }
        axis.insert(
            "majorGridlines".to_string(),
            json!(direct_child(child, "majorGridlines").is_some()),
        );
        axis.insert(
            "minorGridlines".to_string(),
            json!(direct_child(child, "minorGridlines").is_some()),
        );
        if let Some(font) = inspect_axis_tick_label_font(child) {
            axis.insert("tickLabelFont".to_string(), font);
        }
        axes.push(Value::Object(axis));
    }
    axes
}

pub(super) fn inspect_series_style(series: &XmlNode, number: usize) -> Value {
    let mut object = Map::new();
    object.insert("number".to_string(), json!(number));
    if let Some(tx) = direct_child(series, "tx") {
        insert_nonempty_string_value(&mut object, "name", series_name_text(tx));
    }
    if let Some(sp_pr) = direct_child(series, "spPr") {
        if direct_child(sp_pr, "noFill").is_some() {
            object.insert("noFill".to_string(), json!(true));
        } else {
            insert_nonempty_string_value(&mut object, "fillColor", inspect_fill(sp_pr));
        }
        if let Some(line) = direct_child(sp_pr, "ln") {
            if direct_child(line, "noFill").is_some() {
                object.insert("noLine".to_string(), json!(true));
            } else {
                insert_nonempty_string_value(&mut object, "lineColor", inspect_fill(line));
            }
            if let Some(width) = line.attr("w").and_then(|value| value.parse::<f64>().ok()) {
                object.insert("lineWidthPt".to_string(), json_f64(width / 12700.0));
            }
        }
    }
    if let Some(marker) = direct_child(series, "marker")
        && let Some(marker_json) = inspect_marker(marker)
    {
        object.insert("marker".to_string(), marker_json);
    }
    Value::Object(object)
}

pub(super) fn inspect_marker(marker: &XmlNode) -> Option<Value> {
    let mut object = Map::new();
    if let Some(symbol) = direct_child(marker, "symbol").and_then(|node| node.attr("val")) {
        insert_nonempty_string(&mut object, "symbol", symbol.trim());
    }
    if let Some(size) = direct_child(marker, "size").and_then(attr_val_i64) {
        object.insert("size".to_string(), json!(size));
    }
    if object.is_empty() {
        None
    } else {
        Some(Value::Object(object))
    }
}

pub(super) fn inspect_fill(holder: &XmlNode) -> String {
    let Some(solid) = direct_child(holder, "solidFill") else {
        return String::new();
    };
    if let Some(srgb) = direct_child(solid, "srgbClr")
        && let Some(value) = srgb.attr("val")
    {
        return value.trim().to_ascii_uppercase();
    }
    if let Some(scheme) = direct_child(solid, "schemeClr")
        && let Some(value) = scheme.attr("val")
    {
        return format!("scheme:{}", value.trim());
    }
    String::new()
}
