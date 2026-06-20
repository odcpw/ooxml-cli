use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;

use crate::cli_args::{parse_bool_flag, value_flag_present};
use crate::pptx_readback::pptx_charts_show;
use crate::{
    CliError, CliResult, command_arg, copy_zip_with_part_overrides, decode_xml_text, local_name,
    package_mutation_temp_path, package_type, parse_i64_flag, parse_string_flag, validate,
    validate_xlsx_mutation_output_flags, xml_attr_escape, xml_attrs_map, xml_escape,
    xml_general_ref, zip_text,
};

const CHART_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const DRAWING_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

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
const TX_PR_CHILD_ORDER: &[&str] = &["bodyPr", "lstStyle", "p"];
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
const PLOT_AREA_CHILD_ORDER: &[&str] = &["spPr", "extLst"];
const CHART_SPACE_CHILD_ORDER: &[&str] = &[
    "spPr",
    "txPr",
    "externalData",
    "printSettings",
    "userShapes",
    "extLst",
];

fn axis_child_order(element: &str) -> &'static [&'static str] {
    if element == "valAx" {
        VAL_AXIS_CHILD_ORDER
    } else {
        CAT_AXIS_CHILD_ORDER
    }
}

#[derive(Clone)]
struct PptxChartMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

#[derive(Clone, Default)]
struct FontOptions {
    family: Option<String>,
    size_pt: Option<f64>,
    color: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
}

struct SeriesStyleSpec<'a> {
    fill_color: Option<&'a str>,
    line_color: Option<&'a str>,
    line_width_pt: Option<f64>,
    marker_symbol: Option<&'a str>,
    marker_size: Option<i64>,
    expect_series_count: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChartType {
    Bar,
    Column,
    Line,
    Area,
    Pie,
    Scatter,
}

impl ChartType {
    fn as_str(self) -> &'static str {
        match self {
            ChartType::Bar => "bar",
            ChartType::Column => "column",
            ChartType::Line => "line",
            ChartType::Area => "area",
            ChartType::Pie => "pie",
            ChartType::Scatter => "scatter",
        }
    }

    fn element(self) -> &'static str {
        match self {
            ChartType::Bar | ChartType::Column => "barChart",
            ChartType::Line => "lineChart",
            ChartType::Area => "areaChart",
            ChartType::Pie => "pieChart",
            ChartType::Scatter => "scatterChart",
        }
    }
}

struct ConvertChartTypeResult {
    previous_type: ChartType,
    warnings: Vec<String>,
}

#[derive(Clone, Copy)]
enum AxisKind {
    Category,
    Value,
}

struct AxisFlags {
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
    tick_font: FontOptions,
    title_font: FontOptions,
}

impl AxisFlags {
    fn has_any(&self) -> bool {
        self.set_title
            || self.set_hidden
            || self.min.is_some()
            || self.max.is_some()
            || self.major_unit.is_some()
            || self.number_format.is_some()
            || self.set_major_gridlines
            || self.set_minor_gridlines
            || !self.tick_font.is_empty()
            || !self.title_font.is_empty()
    }
}

impl FontOptions {
    fn is_empty(&self) -> bool {
        self.family.is_none()
            && self.size_pt.is_none()
            && self.color.is_none()
            && self.bold.is_none()
            && self.italic.is_none()
    }
}

#[derive(Clone, Default)]
struct ChartStyleSnapshot {
    title_font: Option<FontOptions>,
    legend: Option<LegendStyleSnapshot>,
    axes: Vec<AxisStyleSnapshot>,
    series: Vec<SeriesStyleSnapshot>,
    plot_area_fill: Option<String>,
    chart_space_fill: Option<String>,
}

#[derive(Clone)]
struct LegendStyleSnapshot {
    position: Option<String>,
    overlay: Option<bool>,
}

#[derive(Clone, Default)]
struct AxisStyleSnapshot {
    element: String,
    axis_id: Option<String>,
    title_font: Option<FontOptions>,
    tick_font: Option<FontOptions>,
    number_format: Option<String>,
    major_gridlines: bool,
    minor_gridlines: bool,
}

#[derive(Clone, Default)]
struct SeriesStyleSnapshot {
    fill_color: Option<String>,
    line_color: Option<String>,
    line_width_pt: Option<f64>,
    marker_symbol: Option<String>,
    marker_size: Option<i64>,
}

struct ChartMutationResultInput<'a> {
    file: &'a str,
    output_path: Option<&'a str>,
    dry_run: bool,
    action: &'a str,
    chart: Value,
    extra_fields: Map<String, Value>,
    slide: i64,
    chart_selector: &'a str,
}

#[derive(Clone, Debug, Default)]
struct XmlNode {
    name: String,
    attrs: BTreeMap<String, String>,
    text: String,
    children: Vec<XmlNode>,
}

struct ChartXml {
    root: XmlNode,
    chart_prefix: String,
    drawing_prefix: String,
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

enum AreaFillTarget {
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

struct SelectedChart {
    part_uri: String,
}

impl SelectedChart {
    fn part_name(&self) -> String {
        self.part_uri.trim_start_matches('/').to_string()
    }

