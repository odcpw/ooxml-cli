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
    append_xml_text_event, chrono_like_counter, col_name, command_arg,
    copy_zip_with_binary_part_overrides_and_removals, decode_xml_text,
    ensure_content_type_override, is_xml_text_event, local_name, package_mutation_temp_path,
    package_type, parse_cli_range, parse_i64_flag, parse_range, parse_string_flag,
    range_bounds_ref, relationship_entries, relationship_entries_from_xml,
    relationship_target_from_source_to_target, relationships, relationships_part_for, validate,
    validate_xlsx_mutation_output_flags, xlsx_range_export_with_options, xlsx_ranges_set,
    xml_attr_escape, xml_attrs_map, xml_escape, zip_bytes, zip_entry_names, zip_text,
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

mod commands;
mod data;
mod package;
mod style;
mod xml;

use xml::qname;

pub(crate) use commands::{
    pptx_charts_convert_type, pptx_charts_copy_style, pptx_charts_create, pptx_charts_set_axis,
    pptx_charts_set_chart_area_fill, pptx_charts_set_legend, pptx_charts_set_plot_area_fill,
    pptx_charts_set_series_style, pptx_charts_set_title, pptx_charts_update_data,
};

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
