use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::xlsx_tables::{XlsxTableRef, select_xlsx_table, xlsx_tables};
use crate::{
    CliError, CliResult, WorkbookSheet, command_arg, copy_zip_with_part_override, local_name,
    normalize_xl_target, parse_range, range_bounds_ref, relationships, render_xml_attrs,
    replace_xml_span, resolve_sheet, validate, validate_xlsx_mutation_output_flags,
    workbook_sheets, xlsx_ranges_set_temp_path, xml_attr_escape, xml_attrs_map,
    xml_direct_child_ranges, xml_fragment_bounds, xml_open_tag_from_start, xml_tag_prefix,
    zip_text,
};

const FILTERS_SORTS_NOTE: &str = "Note: applying a filter or sort does NOT physically hide or reorder rows in the file. Excel/Calc re-evaluates the autoFilter/sortState when the workbook is opened.";

pub(crate) struct XlsxFiltersSortsSetAutoFilterOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: Option<&'a str>,
    pub(crate) table: Option<&'a str>,
    pub(crate) expect_range: Option<&'a str>,
    pub(crate) expect_range_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
struct XmlRootBounds {
    start: usize,
    open_end: usize,
    close_start: usize,
    end: usize,
    tag_name: String,
    self_closing: bool,
}

#[derive(Clone)]
struct FilterColumnState {
    col_id: i64,
    values: Vec<String>,
    custom_filter: Option<CustomFilterState>,
}

#[derive(Clone)]
struct CustomFilterState {
    and: bool,
    criteria: Vec<CustomFilterCriterionState>,
}

#[derive(Clone)]
struct CustomFilterCriterionState {
    operator: String,
    val: String,
}

#[derive(Clone)]
struct AutoFilterState {
    ref_text: String,
    columns: Vec<FilterColumnState>,
}

#[derive(Clone)]
struct SortConditionState {
    ref_text: String,
    descending: bool,
}

#[derive(Clone)]
struct SortState {
    ref_text: String,
    conditions: Vec<SortConditionState>,
}

pub(crate) fn xlsx_filters_sorts_show(
    file: &str,
    sheet_selector: Option<&str>,
    table_selector: Option<&str>,
) -> CliResult<Value> {
    let use_table = table_selector.is_some_and(|value| !value.trim().is_empty());
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));

    if use_table {
        let target = resolve_filters_sorts_table(file, sheet_selector, table_selector)?;
        result.insert("sheet".to_string(), json!(target.table.sheet));
        result.insert("sheetNumber".to_string(), json!(target.table.sheet_number));
        result.insert("table".to_string(), json!(target.table.display_name));
        result.insert("note".to_string(), json!(FILTERS_SORTS_NOTE));
        if let Some(auto_filter) = read_auto_filter_state(&target.table_xml, "table")? {
            result.insert("autoFilter".to_string(), auto_filter_json(&auto_filter));
        }
        if let Some(sort_state) = read_sort_state(&target.sheet_xml, "worksheet")? {
            result.insert("sortState".to_string(), sort_state_json(&sort_state));
        }
        result.insert(
            "showCommand".to_string(),
            json!(filters_sorts_show_command(
                file,
                None,
                Some(&target.table.display_name)
            )),
        );
    } else {
        let (sheet, _sheet_part, sheet_xml) = resolve_filters_sorts_sheet(file, sheet_selector)?;
        let selector = filters_sorts_sheet_selector(&sheet);
        result.insert("sheet".to_string(), json!(sheet.name));
        result.insert("sheetNumber".to_string(), json!(sheet.position));
        result.insert("note".to_string(), json!(FILTERS_SORTS_NOTE));
        if let Some(auto_filter) = read_auto_filter_state(&sheet_xml, "worksheet")? {
            result.insert("autoFilter".to_string(), auto_filter_json(&auto_filter));
        }
        if let Some(sort_state) = read_sort_state(&sheet_xml, "worksheet")? {
            result.insert("sortState".to_string(), sort_state_json(&sort_state));
        }
        result.insert(
            "setAutoFilterCommand".to_string(),
            json!(format!(
                "ooxml xlsx filters-sorts set-autofilter {} --sheet {} --range <A1:D10> --in-place",
                command_arg(file),
                command_arg(&selector)
            )),
        );
        result.insert(
            "addColumnFilterCommand".to_string(),
            json!(format!(
                "ooxml xlsx filters-sorts add-column-filter {} --sheet {} --column 0 --values <a,b,c> --in-place",
                command_arg(file),
                command_arg(&selector)
            )),
        );
        result.insert(
            "setSortCommand".to_string(),
            json!(format!(
                "ooxml xlsx filters-sorts set-sort {} --sheet {} --ref <A1:D10> --column A --in-place",
                command_arg(file),
                command_arg(&selector)
            )),
        );
        result.insert(
            "showCommand".to_string(),
            json!(filters_sorts_show_command(file, Some(&sheet), None)),
        );
    }

    Ok(Value::Object(result))
}

