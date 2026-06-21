use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::{CliError, CliResult, local_name, render_xml_attrs, xml_attrs_map};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ConditionalFormatCfvo {
    pub(super) cfvo_type: String,
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ConditionalFormatColor {
    pub(super) rgb: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ConditionalFormatColorScale {
    pub(super) cfvo: Vec<ConditionalFormatCfvo>,
    pub(super) colors: Vec<ConditionalFormatColor>,
}

pub(super) fn parse_cfvo_spec(spec: &str) -> CliResult<ConditionalFormatCfvo> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(CliError::invalid_args("--cfvo cannot be empty"));
    }
    let (cfvo_type, value) = spec
        .split_once(':')
        .or_else(|| spec.split_once('='))
        .unwrap_or((spec, ""));
    normalize_cfvo(ConditionalFormatCfvo {
        cfvo_type: cfvo_type.to_string(),
        value: value.to_string(),
    })
}

pub(super) fn validate_color_scale(
    cfvo: &[ConditionalFormatCfvo],
    colors: &[ConditionalFormatColor],
) -> CliResult<ConditionalFormatColorScale> {
    if cfvo.len() != 2 && cfvo.len() != 3 {
        return Err(CliError::invalid_args(
            "color-scale conditional formats require exactly 2 or 3 --cfvo values",
        ));
    }
    if colors.len() != cfvo.len() {
        return Err(CliError::invalid_args(
            "color-scale conditional formats require the same number of --color and --cfvo values",
        ));
    }
    let cfvo = cfvo
        .iter()
        .cloned()
        .map(normalize_cfvo)
        .collect::<CliResult<Vec<_>>>()?;
    let colors = colors
        .iter()
        .map(|color| normalize_color(&color.rgb).map(|rgb| ConditionalFormatColor { rgb }))
        .collect::<CliResult<Vec<_>>>()?;
    Ok(ConditionalFormatColorScale { cfvo, colors })
}

pub(super) fn parse_color_scale(fragment: &str) -> CliResult<Option<ConditionalFormatColorScale>> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut color_scale_depth = None::<usize>;
    let mut scale = None::<ConditionalFormatColorScale>;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                handle_color_scale_element(
                    &name,
                    &e,
                    stack.len(),
                    &mut color_scale_depth,
                    &mut scale,
                    false,
                );
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                handle_color_scale_element(
                    &name,
                    &e,
                    stack.len(),
                    &mut color_scale_depth,
                    &mut scale,
                    true,
                );
            }
            Ok(Event::End(_)) => {
                if let Some(name) = stack.pop()
                    && name == "colorScale"
                {
                    color_scale_depth = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(scale)
}

pub(super) fn color_scale_json(scale: &ConditionalFormatColorScale) -> Value {
    let cfvo = scale
        .cfvo
        .iter()
        .map(|cfvo| {
            let mut object = serde_json::Map::new();
            object.insert("type".to_string(), json!(cfvo.cfvo_type));
            if !cfvo.value.is_empty() {
                object.insert("value".to_string(), json!(cfvo.value));
            }
            Value::Object(object)
        })
        .collect::<Vec<_>>();
    let colors = scale
        .colors
        .iter()
        .map(|color| json!({ "rgb": color.rgb }))
        .collect::<Vec<_>>();
    json!({
        "cfvo": if cfvo.is_empty() { Value::Null } else { Value::Array(cfvo) },
        "colors": if colors.is_empty() { Value::Null } else { Value::Array(colors) },
    })
}

pub(super) fn render_color_scale(prefix: &str, scale: &ConditionalFormatColorScale) -> String {
    let color_scale_tag = element_name(prefix, "colorScale");
    let mut inner = String::new();
    for cfvo in &scale.cfvo {
        let mut attrs = BTreeMap::new();
        attrs.insert("type".to_string(), cfvo.cfvo_type.clone());
        if !cfvo.value.is_empty() {
            attrs.insert("val".to_string(), cfvo.value.clone());
        }
        let tag = element_name(prefix, "cfvo");
        inner.push_str(&format!("<{}{}/>", tag, render_xml_attrs(&attrs)));
    }
    for color in &scale.colors {
        let mut attrs = BTreeMap::new();
        attrs.insert("rgb".to_string(), color.rgb.clone());
        let tag = element_name(prefix, "color");
        inner.push_str(&format!("<{}{}/>", tag, render_xml_attrs(&attrs)));
    }
    format!("<{color_scale_tag}>{inner}</{color_scale_tag}>")
}

fn handle_color_scale_element(
    name: &str,
    element: &BytesStart<'_>,
    stack_len: usize,
    color_scale_depth: &mut Option<usize>,
    scale: &mut Option<ConditionalFormatColorScale>,
    self_closing: bool,
) {
    if name == "colorScale" && color_scale_depth.is_none() {
        *scale = Some(ConditionalFormatColorScale {
            cfvo: Vec::new(),
            colors: Vec::new(),
        });
        if !self_closing {
            *color_scale_depth = Some(stack_len + 1);
        }
        return;
    }
    if *color_scale_depth != Some(stack_len) {
        return;
    }
    let Some(scale) = scale.as_mut() else {
        return;
    };
    let attrs = xml_attrs_map(element);
    match name {
        "cfvo" => scale.cfvo.push(ConditionalFormatCfvo {
            cfvo_type: attr_local(&attrs, "type").unwrap_or_default(),
            value: attr_local(&attrs, "val").unwrap_or_default(),
        }),
        "color" => scale.colors.push(ConditionalFormatColor {
            rgb: attr_local(&attrs, "rgb").unwrap_or_default(),
        }),
        _ => {}
    }
}

fn normalize_cfvo(mut cfvo: ConditionalFormatCfvo) -> CliResult<ConditionalFormatCfvo> {
    cfvo.cfvo_type = cfvo.cfvo_type.trim().to_string();
    cfvo.value = cfvo.value.trim().to_string();
    match cfvo.cfvo_type.to_ascii_lowercase().as_str() {
        "min" => cfvo.cfvo_type = "min".to_string(),
        "max" => cfvo.cfvo_type = "max".to_string(),
        "num" => cfvo.cfvo_type = "num".to_string(),
        "percent" => cfvo.cfvo_type = "percent".to_string(),
        "percentile" => cfvo.cfvo_type = "percentile".to_string(),
        _ => {}
    }
    if !matches!(
        cfvo.cfvo_type.as_str(),
        "min" | "max" | "num" | "percent" | "percentile"
    ) {
        return Err(CliError::invalid_args(format!(
            "invalid --cfvo type {:?} (use min, max, num, percent, or percentile)",
            cfvo.cfvo_type
        )));
    }
    match cfvo.cfvo_type.as_str() {
        "min" | "max" => {
            if !cfvo.value.is_empty() {
                return Err(CliError::invalid_args(format!(
                    "--cfvo {} must not include a value",
                    cfvo.cfvo_type
                )));
            }
        }
        "num" | "percent" | "percentile" => {
            if cfvo.value.is_empty() {
                return Err(CliError::invalid_args(format!(
                    "--cfvo {} requires a numeric value, e.g. {}:50",
                    cfvo.cfvo_type, cfvo.cfvo_type
                )));
            }
            let number = cfvo.value.parse::<f64>().map_err(|_| {
                CliError::invalid_args(format!(
                    "--cfvo {} value {:?} must be a finite number",
                    cfvo.cfvo_type, cfvo.value
                ))
            })?;
            if !number.is_finite() {
                return Err(CliError::invalid_args(format!(
                    "--cfvo {} value {:?} must be a finite number",
                    cfvo.cfvo_type, cfvo.value
                )));
            }
            if matches!(cfvo.cfvo_type.as_str(), "percent" | "percentile")
                && !(0.0..=100.0).contains(&number)
            {
                return Err(CliError::invalid_args(format!(
                    "--cfvo {} value must be between 0 and 100",
                    cfvo.cfvo_type
                )));
            }
        }
        _ => {}
    }
    Ok(cfvo)
}

fn normalize_color(value: &str) -> CliResult<String> {
    let raw = value.trim();
    let color = raw.strip_prefix('#').unwrap_or(raw).to_ascii_uppercase();
    if !color.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(CliError::invalid_args(format!(
            "invalid color {value:?} (use hex like #1A2B3C)"
        )));
    }
    match color.len() {
        6 => Ok(format!("FF{color}")),
        8 => Ok(color),
        _ => Err(CliError::invalid_args(format!(
            "invalid color {value:?} (expected 6 or 8 hex digits)"
        ))),
    }
}

