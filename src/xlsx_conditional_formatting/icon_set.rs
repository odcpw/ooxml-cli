use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::{CliError, CliResult, local_name, render_xml_attrs, xml_attrs_map};

use super::color_scale::{ConditionalFormatCfvo, normalize_cfvo};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ConditionalFormatIconSet {
    pub(super) icon_set: String,
    pub(super) cfvo: Vec<ConditionalFormatCfvo>,
    pub(super) show_value: Option<bool>,
    pub(super) percent: Option<bool>,
    pub(super) reverse: Option<bool>,
}

pub(super) fn validate_icon_set(
    icon_set: &str,
    cfvo: &[ConditionalFormatCfvo],
) -> CliResult<ConditionalFormatIconSet> {
    let icon_set = icon_set.trim();
    if icon_set.is_empty() {
        return Err(CliError::invalid_args("--icon-set cannot be empty"));
    }
    let expected = icon_set_cfvo_count(icon_set)?;
    if cfvo.len() != expected {
        return Err(CliError::invalid_args(format!(
            "icon-set {icon_set} conditional formats require exactly {expected} --cfvo values",
        )));
    }
    let cfvo = cfvo
        .iter()
        .cloned()
        .map(normalize_cfvo)
        .collect::<CliResult<Vec<_>>>()?;
    Ok(ConditionalFormatIconSet {
        icon_set: icon_set.to_string(),
        cfvo,
        show_value: None,
        percent: None,
        reverse: None,
    })
}

pub(super) fn parse_icon_set(fragment: &str) -> CliResult<Option<ConditionalFormatIconSet>> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut icon_set_depth = None::<usize>;
    let mut icon_set = None::<ConditionalFormatIconSet>;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                handle_icon_set_element(
                    &name,
                    &e,
                    stack.len(),
                    &mut icon_set_depth,
                    &mut icon_set,
                    false,
                );
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                handle_icon_set_element(
                    &name,
                    &e,
                    stack.len(),
                    &mut icon_set_depth,
                    &mut icon_set,
                    true,
                );
            }
            Ok(Event::End(_)) => {
                if let Some(name) = stack.pop()
                    && name == "iconSet"
                {
                    icon_set_depth = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(icon_set)
}

pub(super) fn icon_set_json(icon_set: &ConditionalFormatIconSet) -> Value {
    let cfvo = icon_set
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
    let mut object = serde_json::Map::new();
    object.insert("iconSet".to_string(), json!(icon_set.icon_set));
    object.insert(
        "cfvo".to_string(),
        if cfvo.is_empty() {
            Value::Null
        } else {
            Value::Array(cfvo)
        },
    );
    if let Some(show_value) = icon_set.show_value {
        object.insert("showValue".to_string(), json!(show_value));
    }
    if let Some(percent) = icon_set.percent {
        object.insert("percent".to_string(), json!(percent));
    }
    if let Some(reverse) = icon_set.reverse {
        object.insert("reverse".to_string(), json!(reverse));
    }
    Value::Object(object)
}

pub(super) fn render_icon_set(prefix: &str, icon_set: &ConditionalFormatIconSet) -> String {
    let icon_set_tag = element_name(prefix, "iconSet");
    let mut attrs = BTreeMap::new();
    attrs.insert("iconSet".to_string(), icon_set.icon_set.clone());
    let mut inner = String::new();
    for cfvo in &icon_set.cfvo {
        let mut attrs = BTreeMap::new();
        attrs.insert("type".to_string(), cfvo.cfvo_type.clone());
        if !cfvo.value.is_empty() {
            attrs.insert("val".to_string(), cfvo.value.clone());
        }
        let tag = element_name(prefix, "cfvo");
        inner.push_str(&format!("<{}{}/>", tag, render_xml_attrs(&attrs)));
    }
    format!(
        "<{}{}>{}</{}>",
        icon_set_tag,
        render_xml_attrs(&attrs),
        inner,
        icon_set_tag
    )
}

fn icon_set_cfvo_count(icon_set: &str) -> CliResult<usize> {
    match icon_set.as_bytes().first().copied() {
        Some(b'3' | b'4' | b'5') => Ok((icon_set.as_bytes()[0] - b'0') as usize),
        _ => Err(CliError::invalid_args(
            "--icon-set must begin with 3, 4, or 5 so the required --cfvo count is unambiguous",
        )),
    }
}

fn handle_icon_set_element(
    name: &str,
    element: &BytesStart<'_>,
    stack_len: usize,
    icon_set_depth: &mut Option<usize>,
    icon_set: &mut Option<ConditionalFormatIconSet>,
    self_closing: bool,
) {
    if name == "iconSet" && icon_set_depth.is_none() {
        let attrs = xml_attrs_map(element);
        *icon_set = Some(ConditionalFormatIconSet {
            icon_set: attr_local(&attrs, "iconSet")
                .unwrap_or_else(|| "3TrafficLights1".to_string()),
            cfvo: Vec::new(),
            show_value: optional_bool_attr(&attrs, "showValue"),
            percent: optional_bool_attr(&attrs, "percent"),
            reverse: optional_bool_attr(&attrs, "reverse"),
        });
        if !self_closing {
            *icon_set_depth = Some(stack_len + 1);
        }
        return;
    }
    if *icon_set_depth != Some(stack_len) {
        return;
    }
    let Some(icon_set) = icon_set.as_mut() else {
        return;
    };
    let attrs = xml_attrs_map(element);
    if name == "cfvo" {
        icon_set.cfvo.push(ConditionalFormatCfvo {
            cfvo_type: attr_local(&attrs, "type").unwrap_or_default(),
            value: attr_local(&attrs, "val").unwrap_or_default(),
        });
    }
}

fn attr_local(attrs: &BTreeMap<String, String>, wanted: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(key, _)| local_name(key.as_bytes()) == wanted)
        .map(|(_, value)| value.clone())
}

