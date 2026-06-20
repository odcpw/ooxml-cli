use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct FormulaCellRef {
    pub(super) column: u32,
    pub(super) row: u32,
    pub(super) abs_column: bool,
    pub(super) abs_row: bool,
}

pub(super) fn normalize_formula_range(range: &str) -> Option<String> {
    let parts = range.trim().split(':').collect::<Vec<_>>();
    if parts.len() > 2 || parts.first()?.trim().is_empty() {
        return None;
    }
    let start = parse_formula_cell(parts[0])?;
    let end = if let Some(end) = parts.get(1) {
        if end.trim().is_empty() {
            return None;
        }
        parse_formula_cell(end)?
    } else {
        start
    };
    if start == end {
        Some(format_formula_cell(start))
    } else {
        Some(format!(
            "{}:{}",
            format_formula_cell(start),
            format_formula_cell(end)
        ))
    }
}

pub(super) fn parse_formula_cell(value: &str) -> Option<FormulaCellRef> {
    let mut rest = value.trim();
    if rest.is_empty() {
        return None;
    }
    let abs_column = rest.starts_with('$');
    if abs_column {
        rest = &rest[1..];
    }
    let col_len = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    if col_len == 0 {
        return None;
    }
    let column = column_letters_to_index(&rest[..col_len])?;
    rest = &rest[col_len..];
    let abs_row = rest.starts_with('$');
    if abs_row {
        rest = &rest[1..];
    }
    if rest.is_empty() || rest.contains('$') || !rest.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let row = rest.parse::<u32>().ok()?;
    if row == 0 || row > 1_048_576 {
        return None;
    }
    Some(FormulaCellRef {
        column,
        row,
        abs_column,
        abs_row,
    })
}

pub(super) fn column_letters_to_index(value: &str) -> Option<u32> {
    let mut index = 0_u32;
    for ch in value.chars() {
        if !ch.is_ascii_alphabetic() {
            return None;
        }
        index = index * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        if index > 16_384 {
            return None;
        }
    }
    Some(index)
}

pub(super) fn format_formula_cell(cell: FormulaCellRef) -> String {
    let mut out = String::new();
    if cell.abs_column {
        out.push('$');
    }
    out.push_str(&column_index_to_letters(cell.column));
    if cell.abs_row {
        out.push('$');
    }
    out.push_str(&cell.row.to_string());
    out
}

pub(super) fn column_index_to_letters(mut index: u32) -> String {
    let mut chars = Vec::new();
    while index > 0 {
        index -= 1;
        chars.push((b'A' + (index % 26) as u8) as char);
        index /= 26;
    }
    chars.iter().rev().collect()
}

pub(super) fn resolve_workbook_target_uri(target: &str) -> String {
    let trimmed = target.trim_start_matches('/');
    if target.starts_with('/') || trimmed.starts_with("xl/") {
        format!("/{trimmed}")
    } else {
        resolve_relationship_target("/xl/workbook.xml", target)
    }
}

pub(super) fn optional_zip_text(file: &str, part: &str) -> CliResult<Option<String>> {
    match zip_text(file, part) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.message.starts_with("missing zip part ") => Ok(None),
        Err(err) => Err(err),
    }
}

pub(super) fn sheet_part_uri_for_chart(
    sheet: &WorkbookSheet,
    workbook_rels: &[RelationshipEntry],
) -> Option<String> {
    workbook_rels
        .iter()
        .find(|rel| rel.id == sheet.rel_id && rel.rel_type == REL_WORKSHEET)
        .map(|rel| resolve_relationship_target("/xl/workbook.xml", &rel.target))
}

pub(super) fn allocate_numbered_package_part(
    entries: &mut BTreeSet<String>,
    prefix: &str,
    suffix: &str,
) -> String {
    let mut number = 1_u32;
    loop {
        let part = format!("{prefix}{number}{suffix}");
        if !entries.contains(part.trim_start_matches('/')) {
            entries.insert(part.trim_start_matches('/').to_string());
            return part;
        }
        number += 1;
    }
}

pub(super) fn part_name(part_uri: &str) -> String {
    part_uri.trim_start_matches('/').to_string()
}

pub(super) fn empty_relationships_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
}

pub(super) fn ensure_chart_xml_namespaces(root: &mut XmlNode) -> ChartXmlContext {
    let chart_prefix = root
        .namespace_prefix_for(NS_CHART)
        .unwrap_or_else(|| prefix_from_qname(&root.qname).unwrap_or("c").to_string());
    let drawing_prefix = root
        .namespace_prefix_for(NS_DRAWING_MAIN)
        .unwrap_or_else(|| "a".to_string());
    if !chart_prefix.is_empty() {
        root.ensure_namespace(&chart_prefix, NS_CHART);
    }
    if !drawing_prefix.is_empty() {
        root.ensure_namespace(&drawing_prefix, NS_DRAWING_MAIN);
    }
    ChartXmlContext {
        chart_prefix,
        drawing_prefix,
    }
}

