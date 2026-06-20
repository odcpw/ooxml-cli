use super::xml::*;
use super::*;

pub(super) fn mutate_chart<F>(
    file: &str,
    slide: i64,
    chart_selector: Option<&str>,
    options: PptxChartMutationOptions,
    action: &str,
    build: F,
) -> CliResult<Value>
where
    F: FnOnce(&mut ChartXml, &SelectedChart) -> CliResult<(String, Map<String, Value>, String)>,
{
    ensure_pptx(file)?;
    let selected = selected_chart(file, slide, chart_selector)?;
    let chart_xml_text = zip_text(file, &selected.part_name())?;
    let mut chart_xml = parse_chart_xml(&chart_xml_text)?;
    let (updated_xml, extra_fields, part_name) = build(&mut chart_xml, &selected)?;

    let mut overrides = BTreeMap::new();
    overrides.insert(part_name, updated_xml);
    let staged_path = stage_chart_mutation(file, &overrides, &options)?;
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
    if options.dry_run
        && let Some(object) = chart.as_object_mut()
    {
        object.remove("showCommand");
    }
    if options.dry_run {
        let _ = fs::remove_file(&staged_path);
    }

    Ok(chart_mutation_result_json(ChartMutationResultInput {
        file,
        output_path: output_path.as_deref(),
        dry_run: options.dry_run,
        action,
        chart,
        extra_fields,
        slide,
        chart_selector: &selected.part_selector(),
    }))
}

pub(super) fn ensure_pptx(file: &str) -> CliResult<()> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    Ok(())
}

pub(super) fn selected_chart(
    file: &str,
    slide: i64,
    chart_selector: Option<&str>,
) -> CliResult<SelectedChart> {
    let result = pptx_charts_show(file, slide, chart_selector)?;
    let charts = result
        .get("charts")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::unexpected("pptx charts show returned no charts array"))?;
    let chart = charts
        .first()
        .ok_or_else(|| CliError::unexpected("pptx charts show returned no selected chart"))?;
    let part_uri = chart
        .get("partUri")
        .and_then(Value::as_str)
        .ok_or_else(|| CliError::unexpected("selected chart has no partUri"))?
        .to_string();
    let embedded_workbook_part_uri = chart
        .get("embeddedWorkbookPartUri")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(SelectedChart {
        part_uri,
        embedded_workbook_part_uri,
    })
}

pub(super) fn selected_chart_json(file: &str, slide: i64, selector: &str) -> CliResult<Value> {
    let result = pptx_charts_show(file, slide, Some(selector))?;
    result
        .get("charts")
        .and_then(Value::as_array)
        .and_then(|charts| charts.first())
        .cloned()
        .ok_or_else(|| CliError::unexpected("pptx charts show returned no selected chart"))
}

pub(super) fn parse_chart_slide(args: &[String]) -> CliResult<i64> {
    parse_chart_slide_flag(args, "--slide")
}

pub(super) fn parse_chart_slide_flag(args: &[String], name: &str) -> CliResult<i64> {
    let slide = parse_i64_flag(args, name)?.unwrap_or(0);
    if slide < 0 {
        return Err(CliError::invalid_args(format!("{name} must be >= 1")));
    }
    Ok(slide)
}

pub(super) fn parse_required_chart_type(args: &[String], name: &str) -> CliResult<ChartType> {
    if !value_flag_present(args, name) {
        return Err(CliError::invalid_args(format!(
            "{name} is required (bar, column, line, area, pie, or scatter)"
        )));
    }
    let value = parse_string_flag(args, name)?.unwrap_or_default();
    parse_chart_type(&value).map_err(CliError::invalid_args)
}

pub(super) fn parse_chart_type(value: &str) -> Result<ChartType, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "bar" | "barchart" => Ok(ChartType::Bar),
        "column" | "col" | "columnchart" => Ok(ChartType::Column),
        "line" | "linechart" => Ok(ChartType::Line),
        "area" | "areachart" => Ok(ChartType::Area),
        "pie" | "piechart" => Ok(ChartType::Pie),
        "scatter" | "scatterchart" | "xy" => Ok(ChartType::Scatter),
        _ => Err(format!(
            "invalid chart type {value:?} (use bar, column, line, area, pie, or scatter)"
        )),
    }
}