pub(crate) fn xlsx_filters_sorts_set_autofilter(
    file: &str,
    options: XlsxFiltersSortsSetAutoFilterOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let use_table = options.table.is_some_and(|value| !value.trim().is_empty());
    if !use_table && options.range.is_none_or(|value| value.trim().is_empty()) {
        return Err(CliError::invalid_args(
            "--range is required (or use --table)",
        ));
    }
    if use_table && options.range.is_some_and(|value| !value.trim().is_empty()) {
        return Err(CliError::invalid_args(
            "specify only one of --range or --table",
        ));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let mutation = if use_table {
        let target = resolve_filters_sorts_table(file, options.sheet, options.table)?;
        let range = normalize_filters_sorts_range(&target.table.range)
            .map_err(|err| map_filters_sorts_error("set-autofilter", err))?;
        guard_expect_range(
            read_auto_filter_state(&target.table_xml, "table")?.as_ref(),
            options.expect_range_present,
            options.expect_range,
        )
        .map_err(|err| map_filters_sorts_error("set-autofilter", err))?;
        let (updated_xml, auto_filter) = set_autofilter_in_xml(&target.table_xml, "table", &range)
            .map_err(|err| map_filters_sorts_error("set-autofilter", err))?;
        XlsxFiltersSortsMutationTarget {
            sheet_name: target.table.sheet,
            sheet_number: target.table.sheet_number,
            sheet_id: target.table.sheet_number,
            table_name: Some(target.table.display_name),
            part: target.table_part,
            updated_xml,
            range,
            auto_filter,
        }
    } else {
        let (sheet, sheet_part, sheet_xml) = resolve_filters_sorts_sheet(file, options.sheet)?;
        let range = normalize_filters_sorts_range(options.range.unwrap_or_default())
            .map_err(|err| map_filters_sorts_error("set-autofilter", err))?;
        guard_expect_range(
            read_auto_filter_state(&sheet_xml, "worksheet")?.as_ref(),
            options.expect_range_present,
            options.expect_range,
        )
        .map_err(|err| map_filters_sorts_error("set-autofilter", err))?;
        let (updated_xml, auto_filter) = set_autofilter_in_xml(&sheet_xml, "worksheet", &range)
            .map_err(|err| map_filters_sorts_error("set-autofilter", err))?;
        XlsxFiltersSortsMutationTarget {
            sheet_name: sheet.name,
            sheet_number: sheet.position,
            sheet_id: sheet.sheet_id,
            table_name: None,
            part: sheet_part,
            updated_xml,
            range,
            auto_filter,
        }
    };

    write_filters_sorts_mutation_result(file, "set-autofilter", mutation, options)
}

struct XlsxFiltersSortsTableTarget {
    table: XlsxTableRef,
    table_part: String,
    sheet_xml: String,
    table_xml: String,
}

struct XlsxFiltersSortsMutationTarget {
    sheet_name: String,
    sheet_number: u32,
    sheet_id: u32,
    table_name: Option<String>,
    part: String,
    updated_xml: String,
    range: String,
    auto_filter: AutoFilterState,
}

fn write_filters_sorts_mutation_result(
    file: &str,
    action: &str,
    mutation: XlsxFiltersSortsMutationTarget,
    options: XlsxFiltersSortsSetAutoFilterOptions<'_>,
) -> CliResult<Value> {
    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        xlsx_ranges_set_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };

    copy_zip_with_part_override(file, &readback_path, &mutation.part, &mutation.updated_xml)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(mutation.sheet_name));
    result.insert("sheetNumber".to_string(), json!(mutation.sheet_number));
    if let Some(table_name) = mutation.table_name.as_deref() {
        result.insert("table".to_string(), json!(table_name));
    }
    result.insert("action".to_string(), json!(action));
    result.insert("ref".to_string(), json!(mutation.range));
    result.insert("note".to_string(), json!(FILTERS_SORTS_NOTE));
    result.insert(
        "autoFilter".to_string(),
        auto_filter_json(&mutation.auto_filter),
    );
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    if let Some(commit_path) = commit_path {
        result.insert(
            "validateCommand".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(commit_path)
            )),
        );
        let show_command = if let Some(table_name) = mutation.table_name.as_deref() {
            filters_sorts_show_command(commit_path, None, Some(table_name))
        } else {
            let sheet = WorkbookSheet {
                name: mutation.sheet_name,
                sheet_id: mutation.sheet_id,
                position: mutation.sheet_number,
                rel_id: String::new(),
                state: String::new(),
            };
            filters_sorts_show_command(commit_path, Some(&sheet), None)
        };
        result.insert("showCommand".to_string(), json!(show_command));
    }
    Ok(Value::Object(result))
}

