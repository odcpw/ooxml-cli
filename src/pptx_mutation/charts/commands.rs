use super::data::*;
use super::package::*;
use super::style::*;
use super::xml::{parse_chart_xml, serialize_xml};
use super::*;

pub(crate) fn pptx_charts_create(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let chart_type = parse_string_flag(args, "--type")?
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    if chart_type.is_empty() {
        return Err(CliError::invalid_args(
            "--type is required (bar, line, area, pie, scatter)",
        ));
    }
    if !matches!(
        chart_type.as_str(),
        "bar" | "line" | "area" | "pie" | "scatter"
    ) {
        return Err(CliError::invalid_args(format!(
            "failed to create chart: invalid chart type {chart_type:?} (bar, line, area, pie, scatter)"
        )));
    }
    let title = parse_string_flag(args, "--title")?.unwrap_or_default();
    let source = resolve_chart_create_source(args)?;
    let geometry = resolve_chart_create_geometry(file, args)?;
    let options = parse_chart_mutation_options(args)?;

    let create = create_slide_chart_package_updates(
        file,
        slide as usize,
        &chart_type,
        &title,
        &source,
        &geometry,
        &options,
    )?;

    let output_path = chart_mutation_output_path(file, &options);
    let readback_path = if options.dry_run {
        create.staged_path.clone()
    } else if options.in_place || output_path.as_deref() == Some(file) {
        finish_chart_mutation(file, &create.staged_path, &options, output_path.as_deref())?;
        file.to_string()
    } else {
        create.staged_path.clone()
    };

    let mut chart = selected_chart_json(
        &readback_path,
        slide,
        &format!("part:{}", create.result.chart_uri),
    )?;
    if let Some(object) = chart.as_object_mut() {
        object.remove("style");
    }
    if options.dry_run
        && let Some(object) = chart.as_object_mut()
    {
        object.remove("showCommand");
    }
    if options.dry_run {
        let _ = fs::remove_file(&create.staged_path);
    }

    Ok(chart_create_result_json(ChartCreateResultInput {
        file,
        output_path: output_path.as_deref(),
        dry_run: options.dry_run,
        slide,
        create: &create.result,
        source: &source,
        geometry: &geometry,
        chart,
    }))
}

