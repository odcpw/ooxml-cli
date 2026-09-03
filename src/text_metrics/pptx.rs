use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Value, json};
use std::collections::BTreeSet;

use super::{ParagraphMeasure, measure_text_box};
use crate::{
    CliError, CliResult, attr, local_name, parse_i64_flag, parse_string_flag, pptx_shapes_get,
    zip_text,
};

const DEFAULT_HORIZONTAL_INSET: i64 = 91_440;
const DEFAULT_VERTICAL_INSET: i64 = 45_720;
const DEFAULT_BODY_INDENT: i64 = 342_900;

pub(crate) fn pptx_text_measure(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_i64_flag(args, "--slide")?
        .ok_or_else(|| CliError::invalid_args("--slide is required"))?;
    let slide = u32::try_from(slide)
        .ok()
        .filter(|slide| *slide > 0)
        .ok_or_else(|| CliError::invalid_args("--slide must be >= 1"))?;
    let target = parse_string_flag(args, "--target")?
        .filter(|target| !target.trim().is_empty())
        .ok_or_else(|| CliError::invalid_args("--target is required"))?;
    let report = pptx_shapes_get(file, slide, &target, true, true)?;
    let shape = report["shapes"]
        .as_array()
        .and_then(|shapes| shapes.first())
        .ok_or_else(|| CliError::target_not_found(format!("target not found: {target}")))?;
    let bounds = shape["bounds"].as_object().ok_or_else(|| {
        CliError::invalid_args(format!(
            "target {target} has no resolvable bounds; run pptx shapes show --include-bounds"
        ))
    })?;
    let width = bounds
        .get("cx")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| CliError::invalid_args(format!("target {target} has no positive width")))?;
    let height = bounds
        .get("cy")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| CliError::invalid_args(format!("target {target} has no positive height")))?;
    let font_family = presentation_minor_font(file).unwrap_or_else(|| "Aptos".to_string());
    let font_size = inferred_font_size(shape);
    let paragraph_values = shape["paragraphs"].as_array().cloned().unwrap_or_default();
    let owned_text = paragraph_values
        .iter()
        .map(|paragraph| paragraph["text"].as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    let paragraphs = paragraph_values
        .iter()
        .zip(&owned_text)
        .map(|(paragraph, text)| {
            let level = paragraph["level"].as_i64().unwrap_or_default().max(0);
            let bullet = paragraph["bullet"].as_bool().unwrap_or(false);
            ParagraphMeasure {
                text,
                font_family: &font_family,
                font_size_points: font_size,
                bold: false,
                left_indent_emu: if bullet {
                    DEFAULT_BODY_INDENT.saturating_mul(level + 1)
                } else {
                    0
                },
                right_indent_emu: 0,
                first_line_indent_emu: if bullet { -285_750 } else { 0 },
                bullet,
                line_spacing: 1.0,
            }
        })
        .collect::<Vec<_>>();
    let paragraphs = if paragraphs.is_empty() {
        vec![ParagraphMeasure::plain("", &font_family, font_size)]
    } else {
        paragraphs
    };
    let measurement = measure_text_box(
        &paragraphs,
        width,
        height,
        DEFAULT_HORIZONTAL_INSET,
        DEFAULT_HORIZONTAL_INSET,
        DEFAULT_VERTICAL_INSET,
        DEFAULT_VERTICAL_INSET,
    );
    let paragraph_reports = measurement
        .paragraphs
        .iter()
        .enumerate()
        .map(|(index, measured)| {
            json!({
                "index": index,
                "text": paragraphs[index].text,
                "fontFamily": measured.font_family,
                "sourceFontFamily": measured.source_font_family,
                "metricSelection": measured.metric_selection,
                "warning": measured.warning,
                "fontSizePoints": font_size,
                "bold": paragraphs[index].bold,
                "bullet": paragraphs[index].bullet,
                "leftIndentEmu": paragraphs[index].left_indent_emu,
                "firstLineIndentEmu": paragraphs[index].first_line_indent_emu,
                "lineCount": measured.line_count,
                "lineHeightEmu": measured.line_height_emu,
                "estimatedHeightEmu": measured.height_emu,
                "maxLineWidthEmu": measured.max_line_width_emu,
                "unwrappedWidthEmu": measured.unwrapped_width_emu,
                "availableWidthEmu": measured.available_width_emu,
            })
        })
        .collect::<Vec<_>>();
    let warnings = measurement
        .paragraphs
        .iter()
        .filter_map(|paragraph| paragraph.warning.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(json!({
        "file": file,
        "slide": slide,
        "target": target,
        "shapeId": shape["shapeId"],
        "shapeName": shape["shapeName"],
        "primarySelector": shape["primarySelector"],
        "bounds": shape["bounds"],
        "boundsSource": shape["boundsSource"],
        "metricDataVersion": 1,
        "fontFamily": font_family,
        "fontSizePoints": font_size,
        "lineCount": measurement.line_count,
        "estimatedHeightEmu": measurement.height_emu,
        "availableWidthEmu": measurement.available_width_emu,
        "availableHeightEmu": measurement.available_height_emu,
        "overflowsVertically": measurement.overflows_vertically,
        "paragraphs": paragraph_reports,
        "warnings": warnings,
        "limitations": [
            "Uses resolved shape bounds and committed numeric font advances; it does not invoke a platform font renderer.",
            "Run-level size, weight, and typeface inheritance that is absent from readback uses the built-in master defaults.",
            "Complex-script shaping, ligatures, and kerning pairs use deterministic fallback advances.",
        ],
    }))
}

fn inferred_font_size(shape: &Value) -> f64 {
    let selector = shape["primarySelector"].as_str().unwrap_or_default();
    let role = shape["placeholder"]["role"].as_str().unwrap_or_default();
    if matches!(selector, "title" | "ctrTitle") || matches!(role, "title" | "ctrTitle") {
        40.0
    } else if matches!(selector, "subtitle" | "body" | "content")
        || matches!(role, "subtitle" | "body" | "content")
    {
        20.0
    } else {
        18.0
    }
}

fn presentation_minor_font(file: &str) -> Option<String> {
    let xml = zip_text(file, "ppt/theme/theme1.xml").ok()?;
    let mut reader = Reader::from_str(&xml);
    let mut in_minor_font = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) if local_name(element.name().as_ref()) == "minorFont" => {
                in_minor_font = true;
            }
            Ok(Event::End(element)) if local_name(element.name().as_ref()) == "minorFont" => {
                in_minor_font = false;
            }
            Ok(Event::Start(element) | Event::Empty(element))
                if in_minor_font && local_name(element.name().as_ref()) == "latin" =>
            {
                if let Some(typeface) = attr(&element, "typeface")
                    && !typeface.trim().is_empty()
                {
                    return Some(typeface);
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}
