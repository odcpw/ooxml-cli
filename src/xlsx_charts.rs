use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, WorkbookSheet, add_selector, command_arg, copy_zip_with_part_override,
    decode_xml_text, local_name, relationship_entries, relationships_part_for,
    resolve_relationship_target, resolve_sheet, selector_candidates, validate,
    validate_xlsx_mutation_output_flags, workbook_sheets, xlsx_ranges_set_temp_path,
    xlsx_sheet_selectors, xml_attr_escape, xml_escape, zip_text,
};

const REL_WORKSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
const REL_DRAWING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing";
const REL_CHART: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const NS_CHART: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const NS_DRAWING_MAIN: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

const CHART_CHILD_ORDER: &[&str] = &[
    "title",
    "autoTitleDeleted",
    "pivotFmts",
    "view3D",
    "floor",
    "sideWall",
    "backWall",
    "plotArea",
    "legend",
    "plotVisOnly",
    "dispBlanksAs",
    "showDLblsOverMax",
    "extLst",
];
const TITLE_CHILD_ORDER: &[&str] = &["tx", "layout", "overlay", "spPr", "txPr", "extLst"];
const LEGEND_CHILD_ORDER: &[&str] = &[
    "legendPos",
    "legendEntry",
    "layout",
    "overlay",
    "spPr",
    "txPr",
    "extLst",
];
const SERIES_CHILD_ORDER: &[&str] = &[
    "idx",
    "order",
    "tx",
    "spPr",
    "invertIfNegative",
    "pictureOptions",
    "explosion",
    "marker",
    "dPt",
    "dLbls",
    "trendline",
    "errBars",
    "cat",
    "cPt",
    "val",
    "xVal",
    "yVal",
    "bubbleSize",
    "bubble3D",
    "shape",
    "smooth",
    "extLst",
];
const SHAPE_PROPS_CHILD_ORDER: &[&str] = &[
    "xfrm",
    "custGeom",
    "prstGeom",
    "noFill",
    "solidFill",
    "gradFill",
    "blipFill",
    "pattFill",
    "grpFill",
    "ln",
    "effectLst",
    "effectDag",
    "scene3d",
    "sp3d",
    "extLst",
];
const LINE_CHILD_ORDER: &[&str] = &[
    "noFill",
    "solidFill",
    "gradFill",
    "pattFill",
    "prstDash",
    "custDash",
    "round",
    "bevel",
    "miter",
    "headEnd",
    "tailEnd",
    "extLst",
];
const MARKER_CHILD_ORDER: &[&str] = &["symbol", "size", "spPr", "extLst"];
const PARAGRAPH_CHILD_ORDER: &[&str] = &["pPr", "r", "br", "fld", "endParaRPr"];
const RUN_CHILD_ORDER: &[&str] = &["rPr", "t"];
const RPR_CHILD_ORDER: &[&str] = &[
    "ln",
    "noFill",
    "solidFill",
    "gradFill",
    "blipFill",
    "pattFill",
    "grpFill",
    "effectLst",
    "effectDag",
    "highlight",
    "uLnTx",
    "uLn",
    "uFillTx",
    "uFill",
    "latin",
    "ea",
    "cs",
    "sym",
    "hlinkClick",
    "hlinkMouseOver",
    "rtl",
    "extLst",
];
const PLOT_AREA_CHILD_ORDER: &[&str] = &["spPr", "extLst"];
const CHART_SPACE_CHILD_ORDER: &[&str] = &[
    "spPr",
    "txPr",
    "externalData",
    "printSettings",
    "userShapes",
    "extLst",
];
const SCALING_CHILD_ORDER: &[&str] = &["logBase", "orientation", "max", "min", "extLst"];
const CAT_AXIS_CHILD_ORDER: &[&str] = &[
    "axId",
    "scaling",
    "delete",
    "axPos",
    "majorGridlines",
    "minorGridlines",
    "title",
    "numFmt",
    "majorTickMark",
    "minorTickMark",
    "tickLblPos",
    "spPr",
    "txPr",
    "crossAx",
    "crosses",
    "crossesAt",
    "auto",
    "lblAlgn",
    "lblOffset",
    "tickLblSkip",
    "tickMarkSkip",
    "noMultiLvlLbl",
    "extLst",
];
const VAL_AXIS_CHILD_ORDER: &[&str] = &[
    "axId",
    "scaling",
    "delete",
    "axPos",
    "majorGridlines",
    "minorGridlines",
    "title",
    "numFmt",
    "majorTickMark",
    "minorTickMark",
    "tickLblPos",
    "spPr",
    "txPr",
    "crossAx",
    "crosses",
    "crossesAt",
    "crossBetween",
    "majorUnit",
    "minorUnit",
    "dispUnits",
    "extLst",
];
const TX_PR_CHILD_ORDER: &[&str] = &["bodyPr", "lstStyle", "p"];