pub(crate) fn pptx_charts_update_data(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let slide = parse_chart_slide(args)?;
    let chart_selector = parse_string_flag(args, "--chart")?;
    let series = parse_i64_flag(args, "--series")?.unwrap_or(1);
    if series < 1 {
        return Err(CliError::invalid_args("--series must be >= 1"));
    }
    let values_changed = value_flag_present(args, "--values");
    let values_json_changed = value_flag_present(args, "--values-json");
    let categories_changed = value_flag_present(args, "--categories");
    let categories_json_changed = value_flag_present(args, "--categories-json");
    if values_changed && values_json_changed {
        return Err(CliError::invalid_args(
            "specify only one of --values or --values-json",
        ));
    }
    if categories_changed && categories_json_changed {
        return Err(CliError::invalid_args(
            "specify only one of --categories or --categories-json",
        ));
    }
    if !values_changed && !values_json_changed && !categories_changed && !categories_json_changed {
        return Err(CliError::invalid_args(
            "must specify --values, --values-json, --categories, or --categories-json",
        ));
    }
    let expect_point_count = parse_i64_flag(args, "--expect-point-count")?.unwrap_or(0);
    if expect_point_count < 0 {
        return Err(CliError::invalid_args("--expect-point-count must be >= 0"));
    }
    let expect_values_hash = parse_string_flag(args, "--expect-values-hash")?.unwrap_or_default();
    let input_roles = resolve_update_input_roles(args)?;
    if input_roles.is_empty() {
        return Err(CliError::invalid_args(
            "at least one non-empty values or categories input is required",
        ));
    }
    if input_roles.len() == 2 && input_roles[0].values.len() != input_roles[1].values.len() {
        return Err(CliError::invalid_args(format!(
            "values and categories must have the same point count when both are supplied ({} vs {})",
            input_roles[0].values.len(),
            input_roles[1].values.len()
        )));
    }
    let options = parse_chart_mutation_options(args)?;
    let selected = selected_chart(file, slide, chart_selector.as_deref())?;
    let chart_xml_text = zip_text(file, &selected.part_name())?;
    let mut chart_xml = parse_chart_xml(&chart_xml_text)?;
    let total_series = series_count(&chart_xml.root);
    if series as usize > total_series {
        return Err(CliError::invalid_args(format!(
            "series {series} is out of range (1-{total_series})"
        )));
    }

    let values_snapshot =
        read_series_source(&chart_xml, series as usize, "values").map_err(|err| {
            CliError::invalid_args(format!(
                "failed to inspect current values source: {}",
                err.message
            ))
        })?;
    let current_values_hash = chart_values_hash(&values_snapshot.values);
    if !expect_values_hash.trim().is_empty()
        && !chart_hash_matches(&current_values_hash, &expect_values_hash)
    {
        return Err(CliError::invalid_args(format!(
            "--expect-values-hash mismatch: current {current_values_hash}"
        )));
    }

    let mut warnings = Vec::new();
    if selected.embedded_workbook_part_uri.is_empty() {
        warnings.push("chart has no embedded workbook; updated chart cache only".to_string());
    }
    let mut embedded_workbook_bytes = if selected.embedded_workbook_part_uri.is_empty() {
        None
    } else {
        Some(zip_bytes(
            file,
            selected.embedded_workbook_part_uri.trim_start_matches('/'),
        )?)
    };
    let mut embedded_updated = false;
    let mut updated_roles = Vec::new();

    for input_role in input_roles {
        let snapshot =
            read_series_source(&chart_xml, series as usize, &input_role.role).map_err(|err| {
                CliError::invalid_args(format!(
                    "failed to inspect current {} source: {}",
                    input_role.role, err.message
                ))
            })?;
        if snapshot.formula.trim().is_empty() {
            return Err(CliError::invalid_args(format!(
                "series {series} {} source has no editable local source formula",
                input_role.role
            )));
        }
        if parse_local_range_formula(&snapshot.formula).is_none() {
            return Err(CliError::invalid_args(format!(
                "series {series} {} source formula {:?} is not a supported local A1 range",
                input_role.role, snapshot.formula
            )));
        }
        if expect_point_count > 0 && snapshot.point_count != expect_point_count as usize {
            return Err(CliError::invalid_args(format!(
                "--expect-point-count mismatch for {}: current {}",
                input_role.role, snapshot.point_count
            )));
        }
        let cache_points = chart_cache_points_for_values(&input_role.values, &snapshot.ref_kind)
            .map_err(|message| {
                CliError::invalid_args(format!(
                    "{} input is invalid for chart source: {message}",
                    input_role.role
                ))
            })?;
        let mutation = set_series_source(
            &mut chart_xml,
            series as usize,
            &input_role.role,
            &snapshot.formula,
            &cache_points,
        )
        .map_err(|err| {
            CliError::invalid_args(format!(
                "failed to update chart {} cache: {}",
                input_role.role, err.message
            ))
        })?;
        warnings.extend(mutation.warnings.clone());

        let mut embedded_role_updated = false;
        if let Some(bytes) = embedded_workbook_bytes.as_mut() {
            let updated =
                update_embedded_workbook_chart_range(file, bytes, &snapshot, &input_role.values)?;
            if let Some(updated) = updated {
                *bytes = updated;
                embedded_role_updated = true;
                embedded_updated = true;
            } else {
                warnings.push(format!(
                    "could not update embedded workbook range for {}; updated chart cache only",
                    input_role.role
                ));
            }
        }

        updated_roles.push(UpdatedRoleResult {
            role: input_role.role,
            previous_values_hash: chart_values_hash(&snapshot.values),
            snapshot,
            mutation,
            embedded_workbook_range_updated: embedded_role_updated,
        });
    }

    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(selected.part_name(), serialize_xml(&chart_xml.root));
    let mut binary_overrides = BTreeMap::new();
    if embedded_updated && let Some(bytes) = embedded_workbook_bytes {
        binary_overrides.insert(
            selected
                .embedded_workbook_part_uri
                .trim_start_matches('/')
                .to_string(),
            bytes,
        );
    }
    let staged_path =
        stage_chart_package_mutation(file, &text_overrides, &binary_overrides, &options)?;
    let output_path = chart_mutation_output_path(file, &options);
    let readback_path = if options.dry_run {
        staged_path.clone()
    } else if options.in_place || output_path.as_deref() == Some(file) {
        finish_chart_mutation(file, &staged_path, &options, output_path.as_deref())?;
        file.to_string()
    } else {
        staged_path.clone()
    };
    let mut chart = selected_chart_json(&readback_path, slide, &selected.part_selector())?;
    if let Some(object) = chart.as_object_mut() {
        object.remove("style");
    }
    if options.dry_run
        && let Some(object) = chart.as_object_mut()
    {
        object.remove("showCommand");
    }
    if options.dry_run {
        let _ = fs::remove_file(&staged_path);
    }

    Ok(chart_update_data_result_json(ChartUpdateDataResultInput {
        file,
        output_path: output_path.as_deref(),
        dry_run: options.dry_run,
        slide,
        series,
        chart,
        selected: &selected,
        updated_roles,
        embedded_updated,
        current_values_hash: &current_values_hash,
        expect_values_hash: &expect_values_hash,
        warnings: unique_sorted_warnings(warnings),
    }))
}

