use super::commands::AreaFillTarget;
use super::package::*;
use super::xml::*;
use super::*;

pub(super) fn set_chart_title(
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

pub(super) fn set_chart_legend(
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

pub(super) fn normalize_legend_position(value: &str, flag: &str) -> CliResult<String> {
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

pub(super) fn set_area_fill(
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

pub(super) fn set_series_style(
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

pub(super) fn normalize_marker_symbol(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "circle" | "square" | "diamond" | "triangle" | "none" => {
            Ok(value.trim().to_ascii_lowercase())
        }
        _ => Err(CliError::invalid_args(
            "--marker-symbol must be circle, square, diamond, triangle, or none",
        )),
    }
}

pub(super) fn convert_chart_type(
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

pub(super) fn first_plot_index(plot_area: &XmlNode) -> Option<usize> {
    plot_area
        .children
        .iter()
        .position(|child| child.local().ends_with("Chart"))
}

pub(super) fn canonical_chart_type(plot: &XmlNode) -> CliResult<ChartType> {
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

pub(super) fn set_bar_dir(plot: &mut XmlNode, target: ChartType, chart_prefix: &str) {
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

pub(super) fn plot_axis_ids(plot: &XmlNode) -> Vec<String> {
    plot.children
        .iter()
        .filter(|child| child.local() == "axId")
        .filter_map(|child| child.attr("val"))
        .map(|value| value.trim().to_string())
        .collect()
}

pub(super) fn transform_series_for_type(
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

pub(super) fn rename_direct_child(parent: &mut XmlNode, from: &str, to: &str, chart_prefix: &str) {
    if let Some(child) = parent
        .children
        .iter_mut()
        .find(|child| child.local() == from)
    {
        child.name = qname(chart_prefix, to);
    }
}

pub(super) fn reorder_children(parent: &mut XmlNode, order: &[&str]) {
    let children = std::mem::take(&mut parent.children);
    for child in children {
        insert_child_in_order(parent, child, order);
    }
}

pub(super) fn build_plot_wrapper(
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

pub(super) fn append_axis_ids(
    plot: &mut XmlNode,
    axis_ids: &[String],
    count: usize,
    chart_prefix: &str,
) {
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

pub(super) fn transform_axes_for_type(
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

pub(super) fn axis_by_id_index(plot_area: &XmlNode, id: &str, fallback: &str) -> Option<usize> {
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

pub(super) fn rename_axis_element(axis: &mut XmlNode, new_local: &str, chart_prefix: &str) {
    axis.name = qname(chart_prefix, new_local);
    let order = axis_child_order(new_local);
    axis.children
        .retain(|child| order.iter().any(|candidate| *candidate == child.local()));
}

pub(super) fn set_chart_axis(
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

pub(super) fn apply_gridlines(
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

pub(super) fn apply_axis_tick_label_font(
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

pub(super) fn set_or_create_val_child(
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

pub(super) fn chart_value_node(chart_prefix: &str, local: &str, value: &str) -> XmlNode {
    let mut node = XmlNode::new(qname(chart_prefix, local));
    node.set_attr("val", value);
    node
}

pub(super) fn format_float(value: f64) -> String {
    let formatted = format!("{value}");
    formatted
}

pub(super) fn read_template_chart_style(
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

pub(super) fn inspect_style_snapshot(root: &XmlNode) -> ChartStyleSnapshot {
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

pub(super) fn inspect_axis_snapshots(plot_area: &XmlNode) -> Vec<AxisStyleSnapshot> {
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

pub(super) fn inspect_series_snapshot(ser: &XmlNode) -> SeriesStyleSnapshot {
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

pub(super) fn inspect_title_font_snapshot(title: &XmlNode) -> Option<FontOptions> {
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

pub(super) fn inspect_tick_font_snapshot(tx_pr: &XmlNode) -> Option<FontOptions> {
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

pub(super) fn inspect_font_snapshot(r_pr: &XmlNode) -> Option<FontOptions> {
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

pub(super) fn apply_chart_style(
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

pub(super) fn find_target_axis_index(
    plot_area: &XmlNode,
    source: &AxisStyleSnapshot,
) -> Option<usize> {
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

pub(super) fn apply_source_series_styles(
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

pub(super) fn apply_series_style_snapshot(
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

pub(super) fn title_tx_node(chart_xml: &ChartXml, text: &str, font: &FontOptions) -> XmlNode {
    title_tx_node_for_prefixes(
        &chart_xml.chart_prefix,
        &chart_xml.drawing_prefix,
        text,
        font,
    )
}

pub(super) fn title_tx_node_for_prefixes(
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

pub(super) fn apply_font_options(r_pr: &mut XmlNode, chart_xml: &ChartXml, font: &FontOptions) {
    apply_font_options_with_scheme(r_pr, &chart_xml.drawing_prefix, font)
}

pub(super) fn apply_font_options_with_scheme(
    r_pr: &mut XmlNode,
    drawing_prefix: &str,
    font: &FontOptions,
) {
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

pub(super) fn bool_val(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

pub(super) fn parse_ooxml_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on"
    )
}

pub(super) fn set_shape_fill(holder: &mut XmlNode, drawing_prefix: &str, fill: &str) {
    remove_fill_children(holder);
    let child = if fill.is_empty() {
        XmlNode::new(qname(drawing_prefix, "noFill"))
    } else {
        solid_fill_node(drawing_prefix, fill)
    };
    insert_child_in_order(holder, child, SHAPE_PROPS_CHILD_ORDER);
}

pub(super) fn set_shape_fill_from_source(holder: &mut XmlNode, drawing_prefix: &str, fill: &str) {
    remove_fill_children(holder);
    insert_child_in_order(
        holder,
        fill_node_from_source(drawing_prefix, fill),
        SHAPE_PROPS_CHILD_ORDER,
    );
}

pub(super) fn set_line_fill(line: &mut XmlNode, drawing_prefix: &str, fill: &str) {
    remove_fill_children(line);
    let child = if fill.is_empty() {
        XmlNode::new(qname(drawing_prefix, "noFill"))
    } else {
        solid_fill_node(drawing_prefix, fill)
    };
    insert_child_in_order(line, child, LINE_CHILD_ORDER);
}

pub(super) fn set_line_fill_from_source(line: &mut XmlNode, drawing_prefix: &str, fill: &str) {
    remove_fill_children(line);
    insert_child_in_order(
        line,
        fill_node_from_source(drawing_prefix, fill),
        LINE_CHILD_ORDER,
    );
}

pub(super) fn remove_fill_children(holder: &mut XmlNode) {
    holder.children.retain(|child| {
        !matches!(
            child.local(),
            "noFill" | "solidFill" | "gradFill" | "blipFill" | "pattFill" | "grpFill"
        )
    });
}

pub(super) fn solid_fill_node(drawing_prefix: &str, fill: &str) -> XmlNode {
    let mut solid = XmlNode::new(qname(drawing_prefix, "solidFill"));
    let mut srgb = XmlNode::new(qname(drawing_prefix, "srgbClr"));
    srgb.set_attr("val", fill);
    solid.children.push(srgb);
    solid
}

pub(super) fn fill_node_from_source(drawing_prefix: &str, fill: &str) -> XmlNode {
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

pub(super) fn title_text(title: &XmlNode) -> String {
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

pub(super) fn inspect_fill(holder: &XmlNode) -> String {
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

pub(super) fn series_count(root: &XmlNode) -> usize {
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

pub(super) fn walk_series(root: &XmlNode) -> Vec<&XmlNode> {
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

pub(super) fn ensure_child_index(
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

pub(super) fn insert_child_in_order(parent: &mut XmlNode, child: XmlNode, order: &[&str]) {
    let local = child.local().to_string();
    let index = insertion_index(parent, &local, order);
    parent.children.insert(index, child);
}

pub(super) fn insertion_index(parent: &XmlNode, local: &str, order: &[&str]) -> usize {
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
