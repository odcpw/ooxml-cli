use super::*;

pub(super) fn run_xlsx_chart_style_mutation<F>(
    file: &str,
    sheet_selector: Option<&str>,
    chart_selector: Option<&str>,
    action: &str,
    output_options: XlsxChartOutputOptions<'_>,
    apply: F,
) -> CliResult<Value>
where
    F: FnOnce(&mut XmlNode, &ChartXmlContext) -> CliResult<ChartMutationExtra>,
{
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    validate_xlsx_mutation_output_flags(
        output_options.out,
        output_options.in_place,
        output_options.backup,
        output_options.dry_run,
    )?;

    let charts = load_xlsx_charts(file, sheet_selector)?;
    let selected = select_xlsx_chart(&charts, chart_selector.unwrap_or_default())?;
    let chart_part = selected.part_uri.trim_start_matches('/').to_string();
    let chart_xml = zip_text(file, &chart_part)?;
    let mut root = parse_xml_node(&chart_xml)?;
    if root.name != "chartSpace" {
        return Err(CliError::unexpected(format!(
            "chart part {} root element not found",
            selected.part_uri
        )));
    }
    let ctx = ensure_chart_xml_namespaces(&mut root);
    let extra = apply(&mut root, &ctx)?;
    let updated_xml = render_xml_document(&root);

    let output_path = output_options.out.filter(|value| !value.trim().is_empty());
    let commit_path = if output_options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path =
        if output_options.dry_run || output_options.in_place || output_path == Some(file) {
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

    copy_zip_with_part_override(file, &readback_path, &chart_part, &updated_xml)?;
    if !output_options.no_validate {
        validate(&readback_path, true)?;
    }

    let readback_charts = load_xlsx_charts(&readback_path, sheet_selector)?;
    let readback = select_xlsx_chart(&readback_charts, &format!("part:{}", selected.part_uri))?;
    let chart_item = xlsx_chart_item_for_update(commit_path, &readback);

    if output_options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if output_options.in_place || output_path == Some(file) {
        if let Some(backup_path) = output_options
            .backup
            .filter(|value| !value.trim().is_empty())
        {
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

    Ok(xlsx_chart_style_result(ChartStyleResultArgs {
        file,
        output: commit_path,
        dry_run: output_options.dry_run,
        action,
        chart_item,
        sheet_selector,
        chart: &readback,
        extra,
    }))
}

pub(super) fn xlsx_chart_style_result(args: ChartStyleResultArgs<'_>) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(args.file));
    if let Some(output) = args.output {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(args.dry_run));
    result.insert("action".to_string(), json!(args.action));
    result.insert("chart".to_string(), args.chart_item);
    if let Some(previous_title) = args.extra.previous_title.filter(|value| !value.is_empty()) {
        result.insert("previousTitle".to_string(), json!(previous_title));
    }
    if args.extra.legend_removed {
        result.insert("legendRemoved".to_string(), json!(true));
    }
    if let Some(series) = args.extra.series {
        result.insert("series".to_string(), json!(series));
    }
    if let Some(previous_type) = args.extra.previous_type {
        result.insert("previousType".to_string(), json!(previous_type));
    }
    if let Some(new_type) = args.extra.new_type {
        result.insert("newType".to_string(), json!(new_type));
    }
    if let Some(previous_fill) = args.extra.previous_fill {
        result.insert("previousFill".to_string(), json!(previous_fill));
    }
    if let Some(new_fill) = args.extra.new_fill {
        result.insert("newFill".to_string(), json!(new_fill));
    }
    if !args.extra.applied_style.is_empty() {
        result.insert("appliedStyle".to_string(), json!(args.extra.applied_style));
    }
    if !args.extra.warnings.is_empty() {
        result.insert("warnings".to_string(), json!(args.extra.warnings));
    }

    let selector = if args.chart.primary_selector.trim().is_empty() {
        "chart:1"
    } else {
        args.chart.primary_selector.as_str()
    };
    let sheet = args
        .sheet_selector
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&args.chart.sheet);
    let target = args.output.unwrap_or("<out.xlsx>");
    let validate_key = if args.output.is_some() {
        "validateCommand"
    } else {
        "validateCommandTemplate"
    };
    let show_key = if args.output.is_some() {
        "chartShowCommand"
    } else {
        "chartShowCommandTemplate"
    };
    result.insert(
        validate_key.to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(target))),
    );
    result.insert(
        show_key.to_string(),
        json!(format!(
            "ooxml --json xlsx charts show {} --sheet {} --chart {}",
            command_arg(target),
            command_arg(sheet),
            command_arg(selector)
        )),
    );
    Value::Object(result)
}

pub(super) fn normalize_optional_nonempty(
    value: Option<&str>,
    flag: &str,
) -> CliResult<Option<String>> {
    match value {
        Some(value) if value.trim().is_empty() => {
            Err(CliError::invalid_args(format!("{flag} must not be empty")))
        }
        Some(value) => Ok(Some(value.trim().to_string())),
        None => Ok(None),
    }
}

pub(super) fn normalize_optional_hex(value: Option<&str>) -> CliResult<Option<String>> {
    value.map(normalize_hex_color).transpose()
}

pub(super) fn normalize_hex_color(value: &str) -> CliResult<String> {
    let trimmed = value.trim();
    let trimmed = trimmed.strip_prefix('#').unwrap_or(trimmed);
    let upper = trimmed.to_ascii_uppercase();
    if upper.len() != 6 || !upper.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CliError::invalid_args(format!(
            "color {value:?} must be a 6-digit hex like #1F77B4"
        )));
    }
    Ok(upper)
}

pub(super) fn parse_chart_legend_position(value: &str) -> CliResult<LegendPosition> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized == "none" {
        return Ok(LegendPosition {
            code: String::new(),
            remove: true,
        });
    }
    let code = match normalized.as_str() {
        "right" | "r" => "r",
        "left" | "l" => "l",
        "top" | "t" => "t",
        "bottom" | "b" => "b",
        "tr" => "tr",
        _ => {
            return Err(CliError::invalid_args(
                "--position must be right, left, top, bottom, or none",
            ));
        }
    };
    Ok(LegendPosition {
        code: code.to_string(),
        remove: false,
    })
}

pub(super) fn parse_chart_expect_legend_position(value: &str) -> CliResult<String> {
    let parsed = parse_chart_legend_position(value).map_err(|_| {
        CliError::invalid_args("--expect-position must be right, left, top, bottom, or none")
    })?;
    Ok(if parsed.remove {
        String::new()
    } else {
        parsed.code
    })
}

pub(super) fn parse_chart_fill_color(value: &str) -> CliResult<ChartFillOptions> {
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok(ChartFillOptions {
            color: String::new(),
            no_fill: true,
        });
    }
    Ok(ChartFillOptions {
        color: normalize_hex_color(value)?,
        no_fill: false,
    })
}