pub(crate) fn pptx_charts_set_title(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_chart_slide(args)?;
    let chart_selector = parse_string_flag(args, "--chart")?;
    let title = parse_string_flag(args, "--title")?
        .ok_or_else(|| CliError::invalid_args("--title is required"))?;
    let expect_title = parse_string_flag(args, "--expect-title")?;
    let font = parse_font_options(args)?;
    let options = parse_chart_mutation_options(args)?;
    mutate_chart(
        file,
        slide,
        chart_selector.as_deref(),
        options,
        "pptx.chart.set-title",
        |chart_xml, chart| {
            let previous_title =
                set_chart_title(chart_xml, &title, expect_title.as_deref(), &font)?;
            let mut fields = Map::new();
            if !previous_title.is_empty() {
                fields.insert("previousTitle".to_string(), json!(previous_title));
            }
            Ok((serialize_xml(&chart_xml.root), fields, chart.part_name()))
        },
    )
}

pub(crate) fn pptx_charts_set_legend(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_chart_slide(args)?;
    let chart_selector = parse_string_flag(args, "--chart")?;
    let position = parse_string_flag(args, "--position")?;
    let overlay = parse_bool_flag(args, "--overlay")?;
    if position.as_deref().unwrap_or_default().trim().is_empty() && overlay.is_none() {
        return Err(CliError::invalid_args(
            "must specify at least one legend property flag",
        ));
    }
    let expect_position = parse_string_flag(args, "--expect-position")?;
    let options = parse_chart_mutation_options(args)?;
    mutate_chart(
        file,
        slide,
        chart_selector.as_deref(),
        options,
        "pptx.chart.set-legend",
        |chart_xml, chart| {
            let removed = set_chart_legend(
                chart_xml,
                position.as_deref(),
                overlay,
                expect_position.as_deref(),
            )?;
            let mut fields = Map::new();
            if removed {
                fields.insert("legendRemoved".to_string(), json!(true));
            }
            Ok((serialize_xml(&chart_xml.root), fields, chart.part_name()))
        },
    )
}