fn resolve_filters_sorts_table(
    file: &str,
    sheet_selector: Option<&str>,
    table_selector: Option<&str>,
) -> CliResult<XlsxFiltersSortsTableTarget> {
    let tables = xlsx_tables(
        file,
        sheet_selector.filter(|value| !value.trim().is_empty()),
    )?;
    let table = select_xlsx_table(&tables, table_selector.unwrap_or_default())?;
    let table_part = table.part_uri.trim_start_matches('/').to_string();
    let sheet_part = table.sheet_part_uri.trim_start_matches('/').to_string();
    let table_xml = zip_text(file, &table_part)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    Ok(XlsxFiltersSortsTableTarget {
        table,
        table_part,
        sheet_xml,
        table_xml,
    })
}

fn resolve_filters_sorts_sheet(
    file: &str,
    sheet_selector: Option<&str>,
) -> CliResult<(WorkbookSheet, String, String)> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    if sheets.is_empty() {
        return Err(CliError::invalid_args("workbook has no sheets"));
    }
    let selector = sheet_selector.unwrap_or_default().trim();
    let sheet = if selector.is_empty() {
        sheets[0].clone()
    } else {
        resolve_sheet(&sheets, selector)?
    };
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    if !sheet_part.starts_with("xl/worksheets/") {
        return Err(CliError::invalid_args(format!(
            "sheet {:?} is not a worksheet",
            sheet.name
        )));
    }
    let sheet_xml = zip_text(file, &sheet_part)?;
    Ok((sheet, sheet_part, sheet_xml))
}

fn normalize_filters_sorts_range(range: &str) -> CliResult<String> {
    let bounds = parse_range(range)
        .map_err(|err| CliError::invalid_args(format!("invalid range: {}", err.message)))?;
    Ok(range_bounds_ref(bounds.normalized()))
}

