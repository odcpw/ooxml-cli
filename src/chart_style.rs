//! Shared, deterministic house-style decisions for newly created charts.
//!
//! The family-specific writers remain responsible for valid DrawingML chart
//! child order. This module owns the policy so PPTX and XLSX do not drift.

use std::collections::BTreeMap;

use crate::{CliError, CliResult};

const EMU_PER_POINT: f64 = 12_700.0;
const ACCENTS: [&str; 6] = [
    "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChartStyleVariant {
    Minimal,
    Default,
    Dense,
}

impl ChartStyleVariant {
    pub(crate) fn parse(value: Option<&str>) -> CliResult<Self> {
        match value
            .unwrap_or("default")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "minimal" => Ok(Self::Minimal),
            "" | "default" => Ok(Self::Default),
            "dense" => Ok(Self::Dense),
            value => Err(CliError::invalid_args(format!(
                "--style must be minimal, default, or dense (found {value:?})"
            ))),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Default => "default",
            Self::Dense => "dense",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChartHouseStyle {
    pub(crate) variant: ChartStyleVariant,
    pub(crate) series_scheme_colors: Vec<&'static str>,
    pub(crate) value_number_format: String,
    pub(crate) major_value_gridlines: bool,
    pub(crate) legend_position: Option<&'static str>,
    pub(crate) data_labels: bool,
    /// Rotation in 1/60,000 degrees, matching DrawingML `a:bodyPr@rot`.
    pub(crate) category_label_rotation: Option<i32>,
}

pub(crate) struct ChartHouseStyleInput<'a> {
    pub(crate) style: Option<&'a str>,
    pub(crate) number_format: Option<&'a str>,
    pub(crate) data_labels: bool,
    pub(crate) series_count: usize,
    pub(crate) categories: &'a [String],
    pub(crate) value_formats: &'a [String],
    pub(crate) values: &'a [String],
    pub(crate) chart_width_points: f64,
}

pub(crate) fn resolve_chart_house_style(
    input: ChartHouseStyleInput<'_>,
) -> CliResult<ChartHouseStyle> {
    let variant = ChartStyleVariant::parse(input.style)?;
    let explicit_format = input.number_format.map(str::trim).unwrap_or_default();
    if input.number_format.is_some() && explicit_format.is_empty() {
        return Err(CliError::invalid_args("--number-format must not be empty"));
    }
    Ok(ChartHouseStyle {
        variant,
        series_scheme_colors: (0..input.series_count)
            .map(|index| ACCENTS[index % ACCENTS.len()])
            .collect(),
        value_number_format: if explicit_format.is_empty() {
            infer_axis_number_format(input.value_formats, input.values)
        } else {
            explicit_format.to_string()
        },
        major_value_gridlines: variant != ChartStyleVariant::Minimal,
        legend_position: (input.series_count > 1).then_some("b"),
        data_labels: input.data_labels,
        category_label_rotation: category_label_rotation(
            input.categories,
            input.chart_width_points,
            variant,
        ),
    })
}

pub(crate) fn resolved_chart_title(explicit: Option<&str>, series_headers: &[String]) -> String {
    if let Some(explicit) = explicit {
        return explicit.trim().to_string();
    }
    series_headers
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn infer_axis_number_format(formats: &[String], values: &[String]) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    for format in formats {
        let format = format.trim();
        if !format.is_empty() && !format.eq_ignore_ascii_case("general") {
            *counts.entry(format.to_string()).or_default() += 1;
        }
    }
    if !counts.is_empty() {
        let mut candidates = counts.into_iter().collect::<Vec<_>>();
        candidates.sort_by(|(left_format, left_count), (right_format, right_count)| {
            right_count
                .cmp(left_count)
                .then_with(|| left_format.cmp(right_format))
        });
        return candidates.remove(0).0;
    }

    let numbers = values
        .iter()
        .filter_map(|value| value.trim().parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if numbers.is_empty()
        || numbers
            .iter()
            .all(|value| (value.round() - value).abs() <= 1e-9)
    {
        "#,##0".to_string()
    } else {
        "#,##0.00".to_string()
    }
}

fn category_label_rotation(
    categories: &[String],
    chart_width_points: f64,
    variant: ChartStyleVariant,
) -> Option<i32> {
    if categories.len() < 2 || !chart_width_points.is_finite() || chart_width_points <= 0.0 {
        return None;
    }
    let available_per_label = chart_width_points * 0.82 / categories.len() as f64;
    let widest = categories
        .iter()
        .map(|label| {
            crate::text_metrics::measure_text_width_emu(label, "Aptos", 10.0, false) as f64
                / EMU_PER_POINT
        })
        .fold(0.0, f64::max);
    let density = match variant {
        ChartStyleVariant::Minimal => 1.05,
        ChartStyleVariant::Default => 0.95,
        ChartStyleVariant::Dense => 0.82,
    };
    (widest > available_per_label * density).then_some(-2_700_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accents_cycle_in_theme_order_and_legend_depends_on_series_count() {
        let categories = vec!["A".to_string(), "B".to_string()];
        let style = resolve_chart_house_style(ChartHouseStyleInput {
            style: None,
            number_format: None,
            data_labels: false,
            series_count: 8,
            categories: &categories,
            value_formats: &[],
            values: &["1".to_string(), "2".to_string()],
            chart_width_points: 400.0,
        })
        .expect("house style");
        assert_eq!(
            style.series_scheme_colors,
            [
                "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "accent1",
                "accent2"
            ]
        );
        assert_eq!(style.legend_position, Some("b"));

        let single = resolve_chart_house_style(ChartHouseStyleInput {
            style: None,
            number_format: None,
            data_labels: false,
            series_count: 1,
            categories: &categories,
            value_formats: &[],
            values: &[],
            chart_width_points: 400.0,
        })
        .expect("single-series style");
        assert_eq!(single.legend_position, None);
    }

    #[test]
    fn source_format_majority_wins_and_explicit_format_overrides() {
        assert_eq!(
            infer_axis_number_format(&["$#,##0.00".into(), "$#,##0.00".into(), "0%".into()], &[]),
            "$#,##0.00"
        );
        assert_eq!(infer_axis_number_format(&["0%".into()], &[]), "0%");
        assert_eq!(
            infer_axis_number_format(&["yyyy-mm-dd".into()], &[]),
            "yyyy-mm-dd"
        );
        let style = resolve_chart_house_style(ChartHouseStyleInput {
            style: Some("minimal"),
            number_format: Some("0.0%"),
            data_labels: true,
            series_count: 1,
            categories: &[],
            value_formats: &["$#,##0.00".into()],
            values: &["0.5".into()],
            chart_width_points: 300.0,
        })
        .expect("explicit format");
        assert_eq!(style.value_number_format, "0.0%");
        assert!(!style.major_value_gridlines);
        assert!(style.data_labels);
    }

    #[test]
    fn integer_decimal_and_overlap_inference_is_deterministic() {
        assert_eq!(
            infer_axis_number_format(&[], &["1".into(), "2000".into()]),
            "#,##0"
        );
        assert_eq!(
            infer_axis_number_format(&[], &["1.25".into(), "2000".into()]),
            "#,##0.00"
        );
        let long_categories = (0..12)
            .map(|index| format!("Long category label {index}"))
            .collect::<Vec<_>>();
        let style = resolve_chart_house_style(ChartHouseStyleInput {
            style: Some("dense"),
            number_format: None,
            data_labels: false,
            series_count: 2,
            categories: &long_categories,
            value_formats: &[],
            values: &[],
            chart_width_points: 300.0,
        })
        .expect("dense style");
        assert_eq!(style.category_label_rotation, Some(-2_700_000));
        assert_eq!(style.variant.as_str(), "dense");
    }

    #[test]
    fn blank_override_and_unknown_style_are_refused() {
        for (style, format) in [(Some("loud"), None), (None, Some("  "))] {
            let error = resolve_chart_house_style(ChartHouseStyleInput {
                style,
                number_format: format,
                data_labels: false,
                series_count: 1,
                categories: &[],
                value_formats: &[],
                values: &[],
                chart_width_points: 300.0,
            })
            .expect_err("invalid house style");
            assert_eq!(error.code, "invalid_args");
        }
    }

    #[test]
    fn title_defaults_to_first_nonempty_series_header() {
        let headers = vec!["".into(), " Revenue ".into(), "Units".into()];
        assert_eq!(resolved_chart_title(None, &headers), "Revenue");
        assert_eq!(
            resolved_chart_title(Some(" Explicit "), &headers),
            "Explicit"
        );
        assert_eq!(resolved_chart_title(Some(""), &headers), "");
    }
}
