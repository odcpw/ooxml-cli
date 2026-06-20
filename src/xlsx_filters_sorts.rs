use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::xlsx_tables::{XlsxTableRef, select_xlsx_table, xlsx_tables};
use crate::{
    CliError, CliResult, RangeBounds, WorkbookSheet, command_arg, copy_zip_with_part_override,
    local_name, normalize_xl_target, parse_range, range_bounds_ref, relationships,
    render_xml_attrs, replace_xml_span, resolve_sheet, validate,
    validate_xlsx_mutation_output_flags, workbook_sheets, xlsx_ranges_set_temp_path,
    xml_attr_escape, xml_attrs_map, xml_direct_child_ranges, xml_fragment_bounds,
    xml_open_tag_from_start, xml_tag_prefix, zip_text,
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

pub(crate) struct XlsxFiltersSortsClearAutoFilterOptions<'a> {
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

pub(crate) struct XlsxFiltersSortsAddColumnFilterOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) column: i64,
    pub(crate) values: Option<&'a str>,
    pub(crate) custom_op: Option<&'a str>,
    pub(crate) custom_val1: Option<&'a str>,
    pub(crate) custom_val2: Option<&'a str>,
    pub(crate) custom_present: bool,
    pub(crate) expect_filter: Option<&'a str>,
    pub(crate) expect_filter_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxFiltersSortsClearColumnFilterOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) column: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxFiltersSortsSetSortOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) ref_range: Option<&'a str>,
    pub(crate) column: Option<&'a str>,
    pub(crate) descending: bool,
    pub(crate) expect_sort: Option<&'a str>,
    pub(crate) expect_sort_present: bool,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) struct XlsxFiltersSortsClearSortOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone, Copy)]
struct XlsxFiltersSortsOutputOptions<'a> {
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
    in_place: bool,
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
            ref_text: Some(range),
            auto_filter: Some(auto_filter),
            sort_state: None,
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
            ref_text: Some(range),
            auto_filter: Some(auto_filter),
            sort_state: None,
        }
    };

    write_filters_sorts_mutation_result(
        file,
        "set-autofilter",
        mutation,
        XlsxFiltersSortsOutputOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )
}

pub(crate) fn xlsx_filters_sorts_clear_autofilter(
    file: &str,
    options: XlsxFiltersSortsClearAutoFilterOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let _range_hint = options.range;

    let use_table = options.table.is_some_and(|value| !value.trim().is_empty());
    let mutation = if use_table {
        let target = resolve_filters_sorts_table(file, options.sheet, options.table)?;
        guard_expect_range(
            read_auto_filter_state(&target.table_xml, "table")?.as_ref(),
            options.expect_range_present,
            options.expect_range,
        )
        .map_err(|err| map_filters_sorts_error("clear-autofilter", err))?;
        let updated_xml = clear_autofilter_in_xml(&target.table_xml, "table")
            .map_err(|err| map_filters_sorts_error("clear-autofilter", err))?;
        XlsxFiltersSortsMutationTarget {
            sheet_name: target.table.sheet,
            sheet_number: target.table.sheet_number,
            sheet_id: target.table.sheet_number,
            table_name: Some(target.table.display_name),
            part: target.table_part,
            updated_xml,
            ref_text: None,
            auto_filter: None,
            sort_state: None,
        }
    } else {
        let (sheet, sheet_part, sheet_xml) = resolve_filters_sorts_sheet(file, options.sheet)?;
        guard_expect_range(
            read_auto_filter_state(&sheet_xml, "worksheet")?.as_ref(),
            options.expect_range_present,
            options.expect_range,
        )
        .map_err(|err| map_filters_sorts_error("clear-autofilter", err))?;
        let updated_xml = clear_autofilter_in_xml(&sheet_xml, "worksheet")
            .map_err(|err| map_filters_sorts_error("clear-autofilter", err))?;
        XlsxFiltersSortsMutationTarget {
            sheet_name: sheet.name,
            sheet_number: sheet.position,
            sheet_id: sheet.sheet_id,
            table_name: None,
            part: sheet_part,
            updated_xml,
            ref_text: None,
            auto_filter: None,
            sort_state: None,
        }
    };

    write_filters_sorts_mutation_result(
        file,
        "clear-autofilter",
        mutation,
        XlsxFiltersSortsOutputOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )
}

