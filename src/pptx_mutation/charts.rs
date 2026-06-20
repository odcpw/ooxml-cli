use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::cli_args::{parse_bool_flag, value_flag_present};
use crate::pptx_readback::pptx_charts_show;
use crate::xml_util::attr_exact;
use crate::{
    CliError, CliResult, RangeBounds, XlsxRangeExportOptions, XlsxRangesSetOptions,
    chrono_like_counter, col_name, command_arg, copy_zip_with_binary_part_overrides_and_removals,
    decode_xml_text, ensure_content_type_override, local_name, package_mutation_temp_path,
    package_type, parse_cli_range, parse_i64_flag, parse_range, parse_string_flag,
    range_bounds_ref, relationship_entries, relationship_entries_from_xml,
    relationship_target_from_source_to_target, relationships, relationships_part_for, validate,
    validate_xlsx_mutation_output_flags, xlsx_range_export_with_options, xlsx_ranges_set,
    xml_attr_escape, xml_attrs_map, xml_escape, xml_general_ref, zip_bytes, zip_entry_names,
    zip_text,
};

const CHART_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const DRAWING_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const PRESENTATION_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const REL_CHART: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const REL_PACKAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package";
const CONTENT_TYPE_CHART: &str =
    "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
const CONTENT_TYPE_EMBEDDED_XLSX: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const EMU_PER_INCH: i64 = 914_400;
const CAT_AXIS_ID: i64 = 111_111_111;
const VAL_AXIS_ID: i64 = 222_222_222;

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

#[derive(Clone)]
struct ChartDataCell {
    kind: String,
    value: String,
    null: bool,
}

struct ChartCreateSource {
    mode: String,
    sheet: String,
    range: String,
    bounds: RangeBounds,
    cells: Vec<Vec<ChartDataCell>>,
    embedded_workbook: Vec<u8>,
    source_file: String,
}

struct CreateChartPartResult {
    xml: String,
    series_count: usize,
    categories: usize,
    warnings: Vec<String>,
}

struct CreateSlideChartResult {
    chart_uri: String,
    chart_relationship_id: String,
    shape_id: i64,
    shape_name: String,
    chart_type: String,
    title: String,
    series_count: usize,
    categories: usize,
    embedded_workbook_part_uri: String,
    warnings: Vec<String>,
}

struct StagedCreateSlideChart {
    result: CreateSlideChartResult,
    staged_path: String,
}

#[derive(Clone)]
struct ChartSeriesData {
    name: String,
    name_ref: String,
    categories: Vec<String>,
    category_ref: String,
    values: Vec<String>,
    value_ref: String,
}

struct ChartGeometry {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

struct CachePoint {
    index: usize,
    value: String,
}

#[derive(Clone, Default)]
struct SeriesSourceSnapshot {
    role: String,
    formula: String,
    sheet: String,
    range: String,
    ref_kind: String,
    cache_type: String,
    point_count: usize,
    values: Vec<String>,
}

struct SetSeriesSourceResult {
    cache_type: String,
    cache_point_count: usize,
    cache_preview: Vec<String>,
    warnings: Vec<String>,
}

struct UpdateInputRole {
    role: String,
    values: Vec<String>,
}

struct UpdatedRoleResult {
    role: String,
    snapshot: SeriesSourceSnapshot,
    previous_values_hash: String,
    mutation: SetSeriesSourceResult,
    embedded_workbook_range_updated: bool,
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

fn resolve_chart_create_source(args: &[String]) -> CliResult<ChartCreateSource> {
    let inline_json = parse_string_flag(args, "--values-json")?.unwrap_or_default();
    let inline_file = parse_string_flag(args, "--values-file")?.unwrap_or_default();
    let source_file = parse_string_flag(args, "--source-file")?.unwrap_or_default();
    let inline_count =
        usize::from(!inline_json.trim().is_empty()) + usize::from(!inline_file.trim().is_empty());
    if inline_count > 1 {
        return Err(CliError::invalid_args(
            "specify only one of --values-json or --values-file",
        ));
    }
    if inline_count == 1 && !source_file.trim().is_empty() {
        return Err(CliError::invalid_args(
            "specify either inline values or --source-file, not both",
        ));
    }
    let max_cells = parse_i64_flag(args, "--max-cells")?.unwrap_or(100000);
    if max_cells < 0 {
        return Err(CliError::invalid_args("--max-cells must be >= 0"));
    }

    if !source_file.trim().is_empty() {
        if !Path::new(&source_file).exists() {
            return Err(CliError::file_not_found(format!(
                "file not found: {source_file}"
            )));
        }
        let source_sheet = parse_string_flag(args, "--source-sheet")?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "1".to_string());
        let raw_range = parse_string_flag(args, "--source-range")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| CliError::invalid_args("--source-range is required"))?;
        let bounds = parse_cli_range(&raw_range)?.normalized();
        let source_range = range_bounds_ref_no_abs(bounds);
        let expect_range = parse_string_flag(args, "--expect-source-range")?.unwrap_or_default();
        if !expect_range.trim().is_empty() && !source_range.eq_ignore_ascii_case(&expect_range) {
            return Err(CliError::invalid_args(format!(
                "source range mismatch: expected {} but found {}",
                expect_range.trim(),
                source_range
            )));
        }
        let exported = xlsx_range_export_with_options(
            &source_file,
            &source_sheet,
            &source_range,
            XlsxRangeExportOptions {
                include_types: true,
                include_formulas: false,
                include_formats: false,
                data_out: None,
                max_cells,
            },
        )?;
        let cells = chart_cells_from_xlsx_export(&exported)?;
        let embedded_workbook = if crate::has_flag(args, "--embed-workbook") {
            fs::read(&source_file).map_err(|err| {
                CliError::unexpected(format!(
                    "failed to read source workbook for embedding: {err}"
                ))
            })?
        } else {
            Vec::new()
        };
        let sheet = exported
            .get("sheet")
            .and_then(Value::as_str)
            .unwrap_or(&source_sheet)
            .to_string();
        return Ok(ChartCreateSource {
            mode: "external".to_string(),
            sheet,
            range: source_range,
            bounds,
            cells,
            embedded_workbook,
            source_file,
        });
    }

    let raw = if !inline_file.trim().is_empty() {
        fs::read_to_string(&inline_file)
            .map_err(|err| CliError::invalid_args(format!("failed to read --values-file: {err}")))?
    } else if !inline_json.trim().is_empty() {
        inline_json
    } else {
        return Err(CliError::invalid_args(
            "must specify --values-json, --values-file, or --source-file",
        ));
    };
    let (cells, range) = parse_chart_inline_matrix(&raw, max_cells)?;
    let bounds = parse_cli_range(&range)?.normalized();
    Ok(ChartCreateSource {
        mode: "inline".to_string(),
        sheet: "Sheet1".to_string(),
        range,
        bounds,
        cells,
        embedded_workbook: Vec::new(),
        source_file: String::new(),
    })
}

fn parse_chart_inline_matrix(
    raw: &str,
    max_cells: i64,
) -> CliResult<(Vec<Vec<ChartDataCell>>, String)> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|err| CliError::invalid_args(format!("invalid --values JSON matrix: {err}")))?;
    let rows = value
        .as_array()
        .ok_or_else(|| CliError::invalid_args("invalid --values JSON matrix: json: cannot unmarshal non-array into Go value of type [][]interface {}"))?;
    if rows.is_empty() {
        return Err(CliError::invalid_args("inline values matrix is empty"));
    }
    let mut cols = 0usize;
    for row in rows {
        let row = row.as_array().ok_or_else(|| {
            CliError::invalid_args(
                "invalid --values JSON matrix: values must be an array of arrays",
            )
        })?;
        cols = cols.max(row.len());
    }
    if cols == 0 {
        return Err(CliError::invalid_args(
            "inline values matrix has no columns",
        ));
    }
    let cell_count = rows.len() * cols;
    if max_cells > 0 && cell_count > max_cells as usize {
        return Err(CliError::invalid_args(format!(
            "inline matrix has {cell_count} cells, exceeding --max-cells {max_cells}"
        )));
    }
    let mut matrix = Vec::with_capacity(rows.len());
    for row in rows {
        let row_values = row.as_array().expect("checked row array");
        let mut out_row = Vec::with_capacity(cols);
        for col_index in 0..cols {
            let cell = row_values
                .get(col_index)
                .map(chart_cell_from_json)
                .unwrap_or_else(|| ChartDataCell {
                    kind: String::new(),
                    value: String::new(),
                    null: true,
                });
            out_row.push(cell);
        }
        matrix.push(out_row);
    }
    let end_col = col_name(cols as u32);
    Ok((matrix, format!("A1:{end_col}{}", rows.len())))
}