pub(super) fn resolve_chart_expect_fill(value: &str) -> CliResult<String> {
    if value.trim().is_empty() || value.trim().eq_ignore_ascii_case("none") {
        return Ok(String::new());
    }
    if value.trim().to_ascii_lowercase().starts_with("scheme:") {
        return Ok(value.trim().to_string());
    }
    normalize_hex_color(value)
}

pub(super) fn parse_chart_marker_symbol(value: &str) -> CliResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "circle" | "square" | "diamond" | "triangle" | "none" => Ok(normalized),
        _ => Err(CliError::invalid_args(
            "--marker-symbol must be circle, square, diamond, triangle, or none",
        )),
    }
}

pub(super) fn parse_chart_type(value: &str, flag: &str) -> CliResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    let chart_type = match normalized.as_str() {
        "bar" | "barchart" => "bar",
        "column" | "col" | "columnchart" => "column",
        "line" | "linechart" => "line",
        "area" | "areachart" => "area",
        "pie" | "piechart" => "pie",
        "scatter" | "scatterchart" | "xy" => "scatter",
        _ => {
            let message = if flag == "--expect-type" {
                format!(
                    "invalid --expect-type: invalid chart type {value:?} (use bar, column, line, area, pie, or scatter)"
                )
            } else {
                format!(
                    "invalid chart type {value:?} (use bar, column, line, area, pie, or scatter)"
                )
            };
            return Err(CliError::invalid_args(message));
        }
    };
    Ok(chart_type.to_string())
}

pub(super) fn parse_chart_axis_kind(value: &str) -> CliResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "category" | "value" => Ok(normalized),
        _ => Err(CliError::invalid_args(
            "--axis is required; use category or value",
        )),
    }
}

pub(super) fn resolve_chart_axis_flags(
    options: &XlsxChartSetAxisOptions<'_>,
) -> CliResult<ChartAxisFlags> {
    let mut any = false;
    let mut flags = ChartAxisFlags {
        set_title: options.title_present,
        title: options.title.unwrap_or_default().to_string(),
        set_hidden: options.hidden.is_some(),
        hidden: options.hidden.unwrap_or(false),
        min: options.min,
        max: options.max,
        major_unit: options.major_unit,
        number_format: None,
        set_major_gridlines: options.major_gridlines.is_some(),
        major_gridlines: options.major_gridlines.unwrap_or(false),
        set_minor_gridlines: options.minor_gridlines.is_some(),
        minor_gridlines: options.minor_gridlines.unwrap_or(false),
        tick_label_font: ChartFontOptions {
            family: None,
            size_pt: None,
            color: None,
            bold: options.tick_label_font_bold,
            italic: options.tick_label_font_italic,
        },
        title_font: ChartFontOptions {
            family: None,
            size_pt: None,
            color: None,
            bold: options.title_font_bold,
            italic: options.title_font_italic,
        },
    };
    any |= flags.set_title
        || flags.set_hidden
        || flags.min.is_some()
        || flags.max.is_some()
        || flags.major_unit.is_some()
        || flags.set_major_gridlines
        || flags.set_minor_gridlines
        || flags.tick_label_font.bold.is_some()
        || flags.tick_label_font.italic.is_some()
        || flags.title_font.bold.is_some()
        || flags.title_font.italic.is_some();

    if let (Some(min), Some(max)) = (flags.min, flags.max)
        && min >= max
    {
        return Err(CliError::invalid_args("--min must be less than --max"));
    }
    if let Some(unit) = flags.major_unit {
        if unit <= 0.0 {
            return Err(CliError::invalid_args(
                "--major-unit must be greater than 0",
            ));
        }
        any = true;
    }
    if let Some(format) = options.number_format {
        if format.trim().is_empty() {
            return Err(CliError::invalid_args("--number-format must not be empty"));
        }
        flags.number_format = Some(format.to_string());
        any = true;
    }
    if let Some(size) = options.tick_label_font_size {
        if size <= 0.0 {
            return Err(CliError::invalid_args(
                "--tick-label-font-size must be greater than 0",
            ));
        }
        flags.tick_label_font.size_pt = Some(size);
        any = true;
    }
    if let Some(color) = options.tick_label_font_color {
        flags.tick_label_font.color = Some(normalize_hex_color(color)?);
        any = true;
    }
    if let Some(family) = options.tick_label_font_family {
        if family.trim().is_empty() {
            return Err(CliError::invalid_args(
                "--tick-label-font-family must not be empty",
            ));
        }
        flags.tick_label_font.family = Some(family.trim().to_string());
        any = true;
    }
    if let Some(family) = options.title_font_family {
        if family.trim().is_empty() {
            return Err(CliError::invalid_args(
                "--title-font-family must not be empty",
            ));
        }
        flags.title_font.family = Some(family.trim().to_string());
        any = true;
    }
    if let Some(size) = options.title_font_size {
        if size <= 0.0 {
            return Err(CliError::invalid_args(
                "--title-font-size must be greater than 0",
            ));
        }
        flags.title_font.size_pt = Some(size);
        any = true;
    }
    if let Some(color) = options.title_font_color {
        flags.title_font.color = Some(normalize_hex_color(color)?);
        any = true;
    }
    if !any {
        return Err(CliError::invalid_args(
            "set-axis requires at least one of --title, --hidden, --min, --max, --major-unit, --number-format, --major-gridlines, --minor-gridlines, or a --title-font-*/--tick-label-font-* flag",
        ));
    }
    Ok(flags)
}

pub(super) fn apply_chart_set_title(
    root: &mut XmlNode,
    ctx: &ChartXmlContext,
    title_text_value: &str,
    expect_title: Option<&str>,
    font: &ChartFontOptions,
) -> CliResult<String> {
    let chart_index = child_index(root, "chart")
        .ok_or_else(|| CliError::unexpected("chart part has no chart element"))?;
    let chart = &mut root.children[chart_index];
    let previous = direct_child(chart, "title")
        .map(title_text)
        .unwrap_or_default();
    if let Some(expected) = expect_title
        && previous.trim() != expected.trim()
    {
        return Err(CliError::invalid_args(format!(
            "chart title mismatch: expected {expected:?} but found {previous:?}"
        )));
    }
    let title_index = ensure_child_index(chart, "title", ctx.c("title"), CHART_CHILD_ORDER);
    let title = &mut chart.children[title_index];
    if direct_child(title, "tx")
        .and_then(|tx| direct_child(tx, "strRef"))
        .is_some()
    {
        return Err(CliError::invalid_args(
            "title is linked to a cell; setting literal title text is not supported",
        ));
    }
    replace_title_text_tree(title, ctx, title_text_value, font);
    set_or_create_val_child(chart, ctx, "autoTitleDeleted", "0", CHART_CHILD_ORDER);
    Ok(previous)
}