fn guard_expect_range(
    state: Option<&AutoFilterState>,
    has_expect: bool,
    expect: Option<&str>,
) -> CliResult<()> {
    if !has_expect {
        return Ok(());
    }
    let want = parse_range(expect.unwrap_or_default())
        .map_err(|err| CliError::invalid_args(format!("invalid --expect-range: {}", err.message)))
        .map(|bounds| range_bounds_ref(bounds.normalized()))?;
    let current = state
        .map(|state| state.ref_text.as_str())
        .unwrap_or_default();
    let got = if current.is_empty() {
        String::new()
    } else {
        parse_range(current)
            .map(|bounds| range_bounds_ref(bounds.normalized()))
            .unwrap_or_else(|_| current.to_string())
    };
    if got != want {
        return Err(CliError::invalid_args(format!(
            "range mismatch: expected {want}, found {current:?}"
        )));
    }
    Ok(())
}

fn set_autofilter_in_xml(
    xml: &str,
    root_kind: &str,
    range: &str,
) -> CliResult<(String, AutoFilterState)> {
    let root = xml_root_bounds(xml, root_kind)?;
    let prefix = xml_tag_prefix(&root.tag_name);
    if let Some(existing) = direct_child_range(xml, &root, "autoFilter")? {
        let fragment = &xml[existing.start..existing.end];
        let updated_fragment = replace_element_ref_attr(fragment, range)?;
        let state = parse_auto_filter_fragment(&updated_fragment)?;
        return Ok((
            replace_xml_span(xml, existing.start, existing.end, &updated_fragment),
            state,
        ));
    }

    let child_xml = format!(
        "<{} ref=\"{}\"/>",
        element_name(&prefix, "autoFilter"),
        xml_attr_escape(range)
    );
    let updated = if root_kind == "table" {
        insert_first_child(xml, &root, &child_xml)?
    } else {
        insert_ordered_child(xml, &root, "autoFilter", &child_xml)?
    };
    Ok((
        updated,
        AutoFilterState {
            ref_text: range.to_string(),
            columns: Vec::new(),
        },
    ))
}

fn read_auto_filter_state(xml: &str, root_kind: &str) -> CliResult<Option<AutoFilterState>> {
    let root = xml_root_bounds(xml, root_kind)?;
    let Some(auto_filter) = direct_child_range(xml, &root, "autoFilter")? else {
        return Ok(None);
    };
    parse_auto_filter_fragment(&xml[auto_filter.start..auto_filter.end]).map(Some)
}

fn read_sort_state(xml: &str, root_kind: &str) -> CliResult<Option<SortState>> {
    let root = xml_root_bounds(xml, root_kind)?;
    let Some(sort_state) = direct_child_range(xml, &root, "sortState")? else {
        return Ok(None);
    };
    parse_sort_state_fragment(&xml[sort_state.start..sort_state.end]).map(Some)
}