fn chart_cell_from_json(value: &Value) -> ChartDataCell {
    if value.is_null() {
        return ChartDataCell {
            kind: String::new(),
            value: String::new(),
            null: true,
        };
    }
    if let Some(number) = value.as_number() {
        return ChartDataCell {
            kind: "number".to_string(),
            value: number.to_string(),
            null: false,
        };
    }
    if let Some(boolean) = value.as_bool() {
        return ChartDataCell {
            kind: "boolean".to_string(),
            value: if boolean { "1" } else { "0" }.to_string(),
            null: false,
        };
    }
    if let Some(text) = value.as_str() {
        return ChartDataCell {
            kind: "string".to_string(),
            value: text.to_string(),
            null: false,
        };
    }
    ChartDataCell {
        kind: "string".to_string(),
        value: value.to_string(),
        null: false,
    }
}

fn chart_cells_from_xlsx_export(exported: &Value) -> CliResult<Vec<Vec<ChartDataCell>>> {
    let values = exported
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::unexpected("xlsx range export missing values"))?;
    let types = exported.get("types").and_then(Value::as_array);
    let mut rows = Vec::with_capacity(values.len());
    for (row_index, row) in values.iter().enumerate() {
        let row_values = row
            .as_array()
            .ok_or_else(|| CliError::unexpected("xlsx range values must be rows"))?;
        let row_types = types
            .and_then(|items| items.get(row_index))
            .and_then(Value::as_array);
        let mut out_row = Vec::with_capacity(row_values.len());
        for (col_index, value) in row_values.iter().enumerate() {
            let kind = row_types
                .and_then(|items| items.get(col_index))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let mut cell = chart_cell_from_json(value);
            if kind == "number" || kind == "boolean" || kind == "string" {
                cell.kind = kind.to_string();
            }
            out_row.push(cell);
        }
        rows.push(out_row);
    }
    Ok(rows)
}

fn resolve_chart_create_geometry(file: &str, args: &[String]) -> CliResult<ChartGeometry> {
    let (slide_cx, slide_cy) = presentation_slide_size(file)?;
    let mut cx = parse_i64_flag(args, "--cx")?.unwrap_or(0);
    let mut cy = parse_i64_flag(args, "--cy")?.unwrap_or(0);
    if cx <= 0 {
        cx = slide_cx / 2;
    }
    if cy <= 0 {
        cy = slide_cy / 2;
    }
    let mut x = parse_i64_flag(args, "--x")?.unwrap_or(0);
    let mut y = parse_i64_flag(args, "--y")?.unwrap_or(0);
    if !value_flag_present(args, "--x") {
        x = ((slide_cx - cx) / 2).max(0);
    }
    if !value_flag_present(args, "--y") {
        y = ((slide_cy - cy) / 2).max(0);
    }
    Ok(ChartGeometry { x, y, cx, cy })
}

fn presentation_slide_size(file: &str) -> CliResult<(i64, i64)> {
    let xml = zip_text(file, "ppt/presentation.xml")?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldSz" =>
            {
                let cx = e
                    .attributes()
                    .flatten()
                    .find_map(|attr| {
                        (local_name(attr.key.as_ref()) == "cx")
                            .then(|| decode_xml_text(attr.value.as_ref()))
                    })
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(10 * EMU_PER_INCH);
                let cy = e
                    .attributes()
                    .flatten()
                    .find_map(|attr| {
                        (local_name(attr.key.as_ref()) == "cy")
                            .then(|| decode_xml_text(attr.value.as_ref()))
                    })
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(EMU_PER_INCH * 15 / 2);
                return Ok((cx, cy));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok((10 * EMU_PER_INCH, EMU_PER_INCH * 15 / 2))
}

fn range_bounds_ref_no_abs(bounds: RangeBounds) -> String {
    let bounds = bounds.normalized();
    let start = format!("{}{}", col_name(bounds.start_col), bounds.start_row);
    let end = format!("{}{}", col_name(bounds.end_col), bounds.end_row);
    if start == end {
        start
    } else {
        format!("{start}:{end}")
    }
}

fn create_slide_chart_package_updates(
    file: &str,
    slide: usize,
    chart_type: &str,
    title: &str,
    source: &ChartCreateSource,
    geometry: &ChartGeometry,
    options: &PptxChartMutationOptions,
) -> CliResult<StagedCreateSlideChart> {
    let slide_ref = slide_part_for_number(file, slide)?;
    let chart_part = allocate_numbered_part(file, "/ppt/charts/chart", ".xml")?;
    let chart = build_chart_part(
        chart_type,
        title,
        &source.sheet,
        source.bounds,
        &source.cells,
    )
    .map_err(|err| CliError::invalid_args(format!("failed to create chart: {}", err.message)))?;

    let mut text_overrides = BTreeMap::new();
    let mut binary_overrides = BTreeMap::new();
    let mut content_types = zip_text(file, "[Content_Types].xml")?;
    content_types = ensure_content_type_override(
        content_types,
        chart_part.trim_start_matches('/'),
        CONTENT_TYPE_CHART,
    );
    let mut chart_xml = chart.xml;
    let mut embedded_part = String::new();
    if !source.embedded_workbook.is_empty() {
        embedded_part =
            allocate_numbered_part(file, "/ppt/embeddings/Microsoft_Excel_Sheet", ".xlsx")?;
        content_types = ensure_content_type_override(
            content_types,
            embedded_part.trim_start_matches('/'),
            CONTENT_TYPE_EMBEDDED_XLSX,
        );
        binary_overrides.insert(
            embedded_part.trim_start_matches('/').to_string(),
            source.embedded_workbook.clone(),
        );
        let chart_rels_part = relationships_part_for(&chart_part);
        let chart_rels_xml = zip_text(file, &chart_rels_part).unwrap_or_else(|_| {
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
        });
        let chart_rels = relationship_entries_from_xml(&chart_rels_xml);
        let package_rid = crate::allocate_relationship_id(&chart_rels);
        let target = relationship_target_from_source_to_target(&chart_part, &embedded_part);
        text_overrides.insert(
            chart_rels_part,
            crate::add_relationship_to_xml(chart_rels_xml, &package_rid, REL_PACKAGE, &target),
        );
        chart_xml = add_chart_external_data(&chart_xml, &package_rid)?;
    }
    text_overrides.insert(chart_part.trim_start_matches('/').to_string(), chart_xml);

    let slide_rels_part = relationships_part_for(&slide_ref.part_uri);
    let slide_rels_xml = zip_text(file, &slide_rels_part)?;
    let mut slide_rels = relationship_entries(file, &slide_rels_part)?;
    let chart_rid = crate::allocate_relationship_id(&slide_rels);
    let chart_target = relationship_target_from_source_to_target(&slide_ref.part_uri, &chart_part);
    slide_rels.push(crate::RelationshipEntry {
        id: chart_rid.clone(),
        rel_type: REL_CHART.to_string(),
        target: chart_target.clone(),
        target_mode: String::new(),
    });
    text_overrides.insert(
        slide_rels_part,
        crate::add_relationship_to_xml(slide_rels_xml, &chart_rid, REL_CHART, &chart_target),
    );

    let slide_part_name = slide_ref.part_uri.trim_start_matches('/').to_string();
    let slide_xml = zip_text(file, &slide_part_name)?;
    let (updated_slide_xml, shape_id, shape_name) =
        add_chart_graphic_frame_to_slide(&slide_xml, &chart_rid, geometry)?;
    text_overrides.insert(slide_part_name, updated_slide_xml);
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);

    let staged_path =
        stage_chart_package_mutation(file, &text_overrides, &binary_overrides, options)?;
    Ok(StagedCreateSlideChart {
        staged_path,
        result: CreateSlideChartResult {
            chart_uri: chart_part,
            chart_relationship_id: chart_rid,
            shape_id,
            shape_name,
            chart_type: chart_type.to_string(),
            title: title.to_string(),
            series_count: chart.series_count,
            categories: chart.categories,
            embedded_workbook_part_uri: embedded_part,
            warnings: chart.warnings,
        },
    })
}