pub(crate) fn pptx_charts_set_plot_area_fill(file: &str, args: &[String]) -> CliResult<Value> {
    set_area_fill_command(file, args, AreaFillTarget::PlotArea)
}

pub(crate) fn pptx_charts_set_chart_area_fill(file: &str, args: &[String]) -> CliResult<Value> {
    set_area_fill_command(file, args, AreaFillTarget::ChartArea)
}

pub(crate) fn pptx_charts_set_series_style(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_chart_slide(args)?;
    let chart_selector = parse_string_flag(args, "--chart")?;
    let series = parse_i64_flag(args, "--series")?
        .ok_or_else(|| CliError::invalid_args("--series is required"))?;
    if series < 1 {
        return Err(CliError::invalid_args("--series must be >= 1"));
    }
    let fill_color = parse_optional_color(args, "--fill-color")?;
    let line_color = parse_optional_color(args, "--line-color")?;
    let line_width_pt = parse_optional_positive_f64(args, "--line-width-pt")?;
    let marker_symbol = parse_string_flag(args, "--marker-symbol")?;
    let marker_size = parse_i64_flag(args, "--marker-size")?;
    if marker_size.is_some_and(|value| !(2..=72).contains(&value)) {
        return Err(CliError::invalid_args(
            "--marker-size must be between 2 and 72",
        ));
    }
    if fill_color.is_none()
        && line_color.is_none()
        && line_width_pt.is_none()
        && marker_symbol
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        && marker_size.is_none()
    {
        return Err(CliError::invalid_args(
            "must specify at least one series style flag",
        ));
    }
    let expect_series_count = parse_i64_flag(args, "--expect-series-count")?;
    let options = parse_chart_mutation_options(args)?;
    mutate_chart(
        file,
        slide,
        chart_selector.as_deref(),
        options,
        "pptx.chart.set-series-style",
        |chart_xml, chart| {
            set_series_style(
                chart_xml,
                series as usize,
                &SeriesStyleSpec {
                    fill_color: fill_color.as_deref(),
                    line_color: line_color.as_deref(),
                    line_width_pt,
                    marker_symbol: marker_symbol.as_deref(),
                    marker_size,
                    expect_series_count,
                },
            )?;
            let mut fields = Map::new();
            fields.insert("series".to_string(), json!(series));
            Ok((serialize_xml(&chart_xml.root), fields, chart.part_name()))
        },
    )
}

pub(crate) fn pptx_charts_convert_type(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_chart_slide(args)?;
    let chart_selector = parse_string_flag(args, "--chart")?;
    let target = parse_required_chart_type(args, "--to")?;
    let expect_type = if value_flag_present(args, "--expect-type") {
        Some(parse_required_chart_type(args, "--expect-type")?)
    } else {
        None
    };
    let options = parse_chart_mutation_options(args)?;
    mutate_chart(
        file,
        slide,
        chart_selector.as_deref(),
        options,
        "pptx.chart.convert-type",
        |chart_xml, chart| {
            let result = convert_chart_type(chart_xml, target, expect_type)?;
            let mut fields = Map::new();
            fields.insert(
                "previousType".to_string(),
                json!(result.previous_type.as_str()),
            );
            fields.insert("newType".to_string(), json!(target.as_str()));
            if !result.warnings.is_empty() {
                fields.insert("warnings".to_string(), json!(result.warnings));
            }
            Ok((serialize_xml(&chart_xml.root), fields, chart.part_name()))
        },
    )
}

