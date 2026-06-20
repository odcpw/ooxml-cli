use super::*;

pub(super) const REL_WORKSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
pub(super) const REL_DRAWING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing";
pub(super) const REL_CHART: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
pub(super) const NS_CHART: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
pub(super) const NS_DRAWING_MAIN: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub(super) const NS_RELATIONSHIPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub(super) const NS_SPREADSHEET_DRAWING: &str =
    "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
pub(super) const CONTENT_TYPE_CHART: &str =
    "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
pub(super) const CONTENT_TYPE_DRAWING: &str =
    "application/vnd.openxmlformats-officedocument.drawing+xml";

pub(super) const CHART_CHILD_ORDER: &[&str] = &[
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
pub(super) const TITLE_CHILD_ORDER: &[&str] =
    &["tx", "layout", "overlay", "spPr", "txPr", "extLst"];
pub(super) const LEGEND_CHILD_ORDER: &[&str] = &[
    "legendPos",
    "legendEntry",
    "layout",
    "overlay",
    "spPr",
    "txPr",
    "extLst",
];
pub(super) const SERIES_CHILD_ORDER: &[&str] = &[
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
pub(super) const SHAPE_PROPS_CHILD_ORDER: &[&str] = &[
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
pub(super) const LINE_CHILD_ORDER: &[&str] = &[
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
pub(super) const MARKER_CHILD_ORDER: &[&str] = &["symbol", "size", "spPr", "extLst"];
pub(super) const PARAGRAPH_CHILD_ORDER: &[&str] = &["pPr", "r", "br", "fld", "endParaRPr"];
pub(super) const RUN_CHILD_ORDER: &[&str] = &["rPr", "t"];
pub(super) const RPR_CHILD_ORDER: &[&str] = &[
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
pub(super) const PLOT_AREA_CHILD_ORDER: &[&str] = &["spPr", "extLst"];
pub(super) const CHART_SPACE_CHILD_ORDER: &[&str] = &[
    "spPr",
    "txPr",
    "externalData",
    "printSettings",
    "userShapes",
    "extLst",
];
pub(super) const SCALING_CHILD_ORDER: &[&str] = &["logBase", "orientation", "max", "min", "extLst"];
pub(super) const CAT_AXIS_CHILD_ORDER: &[&str] = &[
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
pub(super) const VAL_AXIS_CHILD_ORDER: &[&str] = &[
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
pub(super) const TX_PR_CHILD_ORDER: &[&str] = &["bodyPr", "lstStyle", "p"];
pub(super) const WORKSHEET_CHILD_ORDER: &[&str] = &[
    "sheetPr",
    "dimension",
    "sheetViews",
    "sheetFormatPr",
    "cols",
    "sheetData",
    "sheetCalcPr",
    "sheetProtection",
    "protectedRanges",
    "scenarios",
    "autoFilter",
    "sortState",
    "dataConsolidate",
    "customSheetViews",
    "mergeCells",
    "phoneticPr",
    "conditionalFormatting",
    "dataValidations",
    "hyperlinks",
    "printOptions",
    "pageMargins",
    "pageSetup",
    "headerFooter",
    "rowBreaks",
    "colBreaks",
    "customProperties",
    "cellWatches",
    "ignoredErrors",
    "smartTags",
    "drawing",
    "drawingHF",
    "picture",
    "oleObjects",
    "controls",
    "webPublishItems",
    "tableParts",
    "extLst",
];

#[derive(Clone)]
pub(super) struct ChartRef {
    pub(super) number: u32,
    pub(super) sheet: String,
    pub(super) sheet_number: u32,
    pub(super) sheet_part_uri: String,
    pub(super) drawing_relationship_id: String,
    pub(super) drawing_part_uri: String,
    pub(super) relationship_id: String,
    pub(super) part_uri: String,
    pub(super) name: String,
    pub(super) title: String,
    pub(super) types: Vec<String>,
    pub(super) anchor: Option<ChartAnchor>,
    pub(super) primary_selector: String,
    pub(super) selectors: Vec<String>,
    pub(super) series: Vec<ChartSeries>,
    pub(super) style: Option<Value>,
}

#[derive(Clone)]
pub(super) struct ChartMarker {
    pub(super) column: i64,
    pub(super) column_offset: i64,
    pub(super) row: i64,
    pub(super) row_offset: i64,
}

#[derive(Clone)]
pub(super) struct ChartAnchor {
    pub(super) kind: String,
    pub(super) from: Option<ChartMarker>,
    pub(super) to: Option<ChartMarker>,
}

#[derive(Clone)]
pub(super) struct ChartDataSource {
    pub(super) formula: String,
    pub(super) sheet: String,
    pub(super) range: String,
    pub(super) ref_kind: String,
    pub(super) cache_type: String,
    pub(super) point_count: i64,
    pub(super) cache_preview: Vec<String>,
}

#[derive(Clone)]
pub(super) struct ChartSeries {
    pub(super) number: u32,
    pub(super) index: i64,
    pub(super) order: i64,
    pub(super) name: Option<ChartDataSource>,
    pub(super) categories: Option<ChartDataSource>,
    pub(super) values: Option<ChartDataSource>,
    pub(super) x_values: Option<ChartDataSource>,
    pub(super) y_values: Option<ChartDataSource>,
    pub(super) bubble_size: Option<ChartDataSource>,
}

#[derive(Clone, Debug)]
pub(super) struct XmlAttr {
    pub(super) qname: String,
    pub(super) local: String,
    pub(super) value: String,
}

#[derive(Clone, Debug)]
pub(super) struct XmlNode {
    pub(super) qname: String,
    pub(super) name: String,
    pub(super) attrs: BTreeMap<String, String>,
    pub(super) raw_attrs: Vec<XmlAttr>,
    pub(super) text: String,
    pub(super) children: Vec<XmlNode>,
}

pub(super) fn xlsx_charts_result(file: &str, charts: Vec<ChartRef>) -> Value {
    json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "charts": charts.iter().map(|chart| xlsx_chart_item(file, chart)).collect::<Vec<_>>(),
    })
}

#[derive(Clone)]
pub(super) struct XlsxChartOutputOptions<'a> {
    pub(super) out: Option<&'a str>,
    pub(super) backup: Option<&'a str>,
    pub(super) dry_run: bool,
    pub(super) no_validate: bool,
    pub(super) in_place: bool,
}

impl<'a> XlsxChartOutputOptions<'a> {
    pub(super) fn from_title(options: &'a XlsxChartSetTitleOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    pub(super) fn from_legend(options: &'a XlsxChartSetLegendOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    pub(super) fn from_fill(options: &'a XlsxChartSetFillOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    pub(super) fn from_series(options: &'a XlsxChartSetSeriesStyleOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    pub(super) fn from_convert(options: &'a XlsxChartConvertTypeOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    pub(super) fn from_copy_style(options: &'a XlsxChartCopyStyleOptions<'a>) -> Self {
        Self {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        }
    }

    pub(super) fn from_axis(options: &'a XlsxChartSetAxisOptions<'a>) -> Self {
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
pub(super) struct ChartMutationExtra {
    pub(super) previous_title: Option<String>,
    pub(super) legend_removed: bool,
    pub(super) series: Option<i64>,
    pub(super) previous_type: Option<String>,
    pub(super) new_type: Option<String>,
    pub(super) previous_fill: Option<String>,
    pub(super) new_fill: Option<String>,
    pub(super) applied_style: Vec<String>,
    pub(super) warnings: Vec<String>,
}

pub(super) struct ChartStyleResultArgs<'a> {
    pub(super) file: &'a str,
    pub(super) output: Option<&'a str>,
    pub(super) dry_run: bool,
    pub(super) action: &'a str,
    pub(super) chart_item: Value,
    pub(super) sheet_selector: Option<&'a str>,
    pub(super) chart: &'a ChartRef,
    pub(super) extra: ChartMutationExtra,
}

#[derive(Clone)]
pub(super) struct ChartXmlContext {
    pub(super) chart_prefix: String,
    pub(super) drawing_prefix: String,
}

#[derive(Clone)]
pub(super) struct ChartFontOptions {
    pub(super) family: Option<String>,
    pub(super) size_pt: Option<f64>,
    pub(super) color: Option<String>,
    pub(super) bold: Option<bool>,
    pub(super) italic: Option<bool>,
}

impl ChartFontOptions {
    pub(super) fn is_empty(&self) -> bool {
        self.family.is_none()
            && self.size_pt.is_none()
            && self.color.is_none()
            && self.bold.is_none()
            && self.italic.is_none()
    }
}

#[derive(Clone)]
pub(super) struct LegendPosition {
    pub(super) code: String,
    pub(super) remove: bool,
}

#[derive(Clone)]
pub(super) struct ChartFillOptions {
    pub(super) color: String,
    pub(super) no_fill: bool,
}

#[derive(Clone)]
pub(super) struct ChartSeriesStyleOptions {
    pub(super) fill_color: Option<String>,
    pub(super) line_color: Option<String>,
    pub(super) line_width_pt: Option<f64>,
    pub(super) marker_symbol: Option<String>,
    pub(super) marker_size: Option<i64>,
}

impl ChartSeriesStyleOptions {
    pub(super) fn is_empty(&self) -> bool {
        self.fill_color.is_none()
            && self.line_color.is_none()
            && self.line_width_pt.is_none()
            && self.marker_symbol.is_none()
            && self.marker_size.is_none()
    }
}

pub(super) struct ChartTypeConversion {
    pub(super) previous_type: String,
    pub(super) new_type: String,
    pub(super) warnings: Vec<String>,
}

pub(super) struct ChartAxisFlags {
    pub(super) set_title: bool,
    pub(super) title: String,
    pub(super) set_hidden: bool,
    pub(super) hidden: bool,
    pub(super) min: Option<f64>,
    pub(super) max: Option<f64>,
    pub(super) major_unit: Option<f64>,
    pub(super) number_format: Option<String>,
    pub(super) set_major_gridlines: bool,
    pub(super) major_gridlines: bool,
    pub(super) set_minor_gridlines: bool,
    pub(super) minor_gridlines: bool,
    pub(super) tick_label_font: ChartFontOptions,
    pub(super) title_font: ChartFontOptions,
}

#[derive(Clone)]
pub(super) struct ChartSourceCell {
    pub(super) value: String,
    pub(super) kind: String,
    pub(super) null: bool,
    pub(super) number_format_code: String,
}

pub(super) struct ChartCreateSource {
    pub(super) sheet: String,
    pub(super) sheet_number: u32,
    pub(super) range: String,
    pub(super) bounds: RangeBounds,
    pub(super) cells: Vec<Vec<ChartSourceCell>>,
}

pub(super) struct BuiltChartSeries {
    pub(super) name: String,
    pub(super) name_ref: String,
    pub(super) cats: Vec<String>,
    pub(super) cat_ref: String,
    pub(super) values: Vec<String>,
    pub(super) val_ref: String,
}

pub(super) struct ChartCreateArtifacts {
    pub(super) chart_uri: String,
    pub(super) drawing_uri: String,
    pub(super) chart_type: String,
    pub(super) title: String,
    pub(super) series_count: usize,
    pub(super) categories: usize,
    pub(super) anchor: String,
    pub(super) warnings: Vec<String>,
    pub(super) overrides: BTreeMap<String, String>,
}

#[derive(Clone)]
pub(super) struct ChartSourceRole {
    pub(super) canonical: String,
    pub(super) element: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ChartCacheMode {
    Auto,
    Clear,
    Keep,
}

#[derive(Clone)]
pub(super) struct ChartCachePoint {
    pub(super) index: i64,
    pub(super) value: String,
}

#[derive(Clone, Default)]
pub(super) struct ChartCacheUpdate {
    pub(super) points: Vec<ChartCachePoint>,
    pub(super) skipped: i64,
    pub(super) format_code: String,
    pub(super) warnings: Vec<String>,
}

pub(super) struct ResolvedChartUpdateSource {
    pub(super) sheet: String,
    pub(super) range: String,
    pub(super) formula: String,
    pub(super) bounds: RangeBounds,
}

pub(super) struct ChartSourceMutation {
    pub(super) previous_formula: String,
    pub(super) formula: String,
    pub(super) ref_kind: String,
    pub(super) cache_type: String,
    pub(super) cache_point_count: i64,
    pub(super) cache_preview: Vec<String>,
    pub(super) cache_skipped: i64,
    pub(super) warnings: Vec<String>,
}
