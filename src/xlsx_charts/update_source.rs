use super::*;

pub(super) fn normalize_chart_source_role(value: &str) -> CliResult<ChartSourceRole> {
    match value.trim().to_ascii_lowercase().as_str() {
        "name" | "tx" | "series-name" | "seriesname" => Ok(ChartSourceRole {
            canonical: "name".to_string(),
            element: "tx",
        }),
        "categories" | "category" | "cat" | "cats" => Ok(ChartSourceRole {
            canonical: "categories".to_string(),
            element: "cat",
        }),
        "values" | "value" | "val" | "vals" => Ok(ChartSourceRole {
            canonical: "values".to_string(),
            element: "val",
        }),
        "xvalues" | "x" | "xval" | "x-val" | "x-values" => Ok(ChartSourceRole {
            canonical: "xValues".to_string(),
            element: "xVal",
        }),
        "yvalues" | "y" | "yval" | "y-val" | "y-values" => Ok(ChartSourceRole {
            canonical: "yValues".to_string(),
            element: "yVal",
        }),
        "bubblesize" | "bubble" | "bubble-size" => Ok(ChartSourceRole {
            canonical: "bubbleSize".to_string(),
            element: "bubbleSize",
        }),
        _ => Err(CliError::invalid_args(format!(
            "invalid chart source role {value:?} (must be name, categories, values, xValues, yValues, or bubbleSize)"
        ))),
    }
}

pub(super) fn normalize_chart_cache_mode(value: &str) -> CliResult<ChartCacheMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(ChartCacheMode::Auto),
        "clear" => Ok(ChartCacheMode::Clear),
        "keep" => Ok(ChartCacheMode::Keep),
        _ => Err(CliError::invalid_args(
            "--cache must be auto, clear, or keep",
        )),
    }
}

pub(super) fn chart_series_source_by_role(
    chart: &ChartRef,
    series_number: usize,
    role: &ChartSourceRole,
) -> CliResult<ChartDataSource> {
    if series_number == 0 || series_number > chart.series.len() {
        return Err(CliError::invalid_args(format!(
            "series {series_number} is out of range (1-{})",
            chart.series.len()
        )));
    }
    let series = &chart.series[series_number - 1];
    let source = match role.canonical.as_str() {
        "name" => series.name.as_ref(),
        "categories" => series.categories.as_ref(),
        "values" => series.values.as_ref(),
        "xValues" => series.x_values.as_ref(),
        "yValues" => series.y_values.as_ref(),
        "bubbleSize" => series.bubble_size.as_ref(),
        _ => None,
    };
    let source = source.ok_or_else(|| {
        CliError::invalid_args(format!(
            "series {series_number} has no {} source",
            role.canonical
        ))
    })?;
    if source.ref_kind.is_empty() {
        return Err(CliError::invalid_args(format!(
            "series {series_number} {} source is not a cell reference",
            role.canonical
        )));
    }
    Ok(source.clone())
}

pub(super) fn resolve_chart_update_source(
    file: &str,
    sheets: &[WorkbookSheet],
    current_source: &ChartDataSource,
    options: &XlsxChartUpdateSourceOptions<'_>,
) -> CliResult<ResolvedChartUpdateSource> {
    if let Some(formula) = options.formula.filter(|value| !value.trim().is_empty()) {
        let (sheet, range) = parse_local_range_formula(formula).ok_or_else(|| {
            CliError::invalid_args(
                "--formula must be a simple local worksheet A1 range such as Data!$B$2:$B$10",
            )
        })?;
        let bounds = parse_range(&range)
            .map_err(|err| {
                CliError::invalid_args(format!("invalid --formula range: {}", err.message))
            })?
            .normalized();
        let sheet_ref = resolve_sheet_for_chart_cli(sheets, &sheet)?;
        return Ok(ResolvedChartUpdateSource {
            formula: local_update_formula(&sheet_ref.name, &range),
            sheet: sheet_ref.name,
            range,
            bounds,
        });
    }

    let source_range = options.source_range.unwrap_or_default().trim();
    let bounds = parse_range(source_range)
        .map_err(|err| CliError::invalid_args(format!("invalid --source-range: {}", err.message)))?
        .normalized();
    let range = normalize_formula_range(source_range)
        .ok_or_else(|| CliError::invalid_args("invalid --source-range"))?;
    let mut sheet = options.source_sheet.unwrap_or_default().trim().to_string();
    if sheet.is_empty() {
        sheet = current_source.sheet.clone();
    }
    if sheet.is_empty() {
        return Err(CliError::invalid_args(
            "--source-sheet is required when the current chart source has no local worksheet sheet",
        ));
    }
    let workbook_xml = zip_text(file, "xl/workbook.xml")?;
    let workbook_sheets = workbook_sheets(&workbook_xml)?;
    let sheet_ref = resolve_sheet_for_chart_cli(&workbook_sheets, &sheet)?;
    Ok(ResolvedChartUpdateSource {
        formula: local_update_formula(&sheet_ref.name, &range),
        sheet: sheet_ref.name,
        range,
        bounds,
    })
}