fn attr_local(attrs: &BTreeMap<String, String>, wanted: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(key, _)| local_name(key.as_bytes()) == wanted)
        .map(|(_, value)| value.clone())
}

fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_renders_color_scale() {
        let xml = r#"<cfRule type="colorScale" priority="4"><colorScale><cfvo type="min"/><cfvo type="percentile" val="50"/><cfvo type="max"/><color rgb="FFF8696B"/><color rgb="FFFFEB84"/><color rgb="FF63BE7B"/></colorScale></cfRule>"#;
        let scale = parse_color_scale(xml)
            .expect("parse color scale")
            .expect("color scale");
        assert_eq!(scale.cfvo.len(), 3);
        assert_eq!(scale.cfvo[1].cfvo_type, "percentile");
        assert_eq!(scale.cfvo[1].value, "50");
        assert_eq!(scale.colors[2].rgb, "FF63BE7B");

        let normalized = validate_color_scale(
            &[
                parse_cfvo_spec("min").expect("min"),
                parse_cfvo_spec("percentile:50").expect("percentile"),
                parse_cfvo_spec("max").expect("max"),
            ],
            &[
                ConditionalFormatColor {
                    rgb: "F8696B".to_string(),
                },
                ConditionalFormatColor {
                    rgb: "#FFEB84".to_string(),
                },
                ConditionalFormatColor {
                    rgb: "FF63BE7B".to_string(),
                },
            ],
        )
        .expect("validate color scale");
        assert_eq!(normalized.colors[0].rgb, "FFF8696B");
        assert_eq!(
            render_color_scale("", &normalized),
            r#"<colorScale><cfvo type="min"/><cfvo type="percentile" val="50"/><cfvo type="max"/><color rgb="FFF8696B"/><color rgb="FFFFEB84"/><color rgb="FF63BE7B"/></colorScale>"#
        );
    }

    #[test]
    fn validates_cfvo_and_color_inputs() {
        assert!(parse_cfvo_spec("").is_err());
        assert!(parse_cfvo_spec("min:0").is_err());
        assert!(parse_cfvo_spec("percent:101").is_err());
        assert!(parse_cfvo_spec("num:NaN").is_err());
        assert!(normalize_color("not-a-color").is_err());
        assert!(normalize_color("12345").is_err());
    }
}