struct SlidePartRef {
    part_uri: String,
}

fn slide_part_for_number(file: &str, slide: usize) -> CliResult<SlidePartRef> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = presentation_slide_refs(&presentation);
    if slide == 0 || slide > slides.len() {
        return Err(CliError::target_not_found(format!(
            "slide {slide} not found (presentation has {} slides)",
            slides.len()
        )));
    }
    let rel_id = &slides[slide - 1].1;
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    Ok(SlidePartRef {
        part_uri: format!("/{}", normalize_ppt_target(target)),
    })
}

fn presentation_slide_refs(xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let (Some(id), Some(rel)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    slides.push((id, rel));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

fn normalize_ppt_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("ppt/") {
        target.to_string()
    } else {
        format!("ppt/{}", target.trim_start_matches("../"))
    }
}

fn allocate_numbered_part(file: &str, prefix: &str, suffix: &str) -> CliResult<String> {
    let mut max_number = 0usize;
    for entry in zip_entry_names(file)? {
        let uri = format!("/{}", entry.trim_start_matches('/'));
        if !uri.starts_with(prefix) || !uri.ends_with(suffix) {
            continue;
        }
        let middle = uri
            .trim_start_matches(prefix)
            .trim_end_matches(suffix)
            .trim();
        if let Ok(number) = middle.parse::<usize>() {
            max_number = max_number.max(number);
        }
    }
    Ok(format!("{prefix}{}{suffix}", max_number + 1))
}

fn add_chart_external_data(xml: &str, rid: &str) -> CliResult<String> {
    let mut chart_xml = parse_chart_xml(xml)?;
    chart_xml
        .root
        .attrs
        .entry("xmlns:r".to_string())
        .or_insert_with(|| {
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships".to_string()
        });
    let mut external = XmlNode::new(chart_xml.chart_name("externalData"));
    external.set_attr("r:id", rid);
    let mut auto_update = XmlNode::new(chart_xml.chart_name("autoUpdate"));
    auto_update.set_attr("val", "0");
    external.children.push(auto_update);
    chart_xml.root.children.push(external);
    Ok(serialize_xml(&chart_xml.root))
}

fn add_chart_graphic_frame_to_slide(
    slide_xml: &str,
    chart_rid: &str,
    geometry: &ChartGeometry,
) -> CliResult<(String, i64, String)> {
    let mut root = parse_xml_tree(slide_xml)?;
    let p_prefix = prefix_for_namespace(&root, PRESENTATION_NS).unwrap_or_else(|| "p".to_string());
    let a_prefix = prefix_for_namespace(&root, DRAWING_NS).unwrap_or_else(|| "a".to_string());
    let shape_id = root
        .first_descendant("spTree")
        .map(next_sp_tree_shape_id)
        .ok_or_else(|| CliError::unexpected("shape tree not found in slide"))?;
    let shape_name = format!("Chart {shape_id}");
    let frame = build_chart_graphic_frame(
        &p_prefix,
        &a_prefix,
        shape_id,
        &shape_name,
        chart_rid,
        geometry,
    );
    let sp_tree = root
        .first_descendant_mut("spTree")
        .ok_or_else(|| CliError::unexpected("shape tree not found in slide"))?;
    sp_tree.children.push(frame);
    Ok((serialize_xml(&root), shape_id, shape_name))
}

fn next_sp_tree_shape_id(sp_tree: &XmlNode) -> i64 {
    sp_tree
        .descendants("cNvPr")
        .into_iter()
        .filter_map(|node| node.attr("id"))
        .filter_map(|id| id.trim().parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn build_chart_graphic_frame(
    p_prefix: &str,
    a_prefix: &str,
    shape_id: i64,
    shape_name: &str,
    chart_rid: &str,
    geometry: &ChartGeometry,
) -> XmlNode {
    let mut frame = XmlNode::new(qname(p_prefix, "graphicFrame"));
    let mut nv = XmlNode::new(qname(p_prefix, "nvGraphicFramePr"));
    let mut c_nv_pr = XmlNode::new(qname(p_prefix, "cNvPr"));
    c_nv_pr.set_attr("id", &shape_id.to_string());
    c_nv_pr.set_attr("name", shape_name);
    nv.children.push(c_nv_pr);
    nv.children
        .push(XmlNode::new(qname(p_prefix, "cNvGraphicFramePr")));
    nv.children.push(XmlNode::new(qname(p_prefix, "nvPr")));
    frame.children.push(nv);

    let mut xfrm = XmlNode::new(qname(p_prefix, "xfrm"));
    let mut off = XmlNode::new(qname(a_prefix, "off"));
    off.set_attr("x", &geometry.x.to_string());
    off.set_attr("y", &geometry.y.to_string());
    let mut ext = XmlNode::new(qname(a_prefix, "ext"));
    ext.set_attr("cx", &geometry.cx.to_string());
    ext.set_attr("cy", &geometry.cy.to_string());
    xfrm.children.push(off);
    xfrm.children.push(ext);
    frame.children.push(xfrm);

    let mut graphic = XmlNode::new(qname(a_prefix, "graphic"));
    let mut graphic_data = XmlNode::new(qname(a_prefix, "graphicData"));
    graphic_data.set_attr("uri", CHART_NS);
    let mut chart = XmlNode::new("c:chart".to_string());
    chart.set_attr("xmlns:c", CHART_NS);
    chart.set_attr(
        "xmlns:r",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
    );
    chart.set_attr("r:id", chart_rid);
    graphic_data.children.push(chart);
    graphic.children.push(graphic_data);
    frame.children.push(graphic);
    frame
}

fn build_chart_part(
    chart_type: &str,
    title: &str,
    source_sheet: &str,
    source_range: RangeBounds,
    cells: &[Vec<ChartDataCell>],
) -> CliResult<CreateChartPartResult> {
    if !matches!(chart_type, "bar" | "line" | "area" | "pie" | "scatter") {
        return Err(CliError::invalid_args(format!(
            "invalid chart type {chart_type:?} (bar, line, area, pie, scatter)"
        )));
    }
    let (mut series, categories, mut warnings) =
        build_chart_series(source_sheet, source_range, cells)?;
    if series.is_empty() {
        return Err(CliError::invalid_args(
            "source range produced no chart series",
        ));
    }
    if chart_type == "pie" && series.len() > 1 {
        series.truncate(1);
        warnings.push("pie chart uses only the first series".to_string());
    }
    let root = build_chart_part_xml(chart_type, title, &series);
    Ok(CreateChartPartResult {
        xml: serialize_xml(&root),
        series_count: series.len(),
        categories,
        warnings,
    })
}

fn build_chart_series(
    source_sheet: &str,
    source_range: RangeBounds,
    cells: &[Vec<ChartDataCell>],
) -> CliResult<(Vec<ChartSeriesData>, usize, Vec<String>)> {
    if cells.is_empty() {
        return Err(CliError::invalid_args("source range is empty"));
    }
    let bounds = source_range.normalized();
    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let has_header = rows > 1;
    let data_start_row = if has_header {
        bounds.start_row + 1
    } else {
        bounds.start_row
    };
    if bounds.end_row < data_start_row {
        return Err(CliError::invalid_args("source range has no data rows"));
    }

    let cell_at = |row: u32, col: u32| -> ChartDataCell {
        let row_index = row.saturating_sub(bounds.start_row) as usize;
        let col_index = col.saturating_sub(bounds.start_col) as usize;
        cells
            .get(row_index)
            .and_then(|row| row.get(col_index))
            .cloned()
            .unwrap_or_else(|| ChartDataCell {
                kind: String::new(),
                value: String::new(),
                null: true,
            })
    };
    let text =
        |cell: ChartDataCell| -> String { if cell.null { String::new() } else { cell.value } };

    let has_categories = cols > 1;
    let mut categories = Vec::new();
    if has_categories {
        for row in data_start_row..=bounds.end_row {
            categories.push(text(cell_at(row, bounds.start_col)));
        }
    }
    let category_ref = abs_ref(
        source_sheet,
        bounds.start_col,
        data_start_row,
        bounds.start_col,
        bounds.end_row,
    );

    let first_series_col = if has_categories {
        bounds.start_col + 1
    } else {
        bounds.start_col
    };
    let mut coerced = 0usize;
    let mut series = Vec::new();
    for col in first_series_col..=bounds.end_col {
        let mut item = ChartSeriesData {
            name: String::new(),
            name_ref: String::new(),
            categories: categories.clone(),
            category_ref: category_ref.clone(),
            values: Vec::new(),
            value_ref: String::new(),
        };
        if has_header {
            item.name = text(cell_at(bounds.start_row, col));
            item.name_ref = abs_ref(source_sheet, col, bounds.start_row, col, bounds.start_row);
        }
        for row in data_start_row..=bounds.end_row {
            let (value, was_coerced) = numeric_text_coerced(&cell_at(row, col));
            if was_coerced {
                coerced += 1;
            }
            item.values.push(value);
        }
        item.value_ref = abs_ref(source_sheet, col, data_start_row, col, bounds.end_row);
        series.push(item);
    }

    let mut warnings = Vec::new();
    if !has_categories {
        warnings.push("single-column source: no categories axis".to_string());
    }
    if coerced > 0 {
        warnings.push(format!("{coerced} non-numeric value(s) treated as 0"));
    }
    Ok((series, categories.len(), warnings))
}

fn numeric_text_coerced(cell: &ChartDataCell) -> (String, bool) {
    if cell.null || cell.value.is_empty() {
        return ("0".to_string(), false);
    }
    if cell.value.parse::<f64>().is_ok() {
        return (cell.value.clone(), false);
    }
    ("0".to_string(), true)
}

fn abs_ref(sheet: &str, col1: u32, row1: u32, col2: u32, row2: u32) -> String {
    let start = format!("${}${row1}", col_name(col1));
    let end = format!("${}${row2}", col_name(col2));
    let quoted = quote_chart_sheet(sheet);
    if col1 == col2 && row1 == row2 {
        format!("{quoted}!{start}")
    } else {
        format!("{quoted}!{start}:{end}")
    }
}

fn quote_chart_sheet(sheet: &str) -> String {
    format!("'{}'", sheet.replace('\'', "''"))
}

fn chart_cel(local: &str) -> XmlNode {
    XmlNode::new(qname("c", local))
}

fn chart_cel_val(local: &str, value: impl ToString) -> XmlNode {
    let mut node = chart_cel(local);
    node.set_attr("val", &value.to_string());
    node
}

fn drawing_cel(local: &str) -> XmlNode {
    XmlNode::new(qname("a", local))
}

fn build_chart_part_xml(chart_type: &str, title: &str, series: &[ChartSeriesData]) -> XmlNode {
    let mut root = chart_cel("chartSpace");
    root.set_attr("xmlns:c", CHART_NS);
    root.set_attr("xmlns:a", DRAWING_NS);
    root.set_attr(
        "xmlns:r",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
    );

    let mut chart = chart_cel("chart");
    if !title.trim().is_empty() {
        chart.children.push(build_chart_title_node(title));
        chart.children.push(chart_cel_val("autoTitleDeleted", "0"));
    } else {
        chart.children.push(chart_cel_val("autoTitleDeleted", "1"));
    }

    let mut plot_area = chart_cel("plotArea");
    plot_area.children.push(chart_cel("layout"));
    plot_area.children.push(build_plot_node(chart_type, series));
    if chart_type != "pie" {
        plot_area.children.push(build_cat_axis(chart_type));
        plot_area.children.push(build_val_axis());
    }
    chart.children.push(plot_area);
    chart.children.push(chart_cel_val("plotVisOnly", "1"));
    chart.children.push(chart_cel_val("dispBlanksAs", "gap"));
    root.children.push(chart);
    root
}

fn build_chart_title_node(title: &str) -> XmlNode {
    let mut title_node = chart_cel("title");
    let mut tx = chart_cel("tx");
    let mut rich = chart_cel("rich");
    rich.children.push(drawing_cel("bodyPr"));
    rich.children.push(drawing_cel("lstStyle"));
    let mut paragraph = drawing_cel("p");
    let mut run = drawing_cel("r");
    let mut text = drawing_cel("t");
    text.text = title.to_string();
    run.children.push(text);
    paragraph.children.push(run);
    rich.children.push(paragraph);
    tx.children.push(rich);
    title_node.children.push(tx);
    title_node.children.push(chart_cel_val("overlay", "0"));
    title_node
}

fn build_plot_node(chart_type: &str, series: &[ChartSeriesData]) -> XmlNode {
    match chart_type {
        "bar" => {
            let mut plot = chart_cel("barChart");
            plot.children.push(chart_cel_val("barDir", "col"));
            plot.children.push(chart_cel_val("grouping", "clustered"));
            plot.children.push(chart_cel_val("varyColors", "0"));
            for (idx, item) in series.iter().enumerate() {
                plot.children.push(build_category_series(idx, item));
            }
            plot.children.push(chart_cel_val("axId", CAT_AXIS_ID));
            plot.children.push(chart_cel_val("axId", VAL_AXIS_ID));
            plot
        }
        "line" => {
            let mut plot = chart_cel("lineChart");
            plot.children.push(chart_cel_val("grouping", "standard"));
            plot.children.push(chart_cel_val("varyColors", "0"));
            for (idx, item) in series.iter().enumerate() {
                plot.children.push(build_category_series(idx, item));
            }
            plot.children.push(chart_cel_val("marker", "1"));
            plot.children.push(chart_cel_val("axId", CAT_AXIS_ID));
            plot.children.push(chart_cel_val("axId", VAL_AXIS_ID));
            plot
        }
        "area" => {
            let mut plot = chart_cel("areaChart");
            plot.children.push(chart_cel_val("grouping", "standard"));
            plot.children.push(chart_cel_val("varyColors", "0"));
            for (idx, item) in series.iter().enumerate() {
                plot.children.push(build_category_series(idx, item));
            }
            plot.children.push(chart_cel_val("axId", CAT_AXIS_ID));
            plot.children.push(chart_cel_val("axId", VAL_AXIS_ID));
            plot
        }
        "pie" => {
            let mut plot = chart_cel("pieChart");
            plot.children.push(chart_cel_val("varyColors", "1"));
            for (idx, item) in series.iter().enumerate() {
                plot.children.push(build_category_series(idx, item));
            }
            plot.children.push(chart_cel_val("firstSliceAng", "0"));
            plot
        }
        "scatter" => {
            let mut plot = chart_cel("scatterChart");
            plot.children
                .push(chart_cel_val("scatterStyle", "lineMarker"));
            plot.children.push(chart_cel_val("varyColors", "0"));
            for (idx, item) in series.iter().enumerate() {
                plot.children.push(build_scatter_series(idx, item));
            }
            plot.children.push(chart_cel_val("axId", CAT_AXIS_ID));
            plot.children.push(chart_cel_val("axId", VAL_AXIS_ID));
            plot
        }
        _ => chart_cel("barChart"),
    }
}

fn build_series_header(idx: usize, series: &ChartSeriesData) -> XmlNode {
    let mut ser = chart_cel("ser");
    ser.children.push(chart_cel_val("idx", idx));
    ser.children.push(chart_cel_val("order", idx));
    if !series.name_ref.is_empty() {
        let mut tx = chart_cel("tx");
        tx.children.push(build_str_ref(
            &series.name_ref,
            std::slice::from_ref(&series.name),
        ));
        ser.children.push(tx);
    }
    ser
}

fn build_category_series(idx: usize, series: &ChartSeriesData) -> XmlNode {
    let mut ser = build_series_header(idx, series);
    if !series.category_ref.is_empty() && !series.categories.is_empty() {
        let mut cat = chart_cel("cat");
        cat.children
            .push(build_str_ref(&series.category_ref, &series.categories));
        ser.children.push(cat);
    }
    let mut val = chart_cel("val");
    val.children
        .push(build_num_ref(&series.value_ref, &series.values));
    ser.children.push(val);
    ser
}

fn build_scatter_series(idx: usize, series: &ChartSeriesData) -> XmlNode {
    let mut ser = build_series_header(idx, series);
    let mut x_val = chart_cel("xVal");
    if !series.category_ref.is_empty() && !series.categories.is_empty() {
        x_val.children.push(build_num_ref(
            &series.category_ref,
            &numeric_axis(&series.categories),
        ));
    } else {
        x_val
            .children
            .push(build_num_ref(&series.value_ref, &series.values));
    }
    ser.children.push(x_val);
    let mut y_val = chart_cel("yVal");
    y_val
        .children
        .push(build_num_ref(&series.value_ref, &series.values));
    ser.children.push(y_val);
    ser
}

fn numeric_axis(values: &[String]) -> Vec<String> {
    values
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            if !value.trim().is_empty() && value.trim().parse::<f64>().is_ok() {
                value.clone()
            } else {
                (idx + 1).to_string()
            }
        })
        .collect()
}

fn build_str_ref(reference: &str, values: &[String]) -> XmlNode {
    let mut str_ref = chart_cel("strRef");
    let mut formula = chart_cel("f");
    formula.text = reference.to_string();
    str_ref.children.push(formula);
    let mut cache = chart_cel("strCache");
    cache.children.push(chart_cel_val("ptCount", values.len()));
    for (idx, value) in values.iter().enumerate() {
        cache.children.push(build_cache_point(idx, value));
    }
    str_ref.children.push(cache);
    str_ref
}

fn build_num_ref(reference: &str, values: &[String]) -> XmlNode {
    let mut num_ref = chart_cel("numRef");
    let mut formula = chart_cel("f");
    formula.text = reference.to_string();
    num_ref.children.push(formula);
    num_ref
        .children
        .push(build_cache_element("numCache", values, Some("General")));
    num_ref
}

fn build_cache_element(cache_type: &str, values: &[String], format_code: Option<&str>) -> XmlNode {
    let mut cache = chart_cel(cache_type);
    if cache_type == "numCache" {
        let mut format = chart_cel("formatCode");
        format.text = format_code.unwrap_or("General").to_string();
        cache.children.push(format);
    }
    cache.children.push(chart_cel_val("ptCount", values.len()));
    for (idx, value) in values.iter().enumerate() {
        cache.children.push(build_cache_point(idx, value));
    }
    cache
}

fn build_cache_point(idx: usize, value: &str) -> XmlNode {
    let mut point = chart_cel("pt");
    point.set_attr("idx", &idx.to_string());
    let mut v = chart_cel("v");
    v.text = value.to_string();
    point.children.push(v);
    point
}

fn build_cat_axis(chart_type: &str) -> XmlNode {
    let mut axis = if chart_type == "scatter" {
        chart_cel("valAx")
    } else {
        chart_cel("catAx")
    };
    axis.children.push(chart_cel_val("axId", CAT_AXIS_ID));
    let mut scaling = chart_cel("scaling");
    scaling
        .children
        .push(chart_cel_val("orientation", "minMax"));
    axis.children.push(scaling);
    axis.children.push(chart_cel_val("delete", "0"));
    axis.children.push(chart_cel_val("axPos", "b"));
    axis.children.push(chart_cel_val("crossAx", VAL_AXIS_ID));
    axis
}

fn build_val_axis() -> XmlNode {
    let mut axis = chart_cel("valAx");
    axis.children.push(chart_cel_val("axId", VAL_AXIS_ID));
    let mut scaling = chart_cel("scaling");
    scaling
        .children
        .push(chart_cel_val("orientation", "minMax"));
    axis.children.push(scaling);
    axis.children.push(chart_cel_val("delete", "0"));
    axis.children.push(chart_cel_val("axPos", "l"));
    axis.children.push(chart_cel_val("crossAx", CAT_AXIS_ID));
    axis
}

#[derive(Clone, Copy)]
struct ChartSourceRole {
    canonical: &'static str,
    element: &'static str,
}

fn resolve_update_input_roles(args: &[String]) -> CliResult<Vec<UpdateInputRole>> {
    let (values, values_changed) = resolve_update_input_values(
        parse_string_flag(args, "--values")?,
        parse_string_flag(args, "--values-json")?,
    )?;
    let (categories, categories_changed) = resolve_update_input_values(
        parse_string_flag(args, "--categories")?,
        parse_string_flag(args, "--categories-json")?,
    )?;
    let mut roles = Vec::new();
    if values_changed {
        roles.push(UpdateInputRole {
            role: "values".to_string(),
            values,
        });
    }
    if categories_changed {
        roles.push(UpdateInputRole {
            role: "categories".to_string(),
            values: categories,
        });
    }
    Ok(roles)
}

fn resolve_update_input_values(
    csv_values: Option<String>,
    json_values: Option<String>,
) -> CliResult<(Vec<String>, bool)> {
    if let Some(raw) = json_values.filter(|value| !value.trim().is_empty()) {
        let value: Value = serde_json::from_str(&raw)
            .map_err(|err| CliError::invalid_args(format!("invalid JSON values array: {err}")))?;
        let values = value
            .as_array()
            .ok_or_else(|| {
                CliError::invalid_args("invalid JSON values array: values must be an array")
            })?
            .iter()
            .map(|item| {
                item.as_str()
                    .map(|value| value.trim().to_string())
                    .ok_or_else(|| {
                        CliError::invalid_args("invalid JSON values array: values must be strings")
                    })
            })
            .collect::<CliResult<Vec<_>>>()?;
        return Ok((values, true));
    }
    if let Some(raw) = csv_values.filter(|value| !value.trim().is_empty()) {
        let values = parse_single_csv_record(&raw)
            .map_err(|message| {
                CliError::invalid_args(format!("invalid comma-separated values: {message}"))
            })?
            .into_iter()
            .map(|value| value.trim().to_string())
            .collect();
        return Ok((values, true));
    }
    Ok((Vec::new(), false))
}

fn parse_single_csv_record(data: &str) -> Result<Vec<String>, String> {
    let mut record = Vec::new();
    let mut field = String::new();
    let mut chars = data.chars().peekable();
    let mut in_quotes = false;
    let mut field_started = false;
    let mut just_closed_quote = false;

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                    just_closed_quote = true;
                }
            } else {
                field.push(ch);
            }
            continue;
        }

        if ch == '"' {
            if !field_started {
                in_quotes = true;
                field_started = true;
                continue;
            }
            return Err("parse error on line 1, column 1: bare \" in non-quoted-field".to_string());
        }
        if ch == ',' {
            record.push(std::mem::take(&mut field));
            field_started = false;
            just_closed_quote = false;
            continue;
        }
        if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            record.push(std::mem::take(&mut field));
            return Ok(record);
        }
        if just_closed_quote {
            return Err(
                "parse error on line 1, column 1: extraneous or missing \" in quoted-field"
                    .to_string(),
            );
        }
        field_started = true;
        field.push(ch);
    }

    if in_quotes {
        return Err(
            "parse error on line 1, column 1: extraneous or missing \" in quoted-field".to_string(),
        );
    }
    record.push(field);
    Ok(record)
}

