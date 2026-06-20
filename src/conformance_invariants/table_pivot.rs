use quick_xml::NsReader;
use quick_xml::events::BytesStart;
use quick_xml::events::Event;
use quick_xml::name::{Namespace, ResolveResult};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::{CliResult, col_name, local_name, xml_attrs, zip_text};

use super::spec::{
    CONTENT_TYPE_XLSX_PIVOT_CACHE, CONTENT_TYPE_XLSX_PIVOT_RECORDS, CONTENT_TYPE_XLSX_PIVOT_TABLE,
    CONTENT_TYPE_XLSX_TABLE, SPREADSHEETML_NAMESPACE,
};
use super::types::PartInfo;
use super::util::diag;

const MAX_COLUMN: u32 = 16_384;
const MAX_ROW: u32 = 1_048_576;

#[derive(Clone, Default)]
struct StructuralNode {
    local_name: String,
    namespace: String,
    attrs: BTreeMap<String, String>,
    parent: Option<usize>,
}

#[derive(Default)]
struct StructuralXmlPart {
    root: Option<usize>,
    nodes: Vec<StructuralNode>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CellRef {
    column: u32,
    row: u32,
    abs_column: bool,
    abs_row: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RangeRef {
    start: CellRef,
    end: CellRef,
}

pub(super) fn check_table_pivot_invariants(file: &str, part: &PartInfo) -> CliResult<Vec<Value>> {
    let expected_root = match part.content_type.as_str() {
        CONTENT_TYPE_XLSX_TABLE => Some("table"),
        CONTENT_TYPE_XLSX_PIVOT_TABLE => Some("pivotTableDefinition"),
        CONTENT_TYPE_XLSX_PIVOT_CACHE => Some("pivotCacheDefinition"),
        CONTENT_TYPE_XLSX_PIVOT_RECORDS => Some("pivotCacheRecords"),
        _ => None,
    };
    let Some(expected_root) = expected_root else {
        return Ok(Vec::new());
    };

    let Ok(info) = read_structural_xml_part(file, part) else {
        return Ok(Vec::new());
    };
    let Some(root) = root_node(&info) else {
        return Ok(Vec::new());
    };
    if root.local_name != expected_root || root.namespace != SPREADSHEETML_NAMESPACE {
        return Ok(Vec::new());
    }

    let root_id = info.root.expect("root checked above");
    let diagnostics = match part.content_type.as_str() {
        CONTENT_TYPE_XLSX_TABLE => check_table_definition(&part.uri, &info, root_id),
        CONTENT_TYPE_XLSX_PIVOT_TABLE => check_pivot_table_definition(&part.uri, &info, root_id),
        CONTENT_TYPE_XLSX_PIVOT_CACHE => check_pivot_cache_definition(&part.uri, &info, root_id),
        CONTENT_TYPE_XLSX_PIVOT_RECORDS => {
            check_pivot_records_definition(&part.uri, &info, root_id)
        }
        _ => Vec::new(),
    };
    Ok(diagnostics)
}

fn read_structural_xml_part(file: &str, part: &PartInfo) -> CliResult<StructuralXmlPart> {
    let xml = zip_text(file, &part.entry_name)?;
    let mut reader = NsReader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut info = StructuralXmlPart::default();
    let mut stack = Vec::<usize>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let id = push_node(&mut info, &e, &reader, stack.last().copied());
                stack.push(id);
            }
            Ok(Event::Empty(e)) => {
                push_node(&mut info, &e, &reader, stack.last().copied());
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(crate::CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(info)
}

fn push_node(
    info: &mut StructuralXmlPart,
    element: &BytesStart<'_>,
    reader: &NsReader<&[u8]>,
    parent: Option<usize>,
) -> usize {
    let id = info.nodes.len();
    if parent.is_none() && info.root.is_none() {
        info.root = Some(id);
    }
    info.nodes.push(StructuralNode {
        local_name: local_name(element.name().as_ref()).to_string(),
        namespace: element_namespace(element, reader),
        attrs: xml_attrs(element),
        parent,
    });
    id
}

fn element_namespace(element: &BytesStart<'_>, reader: &NsReader<&[u8]>) -> String {
    match reader.resolver().resolve_element(element.name()) {
        (ResolveResult::Bound(Namespace(uri)), _) => String::from_utf8_lossy(uri).to_string(),
        _ => String::new(),
    }
}

fn check_table_definition(part_uri: &str, info: &StructuralXmlPart, root_id: usize) -> Vec<Value> {
    let root = &info.nodes[root_id];
    let mut diagnostics = check_ordered_container(
        part_uri,
        info,
        root_id,
        table_child_order,
        "XLSX_TABLE_CHILD_ORDER",
    );

    let raw_id = attr_trim(root, "id");
    if raw_id.is_empty() {
        diagnostics.push(diag(
            "XLSX_TABLE_DEFINITION",
            format!("{part_uri} <table> is missing required id"),
        ));
    } else if raw_id.parse::<i64>().map_or(true, |id| id <= 0) {
        diagnostics.push(diag(
            "XLSX_TABLE_DEFINITION",
            format!("{part_uri} <table> has invalid id {raw_id:?}"),
        ));
    }

    if attr_trim(root, "name").is_empty() {
        diagnostics.push(diag(
            "XLSX_TABLE_DEFINITION",
            format!("{part_uri} <table> is missing required name"),
        ));
    }
    if attr_trim(root, "displayName").is_empty() {
        diagnostics.push(diag(
            "XLSX_TABLE_DEFINITION",
            format!("{part_uri} <table> is missing required displayName"),
        ));
    }

    let ref_text = attr_trim(root, "ref");
    let mut table_ref = None;
    if ref_text.is_empty() {
        diagnostics.push(diag(
            "XLSX_TABLE_DEFINITION",
            format!("{part_uri} <table> is missing required ref"),
        ));
    } else {
        match parse_range_ref(&ref_text) {
            Ok(parsed) => {
                if parsed.start.column > parsed.end.column || parsed.start.row > parsed.end.row {
                    diagnostics.push(diag(
                        "XLSX_TABLE_DEFINITION",
                        format!(
                            "{part_uri} <table> ref {ref_text:?} is not top-left to bottom-right"
                        ),
                    ));
                }
                table_ref = Some(parsed);
            }
            Err(err) => diagnostics.push(diag(
                "XLSX_TABLE_DEFINITION",
                format!("{part_uri} <table> has invalid ref {ref_text:?}: {err}"),
            )),
        }
    }

    if let Some(auto_filter) = spreadsheet_child(info, root_id, "autoFilter") {
        let raw = attr_trim(auto_filter, "ref");
        if raw.is_empty() {
            diagnostics.push(diag(
                "XLSX_TABLE_DEFINITION",
                format!("{part_uri} <autoFilter> is missing required ref"),
            ));
        } else {
            match parse_range_ref(&raw) {
                Ok(parsed) => {
                    let parsed_string = range_ref_string(&parsed);
                    if let Some(table_ref_string) = table_ref
                        .as_ref()
                        .map(range_ref_string)
                        .filter(|table_ref_string| *table_ref_string != parsed_string)
                    {
                        diagnostics.push(diag(
                            "XLSX_TABLE_DEFINITION",
                            format!(
                                "{part_uri} <autoFilter> ref {parsed_string:?} does not match table ref {table_ref_string:?}",
                            ),
                        ));
                    }
                }
                Err(err) => diagnostics.push(diag(
                    "XLSX_TABLE_DEFINITION",
                    format!("{part_uri} <autoFilter> has invalid ref {raw:?}: {err}"),
                )),
            }
        }
    }

    let Some(table_columns) = spreadsheet_child(info, root_id, "tableColumns") else {
        diagnostics.push(diag(
            "XLSX_TABLE_DEFINITION",
            format!("{part_uri} <table> is missing required <tableColumns>"),
        ));
        return diagnostics;
    };
    let table_columns_id = node_id(info, table_columns).expect("node came from part");
    let columns = spreadsheet_children(info, table_columns_id, "tableColumn");
    let raw_count = attr_trim(table_columns, "count");
    if raw_count.is_empty() {
        diagnostics.push(diag(
            "XLSX_TABLE_DEFINITION",
            format!("{part_uri} <tableColumns> is missing required count"),
        ));
    } else if let Ok(count) = raw_count.parse::<i64>() {
        if count != columns.len() as i64 {
            diagnostics.push(diag(
                "XLSX_TABLE_DEFINITION",
                format!(
                    "{part_uri} <tableColumns> count is {count} but contains {} <tableColumn> entries",
                    columns.len()
                ),
            ));
        }
    } else {
        diagnostics.push(diag(
            "XLSX_TABLE_DEFINITION",
            format!("{part_uri} <tableColumns> count {raw_count:?} is not a valid integer"),
        ));
    }

    if let Some(table_ref) = table_ref {
        let width = table_ref.width();
        if width != columns.len() as u32 {
            diagnostics.push(diag(
                "XLSX_TABLE_DEFINITION",
                format!(
                    "{part_uri} table ref spans {width} columns but <tableColumns> contains {} entries",
                    columns.len()
                ),
            ));
        }
    }

    let mut seen_ids = BTreeSet::<i64>::new();
    for (idx, column) in columns.iter().enumerate() {
        let label = table_column_label(idx + 1, column);
        let raw_column_id = attr_trim(column, "id");
        if raw_column_id.is_empty() {
            diagnostics.push(diag(
                "XLSX_TABLE_DEFINITION",
                format!("{part_uri} {label} is missing required id"),
            ));
        } else if let Ok(id) = raw_column_id.parse::<i64>() {
            if id <= 0 {
                diagnostics.push(diag(
                    "XLSX_TABLE_DEFINITION",
                    format!("{part_uri} {label} has invalid id {raw_column_id:?}"),
                ));
            } else if seen_ids.contains(&id) {
                diagnostics.push(diag(
                    "XLSX_TABLE_DEFINITION",
                    format!("{part_uri} {label} duplicates tableColumn id {id}"),
                ));
            } else {
                seen_ids.insert(id);
            }
        } else {
            diagnostics.push(diag(
                "XLSX_TABLE_DEFINITION",
                format!("{part_uri} {label} has invalid id {raw_column_id:?}"),
            ));
        }
        if attr_trim(column, "name").is_empty() {
            diagnostics.push(diag(
                "XLSX_TABLE_DEFINITION",
                format!("{part_uri} {label} is missing required name"),
            ));
        }
    }

    diagnostics
}

fn check_pivot_table_definition(
    part_uri: &str,
    info: &StructuralXmlPart,
    root_id: usize,
) -> Vec<Value> {
    let root = &info.nodes[root_id];
    let mut diagnostics = check_ordered_container(
        part_uri,
        info,
        root_id,
        pivot_table_child_order,
        "XLSX_PIVOT_TABLE_CHILD_ORDER",
    );

    if attr_trim(root, "name").is_empty() {
        diagnostics.push(diag(
            "XLSX_PIVOT_TABLE_DEFINITION",
            format!("{part_uri} <pivotTableDefinition> is missing required name"),
        ));
    }
    let raw_cache_id = attr_trim(root, "cacheId");
    if raw_cache_id.is_empty() {
        diagnostics.push(diag(
            "XLSX_PIVOT_TABLE_DEFINITION",
            format!("{part_uri} <pivotTableDefinition> is missing required cacheId"),
        ));
    } else if raw_cache_id
        .parse::<i64>()
        .map_or(true, |cache_id| cache_id <= 0)
    {
        diagnostics.push(diag(
            "XLSX_PIVOT_TABLE_DEFINITION",
            format!("{part_uri} <pivotTableDefinition> has invalid cacheId {raw_cache_id:?}"),
        ));
    }

    if let Some(location) = spreadsheet_child(info, root_id, "location") {
        let reference = attr_trim(location, "ref");
        if reference.is_empty() {
            diagnostics.push(diag(
                "XLSX_PIVOT_TABLE_DEFINITION",
                format!("{part_uri} <location> is missing required ref"),
            ));
        } else if let Err(err) = parse_range_ref(&reference) {
            diagnostics.push(diag(
                "XLSX_PIVOT_TABLE_DEFINITION",
                format!("{part_uri} <location> has invalid ref {reference:?}: {err}"),
            ));
        }
    } else {
        diagnostics.push(diag(
            "XLSX_PIVOT_TABLE_DEFINITION",
            format!("{part_uri} <pivotTableDefinition> is missing required <location>"),
        ));
    }

    let Some(pivot_fields) = spreadsheet_child(info, root_id, "pivotFields") else {
        diagnostics.push(diag(
            "XLSX_PIVOT_TABLE_DEFINITION",
            format!("{part_uri} <pivotTableDefinition> is missing required <pivotFields>"),
        ));
        return diagnostics;
    };
    let pivot_fields_id = node_id(info, pivot_fields).expect("node came from part");
    let fields = spreadsheet_children(info, pivot_fields_id, "pivotField");
    diagnostics.extend(check_counted_children(
        part_uri,
        info,
        pivot_fields_id,
        "pivotFields",
        "pivotField",
        "XLSX_PIVOT_TABLE_DEFINITION",
    ));
    let field_count = fields.len();

    diagnostics.extend(check_pivot_field_index_collection(
        part_uri,
        info,
        root_id,
        "rowFields",
        "field",
        "x",
        field_count,
    ));
    diagnostics.extend(check_pivot_field_index_collection(
        part_uri,
        info,
        root_id,
        "colFields",
        "field",
        "x",
        field_count,
    ));
    diagnostics.extend(check_pivot_field_index_collection(
        part_uri,
        info,
        root_id,
        "pageFields",
        "pageField",
        "fld",
        field_count,
    ));
    if let Some(data_fields) = spreadsheet_child(info, root_id, "dataFields") {
        let data_fields_id = node_id(info, data_fields).expect("node came from part");
        diagnostics.extend(check_counted_children(
            part_uri,
            info,
            data_fields_id,
            "dataFields",
            "dataField",
            "XLSX_PIVOT_TABLE_DEFINITION",
        ));
        for (idx, data_field) in spreadsheet_children(info, data_fields_id, "dataField")
            .iter()
            .enumerate()
        {
            let label = pivot_child_label("dataField", idx + 1, data_field, "fld");
            diagnostics.extend(check_pivot_field_index_attr(
                part_uri,
                &label,
                data_field,
                "fld",
                field_count,
            ));
            if attr_trim(data_field, "name").is_empty() {
                diagnostics.push(diag(
                    "XLSX_PIVOT_TABLE_DEFINITION",
                    format!("{part_uri} {label} is missing required name"),
                ));
            }
        }
    }

    diagnostics
}

fn check_pivot_cache_definition(
    part_uri: &str,
    info: &StructuralXmlPart,
    root_id: usize,
) -> Vec<Value> {
    let root = &info.nodes[root_id];
    let mut diagnostics = check_ordered_container(
        part_uri,
        info,
        root_id,
        pivot_cache_child_order,
        "XLSX_PIVOT_CACHE_CHILD_ORDER",
    );

    let record_count = attr_trim(root, "recordCount");
    if !record_count.is_empty() && record_count.parse::<i64>().map_or(true, |count| count < 0) {
        diagnostics.push(diag(
            "XLSX_PIVOT_CACHE_DEFINITION",
            format!(
                "{part_uri} <pivotCacheDefinition> recordCount {record_count:?} is not a valid non-negative integer"
            ),
        ));
    }

    if let Some(cache_source) = spreadsheet_child(info, root_id, "cacheSource") {
        let source_type = attr_trim(cache_source, "type");
        if source_type.is_empty() {
            diagnostics.push(diag(
                "XLSX_PIVOT_CACHE_DEFINITION",
                format!("{part_uri} <cacheSource> is missing required type"),
            ));
        }
        if source_type == "worksheet" {
            let cache_source_id = node_id(info, cache_source).expect("node came from part");
            if let Some(worksheet_source) =
                spreadsheet_child(info, cache_source_id, "worksheetSource")
            {
                let reference = attr_trim(worksheet_source, "ref");
                let name = attr_trim(worksheet_source, "name");
                if reference.is_empty() && name.is_empty() {
                    diagnostics.push(diag(
                        "XLSX_PIVOT_CACHE_DEFINITION",
                        format!("{part_uri} <worksheetSource> must define ref or name"),
                    ));
                }
                if !reference.is_empty() {
                    if let Err(err) = parse_range_ref(&reference) {
                        diagnostics.push(diag(
                            "XLSX_PIVOT_CACHE_DEFINITION",
                            format!(
                                "{part_uri} <worksheetSource> has invalid ref {reference:?}: {err}"
                            ),
                        ));
                    }
                    if attr_trim(worksheet_source, "sheet").is_empty() {
                        diagnostics.push(diag(
                            "XLSX_PIVOT_CACHE_DEFINITION",
                            format!(
                                "{part_uri} <worksheetSource> with ref {reference:?} is missing required sheet"
                            ),
                        ));
                    }
                }
            } else {
                diagnostics.push(diag(
                    "XLSX_PIVOT_CACHE_DEFINITION",
                    format!(
                        "{part_uri} worksheet <cacheSource> is missing required <worksheetSource>"
                    ),
                ));
            }
        }
    } else {
        diagnostics.push(diag(
            "XLSX_PIVOT_CACHE_DEFINITION",
            format!("{part_uri} <pivotCacheDefinition> is missing required <cacheSource>"),
        ));
    }

    let Some(cache_fields) = spreadsheet_child(info, root_id, "cacheFields") else {
        diagnostics.push(diag(
            "XLSX_PIVOT_CACHE_DEFINITION",
            format!("{part_uri} <pivotCacheDefinition> is missing required <cacheFields>"),
        ));
        return diagnostics;
    };
    let cache_fields_id = node_id(info, cache_fields).expect("node came from part");
    diagnostics.extend(check_counted_children(
        part_uri,
        info,
        cache_fields_id,
        "cacheFields",
        "cacheField",
        "XLSX_PIVOT_CACHE_DEFINITION",
    ));
    for (idx, field) in spreadsheet_children(info, cache_fields_id, "cacheField")
        .iter()
        .enumerate()
    {
        let label = pivot_child_label("cacheField", idx + 1, field, "name");
        if attr_trim(field, "name").is_empty() {
            diagnostics.push(diag(
                "XLSX_PIVOT_CACHE_DEFINITION",
                format!("{part_uri} {label} is missing required name"),
            ));
        }
    }
    diagnostics
}

fn check_pivot_records_definition(
    part_uri: &str,
    info: &StructuralXmlPart,
    root_id: usize,
) -> Vec<Value> {
    let root = &info.nodes[root_id];
    let records = spreadsheet_children(info, root_id, "r");
    let raw_count = attr_trim(root, "count");
    if raw_count.is_empty() {
        return Vec::new();
    }
    match raw_count.parse::<i64>() {
        Ok(count) if count >= 0 && count == records.len() as i64 => Vec::new(),
        Ok(count) if count >= 0 => vec![diag(
            "XLSX_PIVOT_RECORDS_DEFINITION",
            format!(
                "{part_uri} <pivotCacheRecords> count is {count} but contains {} <r> records",
                records.len()
            ),
        )],
        _ => vec![diag(
            "XLSX_PIVOT_RECORDS_DEFINITION",
            format!(
                "{part_uri} <pivotCacheRecords> count {raw_count:?} is not a valid non-negative integer"
            ),
        )],
    }
}

fn check_pivot_field_index_collection(
    part_uri: &str,
    info: &StructuralXmlPart,
    root_id: usize,
    parent_name: &str,
    child_name: &str,
    attr_name: &str,
    field_count: usize,
) -> Vec<Value> {
    let Some(parent) = spreadsheet_child(info, root_id, parent_name) else {
        return Vec::new();
    };
    let parent_id = node_id(info, parent).expect("node came from part");
    let mut diagnostics = check_counted_children(
        part_uri,
        info,
        parent_id,
        parent_name,
        child_name,
        "XLSX_PIVOT_TABLE_DEFINITION",
    );
    for (idx, child) in spreadsheet_children(info, parent_id, child_name)
        .iter()
        .enumerate()
    {
        let label = pivot_child_label(child_name, idx + 1, child, attr_name);
        diagnostics.extend(check_pivot_field_index_attr(
            part_uri,
            &label,
            child,
            attr_name,
            field_count,
        ));
    }
    diagnostics
}

fn check_pivot_field_index_attr(
    part_uri: &str,
    label: &str,
    elem: &StructuralNode,
    attr_name: &str,
    field_count: usize,
) -> Vec<Value> {
    let raw = attr_trim(elem, attr_name);
    if raw.is_empty() {
        return vec![diag(
            "XLSX_PIVOT_TABLE_DEFINITION",
            format!("{part_uri} {label} is missing required {attr_name}"),
        )];
    }
    let Ok(index) = raw.parse::<i64>() else {
        return vec![diag(
            "XLSX_PIVOT_TABLE_DEFINITION",
            format!("{part_uri} {label} has invalid {attr_name} {raw:?}"),
        )];
    };
    if index < 0 {
        return vec![diag(
            "XLSX_PIVOT_TABLE_DEFINITION",
            format!("{part_uri} {label} has invalid {attr_name} {raw:?}"),
        )];
    }
    if index >= field_count as i64 {
        return vec![diag(
            "XLSX_PIVOT_TABLE_DEFINITION",
            format!(
                "{part_uri} {label} references pivot field index {index} outside available fields 0..{}",
                field_count as i64 - 1
            ),
        )];
    }
    Vec::new()
}

fn check_counted_children(
    part_uri: &str,
    info: &StructuralXmlPart,
    parent_id: usize,
    parent_name: &str,
    child_name: &str,
    code: &str,
) -> Vec<Value> {
    let parent = &info.nodes[parent_id];
    let actual = spreadsheet_children(info, parent_id, child_name).len();
    let raw_count = attr_trim(parent, "count");
    if raw_count.is_empty() {
        return vec![diag(
            code,
            format!("{part_uri} <{parent_name}> is missing required count"),
        )];
    }
    let Ok(count) = raw_count.parse::<i64>() else {
        return vec![diag(
            code,
            format!(
                "{part_uri} <{parent_name}> count {raw_count:?} is not a valid non-negative integer"
            ),
        )];
    };
    if count < 0 {
        return vec![diag(
            code,
            format!(
                "{part_uri} <{parent_name}> count {raw_count:?} is not a valid non-negative integer"
            ),
        )];
    }
    if count != actual as i64 {
        return vec![diag(
            code,
            format!(
                "{part_uri} <{parent_name}> count is {count} but contains {actual} <{child_name}> entries"
            ),
        )];
    }
    Vec::new()
}

fn check_ordered_container(
    part_uri: &str,
    info: &StructuralXmlPart,
    parent_id: usize,
    order: fn(&str) -> usize,
    code: &str,
) -> Vec<Value> {
    let parent_name = info.nodes[parent_id].local_name.clone();
    let mut diagnostics = Vec::new();
    let mut last_order = 0usize;
    let mut last_name = String::new();
    for child in direct_children(info, parent_id) {
        let current = order(&child.local_name);
        if current == 0 {
            continue;
        }
        if last_order > current {
            diagnostics.push(diag(
                code,
                format!(
                    "{part_uri} <{parent_name}> has <{}> after <{last_name}>; expected schema child order",
                    child.local_name
                ),
            ));
            continue;
        }
        last_order = current;
        last_name = child.local_name.clone();
    }
    diagnostics
}

fn root_node(info: &StructuralXmlPart) -> Option<&StructuralNode> {
    info.root.and_then(|id| info.nodes.get(id))
}

fn direct_children(info: &StructuralXmlPart, parent_id: usize) -> Vec<&StructuralNode> {
    info.nodes
        .iter()
        .filter(|node| node.parent == Some(parent_id))
        .collect()
}

fn spreadsheet_child<'a>(
    info: &'a StructuralXmlPart,
    parent_id: usize,
    name: &str,
) -> Option<&'a StructuralNode> {
    direct_children(info, parent_id)
        .into_iter()
        .find(|node| node.local_name == name && node.namespace == SPREADSHEETML_NAMESPACE)
}

fn spreadsheet_children<'a>(
    info: &'a StructuralXmlPart,
    parent_id: usize,
    name: &str,
) -> Vec<&'a StructuralNode> {
    direct_children(info, parent_id)
        .into_iter()
        .filter(|node| node.local_name == name && node.namespace == SPREADSHEETML_NAMESPACE)
        .collect()
}