pub(super) fn parse_local_range_formula(formula: &str) -> Option<(String, String)> {
    let (sheet, range) = split_sheet_range_formula(formula);
    if sheet.is_empty() || range.is_empty() {
        None
    } else {
        Some((sheet, range))
    }
}

pub(super) fn collect_chart_cache_points(
    file: &str,
    sheet: &str,
    range: &str,
    bounds: RangeBounds,
    ref_kind: &str,
    max_cells: i64,
) -> CliResult<ChartCacheUpdate> {
    check_range_max_cells(range, bounds, max_cells)?;
    let exported = xlsx_range_export_with_options(
        file,
        sheet,
        range,
        XlsxRangeExportOptions {
            include_types: true,
            include_formulas: true,
            include_formats: true,
            data_out: None,
            max_cells,
        },
    )?;
    let cells =
        chart_cells_from_range_export(&exported, xlsx_workbook_waiting_for_formula_recalc(file)?)?;
    let mut points = Vec::new();
    let mut skipped = 0_i64;
    let mut flat_index = 0_i64;
    let mut format_counts = BTreeMap::<String, i64>::new();
    for row in cells {
        for cell in row {
            if let Some(value) = chart_cache_value_from_cell(&cell, ref_kind) {
                if ref_kind == "numRef" && !cell.number_format_code.is_empty() {
                    *format_counts
                        .entry(cell.number_format_code.clone())
                        .or_default() += 1;
                }
                points.push(ChartCachePoint {
                    index: flat_index,
                    value,
                });
            } else {
                skipped += 1;
            }
            flat_index += 1;
        }
    }
    if ref_kind == "numRef" && points.is_empty() {
        return Err(CliError::invalid_args(format!(
            "source range {range} has no numeric values for a numeric chart source"
        )));
    }
    let mut warnings = Vec::new();
    if skipped > 0 {
        warnings.push(format!(
            "skipped {skipped} source cell(s) that could not be represented in the {ref_kind} chart cache"
        ));
    }
    Ok(ChartCacheUpdate {
        points,
        skipped,
        format_code: dominant_format_code(&format_counts),
        warnings,
    })
}

pub(super) fn chart_cache_value_from_cell(
    cell: &ChartSourceCell,
    ref_kind: &str,
) -> Option<String> {
    if ref_kind == "numRef" {
        if cell.kind != "number" && cell.kind != "date" {
            return None;
        }
        let value = cell.value.trim();
        if value.is_empty() || value.parse::<f64>().is_err() {
            return None;
        }
        return Some(value.to_string());
    }
    if cell.kind == "error" {
        None
    } else {
        Some(cell.value.clone())
    }
}