pub(crate) fn xlsx_filters_sorts_add_column_filter(
    file: &str,
    options: XlsxFiltersSortsAddColumnFilterOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let values = parse_filter_values(options.values);
    if values.is_empty() && !options.custom_present {
        return Err(CliError::invalid_args(
            "provide --values and/or --custom-op",
        ));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheet, sheet_part, sheet_xml) = resolve_filters_sorts_sheet(file, options.sheet)?;
    let (updated_xml, auto_filter) = add_column_filter_in_xml(
        &sheet_xml,
        AddColumnFilterXmlSpec {
            col_id: options.column,
            values: &values,
            custom_op: options.custom_op,
            custom_val1: options.custom_val1,
            custom_val2: options.custom_val2,
            custom_present: options.custom_present,
            expect_filter: options.expect_filter,
            expect_filter_present: options.expect_filter_present,
        },
    )
    .map_err(|err| map_filters_sorts_error("add-column-filter", err))?;
    let ref_text = auto_filter.ref_text.clone();
    let mutation = XlsxFiltersSortsMutationTarget {
        sheet_name: sheet.name,
        sheet_number: sheet.position,
        sheet_id: sheet.sheet_id,
        table_name: None,
        part: sheet_part,
        updated_xml,
        ref_text: Some(ref_text),
        auto_filter: Some(auto_filter),
        sort_state: None,
    };

    write_filters_sorts_mutation_result(
        file,
        "add-column-filter",
        mutation,
        XlsxFiltersSortsOutputOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )
}

pub(crate) fn xlsx_filters_sorts_clear_column_filter(
    file: &str,
    options: XlsxFiltersSortsClearColumnFilterOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheet, sheet_part, sheet_xml) = resolve_filters_sorts_sheet(file, options.sheet)?;
    let (updated_xml, auto_filter) = clear_column_filter_in_xml(&sheet_xml, options.column)
        .map_err(|err| map_filters_sorts_error("clear-column-filter", err))?;
    let ref_text = auto_filter.ref_text.clone();
    let mutation = XlsxFiltersSortsMutationTarget {
        sheet_name: sheet.name,
        sheet_number: sheet.position,
        sheet_id: sheet.sheet_id,
        table_name: None,
        part: sheet_part,
        updated_xml,
        ref_text: Some(ref_text),
        auto_filter: Some(auto_filter),
        sort_state: None,
    };

    write_filters_sorts_mutation_result(
        file,
        "clear-column-filter",
        mutation,
        XlsxFiltersSortsOutputOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )
}

pub(crate) fn xlsx_filters_sorts_set_sort(
    file: &str,
    options: XlsxFiltersSortsSetSortOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    if options
        .ref_range
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err(CliError::invalid_args("--ref is required (e.g. A1:D10)"));
    }
    if options.column.is_none_or(|value| value.trim().is_empty()) {
        return Err(CliError::invalid_args(
            "--column is required (a column letter such as A)",
        ));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheet, sheet_part, sheet_xml) = resolve_filters_sorts_sheet(file, options.sheet)?;
    let (updated_xml, sort_state) = set_sort_in_xml(
        &sheet_xml,
        SetSortXmlSpec {
            ref_range: options.ref_range.unwrap_or_default(),
            column: options.column.unwrap_or_default(),
            descending: options.descending,
            expect_sort: options.expect_sort,
            expect_sort_present: options.expect_sort_present,
        },
    )
    .map_err(|err| map_filters_sorts_error("set-sort", err))?;
    let ref_text = sort_state.ref_text.clone();
    let mutation = XlsxFiltersSortsMutationTarget {
        sheet_name: sheet.name,
        sheet_number: sheet.position,
        sheet_id: sheet.sheet_id,
        table_name: None,
        part: sheet_part,
        updated_xml,
        ref_text: Some(ref_text),
        auto_filter: None,
        sort_state: Some(sort_state),
    };

    write_filters_sorts_mutation_result(
        file,
        "set-sort",
        mutation,
        XlsxFiltersSortsOutputOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )
}