fn node_id(info: &StructuralXmlPart, needle: &StructuralNode) -> Option<usize> {
    info.nodes
        .iter()
        .position(|node| std::ptr::eq(node, needle))
}

fn attr_trim(node: &StructuralNode, name: &str) -> String {
    node.attrs
        .get(name)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn table_column_label(position: usize, elem: &StructuralNode) -> String {
    let mut attrs = Vec::new();
    let id = attr_trim(elem, "id");
    if !id.is_empty() {
        attrs.push(format!("id={id:?}"));
    }
    let name = attr_trim(elem, "name");
    if !name.is_empty() {
        attrs.push(format!("name={name:?}"));
    }
    if attrs.is_empty() {
        format!("<tableColumn #{position}>")
    } else {
        format!("<tableColumn #{position} {}>", attrs.join(" "))
    }
}

fn pivot_child_label(
    name: &str,
    position: usize,
    elem: &StructuralNode,
    attr_name: &str,
) -> String {
    let value = attr_trim(elem, attr_name);
    if value.is_empty() {
        format!("<{name} #{position}>")
    } else {
        format!("<{name} #{position} {attr_name}={value:?}>")
    }
}

fn parse_range_ref(value: &str) -> Result<RangeRef, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("range reference cannot be empty".to_string());
    }
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() > 2 {
        return Err(format!("invalid range reference {value:?}"));
    }
    let start = parse_cell_ref(parts[0]).map_err(|err| format!("invalid range start: {err}"))?;
    let end = if parts.len() == 2 {
        if parts[1].trim().is_empty() {
            return Err("range end cannot be empty".to_string());
        }
        parse_cell_ref(parts[1]).map_err(|err| format!("invalid range end: {err}"))?
    } else {
        start
    };
    Ok(RangeRef { start, end })
}