fn parse_auto_filter_fragment(fragment: &str) -> CliResult<AutoFilterState> {
    let (_, attrs, _, _) = first_element(fragment)?;
    let (_, _, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let mut columns = Vec::new();
    if !self_closing {
        for child in
            xml_direct_child_ranges(fragment, fragment.find('>').unwrap_or(0) + 1, close_start)?
        {
            if child.kind == "filterColumn" {
                columns.push(parse_filter_column_fragment(
                    &fragment[child.start..child.end],
                )?);
            }
        }
    }
    columns.sort_by_key(|column| column.col_id);
    Ok(AutoFilterState {
        ref_text: attr_local(&attrs, "ref").unwrap_or_default(),
        columns,
    })
}

fn parse_filter_column_fragment(fragment: &str) -> CliResult<FilterColumnState> {
    let (_, attrs, _, _) = first_element(fragment)?;
    let mut values = Vec::new();
    let mut custom_filter: Option<CustomFilterState> = None;
    let mut in_filters = false;
    let mut in_custom_filters = false;
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match local_name(e.name().as_ref()) {
                "filters" => in_filters = true,
                "customFilters" => {
                    in_custom_filters = true;
                    custom_filter = Some(CustomFilterState {
                        and: attr_local_start(&e, "and").as_deref() == Some("1"),
                        criteria: Vec::new(),
                    });
                }
                "filter" if in_filters => {
                    values.push(attr_local_start(&e, "val").unwrap_or_default());
                }
                "customFilter" if in_custom_filters => {
                    push_custom_filter_criterion(&mut custom_filter, &e);
                }
                _ => {}
            },
            Ok(Event::Empty(e)) => match local_name(e.name().as_ref()) {
                "filter" if in_filters => {
                    values.push(attr_local_start(&e, "val").unwrap_or_default());
                }
                "customFilters" => {
                    custom_filter = Some(CustomFilterState {
                        and: attr_local_start(&e, "and").as_deref() == Some("1"),
                        criteria: Vec::new(),
                    });
                }
                "customFilter" if in_custom_filters => {
                    push_custom_filter_criterion(&mut custom_filter, &e);
                }
                _ => {}
            },
            Ok(Event::End(e)) => match local_name(e.name().as_ref()) {
                "filters" => in_filters = false,
                "customFilters" => in_custom_filters = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(FilterColumnState {
        col_id: attr_local(&attrs, "colId")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0),
        values,
        custom_filter,
    })
}

fn push_custom_filter_criterion(
    custom_filter: &mut Option<CustomFilterState>,
    element: &BytesStart<'_>,
) {
    let criterion = CustomFilterCriterionState {
        operator: attr_local_start(element, "operator").unwrap_or_else(|| "equal".to_string()),
        val: attr_local_start(element, "val").unwrap_or_default(),
    };
    custom_filter
        .get_or_insert_with(|| CustomFilterState {
            and: false,
            criteria: Vec::new(),
        })
        .criteria
        .push(criterion);
}

fn parse_sort_state_fragment(fragment: &str) -> CliResult<SortState> {
    let (_, attrs, _, _) = first_element(fragment)?;
    let (_, _, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let mut conditions = Vec::new();
    if !self_closing {
        for child in
            xml_direct_child_ranges(fragment, fragment.find('>').unwrap_or(0) + 1, close_start)?
        {
            if child.kind == "sortCondition" {
                let (_, attrs, _, _) = first_element(&fragment[child.start..child.end])?;
                conditions.push(SortConditionState {
                    ref_text: attr_local(&attrs, "ref").unwrap_or_default(),
                    descending: attr_local(&attrs, "descending").as_deref() == Some("1"),
                });
            }
        }
    }
    Ok(SortState {
        ref_text: attr_local(&attrs, "ref").unwrap_or_default(),
        conditions,
    })
}

fn auto_filter_json(state: &AutoFilterState) -> Value {
    let mut object = Map::new();
    object.insert("ref".to_string(), json!(state.ref_text));
    if !state.columns.is_empty() {
        object.insert(
            "columns".to_string(),
            Value::Array(state.columns.iter().map(filter_column_json).collect()),
        );
    }
    Value::Object(object)
}

fn filter_column_json(column: &FilterColumnState) -> Value {
    let mut object = Map::new();
    object.insert("colId".to_string(), json!(column.col_id));
    if !column.values.is_empty() {
        object.insert("values".to_string(), json!(column.values));
    }
    if let Some(custom_filter) = column.custom_filter.as_ref() {
        object.insert(
            "customFilter".to_string(),
            json!({
                "and": custom_filter.and,
                "criteria": custom_filter.criteria.iter().map(|criterion| {
                    let mut item = Map::new();
                    if !criterion.operator.is_empty() {
                        item.insert("operator".to_string(), json!(criterion.operator));
                    }
                    item.insert("val".to_string(), json!(criterion.val));
                    Value::Object(item)
                }).collect::<Vec<_>>(),
            }),
        );
    }
    Value::Object(object)
}

fn sort_state_json(state: &SortState) -> Value {
    let mut object = Map::new();
    object.insert("ref".to_string(), json!(state.ref_text));
    if !state.conditions.is_empty() {
        object.insert(
            "conditions".to_string(),
            Value::Array(
                state
                    .conditions
                    .iter()
                    .map(|condition| {
                        json!({
                            "ref": condition.ref_text,
                            "descending": condition.descending,
                        })
                    })
                    .collect(),
            ),
        );
    }
    Value::Object(object)
}

fn xml_root_bounds(xml: &str, expected_local: &str) -> CliResult<XmlRootBounds> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == expected_local => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let close_tag = format!("</{tag_name}>");
                let close_start = xml.rfind(&close_tag).ok_or_else(|| {
                    CliError::unexpected(format!("{expected_local} root has no closing tag"))
                })?;
                return Ok(XmlRootBounds {
                    start: before,
                    open_end: reader.buffer_position() as usize,
                    close_start,
                    end: close_start + close_tag.len(),
                    tag_name,
                    self_closing: false,
                });
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == expected_local => {
                let end = reader.buffer_position() as usize;
                return Ok(XmlRootBounds {
                    start: before,
                    open_end: end,
                    close_start: end,
                    end,
                    tag_name: String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    self_closing: true,
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                return Err(CliError::unexpected(format!(
                    "{expected_local} root is {:?}",
                    local_name(e.name().as_ref())
                )));
            }
            Ok(Event::Eof) => {
                return Err(CliError::unexpected(format!(
                    "{expected_local} root not found"
                )));
            }
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn direct_child_range(
    xml: &str,
    root: &XmlRootBounds,
    kind: &str,
) -> CliResult<Option<crate::XmlNamedRange>> {
    Ok(
        xml_direct_child_ranges(xml, root.open_end, root.close_start)?
            .into_iter()
            .find(|child| child.kind == kind),
    )
}

fn insert_ordered_child(
    xml: &str,
    root: &XmlRootBounds,
    local_name: &str,
    child_xml: &str,
) -> CliResult<String> {
    if root.self_closing {
        let start_tag = xml_open_tag_from_start(&xml[root.start..root.open_end]);
        let mut updated = String::new();
        updated.push_str(&xml[..root.start]);
        updated.push_str(&start_tag);
        updated.push_str(child_xml);
        updated.push_str(&format!("</{}>", root.tag_name));
        updated.push_str(&xml[root.end..]);
        return Ok(updated);
    }
    let target_order = worksheet_child_order(local_name);
    let insert_at = xml_direct_child_ranges(xml, root.open_end, root.close_start)?
        .into_iter()
        .find(|child| worksheet_child_order(&child.kind) > target_order)
        .map(|child| child.start)
        .unwrap_or(root.close_start);
    let mut updated = String::new();
    updated.push_str(&xml[..insert_at]);
    updated.push_str(child_xml);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
}

fn insert_first_child(xml: &str, root: &XmlRootBounds, child_xml: &str) -> CliResult<String> {
    if root.self_closing {
        let start_tag = xml_open_tag_from_start(&xml[root.start..root.open_end]);
        let mut updated = String::new();
        updated.push_str(&xml[..root.start]);
        updated.push_str(&start_tag);
        updated.push_str(child_xml);
        updated.push_str(&format!("</{}>", root.tag_name));
        updated.push_str(&xml[root.end..]);
        return Ok(updated);
    }
    let insert_at = xml_direct_child_ranges(xml, root.open_end, root.close_start)?
        .into_iter()
        .next()
        .map(|child| child.start)
        .unwrap_or(root.close_start);
    let mut updated = String::new();
    updated.push_str(&xml[..insert_at]);
    updated.push_str(child_xml);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
}

fn replace_element_ref_attr(fragment: &str, range: &str) -> CliResult<String> {
    let (tag_name, mut attrs, self_closing, open_end) = first_element(fragment)?;
    attrs.insert("ref".to_string(), range.to_string());
    let tag = if self_closing {
        format!("<{}{}{}>", tag_name, render_xml_attrs(&attrs), "/")
    } else {
        format!("<{}{}>", tag_name, render_xml_attrs(&attrs))
    };
    Ok(replace_xml_span(fragment, 0, open_end, &tag))
}

fn first_element(fragment: &str) -> CliResult<(String, BTreeMap<String, String>, bool, usize)> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let end = reader.buffer_position() as usize;
                return Ok((
                    String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    xml_attrs_map(&e),
                    false,
                    end,
                ));
            }
            Ok(Event::Empty(e)) => {
                let end = reader.buffer_position() as usize;
                return Ok((
                    String::from_utf8_lossy(e.name().as_ref()).to_string(),
                    xml_attrs_map(&e),
                    true,
                    end,
                ));
            }
            Ok(Event::Eof) => return Err(CliError::unexpected("XML element not found")),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn attr_local(attrs: &BTreeMap<String, String>, wanted: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(key, _)| local_name(key.as_bytes()) == wanted)
        .map(|(_, value)| value.clone())
}

fn attr_local_start(element: &BytesStart<'_>, wanted: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attr| {
        if local_name(attr.key.as_ref()) == wanted {
            Some(crate::decode_xml_text(attr.value.as_ref()))
        } else {
            None
        }
    })
}