pub(crate) fn xlsx_filters_sorts_clear_sort(
    file: &str,
    options: XlsxFiltersSortsClearSortOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheet, sheet_part, sheet_xml) = resolve_filters_sorts_sheet(file, options.sheet)?;
    let updated_xml =
        clear_sort_in_xml(&sheet_xml).map_err(|err| map_filters_sorts_error("clear-sort", err))?;
    let mutation = XlsxFiltersSortsMutationTarget {
        sheet_name: sheet.name,
        sheet_number: sheet.position,
        sheet_id: sheet.sheet_id,
        table_name: None,
        part: sheet_part,
        updated_xml,
        ref_text: None,
        auto_filter: None,
        sort_state: None,
    };

    write_filters_sorts_mutation_result(
        file,
        "clear-sort",
        mutation,
        XlsxFiltersSortsOutputOptions {
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            no_validate: options.no_validate,
            in_place: options.in_place,
        },
    )
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
    ref_text: Option<String>,
    auto_filter: Option<AutoFilterState>,
    sort_state: Option<SortState>,
}

struct AddColumnFilterXmlSpec<'a> {
    col_id: i64,
    values: &'a [String],
    custom_op: Option<&'a str>,
    custom_val1: Option<&'a str>,
    custom_val2: Option<&'a str>,
    custom_present: bool,
    expect_filter: Option<&'a str>,
    expect_filter_present: bool,
}

struct SetSortXmlSpec<'a> {
    ref_range: &'a str,
    column: &'a str,
    descending: bool,
    expect_sort: Option<&'a str>,
    expect_sort_present: bool,
}

fn write_filters_sorts_mutation_result(
    file: &str,
    action: &str,
    mutation: XlsxFiltersSortsMutationTarget,
    options: XlsxFiltersSortsOutputOptions<'_>,
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
    if let Some(ref_text) = mutation
        .ref_text
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        result.insert("ref".to_string(), json!(ref_text));
    }
    result.insert("note".to_string(), json!(FILTERS_SORTS_NOTE));
    if let Some(auto_filter) = mutation.auto_filter.as_ref() {
        result.insert("autoFilter".to_string(), auto_filter_json(auto_filter));
    }
    if let Some(sort_state) = mutation.sort_state.as_ref() {
        result.insert("sortState".to_string(), sort_state_json(sort_state));
    }
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

fn clear_autofilter_in_xml(xml: &str, root_kind: &str) -> CliResult<String> {
    let root = xml_root_bounds(xml, root_kind)?;
    let Some(existing) = direct_child_range(xml, &root, "autoFilter")? else {
        return Err(CliError::invalid_args(
            "worksheet has no autoFilter; run set-autofilter first",
        ));
    };
    Ok(replace_xml_span(xml, existing.start, existing.end, ""))
}

fn add_column_filter_in_xml(
    xml: &str,
    spec: AddColumnFilterXmlSpec<'_>,
) -> CliResult<(String, AutoFilterState)> {
    if spec.col_id < 0 {
        return Err(CliError::invalid_args("colId must be >= 0"));
    }
    let root = xml_root_bounds(xml, "worksheet")?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let Some(auto_filter_range) = direct_child_range(xml, &root, "autoFilter")? else {
        return Err(CliError::invalid_args(
            "worksheet has no autoFilter; run set-autofilter first",
        ));
    };
    let auto_filter_fragment = &xml[auto_filter_range.start..auto_filter_range.end];
    let auto_filter = parse_auto_filter_fragment(auto_filter_fragment)?;
    let col_count = auto_filter_column_count(&auto_filter)?;
    if spec.col_id as u32 >= col_count {
        return Err(CliError::invalid_args(format!(
            "column ID exceeds range column count: colId {} not in 0-{}",
            spec.col_id,
            col_count.saturating_sub(1)
        )));
    }

    let existing_range = find_filter_column_range(auto_filter_fragment, spec.col_id)?;
    let existing_state = existing_range
        .as_ref()
        .map(|range| parse_filter_column_fragment(&auto_filter_fragment[range.start..range.end]))
        .transpose()?;
    guard_expect_filter(
        existing_state.as_ref(),
        spec.expect_filter_present,
        spec.expect_filter,
    )?;

    let new_column = render_filter_column_fragment(
        &prefix,
        spec.col_id,
        spec.values,
        spec.custom_op,
        spec.custom_val1,
        spec.custom_val2,
        spec.custom_present,
    )?;
    let base_fragment = if let Some(existing_range) = existing_range {
        replace_xml_span(
            auto_filter_fragment,
            existing_range.start,
            existing_range.end,
            "",
        )
    } else {
        auto_filter_fragment.to_string()
    };
    let updated_fragment = insert_filter_column_fragment(&base_fragment, &new_column, spec.col_id)?;
    let updated_state = parse_auto_filter_fragment(&updated_fragment)?;
    Ok((
        replace_xml_span(
            xml,
            auto_filter_range.start,
            auto_filter_range.end,
            &updated_fragment,
        ),
        updated_state,
    ))
}