    fn part_selector(&self) -> String {
        format!("part:{}", self.part_uri)
    }
}

fn mutate_chart<F>(
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

fn ensure_pptx(file: &str) -> CliResult<()> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    Ok(())
}

fn selected_chart(
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
    Ok(SelectedChart { part_uri })
}

fn selected_chart_json(file: &str, slide: i64, selector: &str) -> CliResult<Value> {
    let result = pptx_charts_show(file, slide, Some(selector))?;
    result
        .get("charts")
        .and_then(Value::as_array)
        .and_then(|charts| charts.first())
        .cloned()
        .ok_or_else(|| CliError::unexpected("pptx charts show returned no selected chart"))
}

fn parse_chart_slide(args: &[String]) -> CliResult<i64> {
    parse_chart_slide_flag(args, "--slide")
}

fn parse_chart_slide_flag(args: &[String], name: &str) -> CliResult<i64> {
    let slide = parse_i64_flag(args, name)?.unwrap_or(0);
    if slide < 0 {
        return Err(CliError::invalid_args(format!("{name} must be >= 1")));
    }
    Ok(slide)
}

fn parse_required_chart_type(args: &[String], name: &str) -> CliResult<ChartType> {
    if !value_flag_present(args, name) {
        return Err(CliError::invalid_args(format!(
            "{name} is required (bar, column, line, area, pie, or scatter)"
        )));
    }
    let value = parse_string_flag(args, name)?.unwrap_or_default();
    parse_chart_type(&value).map_err(CliError::invalid_args)
}

fn parse_chart_type(value: &str) -> Result<ChartType, String> {
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

fn parse_axis_kind(args: &[String]) -> CliResult<AxisKind> {
    let value = parse_string_flag(args, "--axis")?.unwrap_or_default();
    match value.trim().to_ascii_lowercase().as_str() {
        "category" => Ok(AxisKind::Category),
        "value" => Ok(AxisKind::Value),
        _ => Err(CliError::invalid_args(
            "--axis is required; use category or value",
        )),
    }
}

fn parse_axis_flags(args: &[String]) -> CliResult<AxisFlags> {
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

fn parse_optional_f64_flag(args: &[String], name: &str) -> CliResult<Option<f64>> {
    let Some(raw) = parse_string_flag(args, name)? else {
        return Ok(None);
    };
    raw.trim()
        .parse::<f64>()
        .map(Some)
        .map_err(|_| CliError::invalid_args(format!("{name} must be a number")))
}

fn parse_optional_nonempty_string(args: &[String], name: &str) -> CliResult<Option<String>> {
    if !value_flag_present(args, name) {
        return Ok(None);
    }
    let value = parse_string_flag(args, name)?.unwrap_or_default();
    if value.trim().is_empty() {
        return Err(CliError::invalid_args(format!("{name} must not be empty")));
    }
    Ok(Some(value.trim().to_string()))
}

fn parse_optional_hex_color(args: &[String], name: &str) -> CliResult<Option<String>> {
    if !value_flag_present(args, name) {
        return Ok(None);
    }
    let value = parse_string_flag(args, name)?.unwrap_or_default();
    normalize_hex_color(&value).map(Some)
}

fn normalize_hex_color(value: &str) -> CliResult<String> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(hex.to_ascii_uppercase());
    }
    Err(CliError::invalid_args(format!(
        "color {value:?} must be a 6-digit hex like #1F77B4"
    )))
}

fn parse_chart_mutation_options(args: &[String]) -> CliResult<PptxChartMutationOptions> {
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

fn parse_font_options(args: &[String]) -> CliResult<FontOptions> {
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

fn parse_optional_positive_f64(args: &[String], name: &str) -> CliResult<Option<f64>> {
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

fn parse_required_color(args: &[String], name: &str) -> CliResult<String> {
    if !value_flag_present(args, name) {
        return Err(CliError::invalid_args(format!("{name} is required")));
    }
    normalize_color_value(&parse_string_flag(args, name)?.unwrap_or_default(), name)
}

fn parse_optional_color(args: &[String], name: &str) -> CliResult<Option<String>> {
    if !value_flag_present(args, name) {
        return Ok(None);
    }
    Ok(Some(normalize_color_value(
        &parse_string_flag(args, name)?.unwrap_or_default(),
        name,
    )?))
}

fn normalize_color_value(value: &str, name: &str) -> CliResult<String> {
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

fn set_chart_title(
    chart_xml: &mut ChartXml,
    title_text_value: &str,
    expect_title: Option<&str>,
    font: &FontOptions,
) -> CliResult<String> {
    let title_name = chart_xml.chart_name("title");
    let auto_title_deleted_name = chart_xml.chart_name("autoTitleDeleted");
    let tx = title_tx_node(chart_xml, title_text_value, font);
    let chart_index = chart_xml
        .root
        .direct_child_index("chart")
        .ok_or_else(|| CliError::unexpected("chart part has no chart element"))?;
    let chart = &mut chart_xml.root.children[chart_index];
    let previous_title = chart
        .direct_child("title")
        .map(title_text)
        .unwrap_or_default();
    if let Some(expect_title) = expect_title
        && previous_title != expect_title
    {
        return Err(CliError::invalid_args(format!(
            "title mismatch: expected {expect_title:?} but found {previous_title:?}"
        )));
    }

    let title_index =
        ensure_child_index(chart, "title", XmlNode::new(title_name), CHART_CHILD_ORDER);
    let title = &mut chart.children[title_index];
    if title
        .direct_child("tx")
        .and_then(|tx| tx.direct_child("strRef"))
        .is_some()
    {
        return Err(CliError::invalid_args(
            "chart title is linked to a cell; setting linked chart titles is not supported",
        ));
    }
    title.remove_direct_children("tx");
    insert_child_in_order(title, tx, TITLE_CHILD_ORDER);

    let auto_title_deleted_index = ensure_child_index(
        chart,
        "autoTitleDeleted",
        XmlNode::new(auto_title_deleted_name),
        CHART_CHILD_ORDER,
    );
    chart.children[auto_title_deleted_index].set_attr("val", "0");
    Ok(previous_title)
}

fn set_chart_legend(
    chart_xml: &mut ChartXml,
    position: Option<&str>,
    overlay: Option<bool>,
    expect_position: Option<&str>,
) -> CliResult<bool> {
    let legend_name = chart_xml.chart_name("legend");
    let legend_pos_name = chart_xml.chart_name("legendPos");
    let overlay_name = chart_xml.chart_name("overlay");
    let chart_index = chart_xml
        .root
        .direct_child_index("chart")
        .ok_or_else(|| CliError::unexpected("chart part has no chart element"))?;
    let chart = &mut chart_xml.root.children[chart_index];
    let current_position = chart
        .direct_child("legend")
        .and_then(|legend| legend.direct_child("legendPos"))
        .and_then(|pos| pos.attr("val"))
        .unwrap_or_default()
        .to_string();
    if let Some(expect) = expect_position {
        let expect = normalize_legend_position(expect, "--expect-position")?;
        if current_position != expect {
            return Err(CliError::invalid_args(format!(
                "legend position mismatch: expected {expect:?} but found {current_position:?}"
            )));
        }
    }

    let normalized_position = position
        .map(|value| normalize_legend_position(value, "--position"))
        .transpose()?;
    if normalized_position.as_deref() == Some("") {
        chart.remove_direct_children("legend");
        return Ok(true);
    }

    let legend_index = ensure_child_index(
        chart,
        "legend",
        XmlNode::new(legend_name),
        CHART_CHILD_ORDER,
    );
    let legend = &mut chart.children[legend_index];
    let position_value = normalized_position
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if current_position.is_empty() {
                "r"
            } else {
                &current_position
            }
        });
    if normalized_position.is_some() || legend.direct_child("legendPos").is_none() {
        let legend_pos_index = ensure_child_index(
            legend,
            "legendPos",
            XmlNode::new(legend_pos_name),
            LEGEND_CHILD_ORDER,
        );
        legend.children[legend_pos_index].set_attr("val", position_value);
    }
    if let Some(overlay) = overlay {
        let overlay_index = ensure_child_index(
            legend,
            "overlay",
            XmlNode::new(overlay_name),
            LEGEND_CHILD_ORDER,
        );
        legend.children[overlay_index].set_attr("val", bool_val(overlay));
    }
    Ok(false)
}

fn normalize_legend_position(value: &str, flag: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "right" | "r" => Ok("r".to_string()),
        "left" | "l" => Ok("l".to_string()),
        "top" | "t" => Ok("t".to_string()),
        "bottom" | "b" => Ok("b".to_string()),
        "none" => Ok(String::new()),
        _ => Err(CliError::invalid_args(format!(
            "{flag} must be right, left, top, bottom, or none"
        ))),
    }
}

fn set_area_fill(
    chart_xml: &mut ChartXml,
    target: &AreaFillTarget,
    fill: &str,
    expect_fill: Option<&str>,
) -> CliResult<String> {
    let sp_pr_name = chart_xml.chart_name("spPr");
    let drawing_prefix = chart_xml.drawing_prefix.clone();
    let previous_fill = match target {
        AreaFillTarget::ChartArea => chart_xml
            .root
            .direct_child("spPr")
            .map(inspect_fill)
            .unwrap_or_default(),
        AreaFillTarget::PlotArea => chart_xml
            .root
            .first_descendant("plotArea")
            .and_then(|plot_area| plot_area.direct_child("spPr"))
            .map(inspect_fill)
            .unwrap_or_default(),
    };
    if let Some(expect_fill) = expect_fill
        && previous_fill != expect_fill
    {
        return Err(CliError::invalid_args(format!(
            "fill mismatch: expected {expect_fill:?} but found {previous_fill:?}"
        )));
    }

    match target {
        AreaFillTarget::ChartArea => {
            let sp_pr_index = ensure_child_index(
                &mut chart_xml.root,
                "spPr",
                XmlNode::new(sp_pr_name),
                &[
                    "date1904",
                    "lang",
                    "roundedCorners",
                    "style",
                    "clrMapOvr",
                    "pivotSource",
                    "protection",
                    "chart",
                    "spPr",
                    "txPr",
                    "externalData",
                    "printSettings",
                    "userShapes",
                    "extLst",
                ],
            );
            set_shape_fill(
                &mut chart_xml.root.children[sp_pr_index],
                &drawing_prefix,
                fill,
            );
        }
        AreaFillTarget::PlotArea => {
            let plot_area = chart_xml
                .root
                .first_descendant_mut("plotArea")
                .ok_or_else(|| CliError::unexpected("chart has no plotArea"))?;
            let sp_pr_index = ensure_child_index(
                plot_area,
                "spPr",
                XmlNode::new(sp_pr_name),
                &[
                    "layout",
                    "area3DChart",
                    "areaChart",
                    "lineChart",
                    "stockChart",
                    "radarChart",
                    "scatterChart",
                    "pieChart",
                    "pie3DChart",
                    "doughnutChart",
                    "barChart",
                    "bar3DChart",
                    "ofPieChart",
                    "surfaceChart",
                    "surface3DChart",
                    "bubbleChart",
                    "valAx",
                    "catAx",
                    "dateAx",
                    "serAx",
                    "dTable",
                    "spPr",
                    "extLst",
                ],
            );
            set_shape_fill(&mut plot_area.children[sp_pr_index], &drawing_prefix, fill);
        }
    }
    Ok(previous_fill)
}

fn set_series_style(
    chart_xml: &mut ChartXml,
    series: usize,
    spec: &SeriesStyleSpec<'_>,
) -> CliResult<()> {
    let count = series_count(&chart_xml.root);
    if let Some(expect) = spec.expect_series_count
        && count as i64 != expect
    {
        return Err(CliError::invalid_args(format!(
            "series count mismatch: expected {expect} but found {count}"
        )));
    }
    if series == 0 || series > count {
        return Err(CliError::invalid_args(format!(
            "series {series} is out of range (1-{count})"
        )));
    }
    let c_prefix = chart_xml.chart_prefix.clone();
    let a_prefix = chart_xml.drawing_prefix.clone();
    let mut seen = 0_usize;
    let plot_area = chart_xml
        .root
        .first_descendant_mut("plotArea")
        .ok_or_else(|| CliError::unexpected("chart has no plotArea"))?;
    for chart_type in &mut plot_area.children {
        if !chart_type.local().ends_with("Chart") {
            continue;
        }
        let chart_type_name = chart_type.local().to_string();
        for ser in chart_type
            .children
            .iter_mut()
            .filter(|node| node.local() == "ser")
        {
            seen += 1;
            if seen != series {
                continue;
            }
            let sp_pr_index = ensure_child_index(
                ser,
                "spPr",
                XmlNode::new(qname(&c_prefix, "spPr")),
                SERIES_CHILD_ORDER,
            );
            let sp_pr = &mut ser.children[sp_pr_index];
            if let Some(fill_color) = spec.fill_color {
                set_shape_fill(sp_pr, &a_prefix, fill_color);
            }
            if spec.line_color.is_some() || spec.line_width_pt.is_some() {
                let line_index = ensure_child_index(
                    sp_pr,
                    "ln",
                    XmlNode::new(qname(&a_prefix, "ln")),
                    SHAPE_PROPS_CHILD_ORDER,
                );
                let line = &mut sp_pr.children[line_index];
                if let Some(width) = spec.line_width_pt {
                    line.set_attr("w", &(width * 12700.0).round().to_string());
                }
                if let Some(line_color) = spec.line_color {
                    set_line_fill(line, &a_prefix, line_color);
                }
            }
            if spec.marker_symbol.is_some() || spec.marker_size.is_some() {
                if !matches!(
                    chart_type_name.as_str(),
                    "lineChart" | "scatterChart" | "radarChart"
                ) {
                    return Err(CliError::invalid_args(format!(
                        "markers are not supported for {chart_type_name}"
                    )));
                }
                let marker_index = ensure_child_index(
                    ser,
                    "marker",
                    XmlNode::new(qname(&c_prefix, "marker")),
                    SERIES_CHILD_ORDER,
                );
                let marker = &mut ser.children[marker_index];
                if let Some(symbol) = spec.marker_symbol {
                    let symbol = normalize_marker_symbol(symbol)?;
                    let symbol_index = ensure_child_index(
                        marker,
                        "symbol",
                        XmlNode::new(qname(&c_prefix, "symbol")),
                        MARKER_CHILD_ORDER,
                    );
                    marker.children[symbol_index].set_attr("val", &symbol);
                }
                if let Some(size) = spec.marker_size {
                    let size_index = ensure_child_index(
                        marker,
                        "size",
                        XmlNode::new(qname(&c_prefix, "size")),
                        MARKER_CHILD_ORDER,
                    );
                    marker.children[size_index].set_attr("val", &size.to_string());
                }
            }
            return Ok(());
        }
    }
    Err(CliError::unexpected("selected series was not found"))
}

fn normalize_marker_symbol(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "circle" | "square" | "diamond" | "triangle" | "none" => {
            Ok(value.trim().to_ascii_lowercase())
        }
        _ => Err(CliError::invalid_args(
            "--marker-symbol must be circle, square, diamond, triangle, or none",
        )),
    }
}

fn convert_chart_type(
    chart_xml: &mut ChartXml,
    target: ChartType,
    expect_type: Option<ChartType>,
) -> CliResult<ConvertChartTypeResult> {
    let c_prefix = chart_xml.chart_prefix.clone();
    let plot_area = chart_xml
        .root
        .first_descendant_mut("plotArea")
        .ok_or_else(|| CliError::invalid_args("chart part has no plotArea"))?;
    let plot_index = first_plot_index(plot_area)
        .ok_or_else(|| CliError::invalid_args("chart part has no chart-type plot element"))?;
    let previous = canonical_chart_type(&plot_area.children[plot_index])?;
    if let Some(expect) = expect_type
        && expect != previous
    {
        return Err(CliError::invalid_args(format!(
            "chart type mismatch: expected {} but found {}; use --dry-run to inspect",
            expect.as_str(),
            previous.as_str()
        )));
    }
    if previous == target {
        return Err(CliError::invalid_args(format!(
            "chart is already a {} chart",
            target.as_str()
        )));
    }
    if previous == ChartType::Pie {
        return Err(CliError::invalid_args(format!(
            "cannot convert from pie to {}: pie charts have no category/value axis structure to carry over; recreate the chart with `charts create --type {}`",
            target.as_str(),
            target.as_str()
        )));
    }

    let old_plot = plot_area.children[plot_index].clone();
    let mut series = old_plot
        .children
        .iter()
        .filter(|child| child.local() == "ser")
        .cloned()
        .collect::<Vec<_>>();
    if series.is_empty() {
        return Err(CliError::invalid_args("chart has no series to convert"));
    }
    if target == ChartType::Pie && series.len() > 1 {
        return Err(CliError::invalid_args(format!(
            "cannot convert to pie: a pie chart supports a single series but this chart has {}; remove series before converting",
            series.len()
        )));
    }

    let mut warnings = Vec::new();
    if previous.element() == target.element() {
        set_bar_dir(&mut plot_area.children[plot_index], target, &c_prefix);
        return Ok(ConvertChartTypeResult {
            previous_type: previous,
            warnings,
        });
    }

    let axis_ids = plot_axis_ids(&old_plot);
    for (idx, ser) in series.iter_mut().enumerate() {
        warnings.extend(transform_series_for_type(
            ser,
            previous,
            target,
            idx + 1,
            &c_prefix,
        ));
    }
    plot_area.children[plot_index] = build_plot_wrapper(target, series, &axis_ids, &c_prefix);
    if let Some(warning) =
        transform_axes_for_type(plot_area, previous, target, &axis_ids, &c_prefix)
    {
        warnings.push(warning);
    }
    Ok(ConvertChartTypeResult {
        previous_type: previous,
        warnings,
    })
}

fn first_plot_index(plot_area: &XmlNode) -> Option<usize> {
    plot_area
        .children
        .iter()
        .position(|child| child.local().ends_with("Chart"))
}

fn canonical_chart_type(plot: &XmlNode) -> CliResult<ChartType> {
    match plot.local() {
        "barChart" | "bar3DChart" => {
            if plot
                .direct_child("barDir")
                .and_then(|node| node.attr("val"))
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("bar"))
            {
                Ok(ChartType::Bar)
            } else {
                Ok(ChartType::Column)
            }
        }
        "lineChart" | "line3DChart" => Ok(ChartType::Line),
        "areaChart" | "area3DChart" => Ok(ChartType::Area),
        "pieChart" | "pie3DChart" | "doughnutChart" | "ofPieChart" => Ok(ChartType::Pie),
        "scatterChart" => Ok(ChartType::Scatter),
        other => Err(CliError::invalid_args(format!(
            "chart type {other:?} is not supported for conversion"
        ))),
    }
}

fn set_bar_dir(plot: &mut XmlNode, target: ChartType, chart_prefix: &str) {
    let value = if target == ChartType::Bar {
        "bar"
    } else {
        "col"
    };
    let index = ensure_child_index(
        plot,
        "barDir",
        chart_value_node(chart_prefix, "barDir", value),
        &[
            "barDir",
            "grouping",
            "varyColors",
            "ser",
            "dLbls",
            "gapWidth",
            "overlap",
            "serLines",
            "axId",
            "extLst",
        ],
    );
    plot.children[index].set_attr("val", value);
}

fn plot_axis_ids(plot: &XmlNode) -> Vec<String> {
    plot.children
        .iter()
        .filter(|child| child.local() == "axId")
        .filter_map(|child| child.attr("val"))
        .map(|value| value.trim().to_string())
        .collect()
}

fn transform_series_for_type(
    ser: &mut XmlNode,
    from: ChartType,
    to: ChartType,
    number: usize,
    chart_prefix: &str,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if to == ChartType::Scatter && from != ChartType::Scatter {
        rename_direct_child(ser, "cat", "xVal", chart_prefix);
        rename_direct_child(ser, "val", "yVal", chart_prefix);
        if let Some(x_values) = ser.direct_child("xVal")
            && (x_values.direct_child("strRef").is_some()
                || x_values.direct_child("multiLvlStrRef").is_some())
        {
            warnings.push(format!(
                "series {number} x-values are a text reference; scatter charts expect numeric x-values, so the chart may misrender until the source is re-pointed at numeric data"
            ));
        }
    } else if from == ChartType::Scatter && to != ChartType::Scatter {
        rename_direct_child(ser, "xVal", "cat", chart_prefix);
        rename_direct_child(ser, "yVal", "val", chart_prefix);
    }
    if !matches!(to, ChartType::Line | ChartType::Scatter) {
        let before = ser.children.len();
        ser.children.retain(|child| child.local() != "marker");
        if ser.children.len() != before {
            warnings.push(format!(
                "series {number} had a marker style; {} charts do not support markers, so it was removed",
                to.as_str()
            ));
        }
    }
    reorder_children(ser, SERIES_CHILD_ORDER);
    warnings
}

fn rename_direct_child(parent: &mut XmlNode, from: &str, to: &str, chart_prefix: &str) {
    if let Some(child) = parent
        .children
        .iter_mut()
        .find(|child| child.local() == from)
    {
        child.name = qname(chart_prefix, to);
    }
}

fn reorder_children(parent: &mut XmlNode, order: &[&str]) {
    let children = std::mem::take(&mut parent.children);
    for child in children {
        insert_child_in_order(parent, child, order);
    }
}

fn build_plot_wrapper(
    target: ChartType,
    series: Vec<XmlNode>,
    axis_ids: &[String],
    chart_prefix: &str,
) -> XmlNode {
    let mut plot = XmlNode::new(qname(chart_prefix, target.element()));
    match target {
        ChartType::Bar | ChartType::Column => {
            let direction = if target == ChartType::Bar {
                "bar"
            } else {
                "col"
            };
            plot.children
                .push(chart_value_node(chart_prefix, "barDir", direction));
            plot.children
                .push(chart_value_node(chart_prefix, "grouping", "clustered"));
            plot.children
                .push(chart_value_node(chart_prefix, "varyColors", "0"));
            plot.children.extend(series);
            append_axis_ids(&mut plot, axis_ids, 2, chart_prefix);
        }
        ChartType::Line => {
            plot.children
                .push(chart_value_node(chart_prefix, "grouping", "standard"));
            plot.children
                .push(chart_value_node(chart_prefix, "varyColors", "0"));
            plot.children.extend(series);
            plot.children
                .push(chart_value_node(chart_prefix, "marker", "1"));
            append_axis_ids(&mut plot, axis_ids, 2, chart_prefix);
        }
        ChartType::Area => {
            plot.children
                .push(chart_value_node(chart_prefix, "grouping", "standard"));
            plot.children
                .push(chart_value_node(chart_prefix, "varyColors", "0"));
            plot.children.extend(series);
            append_axis_ids(&mut plot, axis_ids, 2, chart_prefix);
        }
        ChartType::Pie => {
            plot.children
                .push(chart_value_node(chart_prefix, "varyColors", "1"));
            plot.children.extend(series);
            plot.children
                .push(chart_value_node(chart_prefix, "firstSliceAng", "0"));
        }
        ChartType::Scatter => {
            plot.children
                .push(chart_value_node(chart_prefix, "scatterStyle", "lineMarker"));
            plot.children
                .push(chart_value_node(chart_prefix, "varyColors", "0"));
            plot.children.extend(series);
            append_axis_ids(&mut plot, axis_ids, 2, chart_prefix);
        }
    }
    plot
}

fn append_axis_ids(plot: &mut XmlNode, axis_ids: &[String], count: usize, chart_prefix: &str) {
    let fallback = ["111111111", "222222222"];
    for (idx, fallback_id) in fallback.iter().enumerate().take(count) {
        let id = axis_ids
            .get(idx)
            .filter(|value| !value.trim().is_empty())
            .map(String::as_str)
            .unwrap_or(fallback_id);
        plot.children
            .push(chart_value_node(chart_prefix, "axId", id));
    }
}

fn transform_axes_for_type(
    plot_area: &mut XmlNode,
    from: ChartType,
    to: ChartType,
    axis_ids: &[String],
    chart_prefix: &str,
) -> Option<String> {
    if to == ChartType::Pie {
        plot_area
            .children
            .retain(|child| !matches!(child.local(), "catAx" | "valAx" | "dateAx" | "serAx"));
        return None;
    }
    let category_axis_id = axis_ids.first().map(String::as_str).unwrap_or_default();
    if to == ChartType::Scatter && from != ChartType::Scatter {
        if let Some(index) = axis_by_id_index(plot_area, category_axis_id, "catAx") {
            rename_axis_element(&mut plot_area.children[index], "valAx", chart_prefix);
            return Some("category axis converted to a value axis for the scatter chart; review its scale and number format".to_string());
        }
    } else if from == ChartType::Scatter
        && to != ChartType::Scatter
        && let Some(index) = axis_by_id_index(plot_area, category_axis_id, "valAx")
    {
        rename_axis_element(&mut plot_area.children[index], "catAx", chart_prefix);
        return Some(
            "scatter x value axis converted to a category axis; review its labels and number format"
                .to_string(),
        );
    }
    None
}

fn axis_by_id_index(plot_area: &XmlNode, id: &str, fallback: &str) -> Option<usize> {
    let trimmed = id.trim();
    if !trimmed.is_empty()
        && let Some(index) = plot_area.children.iter().position(|child| {
            matches!(child.local(), "catAx" | "valAx" | "dateAx" | "serAx")
                && child
                    .direct_child("axId")
                    .and_then(|axis_id| axis_id.attr("val"))
                    .is_some_and(|value| value.trim() == trimmed)
        })
    {
        return Some(index);
    }
    plot_area
        .children
        .iter()
        .position(|child| child.local() == fallback)
}

fn rename_axis_element(axis: &mut XmlNode, new_local: &str, chart_prefix: &str) {
    axis.name = qname(chart_prefix, new_local);
    let order = axis_child_order(new_local);
    axis.children
        .retain(|child| order.iter().any(|candidate| *candidate == child.local()));
}

fn set_chart_axis(
    chart_xml: &mut ChartXml,
    kind: AxisKind,
    flags: &AxisFlags,
    expect_title: Option<&str>,
    expect_count: Option<i64>,
) -> CliResult<String> {
    if flags.major_unit.is_some() && !matches!(kind, AxisKind::Value) {
        return Err(CliError::invalid_args(
            "major unit applies only to the value axis (use --axis value)",
        ));
    }
    let c_prefix = chart_xml.chart_prefix.clone();
    let a_prefix = chart_xml.drawing_prefix.clone();
    let plot_area = chart_xml
        .root
        .first_descendant_mut("plotArea")
        .ok_or_else(|| CliError::invalid_args("chart part has no plotArea"))?;
    let axis_count = plot_area
        .children
        .iter()
        .filter(|child| matches!(child.local(), "catAx" | "valAx" | "dateAx" | "serAx"))
        .count();
    if let Some(expect_count) = expect_count
        && axis_count as i64 != expect_count
    {
        return Err(CliError::invalid_args(format!(
            "axis count mismatch: expected {expect_count} but found {axis_count}"
        )));
    }

    let wanted = match kind {
        AxisKind::Category => "catAx",
        AxisKind::Value => "valAx",
    };
    let matches = plot_area
        .children
        .iter()
        .enumerate()
        .filter_map(|(idx, child)| (child.local() == wanted).then_some(idx))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        let label = match kind {
            AxisKind::Category => "category",
            AxisKind::Value => "value",
        };
        return Err(CliError::invalid_args(format!("chart has no {label} axis")));
    }
    if matches.len() > 1 {
        let label = match kind {
            AxisKind::Category => "category",
            AxisKind::Value => "value",
        };
        return Err(CliError::invalid_args(format!(
            "chart has {} {label} axes (e.g. a scatter chart's x and y axes); axis selection is ambiguous, narrow the chart with --chart or guard with --expect-axis-count",
            matches.len()
        )));
    }
    let axis = &mut plot_area.children[matches[0]];
    let order = axis_child_order(axis.local());
    let previous_title = axis
        .direct_child("title")
        .map(title_text)
        .unwrap_or_default();
    if let Some(expect_title) = expect_title
        && previous_title.trim() != expect_title.trim()
    {
        return Err(CliError::invalid_args(format!(
            "axis title mismatch: expected {expect_title:?} but found {previous_title:?}"
        )));
    }
    if flags.set_title {
        if flags.title.trim().is_empty() {
            axis.remove_direct_children("title");
        } else {
            let title_index = ensure_child_index(
                axis,
                "title",
                XmlNode::new(qname(&c_prefix, "title")),
                order,
            );
            let title = &mut axis.children[title_index];
            if title
                .direct_child("tx")
                .and_then(|tx| tx.direct_child("strRef"))
                .is_some()
            {
                return Err(CliError::invalid_args(
                    "title is linked to a cell; setting literal title text is not supported",
                ));
            }
            title.remove_direct_children("tx");
            insert_child_in_order(
                title,
                title_tx_node_for_prefixes(&c_prefix, &a_prefix, &flags.title, &flags.title_font),
                TITLE_CHILD_ORDER,
            );
        }
    }
    if flags.set_hidden {
        set_or_create_val_child(axis, "delete", bool_val(flags.hidden), order, &c_prefix);
    }
    if flags.min.is_some() || flags.max.is_some() {
        let scaling_index = ensure_child_index(
            axis,
            "scaling",
            XmlNode::new(qname(&c_prefix, "scaling")),
            order,
        );
        let scaling = &mut axis.children[scaling_index];
        if let Some(max) = flags.max {
            set_or_create_val_child(
                scaling,
                "max",
                &format_float(max),
                SCALING_CHILD_ORDER,
                &c_prefix,
            );
        }
        if let Some(min) = flags.min {
            set_or_create_val_child(
                scaling,
                "min",
                &format_float(min),
                SCALING_CHILD_ORDER,
                &c_prefix,
            );
        }
    }
    if let Some(major_unit) = flags.major_unit {
        set_or_create_val_child(
            axis,
            "majorUnit",
            &format_float(major_unit),
            order,
            &c_prefix,
        );
    }
    if let Some(format_code) = flags.number_format.as_deref() {
        let index = ensure_child_index(
            axis,
            "numFmt",
            XmlNode::new(qname(&c_prefix, "numFmt")),
            order,
        );
        axis.children[index].set_attr("formatCode", format_code);
        axis.children[index].set_attr("sourceLinked", "0");
    }
    if flags.set_major_gridlines {
        apply_gridlines(
            axis,
            "majorGridlines",
            flags.major_gridlines,
            order,
            &c_prefix,
        );
    }
    if flags.set_minor_gridlines {
        apply_gridlines(
            axis,
            "minorGridlines",
            flags.minor_gridlines,
            order,
            &c_prefix,
        );
    }
    if !flags.tick_font.is_empty() {
        let prefix_chart_xml = ChartXml {
            root: XmlNode::new(String::new()),
            chart_prefix: c_prefix.clone(),
            drawing_prefix: a_prefix.clone(),
        };
        apply_axis_tick_label_font(axis, &flags.tick_font, order, &prefix_chart_xml);
    }
    Ok(previous_title)
}