fn chart_source_role_for(value: &str) -> Option<ChartSourceRole> {
    match value.trim().to_ascii_lowercase().as_str() {
        "name" | "tx" | "series-name" | "seriesname" => Some(ChartSourceRole {
            canonical: "name",
            element: "tx",
        }),
        "categories" | "category" | "cat" | "cats" => Some(ChartSourceRole {
            canonical: "categories",
            element: "cat",
        }),
        "values" | "value" | "val" | "vals" => Some(ChartSourceRole {
            canonical: "values",
            element: "val",
        }),
        "xvalues" | "x" | "xval" | "x-val" | "x-values" => Some(ChartSourceRole {
            canonical: "xValues",
            element: "xVal",
        }),
        "yvalues" | "y" | "yval" | "y-val" | "y-values" => Some(ChartSourceRole {
            canonical: "yValues",
            element: "yVal",
        }),
        "bubblesize" | "bubble" | "bubble-size" => Some(ChartSourceRole {
            canonical: "bubbleSize",
            element: "bubbleSize",
        }),
        _ => None,
    }
}

fn read_series_source(
    chart_xml: &ChartXml,
    series_number: usize,
    role_name: &str,
) -> CliResult<SeriesSourceSnapshot> {
    let role = chart_source_role_for(role_name).ok_or_else(|| {
        CliError::invalid_args(format!(
            "invalid chart source role {role_name:?} (must be name, categories, values, xValues, yValues, or bubbleSize)"
        ))
    })?;
    let series = walk_series(&chart_xml.root);
    if series_number == 0 || series_number > series.len() {
        return Err(CliError::invalid_args(format!(
            "series {series_number} is out of range (1-{})",
            series.len()
        )));
    }
    let ser = series[series_number - 1];
    let role_elem = ser.direct_child(role.element).ok_or_else(|| {
        CliError::invalid_args(format!(
            "series {series_number} has no {} source (available roles: {})",
            role.canonical,
            series_roles(ser).join(", ")
        ))
    })?;
    let (source_ref, ref_kind) = source_ref_child(role_elem)?;
    let mut snapshot = SeriesSourceSnapshot {
        role: role.canonical.to_string(),
        ref_kind,
        ..SeriesSourceSnapshot::default()
    };
    if let Some(formula) = source_ref.direct_child("f") {
        snapshot.formula = normalize_formula_text(&formula.text);
        if let Some((sheet, range)) = parse_local_range_formula(&snapshot.formula) {
            snapshot.sheet = sheet;
            snapshot.range = range;
        }
    }
    if let Some(cache) = first_cache_child(source_ref) {
        snapshot.cache_type = cache.local().to_string();
        snapshot.point_count = cache_point_count(cache);
        snapshot.values = cache_values(cache);
    }
    Ok(snapshot)
}