pub(super) fn replace_title_text_tree(
    title: &mut XmlNode,
    ctx: &ChartXmlContext,
    text: &str,
    font: &ChartFontOptions,
) {
    title.children.retain(|child| child.name != "tx");
    let mut tx = XmlNode::new(ctx.c("tx"));
    let mut rich = XmlNode::new(ctx.c("rich"));
    rich.children.push(XmlNode::new(ctx.a("bodyPr")));
    rich.children.push(XmlNode::new(ctx.a("lstStyle")));
    let mut paragraph = XmlNode::new(ctx.a("p"));
    let mut run = XmlNode::new(ctx.a("r"));
    if !font.is_empty() {
        let mut r_pr = XmlNode::new(ctx.a("rPr"));
        apply_font_to_rpr(&mut r_pr, ctx, font);
        insert_child_in_order(&mut run, r_pr, RUN_CHILD_ORDER);
    }
    let mut text_node = XmlNode::new(ctx.a("t"));
    text_node.text = text.to_string();
    insert_child_in_order(&mut run, text_node, RUN_CHILD_ORDER);
    insert_child_in_order(&mut paragraph, run, PARAGRAPH_CHILD_ORDER);
    rich.children.push(paragraph);
    tx.children.push(rich);
    insert_child_in_order(title, tx, TITLE_CHILD_ORDER);
}

pub(super) fn apply_font_to_rpr(
    r_pr: &mut XmlNode,
    ctx: &ChartXmlContext,
    font: &ChartFontOptions,
) {
    if let Some(size) = font.size_pt {
        r_pr.set_attr("sz", &((size * 100.0 + 0.5) as i64).to_string());
    }
    if let Some(bold) = font.bold {
        r_pr.set_attr("b", bool_attr(bold));
    }
    if let Some(italic) = font.italic {
        r_pr.set_attr("i", bool_attr(italic));
    }
    if let Some(color) = font.color.as_deref() {
        apply_color_fill(r_pr, ctx, color, RPR_CHILD_ORDER);
    }
    if let Some(family) = font.family.as_deref() {
        let latin_index = ensure_child_index(r_pr, "latin", ctx.a("latin"), RPR_CHILD_ORDER);
        r_pr.children[latin_index].set_attr("typeface", family);
    }
}

pub(super) fn apply_chart_set_legend(
    root: &mut XmlNode,
    ctx: &ChartXmlContext,
    position: Option<&LegendPosition>,
    overlay: Option<bool>,
    expect_position: Option<&str>,
) -> CliResult<bool> {
    let chart_index = child_index(root, "chart")
        .ok_or_else(|| CliError::unexpected("chart part has no chart element"))?;
    let chart = &mut root.children[chart_index];
    let previous = direct_child(chart, "legend")
        .and_then(|legend| direct_child(legend, "legendPos"))
        .and_then(|pos| pos.attr("val"))
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if let Some(expected) = expect_position
        && !previous.eq_ignore_ascii_case(expected.trim())
    {
        return Err(CliError::invalid_args(format!(
            "legend position mismatch: expected {expected:?} but found {previous:?}"
        )));
    }
    if position.is_some_and(|value| value.remove) {
        chart.children.retain(|child| child.name != "legend");
        return Ok(true);
    }
    let legend_index = ensure_child_index(chart, "legend", ctx.c("legend"), CHART_CHILD_ORDER);
    let legend = &mut chart.children[legend_index];
    if let Some(position) = position {
        set_or_create_val_child(legend, ctx, "legendPos", &position.code, LEGEND_CHILD_ORDER);
    } else if direct_child(legend, "legendPos").is_none() {
        set_or_create_val_child(legend, ctx, "legendPos", "r", LEGEND_CHILD_ORDER);
    }
    if let Some(overlay) = overlay {
        set_or_create_val_child(
            legend,
            ctx,
            "overlay",
            bool_attr(overlay),
            LEGEND_CHILD_ORDER,
        );
    }
    Ok(false)
}

pub(super) fn apply_chart_set_fill(
    root: &mut XmlNode,
    ctx: &ChartXmlContext,
    target: &XlsxChartFillTarget,
    fill: &ChartFillOptions,
    expect_fill: Option<&str>,
) -> CliResult<(String, String)> {
    let (holder, order) = match target {
        XlsxChartFillTarget::ChartArea => (root, CHART_SPACE_CHILD_ORDER),
        XlsxChartFillTarget::PlotArea => {
            let plot_area = first_descendant_mut(root, "plotArea")
                .ok_or_else(|| CliError::unexpected("chart part has no plotArea"))?;
            (plot_area, PLOT_AREA_CHILD_ORDER)
        }
    };
    let previous = direct_child(holder, "spPr")
        .map(inspect_fill)
        .unwrap_or_default();
    if let Some(expected) = expect_fill
        && !fill_matches(&previous, expected)
    {
        let have = if previous.is_empty() {
            "none".to_string()
        } else {
            previous.clone()
        };
        return Err(CliError::invalid_args(format!(
            "fill mismatch: expected {expected:?} but found {have:?}"
        )));
    }
    let sp_pr_index = ensure_child_index(holder, "spPr", ctx.c("spPr"), order);
    let sp_pr = &mut holder.children[sp_pr_index];
    if fill.no_fill {
        remove_fill_group_children(sp_pr);
        insert_child_in_order(
            sp_pr,
            XmlNode::new(ctx.a("noFill")),
            SHAPE_PROPS_CHILD_ORDER,
        );
        Ok((previous, String::new()))
    } else {
        set_solid_fill(sp_pr, ctx, &fill.color, SHAPE_PROPS_CHILD_ORDER);
        Ok((previous, fill.color.clone()))
    }
}