fn clear_column_filter_in_xml(xml: &str, col_id: i64) -> CliResult<(String, AutoFilterState)> {
    let root = xml_root_bounds(xml, "worksheet")?;
    let Some(auto_filter_range) = direct_child_range(xml, &root, "autoFilter")? else {
        return Err(CliError::invalid_args(
            "worksheet has no autoFilter; run set-autofilter first",
        ));
    };
    let auto_filter_fragment = &xml[auto_filter_range.start..auto_filter_range.end];
    let Some(existing_range) = find_filter_column_range(auto_filter_fragment, col_id)? else {
        return Err(CliError::invalid_args(format!(
            "column has no filter: colId {col_id}"
        )));
    };
    let updated_fragment = replace_xml_span(
        auto_filter_fragment,
        existing_range.start,
        existing_range.end,
        "",
    );
    let updated_state = parse_auto_filter_fragment(&updated_fragment)?;
    Ok((
        replace_xml_span(
            xml,
            auto_filter_range.start,
            auto_filter_range.end,
            &updated_fragment,
        ),
        updated_state,
    ))
}

fn set_sort_in_xml(xml: &str, spec: SetSortXmlSpec<'_>) -> CliResult<(String, SortState)> {
    let sort_bounds = parse_range(spec.ref_range)
        .map_err(|err| CliError::invalid_args(format!("invalid --ref: {}", err.message)))?;
    let sort_ref = range_bounds_ref(sort_bounds);
    let condition_ref = sort_condition_ref(sort_bounds, spec.column)?;
    let root = xml_root_bounds(xml, "worksheet")?;
    let prefix = xml_tag_prefix(&root.tag_name);
    let existing_range = direct_child_range(xml, &root, "sortState")?;
    let existing_state = existing_range
        .as_ref()
        .map(|range| parse_sort_state_fragment(&xml[range.start..range.end]))
        .transpose()?;
    guard_expect_sort(
        existing_state.as_ref(),
        spec.expect_sort_present,
        spec.expect_sort,
    )?;

    let condition = render_sort_condition_fragment(&prefix, &condition_ref, spec.descending);
    let sort_state_fragment = if let Some(existing_range) = existing_range.as_ref() {
        let existing_fragment = &xml[existing_range.start..existing_range.end];
        let updated_ref = replace_element_ref_attr(existing_fragment, &sort_ref)?;
        let without_existing = remove_sort_condition_fragment(&updated_ref, &condition_ref)?;
        append_sort_condition_fragment(&without_existing, &condition)?
    } else {
        render_sort_state_fragment(&prefix, &sort_ref, &condition)
    };
    let sort_state = parse_sort_state_fragment(&sort_state_fragment)?;
    let updated_xml = if let Some(existing_range) = existing_range {
        replace_xml_span(
            xml,
            existing_range.start,
            existing_range.end,
            &sort_state_fragment,
        )
    } else {
        insert_ordered_child(xml, &root, "sortState", &sort_state_fragment)?
    };
    Ok((updated_xml, sort_state))
}