fn set_series_source(
    chart_xml: &mut ChartXml,
    series_number: usize,
    role_name: &str,
    formula: &str,
    cache_points: &[CachePoint],
) -> CliResult<SetSeriesSourceResult> {
    let role = chart_source_role_for(role_name).ok_or_else(|| {
        CliError::invalid_args(format!(
            "invalid chart source role {role_name:?} (must be name, categories, values, xValues, yValues, or bubbleSize)"
        ))
    })?;
    let total_series = series_count(&chart_xml.root);
    let ser = nth_series_mut(&mut chart_xml.root, series_number).ok_or_else(|| {
        CliError::invalid_args(format!(
            "series {series_number} is out of range (1-{})",
            total_series
        ))
    })?;
    let available_roles = series_roles(ser).join(", ");
    let role_elem = ser.direct_child_mut(role.element).ok_or_else(|| {
        CliError::invalid_args(format!(
            "series {series_number} has no {} source (available roles: {available_roles})",
            role.canonical
        ))
    })?;

    let (ref_kind, cache_type, cache_preview, cache_point_count) = {
        let (ref_kind, source_ref) = source_ref_child_mut(role_elem)?;
        let formula = normalize_formula_text(formula);
        let prefix = prefix_from_name(&source_ref.name).unwrap_or_else(|| "c".to_string());
        let formula_index = match source_ref.direct_child_index("f") {
            Some(index) => index,
            None => {
                source_ref
                    .children
                    .insert(0, XmlNode::new(qname(&prefix, "f")));
                0
            }
        };
        source_ref.children[formula_index].text = formula;
        source_ref.children.retain(|child| !is_cache_child(child));
        let formula_index = source_ref.direct_child_index("f").unwrap_or(0);
        let cache_type = cache_type_for_ref_kind(&ref_kind);
        let cache_values = cache_points
            .iter()
            .map(|point| point.value.clone())
            .collect::<Vec<_>>();
        let cache = build_cache_element_for_prefix(&prefix, &cache_type, cache_points, "General");
        source_ref.children.insert(formula_index + 1, cache);
        (
            ref_kind,
            cache_type,
            preview_strings(&cache_values, 5),
            cache_points.len(),
        )
    };

    let counts = sibling_point_counts(ser);
    let mut warnings = Vec::new();
    if let Some(edited_count) = counts.get(role.canonical).copied()
        && edited_count > 0
        && comparable_point_role(role.canonical)
    {
        for (sibling_role, count) in counts {
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

    let _ = ref_kind;
    Ok(SetSeriesSourceResult {
        cache_type,
        cache_point_count,
        cache_preview,
        warnings,
    })
}

fn nth_series_mut(root: &mut XmlNode, series_number: usize) -> Option<&mut XmlNode> {
    if series_number == 0 {
        return None;
    }
    let plot_area = root.first_descendant_mut("plotArea")?;
    let mut count = 0usize;
    for chart_type in &mut plot_area.children {
        if !chart_type.local().ends_with("Chart") {
            continue;
        }
        for ser in &mut chart_type.children {
            if ser.local() != "ser" {
                continue;
            }
            count += 1;
            if count == series_number {
                return Some(ser);
            }
        }
    }
    None
}

fn source_ref_child(role_elem: &XmlNode) -> CliResult<(&XmlNode, String)> {
    for local in ["numRef", "strRef", "multiLvlStrRef"] {
        if let Some(child) = role_elem.direct_child(local) {
            if local == "multiLvlStrRef" {
                return Err(CliError::invalid_args(
                    "multi-level category sources are not supported",
                ));
            }
            return Ok((child, local.to_string()));
        }
    }
    if role_elem.direct_child("v").is_some() {
        return Err(CliError::invalid_args(
            "series source is a literal value, not a cell reference; setting literal chart sources is not supported",
        ));
    }
    Err(CliError::invalid_args(
        "series source has no supported reference",
    ))
}

fn source_ref_child_mut(role_elem: &mut XmlNode) -> CliResult<(String, &mut XmlNode)> {
    if let Some(index) = role_elem
        .children
        .iter()
        .position(|child| matches!(child.local(), "numRef" | "strRef" | "multiLvlStrRef"))
    {
        let ref_kind = role_elem.children[index].local().to_string();
        if ref_kind == "multiLvlStrRef" {
            return Err(CliError::invalid_args(
                "multi-level category sources are not supported",
            ));
        }
        return Ok((ref_kind, &mut role_elem.children[index]));
    }
    if role_elem.direct_child("v").is_some() {
        return Err(CliError::invalid_args(
            "series source is a literal value, not a cell reference; setting literal chart sources is not supported",
        ));
    }
    Err(CliError::invalid_args(
        "series source has no supported reference",
    ))
}

fn first_cache_child(node: &XmlNode) -> Option<&XmlNode> {
    node.children.iter().find(|child| is_cache_child(child))
}

fn is_cache_child(node: &XmlNode) -> bool {
    matches!(node.local(), "strCache" | "numCache" | "multiLvlStrCache")
}

fn cache_point_count(cache: &XmlNode) -> usize {
    cache
        .direct_child("ptCount")
        .and_then(|node| node.attr("val"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or_else(|| cache.descendants("pt").len())
}

fn cache_values(cache: &XmlNode) -> Vec<String> {
    cache
        .descendants("pt")
        .into_iter()
        .filter_map(|point| point.direct_child("v"))
        .map(|value| value.text.clone())
        .collect()
}

fn series_roles(ser: &XmlNode) -> Vec<String> {
    let mut roles = Vec::new();
    for role_name in [
        "name",
        "categories",
        "values",
        "xValues",
        "yValues",
        "bubbleSize",
    ] {
        if let Some(role) = chart_source_role_for(role_name)
            && ser.direct_child(role.element).is_some()
        {
            roles.push(role.canonical.to_string());
        }
    }
    if roles.is_empty() {
        roles.push("none".to_string());
    }
    roles
}

fn sibling_point_counts(ser: &XmlNode) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for role_name in [
        "name",
        "categories",
        "values",
        "xValues",
        "yValues",
        "bubbleSize",
    ] {
        let Some(role) = chart_source_role_for(role_name) else {
            continue;
        };
        let Some(role_elem) = ser.direct_child(role.element) else {
            continue;
        };
        let Ok((source_ref, _)) = source_ref_child(role_elem) else {
            continue;
        };
        let Some(cache) = first_cache_child(source_ref) else {
            continue;
        };
        counts.insert(role.canonical.to_string(), cache_point_count(cache));
    }
    counts
}

fn cache_type_for_ref_kind(ref_kind: &str) -> String {
    if ref_kind == "numRef" {
        "numCache".to_string()
    } else {
        "strCache".to_string()
    }
}

fn comparable_point_role(role: &str) -> bool {
    role != "name"
}

fn build_cache_element_for_prefix(
    prefix: &str,
    cache_type: &str,
    points: &[CachePoint],
    format_code: &str,
) -> XmlNode {
    let mut cache = XmlNode::new(qname(prefix, cache_type));
    if cache_type == "numCache" {
        let mut format = XmlNode::new(qname(prefix, "formatCode"));
        format.text = if format_code.trim().is_empty() {
            "General".to_string()
        } else {
            format_code.to_string()
        };
        cache.children.push(format);
    }
    let mut pt_count = XmlNode::new(qname(prefix, "ptCount"));
    pt_count.set_attr("val", &points.len().to_string());
    cache.children.push(pt_count);
    for (idx, point) in points.iter().enumerate() {
        let point_index = point.index;
        let mut pt = XmlNode::new(qname(prefix, "pt"));
        pt.set_attr("idx", &point_index.max(idx).to_string());
        let mut value = XmlNode::new(qname(prefix, "v"));
        value.text = point.value.clone();
        pt.children.push(value);
        cache.children.push(pt);
    }
    cache
}

fn normalize_formula_text(value: &str) -> String {
    value.trim().trim_start_matches('=').trim().to_string()
}

fn parse_local_range_formula(formula: &str) -> Option<(String, String)> {
    let formula = normalize_formula_text(formula);
    if formula.is_empty() {
        return None;
    }
    let mut bang = None;
    let mut in_quote = false;
    let bytes = formula.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                if in_quote && index + 1 < bytes.len() && bytes[index + 1] == b'\'' {
                    index += 2;
                    continue;
                }
                in_quote = !in_quote;
            }
            b'!' if !in_quote => bang = Some(index),
            _ => {}
        }
        index += 1;
    }
    let bang = bang?;
    let mut sheet = formula[..bang].to_string();
    let reference = &formula[bang + 1..];
    if sheet.starts_with('\'') && sheet.ends_with('\'') && sheet.len() >= 2 {
        sheet = sheet[1..sheet.len() - 1].replace("''", "'");
    }
    if sheet.contains(['[', ']']) || reference.contains(['[', ']', ',']) {
        return None;
    }
    let normalized = normalize_chart_range_ref(reference)?;
    Some((sheet, normalized))
}