fn optional_bool_attr(attrs: &BTreeMap<String, String>, wanted: &str) -> Option<bool> {
    attr_local(attrs, wanted).map(|value| {
        let value = value.trim();
        value == "1" || value.eq_ignore_ascii_case("true")
    })
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
    fn parses_and_renders_icon_set() {
        let xml = r#"<cfRule type="iconSet" priority="4"><iconSet iconSet="3TrafficLights1" showValue="0" percent="0" reverse="1"><cfvo type="percent" val="0"/><cfvo type="percent" val="33"/><cfvo type="percent" val="67"/></iconSet></cfRule>"#;
        let icon_set = parse_icon_set(xml)
            .expect("parse icon set")
            .expect("icon set");
        assert_eq!(icon_set.icon_set, "3TrafficLights1");
        assert_eq!(icon_set.cfvo.len(), 3);
        assert_eq!(icon_set.cfvo[1].cfvo_type, "percent");
        assert_eq!(icon_set.cfvo[1].value, "33");
        assert_eq!(icon_set.show_value, Some(false));
        assert_eq!(icon_set.percent, Some(false));
        assert_eq!(icon_set.reverse, Some(true));

        let normalized = validate_icon_set(
            "3TrafficLights1",
            &[
                parse_cfvo_spec("percent:0").expect("percent 0"),
                parse_cfvo_spec("percent:33").expect("percent 33"),
                parse_cfvo_spec("percent:67").expect("percent 67"),
            ],
        )
        .expect("validate icon set");
        assert_eq!(
            render_icon_set("", &normalized),
            r#"<iconSet iconSet="3TrafficLights1"><cfvo type="percent" val="0"/><cfvo type="percent" val="33"/><cfvo type="percent" val="67"/></iconSet>"#
        );
    }

    #[test]
    fn validates_icon_set_shape_from_leading_digit() {
        let cfvo = [
            parse_cfvo_spec("percent:0").expect("percent 0"),
            parse_cfvo_spec("percent:33").expect("percent 33"),
            parse_cfvo_spec("percent:67").expect("percent 67"),
            parse_cfvo_spec("percent:90").expect("percent 90"),
        ];
        assert!(validate_icon_set("", &cfvo[..3]).is_err());
        assert!(validate_icon_set("2TrafficLights", &cfvo[..3]).is_err());
        assert!(validate_icon_set("3TrafficLights1", &cfvo[..2]).is_err());
        assert!(validate_icon_set("4Arrows", &cfvo[..3]).is_err());
        assert!(validate_icon_set("4Arrows", &cfvo).is_ok());
    }
}