fn parse_cell_ref(value: &str) -> Result<CellRef, String> {
    let mut value = value.trim();
    if value.is_empty() {
        return Err("cell reference cannot be empty".to_string());
    }

    let mut reference = CellRef {
        column: 0,
        row: 0,
        abs_column: false,
        abs_row: false,
    };
    if value.as_bytes()[0] == b'$' {
        reference.abs_column = true;
        value = &value[1..];
        if value.is_empty() {
            return Err("missing column in cell reference".to_string());
        }
    }

    let col_end = value
        .bytes()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    if col_end == 0 {
        return Err("missing column in cell reference".to_string());
    }
    let letters = &value[..col_end];
    reference.column = column_letters_to_index(letters)?;
    value = &value[col_end..];

    if value.is_empty() {
        return Err("missing row in cell reference".to_string());
    }
    if value.as_bytes()[0] == b'$' {
        reference.abs_row = true;
        value = &value[1..];
        if value.is_empty() {
            return Err("missing row in cell reference".to_string());
        }
    }
    if value.contains('$') {
        return Err("invalid absolute marker in row reference".to_string());
    }
    if !value.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("invalid row {value:?} in cell reference"));
    }

    let row = value
        .parse::<i64>()
        .map_err(|err| format!("invalid row {value:?}: {err}"))?;
    if !(1..=i64::from(MAX_ROW)).contains(&row) {
        return Err(format!("row {row} out of XLSX bounds 1-{MAX_ROW}"));
    }
    reference.row = row as u32;
    Ok(reference)
}