impl ChartXmlContext {
    pub(super) fn c(&self, local: &str) -> String {
        prefixed_qname(&self.chart_prefix, local)
    }

    pub(super) fn a(&self, local: &str) -> String {
        prefixed_qname(&self.drawing_prefix, local)
    }
}

pub(super) fn prefixed_qname(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

pub(super) fn prefix_from_qname(qname: &str) -> Option<&str> {
    qname.split_once(':').map(|(prefix, _)| prefix)
}

pub(super) fn child_index(node: &XmlNode, name: &str) -> Option<usize> {
    node.children.iter().position(|child| child.name == name)
}

pub(super) fn ensure_child_index(
    parent: &mut XmlNode,
    name: &str,
    qname: String,
    order: &[&str],
) -> usize {
    if let Some(index) = child_index(parent, name) {
        return index;
    }
    insert_child_in_order(parent, XmlNode::new(qname), order);
    child_index(parent, name).expect("inserted child")
}

pub(super) fn set_or_create_val_child(
    parent: &mut XmlNode,
    ctx: &ChartXmlContext,
    name: &str,
    value: &str,
    order: &[&str],
) {
    if let Some(index) = child_index(parent, name) {
        parent.children[index].set_attr("val", value);
        return;
    }
    let mut child = XmlNode::new(ctx.c(name));
    child.set_attr("val", value);
    insert_child_in_order(parent, child, order);
}

pub(super) fn insert_child_in_order(parent: &mut XmlNode, child: XmlNode, order: &[&str]) {
    if let Some(child_rank) = order.iter().position(|name| *name == child.name)
        && let Some(index) = parent.children.iter().position(|existing| {
            order
                .iter()
                .position(|name| *name == existing.name)
                .is_some_and(|rank| rank > child_rank)
        })
    {
        parent.children.insert(index, child);
        return;
    }
    parent.children.push(child);
}

pub(super) fn first_descendant_mut<'a>(
    node: &'a mut XmlNode,
    name: &str,
) -> Option<&'a mut XmlNode> {
    if node.name == name {
        return Some(node);
    }
    for child in &mut node.children {
        if let Some(found) = first_descendant_mut(child, name) {
            return Some(found);
        }
    }
    None
}

pub(super) fn series_node_paths(root: &XmlNode) -> Vec<(usize, usize)> {
    let Some(plot_area) = first_descendant(root, "plotArea") else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for (chart_type_index, chart_type) in plot_area.children.iter().enumerate() {
        if !chart_type.name.ends_with("Chart") {
            continue;
        }
        for (series_index, series) in chart_type.children.iter().enumerate() {
            if series.name == "ser" {
                paths.push((chart_type_index, series_index));
            }
        }
    }
    paths
}

pub(super) fn render_xml_document(root: &XmlNode) -> String {
    let mut out = String::new();
    render_xml_node(root, &mut out);
    out
}

pub(super) fn render_xml_node(node: &XmlNode, out: &mut String) {
    out.push('<');
    out.push_str(&node.qname);
    for attr in &node.raw_attrs {
        out.push(' ');
        out.push_str(&attr.qname);
        out.push_str("=\"");
        out.push_str(&xml_attr_escape(&attr.value));
        out.push('"');
    }
    if node.text.is_empty() && node.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    if !node.text.is_empty() {
        out.push_str(&xml_escape(&node.text));
    }
    for child in &node.children {
        render_xml_node(child, out);
    }
    out.push_str("</");
    out.push_str(&node.qname);
    out.push('>');
}

pub(super) fn parse_xml_node(xml: &str) -> CliResult<XmlNode> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<XmlNode>::new();
    let mut root: Option<XmlNode> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => stack.push(XmlNode::from_start(&e)),
            Ok(Event::Empty(e)) => attach_xml_node(XmlNode::from_start(&e), &mut stack, &mut root)?,
            Ok(Event::Text(e)) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&crate::xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::End(_)) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| CliError::unexpected("malformed XML"))?;
                attach_xml_node(node, &mut stack, &mut root)?;
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(CliError::unexpected("unexpected EOF"));
    }
    root.ok_or_else(|| CliError::unexpected("XML part has no root element"))
}

pub(super) fn attach_xml_node(
    node: XmlNode,
    stack: &mut [XmlNode],
    root: &mut Option<XmlNode>,
) -> CliResult<()> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
        Ok(())
    } else if root.is_none() {
        *root = Some(node);
        Ok(())
    } else {
        Err(CliError::unexpected("XML part has multiple root elements"))
    }
}

impl XmlNode {
    pub(super) fn new(qname: String) -> Self {
        Self {
            name: local_name(qname.as_bytes()).to_string(),
            qname,
            attrs: BTreeMap::new(),
            raw_attrs: Vec::new(),
            text: String::new(),
            children: Vec::new(),
        }
    }

