use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::{
    XlsxChartConvertTypeOptions, XlsxChartCopyStyleOptions, XlsxChartCreateOptions,
    XlsxChartSetAxisOptions, XlsxChartSetFillOptions, XlsxChartSetLegendOptions,
    XlsxChartSetSeriesStyleOptions, XlsxChartSetTitleOptions, XlsxChartUpdateSourceOptions,
    xlsx_charts_convert_type, xlsx_charts_copy_style, xlsx_charts_create, xlsx_charts_list,
    xlsx_charts_set_axis, xlsx_charts_set_chart_area_fill, xlsx_charts_set_legend,
    xlsx_charts_set_plot_area_fill, xlsx_charts_set_series_style, xlsx_charts_set_title,
    xlsx_charts_show, xlsx_charts_update_source,
};

pub(super) fn dispatch_xlsx_charts(args: &[String]) -> CliResult<Value> {
    match args {
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "charts" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--sheet"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_charts_list(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "charts" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--chart"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let chart = parse_string_flag(rest, "--chart")?;
            xlsx_charts_show(file, sheet.as_deref(), chart.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "charts" && verb == "create" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--type",
                    "--sheet",
                    "--range",
                    "--table",
                    "--title",
                    "--anchor",
                    "--expect-source-range",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let chart_type = parse_string_flag(rest, "--type")?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let table = parse_string_flag(rest, "--table")?;
            let title = parse_string_flag(rest, "--title")?;
            let anchor = parse_string_flag(rest, "--anchor")?;
            let expect_source_range = parse_string_flag(rest, "--expect-source-range")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_charts_create(
                file,
                XlsxChartCreateOptions {
                    chart_type: chart_type.as_deref(),
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    table: table.as_deref(),
                    title: title.as_deref(),
                    anchor: anchor.as_deref(),
                    expect_source_range: expect_source_range.as_deref(),
                    max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "charts" && verb == "update-source" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--chart",
                    "--series",
                    "--role",
                    "--source-sheet",
                    "--source-range",
                    "--formula",
                    "--cache",
                    "--expect-source-range",
                    "--expect-formula",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let chart = parse_string_flag(rest, "--chart")?;
            let series = parse_i64_flag(rest, "--series")?.unwrap_or(1);
            let role = parse_string_flag(rest, "--role")?;
            let source_sheet = parse_string_flag(rest, "--source-sheet")?;
            let source_range = parse_string_flag(rest, "--source-range")?;
            let formula = parse_string_flag(rest, "--formula")?;
            let cache = parse_string_flag(rest, "--cache")?;
            let expect_source_range = parse_string_flag(rest, "--expect-source-range")?;
            let expect_formula = parse_string_flag(rest, "--expect-formula")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_charts_update_source(
                file,
                XlsxChartUpdateSourceOptions {
                    sheet: sheet.as_deref(),
                    chart: chart.as_deref(),
                    series,
                    role: role.as_deref(),
                    source_sheet: source_sheet.as_deref(),
                    source_range: source_range.as_deref(),
                    formula: formula.as_deref(),
                    cache: cache.as_deref(),
                    expect_source_range: expect_source_range.as_deref(),
                    expect_formula: expect_formula.as_deref(),
                    max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "charts" && verb == "set-title" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--chart",
                    "--title",
                    "--expect-title",
                    "--font-family",
                    "--font-size",
                    "--font-color",
                    "--out",
                    "--backup",
                ],
                &[
                    "--font-bold",
                    "--font-italic",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let chart = parse_string_flag(rest, "--chart")?;
            let title = parse_string_flag(rest, "--title")?
                .ok_or_else(|| CliError::invalid_args("--title is required"))?;
            let expect_title = parse_string_flag(rest, "--expect-title")?;
            let font_family = parse_string_flag(rest, "--font-family")?;
            let font_size = parse_f64_flag(rest, "--font-size")?;
            let font_color = parse_string_flag(rest, "--font-color")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_charts_set_title(
                file,
                XlsxChartSetTitleOptions {
                    sheet: sheet.as_deref(),
                    chart: chart.as_deref(),
                    title: &title,
                    expect_title: expect_title.as_deref(),
                    expect_title_present: value_flag_present(rest, "--expect-title"),
                    font_family: font_family.as_deref(),
                    font_size_pt: font_size,
                    font_color: font_color.as_deref(),
                    font_bold: parse_bool_flag(rest, "--font-bold")?,
                    font_italic: parse_bool_flag(rest, "--font-italic")?,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "charts" && verb == "set-legend" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--chart",
                    "--position",
                    "--expect-position",
                    "--out",
                    "--backup",
                ],
                &["--overlay", "--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let chart = parse_string_flag(rest, "--chart")?;
            let position = parse_string_flag(rest, "--position")?;
            let expect_position = parse_string_flag(rest, "--expect-position")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_charts_set_legend(
                file,
                XlsxChartSetLegendOptions {
                    sheet: sheet.as_deref(),
                    chart: chart.as_deref(),
                    position: position.as_deref(),
                    position_present: value_flag_present(rest, "--position"),
                    overlay: parse_bool_flag(rest, "--overlay")?,
                    expect_position: expect_position.as_deref(),
                    expect_position_present: value_flag_present(rest, "--expect-position"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && group == "charts"
                && (verb == "set-chart-area-fill" || verb == "set-plot-area-fill") =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--chart",
                    "--fill-color",
                    "--expect-fill",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let chart = parse_string_flag(rest, "--chart")?;
            let fill_color = parse_string_flag(rest, "--fill-color")?.ok_or_else(|| {
                CliError::invalid_args("--fill-color is required (a #RRGGBB color or none)")
            })?;
            let expect_fill = parse_string_flag(rest, "--expect-fill")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let options = XlsxChartSetFillOptions {
                sheet: sheet.as_deref(),
                chart: chart.as_deref(),
                fill_color: &fill_color,
                expect_fill: expect_fill.as_deref(),
                expect_fill_present: value_flag_present(rest, "--expect-fill"),
                out: out.as_deref(),
                backup: backup.as_deref(),
                dry_run: has_flag(rest, "--dry-run"),
                no_validate: has_flag(rest, "--no-validate"),
                in_place: has_flag(rest, "--in-place"),
            };
            if verb == "set-chart-area-fill" {
                xlsx_charts_set_chart_area_fill(file, options)
            } else {
                xlsx_charts_set_plot_area_fill(file, options)
            }
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "charts" && verb == "set-series-style" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--chart",
                    "--series",
                    "--fill-color",
                    "--line-color",
                    "--line-width-pt",
                    "--marker-symbol",
                    "--marker-size",
                    "--expect-series-count",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let chart = parse_string_flag(rest, "--chart")?;
            let series = parse_i64_flag(rest, "--series")?.unwrap_or(1);
            let fill_color = parse_string_flag(rest, "--fill-color")?;
            let line_color = parse_string_flag(rest, "--line-color")?;
            let line_width_pt = parse_f64_flag(rest, "--line-width-pt")?;
            let marker_symbol = parse_string_flag(rest, "--marker-symbol")?;
            let marker_size = parse_i64_flag(rest, "--marker-size")?;
            let expect_series_count = parse_i64_flag(rest, "--expect-series-count")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_charts_set_series_style(
                file,
                XlsxChartSetSeriesStyleOptions {
                    sheet: sheet.as_deref(),
                    chart: chart.as_deref(),
                    series,
                    fill_color: fill_color.as_deref(),
                    line_color: line_color.as_deref(),
                    line_width_pt,
                    marker_symbol: marker_symbol.as_deref(),
                    marker_size,
                    expect_series_count,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "charts" && verb == "convert-type" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--chart",
                    "--to",
                    "--expect-type",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let chart = parse_string_flag(rest, "--chart")?;
            let to = parse_string_flag(rest, "--to")?;
            let expect_type = parse_string_flag(rest, "--expect-type")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_charts_convert_type(
                file,
                XlsxChartConvertTypeOptions {
                    sheet: sheet.as_deref(),
                    chart: chart.as_deref(),
                    to: to.as_deref(),
                    expect_type: expect_type.as_deref(),
                    expect_type_present: value_flag_present(rest, "--expect-type"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "charts" && verb == "copy-style" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--chart",
                    "--to-chart",
                    "--from",
                    "--from-chart",
                    "--expect-series-count",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let chart = parse_string_flag(rest, "--chart")?;
            let to_chart = parse_string_flag(rest, "--to-chart")?;
            let from = parse_string_flag(rest, "--from")?;
            let from_chart = parse_string_flag(rest, "--from-chart")?;
            let expect_series_count = parse_i64_flag(rest, "--expect-series-count")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_charts_copy_style(
                file,
                XlsxChartCopyStyleOptions {
                    sheet: sheet.as_deref(),
                    chart: chart.as_deref(),
                    to_chart: to_chart.as_deref(),
                    to_chart_present: value_flag_present(rest, "--to-chart"),
                    from: from.as_deref(),
                    from_chart: from_chart.as_deref(),
                    expect_series_count,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "charts" && verb == "set-axis" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--chart",
                    "--axis",
                    "--title",
                    "--expect-axis-title",
                    "--min",
                    "--max",
                    "--major-unit",
                    "--number-format",
                    "--tick-label-font-family",
                    "--tick-label-font-size",
                    "--tick-label-font-color",
                    "--title-font-family",
                    "--title-font-size",
                    "--title-font-color",
                    "--expect-axis-count",
                    "--out",
                    "--backup",
                ],
                &[
                    "--hidden",
                    "--major-gridlines",
                    "--minor-gridlines",
                    "--tick-label-font-bold",
                    "--tick-label-font-italic",
                    "--title-font-bold",
                    "--title-font-italic",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let chart = parse_string_flag(rest, "--chart")?;
            let axis = parse_string_flag(rest, "--axis")?;
            let title = parse_string_flag(rest, "--title")?;
            let expect_axis_title = parse_string_flag(rest, "--expect-axis-title")?;
            let min = parse_f64_flag(rest, "--min")?;
            let max = parse_f64_flag(rest, "--max")?;
            let major_unit = parse_f64_flag(rest, "--major-unit")?;
            let number_format = parse_string_flag(rest, "--number-format")?;
            let tick_label_font_family = parse_string_flag(rest, "--tick-label-font-family")?;
            let tick_label_font_size = parse_f64_flag(rest, "--tick-label-font-size")?;
            let tick_label_font_color = parse_string_flag(rest, "--tick-label-font-color")?;
            let title_font_family = parse_string_flag(rest, "--title-font-family")?;
            let title_font_size = parse_f64_flag(rest, "--title-font-size")?;
            let title_font_color = parse_string_flag(rest, "--title-font-color")?;
            let expect_axis_count = parse_i64_flag(rest, "--expect-axis-count")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_charts_set_axis(
                file,
                XlsxChartSetAxisOptions {
                    sheet: sheet.as_deref(),
                    chart: chart.as_deref(),
                    axis: axis.as_deref(),
                    title: title.as_deref(),
                    title_present: value_flag_present(rest, "--title"),
                    expect_axis_title: expect_axis_title.as_deref(),
                    expect_axis_title_present: value_flag_present(rest, "--expect-axis-title"),
                    hidden: parse_bool_flag(rest, "--hidden")?,
                    min,
                    max,
                    major_unit,
                    number_format: number_format.as_deref(),
                    major_gridlines: parse_bool_flag(rest, "--major-gridlines")?,
                    minor_gridlines: parse_bool_flag(rest, "--minor-gridlines")?,
                    tick_label_font_family: tick_label_font_family.as_deref(),
                    tick_label_font_size,
                    tick_label_font_color: tick_label_font_color.as_deref(),
                    tick_label_font_bold: parse_bool_flag(rest, "--tick-label-font-bold")?,
                    tick_label_font_italic: parse_bool_flag(rest, "--tick-label-font-italic")?,
                    title_font_family: title_font_family.as_deref(),
                    title_font_size,
                    title_font_color: title_font_color.as_deref(),
                    title_font_bold: parse_bool_flag(rest, "--title-font-bold")?,
                    title_font_italic: parse_bool_flag(rest, "--title-font-italic")?,
                    expect_axis_count,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}

fn parse_f64_flag(args: &[String], name: &str) -> CliResult<Option<f64>> {
    parse_string_flag(args, name)?
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| CliError::invalid_args(format!("{name} must be a number")))
        })
        .transpose()
}
