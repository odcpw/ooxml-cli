use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::collections::BTreeMap;

use crate::{CliError, CliResult, RangeBounds, local_name, parse_range, render_xml_attrs};

struct TableStartTag {
    start: usize,
    end: usize,
    name: String,
    attrs: BTreeMap<String, String>,
    self_closing: bool,
}

struct TableRefScanState {
    saw_table: bool,
    replacements: Vec<TableStartTag>,
}

pub(super) fn validate_xlsx_table_append_xml(xml: &str, part_uri: &str) -> CliResult<RangeBounds> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut table_range = None;
    let mut saw_table = false;
    let mut saw_table_columns = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                validate_xlsx_table_append_element(
                    &e,
                    &name,
                    &stack,
                    part_uri,
                    &mut saw_table,
                    &mut saw_table_columns,
                    &mut table_range,
                )?;
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                validate_xlsx_table_append_element(
                    &e,
                    &name,
                    &stack,
                    part_uri,
                    &mut saw_table,
                    &mut saw_table_columns,
                    &mut table_range,
                )?;
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !saw_table {
        return Err(CliError::unexpected(format!(
            "table part {part_uri} root element not found"
        )));
    }
    if !saw_table_columns {
        return Err(CliError::invalid_args(
            "table has unsupported features: missing tableColumns",
        ));
    }
    table_range.ok_or_else(|| CliError::unexpected(format!("table {part_uri} has no ref")))
}

fn validate_xlsx_table_append_element(
    element: &BytesStart<'_>,
    name: &str,
    stack: &[String],
    part_uri: &str,
    saw_table: &mut bool,
    saw_table_columns: &mut bool,
    table_range: &mut Option<RangeBounds>,
) -> CliResult<()> {
    if !*saw_table && stack.is_empty() && name == "table" {
        *saw_table = true;
        if parse_boolish(
            xlsx_attr(element, "totalsRowShown")
                .as_deref()
                .unwrap_or_default(),
        ) || parse_positive_int(
            xlsx_attr(element, "totalsRowCount")
                .as_deref()
                .unwrap_or_default(),
        ) {
            return Err(CliError::invalid_args("table has totals rows"));
        }
        let table_type = xlsx_attr(element, "tableType").unwrap_or_default();
        if !table_type.is_empty() && table_type != "worksheet" {
            return Err(CliError::invalid_args(format!(
                "table has unsupported features: tableType={table_type}"
            )));
        }
        let range_text = xlsx_attr(element, "ref").unwrap_or_default();
        *table_range = Some(parse_range(&range_text).map_err(|err| {
            CliError::unexpected(format!("invalid table ref {range_text:?}: {}", err.message))
        })?);
        return Ok(());
    }
    if stack.len() == 1 && stack[0] == "table" {
        match name {
            "extLst" => {
                return Err(CliError::invalid_args(
                    "table has unsupported features: extLst",
                ));
            }
            "tableColumns" => {
                *saw_table_columns = true;
            }
            _ => {}
        }
    }
    if name == "sortState" && stack.last().is_some_and(|parent| parent == "autoFilter") {
        return Err(CliError::invalid_args(
            "table has unsupported features: sortState",
        ));
    }
    if name == "calculatedColumnFormula" && stack.iter().any(|part| part == "tableColumn") {
        return Err(CliError::invalid_args("table has calculated columns"));
    }
    if !*saw_table && stack.is_empty() {
        return Err(CliError::unexpected(format!(
            "table part {part_uri} root element not found"
        )));
    }
    Ok(())
}

pub(super) fn update_xlsx_table_refs(xml: &str, new_range: &str) -> CliResult<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut state = TableRefScanState {
        saw_table: false,
        replacements: Vec::new(),
    };
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                collect_xlsx_table_ref_replacement(
                    &e,
                    before,
                    reader.buffer_position() as usize,
                    false,
                    &name,
                    &stack,
                    &mut state,
                );
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                collect_xlsx_table_ref_replacement(
                    &e,
                    before,
                    reader.buffer_position() as usize,
                    true,
                    &name,
                    &stack,
                    &mut state,
                );
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    let mut out = xml.to_string();
    state
        .replacements
        .sort_by_key(|replacement| replacement.start);
    for mut replacement in state.replacements.into_iter().rev() {
        replacement
            .attrs
            .insert("ref".to_string(), new_range.to_string());
        let tag = if replacement.self_closing {
            format!(
                "<{}{}{}",
                replacement.name,
                render_xml_attrs(&replacement.attrs),
                "/>"
            )
        } else {
            format!(
                "<{}{}>",
                replacement.name,
                render_xml_attrs(&replacement.attrs)
            )
        };
        out.replace_range(replacement.start..replacement.end, &tag);
    }
    Ok(out)
}

fn collect_xlsx_table_ref_replacement(
    element: &BytesStart<'_>,
    start: usize,
    end: usize,
    self_closing: bool,
    name: &str,
    stack: &[String],
    state: &mut TableRefScanState,
) {
    let is_table_root = !state.saw_table && stack.is_empty() && name == "table";
    let is_direct_auto_filter = stack.len() == 1 && stack[0] == "table" && name == "autoFilter";
    if !is_table_root && !is_direct_auto_filter {
        return;
    }
    if is_table_root {
        state.saw_table = true;
    }
    state.replacements.push(TableStartTag {
        start,
        end,
        name: String::from_utf8_lossy(element.name().as_ref()).to_string(),
        attrs: crate::xml_util::decode_xml_attrs(element),
        self_closing,
    });
}

fn xlsx_attr(element: &BytesStart<'_>, wanted: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attr| {
        if local_name(attr.key.as_ref()) == wanted {
            Some(crate::xml_util::decode_xml_text(attr.value.as_ref()))
        } else {
            None
        }
    })
}

fn parse_boolish(value: &str) -> bool {
    value == "1" || value == "true"
}

fn parse_positive_int(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) && value != "0"
}