pub(super) fn dominant_format_code(counts: &BTreeMap<String, i64>) -> String {
    let mut best = String::new();
    let mut best_count = 0_i64;
    for (format, count) in counts {
        if *count > best_count || (*count == best_count && (best.is_empty() || format < &best)) {
            best = format.clone();
            best_count = *count;
        }
    }
    best
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_chart_update_source(
    root: &mut XmlNode,
    ctx: &ChartXmlContext,
    series_number: usize,
    role: &ChartSourceRole,
    formula: &str,
    cache_mode: ChartCacheMode,
    cache_update: &ChartCacheUpdate,
    expect_formula: Option<&str>,
    expect_source_range: Option<&str>,
) -> CliResult<ChartSourceMutation> {
    if first_descendant(root, "pivotSource").is_some() {
        return Err(CliError::invalid_args(
            "failed to update chart source: pivot-backed chart sources are not supported",
        ));
    }
    let series_paths = series_node_paths(root);
    if series_number == 0 || series_number > series_paths.len() {
        return Err(CliError::invalid_args(format!(
            "failed to update chart source: series {series_number} is out of range (1-{})",
            series_paths.len()
        )));
    }
    let (chart_type_index, series_index) = series_paths[series_number - 1];
    let plot_area = first_descendant_mut(root, "plotArea")
        .ok_or_else(|| CliError::unexpected("chart part has no plotArea"))?;
    let series = &mut plot_area.children[chart_type_index].children[series_index];
    let role_index = child_index(series, role.element).ok_or_else(|| {
        CliError::invalid_args(format!(
            "failed to update chart source: series {series_number} has no {} source (available roles: {})",
            role.canonical,
            series_roles(series).join(", ")
        ))
    })?;
    let role_elem = &mut series.children[role_index];
    let (source_index, ref_kind) = source_ref_child_index(role_elem).map_err(|message| {
        CliError::invalid_args(format!("failed to update chart source: {message}"))
    })?;
    if ref_kind == "multiLvlStrRef" {
        return Err(CliError::invalid_args(
            "failed to update chart source: multi-level category sources are not supported",
        ));
    }
    let source_ref = &mut role_elem.children[source_index];
    let formula_index = child_index(source_ref, "f");
    let previous_formula = formula_index
        .map(|index| node_text_trimmed(&source_ref.children[index]))
        .unwrap_or_default();
    check_expected_chart_source(&previous_formula, expect_formula, expect_source_range)?;
    let formula_index = if let Some(index) = formula_index {
        index
    } else {
        source_ref.children.insert(0, XmlNode::new(ctx.c("f")));
        0
    };
    source_ref.children[formula_index].text = normalize_formula_text(formula);

    let cache_index = source_ref.children.iter().position(|child| {
        matches!(
            child.name.as_str(),
            "strCache" | "numCache" | "multiLvlStrCache"
        )
    });
    let mut cache_type = String::new();
    let mut cache_point_count = 0_i64;
    let mut cache_preview = Vec::new();
    match cache_mode {
        ChartCacheMode::Clear => {
            if let Some(index) = cache_index {
                source_ref.children.remove(index);
            }
        }
        ChartCacheMode::Keep => {
            if let Some(index) = cache_index {
                let cache = &source_ref.children[index];
                cache_type = cache.name.clone();
                cache_point_count = cache_point_count_from_node(cache);
                cache_preview = cache_preview_from_node(cache, 5);
            }
        }
        ChartCacheMode::Auto => {
            if let Some(index) = cache_index {
                source_ref.children.remove(index);
            }
            cache_type = if ref_kind == "numRef" {
                "numCache".to_string()
            } else {
                "strCache".to_string()
            };
            let cache = build_cache_node(ctx, &cache_type, cache_update);
            let insert_at = child_index(source_ref, "f")
                .map(|index| index + 1)
                .unwrap_or(0);
            source_ref.children.insert(insert_at, cache);
            cache_point_count = cache_update.points.len() as i64;
            cache_preview = cache_update
                .points
                .iter()
                .take(5)
                .map(|point| point.value.clone())
                .collect();
        }
    }
    let sibling_counts = sibling_point_counts(series);
    let mut warnings = cache_update.warnings.clone();
    if cache_mode == ChartCacheMode::Keep {
        warnings.push(
            "stored chart cache was kept from the previous source and may not match the updated formula"
                .to_string(),
        );
    }
    if cache_mode == ChartCacheMode::Clear {
        warnings.push(
            "stored chart cache was removed; spreadsheet applications may refresh it on open"
                .to_string(),
        );
    }
    let edited_count = sibling_counts.get(&role.canonical).copied().unwrap_or(0);
    if comparable_point_role(&role.canonical) && edited_count > 0 {
        for (sibling_role, count) in sibling_counts {
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
    Ok(ChartSourceMutation {
        previous_formula,
        formula: normalize_formula_text(formula),
        ref_kind,
        cache_type,
        cache_point_count,
        cache_preview,
        cache_skipped: cache_update.skipped,
        warnings: unique_sorted_warnings(&warnings),
    })
}

pub(super) fn source_ref_child_index(role_elem: &XmlNode) -> Result<(usize, String), String> {
    for name in ["numRef", "strRef", "multiLvlStrRef"] {
        if let Some(index) = child_index(role_elem, name) {
            return Ok((index, name.to_string()));
        }
    }
    if direct_child(role_elem, "v").is_some() {
        return Err(
            "series source is a literal value, not a cell reference; setting literal chart sources is not supported"
                .to_string(),
        );
    }
    Err("series source has no supported reference".to_string())
}

pub(super) fn check_expected_chart_source(
    previous_formula: &str,
    expect_formula: Option<&str>,
    expect_range: Option<&str>,
) -> CliResult<()> {
    if let Some(expected) = expect_formula.filter(|value| !value.trim().is_empty()) {
        let expected = normalize_formula_text(expected);
        if normalize_formula_text(previous_formula) != expected {
            return Err(CliError::invalid_args(format!(
                "failed to update chart source: chart source formula mismatch: expected {expected} but found {previous_formula}"
            )));
        }
    }
    if let Some(expect_range) = expect_range.filter(|value| !value.trim().is_empty()) {
        let (_, current_range) = split_sheet_range_formula(previous_formula);
        if current_range.is_empty() {
            return Err(CliError::invalid_args(format!(
                "failed to update chart source: current chart source formula {previous_formula:?} is not a supported local A1 range"
            )));
        }
        let expected_range = normalize_formula_range(expect_range).ok_or_else(|| {
            CliError::invalid_args(format!(
                "failed to update chart source: invalid expected source range {expect_range:?}"
            ))
        })?;
        if current_range != expected_range {
            return Err(CliError::invalid_args(format!(
                "failed to update chart source: chart source range mismatch: expected {expected_range} but found {current_range}"
            )));
        }
    }
    Ok(())
}

pub(super) fn normalize_formula_text(value: &str) -> String {
    value.trim().trim_start_matches('=').trim().to_string()
}

pub(super) fn build_cache_node(
    ctx: &ChartXmlContext,
    cache_type: &str,
    cache_update: &ChartCacheUpdate,
) -> XmlNode {
    let mut cache = XmlNode::new(ctx.c(cache_type));
    if cache_type == "numCache" {
        let mut format = XmlNode::new(ctx.c("formatCode"));
        format.text = if cache_update.format_code.trim().is_empty() {
            "General".to_string()
        } else {
            cache_update.format_code.clone()
        };
        cache.children.push(format);
    }
    let mut pt_count = XmlNode::new(ctx.c("ptCount"));
    pt_count.set_attr("val", &cache_update.points.len().to_string());
    cache.children.push(pt_count);
    for point in &cache_update.points {
        let mut pt = XmlNode::new(ctx.c("pt"));
        pt.set_attr("idx", &point.index.to_string());
        let mut value = XmlNode::new(ctx.c("v"));
        value.text = point.value.clone();
        pt.children.push(value);
        cache.children.push(pt);
    }
    cache
}

pub(super) fn cache_point_count_from_node(cache: &XmlNode) -> i64 {
    direct_child(cache, "ptCount")
        .and_then(attr_val_i64)
        .unwrap_or_else(|| descendants(cache, "pt").len() as i64)
}

pub(super) fn cache_preview_from_node(cache: &XmlNode, limit: usize) -> Vec<String> {
    descendants(cache, "pt")
        .into_iter()
        .filter_map(|point| direct_child(point, "v").map(node_text))
        .take(limit)
        .collect()
}

pub(super) fn sibling_point_counts(series: &XmlNode) -> BTreeMap<String, i64> {
    let mut result = BTreeMap::new();
    for role_name in [
        "name",
        "categories",
        "values",
        "xValues",
        "yValues",
        "bubbleSize",
    ] {
        let Ok(role) = normalize_chart_source_role(role_name) else {
            continue;
        };
        let Some(role_elem) = direct_child(series, role.element) else {
            continue;
        };
        let Ok((source_index, _)) = source_ref_child_index(role_elem) else {
            continue;
        };
        let source = &role_elem.children[source_index];
        let Some(cache) = first_cache_child(source) else {
            continue;
        };
        result.insert(role.canonical, cache_point_count_from_node(cache));
    }
    result
}

pub(super) fn series_roles(series: &XmlNode) -> Vec<String> {
    let mut roles = Vec::new();
    for role_name in [
        "name",
        "categories",
        "values",
        "xValues",
        "yValues",
        "bubbleSize",
    ] {
        if let Ok(role) = normalize_chart_source_role(role_name)
            && direct_child(series, role.element).is_some()
        {
            roles.push(role.canonical);
        }
    }
    if roles.is_empty() {
        vec!["none".to_string()]
    } else {
        roles
    }
}

pub(super) fn comparable_point_role(role: &str) -> bool {
    role != "name"
}

pub(super) fn unique_sorted_warnings(warnings: &[String]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for warning in warnings {
        let warning = warning.trim();
        if !warning.is_empty() {
            set.insert(warning.to_string());
        }
    }
    set.into_iter().collect()
}

pub(super) struct ChartUpdateSourceResultInput<'a> {
    pub(super) file: &'a str,
    pub(super) output: Option<&'a str>,
    pub(super) dry_run: bool,
    pub(super) sheet_selector: Option<&'a str>,
    pub(super) chart_selector: Option<&'a str>,
    pub(super) series: i64,
    pub(super) role: &'a ChartSourceRole,
    pub(super) chart_item: Value,
    pub(super) mutation: &'a ChartSourceMutation,
    pub(super) source: &'a ResolvedChartUpdateSource,
    pub(super) cache_update: &'a ChartCacheUpdate,
}

pub(super) fn xlsx_chart_update_source_result(input: ChartUpdateSourceResultInput<'_>) -> Value {
    let ChartUpdateSourceResultInput {
        file,
        output,
        dry_run,
        sheet_selector,
        chart_selector,
        series,
        role,
        mut chart_item,
        mutation,
        source,
        cache_update,
    } = input;
    let mut result = Map::new();
    if let Some(object) = chart_item.as_object_mut() {
        object.remove("style");
    }
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("action".to_string(), json!("xlsx.chart.update-source"));
    result.insert("chart".to_string(), chart_item.clone());
    result.insert("series".to_string(), json!(series));
    result.insert("role".to_string(), json!(role.canonical));
    insert_nonempty_string(&mut result, "previousFormula", &mutation.previous_formula);
    result.insert("formula".to_string(), json!(mutation.formula));
    result.insert("sheet".to_string(), json!(source.sheet));
    result.insert("range".to_string(), json!(source.range));
    result.insert("refKind".to_string(), json!(mutation.ref_kind));
    insert_nonempty_string(&mut result, "cacheType", &mutation.cache_type);
    result.insert(
        "cachePointCount".to_string(),
        json!(mutation.cache_point_count),
    );
    insert_nonempty_array(
        &mut result,
        "cachePreview",
        mutation
            .cache_preview
            .iter()
            .map(|value| json!(value))
            .collect(),
    );
    if mutation.cache_skipped > 0 {
        result.insert("cacheSkipped".to_string(), json!(mutation.cache_skipped));
    } else if cache_update.skipped > 0 {
        result.insert("cacheSkipped".to_string(), json!(cache_update.skipped));
    }
    result.insert("cacheVerified".to_string(), json!(false));
    if !mutation.warnings.is_empty() {
        result.insert("warnings".to_string(), json!(mutation.warnings));
    }
    let selector = xlsx_chart_selector_for_update_template(&chart_item, chart_selector);
    if let Some(output) = output {
        result.insert(
            "validateCommand".to_string(),
            json!(format!("ooxml validate --strict {}", command_arg(output))),
        );
        result.insert(
            "chartShowCommand".to_string(),
            json!(xlsx_chart_show_command_for_update(
                output,
                sheet_selector,
                &selector
            )),
        );
        result.insert(
            "rangesExportCommand".to_string(),
            json!(xlsx_ranges_export_command(
                output,
                &source.sheet,
                &source.range
            )),
        );
        result.insert(
            "sourceRangeExportCommand".to_string(),
            json!(xlsx_ranges_export_command(
                file,
                &source.sheet,
                &source.range
            )),
        );
    } else {
        let placeholder = "<out.xlsx>";
        result.insert(
            "validateCommandTemplate".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(placeholder)
            )),
        );
        result.insert(
            "chartShowCommandTemplate".to_string(),
            json!(xlsx_chart_show_command_for_update(
                placeholder,
                sheet_selector,
                &selector
            )),
        );
        result.insert(
            "rangesExportCommandTemplate".to_string(),
            json!(xlsx_ranges_export_command(
                placeholder,
                &source.sheet,
                &source.range
            )),
        );
        result.insert(
            "sourceRangeExportCommandDryRun".to_string(),
            json!(xlsx_ranges_export_command(
                file,
                &source.sheet,
                &source.range
            )),
        );
    }
    result.insert(
        "storedCacheContract".to_string(),
        json!(
            "stored chart cache values are written from worksheet cell values but not recalculated or verified by Excel"
        ),
    );
    Value::Object(result)
}

pub(super) fn xlsx_chart_selector_for_update_template(
    chart_item: &Value,
    fallback: Option<&str>,
) -> String {
    chart_item
        .get("primarySelector")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            fallback
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "chart:1".to_string())
}

pub(super) fn xlsx_chart_show_command_for_update(
    file: &str,
    sheet_selector: Option<&str>,
    chart_selector: &str,
) -> String {
    let mut args = vec![
        "ooxml".to_string(),
        "--json".to_string(),
        "xlsx".to_string(),
        "charts".to_string(),
        "show".to_string(),
        command_arg(file),
    ];
    if let Some(sheet) = sheet_selector.filter(|value| !value.trim().is_empty()) {
        args.push("--sheet".to_string());
        args.push(command_arg(sheet));
    }
    args.push("--chart".to_string());
    args.push(command_arg(chart_selector));
    args.join(" ")
}