    pub(super) fn from_start(e: &BytesStart<'_>) -> Self {
        let qname = String::from_utf8_lossy(e.name().as_ref()).to_string();
        let mut attrs = BTreeMap::new();
        let mut raw_attrs = Vec::new();
        for attr in e.attributes().with_checks(false).flatten() {
            let attr_qname = String::from_utf8_lossy(attr.key.as_ref()).to_string();
            let local = local_name(attr.key.as_ref()).to_string();
            let value = decode_xml_text(attr.value.as_ref());
            attrs.insert(local.clone(), value.clone());
            raw_attrs.push(XmlAttr {
                qname: attr_qname,
                local,
                value,
            });
        }
        Self {
            qname,
            name: local_name(e.name().as_ref()).to_string(),
            attrs,
            raw_attrs,
            text: String::new(),
            children: Vec::new(),
        }
    }

    pub(super) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }

    pub(super) fn set_attr(&mut self, name: &str, value: &str) {
        let local = local_name(name.as_bytes()).to_string();
        if let Some(attr) = self
            .raw_attrs
            .iter_mut()
            .find(|attr| attr.qname == name || attr.local == local)
        {
            attr.value = value.to_string();
        } else {
            self.raw_attrs.push(XmlAttr {
                qname: name.to_string(),
                local: local.clone(),
                value: value.to_string(),
            });
        }
        self.attrs.insert(local, value.to_string());
    }

    pub(super) fn ensure_namespace(&mut self, prefix: &str, uri: &str) {
        if prefix.is_empty()
            || self.raw_attrs.iter().any(|attr| {
                attr.qname.starts_with("xmlns:")
                    && attr.qname.trim_start_matches("xmlns:") == prefix
                    && attr.value == uri
            })
        {
            return;
        }
        self.set_attr(&format!("xmlns:{prefix}"), uri);
    }

    pub(super) fn namespace_prefix_for(&self, uri: &str) -> Option<String> {
        self.raw_attrs.iter().find_map(|attr| {
            if attr.value != uri {
                return None;
            }
            if attr.qname == "xmlns" {
                Some(String::new())
            } else {
                attr.qname
                    .strip_prefix("xmlns:")
                    .map(|prefix| prefix.to_string())
            }
        })
    }
}

pub(super) fn direct_child<'a>(node: &'a XmlNode, name: &str) -> Option<&'a XmlNode> {
    node.children.iter().find(|child| child.name == name)
}

pub(super) fn first_descendant<'a>(node: &'a XmlNode, name: &str) -> Option<&'a XmlNode> {
    if node.name == name {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| first_descendant(child, name))
}

pub(super) fn descendants<'a>(node: &'a XmlNode, name: &str) -> Vec<&'a XmlNode> {
    let mut out = Vec::new();
    collect_descendants(node, name, &mut out);
    out
}

pub(super) fn collect_descendants<'a>(node: &'a XmlNode, name: &str, out: &mut Vec<&'a XmlNode>) {
    if node.name == name {
        out.push(node);
    }
    for child in &node.children {
        collect_descendants(child, name, out);
    }
}

pub(super) fn parse_child_i64(node: &XmlNode, name: &str) -> i64 {
    direct_child(node, name)
        .map(node_text_trimmed)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
}

pub(super) fn attr_val_i64(node: &XmlNode) -> Option<i64> {
    node.attr("val")?.trim().parse::<i64>().ok()
}

pub(super) fn attr_val_f64(node: &XmlNode) -> Option<f64> {
    node.attr("val")?.trim().parse::<f64>().ok()
}

pub(super) fn node_text(node: &XmlNode) -> String {
    node.text.clone()
}

pub(super) fn node_text_trimmed(node: &XmlNode) -> String {
    node.text.trim().to_string()
}

pub(super) fn parse_ooxml_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on"
    )
}

pub(super) fn axis_kind(element: &str) -> &'static str {
    match element {
        "valAx" => "value",
        "dateAx" => "date",
        "serAx" => "series",
        _ => "category",
    }
}

pub(super) fn insert_nonempty_string(object: &mut Map<String, Value>, key: &str, value: &str) {
    if !value.is_empty() {
        object.insert(key.to_string(), json!(value));
    }
}

pub(super) fn insert_nonempty_string_value(
    object: &mut Map<String, Value>,
    key: &str,
    value: String,
) {
    if !value.is_empty() {
        object.insert(key.to_string(), Value::String(value));
    }
}

pub(super) fn insert_nonzero_i64(object: &mut Map<String, Value>, key: &str, value: i64) {
    if value != 0 {
        object.insert(key.to_string(), json!(value));
    }
}

pub(super) fn insert_nonempty_array(
    object: &mut Map<String, Value>,
    key: &str,
    values: Vec<Value>,
) {
    if !values.is_empty() {
        object.insert(key.to_string(), Value::Array(values));
    }
}

pub(super) fn json_f64(value: f64) -> Value {
    if value.is_finite() && value.fract() == 0.0 {
        json!(value as i64)
    } else {
        json!(value)
    }
}