fn element_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn filters_sorts_sheet_selector(sheet: &WorkbookSheet) -> String {
    if sheet.sheet_id > 0 {
        format!("sheetId:{}", sheet.sheet_id)
    } else if !sheet.name.is_empty() {
        sheet.name.clone()
    } else if sheet.position > 0 {
        format!("sheet:{}", sheet.position)
    } else {
        "1".to_string()
    }
}

fn filters_sorts_show_command(
    file: &str,
    sheet: Option<&WorkbookSheet>,
    table: Option<&str>,
) -> String {
    if let Some(table) = table {
        return format!(
            "ooxml --json xlsx filters-sorts show {} --table {}",
            command_arg(file),
            command_arg(table)
        );
    }
    let selector = sheet
        .map(filters_sorts_sheet_selector)
        .unwrap_or_else(|| "1".to_string());
    format!(
        "ooxml --json xlsx filters-sorts show {} --sheet {}",
        command_arg(file),
        command_arg(&selector)
    )
}

fn map_filters_sorts_error(action: &str, err: CliError) -> CliError {
    if err.code == "invalid_args" {
        CliError::invalid_args(format!("failed to {action}: {}", err.message))
    } else {
        err
    }
}

fn worksheet_child_order(local_name: &str) -> i32 {
    match local_name {
        "sheetPr" => 10,
        "dimension" => 20,
        "sheetViews" => 30,
        "sheetFormatPr" => 40,
        "cols" => 50,
        "sheetData" => 60,
        "sheetCalcPr" => 70,
        "sheetProtection" => 80,
        "protectedRanges" => 90,
        "scenarios" => 100,
        "autoFilter" => 110,
        "sortState" => 120,
        "dataConsolidate" => 130,
        "customSheetViews" => 140,
        "mergeCells" => 150,
        "phoneticPr" => 160,
        "conditionalFormatting" => 170,
        "dataValidations" => 180,
        "hyperlinks" => 190,
        "printOptions" => 200,
        "pageMargins" => 210,
        "pageSetup" => 220,
        "headerFooter" => 230,
        "rowBreaks" => 240,
        "colBreaks" => 250,
        "customProperties" => 260,
        "cellWatches" => 270,
        "ignoredErrors" => 280,
        "smartTags" => 290,
        "drawing" => 300,
        "legacyDrawing" => 310,
        "legacyDrawingHF" => 320,
        "picture" => 330,
        "oleObjects" => 340,
        "controls" => 350,
        "webPublishItems" => 360,
        "tableParts" => 370,
        "extLst" => 380,
        _ => 1000,
    }
}
