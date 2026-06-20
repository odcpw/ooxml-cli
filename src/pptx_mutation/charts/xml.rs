use super::*;

pub(super) fn parse_chart_xml(xml: &str) -> CliResult<ChartXml> {
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

pub(super) fn parse_xml_tree(xml: &str) -> CliResult<XmlNode> {
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

pub(super) fn node_from_start(e: &BytesStart<'_>) -> XmlNode {
    XmlNode {
        name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
        attrs: xml_attrs_map(e),
        text: String::new(),
        children: Vec::new(),
    }
}

pub(super) fn serialize_xml(root: &XmlNode) -> String {
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>");
    render_node(root, &mut output);
    output
}

pub(super) fn render_node(node: &XmlNode, output: &mut String) {
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

pub(super) fn prefix_for_namespace(root: &XmlNode, namespace: &str) -> Option<String> {
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

pub(super) fn prefix_from_name(name: &str) -> Option<String> {
    name.split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .filter(|prefix| !prefix.is_empty())
}

pub(super) fn qname(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}
