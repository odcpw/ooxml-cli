use super::*;

pub(crate) fn xlsx_charts_list(file: &str, sheet_selector: Option<&str>) -> CliResult<Value> {
    let charts = load_xlsx_charts(file, sheet_selector)?;
    Ok(xlsx_charts_result(file, charts))
}

pub(crate) fn xlsx_charts_show(
    file: &str,
    sheet_selector: Option<&str>,
    chart_selector: Option<&str>,
) -> CliResult<Value> {
    let charts = load_xlsx_charts(file, sheet_selector)?;
    let chart = select_xlsx_chart(&charts, chart_selector.unwrap_or_default())?;
    Ok(xlsx_charts_result(file, vec![chart]))
}

pub(crate) fn xlsx_charts_create(
    file: &str,
    options: XlsxChartCreateOptions<'_>,
) -> CliResult<Value> {
    ensure_xlsx_file_exists(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let chart_type = parse_create_chart_type(options.chart_type)?;
    let source = resolve_chart_create_source(file, &options)?;
    if let Some(expect_range) = options
        .expect_source_range
        .filter(|value| !value.trim().is_empty())
        && !source.range.eq_ignore_ascii_case(expect_range.trim())
    {
        return Err(CliError::invalid_args(format!(
            "source range mismatch: expected {} but found {}",
            expect_range, source.range
        )));
    }
    let (anchor_from, anchor_to) = resolve_chart_create_anchor(options.anchor, source.bounds)?;
    let artifacts = build_chart_create_artifacts(
        file,
        &source,
        &chart_type,
        options.title.unwrap_or_default(),
        anchor_from,
        anchor_to,
    )
    .map_err(|err| CliError::invalid_args(format!("failed to create chart: {}", err.message)))?;

    let output_path = options.out.filter(|value| !value.trim().is_empty());
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

    copy_zip_with_part_overrides(file, &readback_path, &artifacts.overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }

    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
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

    Ok(xlsx_chart_create_result(
        file,
        &source,
        &artifacts,
        commit_path,
        options.dry_run,
    ))
}

pub(crate) fn xlsx_charts_update_source(
    file: &str,
    options: XlsxChartUpdateSourceOptions<'_>,
) -> CliResult<Value> {
    ensure_xlsx_file_exists(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    if options.series < 1 {
        return Err(CliError::invalid_args("--series must be >= 1"));
    }
    let role = normalize_chart_source_role(options.role.unwrap_or("values"))?;
    let cache_mode = normalize_chart_cache_mode(options.cache.unwrap_or("auto"))?;
    if options.max_cells < 0 {
        return Err(CliError::invalid_args("--max-cells must be >= 0"));
    }
    let formula_changed = options
        .formula
        .is_some_and(|value| !value.trim().is_empty());
    let range_changed = options
        .source_range
        .is_some_and(|value| !value.trim().is_empty());
    if formula_changed == range_changed {
        return Err(CliError::invalid_args(
            "must specify exactly one of --formula or --source-range",
        ));
    }

    let workbook_xml = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook_xml)?;
    let charts = load_xlsx_charts(file, options.sheet)?;
    let selected = select_xlsx_chart(&charts, options.chart.unwrap_or_default())?;
    let current_source = chart_series_source_by_role(&selected, options.series as usize, &role)?;
    let resolved_source = resolve_chart_update_source(file, &sheets, &current_source, &options)?;
    let rows = resolved_source.bounds.row_count();
    let cols = resolved_source.bounds.col_count();
    if rows != 1 && cols != 1 {
        return Err(CliError::invalid_args(format!(
            "chart source range {} is {}x{}; update-source currently requires a one-row or one-column series range",
            resolved_source.range, rows, cols
        )));
    }

    let cache_update = if cache_mode == ChartCacheMode::Auto {
        collect_chart_cache_points(
            file,
            &resolved_source.sheet,
            &resolved_source.range,
            resolved_source.bounds,
            &current_source.ref_kind,
            options.max_cells,
        )?
    } else {
        ChartCacheUpdate::default()
    };

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
    let mutation = apply_chart_update_source(
        &mut root,
        &ctx,
        options.series as usize,
        &role,
        &resolved_source.formula,
        cache_mode,
        &cache_update,
        options.expect_formula,
        options.expect_source_range,
    )?;
    let updated_xml = render_xml_document(&root);

    let output_path = options.out.filter(|value| !value.trim().is_empty());
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

    copy_zip_with_part_override(file, &readback_path, &chart_part, &updated_xml)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let readback_charts = load_xlsx_charts(&readback_path, options.sheet)?;
    let readback = select_xlsx_chart(&readback_charts, &format!("part:{}", selected.part_uri))?;
    let chart_item = xlsx_chart_item_for_update(commit_path, &readback);

    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
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

    Ok(xlsx_chart_update_source_result(
        ChartUpdateSourceResultInput {
            file,
            output: commit_path,
            dry_run: options.dry_run,
            sheet_selector: options.sheet,
            chart_selector: options.chart,
            series: options.series,
            role: &role,
            chart_item,
            mutation: &mutation,
            source: &resolved_source,
            cache_update: &cache_update,
        },
    ))
}

pub(crate) fn xlsx_charts_set_title(
    file: &str,
    options: XlsxChartSetTitleOptions<'_>,
) -> CliResult<Value> {
    let font = ChartFontOptions {
        family: normalize_optional_nonempty(options.font_family, "--font-family")?,
        size_pt: options.font_size_pt,
        color: normalize_optional_hex(options.font_color)?,
        bold: options.font_bold,
        italic: options.font_italic,
    };
    if let Some(size) = font.size_pt
        && size <= 0.0
    {
        return Err(CliError::invalid_args("--font-size must be greater than 0"));
    }
    let expect_title = if options.expect_title_present {
        Some(options.expect_title.unwrap_or_default().to_string())
    } else {
        None
    };
    run_xlsx_chart_style_mutation(
        file,
        options.sheet,
        options.chart,
        "xlsx.chart.set-title",
        XlsxChartOutputOptions::from_title(&options),
        |root, ctx| {
            let previous =
                apply_chart_set_title(root, ctx, options.title, expect_title.as_deref(), &font)?;
            Ok(ChartMutationExtra {
                previous_title: Some(previous),
                ..ChartMutationExtra::default()
            })
        },
    )
}

pub(crate) fn xlsx_charts_set_legend(
    file: &str,
    options: XlsxChartSetLegendOptions<'_>,
) -> CliResult<Value> {
    if !options.position_present && options.overlay.is_none() {
        return Err(CliError::invalid_args(
            "set-legend requires --position and/or --overlay",
        ));
    }
    let legend_position = if options.position_present {
        Some(parse_chart_legend_position(
            options.position.unwrap_or_default(),
        )?)
    } else {
        None
    };
    if legend_position
        .as_ref()
        .is_some_and(|position| position.remove)
        && options.overlay.is_some()
    {
        return Err(CliError::invalid_args(
            "--overlay cannot be combined with --position none",
        ));
    }
    let expect_position = if options.expect_position_present {
        Some(parse_chart_expect_legend_position(
            options.expect_position.unwrap_or_default(),
        )?)
    } else {
        None
    };
    run_xlsx_chart_style_mutation(
        file,
        options.sheet,
        options.chart,
        "xlsx.chart.set-legend",
        XlsxChartOutputOptions::from_legend(&options),
        |root, ctx| {
            let removed = apply_chart_set_legend(
                root,
                ctx,
                legend_position.as_ref(),
                options.overlay,
                expect_position.as_deref(),
            )?;
            Ok(ChartMutationExtra {
                legend_removed: removed,
                ..ChartMutationExtra::default()
            })
        },
    )
}

pub(crate) fn xlsx_charts_set_chart_area_fill(
    file: &str,
    options: XlsxChartSetFillOptions<'_>,
) -> CliResult<Value> {
    xlsx_charts_set_fill(file, options, XlsxChartFillTarget::ChartArea)
}

pub(crate) fn xlsx_charts_set_plot_area_fill(
    file: &str,
    options: XlsxChartSetFillOptions<'_>,
) -> CliResult<Value> {
    xlsx_charts_set_fill(file, options, XlsxChartFillTarget::PlotArea)
}

pub(super) fn xlsx_charts_set_fill(
    file: &str,
    options: XlsxChartSetFillOptions<'_>,
    target: XlsxChartFillTarget,
) -> CliResult<Value> {
    let fill = parse_chart_fill_color(options.fill_color)?;
    let expect_fill = if options.expect_fill_present {
        Some(resolve_chart_expect_fill(
            options.expect_fill.unwrap_or_default(),
        )?)
    } else {
        None
    };
    let action = match target {
        XlsxChartFillTarget::ChartArea => "xlsx.chart.set-chart-area-fill",
        XlsxChartFillTarget::PlotArea => "xlsx.chart.set-plot-area-fill",
    };
    run_xlsx_chart_style_mutation(
        file,
        options.sheet,
        options.chart,
        action,
        XlsxChartOutputOptions::from_fill(&options),
        |root, ctx| {
            let (previous, new_fill) =
                apply_chart_set_fill(root, ctx, &target, &fill, expect_fill.as_deref())?;
            Ok(ChartMutationExtra {
                previous_fill: if previous.is_empty() {
                    None
                } else {
                    Some(previous)
                },
                new_fill: Some(new_fill),
                ..ChartMutationExtra::default()
            })
        },
    )
}

pub(crate) fn xlsx_charts_set_series_style(
    file: &str,
    options: XlsxChartSetSeriesStyleOptions<'_>,
) -> CliResult<Value> {
    if options.series < 1 {
        return Err(CliError::invalid_args("--series must be >= 1"));
    }
    let style = ChartSeriesStyleOptions {
        fill_color: normalize_optional_hex(options.fill_color)?,
        line_color: normalize_optional_hex(options.line_color)?,
        line_width_pt: options.line_width_pt,
        marker_symbol: options
            .marker_symbol
            .map(parse_chart_marker_symbol)
            .transpose()?,
        marker_size: options.marker_size,
    };
    if let Some(width) = style.line_width_pt
        && width <= 0.0
    {
        return Err(CliError::invalid_args(
            "--line-width-pt must be greater than 0",
        ));
    }
    if let Some(size) = style.marker_size
        && !(2..=72).contains(&size)
    {
        return Err(CliError::invalid_args(
            "--marker-size must be between 2 and 72",
        ));
    }
    if style.is_empty() {
        return Err(CliError::invalid_args(
            "set-series-style requires at least one of --fill-color, --line-color, --line-width-pt, --marker-symbol, or --marker-size",
        ));
    }
    if let Some(count) = options.expect_series_count
        && count < 0
    {
        return Err(CliError::invalid_args("--expect-series-count must be >= 0"));
    }
    run_xlsx_chart_style_mutation(
        file,
        options.sheet,
        options.chart,
        "xlsx.chart.set-series-style",
        XlsxChartOutputOptions::from_series(&options),
        |root, ctx| {
            apply_chart_set_series_style(
                root,
                ctx,
                options.series as usize,
                options.expect_series_count.map(|value| value as usize),
                &style,
            )?;
            Ok(ChartMutationExtra {
                series: Some(options.series),
                ..ChartMutationExtra::default()
            })
        },
    )
}

pub(crate) fn xlsx_charts_convert_type(
    file: &str,
    options: XlsxChartConvertTypeOptions<'_>,
) -> CliResult<Value> {
    let target_type = parse_chart_type(
        options.to.ok_or_else(|| {
            CliError::invalid_args("--to is required (bar, column, line, area, pie, or scatter)")
        })?,
        "--to",
    )?;
    let expect_type = if options.expect_type_present {
        Some(parse_chart_type(
            options.expect_type.unwrap_or_default(),
            "--expect-type",
        )?)
    } else {
        None
    };
    run_xlsx_chart_style_mutation(
        file,
        options.sheet,
        options.chart,
        "xlsx.chart.convert-type",
        XlsxChartOutputOptions::from_convert(&options),
        |root, ctx| {
            let conversion =
                apply_chart_convert_type(root, ctx, &target_type, expect_type.as_deref())?;
            Ok(ChartMutationExtra {
                previous_type: Some(conversion.previous_type),
                new_type: Some(conversion.new_type),
                warnings: conversion.warnings,
                ..ChartMutationExtra::default()
            })
        },
    )
}

pub(crate) fn xlsx_charts_copy_style(
    file: &str,
    options: XlsxChartCopyStyleOptions<'_>,
) -> CliResult<Value> {
    let from = options
        .from
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| CliError::invalid_args("--from <template-file> is required"))?;
    if !Path::new(from).exists() {
        return Err(CliError::file_not_found(format!("file not found: {from}")));
    }
    if options.to_chart_present && options.chart.is_some_and(|value| !value.trim().is_empty()) {
        return Err(CliError::invalid_args(
            "use --chart or --to-chart, not both",
        ));
    }
    if let Some(count) = options.expect_series_count
        && count < 0
    {
        return Err(CliError::invalid_args("--expect-series-count must be >= 0"));
    }
    let source_style = read_xlsx_template_chart_style(from, options.from_chart)?;
    let target_chart = if options.to_chart_present {
        options.to_chart
    } else {
        options.chart
    };
    run_xlsx_chart_style_mutation(
        file,
        options.sheet,
        target_chart,
        "xlsx.chart.copy-style",
        XlsxChartOutputOptions::from_copy_style(&options),
        |root, ctx| {
            let applied = apply_chart_copy_style(
                root,
                ctx,
                &source_style,
                options.expect_series_count.map(|value| value as usize),
            )?;
            Ok(ChartMutationExtra {
                applied_style: applied,
                ..ChartMutationExtra::default()
            })
        },
    )
}

pub(crate) fn xlsx_charts_set_axis(
    file: &str,
    options: XlsxChartSetAxisOptions<'_>,
) -> CliResult<Value> {
    let axis_kind = parse_chart_axis_kind(options.axis.unwrap_or_default())?;
    let flags = resolve_chart_axis_flags(&options)?;
    let expect_title = if options.expect_axis_title_present {
        Some(options.expect_axis_title.unwrap_or_default().to_string())
    } else {
        None
    };
    if let Some(count) = options.expect_axis_count
        && count < 0
    {
        return Err(CliError::invalid_args("--expect-axis-count must be >= 0"));
    }
    run_xlsx_chart_style_mutation(
        file,
        options.sheet,
        options.chart,
        "xlsx.chart.set-axis",
        XlsxChartOutputOptions::from_axis(&options),
        |root, ctx| {
            let previous_title = apply_chart_set_axis(
                root,
                ctx,
                &axis_kind,
                &flags,
                expect_title.as_deref(),
                options.expect_axis_count.map(|value| value as usize),
            )?;
            Ok(ChartMutationExtra {
                previous_title: Some(previous_title),
                ..ChartMutationExtra::default()
            })
        },
    )
}