pub(super) fn apply_chart_set_series_style(
    root: &mut XmlNode,
    ctx: &ChartXmlContext,
    series_number: usize,
    expect_series_count: Option<usize>,
    style: &ChartSeriesStyleOptions,
) -> CliResult<()> {
    let series_paths = series_node_paths(root);
    if let Some(expected) = expect_series_count
        && expected != series_paths.len()
    {
        return Err(CliError::invalid_args(format!(
            "series count mismatch: expected {expected} but found {}",
            series_paths.len()
        )));
    }
    if series_number < 1 || series_number > series_paths.len() {
        return Err(CliError::invalid_args(format!(
            "series {series_number} is out of range (1-{})",
            series_paths.len()
        )));
    }
    let (chart_type_index, series_index) = series_paths[series_number - 1];
    let plot_area = first_descendant_mut(root, "plotArea")
        .ok_or_else(|| CliError::unexpected("chart part has no plotArea"))?;
    let chart_type = &mut plot_area.children[chart_type_index];
    let parent_type = chart_type.name.clone();
    let series = &mut chart_type.children[series_index];

    if style.fill_color.is_some() || style.line_color.is_some() || style.line_width_pt.is_some() {
        let sp_pr_index = ensure_child_index(series, "spPr", ctx.c("spPr"), SERIES_CHILD_ORDER);
        let sp_pr = &mut series.children[sp_pr_index];
        if let Some(color) = style.fill_color.as_deref() {
            set_solid_fill(sp_pr, ctx, color, SHAPE_PROPS_CHILD_ORDER);
        }
        if style.line_color.is_some() || style.line_width_pt.is_some() {
            let line_index = ensure_child_index(sp_pr, "ln", ctx.a("ln"), SHAPE_PROPS_CHILD_ORDER);
            let line = &mut sp_pr.children[line_index];
            if let Some(width) = style.line_width_pt {
                line.set_attr("w", &((width * 12700.0 + 0.5) as i64).to_string());
            }
            if let Some(color) = style.line_color.as_deref() {
                set_solid_fill(line, ctx, color, LINE_CHILD_ORDER);
            }
        }
    }

    if style.marker_symbol.is_some() || style.marker_size.is_some() {
        if !matches!(
            parent_type.as_str(),
            "lineChart" | "scatterChart" | "radarChart"
        ) {
            return Err(CliError::invalid_args(format!(
                "series {series_number} belongs to a {}, which does not support markers",
                if parent_type.is_empty() {
                    "chart of this type"
                } else {
                    parent_type.as_str()
                }
            )));
        }
        let marker_index =
            ensure_child_index(series, "marker", ctx.c("marker"), SERIES_CHILD_ORDER);
        let marker = &mut series.children[marker_index];
        if let Some(symbol) = style.marker_symbol.as_deref() {
            set_or_create_val_child(marker, ctx, "symbol", symbol, MARKER_CHILD_ORDER);
        }
        if let Some(size) = style.marker_size {
            set_or_create_val_child(marker, ctx, "size", &size.to_string(), MARKER_CHILD_ORDER);
        }
    }
    Ok(())
}

pub(super) fn apply_chart_convert_type(
    root: &mut XmlNode,
    ctx: &ChartXmlContext,
    target_type: &str,
    expect_type: Option<&str>,
) -> CliResult<ChartTypeConversion> {
    let plot_area = first_descendant_mut(root, "plotArea")
        .ok_or_else(|| CliError::unexpected("chart part has no plotArea"))?;
    let old_plot_index = plot_area
        .children
        .iter()
        .position(|child| child.name.ends_with("Chart"))
        .ok_or_else(|| CliError::unexpected("chart part has no chart-type plot element"))?;
    let previous_type = canonical_chart_type(&plot_area.children[old_plot_index])?;
    if let Some(expected) = expect_type
        && expected != previous_type
    {
        return Err(CliError::invalid_args(format!(
            "chart type mismatch: expected {expected} but found {previous_type}; use --dry-run to inspect"
        )));
    }
    if previous_type == target_type {
        return Err(CliError::invalid_args(format!(
            "chart is already a {target_type} chart"
        )));
    }
    if previous_type == "pie" {
        return Err(CliError::invalid_args(format!(
            "cannot convert from pie to {target_type}: pie charts have no category/value axis structure to carry over; recreate the chart with `charts create --type {target_type}`"
        )));
    }

    let old_element = element_for_chart_type(&previous_type);
    let target_element = element_for_chart_type(target_type);
    let old_series_count = plot_area.children[old_plot_index]
        .children
        .iter()
        .filter(|child| child.name == "ser")
        .count();
    if old_series_count == 0 {
        return Err(CliError::invalid_args("chart has no series to convert"));
    }
    if target_type == "pie" && old_series_count > 1 {
        return Err(CliError::invalid_args(format!(
            "cannot convert to pie: a pie chart supports a single series but this chart has {old_series_count}; remove series before converting"
        )));
    }

    if old_element == target_element {
        set_bar_dir(ctx, &mut plot_area.children[old_plot_index], target_type);
        return Ok(ChartTypeConversion {
            previous_type,
            new_type: target_type.to_string(),
            warnings: Vec::new(),
        });
    }

    let old_plot = plot_area.children.remove(old_plot_index);
    let axis_ids = plot_axis_ids(&old_plot);
    let mut warnings = Vec::new();
    let mut series = Vec::new();
    for (index, mut child) in old_plot
        .children
        .into_iter()
        .filter(|child| child.name == "ser")
        .enumerate()
    {
        warnings.extend(transform_series_for_chart_type(
            &mut child,
            &previous_type,
            target_type,
            index + 1,
        ));
        reorder_children_in_order(&mut child, SERIES_CHILD_ORDER);
        series.push(child);
    }
    let new_plot = build_plot_wrapper(ctx, target_type, series, &axis_ids);
    plot_area.children.insert(old_plot_index, new_plot);
    if let Some(warning) =
        transform_axes_for_chart_type(plot_area, &previous_type, target_type, &axis_ids)
    {
        warnings.push(warning);
    }
    Ok(ChartTypeConversion {
        previous_type,
        new_type: target_type.to_string(),
        warnings,
    })
}