fn normalize_chart_range_ref(reference: &str) -> Option<String> {
    let cleaned = reference.replace('$', "");
    parse_range(&cleaned).ok().map(|bounds| {
        let bounds = bounds.normalized();
        if reference.contains('$') {
            absolute_range_bounds_ref(bounds)
        } else {
            range_bounds_ref(bounds)
        }
    })
}

fn absolute_range_bounds_ref(bounds: RangeBounds) -> String {
    let start = format!("${}${}", col_name(bounds.start_col), bounds.start_row);
    let end = format!("${}${}", col_name(bounds.end_col), bounds.end_row);
    if start == end {
        start
    } else {
        format!("{start}:{end}")
    }
}

fn chart_cache_points_for_values(
    values: &[String],
    ref_kind: &str,
) -> Result<Vec<CachePoint>, String> {
    if values.is_empty() {
        return Err("at least one point is required".to_string());
    }
    let mut points = Vec::with_capacity(values.len());
    for (idx, value) in values.iter().enumerate() {
        if ref_kind == "numRef" {
            if value.trim().is_empty() {
                return Err(format!(
                    "point {} is empty but numeric chart sources require numbers",
                    idx + 1
                ));
            }
            if value.parse::<f64>().is_err() {
                return Err(format!(
                    "point {} value {:?} is not numeric",
                    idx + 1,
                    value
                ));
            }
        }
        points.push(CachePoint {
            index: idx,
            value: value.clone(),
        });
    }
    Ok(points)
}