pub(crate) fn pptx_charts_set_axis(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_chart_slide(args)?;
    let chart_selector = parse_string_flag(args, "--chart")?;
    let axis = parse_axis_kind(args)?;
    let flags = parse_axis_flags(args)?;
    let expect_title = if value_flag_present(args, "--expect-axis-title") {
        Some(parse_string_flag(args, "--expect-axis-title")?.unwrap_or_default())
    } else {
        None
    };
    let expect_count = parse_i64_flag(args, "--expect-axis-count")?;
    if let Some(count) = expect_count
        && count < 0
    {
        return Err(CliError::invalid_args("--expect-axis-count must be >= 0"));
    }
    let options = parse_chart_mutation_options(args)?;
    mutate_chart(
        file,
        slide,
        chart_selector.as_deref(),
        options,
        "pptx.chart.set-axis",
        |chart_xml, chart| {
            let previous_title = set_chart_axis(
                chart_xml,
                axis,
                &flags,
                expect_title.as_deref(),
                expect_count,
            )?;
            let mut fields = Map::new();
            if !previous_title.is_empty() {
                fields.insert("previousTitle".to_string(), json!(previous_title));
            }
            Ok((serialize_xml(&chart_xml.root), fields, chart.part_name()))
        },
    )
}

pub(crate) fn pptx_charts_copy_style(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_chart_slide(args)?;
    let chart_selector = parse_string_flag(args, "--chart")?;
    let to_chart_selector = parse_string_flag(args, "--to-chart")?;
    if chart_selector.is_some() && to_chart_selector.is_some() {
        return Err(CliError::invalid_args(
            "use --chart or --to-chart, not both",
        ));
    }
    let target_selector = to_chart_selector.or(chart_selector);
    let from_file = parse_string_flag(args, "--from")?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| CliError::invalid_args("--from <template-file> is required"))?;
    fs::metadata(&from_file)
        .map_err(|_| CliError::file_not_found(format!("file not found: {from_file}")))?;
    let from_slide = parse_chart_slide_flag(args, "--from-slide")?;
    let from_chart_selector = parse_string_flag(args, "--from-chart")?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "chart:1".to_string());
    let expect_series_count = parse_i64_flag(args, "--expect-series-count")?;
    if let Some(count) = expect_series_count
        && count < 0
    {
        return Err(CliError::invalid_args("--expect-series-count must be >= 0"));
    }
    let source = read_template_chart_style(&from_file, from_slide, &from_chart_selector)?;
    let options = parse_chart_mutation_options(args)?;
    mutate_chart(
        file,
        slide,
        target_selector.as_deref(),
        options,
        "pptx.chart.copy-style",
        |chart_xml, chart| {
            let applied = apply_chart_style(chart_xml, &source, expect_series_count)?;
            let mut fields = Map::new();
            if !applied.is_empty() {
                fields.insert("appliedStyle".to_string(), json!(applied));
            }
            Ok((serialize_xml(&chart_xml.root), fields, chart.part_name()))
        },
    )
}

pub(super) enum AreaFillTarget {
    ChartArea,
    PlotArea,
}

fn set_area_fill_command(file: &str, args: &[String], target: AreaFillTarget) -> CliResult<Value> {
    let slide = parse_chart_slide(args)?;
    let chart_selector = parse_string_flag(args, "--chart")?;
    let fill = parse_required_color(args, "--fill-color")?;
    let expect_fill = parse_optional_color(args, "--expect-fill")?;
    let options = parse_chart_mutation_options(args)?;
    let action = match target {
        AreaFillTarget::ChartArea => "pptx.chart.set-chart-area-fill",
        AreaFillTarget::PlotArea => "pptx.chart.set-plot-area-fill",
    };
    mutate_chart(
        file,
        slide,
        chart_selector.as_deref(),
        options,
        action,
        |chart_xml, chart| {
            let previous_fill = set_area_fill(chart_xml, &target, &fill, expect_fill.as_deref())?;
            let mut fields = Map::new();
            if !previous_fill.is_empty() {
                fields.insert("previousFill".to_string(), json!(previous_fill));
            }
            if !fill.is_empty() {
                fields.insert("newFill".to_string(), json!(fill.clone()));
            }
            Ok((serialize_xml(&chart_xml.root), fields, chart.part_name()))
        },
    )
}