pub(super) fn apply_chart_set_axis(
    root: &mut XmlNode,
    ctx: &ChartXmlContext,
    axis_kind_value: &str,
    flags: &ChartAxisFlags,
    expect_title: Option<&str>,
    expect_axis_count: Option<usize>,
) -> CliResult<String> {
    if flags.major_unit.is_some() && axis_kind_value != "value" {
        return Err(CliError::invalid_args(
            "major unit applies only to the value axis (use --axis value)",
        ));
    }
    let plot_area = first_descendant_mut(root, "plotArea")
        .ok_or_else(|| CliError::unexpected("chart part has no plotArea"))?;
    let all_axes = plot_area
        .children
        .iter()
        .enumerate()
        .filter(|(_, child)| is_axis_element(&child.name))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if let Some(expected) = expect_axis_count
        && expected != all_axes.len()
    {
        return Err(CliError::invalid_args(format!(
            "axis count mismatch: expected {expected} but found {}",
            all_axes.len()
        )));
    }
    let want_element = if axis_kind_value == "value" {
        "valAx"
    } else {
        "catAx"
    };
    let matches = all_axes
        .iter()
        .copied()
        .filter(|index| plot_area.children[*index].name == want_element)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err(CliError::invalid_args(format!(
            "chart has no {axis_kind_value} axis"
        )));
    }
    if matches.len() > 1 {
        return Err(CliError::invalid_args(format!(
            "chart has {} {axis_kind_value} axes (e.g. a scatter chart's x and y axes); axis selection is ambiguous, narrow the chart with --chart or guard with --expect-axis-count",
            matches.len()
        )));
    }
    let axis_index = matches[0];
    let axis = &mut plot_area.children[axis_index];
    let order = axis_child_order(&axis.name);
    let previous_title = direct_child(axis, "title")
        .map(title_text)
        .unwrap_or_default();
    if let Some(expected) = expect_title
        && previous_title.trim() != expected.trim()
    {
        return Err(CliError::invalid_args(format!(
            "axis title mismatch: expected {expected:?} but found {previous_title:?}"
        )));
    }

    if flags.set_title {
        if flags.title.trim().is_empty() {
            axis.children.retain(|child| child.name != "title");
        } else {
            let title_index = ensure_child_index(axis, "title", ctx.c("title"), order);
            let title = &mut axis.children[title_index];
            if direct_child(title, "tx")
                .and_then(|tx| direct_child(tx, "strRef"))
                .is_some()
            {
                return Err(CliError::invalid_args(
                    "title is linked to a cell; setting literal title text is not supported",
                ));
            }
            replace_title_text_tree(title, ctx, &flags.title, &flags.title_font);
        }
    }
    if flags.set_hidden {
        set_or_create_val_child(axis, ctx, "delete", bool_attr(flags.hidden), order);
    }
    if flags.min.is_some() || flags.max.is_some() {
        let scaling_index = ensure_child_index(axis, "scaling", ctx.c("scaling"), order);
        let scaling = &mut axis.children[scaling_index];
        if let Some(max) = flags.max {
            set_or_create_val_child(scaling, ctx, "max", &format_float(max), SCALING_CHILD_ORDER);
        }
        if let Some(min) = flags.min {
            set_or_create_val_child(scaling, ctx, "min", &format_float(min), SCALING_CHILD_ORDER);
        }
    }
    if let Some(major_unit) = flags.major_unit {
        set_or_create_val_child(axis, ctx, "majorUnit", &format_float(major_unit), order);
    }
    if let Some(number_format) = flags.number_format.as_deref() {
        let num_fmt_index = ensure_child_index(axis, "numFmt", ctx.c("numFmt"), order);
        let num_fmt = &mut axis.children[num_fmt_index];
        num_fmt.set_attr("formatCode", number_format);
        num_fmt.set_attr("sourceLinked", "0");
    }
    if flags.set_major_gridlines {
        apply_gridlines(axis, ctx, "majorGridlines", flags.major_gridlines, order);
    }
    if flags.set_minor_gridlines {
        apply_gridlines(axis, ctx, "minorGridlines", flags.minor_gridlines, order);
    }
    if !flags.tick_label_font.is_empty() {
        apply_axis_tick_label_font(axis, ctx, &flags.tick_label_font, order);
    }
    Ok(previous_title)
}

pub(super) fn apply_chart_copy_style(
    root: &mut XmlNode,
    ctx: &ChartXmlContext,
    source_style: &Value,
    expect_series_count: Option<usize>,
) -> CliResult<Vec<String>> {
    let series_count = walk_series(root).len();
    if let Some(expected) = expect_series_count
        && expected != series_count
    {
        return Err(CliError::invalid_args(format!(
            "series count mismatch: expected {expected} but found {series_count}"
        )));
    }
    let mut applied = Vec::new();

    let chart_index = child_index(root, "chart")
        .ok_or_else(|| CliError::unexpected("chart part has no chart element"))?;
    if let Some(font) = source_style
        .get("title")
        .and_then(|title| title.get("font"))
        .and_then(style_font_from_value)
    {
        let chart = &mut root.children[chart_index];
        if let Some(title_index) = child_index(chart, "title") {
            let title = &mut chart.children[title_index];
            if let Some(run) = first_descendant_mut(title, "r") {
                apply_run_font(run, ctx, &font);
                applied.push("title-font".to_string());
            }
        }
    }

    if let Some(legend) = source_style.get("legend")
        && legend
            .get("present")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && let Some(position) = legend.get("position").and_then(Value::as_str)
        && !position.trim().is_empty()
    {
        let chart = &mut root.children[chart_index];
        let legend_index = ensure_child_index(chart, "legend", ctx.c("legend"), CHART_CHILD_ORDER);
        let legend_node = &mut chart.children[legend_index];
        set_or_create_val_child(legend_node, ctx, "legendPos", position, LEGEND_CHILD_ORDER);
        if let Some(overlay) = legend.get("overlay").and_then(Value::as_bool) {
            set_or_create_val_child(
                legend_node,
                ctx,
                "overlay",
                bool_attr(overlay),
                LEGEND_CHILD_ORDER,
            );
        }
        applied.push("legend".to_string());
    }

    if let Some(plot_area) = first_descendant_mut(root, "plotArea") {
        if let Some(axes) = source_style.get("axes").and_then(Value::as_array) {
            for axis in axes {
                let Some(target_index) = find_target_axis_index(plot_area, axis) else {
                    continue;
                };
                let target = &mut plot_area.children[target_index];
                let element_name = target.name.clone();
                let order = axis_child_order(&element_name);
                let mut changed = false;
                if let Some(font) = axis.get("titleFont").and_then(style_font_from_value)
                    && let Some(title_index) = child_index(target, "title")
                {
                    let title = &mut target.children[title_index];
                    if let Some(run) = first_descendant_mut(title, "r") {
                        apply_run_font(run, ctx, &font);
                        changed = true;
                    }
                }
                if let Some(font) = axis.get("tickLabelFont").and_then(style_font_from_value) {
                    apply_axis_tick_label_font(target, ctx, &font, order);
                    changed = true;
                }
                if let Some(format) = axis.get("numberFormat").and_then(Value::as_str)
                    && !format.trim().is_empty()
                {
                    let num_fmt_index =
                        ensure_child_index(target, "numFmt", ctx.c("numFmt"), order);
                    let num_fmt = &mut target.children[num_fmt_index];
                    num_fmt.set_attr("formatCode", format);
                    num_fmt.set_attr("sourceLinked", "0");
                    changed = true;
                }
                let major = axis
                    .get("majorGridlines")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let minor = axis
                    .get("minorGridlines")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                apply_gridlines(target, ctx, "majorGridlines", major, order);
                apply_gridlines(target, ctx, "minorGridlines", minor, order);
                if changed {
                    applied.push(format!("axis:{element_name}"));
                } else {
                    applied.push(format!("axis-gridlines:{element_name}"));
                }
            }
        }

        if let Some(series_styles) = source_style.get("series").and_then(Value::as_array) {
            for (index, source_series) in series_styles.iter().enumerate() {
                if !apply_series_style_from_source(plot_area, ctx, index, source_series) {
                    break;
                }
                applied.push(format!("series:{}", index + 1));
            }
        }

        if let Some(fill) = source_style.get("plotAreaFill").and_then(Value::as_str)
            && !fill.trim().is_empty()
        {
            let sp_pr_index =
                ensure_child_index(plot_area, "spPr", ctx.c("spPr"), PLOT_AREA_CHILD_ORDER);
            apply_color_fill(
                &mut plot_area.children[sp_pr_index],
                ctx,
                fill,
                SHAPE_PROPS_CHILD_ORDER,
            );
            applied.push("plot-area-fill".to_string());
        }
    }

    if let Some(fill) = source_style.get("chartSpaceFill").and_then(Value::as_str)
        && !fill.trim().is_empty()
    {
        let sp_pr_index = ensure_child_index(root, "spPr", ctx.c("spPr"), CHART_SPACE_CHILD_ORDER);
        apply_color_fill(
            &mut root.children[sp_pr_index],
            ctx,
            fill,
            SHAPE_PROPS_CHILD_ORDER,
        );
        applied.push("chart-area-fill".to_string());
    }

    Ok(applied)
}