fn update_embedded_workbook_chart_range(
    file: &str,
    bytes: &[u8],
    snapshot: &SeriesSourceSnapshot,
    values: &[String],
) -> CliResult<Option<Vec<u8>>> {
    if snapshot.sheet.is_empty() || snapshot.range.is_empty() {
        return Ok(None);
    }
    let bounds = parse_range(&snapshot.range).map_err(|err| {
        CliError::invalid_args(format!(
            "invalid embedded workbook source range {:?}: {}",
            snapshot.range, err.message
        ))
    })?;
    let bounds = bounds.normalized();
    let rows = bounds.row_count() as usize;
    let cols = bounds.col_count() as usize;
    if rows != 1 && cols != 1 {
        return Err(CliError::invalid_args(format!(
            "embedded workbook source range {} is {}x{}; update-data currently requires a one-row or one-column series range",
            range_bounds_ref(bounds),
            rows,
            cols
        )));
    }
    if rows * cols != values.len() {
        return Err(CliError::invalid_args(format!(
            "embedded workbook source range {} has {} cell(s) but {} input has {} point(s)",
            range_bounds_ref(bounds),
            rows * cols,
            snapshot.role,
            values.len()
        )));
    }
    let matrix = chart_values_to_range_matrix(values, rows, cols, &snapshot.ref_kind);
    let values_json = serde_json::to_string(&matrix).map_err(|err| {
        CliError::unexpected(format!(
            "failed to encode embedded workbook update values: {err}"
        ))
    })?;
    let parent = Path::new(file)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let nonce = chrono_like_counter();
    let input_path = parent.join(format!(
        ".ooxml-rust-pptx-chart-embedded-{}-{nonce}.xlsx",
        std::process::id()
    ));
    let output_path = parent.join(format!(
        ".ooxml-rust-pptx-chart-embedded-out-{}-{nonce}.xlsx",
        std::process::id()
    ));
    let input_string = input_path.to_string_lossy().to_string();
    let output_string = output_path.to_string_lossy().to_string();
    fs::write(&input_path, bytes)
        .map_err(|err| CliError::unexpected(format!("failed to stage embedded workbook: {err}")))?;
    let set_result = xlsx_ranges_set(
        &input_string,
        XlsxRangesSetOptions {
            sheet: &snapshot.sheet,
            range: Some(&range_bounds_ref(bounds)),
            anchor: None,
            values: Some(&values_json),
            values_file: None,
            data_format: Some("json"),
            null_policy: Some("empty-string"),
            ragged: Some("reject"),
            max_cells: 0,
            out: Some(&output_string),
            backup: None,
            dry_run: false,
            no_validate: true,
            in_place: false,
            overwrite_formulas: true,
        },
    );
    let result = match set_result {
        Ok(_) => fs::read(&output_path).map(Some).map_err(|err| {
            CliError::unexpected(format!("failed to read updated embedded workbook: {err}"))
        }),
        Err(err) => Err(CliError::invalid_args(format!(
            "failed to update embedded workbook range {}!{}: {}",
            snapshot.sheet, snapshot.range, err.message
        ))),
    };
    let _ = fs::remove_file(&input_path);
    let _ = fs::remove_file(&output_path);
    result
}

fn chart_values_to_range_matrix(
    values: &[String],
    rows: usize,
    cols: usize,
    ref_kind: &str,
) -> Vec<Vec<Value>> {
    let value_type = if ref_kind == "numRef" {
        "number"
    } else {
        "string"
    };
    let mut matrix = Vec::with_capacity(rows);
    let mut index = 0usize;
    for _ in 0..rows {
        let mut row = Vec::with_capacity(cols);
        for _ in 0..cols {
            row.push(json!({
                "type": value_type,
                "value": values.get(index).cloned().unwrap_or_default(),
            }));
            index += 1;
        }
        matrix.push(row);
    }
    matrix
}

fn chart_values_hash(values: &[String]) -> String {
    let data = serde_json::to_vec(values).unwrap_or_else(|_| b"[]".to_vec());
    let digest = Sha256::digest(data);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("sha256:{hex}")
}

fn chart_hash_matches(current: &str, expected: &str) -> bool {
    let expected = expected.trim();
    expected.is_empty()
        || current == expected
        || (!expected.starts_with("sha256:") && current.trim_start_matches("sha256:") == expected)
}

fn preview_strings(values: &[String], limit: usize) -> Vec<String> {
    values.iter().take(limit).cloned().collect()
}