#[derive(Clone)]
struct ChartRef {
    number: u32,
    sheet: String,
    sheet_number: u32,
    sheet_part_uri: String,
    drawing_relationship_id: String,
    drawing_part_uri: String,
    relationship_id: String,
    part_uri: String,
    name: String,
    title: String,
    types: Vec<String>,
    anchor: Option<ChartAnchor>,
    primary_selector: String,
    selectors: Vec<String>,
    series: Vec<ChartSeries>,
    style: Option<Value>,
}

#[derive(Clone)]
struct ChartMarker {
    column: i64,
    column_offset: i64,
    row: i64,
    row_offset: i64,
}

#[derive(Clone)]
struct ChartAnchor {
    kind: String,
    from: Option<ChartMarker>,
    to: Option<ChartMarker>,
}

#[derive(Clone)]
struct ChartDataSource {
    formula: String,
    sheet: String,
    range: String,
    ref_kind: String,
    cache_type: String,
    point_count: i64,
    cache_preview: Vec<String>,
}

#[derive(Clone)]
struct ChartSeries {
    number: u32,
    index: i64,
    order: i64,
    name: Option<ChartDataSource>,
    categories: Option<ChartDataSource>,
    values: Option<ChartDataSource>,
    x_values: Option<ChartDataSource>,
    y_values: Option<ChartDataSource>,
    bubble_size: Option<ChartDataSource>,
}

#[derive(Clone, Debug)]
struct XmlAttr {
    qname: String,
    local: String,
    value: String,
}

#[derive(Clone, Debug)]
struct XmlNode {
    qname: String,
    name: String,
    attrs: BTreeMap<String, String>,
    raw_attrs: Vec<XmlAttr>,
    text: String,
    children: Vec<XmlNode>,
}

#[derive(Clone)]
pub(crate) struct XlsxChartSetTitleOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) chart: Option<&'a str>,
    pub(crate) title: &'a str,
    pub(crate) expect_title: Option<&'a str>,
    pub(crate) expect_title_present: bool,
    pub(crate) font_family: Option<&'a str>,
    pub(crate) font_size_pt: Option<f64>,
    pub(crate) font_color: Option<&'a str>,
    pub(crate) font_bold: Option<bool>,
    pub(crate) font_italic: Option<bool>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
pub(crate) struct XlsxChartSetLegendOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) chart: Option<&'a str>,
    pub(crate) position: Option<&'a str>,
    pub(crate) position_present: bool,
    pub(crate) overlay: Option<bool>,
    pub(crate) expect_position: Option<&'a str>,
    pub(crate) expect_position_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
pub(crate) enum XlsxChartFillTarget {
    ChartArea,
    PlotArea,
}

#[derive(Clone)]
pub(crate) struct XlsxChartSetFillOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) chart: Option<&'a str>,
    pub(crate) fill_color: &'a str,
    pub(crate) expect_fill: Option<&'a str>,
    pub(crate) expect_fill_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
pub(crate) struct XlsxChartSetSeriesStyleOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) chart: Option<&'a str>,
    pub(crate) series: i64,
    pub(crate) fill_color: Option<&'a str>,
    pub(crate) line_color: Option<&'a str>,
    pub(crate) line_width_pt: Option<f64>,
    pub(crate) marker_symbol: Option<&'a str>,
    pub(crate) marker_size: Option<i64>,
    pub(crate) expect_series_count: Option<i64>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
pub(crate) struct XlsxChartConvertTypeOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) chart: Option<&'a str>,
    pub(crate) to: Option<&'a str>,
    pub(crate) expect_type: Option<&'a str>,
    pub(crate) expect_type_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
pub(crate) struct XlsxChartCopyStyleOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) chart: Option<&'a str>,
    pub(crate) to_chart: Option<&'a str>,
    pub(crate) to_chart_present: bool,
    pub(crate) from: Option<&'a str>,
    pub(crate) from_chart: Option<&'a str>,
    pub(crate) expect_series_count: Option<i64>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
pub(crate) struct XlsxChartSetAxisOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) chart: Option<&'a str>,
    pub(crate) axis: Option<&'a str>,
    pub(crate) title: Option<&'a str>,
    pub(crate) title_present: bool,
    pub(crate) expect_axis_title: Option<&'a str>,
    pub(crate) expect_axis_title_present: bool,
    pub(crate) hidden: Option<bool>,
    pub(crate) min: Option<f64>,
    pub(crate) max: Option<f64>,
    pub(crate) major_unit: Option<f64>,
    pub(crate) number_format: Option<&'a str>,
    pub(crate) major_gridlines: Option<bool>,
    pub(crate) minor_gridlines: Option<bool>,
    pub(crate) tick_label_font_family: Option<&'a str>,
    pub(crate) tick_label_font_size: Option<f64>,
    pub(crate) tick_label_font_color: Option<&'a str>,
    pub(crate) tick_label_font_bold: Option<bool>,
    pub(crate) tick_label_font_italic: Option<bool>,
    pub(crate) title_font_family: Option<&'a str>,
    pub(crate) title_font_size: Option<f64>,
    pub(crate) title_font_color: Option<&'a str>,
    pub(crate) title_font_bold: Option<bool>,
    pub(crate) title_font_italic: Option<bool>,
    pub(crate) expect_axis_count: Option<i64>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

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