pub(super) fn read_xlsx_template_chart_style(
    file: &str,
    chart_selector: Option<&str>,
) -> CliResult<Value> {
    let charts = load_xlsx_charts(file, None)?;
    let selector = chart_selector
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("chart:1");
    let selected = select_xlsx_chart(&charts, selector)?;
    let chart_part = selected.part_uri.trim_start_matches('/').to_string();
    let chart_xml = zip_text(file, &chart_part)?;
    let root = parse_xml_node(&chart_xml)?;
    if root.name != "chartSpace" {
        return Err(CliError::unexpected(format!(
            "chart part {} root element not found",
            selected.part_uri
        )));
    }
    Ok(inspect_chart_style(&root, &selected.part_uri))
}

pub(super) fn canonical_chart_type(plot: &XmlNode) -> CliResult<String> {
    let chart_type = match plot.name.as_str() {
        "barChart" | "bar3DChart" => {
            if direct_child(plot, "barDir")
                .and_then(|node| node.attr("val"))
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("bar"))
            {
                "bar"
            } else {
                "column"
            }
        }
        "lineChart" | "line3DChart" => "line",
        "areaChart" | "area3DChart" => "area",
        "pieChart" | "pie3DChart" | "doughnutChart" | "ofPieChart" => "pie",
        "scatterChart" => "scatter",
        other => {
            return Err(CliError::invalid_args(format!(
                "chart type {other:?} is not supported for conversion"
            )));
        }
    };
    Ok(chart_type.to_string())
}

pub(super) fn element_for_chart_type(chart_type: &str) -> &'static str {
    match chart_type {
        "bar" | "column" => "barChart",
        "line" => "lineChart",
        "area" => "areaChart",
        "pie" => "pieChart",
        "scatter" => "scatterChart",
        _ => "",
    }
}

pub(super) fn set_bar_dir(ctx: &ChartXmlContext, plot: &mut XmlNode, chart_type: &str) {
    let direction = if chart_type == "bar" { "bar" } else { "col" };
    if let Some(index) = child_index(plot, "barDir") {
        plot.children[index].set_attr("val", direction);
    } else {
        let mut child = XmlNode::new(ctx.c("barDir"));
        child.set_attr("val", direction);
        plot.children.insert(0, child);
    }
}

pub(super) fn plot_axis_ids(plot: &XmlNode) -> Vec<String> {
    plot.children
        .iter()
        .filter(|child| child.name == "axId")
        .filter_map(|child| child.attr("val"))
        .map(|value| value.trim().to_string())
        .collect()
}

pub(super) fn transform_series_for_chart_type(
    series: &mut XmlNode,
    previous_type: &str,
    target_type: &str,
    number: usize,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let to_scatter = target_type == "scatter";
    let from_scatter = previous_type == "scatter";
    if to_scatter && !from_scatter {
        rename_direct_child(series, "cat", "xVal");
        rename_direct_child(series, "val", "yVal");
        if let Some(x_values) = direct_child(series, "xVal")
            && (direct_child(x_values, "strRef").is_some()
                || direct_child(x_values, "multiLvlStrRef").is_some())
        {
            warnings.push(format!(
                "series {number} x-values are a text reference; scatter charts expect numeric x-values, so the chart may misrender until the source is re-pointed at numeric data"
            ));
        }
    } else if from_scatter && !to_scatter {
        rename_direct_child(series, "xVal", "cat");
        rename_direct_child(series, "yVal", "val");
    }
    if !chart_type_supports_markers(target_type)
        && let Some(marker_index) = child_index(series, "marker")
    {
        series.children.remove(marker_index);
        warnings.push(format!(
            "series {number} had a marker style; {target_type} charts do not support markers, so it was removed"
        ));
    }
    warnings
}

pub(super) fn build_plot_wrapper(
    ctx: &ChartXmlContext,
    chart_type: &str,
    series: Vec<XmlNode>,
    axis_ids: &[String],
) -> XmlNode {
    let mut plot = XmlNode::new(ctx.c(element_for_chart_type(chart_type)));
    match chart_type {
        "bar" | "column" => {
            let direction = if chart_type == "bar" { "bar" } else { "col" };
            push_val_child(&mut plot, ctx, "barDir", direction);
            push_val_child(&mut plot, ctx, "grouping", "clustered");
            push_val_child(&mut plot, ctx, "varyColors", "0");
            plot.children.extend(series);
            append_axis_ids(&mut plot, ctx, axis_ids, 2);
        }
        "line" => {
            push_val_child(&mut plot, ctx, "grouping", "standard");
            push_val_child(&mut plot, ctx, "varyColors", "0");
            plot.children.extend(series);
            push_val_child(&mut plot, ctx, "marker", "1");
            append_axis_ids(&mut plot, ctx, axis_ids, 2);
        }
        "area" => {
            push_val_child(&mut plot, ctx, "grouping", "standard");
            push_val_child(&mut plot, ctx, "varyColors", "0");
            plot.children.extend(series);
            append_axis_ids(&mut plot, ctx, axis_ids, 2);
        }
        "pie" => {
            push_val_child(&mut plot, ctx, "varyColors", "1");
            plot.children.extend(series);
            push_val_child(&mut plot, ctx, "firstSliceAng", "0");
        }
        "scatter" => {
            push_val_child(&mut plot, ctx, "scatterStyle", "lineMarker");
            push_val_child(&mut plot, ctx, "varyColors", "0");
            plot.children.extend(series);
            append_axis_ids(&mut plot, ctx, axis_ids, 2);
        }
        _ => {}
    }
    plot
}

pub(super) fn append_axis_ids(
    plot: &mut XmlNode,
    ctx: &ChartXmlContext,
    axis_ids: &[String],
    want: usize,
) {
    let fallback = ["111111111", "222222222"];
    for index in 0..want {
        let id = axis_ids
            .get(index)
            .filter(|value| !value.trim().is_empty())
            .map(String::as_str)
            .unwrap_or_else(|| fallback.get(index).copied().unwrap_or_default());
        push_val_child(plot, ctx, "axId", id);
    }
}