fn unique_sorted_warnings(warnings: Vec<String>) -> Vec<String> {
    warnings
        .into_iter()
        .map(|warning| warning.trim().to_string())
        .filter(|warning| !warning.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

struct ChartCreateResultInput<'a> {
    file: &'a str,
    output_path: Option<&'a str>,
    dry_run: bool,
    slide: i64,
    create: &'a CreateSlideChartResult,
    source: &'a ChartCreateSource,
    geometry: &'a ChartGeometry,
    chart: Value,
}

fn chart_create_result_json(input: ChartCreateResultInput<'_>) -> Value {
    let ChartCreateResultInput {
        file,
        output_path,
        dry_run,
        slide,
        create,
        source,
        geometry,
        chart,
    } = input;
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if !dry_run && let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("action".to_string(), json!("pptx.chart.create"));
    result.insert("slide".to_string(), json!(slide));
    result.insert("chartType".to_string(), json!(create.chart_type));
    if !create.title.is_empty() {
        result.insert("title".to_string(), json!(create.title));
    }
    result.insert("chartPartUri".to_string(), json!(create.chart_uri));
    result.insert(
        "chartRelationshipId".to_string(),
        json!(create.chart_relationship_id),
    );
    result.insert("shapeId".to_string(), json!(create.shape_id));
    result.insert("shapeName".to_string(), json!(create.shape_name));
    result.insert("seriesCount".to_string(), json!(create.series_count));
    result.insert("categories".to_string(), json!(create.categories));
    result.insert("x".to_string(), json!(geometry.x));
    result.insert("y".to_string(), json!(geometry.y));
    result.insert("cx".to_string(), json!(geometry.cx));
    result.insert("cy".to_string(), json!(geometry.cy));
    result.insert("sourceMode".to_string(), json!(source.mode));
    if source.mode == "external" {
        result.insert("sourceFile".to_string(), json!(source.source_file));
    }
    if !source.sheet.is_empty() {
        result.insert("sourceSheet".to_string(), json!(source.sheet));
    }
    if !source.range.is_empty() {
        result.insert("sourceRange".to_string(), json!(source.range));
    }
    if !create.embedded_workbook_part_uri.is_empty() {
        result.insert(
            "embeddedWorkbookPartUri".to_string(),
            json!(create.embedded_workbook_part_uri),
        );
    }
    result.insert("chart".to_string(), chart);
    let warnings = unique_sorted_warnings(create.warnings.clone());
    if !warnings.is_empty() {
        result.insert("warnings".to_string(), json!(warnings));
    }
    add_pptx_chart_create_commands(&mut result, output_path, dry_run, slide, &create.chart_uri);
    Value::Object(result)
}

struct ChartUpdateDataResultInput<'a> {
    file: &'a str,
    output_path: Option<&'a str>,
    dry_run: bool,
    slide: i64,
    series: i64,
    chart: Value,
    selected: &'a SelectedChart,
    updated_roles: Vec<UpdatedRoleResult>,
    embedded_updated: bool,
    current_values_hash: &'a str,
    expect_values_hash: &'a str,
    warnings: Vec<String>,
}

fn chart_update_data_result_json(input: ChartUpdateDataResultInput<'_>) -> Value {
    let ChartUpdateDataResultInput {
        file,
        output_path,
        dry_run,
        slide,
        series,
        chart,
        selected,
        updated_roles,
        embedded_updated,
        current_values_hash,
        expect_values_hash,
        warnings,
    } = input;
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if !dry_run && let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("action".to_string(), json!("pptx.chart.update-data"));
    result.insert("chart".to_string(), chart);
    result.insert("series".to_string(), json!(series));
    result.insert(
        "updatedRoles".to_string(),
        Value::Array(
            updated_roles
                .iter()
                .map(updated_role_result_json)
                .collect::<Vec<_>>(),
        ),
    );
    if !selected.embedded_workbook_part_uri.is_empty() {
        result.insert(
            "embeddedWorkbookPartUri".to_string(),
            json!(selected.embedded_workbook_part_uri),
        );
    }
    result.insert(
        "embeddedWorkbookUpdated".to_string(),
        json!(embedded_updated),
    );
    result.insert("cacheVerified".to_string(), json!(false));
    if !warnings.is_empty() {
        result.insert("warnings".to_string(), json!(warnings));
    }
    result.insert(
        "storedCacheContract".to_string(),
        json!("stored chart cache values and embedded workbook cells are updated, but chart rendering is not recalculated by PowerPoint until validation/render/open"),
    );
    add_pptx_chart_update_commands(
        &mut result,
        output_path,
        dry_run,
        slide,
        &selected.part_selector(),
    );
    if !current_values_hash.is_empty() {
        result.insert("currentValuesHash".to_string(), json!(current_values_hash));
    }
    if !expect_values_hash.trim().is_empty() {
        result.insert(
            "expectedValuesHashAccepted".to_string(),
            json!(expect_values_hash.trim()),
        );
    }
    Value::Object(result)
}

fn updated_role_result_json(role: &UpdatedRoleResult) -> Value {
    let mut item = Map::new();
    item.insert("role".to_string(), json!(role.role));
    item.insert("formula".to_string(), json!(role.snapshot.formula));
    if !role.snapshot.sheet.is_empty() {
        item.insert("sheet".to_string(), json!(role.snapshot.sheet));
    }
    if !role.snapshot.range.is_empty() {
        item.insert("range".to_string(), json!(role.snapshot.range));
    }
    item.insert("refKind".to_string(), json!(role.snapshot.ref_kind));
    if !role.snapshot.cache_type.is_empty() {
        item.insert(
            "previousCacheType".to_string(),
            json!(role.snapshot.cache_type),
        );
    }
    item.insert(
        "previousCachePointCount".to_string(),
        json!(role.snapshot.point_count),
    );
    let previous_preview = preview_strings(&role.snapshot.values, 5);
    if !previous_preview.is_empty() {
        item.insert("previousCachePreview".to_string(), json!(previous_preview));
    }
    if !role.previous_values_hash.is_empty() {
        item.insert(
            "previousValuesHash".to_string(),
            json!(role.previous_values_hash),
        );
    }
    if !role.mutation.cache_type.is_empty() {
        item.insert("cacheType".to_string(), json!(role.mutation.cache_type));
    }
    item.insert(
        "cachePointCount".to_string(),
        json!(role.mutation.cache_point_count),
    );
    if !role.mutation.cache_preview.is_empty() {
        item.insert(
            "cachePreview".to_string(),
            json!(role.mutation.cache_preview),
        );
    }
    item.insert(
        "embeddedWorkbookRangeUpdated".to_string(),
        json!(role.embedded_workbook_range_updated),
    );
    Value::Object(item)
}

fn add_pptx_chart_create_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    dry_run: bool,
    slide: i64,
    chart_part_uri: &str,
) {
    let target = output_path.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    let selector = format!("part:{chart_part_uri}");
    result.insert(
        format!("chartShowCommand{suffix}"),
        json!(pptx_chart_show_command(target, slide, &selector)),
    );
    result.insert(
        format!("chartsListCommand{suffix}"),
        json!(pptx_charts_list_command(target, slide)),
    );
    result.insert(
        format!("validateCommand{suffix}"),
        json!(pptx_validate_command(target)),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(pptx_render_command(target)),
    );
}

fn add_pptx_chart_update_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    dry_run: bool,
    slide: i64,
    selector: &str,
) {
    let target = output_path.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    result.insert(
        format!("validateCommand{suffix}"),
        json!(pptx_validate_command(target)),
    );
    result.insert(
        format!("chartShowCommand{suffix}"),
        json!(pptx_chart_show_command(target, slide, selector)),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(pptx_render_command(target)),
    );
}

fn pptx_chart_show_command(file: &str, slide: i64, selector: &str) -> String {
    let mut command = format!("ooxml --json pptx charts show {}", command_arg(file));
    if slide > 0 {
        command.push_str(&format!(" --slide {slide}"));
    }
    if !selector.trim().is_empty() {
        command.push_str(&format!(" --chart {}", command_arg(selector)));
    }
    command
}

fn pptx_charts_list_command(file: &str, slide: i64) -> String {
    let mut command = format!("ooxml --json pptx charts list {}", command_arg(file));
    if slide > 0 {
        command.push_str(&format!(" --slide {slide}"));
    }
    command
}

fn pptx_validate_command(file: &str) -> String {
    format!("ooxml validate --strict {}", command_arg(file))
}

fn pptx_render_command(file: &str) -> String {
    format!("ooxml pptx render {} --out render-check", command_arg(file))
}

struct SelectedChart {
    part_uri: String,
    embedded_workbook_part_uri: String,
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
    stage_chart_package_mutation(file, overrides, &BTreeMap::new(), options)
}

fn stage_chart_package_mutation(
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