fn apply_gridlines(
    axis: &mut XmlNode,
    local: &str,
    enabled: bool,
    order: &[&str],
    chart_prefix: &str,
) {
    if enabled {
        ensure_child_index(axis, local, XmlNode::new(qname(chart_prefix, local)), order);
    } else {
        axis.remove_direct_children(local);
    }
}

fn apply_axis_tick_label_font(
    axis: &mut XmlNode,
    font: &FontOptions,
    order: &[&str],
    chart_xml: &ChartXml,
) {
    let tx_pr_index = ensure_child_index(
        axis,
        "txPr",
        {
            let mut tx_pr = XmlNode::new(chart_xml.chart_name("txPr"));
            tx_pr
                .children
                .push(XmlNode::new(chart_xml.drawing_name("bodyPr")));
            tx_pr
                .children
                .push(XmlNode::new(chart_xml.drawing_name("lstStyle")));
            tx_pr
        },
        order,
    );
    let tx_pr = &mut axis.children[tx_pr_index];
    let p_index = ensure_child_index(
        tx_pr,
        "p",
        XmlNode::new(chart_xml.drawing_name("p")),
        TX_PR_CHILD_ORDER,
    );
    let paragraph = &mut tx_pr.children[p_index];
    let p_pr_index = ensure_child_index(
        paragraph,
        "pPr",
        XmlNode::new(chart_xml.drawing_name("pPr")),
        PARAGRAPH_CHILD_ORDER,
    );
    let p_pr = &mut paragraph.children[p_pr_index];
    let def_r_pr_index = ensure_child_index(
        p_pr,
        "defRPr",
        XmlNode::new(chart_xml.drawing_name("defRPr")),
        &["defRPr", "extLst"],
    );
    apply_font_options(&mut p_pr.children[def_r_pr_index], chart_xml, font);
    if paragraph.direct_child("r").is_none() && paragraph.direct_child("endParaRPr").is_none() {
        insert_child_in_order(
            paragraph,
            XmlNode::new(chart_xml.drawing_name("endParaRPr")),
            PARAGRAPH_CHILD_ORDER,
        );
    }
}