pub(super) fn transform_axes_for_chart_type(
    plot_area: &mut XmlNode,
    previous_type: &str,
    target_type: &str,
    axis_ids: &[String],
) -> Option<String> {
    if target_type == "pie" {
        plot_area
            .children
            .retain(|child| !is_axis_element(&child.name));
        return None;
    }
    let category_axis_id = axis_ids.first().map(String::as_str).unwrap_or_default();
    if target_type == "scatter" && previous_type != "scatter" {
        if let Some(index) = axis_by_id_or_fallback_index(plot_area, category_axis_id, "catAx") {
            rename_node_local(&mut plot_area.children[index], "valAx");
            prune_axis_children(&mut plot_area.children[index]);
            return Some(
                "category axis converted to a value axis for the scatter chart; review its scale and number format"
                    .to_string(),
            );
        }
    } else if previous_type == "scatter"
        && target_type != "scatter"
        && let Some(index) = axis_by_id_or_fallback_index(plot_area, category_axis_id, "valAx")
    {
        rename_node_local(&mut plot_area.children[index], "catAx");
        prune_axis_children(&mut plot_area.children[index]);
        return Some(
            "scatter x value axis converted to a category axis; review its labels and number format"
                .to_string(),
        );
    }
    None
}

pub(super) fn axis_by_id_or_fallback_index(
    plot_area: &XmlNode,
    id: &str,
    fallback: &str,
) -> Option<usize> {
    let trimmed = id.trim();
    if !trimmed.is_empty()
        && let Some(index) = plot_area.children.iter().position(|child| {
            is_axis_element(&child.name)
                && direct_child(child, "axId")
                    .and_then(|ax_id| ax_id.attr("val"))
                    .is_some_and(|value| value.trim() == trimmed)
        })
    {
        return Some(index);
    }
    child_index(plot_area, fallback)
}

pub(super) fn prune_axis_children(axis: &mut XmlNode) {
    let allowed = axis_child_order(&axis.name);
    axis.children
        .retain(|child| allowed.iter().any(|name| *name == child.name));
}

pub(super) fn push_val_child(parent: &mut XmlNode, ctx: &ChartXmlContext, name: &str, value: &str) {
    let mut child = XmlNode::new(ctx.c(name));
    child.set_attr("val", value);
    parent.children.push(child);
}

pub(super) fn rename_direct_child(parent: &mut XmlNode, old: &str, new: &str) {
    if let Some(index) = child_index(parent, old) {
        rename_node_local(&mut parent.children[index], new);
    }
}

pub(super) fn rename_node_local(node: &mut XmlNode, new: &str) {
    node.qname = prefix_from_qname(&node.qname)
        .map(|prefix| prefixed_qname(prefix, new))
        .unwrap_or_else(|| new.to_string());
    node.name = new.to_string();
}

pub(super) fn reorder_children_in_order(node: &mut XmlNode, order: &[&str]) {
    let children = std::mem::take(&mut node.children);
    for child in children {
        insert_child_in_order(node, child, order);
    }
}

pub(super) fn is_axis_element(name: &str) -> bool {
    matches!(name, "catAx" | "valAx" | "dateAx" | "serAx")
}

pub(super) fn axis_child_order(element: &str) -> &'static [&'static str] {
    if element == "valAx" {
        VAL_AXIS_CHILD_ORDER
    } else {
        CAT_AXIS_CHILD_ORDER
    }
}

pub(super) fn chart_type_supports_markers(chart_type: &str) -> bool {
    matches!(
        element_for_chart_type(chart_type),
        "lineChart" | "scatterChart"
    )
}

pub(super) fn format_float(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

pub(super) fn apply_gridlines(
    axis: &mut XmlNode,
    ctx: &ChartXmlContext,
    name: &str,
    on: bool,
    order: &[&str],
) {
    if on {
        if child_index(axis, name).is_none() {
            insert_child_in_order(axis, XmlNode::new(ctx.c(name)), order);
        }
    } else {
        axis.children.retain(|child| child.name != name);
    }
}

pub(super) fn apply_axis_tick_label_font(
    axis: &mut XmlNode,
    ctx: &ChartXmlContext,
    font: &ChartFontOptions,
    order: &[&str],
) {
    let tx_pr_index = ensure_child_index(axis, "txPr", ctx.c("txPr"), order);
    let tx_pr = &mut axis.children[tx_pr_index];
    if child_index(tx_pr, "bodyPr").is_none() {
        insert_child_in_order(tx_pr, XmlNode::new(ctx.a("bodyPr")), TX_PR_CHILD_ORDER);
    }
    if child_index(tx_pr, "lstStyle").is_none() {
        insert_child_in_order(tx_pr, XmlNode::new(ctx.a("lstStyle")), TX_PR_CHILD_ORDER);
    }
    let p_index = ensure_child_index(tx_pr, "p", ctx.a("p"), TX_PR_CHILD_ORDER);
    let paragraph = &mut tx_pr.children[p_index];
    let p_pr_index = ensure_child_index(paragraph, "pPr", ctx.a("pPr"), PARAGRAPH_CHILD_ORDER);
    let p_pr = &mut paragraph.children[p_pr_index];
    let def_index = ensure_child_index(p_pr, "defRPr", ctx.a("defRPr"), &[]);
    apply_font_to_rpr(&mut p_pr.children[def_index], ctx, font);
    if direct_child(paragraph, "endParaRPr").is_none() && direct_child(paragraph, "r").is_none() {
        insert_child_in_order(
            paragraph,
            XmlNode::new(ctx.a("endParaRPr")),
            PARAGRAPH_CHILD_ORDER,
        );
    }
}

pub(super) fn apply_run_font(run: &mut XmlNode, ctx: &ChartXmlContext, font: &ChartFontOptions) {
    if font.is_empty() {
        return;
    }
    let r_pr_index = ensure_child_index(run, "rPr", ctx.a("rPr"), RUN_CHILD_ORDER);
    apply_font_to_rpr(&mut run.children[r_pr_index], ctx, font);
}

pub(super) fn style_font_from_value(value: &Value) -> Option<ChartFontOptions> {
    let object = value.as_object()?;
    let font = ChartFontOptions {
        family: object
            .get("family")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_string()),
        size_pt: object.get("sizePt").and_then(Value::as_f64),
        color: object
            .get("color")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_string()),
        bold: object.get("bold").and_then(Value::as_bool),
        italic: object.get("italic").and_then(Value::as_bool),
    };
    if font.is_empty() { None } else { Some(font) }
}