fn clear_sort_in_xml(xml: &str) -> CliResult<String> {
    let root = xml_root_bounds(xml, "worksheet")?;
    let Some(sort_state_range) = direct_child_range(xml, &root, "sortState")? else {
        return Err(CliError::invalid_args("worksheet has no sortState"));
    };
    Ok(replace_xml_span(
        xml,
        sort_state_range.start,
        sort_state_range.end,
        "",
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

fn auto_filter_column_count(auto_filter: &AutoFilterState) -> CliResult<u32> {
    let bounds = parse_range(&auto_filter.ref_text).map_err(|err| {
        CliError::invalid_args(format!(
            "invalid autoFilter ref {:?}: {}",
            auto_filter.ref_text, err.message
        ))
    })?;
    Ok(bounds.normalized().col_count())
}

fn find_filter_column_range(
    auto_filter_fragment: &str,
    col_id: i64,
) -> CliResult<Option<crate::XmlNamedRange>> {
    let (open_end, _, close_start, self_closing) = xml_fragment_bounds(auto_filter_fragment)?;
    if self_closing {
        return Ok(None);
    }
    for child in xml_direct_child_ranges(auto_filter_fragment, open_end + 1, close_start)? {
        if child.kind != "filterColumn" {
            continue;
        }
        let (_, attrs, _, _) = first_element(&auto_filter_fragment[child.start..child.end])?;
        if attr_local(&attrs, "colId").and_then(|value| value.parse::<i64>().ok()) == Some(col_id) {
            return Ok(Some(child));
        }
    }
    Ok(None)
}

fn insert_filter_column_fragment(
    auto_filter_fragment: &str,
    column_fragment: &str,
    col_id: i64,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) =
        xml_fragment_bounds(auto_filter_fragment)?;
    if self_closing {
        let start_tag = xml_open_tag_from_start(&auto_filter_fragment[..=open_end]);
        let mut updated = String::new();
        updated.push_str(&start_tag);
        updated.push_str(column_fragment);
        updated.push_str(&format!("</{tag_name}>"));
        return Ok(updated);
    }

    let insert_at = xml_direct_child_ranges(auto_filter_fragment, open_end + 1, close_start)?
        .into_iter()
        .find(|child| {
            if child.kind == "filterColumn" {
                let Ok((_, attrs, _, _)) =
                    first_element(&auto_filter_fragment[child.start..child.end])
                else {
                    return false;
                };
                return attr_local(&attrs, "colId")
                    .and_then(|value| value.parse::<i64>().ok())
                    .is_some_and(|existing_col_id| existing_col_id > col_id);
            }
            child.kind == "sortState"
        })
        .map(|child| child.start)
        .unwrap_or(close_start);
    Ok(replace_xml_span(
        auto_filter_fragment,
        insert_at,
        insert_at,
        column_fragment,
    ))
}

fn render_filter_column_fragment(
    prefix: &str,
    col_id: i64,
    values: &[String],
    custom_op: Option<&str>,
    custom_val1: Option<&str>,
    custom_val2: Option<&str>,
    custom_present: bool,
) -> CliResult<String> {
    let mut out = format!(
        "<{} colId=\"{}\">",
        element_name(prefix, "filterColumn"),
        col_id
    );
    if !values.is_empty() {
        out.push_str(&format!("<{}>", element_name(prefix, "filters")));
        for value in values {
            out.push_str(&format!(
                "<{} val=\"{}\"/>",
                element_name(prefix, "filter"),
                xml_attr_escape(value)
            ));
        }
        out.push_str(&format!("</{}>", element_name(prefix, "filters")));
    }
    if custom_present {
        out.push_str(&render_custom_filters_fragment(
            prefix,
            custom_op.unwrap_or_default(),
            custom_val1.unwrap_or_default(),
            custom_val2.unwrap_or_default(),
        )?);
    }
    out.push_str(&format!("</{}>", element_name(prefix, "filterColumn")));
    Ok(out)
}

fn render_custom_filters_fragment(
    prefix: &str,
    op: &str,
    val1: &str,
    val2: &str,
) -> CliResult<String> {
    let normalized = normalize_custom_operator(op)?;
    if val1.trim().is_empty() {
        return Err(CliError::invalid_args(
            "--custom-val1 is required for a custom filter",
        ));
    }
    let name = element_name(prefix, "customFilters");
    let mut out = if normalized == "between" {
        format!("<{name} and=\"1\">")
    } else {
        format!("<{name}>")
    };
    match normalized.as_str() {
        "between" => {
            if val2.trim().is_empty() {
                return Err(CliError::invalid_args(format!(
                    "--custom-val2 is required for {op}"
                )));
            }
            push_custom_filter_xml(prefix, &mut out, "greaterThanOrEqual", val1);
            push_custom_filter_xml(prefix, &mut out, "lessThanOrEqual", val2);
        }
        "notBetween" => {
            if val2.trim().is_empty() {
                return Err(CliError::invalid_args(format!(
                    "--custom-val2 is required for {op}"
                )));
            }
            push_custom_filter_xml(prefix, &mut out, "lessThan", val1);
            push_custom_filter_xml(prefix, &mut out, "greaterThan", val2);
        }
        operator => {
            if !val2.trim().is_empty() {
                return Err(CliError::invalid_args(
                    "--custom-val2 is only valid with the between or notBetween operator",
                ));
            }
            push_custom_filter_xml(prefix, &mut out, operator, val1);
        }
    }
    out.push_str(&format!("</{}>", element_name(prefix, "customFilters")));
    Ok(out)
}

fn push_custom_filter_xml(prefix: &str, out: &mut String, operator: &str, val: &str) {
    out.push_str(&format!("<{}", element_name(prefix, "customFilter")));
    if operator != "equal" {
        out.push_str(&format!(" operator=\"{}\"", xml_attr_escape(operator)));
    }
    out.push_str(&format!(" val=\"{}\"/>", xml_attr_escape(val)));
}

fn normalize_custom_operator(op: &str) -> CliResult<String> {
    let trimmed = op.trim();
    if trimmed.is_empty() {
        return Err(CliError::invalid_args("custom operator cannot be empty"));
    }
    let normalized = match trimmed.to_ascii_lowercase().as_str() {
        "eq" | "equals" | "==" | "=" => "equal",
        "ne" | "!=" | "<>" => "notEqual",
        "lt" | "<" | "less-than" => "lessThan",
        "le" | "lte" | "<=" | "less-than-or-equal" => "lessThanOrEqual",
        "gt" | ">" | "greater-than" => "greaterThan",
        "ge" | "gte" | ">=" | "greater-than-or-equal" => "greaterThanOrEqual",
        "between" => "between",
        "not-between" | "notbetween" => "notBetween",
        _ => match trimmed {
            "equal" | "notEqual" | "lessThan" | "lessThanOrEqual" | "greaterThan"
            | "greaterThanOrEqual" => trimmed,
            _ => {
                return Err(CliError::invalid_args(format!(
                    "invalid custom operator {op:?} (use one of equal,notEqual,lessThan,lessThanOrEqual,greaterThan,greaterThanOrEqual,between,notBetween)"
                )));
            }
        },
    };
    Ok(normalized.to_string())
}

fn parse_filter_values(values: Option<&str>) -> Vec<String> {
    let Some(values) = values.filter(|value| !value.trim().is_empty()) else {
        return Vec::new();
    };
    let mut seen = BTreeMap::new();
    let mut deduped = Vec::new();
    for value in values
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if seen.insert(value.to_string(), true).is_none() {
            deduped.push(value.to_string());
        }
    }
    deduped
}

fn guard_expect_filter(
    column: Option<&FilterColumnState>,
    has_expect: bool,
    expect: Option<&str>,
) -> CliResult<()> {
    if !has_expect {
        return Ok(());
    }
    let current = column
        .map(summarize_filter_column)
        .unwrap_or_else(|| "none".to_string());
    let want = expect.unwrap_or_default().trim();
    if current != want {
        return Err(CliError::invalid_args(format!(
            "filter mismatch: expected {want:?}, found {current:?}"
        )));
    }
    Ok(())
}

fn summarize_filter_column(column: &FilterColumnState) -> String {
    if !column.values.is_empty() {
        return format!("values:{}", column.values.join(","));
    }
    if let Some(custom_filter) = column.custom_filter.as_ref() {
        let parts = custom_filter
            .criteria
            .iter()
            .map(|criterion| {
                let operator = if criterion.operator.is_empty() {
                    "equal"
                } else {
                    &criterion.operator
                };
                format!("{}={}", operator, criterion.val)
            })
            .collect::<Vec<_>>();
        return format!("custom:{}", parts.join(","));
    }
    "none".to_string()
}

fn sort_condition_ref(sort_bounds: RangeBounds, column: &str) -> CliResult<String> {
    let col_idx = parse_sort_column_index(column)?;
    let normalized = sort_bounds.normalized();
    if col_idx < normalized.min_col() || col_idx > normalized.max_col() {
        return Err(CliError::invalid_args(format!(
            "column {} is outside sort ref {}",
            column.to_ascii_uppercase(),
            range_bounds_ref(sort_bounds)
        )));
    }
    Ok(range_bounds_ref(RangeBounds {
        start_col: col_idx,
        start_row: normalized.min_row(),
        end_col: col_idx,
        end_row: normalized.max_row(),
    }))
}

fn parse_sort_column_index(column: &str) -> CliResult<u32> {
    let letters = column.trim();
    if letters.is_empty() {
        return Err(CliError::invalid_args(
            "invalid --column: column letters cannot be empty",
        ));
    }
    let mut index = 0u32;
    for ch in letters.chars() {
        let upper = ch.to_ascii_uppercase();
        if !upper.is_ascii_uppercase() {
            return Err(CliError::invalid_args(format!(
                "invalid --column: invalid column letter {ch:?}"
            )));
        }
        index = index * 26 + (upper as u32 - 'A' as u32 + 1);
        if index > 16_384 {
            return Err(CliError::invalid_args(format!(
                "invalid --column: column {letters:?} out of XLSX bounds A-XFD"
            )));
        }
    }
    Ok(index)
}

fn guard_expect_sort(
    state: Option<&SortState>,
    has_expect: bool,
    expect: Option<&str>,
) -> CliResult<()> {
    if !has_expect {
        return Ok(());
    }
    let want = parse_range(expect.unwrap_or_default())
        .map_err(|err| CliError::invalid_args(format!("invalid --expect-sort: {}", err.message)))
        .map(range_bounds_ref)?;
    let current = state
        .map(|state| state.ref_text.as_str())
        .unwrap_or_default();
    let got = if current.is_empty() {
        String::new()
    } else {
        parse_range(current)
            .map(range_bounds_ref)
            .unwrap_or_else(|_| current.to_string())
    };
    if got != want {
        return Err(CliError::invalid_args(format!(
            "sort ref mismatch: expected {want}, found {current:?}"
        )));
    }
    Ok(())
}

fn render_sort_state_fragment(prefix: &str, sort_ref: &str, condition: &str) -> String {
    format!(
        "<{} ref=\"{}\">{}</{}>",
        element_name(prefix, "sortState"),
        xml_attr_escape(sort_ref),
        condition,
        element_name(prefix, "sortState")
    )
}

fn render_sort_condition_fragment(prefix: &str, condition_ref: &str, descending: bool) -> String {
    let name = element_name(prefix, "sortCondition");
    if descending {
        format!(
            "<{name} descending=\"1\" ref=\"{}\"/>",
            xml_attr_escape(condition_ref)
        )
    } else {
        format!("<{name} ref=\"{}\"/>", xml_attr_escape(condition_ref))
    }
}

fn remove_sort_condition_fragment(
    sort_state_fragment: &str,
    condition_ref: &str,
) -> CliResult<String> {
    let (open_end, _, close_start, self_closing) = xml_fragment_bounds(sort_state_fragment)?;
    if self_closing {
        return Ok(sort_state_fragment.to_string());
    }
    for child in xml_direct_child_ranges(sort_state_fragment, open_end + 1, close_start)? {
        if child.kind != "sortCondition" {
            continue;
        }
        let (_, attrs, _, _) = first_element(&sort_state_fragment[child.start..child.end])?;
        if attr_local(&attrs, "ref").as_deref() == Some(condition_ref) {
            return Ok(replace_xml_span(
                sort_state_fragment,
                child.start,
                child.end,
                "",
            ));
        }
    }
    Ok(sort_state_fragment.to_string())
}

fn append_sort_condition_fragment(sort_state_fragment: &str, condition: &str) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(sort_state_fragment)?;
    if self_closing {
        let start_tag = xml_open_tag_from_start(&sort_state_fragment[..=open_end]);
        let mut updated = String::new();
        updated.push_str(&start_tag);
        updated.push_str(condition);
        updated.push_str(&format!("</{tag_name}>"));
        return Ok(updated);
    }
    Ok(replace_xml_span(
        sort_state_fragment,
        close_start,
        close_start,
        condition,
    ))
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