fn set_or_create_val_child(
    parent: &mut XmlNode,
    local: &str,
    value: &str,
    order: &[&str],
    chart_prefix: &str,
) {
    let index = ensure_child_index(
        parent,
        local,
        chart_value_node(chart_prefix, local, value),
        order,
    );
    parent.children[index].set_attr("val", value);
}

fn chart_value_node(chart_prefix: &str, local: &str, value: &str) -> XmlNode {
    let mut node = XmlNode::new(qname(chart_prefix, local));
    node.set_attr("val", value);
    node
}

fn format_float(value: f64) -> String {
    let formatted = format!("{value}");
    formatted
}

fn read_template_chart_style(
    file: &str,
    slide: i64,
    selector: &str,
) -> CliResult<ChartStyleSnapshot> {
    ensure_pptx(file)?;
    let selected = selected_chart(file, slide, Some(selector))?;
    let chart_xml_text = zip_text(file, &selected.part_name())?;
    let chart_xml = parse_chart_xml(&chart_xml_text)?;
    Ok(inspect_style_snapshot(&chart_xml.root))
}

fn inspect_style_snapshot(root: &XmlNode) -> ChartStyleSnapshot {
    let mut snapshot = ChartStyleSnapshot::default();
    if let Some(chart) = root.direct_child("chart") {
        if let Some(title) = chart.direct_child("title") {
            snapshot.title_font = inspect_title_font_snapshot(title);
        }
        if let Some(legend) = chart.direct_child("legend") {
            snapshot.legend = Some(LegendStyleSnapshot {
                position: legend
                    .direct_child("legendPos")
                    .and_then(|node| node.attr("val"))
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                overlay: legend
                    .direct_child("overlay")
                    .and_then(|node| node.attr("val"))
                    .map(parse_ooxml_bool),
            });
        }
    }
    if let Some(plot_area) = root.first_descendant("plotArea") {
        snapshot.axes = inspect_axis_snapshots(plot_area);
        snapshot.plot_area_fill = plot_area
            .direct_child("spPr")
            .map(inspect_fill)
            .filter(|value| !value.is_empty());
    }
    snapshot.chart_space_fill = root
        .direct_child("spPr")
        .map(inspect_fill)
        .filter(|value| !value.is_empty());
    snapshot.series = walk_series(root)
        .into_iter()
        .map(inspect_series_snapshot)
        .collect();
    snapshot
}