pub(super) fn find_target_axis_index(plot_area: &XmlNode, source_axis: &Value) -> Option<usize> {
    let element = source_axis.get("element").and_then(Value::as_str)?;
    let axis_id = source_axis
        .get("axisId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    let mut fallback = None;
    for (index, child) in plot_area.children.iter().enumerate() {
        if child.name != element {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(index);
        }
        if !axis_id.is_empty()
            && direct_child(child, "axId")
                .and_then(|id| id.attr("val"))
                .is_some_and(|value| value.trim() == axis_id)
        {
            return Some(index);
        }
    }
    fallback
}

pub(super) fn apply_series_style_from_source(
    plot_area: &mut XmlNode,
    ctx: &ChartXmlContext,
    series_index: usize,
    source_series: &Value,
) -> bool {
    let paths = series_node_paths_in_plot_area(plot_area);
    let Some((chart_type_index, series_child_index)) = paths.get(series_index).copied() else {
        return false;
    };
    let chart_type = plot_area.children[chart_type_index].name.clone();
    let series = &mut plot_area.children[chart_type_index].children[series_child_index];
    let fill = source_series.get("fillColor").and_then(Value::as_str);
    let line = source_series.get("lineColor").and_then(Value::as_str);
    let width = source_series.get("lineWidthPt").and_then(Value::as_f64);
    if fill.is_some_and(|value| !value.trim().is_empty())
        || line.is_some_and(|value| !value.trim().is_empty())
        || width.is_some()
    {
        let sp_pr_index = ensure_child_index(series, "spPr", ctx.c("spPr"), SERIES_CHILD_ORDER);
        let sp_pr = &mut series.children[sp_pr_index];
        if let Some(fill) = fill.filter(|value| !value.trim().is_empty()) {
            apply_color_fill(sp_pr, ctx, fill, SHAPE_PROPS_CHILD_ORDER);
        }
        if line.is_some_and(|value| !value.trim().is_empty()) || width.is_some() {
            let line_index = ensure_child_index(sp_pr, "ln", ctx.a("ln"), SHAPE_PROPS_CHILD_ORDER);
            let line_node = &mut sp_pr.children[line_index];
            if let Some(width) = width {
                line_node.set_attr("w", &((width * 12700.0 + 0.5) as i64).to_string());
            }
            if let Some(color) = line.filter(|value| !value.trim().is_empty()) {
                apply_color_fill(line_node, ctx, color, LINE_CHILD_ORDER);
            }
        }
    }
    if let Some(marker) = source_series.get("marker")
        && matches!(
            chart_type.as_str(),
            "lineChart" | "scatterChart" | "radarChart"
        )
    {
        let marker_index =
            ensure_child_index(series, "marker", ctx.c("marker"), SERIES_CHILD_ORDER);
        let marker_node = &mut series.children[marker_index];
        if let Some(symbol) = marker.get("symbol").and_then(Value::as_str)
            && !symbol.trim().is_empty()
        {
            set_or_create_val_child(marker_node, ctx, "symbol", symbol, MARKER_CHILD_ORDER);
        }
        if let Some(size) = marker.get("size").and_then(Value::as_i64)
            && size > 0
        {
            set_or_create_val_child(
                marker_node,
                ctx,
                "size",
                &size.to_string(),
                MARKER_CHILD_ORDER,
            );
        }
    }
    true
}

pub(super) fn series_node_paths_in_plot_area(plot_area: &XmlNode) -> Vec<(usize, usize)> {
    let mut paths = Vec::new();
    for (chart_type_index, chart_type) in plot_area.children.iter().enumerate() {
        if !chart_type.name.ends_with("Chart") {
            continue;
        }
        for (series_index, series) in chart_type.children.iter().enumerate() {
            if series.name == "ser" {
                paths.push((chart_type_index, series_index));
            }
        }
    }
    paths
}

pub(super) fn apply_color_fill(
    holder: &mut XmlNode,
    ctx: &ChartXmlContext,
    color: &str,
    order: &[&str],
) {
    let trimmed = color.trim();
    if let Some(scheme) = trimmed.strip_prefix("scheme:") {
        remove_fill_group_children(holder);
        let mut solid = XmlNode::new(ctx.a("solidFill"));
        let mut scheme_clr = XmlNode::new(ctx.a("schemeClr"));
        scheme_clr.set_attr("val", scheme);
        solid.children.push(scheme_clr);
        insert_child_in_order(holder, solid, order);
    } else {
        set_solid_fill(holder, ctx, &trimmed.to_ascii_uppercase(), order);
    }
}

pub(super) fn set_solid_fill(
    holder: &mut XmlNode,
    ctx: &ChartXmlContext,
    color: &str,
    order: &[&str],
) {
    remove_fill_group_children(holder);
    let mut solid = XmlNode::new(ctx.a("solidFill"));
    let mut srgb = XmlNode::new(ctx.a("srgbClr"));
    srgb.set_attr("val", color);
    solid.children.push(srgb);
    insert_child_in_order(holder, solid, order);
}

pub(super) fn remove_fill_group_children(holder: &mut XmlNode) {
    holder.children.retain(|child| {
        !matches!(
            child.name.as_str(),
            "noFill" | "solidFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill"
        )
    });
}

pub(super) fn fill_matches(current: &str, expected: &str) -> bool {
    if expected.trim().is_empty() || expected.trim().eq_ignore_ascii_case("none") {
        current.is_empty()
    } else {
        current.eq_ignore_ascii_case(expected.trim())
    }
}

pub(super) fn bool_attr(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

pub(super) fn series_name_text(tx: &XmlNode) -> String {
    let mut parts = descendants(tx, "v")
        .into_iter()
        .map(node_text)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        parts = descendants(tx, "t")
            .into_iter()
            .map(node_text)
            .collect::<Vec<_>>();
    }
    parts.join("").trim().to_string()
}

pub(super) fn split_sheet_range_formula(formula: &str) -> (String, String) {
    let formula = formula.trim().trim_start_matches('=');
    if formula.is_empty() {
        return (String::new(), String::new());
    }
    let bytes = formula.as_bytes();
    let mut in_quote = false;
    let mut bang = None;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => {
                if in_quote && i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 1;
                } else {
                    in_quote = !in_quote;
                }
            }
            b'!' if !in_quote => bang = Some(i),
            _ => {}
        }
        i += 1;
    }
    let Some(bang) = bang else {
        return (String::new(), String::new());
    };
    let mut sheet = formula[..bang].to_string();
    let range = &formula[bang + 1..];
    if sheet.starts_with('\'') && sheet.ends_with('\'') && sheet.len() >= 2 {
        sheet = sheet[1..sheet.len() - 1].replace("''", "'");
    }
    if sheet.contains(['[', ']']) || range.contains(['[', ']', ',']) {
        return (String::new(), String::new());
    }
    let Some(normalized) = normalize_formula_range(range) else {
        return (String::new(), String::new());
    };
    (sheet, normalized)
}
