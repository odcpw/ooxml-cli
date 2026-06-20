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
mod autofilter;
mod column_filter;
mod model;
mod options;
mod package;
mod sort;
mod xml_support;

pub(crate) use options::{
    XlsxFiltersSortsAddColumnFilterOptions, XlsxFiltersSortsClearAutoFilterOptions,
    XlsxFiltersSortsClearColumnFilterOptions, XlsxFiltersSortsClearSortOptions,
    XlsxFiltersSortsSetAutoFilterOptions, XlsxFiltersSortsSetSortOptions,
};

use autofilter::{
    clear_autofilter_in_xml, guard_expect_range, normalize_filters_sorts_range,
    set_autofilter_in_xml,
};
use column_filter::{
    AddColumnFilterXmlSpec, add_column_filter_in_xml, clear_column_filter_in_xml,
    parse_filter_values,
};
use model::{
    AutoFilterState, FilterColumnState, SortState, auto_filter_column_count, auto_filter_json,
    parse_auto_filter_fragment, parse_filter_column_fragment, parse_sort_state_fragment,
    read_auto_filter_state, read_sort_state, sort_state_json,
};
use options::XlsxFiltersSortsOutputOptions;
use package::{
    XlsxFiltersSortsMutationTarget, filters_sorts_sheet_selector, filters_sorts_show_command,
    resolve_filters_sorts_sheet, resolve_filters_sorts_table, write_filters_sorts_mutation_result,
};
use sort::{SetSortXmlSpec, clear_sort_in_xml, set_sort_in_xml};
use xml_support::{
    attr_local, attr_local_start, direct_child_range, element_name, first_element,
    insert_first_child, insert_ordered_child, replace_element_ref_attr, xml_root_bounds,
};
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

fn map_filters_sorts_error(action: &str, err: CliError) -> CliError {
    if err.code == "invalid_args" {
        CliError::invalid_args(format!("failed to {action}: {}", err.message))
    } else {
        err
    }
}
