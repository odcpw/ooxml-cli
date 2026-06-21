use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::{CliError, CliResult, local_name, render_xml_attrs, xml_attrs_map};

use super::color_scale::{
    ConditionalFormatCfvo, ConditionalFormatColor, normalize_cfvo, normalize_color,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ConditionalFormatDataBar {
    pub(super) cfvo: Vec<ConditionalFormatCfvo>,
    pub(super) color: Option<ConditionalFormatColor>,
}

pub(super) fn validate_data_bar(
    cfvo: &[ConditionalFormatCfvo],
    colors: &[ConditionalFormatColor],
) -> CliResult<ConditionalFormatDataBar> {
    if cfvo.len() != 2 {
        return Err(CliError::invalid_args(
            "data-bar conditional formats require exactly 2 --cfvo values",
        ));
    }
    if colors.len() != 1 {
        return Err(CliError::invalid_args(
            "data-bar conditional formats require exactly 1 --color value",
        ));
    }
    let cfvo = cfvo
        .iter()
        .cloned()
        .map(normalize_cfvo)
        .collect::<CliResult<Vec<_>>>()?;
    let color = colors
        .first()
        .map(|color| normalize_color(&color.rgb).map(|rgb| ConditionalFormatColor { rgb }))
        .transpose()?;
    Ok(ConditionalFormatDataBar { cfvo, color })
}

pub(super) fn parse_data_bar(fragment: &str) -> CliResult<Option<ConditionalFormatDataBar>> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut data_bar_depth = None::<usize>;
    let mut data_bar = None::<ConditionalFormatDataBar>;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                handle_data_bar_element(
                    &name,
                    &e,
                    stack.len(),
                    &mut data_bar_depth,
                    &mut data_bar,
                    false,
                );
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                handle_data_bar_element(
                    &name,
                    &e,
                    stack.len(),
                    &mut data_bar_depth,
                    &mut data_bar,
                    true,
                );
            }
            Ok(Event::End(_)) => {
                if let Some(name) = stack.pop()
                    && name == "dataBar"
                {
                    data_bar_depth = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(data_bar)
}

pub(super) fn data_bar_json(data_bar: &ConditionalFormatDataBar) -> Value {
    let cfvo = data_bar
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
    let color = data_bar
        .color
        .as_ref()
        .map(|color| json!({ "rgb": color.rgb }))
        .unwrap_or(Value::Null);
    json!({
        "cfvo": Value::Array(cfvo),
        "color": color,
    })
}

pub(super) fn render_data_bar(prefix: &str, data_bar: &ConditionalFormatDataBar) -> String {
    let data_bar_tag = element_name(prefix, "dataBar");
    let mut inner = String::new();
    for cfvo in &data_bar.cfvo {
        let mut attrs = BTreeMap::new();
        attrs.insert("type".to_string(), cfvo.cfvo_type.clone());
        if !cfvo.value.is_empty() {
            attrs.insert("val".to_string(), cfvo.value.clone());
        }
        let tag = element_name(prefix, "cfvo");
        inner.push_str(&format!("<{}{}/>", tag, render_xml_attrs(&attrs)));
    }
    if let Some(color) = data_bar.color.as_ref() {
        let mut attrs = BTreeMap::new();
        attrs.insert("rgb".to_string(), color.rgb.clone());
        let tag = element_name(prefix, "color");
        inner.push_str(&format!("<{}{}/>", tag, render_xml_attrs(&attrs)));
    }
    format!("<{data_bar_tag}>{inner}</{data_bar_tag}>")
}

fn handle_data_bar_element(
    name: &str,
    element: &BytesStart<'_>,
    stack_len: usize,
    data_bar_depth: &mut Option<usize>,
    data_bar: &mut Option<ConditionalFormatDataBar>,
    self_closing: bool,
) {
    if name == "dataBar" && data_bar_depth.is_none() {
        *data_bar = Some(ConditionalFormatDataBar {
            cfvo: Vec::new(),
            color: None,
        });
        if !self_closing {
            *data_bar_depth = Some(stack_len + 1);
        }
        return;
    }
    if *data_bar_depth != Some(stack_len) {
        return;
    }
    let Some(data_bar) = data_bar.as_mut() else {
        return;
    };
    let attrs = xml_attrs_map(element);
    match name {
        "cfvo" => data_bar.cfvo.push(ConditionalFormatCfvo {
            cfvo_type: attr_local(&attrs, "type").unwrap_or_default(),
            value: attr_local(&attrs, "val").unwrap_or_default(),
        }),
        "color" if data_bar.color.is_none() => {
            data_bar.color = Some(ConditionalFormatColor {
                rgb: attr_local(&attrs, "rgb").unwrap_or_default(),
            });
        }
        _ => {}
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
    use super::super::color_scale::parse_cfvo_spec;
    use super::*;

    #[test]
    fn parses_and_renders_data_bar() {
        let xml = r#"<cfRule type="dataBar" priority="2"><dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar></cfRule>"#;
        let data_bar = parse_data_bar(xml)
            .expect("parse data bar")
            .expect("data bar");
        assert_eq!(data_bar.cfvo.len(), 2);
        assert_eq!(data_bar.cfvo[0].cfvo_type, "min");
        assert_eq!(data_bar.cfvo[1].cfvo_type, "max");
        assert_eq!(data_bar.color.as_ref().expect("color").rgb, "FF638EC6");

        let normalized = validate_data_bar(
            &[
                parse_cfvo_spec("min").expect("min"),
                parse_cfvo_spec("percent:80").expect("percent"),
            ],
            &[ConditionalFormatColor {
                rgb: "#638EC6".to_string(),
            }],
        )
        .expect("validate data bar");
        assert_eq!(normalized.color.as_ref().expect("color").rgb, "FF638EC6");
        assert_eq!(
            render_data_bar("", &normalized),
            r#"<dataBar><cfvo type="min"/><cfvo type="percent" val="80"/><color rgb="FF638EC6"/></dataBar>"#
        );
    }

    #[test]
    fn validates_data_bar_shape() {
        let cfvo = [
            parse_cfvo_spec("min").expect("min"),
            parse_cfvo_spec("percentile:50").expect("percentile"),
            parse_cfvo_spec("max").expect("max"),
        ];
        let colors = [
            ConditionalFormatColor {
                rgb: "638EC6".to_string(),
            },
            ConditionalFormatColor {
                rgb: "63C684".to_string(),
            },
        ];
        assert!(validate_data_bar(&cfvo[..1], &colors[..1]).is_err());
        assert!(validate_data_bar(&cfvo, &colors[..1]).is_err());
        assert!(validate_data_bar(&cfvo[..2], &[]).is_err());
        assert!(validate_data_bar(&cfvo[..2], &colors).is_err());
    }
}
