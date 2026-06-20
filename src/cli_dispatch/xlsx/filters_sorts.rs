use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::{
    XlsxFiltersSortsAddColumnFilterOptions, XlsxFiltersSortsClearAutoFilterOptions,
    XlsxFiltersSortsClearColumnFilterOptions, XlsxFiltersSortsClearSortOptions,
    XlsxFiltersSortsSetAutoFilterOptions, XlsxFiltersSortsSetSortOptions,
    xlsx_filters_sorts_add_column_filter, xlsx_filters_sorts_clear_autofilter,
    xlsx_filters_sorts_clear_column_filter, xlsx_filters_sorts_clear_sort,
    xlsx_filters_sorts_set_autofilter, xlsx_filters_sorts_set_sort, xlsx_filters_sorts_show,
};

pub(super) fn dispatch_xlsx_filters_sorts(args: &[String]) -> CliResult<Value> {
    match args {
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "filters-sorts" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--range", "--table"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let table = parse_string_flag(rest, "--table")?;
            let _range_hint = parse_string_flag(rest, "--range")?;
            xlsx_filters_sorts_show(file, sheet.as_deref(), table.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "filters-sorts" && verb == "set-autofilter" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--table",
                    "--expect-range",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let table = parse_string_flag(rest, "--table")?;
            let expect_range = parse_string_flag(rest, "--expect-range")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_filters_sorts_set_autofilter(
                file,
                XlsxFiltersSortsSetAutoFilterOptions {
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    table: table.as_deref(),
                    expect_range: expect_range.as_deref(),
                    expect_range_present: value_flag_present(rest, "--expect-range"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "filters-sorts" && verb == "clear-autofilter" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--table",
                    "--expect-range",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let table = parse_string_flag(rest, "--table")?;
            let expect_range = parse_string_flag(rest, "--expect-range")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_filters_sorts_clear_autofilter(
                file,
                XlsxFiltersSortsClearAutoFilterOptions {
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    table: table.as_deref(),
                    expect_range: expect_range.as_deref(),
                    expect_range_present: value_flag_present(rest, "--expect-range"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "filters-sorts" && verb == "add-column-filter" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--column",
                    "--values",
                    "--custom-op",
                    "--custom-val1",
                    "--custom-val2",
                    "--expect-filter",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let column = parse_i64_flag(rest, "--column")?.unwrap_or(0);
            let values = parse_string_flag(rest, "--values")?;
            let custom_op = parse_string_flag(rest, "--custom-op")?;
            let custom_val1 = parse_string_flag(rest, "--custom-val1")?;
            let custom_val2 = parse_string_flag(rest, "--custom-val2")?;
            let expect_filter = parse_string_flag(rest, "--expect-filter")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_filters_sorts_add_column_filter(
                file,
                XlsxFiltersSortsAddColumnFilterOptions {
                    sheet: sheet.as_deref(),
                    column,
                    values: values.as_deref(),
                    custom_op: custom_op.as_deref(),
                    custom_val1: custom_val1.as_deref(),
                    custom_val2: custom_val2.as_deref(),
                    custom_present: value_flag_present(rest, "--custom-op"),
                    expect_filter: expect_filter.as_deref(),
                    expect_filter_present: value_flag_present(rest, "--expect-filter"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "filters-sorts" && verb == "clear-column-filter" =>
        {
            reject_unknown_flags(
                rest,
                &["--sheet", "--column", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let column = parse_i64_flag(rest, "--column")?.unwrap_or(0);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_filters_sorts_clear_column_filter(
                file,
                XlsxFiltersSortsClearColumnFilterOptions {
                    sheet: sheet.as_deref(),
                    column,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "filters-sorts" && verb == "set-sort" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--ref",
                    "--column",
                    "--expect-sort",
                    "--out",
                    "--backup",
                ],
                &["--descending", "--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let ref_range = parse_string_flag(rest, "--ref")?;
            let column = parse_string_flag(rest, "--column")?;
            let expect_sort = parse_string_flag(rest, "--expect-sort")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_filters_sorts_set_sort(
                file,
                XlsxFiltersSortsSetSortOptions {
                    sheet: sheet.as_deref(),
                    ref_range: ref_range.as_deref(),
                    column: column.as_deref(),
                    descending: has_flag(rest, "--descending"),
                    expect_sort: expect_sort.as_deref(),
                    expect_sort_present: value_flag_present(rest, "--expect-sort"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "filters-sorts" && verb == "clear-sort" =>
        {
            reject_unknown_flags(
                rest,
                &["--sheet", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_filters_sorts_clear_sort(
                file,
                XlsxFiltersSortsClearSortOptions {
                    sheet: sheet.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}