pub(super) fn parse_axis_kind(args: &[String]) -> CliResult<AxisKind> {
    let value = parse_string_flag(args, "--axis")?.unwrap_or_default();
    match value.trim().to_ascii_lowercase().as_str() {
        "category" => Ok(AxisKind::Category),
        "value" => Ok(AxisKind::Value),
        _ => Err(CliError::invalid_args(
            "--axis is required; use category or value",
        )),
    }
}

pub(super) fn parse_axis_flags(args: &[String]) -> CliResult<AxisFlags> {
    let min = parse_optional_f64_flag(args, "--min")?;
    let max = parse_optional_f64_flag(args, "--max")?;
    if let (Some(min), Some(max)) = (min, max)
        && min >= max
    {
        return Err(CliError::invalid_args("--min must be less than --max"));
    }
    let major_unit = parse_optional_f64_flag(args, "--major-unit")?;
    if let Some(value) = major_unit
        && value <= 0.0
    {
        return Err(CliError::invalid_args(
            "--major-unit must be greater than 0",
        ));
    }
    let number_format = if value_flag_present(args, "--number-format") {
        let value = parse_string_flag(args, "--number-format")?.unwrap_or_default();
        if value.trim().is_empty() {
            return Err(CliError::invalid_args("--number-format must not be empty"));
        }
        Some(value)
    } else {
        None
    };
    let tick_font = FontOptions {
        family: parse_optional_nonempty_string(args, "--tick-label-font-family")?,
        size_pt: parse_optional_positive_f64(args, "--tick-label-font-size")?,
        color: parse_optional_hex_color(args, "--tick-label-font-color")?,
        bold: parse_bool_flag(args, "--tick-label-font-bold")?,
        italic: parse_bool_flag(args, "--tick-label-font-italic")?,
    };
    let title_font = FontOptions {
        family: parse_optional_nonempty_string(args, "--title-font-family")?,
        size_pt: parse_optional_positive_f64(args, "--title-font-size")?,
        color: parse_optional_hex_color(args, "--title-font-color")?,
        bold: parse_bool_flag(args, "--title-font-bold")?,
        italic: parse_bool_flag(args, "--title-font-italic")?,
    };
    let flags = AxisFlags {
        set_title: value_flag_present(args, "--title"),
        title: parse_string_flag(args, "--title")?.unwrap_or_default(),
        set_hidden: parse_bool_flag(args, "--hidden")?.is_some(),
        hidden: parse_bool_flag(args, "--hidden")?.unwrap_or(false),
        min,
        max,
        major_unit,
        number_format,
        set_major_gridlines: parse_bool_flag(args, "--major-gridlines")?.is_some(),
        major_gridlines: parse_bool_flag(args, "--major-gridlines")?.unwrap_or(false),
        set_minor_gridlines: parse_bool_flag(args, "--minor-gridlines")?.is_some(),
        minor_gridlines: parse_bool_flag(args, "--minor-gridlines")?.unwrap_or(false),
        tick_font,
        title_font,
    };
    if !flags.has_any() {
        return Err(CliError::invalid_args(
            "set-axis requires at least one of --title, --hidden, --min, --max, --major-unit, --number-format, --major-gridlines, --minor-gridlines, or a --title-font-*/--tick-label-font-* flag",
        ));
    }
    Ok(flags)
}

pub(super) fn parse_optional_f64_flag(args: &[String], name: &str) -> CliResult<Option<f64>> {
    let Some(raw) = parse_string_flag(args, name)? else {
        return Ok(None);
    };
    raw.trim()
        .parse::<f64>()
        .map(Some)
        .map_err(|_| CliError::invalid_args(format!("{name} must be a number")))
}

pub(super) fn parse_optional_nonempty_string(
    args: &[String],
    name: &str,
) -> CliResult<Option<String>> {
    if !value_flag_present(args, name) {
        return Ok(None);
    }
    let value = parse_string_flag(args, name)?.unwrap_or_default();
    if value.trim().is_empty() {
        return Err(CliError::invalid_args(format!("{name} must not be empty")));
    }
    Ok(Some(value.trim().to_string()))
}

pub(super) fn parse_optional_hex_color(args: &[String], name: &str) -> CliResult<Option<String>> {
    if !value_flag_present(args, name) {
        return Ok(None);
    }
    let value = parse_string_flag(args, name)?.unwrap_or_default();
    normalize_hex_color(&value).map(Some)
}

pub(super) fn normalize_hex_color(value: &str) -> CliResult<String> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(hex.to_ascii_uppercase());
    }
    Err(CliError::invalid_args(format!(
        "color {value:?} must be a 6-digit hex like #1F77B4"
    )))
}

