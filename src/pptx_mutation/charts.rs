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
    slide: i64,
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

    let mut chart = selected_chart_json(&readback_path, selected.slide, &selected.part_selector())?;
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
        slide: selected.slide,
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
    let chart_slide = chart
        .get("slide")
        .and_then(Value::as_i64)
        .unwrap_or(slide.max(0));
    Ok(SelectedChart {
        slide: chart_slide,
        part_uri,
    })
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
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 0 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    Ok(slide)
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

fn title_tx_node(chart_xml: &ChartXml, text: &str, font: &FontOptions) -> XmlNode {
    let mut tx = XmlNode::new(chart_xml.chart_name("tx"));
    let mut rich = XmlNode::new(chart_xml.chart_name("rich"));
    rich.children
        .push(XmlNode::new(chart_xml.drawing_name("bodyPr")));
    rich.children
        .push(XmlNode::new(chart_xml.drawing_name("lstStyle")));
    let mut paragraph = XmlNode::new(chart_xml.drawing_name("p"));
    let mut run = XmlNode::new(chart_xml.drawing_name("r"));
    let mut run_properties = XmlNode::new(chart_xml.drawing_name("rPr"));
    apply_font_options(&mut run_properties, chart_xml, font);
    run.children.push(run_properties);
    let mut text_node = XmlNode::new(chart_xml.drawing_name("t"));
    text_node.text = text.to_string();
    run.children.push(text_node);
    paragraph.children.push(run);
    rich.children.push(paragraph);
    tx.children.push(rich);
    tx
}

fn apply_font_options(r_pr: &mut XmlNode, chart_xml: &ChartXml, font: &FontOptions) {
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
            XmlNode::new(chart_xml.drawing_name("latin")),
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
        set_shape_fill(r_pr, &chart_xml.drawing_prefix, color);
    }
}

fn bool_val(value: bool) -> &'static str {
    if value { "1" } else { "0" }
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

fn set_line_fill(line: &mut XmlNode, drawing_prefix: &str, fill: &str) {
    remove_fill_children(line);
    let child = if fill.is_empty() {
        XmlNode::new(qname(drawing_prefix, "noFill"))
    } else {
        solid_fill_node(drawing_prefix, fill)
    };
    insert_child_in_order(line, child, LINE_CHILD_ORDER);
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
    result.insert(
        format!("chartShowCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx charts show {} --slide {} --chart {}",
            command_arg(command_target),
            slide,
            command_arg(chart_selector)
        )),
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