fn column_letters_to_index(letters: &str) -> Result<u32, String> {
    let letters = letters.trim();
    if letters.is_empty() {
        return Err("column letters cannot be empty".to_string());
    }
    let mut index = 0u32;
    for mut ch in letters.chars() {
        if ch.is_ascii_lowercase() {
            ch = ch.to_ascii_uppercase();
        }
        if !ch.is_ascii_uppercase() {
            return Err(format!("invalid column letter {ch:?}"));
        }
        index = index * 26 + (ch as u32 - 'A' as u32 + 1);
        if index > MAX_COLUMN {
            return Err(format!("column {letters:?} out of XLSX bounds A-XFD"));
        }
    }
    Ok(index)
}

fn range_ref_string(reference: &RangeRef) -> String {
    let start = cell_ref_string(&reference.start);
    if reference.start == reference.end {
        start
    } else {
        format!("{start}:{}", cell_ref_string(&reference.end))
    }
}

fn cell_ref_string(reference: &CellRef) -> String {
    let mut out = String::new();
    if reference.abs_column {
        out.push('$');
    }
    out.push_str(&col_name(reference.column));
    if reference.abs_row {
        out.push('$');
    }
    out.push_str(&reference.row.to_string());
    out
}

impl RangeRef {
    fn width(self) -> u32 {
        let min = self.start.column.min(self.end.column);
        let max = self.start.column.max(self.end.column);
        max - min + 1
    }
}

fn table_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "autoFilter",
            "sortState",
            "tableColumns",
            "tableStyleInfo",
            "extLst",
        ],
    )
}

fn pivot_table_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "location",
            "pivotFields",
            "rowFields",
            "rowItems",
            "colFields",
            "colItems",
            "pageFields",
            "dataFields",
            "formats",
            "conditionalFormats",
            "chartFormats",
            "pivotHierarchies",
            "pivotTableStyleInfo",
            "filters",
            "rowHierarchiesUsage",
            "colHierarchiesUsage",
            "extLst",
        ],
    )
}

fn pivot_cache_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "cacheSource",
            "cacheFields",
            "cacheHierarchies",
            "kpis",
            "tupleCache",
            "calculatedItems",
            "calculatedMembers",
            "dimensions",
            "measureGroups",
            "maps",
            "extLst",
        ],
    )
}

fn order_index(name: &str, ordered_names: &[&str]) -> usize {
    ordered_names
        .iter()
        .position(|candidate| *candidate == name)
        .map(|idx| idx + 1)
        .unwrap_or(0)
}