pub(super) fn parse_chart_mutation_options(args: &[String]) -> CliResult<PptxChartMutationOptions> {
    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxChartMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

pub(super) fn parse_font_options(args: &[String]) -> CliResult<FontOptions> {
    let family = parse_string_flag(args, "--font-family")?.filter(|value| !value.trim().is_empty());
    let size_pt = parse_optional_positive_f64(args, "--font-size")?;
    let color = parse_optional_color(args, "--font-color")?;
    let bold = parse_bool_flag(args, "--font-bold")?;
    let italic = parse_bool_flag(args, "--font-italic")?;
    Ok(FontOptions {
        family,
        size_pt,
        color,
        bold,
        italic,
    })
}

pub(super) fn parse_optional_positive_f64(args: &[String], name: &str) -> CliResult<Option<f64>> {
    let Some(raw) = parse_string_flag(args, name)? else {
        return Ok(None);
    };
    let value = raw
        .trim()
        .parse::<f64>()
        .map_err(|_| CliError::invalid_args(format!("{name} must be a number")))?;
    if value <= 0.0 {
        return Err(CliError::invalid_args(format!("{name} must be > 0")));
    }
    Ok(Some(value))
}

pub(super) fn parse_required_color(args: &[String], name: &str) -> CliResult<String> {
    if !value_flag_present(args, name) {
        return Err(CliError::invalid_args(format!("{name} is required")));
    }
    normalize_color_value(&parse_string_flag(args, name)?.unwrap_or_default(), name)
}

pub(super) fn parse_optional_color(args: &[String], name: &str) -> CliResult<Option<String>> {
    if !value_flag_present(args, name) {
        return Ok(None);
    }
    Ok(Some(normalize_color_value(
        &parse_string_flag(args, name)?.unwrap_or_default(),
        name,
    )?))
}

pub(super) fn normalize_color_value(value: &str, name: &str) -> CliResult<String> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return Ok(String::new());
    }
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(hex.to_ascii_uppercase());
    }
    Err(CliError::invalid_args(format!(
        "{name} must be a 6-digit hex color or none"
    )))
}

pub(super) fn chart_mutation_output_path(
    file: &str,
    options: &PptxChartMutationOptions,
) -> Option<String> {
    if options.dry_run {
        None
    } else if options.in_place {
        Some(file.to_string())
    } else {
        options
            .out
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    }
}

pub(super) fn stage_chart_mutation(
    file: &str,
    overrides: &BTreeMap<String, String>,
    options: &PptxChartMutationOptions,
) -> CliResult<String> {
    stage_chart_package_mutation(file, overrides, &BTreeMap::new(), options)
}

pub(super) fn stage_chart_package_mutation(
    file: &str,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    options: &PptxChartMutationOptions,
) -> CliResult<String> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-chart")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_binary_part_overrides_and_removals(
        file,
        &write_path,
        text_overrides,
        binary_overrides,
        &BTreeSet::new(),
    )?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    Ok(write_path)
}

pub(super) fn finish_chart_mutation(
    file: &str,
    staged_path: &str,
    options: &PptxChartMutationOptions,
    output_path: Option<&str>,
) -> CliResult<()> {
    if options.dry_run {
        let _ = fs::remove_file(staged_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(staged_path, file)
            .or_else(|_| {
                fs::copy(staged_path, file)?;
                fs::remove_file(staged_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

pub(super) fn chart_mutation_result_json(input: ChartMutationResultInput<'_>) -> Value {
    let ChartMutationResultInput {
        file,
        output_path,
        dry_run,
        action,
        chart,
        extra_fields,
        slide,
        chart_selector,
    } = input;
    let command_target = output_path.unwrap_or("<out.pptx>");
    let command_suffix = if dry_run { "Template" } else { "" };
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if !dry_run && let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("action".to_string(), json!(action));
    result.insert("chart".to_string(), chart);
    for (key, value) in extra_fields {
        result.insert(key, value);
    }
    result.insert(
        format!("validateCommand{command_suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    let mut show_command = format!(
        "ooxml --json pptx charts show {}",
        command_arg(command_target)
    );
    if slide > 0 {
        show_command.push_str(&format!(" --slide {slide}"));
    }
    show_command.push_str(&format!(" --chart {}", command_arg(chart_selector)));
    result.insert(
        format!("chartShowCommand{command_suffix}"),
        json!(show_command),
    );
    result.insert(
        format!("renderCommand{command_suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
    Value::Object(result)
}