fn xlsx_charts_set_fill(
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

fn xlsx_charts_result(file: &str, charts: Vec<ChartRef>) -> Value {
    json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "charts": charts.iter().map(|chart| xlsx_chart_item(file, chart)).collect::<Vec<_>>(),
    })
}

#[derive(Clone)]
struct XlsxChartOutputOptions<'a> {
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
    in_place: bool,
}

impl<'a> XlsxChartOutputOptions<'a> {
    fn from_title(options: &'a XlsxChartSetTitleOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    fn from_legend(options: &'a XlsxChartSetLegendOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    fn from_fill(options: &'a XlsxChartSetFillOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    fn from_series(options: &'a XlsxChartSetSeriesStyleOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    fn from_convert(options: &'a XlsxChartConvertTypeOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    fn from_copy_style(options: &'a XlsxChartCopyStyleOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    fn from_axis(options: &'a XlsxChartSetAxisOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }
}

#[derive(Default)]
struct ChartMutationExtra {
    previous_title: Option<String>,
    legend_removed: bool,
    series: Option<i64>,
    previous_type: Option<String>,
    new_type: Option<String>,
    previous_fill: Option<String>,
    new_fill: Option<String>,
    applied_style: Vec<String>,
    warnings: Vec<String>,
}

struct ChartStyleResultArgs<'a> {
    file: &'a str,
    output: Option<&'a str>,
    dry_run: bool,
    action: &'a str,
    chart_item: Value,
    sheet_selector: Option<&'a str>,
    chart: &'a ChartRef,
    extra: ChartMutationExtra,
}

#[derive(Clone)]
struct ChartXmlContext {
    chart_prefix: String,
    drawing_prefix: String,
}

#[derive(Clone)]
struct ChartFontOptions {
    family: Option<String>,
    size_pt: Option<f64>,
    color: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
}

impl ChartFontOptions {
    fn is_empty(&self) -> bool {
        self.family.is_none()
            && self.size_pt.is_none()
            && self.color.is_none()
            && self.bold.is_none()
            && self.italic.is_none()
    }
}

#[derive(Clone)]
struct LegendPosition {
    code: String,
    remove: bool,
}

#[derive(Clone)]
struct ChartFillOptions {
    color: String,
    no_fill: bool,
}

#[derive(Clone)]
struct ChartSeriesStyleOptions {
    fill_color: Option<String>,
    line_color: Option<String>,
    line_width_pt: Option<f64>,
    marker_symbol: Option<String>,
    marker_size: Option<i64>,
}

impl ChartSeriesStyleOptions {
    fn is_empty(&self) -> bool {
        self.fill_color.is_none()
            && self.line_color.is_none()
            && self.line_width_pt.is_none()
            && self.marker_symbol.is_none()
            && self.marker_size.is_none()
    }
}

struct ChartTypeConversion {
    previous_type: String,
    new_type: String,
    warnings: Vec<String>,
}

struct ChartAxisFlags {
    set_title: bool,
    title: String,
    set_hidden: bool,
    hidden: bool,
    min: Option<f64>,
    max: Option<f64>,
    major_unit: Option<f64>,
    number_format: Option<String>,
    set_major_gridlines: bool,
    major_gridlines: bool,
    set_minor_gridlines: bool,
    minor_gridlines: bool,
    tick_label_font: ChartFontOptions,
    title_font: ChartFontOptions,
}

fn run_xlsx_chart_style_mutation<F>(
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

fn xlsx_chart_style_result(args: ChartStyleResultArgs<'_>) -> Value {
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

fn load_xlsx_charts(file: &str, sheet_selector: Option<&str>) -> CliResult<Vec<ChartRef>> {
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

fn resolve_sheet_for_chart_cli(
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

fn list_charts_for_sheet(
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

fn list_charts_for_drawing(
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

fn worksheet_drawing_relationship_ids(xml: &str, sheet_part_uri: &str) -> CliResult<Vec<String>> {
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

fn read_chart_part(root: &XmlNode, chart_uri: &str) -> CliResult<ChartRef> {
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

fn select_xlsx_chart(charts: &[ChartRef], selector: &str) -> CliResult<ChartRef> {
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

fn add_chart_selectors(chart: &mut ChartRef) {
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

fn xlsx_chart_item(file: &str, chart: &ChartRef) -> Value {
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

fn xlsx_chart_item_for_update(file: Option<&str>, chart: &ChartRef) -> Value {
    let mut item = xlsx_chart_item(file.unwrap_or_default(), chart);
    if file.is_none()
        && let Some(object) = item.as_object_mut()
    {
        object.remove("showCommand");
        object.remove("sourceExportCommands");
    }
    item
}

fn xlsx_chart_source_export_commands(file: &str, chart: &ChartRef) -> Vec<Value> {
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

fn chart_series_sources(series: &ChartSeries) -> Vec<(&'static str, Option<&ChartDataSource>)> {
    vec![
        ("name", series.name.as_ref()),
        ("categories", series.categories.as_ref()),
        ("values", series.values.as_ref()),
        ("xValues", series.x_values.as_ref()),
        ("yValues", series.y_values.as_ref()),
        ("bubbleSize", series.bubble_size.as_ref()),
    ]
}

fn xlsx_chart_show_command(file: &str, chart: &ChartRef) -> String {
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

fn xlsx_ranges_export_command(file: &str, sheet: &str, range: &str) -> String {
    format!(
        "ooxml --json xlsx ranges export {} --sheet {} --range {} --include-types",
        command_arg(file),
        command_arg(sheet),
        command_arg(range)
    )
}

fn anchor_json(anchor: &ChartAnchor) -> Value {
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

fn marker_json(marker: &ChartMarker) -> Value {
    let mut object = Map::new();
    object.insert("column".to_string(), json!(marker.column));
    insert_nonzero_i64(&mut object, "columnOffset", marker.column_offset);
    object.insert("row".to_string(), json!(marker.row));
    insert_nonzero_i64(&mut object, "rowOffset", marker.row_offset);
    Value::Object(object)
}

fn series_json(series: &ChartSeries) -> Value {
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

fn data_source_json(source: &ChartDataSource) -> Value {
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

fn chart_name(anchor: &XmlNode) -> String {
    let Some(frame) = first_descendant(anchor, "graphicFrame") else {
        return String::new();
    };
    descendants(frame, "cNvPr")
        .into_iter()
        .find_map(|node| node.attr("name").map(str::trim).filter(|v| !v.is_empty()))
        .unwrap_or_default()
        .to_string()
}

fn parse_anchor(anchor: &XmlNode) -> ChartAnchor {
    ChartAnchor {
        kind: anchor.name.clone(),
        from: direct_child(anchor, "from").map(parse_marker),
        to: direct_child(anchor, "to").map(parse_marker),
    }
}

fn parse_marker(marker: &XmlNode) -> ChartMarker {
    ChartMarker {
        column: parse_child_i64(marker, "col"),
        column_offset: parse_child_i64(marker, "colOff"),
        row: parse_child_i64(marker, "row"),
        row_offset: parse_child_i64(marker, "rowOff"),
    }
}

fn chart_title(root: &XmlNode) -> String {
    first_descendant(root, "title")
        .map(title_text)
        .unwrap_or_default()
}

fn chart_types(root: &XmlNode) -> Vec<String> {
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

fn chart_series(root: &XmlNode) -> Vec<ChartSeries> {
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

fn walk_series(root: &XmlNode) -> Vec<&XmlNode> {
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

fn chart_data_source(elem: Option<&XmlNode>) -> Option<ChartDataSource> {
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

fn first_cache_child(elem: &XmlNode) -> Option<&XmlNode> {
    elem.children.iter().find(|child| {
        matches!(
            child.name.as_str(),
            "strCache" | "numCache" | "multiLvlStrCache"
        )
    })
}

fn inspect_chart_style(root: &XmlNode, chart_uri: &str) -> Value {
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

fn inspect_title(title: &XmlNode) -> Value {
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

fn title_text(title: &XmlNode) -> String {
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

fn inspect_title_font(title: &XmlNode) -> Option<Value> {
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

fn inspect_axis_tick_label_font(axis: &XmlNode) -> Option<Value> {
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

fn inspect_font(r_pr: &XmlNode) -> Option<Value> {
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

fn inspect_legend(legend: &XmlNode) -> Value {
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

fn inspect_axes(plot_area: &XmlNode) -> Vec<Value> {
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

fn inspect_series_style(series: &XmlNode, number: usize) -> Value {
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

fn inspect_marker(marker: &XmlNode) -> Option<Value> {
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

fn inspect_fill(holder: &XmlNode) -> String {
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

fn normalize_optional_nonempty(value: Option<&str>, flag: &str) -> CliResult<Option<String>> {
    match value {
        Some(value) if value.trim().is_empty() => {
            Err(CliError::invalid_args(format!("{flag} must not be empty")))
        }
        Some(value) => Ok(Some(value.trim().to_string())),
        None => Ok(None),
    }
}

fn normalize_optional_hex(value: Option<&str>) -> CliResult<Option<String>> {
    value.map(normalize_hex_color).transpose()
}

fn normalize_hex_color(value: &str) -> CliResult<String> {
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

fn parse_chart_legend_position(value: &str) -> CliResult<LegendPosition> {
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

fn parse_chart_expect_legend_position(value: &str) -> CliResult<String> {
    let parsed = parse_chart_legend_position(value).map_err(|_| {
        CliError::invalid_args("--expect-position must be right, left, top, bottom, or none")
    })?;
    Ok(if parsed.remove {
        String::new()
    } else {
        parsed.code
    })
}

fn parse_chart_fill_color(value: &str) -> CliResult<ChartFillOptions> {
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

fn resolve_chart_expect_fill(value: &str) -> CliResult<String> {
    if value.trim().is_empty() || value.trim().eq_ignore_ascii_case("none") {
        return Ok(String::new());
    }
    if value.trim().to_ascii_lowercase().starts_with("scheme:") {
        return Ok(value.trim().to_string());
    }
    normalize_hex_color(value)
}

fn parse_chart_marker_symbol(value: &str) -> CliResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "circle" | "square" | "diamond" | "triangle" | "none" => Ok(normalized),
        _ => Err(CliError::invalid_args(
            "--marker-symbol must be circle, square, diamond, triangle, or none",
        )),
    }
}

fn parse_chart_type(value: &str, flag: &str) -> CliResult<String> {
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

fn parse_chart_axis_kind(value: &str) -> CliResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "category" | "value" => Ok(normalized),
        _ => Err(CliError::invalid_args(
            "--axis is required; use category or value",
        )),
    }
}

fn resolve_chart_axis_flags(options: &XlsxChartSetAxisOptions<'_>) -> CliResult<ChartAxisFlags> {
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

fn apply_chart_set_title(
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

fn replace_title_text_tree(
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

fn apply_font_to_rpr(r_pr: &mut XmlNode, ctx: &ChartXmlContext, font: &ChartFontOptions) {
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

fn apply_chart_set_legend(
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

fn apply_chart_set_fill(
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

fn apply_chart_set_series_style(
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

fn apply_chart_convert_type(
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

fn apply_chart_set_axis(
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

fn apply_chart_copy_style(
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

fn read_xlsx_template_chart_style(file: &str, chart_selector: Option<&str>) -> CliResult<Value> {
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

fn canonical_chart_type(plot: &XmlNode) -> CliResult<String> {
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

fn element_for_chart_type(chart_type: &str) -> &'static str {
    match chart_type {
        "bar" | "column" => "barChart",
        "line" => "lineChart",
        "area" => "areaChart",
        "pie" => "pieChart",
        "scatter" => "scatterChart",
        _ => "",
    }
}

fn set_bar_dir(ctx: &ChartXmlContext, plot: &mut XmlNode, chart_type: &str) {
    let direction = if chart_type == "bar" { "bar" } else { "col" };
    if let Some(index) = child_index(plot, "barDir") {
        plot.children[index].set_attr("val", direction);
    } else {
        let mut child = XmlNode::new(ctx.c("barDir"));
        child.set_attr("val", direction);
        plot.children.insert(0, child);
    }
}

fn plot_axis_ids(plot: &XmlNode) -> Vec<String> {
    plot.children
        .iter()
        .filter(|child| child.name == "axId")
        .filter_map(|child| child.attr("val"))
        .map(|value| value.trim().to_string())
        .collect()
}

fn transform_series_for_chart_type(
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

fn build_plot_wrapper(
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

fn append_axis_ids(plot: &mut XmlNode, ctx: &ChartXmlContext, axis_ids: &[String], want: usize) {
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

fn transform_axes_for_chart_type(
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

fn axis_by_id_or_fallback_index(plot_area: &XmlNode, id: &str, fallback: &str) -> Option<usize> {
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

fn prune_axis_children(axis: &mut XmlNode) {
    let allowed = axis_child_order(&axis.name);
    axis.children
        .retain(|child| allowed.iter().any(|name| *name == child.name));
}

fn push_val_child(parent: &mut XmlNode, ctx: &ChartXmlContext, name: &str, value: &str) {
    let mut child = XmlNode::new(ctx.c(name));
    child.set_attr("val", value);
    parent.children.push(child);
}

fn rename_direct_child(parent: &mut XmlNode, old: &str, new: &str) {
    if let Some(index) = child_index(parent, old) {
        rename_node_local(&mut parent.children[index], new);
    }
}

fn rename_node_local(node: &mut XmlNode, new: &str) {
    node.qname = prefix_from_qname(&node.qname)
        .map(|prefix| prefixed_qname(prefix, new))
        .unwrap_or_else(|| new.to_string());
    node.name = new.to_string();
}

fn reorder_children_in_order(node: &mut XmlNode, order: &[&str]) {
    let children = std::mem::take(&mut node.children);
    for child in children {
        insert_child_in_order(node, child, order);
    }
}

fn is_axis_element(name: &str) -> bool {
    matches!(name, "catAx" | "valAx" | "dateAx" | "serAx")
}

fn axis_child_order(element: &str) -> &'static [&'static str] {
    if element == "valAx" {
        VAL_AXIS_CHILD_ORDER
    } else {
        CAT_AXIS_CHILD_ORDER
    }
}

fn chart_type_supports_markers(chart_type: &str) -> bool {
    matches!(
        element_for_chart_type(chart_type),
        "lineChart" | "scatterChart"
    )
}

fn format_float(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn apply_gridlines(
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

fn apply_axis_tick_label_font(
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

fn apply_run_font(run: &mut XmlNode, ctx: &ChartXmlContext, font: &ChartFontOptions) {
    if font.is_empty() {
        return;
    }
    let r_pr_index = ensure_child_index(run, "rPr", ctx.a("rPr"), RUN_CHILD_ORDER);
    apply_font_to_rpr(&mut run.children[r_pr_index], ctx, font);
}

fn style_font_from_value(value: &Value) -> Option<ChartFontOptions> {
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

fn find_target_axis_index(plot_area: &XmlNode, source_axis: &Value) -> Option<usize> {
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

fn apply_series_style_from_source(
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

fn series_node_paths_in_plot_area(plot_area: &XmlNode) -> Vec<(usize, usize)> {
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

fn apply_color_fill(holder: &mut XmlNode, ctx: &ChartXmlContext, color: &str, order: &[&str]) {
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

fn set_solid_fill(holder: &mut XmlNode, ctx: &ChartXmlContext, color: &str, order: &[&str]) {
    remove_fill_group_children(holder);
    let mut solid = XmlNode::new(ctx.a("solidFill"));
    let mut srgb = XmlNode::new(ctx.a("srgbClr"));
    srgb.set_attr("val", color);
    solid.children.push(srgb);
    insert_child_in_order(holder, solid, order);
}

fn remove_fill_group_children(holder: &mut XmlNode) {
    holder.children.retain(|child| {
        !matches!(
            child.name.as_str(),
            "noFill" | "solidFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill"
        )
    });
}

fn fill_matches(current: &str, expected: &str) -> bool {
    if expected.trim().is_empty() || expected.trim().eq_ignore_ascii_case("none") {
        current.is_empty()
    } else {
        current.eq_ignore_ascii_case(expected.trim())
    }
}

fn bool_attr(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

fn series_name_text(tx: &XmlNode) -> String {
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

fn split_sheet_range_formula(formula: &str) -> (String, String) {
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

#[derive(Clone, Copy, PartialEq, Eq)]
struct FormulaCellRef {
    column: u32,
    row: u32,
    abs_column: bool,
    abs_row: bool,
}

fn normalize_formula_range(range: &str) -> Option<String> {
    let parts = range.trim().split(':').collect::<Vec<_>>();
    if parts.len() > 2 || parts.first()?.trim().is_empty() {
        return None;
    }
    let start = parse_formula_cell(parts[0])?;
    let end = if let Some(end) = parts.get(1) {
        if end.trim().is_empty() {
            return None;
        }
        parse_formula_cell(end)?
    } else {
        start
    };
    if start == end {
        Some(format_formula_cell(start))
    } else {
        Some(format!(
            "{}:{}",
            format_formula_cell(start),
            format_formula_cell(end)
        ))
    }
}

fn parse_formula_cell(value: &str) -> Option<FormulaCellRef> {
    let mut rest = value.trim();
    if rest.is_empty() {
        return None;
    }
    let abs_column = rest.starts_with('$');
    if abs_column {
        rest = &rest[1..];
    }
    let col_len = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    if col_len == 0 {
        return None;
    }
    let column = column_letters_to_index(&rest[..col_len])?;
    rest = &rest[col_len..];
    let abs_row = rest.starts_with('$');
    if abs_row {
        rest = &rest[1..];
    }
    if rest.is_empty() || rest.contains('$') || !rest.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let row = rest.parse::<u32>().ok()?;
    if row == 0 || row > 1_048_576 {
        return None;
    }
    Some(FormulaCellRef {
        column,
        row,
        abs_column,
        abs_row,
    })
}

fn column_letters_to_index(value: &str) -> Option<u32> {
    let mut index = 0_u32;
    for ch in value.chars() {
        if !ch.is_ascii_alphabetic() {
            return None;
        }
        index = index * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        if index > 16_384 {
            return None;
        }
    }
    Some(index)
}

fn format_formula_cell(cell: FormulaCellRef) -> String {
    let mut out = String::new();
    if cell.abs_column {
        out.push('$');
    }
    out.push_str(&column_index_to_letters(cell.column));
    if cell.abs_row {
        out.push('$');
    }
    out.push_str(&cell.row.to_string());
    out
}

fn column_index_to_letters(mut index: u32) -> String {
    let mut chars = Vec::new();
    while index > 0 {
        index -= 1;
        chars.push((b'A' + (index % 26) as u8) as char);
        index /= 26;
    }
    chars.iter().rev().collect()
}

fn resolve_workbook_target_uri(target: &str) -> String {
    let trimmed = target.trim_start_matches('/');
    if target.starts_with('/') || trimmed.starts_with("xl/") {
        format!("/{trimmed}")
    } else {
        resolve_relationship_target("/xl/workbook.xml", target)
    }
}

fn ensure_chart_xml_namespaces(root: &mut XmlNode) -> ChartXmlContext {
    let chart_prefix = root
        .namespace_prefix_for(NS_CHART)
        .unwrap_or_else(|| prefix_from_qname(&root.qname).unwrap_or("c").to_string());
    let drawing_prefix = root
        .namespace_prefix_for(NS_DRAWING_MAIN)
        .unwrap_or_else(|| "a".to_string());
    if !chart_prefix.is_empty() {
        root.ensure_namespace(&chart_prefix, NS_CHART);
    }
    if !drawing_prefix.is_empty() {
        root.ensure_namespace(&drawing_prefix, NS_DRAWING_MAIN);
    }
    ChartXmlContext {
        chart_prefix,
        drawing_prefix,
    }
}

impl ChartXmlContext {
    fn c(&self, local: &str) -> String {
        prefixed_qname(&self.chart_prefix, local)
    }

    fn a(&self, local: &str) -> String {
        prefixed_qname(&self.drawing_prefix, local)
    }
}

fn prefixed_qname(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn prefix_from_qname(qname: &str) -> Option<&str> {
    qname.split_once(':').map(|(prefix, _)| prefix)
}

fn child_index(node: &XmlNode, name: &str) -> Option<usize> {
    node.children.iter().position(|child| child.name == name)
}

fn ensure_child_index(parent: &mut XmlNode, name: &str, qname: String, order: &[&str]) -> usize {
    if let Some(index) = child_index(parent, name) {
        return index;
    }
    insert_child_in_order(parent, XmlNode::new(qname), order);
    child_index(parent, name).expect("inserted child")
}

fn set_or_create_val_child(
    parent: &mut XmlNode,
    ctx: &ChartXmlContext,
    name: &str,
    value: &str,
    order: &[&str],
) {
    if let Some(index) = child_index(parent, name) {
        parent.children[index].set_attr("val", value);
        return;
    }
    let mut child = XmlNode::new(ctx.c(name));
    child.set_attr("val", value);
    insert_child_in_order(parent, child, order);
}

fn insert_child_in_order(parent: &mut XmlNode, child: XmlNode, order: &[&str]) {
    if let Some(child_rank) = order.iter().position(|name| *name == child.name)
        && let Some(index) = parent.children.iter().position(|existing| {
            order
                .iter()
                .position(|name| *name == existing.name)
                .is_some_and(|rank| rank > child_rank)
        })
    {
        parent.children.insert(index, child);
        return;
    }
    parent.children.push(child);
}

fn first_descendant_mut<'a>(node: &'a mut XmlNode, name: &str) -> Option<&'a mut XmlNode> {
    if node.name == name {
        return Some(node);
    }
    for child in &mut node.children {
        if let Some(found) = first_descendant_mut(child, name) {
            return Some(found);
        }
    }
    None
}

fn series_node_paths(root: &XmlNode) -> Vec<(usize, usize)> {
    let Some(plot_area) = first_descendant(root, "plotArea") else {
        return Vec::new();
    };
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

fn render_xml_document(root: &XmlNode) -> String {
    let mut out = String::new();
    render_xml_node(root, &mut out);
    out
}

fn render_xml_node(node: &XmlNode, out: &mut String) {
    out.push('<');
    out.push_str(&node.qname);
    for attr in &node.raw_attrs {
        out.push(' ');
        out.push_str(&attr.qname);
        out.push_str("=\"");
        out.push_str(&xml_attr_escape(&attr.value));
        out.push('"');
    }
    if node.text.is_empty() && node.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    if !node.text.is_empty() {
        out.push_str(&xml_escape(&node.text));
    }
    for child in &node.children {
        render_xml_node(child, out);
    }
    out.push_str("</");
    out.push_str(&node.qname);
    out.push('>');
}

fn parse_xml_node(xml: &str) -> CliResult<XmlNode> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<XmlNode>::new();
    let mut root: Option<XmlNode> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => stack.push(XmlNode::from_start(&e)),
            Ok(Event::Empty(e)) => attach_xml_node(XmlNode::from_start(&e), &mut stack, &mut root)?,
            Ok(Event::Text(e)) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&crate::xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::End(_)) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| CliError::unexpected("malformed XML"))?;
                attach_xml_node(node, &mut stack, &mut root)?;
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(CliError::unexpected("unexpected EOF"));
    }
    root.ok_or_else(|| CliError::unexpected("XML part has no root element"))
}

fn attach_xml_node(
    node: XmlNode,
    stack: &mut [XmlNode],
    root: &mut Option<XmlNode>,
) -> CliResult<()> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
        Ok(())
    } else if root.is_none() {
        *root = Some(node);
        Ok(())
    } else {
        Err(CliError::unexpected("XML part has multiple root elements"))
    }
}

impl XmlNode {
    fn new(qname: String) -> Self {
        Self {
            name: local_name(qname.as_bytes()).to_string(),
            qname,
            attrs: BTreeMap::new(),
            raw_attrs: Vec::new(),
            text: String::new(),
            children: Vec::new(),
        }
    }

    fn from_start(e: &BytesStart<'_>) -> Self {
        let qname = String::from_utf8_lossy(e.name().as_ref()).to_string();
        let mut attrs = BTreeMap::new();
        let mut raw_attrs = Vec::new();
        for attr in e.attributes().with_checks(false).flatten() {
            let attr_qname = String::from_utf8_lossy(attr.key.as_ref()).to_string();
            let local = local_name(attr.key.as_ref()).to_string();
            let value = decode_xml_text(attr.value.as_ref());
            attrs.insert(local.clone(), value.clone());
            raw_attrs.push(XmlAttr {
                qname: attr_qname,
                local,
                value,
            });
        }
        Self {
            qname,
            name: local_name(e.name().as_ref()).to_string(),
            attrs,
            raw_attrs,
            text: String::new(),
            children: Vec::new(),
        }
    }

    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }

    fn set_attr(&mut self, name: &str, value: &str) {
        let local = local_name(name.as_bytes()).to_string();
        if let Some(attr) = self
            .raw_attrs
            .iter_mut()
            .find(|attr| attr.qname == name || attr.local == local)
        {
            attr.value = value.to_string();
        } else {
            self.raw_attrs.push(XmlAttr {
                qname: name.to_string(),
                local: local.clone(),
                value: value.to_string(),
            });
        }
        self.attrs.insert(local, value.to_string());
    }

    fn ensure_namespace(&mut self, prefix: &str, uri: &str) {
        if prefix.is_empty()
            || self.raw_attrs.iter().any(|attr| {
                attr.qname.starts_with("xmlns:")
                    && attr.qname.trim_start_matches("xmlns:") == prefix
                    && attr.value == uri
            })
        {
            return;
        }
        self.set_attr(&format!("xmlns:{prefix}"), uri);
    }

    fn namespace_prefix_for(&self, uri: &str) -> Option<String> {
        self.raw_attrs.iter().find_map(|attr| {
            if attr.value != uri {
                return None;
            }
            if attr.qname == "xmlns" {
                Some(String::new())
            } else {
                attr.qname
                    .strip_prefix("xmlns:")
                    .map(|prefix| prefix.to_string())
            }
        })
    }
}

fn direct_child<'a>(node: &'a XmlNode, name: &str) -> Option<&'a XmlNode> {
    node.children.iter().find(|child| child.name == name)
}

fn first_descendant<'a>(node: &'a XmlNode, name: &str) -> Option<&'a XmlNode> {
    if node.name == name {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| first_descendant(child, name))
}

fn descendants<'a>(node: &'a XmlNode, name: &str) -> Vec<&'a XmlNode> {
    let mut out = Vec::new();
    collect_descendants(node, name, &mut out);
    out
}

fn collect_descendants<'a>(node: &'a XmlNode, name: &str, out: &mut Vec<&'a XmlNode>) {
    if node.name == name {
        out.push(node);
    }
    for child in &node.children {
        collect_descendants(child, name, out);
    }
}

fn parse_child_i64(node: &XmlNode, name: &str) -> i64 {
    direct_child(node, name)
        .map(node_text_trimmed)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
}

fn attr_val_i64(node: &XmlNode) -> Option<i64> {
    node.attr("val")?.trim().parse::<i64>().ok()
}

fn attr_val_f64(node: &XmlNode) -> Option<f64> {
    node.attr("val")?.trim().parse::<f64>().ok()
}

fn node_text(node: &XmlNode) -> String {
    node.text.clone()
}

fn node_text_trimmed(node: &XmlNode) -> String {
    node.text.trim().to_string()
}

fn parse_ooxml_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on"
    )
}

fn axis_kind(element: &str) -> &'static str {
    match element {
        "valAx" => "value",
        "dateAx" => "date",
        "serAx" => "series",
        _ => "category",
    }
}

fn insert_nonempty_string(object: &mut Map<String, Value>, key: &str, value: &str) {
    if !value.is_empty() {
        object.insert(key.to_string(), json!(value));
    }
}

fn insert_nonempty_string_value(object: &mut Map<String, Value>, key: &str, value: String) {
    if !value.is_empty() {
        object.insert(key.to_string(), Value::String(value));
    }
}

fn insert_nonzero_i64(object: &mut Map<String, Value>, key: &str, value: i64) {
    if value != 0 {
        object.insert(key.to_string(), json!(value));
    }
}

fn insert_nonempty_array(object: &mut Map<String, Value>, key: &str, values: Vec<Value>) {
    if !values.is_empty() {
        object.insert(key.to_string(), Value::Array(values));
    }
}

fn json_f64(value: f64) -> Value {
    if value.is_finite() && value.fract() == 0.0 {
        json!(value as i64)
    } else {
        json!(value)
    }
}
