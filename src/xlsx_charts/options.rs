pub(crate) struct XlsxChartCreateOptions<'a> {
    pub(crate) chart_type: Option<&'a str>,
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: Option<&'a str>,
    pub(crate) table: Option<&'a str>,
    pub(crate) title: Option<&'a str>,
    pub(crate) anchor: Option<&'a str>,
    pub(crate) expect_source_range: Option<&'a str>,
    pub(crate) max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
pub(crate) struct XlsxChartUpdateSourceOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) chart: Option<&'a str>,
    pub(crate) series: i64,
    pub(crate) role: Option<&'a str>,
    pub(crate) source_sheet: Option<&'a str>,
    pub(crate) source_range: Option<&'a str>,
    pub(crate) formula: Option<&'a str>,
    pub(crate) cache: Option<&'a str>,
    pub(crate) expect_source_range: Option<&'a str>,
    pub(crate) expect_formula: Option<&'a str>,
    pub(crate) max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
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