fn inspect_axis_snapshots(plot_area: &XmlNode) -> Vec<AxisStyleSnapshot> {
    plot_area
        .children
        .iter()
        .filter(|child| matches!(child.local(), "catAx" | "valAx" | "dateAx" | "serAx"))
        .map(|axis| AxisStyleSnapshot {
            element: axis.local().to_string(),
            axis_id: axis
                .direct_child("axId")
                .and_then(|node| node.attr("val"))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            title_font: axis
                .direct_child("title")
                .and_then(inspect_title_font_snapshot),
            tick_font: axis
                .direct_child("txPr")
                .and_then(inspect_tick_font_snapshot),
            number_format: axis
                .direct_child("numFmt")
                .and_then(|node| node.attr("formatCode"))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            major_gridlines: axis.direct_child("majorGridlines").is_some(),
            minor_gridlines: axis.direct_child("minorGridlines").is_some(),
        })
        .collect()
}

fn inspect_series_snapshot(ser: &XmlNode) -> SeriesStyleSnapshot {
    let mut snapshot = SeriesStyleSnapshot::default();
    if let Some(sp_pr) = ser.direct_child("spPr") {
        let fill = inspect_fill(sp_pr);
        if !fill.is_empty() {
            snapshot.fill_color = Some(fill);
        }
        if let Some(line) = sp_pr.direct_child("ln") {
            let line_fill = inspect_fill(line);
            if !line_fill.is_empty() {
                snapshot.line_color = Some(line_fill);
            }
            snapshot.line_width_pt = line
                .attr("w")
                .and_then(|value| value.trim().parse::<f64>().ok())
                .map(|emu| emu / 12700.0);
        }
    }
    if let Some(marker) = ser.direct_child("marker") {
        snapshot.marker_symbol = marker
            .direct_child("symbol")
            .and_then(|node| node.attr("val"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        snapshot.marker_size = marker
            .direct_child("size")
            .and_then(|node| node.attr("val"))
            .and_then(|value| value.trim().parse::<i64>().ok());
    }
    snapshot
}

fn inspect_title_font_snapshot(title: &XmlNode) -> Option<FontOptions> {
    if let Some(rich) = title.first_descendant("rich") {
        if let Some(run) = rich.first_descendant("r")
            && let Some(font) = run.direct_child("rPr").and_then(inspect_font_snapshot)
        {
            return Some(font);
        }
        if let Some(p_pr) = rich.first_descendant("pPr")
            && let Some(font) = p_pr.direct_child("defRPr").and_then(inspect_font_snapshot)
        {
            return Some(font);
        }
    }
    if let Some(tx_pr) = title.direct_child("txPr") {
        return inspect_tick_font_snapshot(tx_pr);
    }
    None
}

fn inspect_tick_font_snapshot(tx_pr: &XmlNode) -> Option<FontOptions> {
    if let Some(p_pr) = tx_pr.first_descendant("pPr")
        && let Some(font) = p_pr.direct_child("defRPr").and_then(inspect_font_snapshot)
    {
        return Some(font);
    }
    if let Some(run) = tx_pr.first_descendant("r") {
        return run.direct_child("rPr").and_then(inspect_font_snapshot);
    }
    None
}

fn inspect_font_snapshot(r_pr: &XmlNode) -> Option<FontOptions> {
    let font = FontOptions {
        family: r_pr
            .direct_child("latin")
            .and_then(|node| node.attr("typeface"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        size_pt: r_pr
            .attr("sz")
            .and_then(|value| value.trim().parse::<f64>().ok())
            .map(|hundredths| hundredths / 100.0),
        color: {
            let fill = inspect_fill(r_pr);
            (!fill.is_empty()).then_some(fill)
        },
        bold: r_pr.attr("b").map(parse_ooxml_bool),
        italic: r_pr.attr("i").map(parse_ooxml_bool),
    };
    (!font.is_empty()).then_some(font)
}

fn apply_chart_style(
    chart_xml: &mut ChartXml,
    source: &ChartStyleSnapshot,
    expect_series_count: Option<i64>,
) -> CliResult<Vec<String>> {
    let target_series_count = series_count(&chart_xml.root);
    if let Some(expect) = expect_series_count
        && target_series_count as i64 != expect
    {
        return Err(CliError::invalid_args(format!(
            "series count mismatch: expected {expect} but found {target_series_count}"
        )));
    }
    let mut applied = Vec::new();
    let c_prefix = chart_xml.chart_prefix.clone();
    let a_prefix = chart_xml.drawing_prefix.clone();
    let chart_index = chart_xml
        .root
        .direct_child_index("chart")
        .ok_or_else(|| CliError::unexpected("chart part has no chart element"))?;
    if let Some(font) = source.title_font.as_ref() {
        let chart = &mut chart_xml.root.children[chart_index];
        if let Some(title) = chart.direct_child_mut("title")
            && let Some(run) = title.first_descendant_mut("r")
        {
            let r_pr_index = ensure_child_index(
                run,
                "rPr",
                XmlNode::new(qname(&a_prefix, "rPr")),
                &["rPr", "t"],
            );
            apply_font_options_with_scheme(&mut run.children[r_pr_index], &a_prefix, font);
            applied.push("title-font".to_string());
        }
    }
    if let Some(legend_source) = source.legend.as_ref()
        && let Some(position) = legend_source.position.as_deref()
    {
        let chart = &mut chart_xml.root.children[chart_index];
        let legend_index = ensure_child_index(
            chart,
            "legend",
            XmlNode::new(qname(&c_prefix, "legend")),
            CHART_CHILD_ORDER,
        );
        let legend = &mut chart.children[legend_index];
        set_or_create_val_child(legend, "legendPos", position, LEGEND_CHILD_ORDER, &c_prefix);
        if let Some(overlay) = legend_source.overlay {
            set_or_create_val_child(
                legend,
                "overlay",
                bool_val(overlay),
                LEGEND_CHILD_ORDER,
                &c_prefix,
            );
        }
        applied.push("legend".to_string());
    }
    if let Some(plot_area) = chart_xml.root.first_descendant_mut("plotArea") {
        for source_axis in &source.axes {
            let Some(axis_index) = find_target_axis_index(plot_area, source_axis) else {
                continue;
            };
            let axis = &mut plot_area.children[axis_index];
            let order = axis_child_order(axis.local());
            let mut changed = false;
            if let Some(font) = source_axis.title_font.as_ref()
                && let Some(title) = axis.direct_child_mut("title")
                && let Some(run) = title.first_descendant_mut("r")
            {
                let r_pr_index = ensure_child_index(
                    run,
                    "rPr",
                    XmlNode::new(qname(&a_prefix, "rPr")),
                    &["rPr", "t"],
                );
                apply_font_options_with_scheme(&mut run.children[r_pr_index], &a_prefix, font);
                changed = true;
            }
            if let Some(font) = source_axis.tick_font.as_ref() {
                let fake_chart_xml = ChartXml {
                    root: XmlNode::new(String::new()),
                    chart_prefix: c_prefix.clone(),
                    drawing_prefix: a_prefix.clone(),
                };
                apply_axis_tick_label_font(axis, font, order, &fake_chart_xml);
                changed = true;
            }
            if let Some(number_format) = source_axis.number_format.as_deref() {
                let index = ensure_child_index(
                    axis,
                    "numFmt",
                    XmlNode::new(qname(&c_prefix, "numFmt")),
                    order,
                );
                axis.children[index].set_attr("formatCode", number_format);
                axis.children[index].set_attr("sourceLinked", "0");
                changed = true;
            }
            apply_gridlines(
                axis,
                "majorGridlines",
                source_axis.major_gridlines,
                order,
                &c_prefix,
            );
            apply_gridlines(
                axis,
                "minorGridlines",
                source_axis.minor_gridlines,
                order,
                &c_prefix,
            );
            if changed {
                applied.push(format!("axis:{}", source_axis.element));
            } else {
                applied.push(format!("axis-gridlines:{}", source_axis.element));
            }
        }
    }
    apply_source_series_styles(
        &mut chart_xml.root,
        source,
        &c_prefix,
        &a_prefix,
        &mut applied,
    );
    if let Some(fill) = source.plot_area_fill.as_deref()
        && let Some(plot_area) = chart_xml.root.first_descendant_mut("plotArea")
    {
        let sp_pr_index = ensure_child_index(
            plot_area,
            "spPr",
            XmlNode::new(qname(&c_prefix, "spPr")),
            PLOT_AREA_CHILD_ORDER,
        );
        set_shape_fill_from_source(&mut plot_area.children[sp_pr_index], &a_prefix, fill);
        applied.push("plot-area-fill".to_string());
    }
    if let Some(fill) = source.chart_space_fill.as_deref() {
        let sp_pr_index = ensure_child_index(
            &mut chart_xml.root,
            "spPr",
            XmlNode::new(qname(&c_prefix, "spPr")),
            CHART_SPACE_CHILD_ORDER,
        );
        set_shape_fill_from_source(&mut chart_xml.root.children[sp_pr_index], &a_prefix, fill);
        applied.push("chart-area-fill".to_string());
    }
    Ok(applied)
}

fn find_target_axis_index(plot_area: &XmlNode, source: &AxisStyleSnapshot) -> Option<usize> {
    let mut fallback = None;
    for (idx, axis) in plot_area.children.iter().enumerate() {
        if axis.local() != source.element {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(idx);
        }
        if let Some(source_id) = source.axis_id.as_deref()
            && axis
                .direct_child("axId")
                .and_then(|node| node.attr("val"))
                .is_some_and(|value| value.trim() == source_id)
        {
            return Some(idx);
        }
    }
    fallback
}

fn apply_source_series_styles(
    root: &mut XmlNode,
    source: &ChartStyleSnapshot,
    chart_prefix: &str,
    drawing_prefix: &str,
    applied: &mut Vec<String>,
) {
    let mut seen = 0_usize;
    if let Some(plot_area) = root.first_descendant_mut("plotArea") {
        for chart_type in &mut plot_area.children {
            let chart_type_name = chart_type.local().to_string();
            if !chart_type_name.ends_with("Chart") {
                continue;
            }
            for ser in chart_type
                .children
                .iter_mut()
                .filter(|child| child.local() == "ser")
            {
                if seen >= source.series.len() {
                    return;
                }
                apply_series_style_snapshot(
                    ser,
                    &source.series[seen],
                    &chart_type_name,
                    chart_prefix,
                    drawing_prefix,
                );
                seen += 1;
                applied.push(format!("series:{seen}"));
            }
        }
    }
}

fn apply_series_style_snapshot(
    ser: &mut XmlNode,
    source: &SeriesStyleSnapshot,
    chart_type_name: &str,
    chart_prefix: &str,
    drawing_prefix: &str,
) {
    if source.fill_color.is_some() || source.line_color.is_some() || source.line_width_pt.is_some()
    {
        let sp_pr_index = ensure_child_index(
            ser,
            "spPr",
            XmlNode::new(qname(chart_prefix, "spPr")),
            SERIES_CHILD_ORDER,
        );
        let sp_pr = &mut ser.children[sp_pr_index];
        if let Some(fill) = source.fill_color.as_deref() {
            set_shape_fill_from_source(sp_pr, drawing_prefix, fill);
        }
        if source.line_color.is_some() || source.line_width_pt.is_some() {
            let line_index = ensure_child_index(
                sp_pr,
                "ln",
                XmlNode::new(qname(drawing_prefix, "ln")),
                SHAPE_PROPS_CHILD_ORDER,
            );
            let line = &mut sp_pr.children[line_index];
            if let Some(width) = source.line_width_pt {
                line.set_attr("w", &(width * 12700.0).round().to_string());
            }
            if let Some(line_color) = source.line_color.as_deref() {
                set_line_fill_from_source(line, drawing_prefix, line_color);
            }
        }
    }
    if (source.marker_symbol.is_some() || source.marker_size.is_some())
        && matches!(chart_type_name, "lineChart" | "scatterChart" | "radarChart")
    {
        let marker_index = ensure_child_index(
            ser,
            "marker",
            XmlNode::new(qname(chart_prefix, "marker")),
            SERIES_CHILD_ORDER,
        );
        let marker = &mut ser.children[marker_index];
        if let Some(symbol) = source.marker_symbol.as_deref() {
            set_or_create_val_child(marker, "symbol", symbol, MARKER_CHILD_ORDER, chart_prefix);
        }
        if let Some(size) = source.marker_size {
            set_or_create_val_child(
                marker,
                "size",
                &size.to_string(),
                MARKER_CHILD_ORDER,
                chart_prefix,
            );
        }
    }
}

fn title_tx_node(chart_xml: &ChartXml, text: &str, font: &FontOptions) -> XmlNode {
    title_tx_node_for_prefixes(
        &chart_xml.chart_prefix,
        &chart_xml.drawing_prefix,
        text,
        font,
    )
}

fn title_tx_node_for_prefixes(
    chart_prefix: &str,
    drawing_prefix: &str,
    text: &str,
    font: &FontOptions,
) -> XmlNode {
    let mut tx = XmlNode::new(qname(chart_prefix, "tx"));
    let mut rich = XmlNode::new(qname(chart_prefix, "rich"));
    rich.children
        .push(XmlNode::new(qname(drawing_prefix, "bodyPr")));
    rich.children
        .push(XmlNode::new(qname(drawing_prefix, "lstStyle")));
    let mut paragraph = XmlNode::new(qname(drawing_prefix, "p"));
    let mut run = XmlNode::new(qname(drawing_prefix, "r"));
    let mut run_properties = XmlNode::new(qname(drawing_prefix, "rPr"));
    apply_font_options_with_scheme(&mut run_properties, drawing_prefix, font);
    run.children.push(run_properties);
    let mut text_node = XmlNode::new(qname(drawing_prefix, "t"));
    text_node.text = text.to_string();
    run.children.push(text_node);
    paragraph.children.push(run);
    rich.children.push(paragraph);
    tx.children.push(rich);
    tx
}

fn apply_font_options(r_pr: &mut XmlNode, chart_xml: &ChartXml, font: &FontOptions) {
    apply_font_options_with_scheme(r_pr, &chart_xml.drawing_prefix, font)
}

fn apply_font_options_with_scheme(r_pr: &mut XmlNode, drawing_prefix: &str, font: &FontOptions) {
    if let Some(size_pt) = font.size_pt {
        r_pr.set_attr("sz", &(size_pt * 100.0).round().to_string());
    }
    if let Some(bold) = font.bold {
        r_pr.set_attr("b", bool_val(bold));
    }
    if let Some(italic) = font.italic {
        r_pr.set_attr("i", bool_val(italic));
    }
    if let Some(family) = font.family.as_deref() {
        let latin_index = ensure_child_index(
            r_pr,
            "latin",
            XmlNode::new(qname(drawing_prefix, "latin")),
            &[
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
            ],
        );
        r_pr.children[latin_index].set_attr("typeface", family);
    }
    if let Some(color) = font.color.as_deref() {
        set_shape_fill_from_source(r_pr, drawing_prefix, color);
    }
}

fn bool_val(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

fn parse_ooxml_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on"
    )
}

fn set_shape_fill(holder: &mut XmlNode, drawing_prefix: &str, fill: &str) {
    remove_fill_children(holder);
    let child = if fill.is_empty() {
        XmlNode::new(qname(drawing_prefix, "noFill"))
    } else {
        solid_fill_node(drawing_prefix, fill)
    };
    insert_child_in_order(holder, child, SHAPE_PROPS_CHILD_ORDER);
}

fn set_shape_fill_from_source(holder: &mut XmlNode, drawing_prefix: &str, fill: &str) {
    remove_fill_children(holder);
    insert_child_in_order(
        holder,
        fill_node_from_source(drawing_prefix, fill),
        SHAPE_PROPS_CHILD_ORDER,
    );
}

fn set_line_fill(line: &mut XmlNode, drawing_prefix: &str, fill: &str) {
    remove_fill_children(line);
    let child = if fill.is_empty() {
        XmlNode::new(qname(drawing_prefix, "noFill"))
    } else {
        solid_fill_node(drawing_prefix, fill)
    };
    insert_child_in_order(line, child, LINE_CHILD_ORDER);
}

fn set_line_fill_from_source(line: &mut XmlNode, drawing_prefix: &str, fill: &str) {
    remove_fill_children(line);
    insert_child_in_order(
        line,
        fill_node_from_source(drawing_prefix, fill),
        LINE_CHILD_ORDER,
    );
}

fn remove_fill_children(holder: &mut XmlNode) {
    holder.children.retain(|child| {
        !matches!(
            child.local(),
            "noFill" | "solidFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill"
        )
    });
}

fn solid_fill_node(drawing_prefix: &str, fill: &str) -> XmlNode {
    let mut solid = XmlNode::new(qname(drawing_prefix, "solidFill"));
    let mut srgb = XmlNode::new(qname(drawing_prefix, "srgbClr"));
    srgb.set_attr("val", fill);
    solid.children.push(srgb);
    solid
}

fn fill_node_from_source(drawing_prefix: &str, fill: &str) -> XmlNode {
    if fill.is_empty() {
        return XmlNode::new(qname(drawing_prefix, "noFill"));
    }
    let mut solid = XmlNode::new(qname(drawing_prefix, "solidFill"));
    if let Some(scheme) = fill.strip_prefix("scheme:") {
        let mut clr = XmlNode::new(qname(drawing_prefix, "schemeClr"));
        clr.set_attr("val", scheme);
        solid.children.push(clr);
    } else {
        let mut clr = XmlNode::new(qname(drawing_prefix, "srgbClr"));
        clr.set_attr("val", fill);
        solid.children.push(clr);
    }
    solid
}

fn title_text(title: &XmlNode) -> String {
    let mut parts = title
        .descendants("t")
        .into_iter()
        .map(|node| node.text.clone())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        parts = title
            .descendants("v")
            .into_iter()
            .map(|node| node.text.clone())
            .filter(|text| !text.is_empty())
            .collect();
    }
    parts.join("").trim().to_string()
}

fn inspect_fill(holder: &XmlNode) -> String {
    let Some(solid) = holder.direct_child("solidFill") else {
        return String::new();
    };
    if let Some(srgb) = solid.direct_child("srgbClr")
        && let Some(value) = srgb.attr("val")
    {
        return value.trim().to_ascii_uppercase();
    }
    if let Some(scheme) = solid.direct_child("schemeClr")
        && let Some(value) = scheme.attr("val")
    {
        return format!("scheme:{}", value.trim());
    }
    String::new()
}

fn series_count(root: &XmlNode) -> usize {
    root.first_descendant("plotArea")
        .map(|plot_area| {
            plot_area
                .children
                .iter()
                .filter(|child| child.local().ends_with("Chart"))
                .map(|chart_type| {
                    chart_type
                        .children
                        .iter()
                        .filter(|child| child.local() == "ser")
                        .count()
                })
                .sum()
        })
        .unwrap_or_default()
}

fn walk_series(root: &XmlNode) -> Vec<&XmlNode> {
    let mut series = Vec::new();
    if let Some(plot_area) = root.first_descendant("plotArea") {
        for chart_type in &plot_area.children {
            if chart_type.local().ends_with("Chart") {
                for ser in &chart_type.children {
                    if ser.local() == "ser" {
                        series.push(ser);
                    }
                }
            }
        }
    }
    series
}

fn ensure_child_index(
    parent: &mut XmlNode,
    local: &str,
    new_node: XmlNode,
    order: &[&str],
) -> usize {
    if let Some(index) = parent.direct_child_index(local) {
        return index;
    }
    let index = insertion_index(parent, local, order);
    parent.children.insert(index, new_node);
    index
}

fn insert_child_in_order(parent: &mut XmlNode, child: XmlNode, order: &[&str]) {
    let local = child.local().to_string();
    let index = insertion_index(parent, &local, order);
    parent.children.insert(index, child);
}

fn insertion_index(parent: &XmlNode, local: &str, order: &[&str]) -> usize {
    let Some(wanted) = order.iter().position(|candidate| *candidate == local) else {
        return parent.children.len();
    };
    for (index, child) in parent.children.iter().enumerate() {
        let child_order = order
            .iter()
            .position(|candidate| *candidate == child.local())
            .unwrap_or(order.len());
        if child_order > wanted {
            return index;
        }
    }
    parent.children.len()
}

fn chart_mutation_output_path(file: &str, options: &PptxChartMutationOptions) -> Option<String> {
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

fn stage_chart_mutation(
    file: &str,
    overrides: &BTreeMap<String, String>,
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
    copy_zip_with_part_overrides(file, &write_path, overrides)?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    Ok(write_path)
}

fn finish_chart_mutation(
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

fn chart_mutation_result_json(input: ChartMutationResultInput<'_>) -> Value {
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

fn parse_chart_xml(xml: &str) -> CliResult<ChartXml> {
    let root = parse_xml_tree(xml)?;
    if root.local() != "chartSpace" {
        return Err(CliError::unexpected("chart part root element not found"));
    }
    let chart_prefix = prefix_for_namespace(&root, CHART_NS)
        .or_else(|| prefix_from_name(&root.name))
        .unwrap_or_else(|| "c".to_string());
    let drawing_prefix = prefix_for_namespace(&root, DRAWING_NS).unwrap_or_else(|| "a".to_string());
    Ok(ChartXml {
        root,
        chart_prefix,
        drawing_prefix,
    })
}

fn parse_xml_tree(xml: &str) -> CliResult<XmlNode> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => stack.push(node_from_start(&e)),
            Ok(Event::Empty(e)) => {
                let node = node_from_start(&e);
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root = Some(node);
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if let Some(current) = stack.last_mut() {
                    current.text.push_str(&xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::End(_)) => {
                let Some(node) = stack.pop() else {
                    continue;
                };
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root = Some(node);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    root.ok_or_else(|| CliError::unexpected("XML root element not found"))
}

fn node_from_start(e: &BytesStart<'_>) -> XmlNode {
    XmlNode {
        name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
        attrs: xml_attrs_map(e),
        text: String::new(),
        children: Vec::new(),
    }
}

fn serialize_xml(root: &XmlNode) -> String {
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>");
    render_node(root, &mut output);
    output
}

fn render_node(node: &XmlNode, output: &mut String) {
    output.push('<');
    output.push_str(&node.name);
    for (key, value) in &node.attrs {
        output.push(' ');
        output.push_str(key);
        output.push_str("=\"");
        output.push_str(&xml_attr_escape(value));
        output.push('"');
    }
    if node.children.is_empty() && node.text.is_empty() {
        output.push_str("/>");
        return;
    }
    output.push('>');
    if !node.text.is_empty() {
        output.push_str(&xml_escape(&node.text));
    }
    for child in &node.children {
        render_node(child, output);
    }
    output.push_str("</");
    output.push_str(&node.name);
    output.push('>');
}

fn prefix_for_namespace(root: &XmlNode, namespace: &str) -> Option<String> {
    root.attrs.iter().find_map(|(key, value)| {
        if value != namespace {
            return None;
        }
        if key == "xmlns" {
            Some(String::new())
        } else {
            key.strip_prefix("xmlns:").map(ToString::to_string)
        }
    })
}

fn prefix_from_name(name: &str) -> Option<String> {
    name.split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .filter(|prefix| !prefix.is_empty())
}

fn qname(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

impl ChartXml {
    fn chart_name(&self, local: &str) -> String {
        qname(&self.chart_prefix, local)
    }

    fn drawing_name(&self, local: &str) -> String {
        qname(&self.drawing_prefix, local)
    }
}

impl XmlNode {
    fn new(name: String) -> Self {
        Self {
            name,
            attrs: BTreeMap::new(),
            text: String::new(),
            children: Vec::new(),
        }
    }

    fn local(&self) -> &str {
        local_name(self.name.as_bytes())
    }

    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(String::as_str).or_else(|| {
            self.attrs.iter().find_map(|(candidate, value)| {
                (local_name(candidate.as_bytes()) == key).then_some(value.as_str())
            })
        })
    }

    fn set_attr(&mut self, key: &str, value: &str) {
        if let Some(existing) = self
            .attrs
            .keys()
            .find(|candidate| local_name(candidate.as_bytes()) == key)
            .cloned()
        {
            self.attrs.insert(existing, value.to_string());
        } else {
            self.attrs.insert(key.to_string(), value.to_string());
        }
    }

    fn direct_child_index(&self, name: &str) -> Option<usize> {
        self.children.iter().position(|child| child.local() == name)
    }

    fn direct_child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|child| child.local() == name)
    }

    fn direct_child_mut(&mut self, name: &str) -> Option<&mut XmlNode> {
        self.children.iter_mut().find(|child| child.local() == name)
    }

    fn remove_direct_children(&mut self, name: &str) {
        self.children.retain(|child| child.local() != name);
    }

    fn first_descendant(&self, name: &str) -> Option<&XmlNode> {
        if self.local() == name {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.first_descendant(name) {
                return Some(found);
            }
        }
        None
    }

    fn first_descendant_mut(&mut self, name: &str) -> Option<&mut XmlNode> {
        if self.local() == name {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.first_descendant_mut(name) {
                return Some(found);
            }
        }
        None
    }

    fn descendants(&self, name: &str) -> Vec<&XmlNode> {
        let mut result = Vec::new();
        self.collect_descendants(name, &mut result);
        result
    }

    fn collect_descendants<'a>(&'a self, name: &str, result: &mut Vec<&'a XmlNode>) {
        if self.local() == name {
            result.push(self);
        }
        for child in &self.children {
            child.collect_descendants(name, result);
        }
    }
}
