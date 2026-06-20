mod tables;

use serde_json::Value;

use self::tables::dispatch_xlsx_tables;
use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::xlsx_freeze::*;
use crate::xlsx_metadata::*;
use crate::xlsx_mutation::*;
use crate::xlsx_names::*;
use crate::xlsx_ranges::*;
use crate::xlsx_sheets::*;
use crate::{
    XlsxColWidthsSetOptions, XlsxColsDeleteOptions, XlsxColsInsertOptions, XlsxCommentsAddOptions,
    XlsxCommentsRemoveOptions, XlsxCommentsUpdateOptions, XlsxDataValidationFields,
    XlsxDataValidationMutationOptions, XlsxFiltersSortsAddColumnFilterOptions,
    XlsxFiltersSortsClearAutoFilterOptions, XlsxFiltersSortsClearColumnFilterOptions,
    XlsxFiltersSortsClearSortOptions, XlsxFiltersSortsSetAutoFilterOptions,
    XlsxFiltersSortsSetSortOptions, XlsxHyperlinkAddOptions, XlsxHyperlinkDeleteOptions,
    XlsxHyperlinkUpdateOptions, XlsxRowHeightsSetOptions, XlsxRowsDeleteOptions,
    XlsxRowsInsertOptions, XlsxSheetsAddOptions, XlsxSheetsDeleteOptions, XlsxSheetsMoveOptions,
    XlsxSheetsRenameOptions, xlsx_cols_delete, xlsx_cols_insert, xlsx_colwidths_set,
    xlsx_colwidths_show, xlsx_comments_add, xlsx_comments_list, xlsx_comments_remove,
    xlsx_comments_update, xlsx_data_validations_create, xlsx_data_validations_delete,
    xlsx_data_validations_list, xlsx_data_validations_show, xlsx_data_validations_update,
    xlsx_filters_sorts_add_column_filter, xlsx_filters_sorts_clear_autofilter,
    xlsx_filters_sorts_clear_column_filter, xlsx_filters_sorts_clear_sort,
    xlsx_filters_sorts_set_autofilter, xlsx_filters_sorts_set_sort, xlsx_filters_sorts_show,
    xlsx_hyperlinks_add, xlsx_hyperlinks_delete, xlsx_hyperlinks_list, xlsx_hyperlinks_show,
    xlsx_hyperlinks_update, xlsx_rowheights_set, xlsx_rowheights_show, xlsx_rows_delete,
    xlsx_rows_insert, xlsx_sheets_add, xlsx_sheets_delete, xlsx_sheets_move, xlsx_sheets_rename,
};

pub(super) fn dispatch_xlsx(args: &[String]) -> CliResult<Value> {
    match args {
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "export" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--data-format",
                    "--data-out",
                    "--max-cells",
                ],
                &["--include-types", "--include-formulas", "--include-formats"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?.unwrap_or_else(|| "1".to_string());
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            require_json_data_format(data_format.as_deref())?;
            let data_out = parse_string_flag(rest, "--data-out")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let include_types = has_flag(rest, "--include-types");
            let include_formulas = has_flag(rest, "--include-formulas");
            let include_formats = has_flag(rest, "--include-formats");
            xlsx_range_export_with_options(
                file,
                &sheet,
                &range,
                XlsxRangeExportOptions {
                    include_types,
                    include_formulas,
                    include_formats,
                    data_out: data_out.as_deref(),
                    max_cells,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--anchor",
                    "--values",
                    "--values-file",
                    "--data-format",
                    "--null-policy",
                    "--ragged",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &[
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                    "--overwrite-formulas",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?
                .ok_or_else(|| CliError::invalid_args("--sheet is required for range commands"))?;
            let range = parse_string_flag(rest, "--range")?;
            let anchor = parse_string_flag(rest, "--anchor")?;
            let values = parse_string_flag(rest, "--values")?;
            let values_file = parse_string_flag(rest, "--values-file")?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            let null_policy = parse_string_flag(rest, "--null-policy")?;
            let ragged = parse_string_flag(rest, "--ragged")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let no_validate = has_flag(rest, "--no-validate");
            let in_place = has_flag(rest, "--in-place");
            let overwrite_formulas = has_flag(rest, "--overwrite-formulas");
            xlsx_ranges_set(
                file,
                XlsxRangesSetOptions {
                    sheet: &sheet,
                    range: range.as_deref(),
                    anchor: anchor.as_deref(),
                    values: values.as_deref(),
                    values_file: values_file.as_deref(),
                    data_format: data_format.as_deref(),
                    null_policy: null_policy.as_deref(),
                    ragged: ragged.as_deref(),
                    max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    no_validate,
                    in_place,
                    overwrite_formulas,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "set-format" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--preset",
                    "--format-code",
                    "--decimals",
                    "--currency-symbol",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?
                .ok_or_else(|| CliError::invalid_args("--sheet is required for range commands"))?;
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            let preset = parse_string_flag(rest, "--preset")?;
            let format_code = parse_string_flag(rest, "--format-code")?;
            let decimals = parse_i64_flag(rest, "--decimals")?.unwrap_or(2);
            let currency_symbol = parse_string_flag(rest, "--currency-symbol")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let no_validate = has_flag(rest, "--no-validate");
            let in_place = has_flag(rest, "--in-place");
            xlsx_ranges_set_format(
                file,
                XlsxRangesSetFormatOptions {
                    sheet: &sheet,
                    range: &range,
                    preset: preset.as_deref(),
                    format_code: format_code.as_deref(),
                    decimals,
                    currency_symbol: currency_symbol.as_deref(),
                    max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    no_validate,
                    in_place,
                },
            )
        }
        [family, group, subgroup, verb, file]
            if family == "xlsx"
                && group == "workbook"
                && subgroup == "metadata"
                && verb == "inspect" =>
        {
            xlsx_workbook_metadata_inspect(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "colwidths" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--range"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required (e.g. B or B:D)"))?;
            xlsx_colwidths_show(file, sheet.as_deref(), &range)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "colwidths" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--width",
                    "--expect-width",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required (e.g. B or B:D)"))?;
            let width = parse_f64_flag(rest, "--width")?;
            let expect_width = parse_f64_flag(rest, "--expect-width")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_colwidths_set(
                file,
                XlsxColWidthsSetOptions {
                    sheet: sheet.as_deref(),
                    range: &range,
                    width,
                    expect_width,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "rowheights" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--range"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required (e.g. 2 or 2:5)"))?;
            xlsx_rowheights_show(file, sheet.as_deref(), &range)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "rowheights" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--height",
                    "--expect-height",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required (e.g. 2 or 2:5)"))?;
            let height = parse_f64_flag(rest, "--height")?;
            let expect_height = parse_f64_flag(rest, "--expect-height")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_rowheights_set(
                file,
                XlsxRowHeightsSetOptions {
                    sheet: sheet.as_deref(),
                    range: &range,
                    height,
                    expect_height,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && (group == "rows" || group == "row") && verb == "insert" =>
        {
            reject_unknown_flags(
                rest,
                &["--sheet", "--at", "--count", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let at = parse_i64_flag(rest, "--at")?;
            let count = parse_i64_flag(rest, "--count")?.unwrap_or(1);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_rows_insert(
                file,
                XlsxRowsInsertOptions {
                    sheet: sheet.as_deref(),
                    at,
                    count,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && (group == "rows" || group == "row") && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &["--sheet", "--row", "--count", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let row = parse_i64_flag(rest, "--row")?;
            let count = parse_i64_flag(rest, "--count")?.unwrap_or(1);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_rows_delete(
                file,
                XlsxRowsDeleteOptions {
                    sheet: sheet.as_deref(),
                    row,
                    count,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "cols"
                    || group == "col"
                    || group == "columns"
                    || group == "column")
                && verb == "insert" =>
        {
            reject_unknown_flags(
                rest,
                &["--sheet", "--at", "--count", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let at = parse_string_flag(rest, "--at")?;
            let count = parse_i64_flag(rest, "--count")?.unwrap_or(1);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_cols_insert(
                file,
                XlsxColsInsertOptions {
                    sheet: sheet.as_deref(),
                    at: at.as_deref(),
                    count,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "cols"
                    || group == "col"
                    || group == "columns"
                    || group == "column")
                && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &["--sheet", "--col", "--count", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let col = parse_string_flag(rest, "--col")?;
            let count = parse_i64_flag(rest, "--count")?.unwrap_or(1);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_cols_delete(
                file,
                XlsxColsDeleteOptions {
                    sheet: sheet.as_deref(),
                    col: col.as_deref(),
                    count,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "comments" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--comment-id"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let comment_id = if value_flag_present(rest, "--comment-id") {
                let value = parse_i64_flag(rest, "--comment-id")?
                    .ok_or_else(|| CliError::invalid_args("--comment-id requires a value"))?;
                if value < 0 {
                    return Err(CliError::invalid_args("--comment-id must be >= 0"));
                }
                Some(value)
            } else {
                None
            };
            xlsx_comments_list(file, sheet.as_deref(), comment_id)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_hyperlinks_group(group) && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--sheet"], &["--include-broken"])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_hyperlinks_list(file, sheet.as_deref(), has_flag(rest, "--include-broken"))
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_hyperlinks_group(group) && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--cell"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            xlsx_hyperlinks_show(file, sheet.as_deref(), cell.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_hyperlinks_group(group) && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--cell",
                    "--url",
                    "--location",
                    "--display",
                    "--tooltip",
                    "--out",
                    "--backup",
                ],
                &["--replace", "--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            let url = parse_string_flag(rest, "--url")?;
            let location = parse_string_flag(rest, "--location")?;
            let display = parse_string_flag(rest, "--display")?;
            let tooltip = parse_string_flag(rest, "--tooltip")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_hyperlinks_add(
                file,
                XlsxHyperlinkAddOptions {
                    sheet: sheet.as_deref(),
                    cell: cell.as_deref(),
                    url: url.as_deref(),
                    location: location.as_deref(),
                    display: display.as_deref(),
                    tooltip: tooltip.as_deref(),
                    replace: has_flag(rest, "--replace"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_hyperlinks_group(group) && verb == "update" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--cell",
                    "--url",
                    "--location",
                    "--display",
                    "--tooltip",
                    "--expect-url",
                    "--expect-location",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            let url = parse_string_flag(rest, "--url")?;
            let location = parse_string_flag(rest, "--location")?;
            let display = parse_string_flag(rest, "--display")?;
            let tooltip = parse_string_flag(rest, "--tooltip")?;
            let expect_url = parse_string_flag(rest, "--expect-url")?;
            let expect_location = parse_string_flag(rest, "--expect-location")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_hyperlinks_update(
                file,
                XlsxHyperlinkUpdateOptions {
                    sheet: sheet.as_deref(),
                    cell: cell.as_deref(),
                    url: url.as_deref(),
                    set_url: value_flag_present(rest, "--url"),
                    location: location.as_deref(),
                    set_location: value_flag_present(rest, "--location"),
                    display: display.as_deref(),
                    set_display: value_flag_present(rest, "--display"),
                    tooltip: tooltip.as_deref(),
                    set_tooltip: value_flag_present(rest, "--tooltip"),
                    expect_url: expect_url.as_deref(),
                    has_expect_url: value_flag_present(rest, "--expect-url"),
                    expect_location: expect_location.as_deref(),
                    has_expect_location: value_flag_present(rest, "--expect-location"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_hyperlinks_group(group) && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--cell",
                    "--expect-url",
                    "--expect-location",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            let expect_url = parse_string_flag(rest, "--expect-url")?;
            let expect_location = parse_string_flag(rest, "--expect-location")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_hyperlinks_delete(
                file,
                XlsxHyperlinkDeleteOptions {
                    sheet: sheet.as_deref(),
                    cell: cell.as_deref(),
                    expect_url: expect_url.as_deref(),
                    has_expect_url: value_flag_present(rest, "--expect-url"),
                    expect_location: expect_location.as_deref(),
                    has_expect_location: value_flag_present(rest, "--expect-location"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "comments" && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--cell",
                    "--author",
                    "--text",
                    "--text-file",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            let author = parse_string_flag(rest, "--author")?;
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_comments_add(
                file,
                XlsxCommentsAddOptions {
                    sheet: sheet.as_deref(),
                    cell: cell.as_deref(),
                    author: author.as_deref(),
                    text: text.as_deref(),
                    text_file: text_file.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "comments" && verb == "update" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--comment-id",
                    "--handle",
                    "--text",
                    "--text-file",
                    "--author",
                    "--expect-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let comment_id = if value_flag_present(rest, "--comment-id") {
                let value = parse_i64_flag(rest, "--comment-id")?
                    .ok_or_else(|| CliError::invalid_args("--comment-id requires a value"))?;
                if value < 0 {
                    return Err(CliError::invalid_args("--comment-id must be >= 0"));
                }
                Some(value)
            } else {
                None
            };
            let handle = parse_string_flag(rest, "--handle")?;
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let author = parse_string_flag(rest, "--author")?;
            let expect_hash = parse_string_flag(rest, "--expect-hash")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_comments_update(
                file,
                XlsxCommentsUpdateOptions {
                    sheet: sheet.as_deref(),
                    comment_id,
                    handle: handle.as_deref(),
                    text: text.as_deref(),
                    text_present: value_flag_present(rest, "--text"),
                    text_file: text_file.as_deref(),
                    author: author.as_deref(),
                    author_present: value_flag_present(rest, "--author"),
                    expect_hash: expect_hash.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && group == "comments"
                && (verb == "remove" || verb == "delete") =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--comment-id",
                    "--handle",
                    "--expect-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let comment_id = if value_flag_present(rest, "--comment-id") {
                let value = parse_i64_flag(rest, "--comment-id")?
                    .ok_or_else(|| CliError::invalid_args("--comment-id requires a value"))?;
                if value < 0 {
                    return Err(CliError::invalid_args("--comment-id must be >= 0"));
                }
                Some(value)
            } else {
                None
            };
            let handle = parse_string_flag(rest, "--handle")?;
            let expect_hash = parse_string_flag(rest, "--expect-hash")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_comments_remove(
                file,
                XlsxCommentsRemoveOptions {
                    sheet: sheet.as_deref(),
                    comment_id,
                    handle: handle.as_deref(),
                    expect_hash: expect_hash.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_data_validations_group(group) && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--sheet"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_data_validations_list(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_data_validations_group(group) && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--range"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            xlsx_data_validations_show(file, sheet.as_deref(), &range)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_data_validations_group(group) && verb == "create" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--type",
                    "--list-values",
                    "--list-range",
                    "--operator",
                    "--formula1",
                    "--formula2",
                    "--input-title",
                    "--input-message",
                    "--error-title",
                    "--error-message",
                    "--error-style",
                    "--out",
                    "--backup",
                ],
                &[
                    "--allow-blank",
                    "--show-input-message",
                    "--show-error-message",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let validation_type = parse_string_flag(rest, "--type")?;
            let list_values = parse_string_flag(rest, "--list-values")?;
            let list_range = parse_string_flag(rest, "--list-range")?;
            let operator = parse_string_flag(rest, "--operator")?;
            let formula1 = parse_string_flag(rest, "--formula1")?;
            let formula2 = parse_string_flag(rest, "--formula2")?;
            let input_title = parse_string_flag(rest, "--input-title")?;
            let input_message = parse_string_flag(rest, "--input-message")?;
            let error_title = parse_string_flag(rest, "--error-title")?;
            let error_message = parse_string_flag(rest, "--error-message")?;
            let error_style = parse_string_flag(rest, "--error-style")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_data_validations_create(
                file,
                XlsxDataValidationMutationOptions {
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    fields: xlsx_data_validation_fields_from_flags(
                        rest,
                        validation_type.as_deref(),
                        operator.as_deref(),
                        formula1.as_deref(),
                        formula2.as_deref(),
                        list_values.as_deref(),
                        list_range.as_deref(),
                        input_title.as_deref(),
                        input_message.as_deref(),
                        error_title.as_deref(),
                        error_message.as_deref(),
                        error_style.as_deref(),
                    ),
                    expect_type: None,
                    expect_type_present: false,
                    expect_formula1: None,
                    expect_formula1_present: false,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_data_validations_group(group) && verb == "update" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--type",
                    "--list-values",
                    "--list-range",
                    "--operator",
                    "--formula1",
                    "--formula2",
                    "--input-title",
                    "--input-message",
                    "--error-title",
                    "--error-message",
                    "--error-style",
                    "--expect-type",
                    "--expect-formula1",
                    "--out",
                    "--backup",
                ],
                &[
                    "--allow-blank",
                    "--show-input-message",
                    "--show-error-message",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let validation_type = parse_string_flag(rest, "--type")?;
            let list_values = parse_string_flag(rest, "--list-values")?;
            let list_range = parse_string_flag(rest, "--list-range")?;
            let operator = parse_string_flag(rest, "--operator")?;
            let formula1 = parse_string_flag(rest, "--formula1")?;
            let formula2 = parse_string_flag(rest, "--formula2")?;
            let input_title = parse_string_flag(rest, "--input-title")?;
            let input_message = parse_string_flag(rest, "--input-message")?;
            let error_title = parse_string_flag(rest, "--error-title")?;
            let error_message = parse_string_flag(rest, "--error-message")?;
            let error_style = parse_string_flag(rest, "--error-style")?;
            let expect_type = parse_string_flag(rest, "--expect-type")?;
            let expect_formula1 = parse_string_flag(rest, "--expect-formula1")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_data_validations_update(
                file,
                XlsxDataValidationMutationOptions {
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    fields: xlsx_data_validation_fields_from_flags(
                        rest,
                        validation_type.as_deref(),
                        operator.as_deref(),
                        formula1.as_deref(),
                        formula2.as_deref(),
                        list_values.as_deref(),
                        list_range.as_deref(),
                        input_title.as_deref(),
                        input_message.as_deref(),
                        error_title.as_deref(),
                        error_message.as_deref(),
                        error_style.as_deref(),
                    ),
                    expect_type: expect_type.as_deref(),
                    expect_type_present: value_flag_present(rest, "--expect-type"),
                    expect_formula1: expect_formula1.as_deref(),
                    expect_formula1_present: value_flag_present(rest, "--expect-formula1"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_data_validations_group(group) && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--expect-type",
                    "--expect-formula1",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let expect_type = parse_string_flag(rest, "--expect-type")?;
            let expect_formula1 = parse_string_flag(rest, "--expect-formula1")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_data_validations_delete(
                file,
                XlsxDataValidationMutationOptions {
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    fields: empty_xlsx_data_validation_fields(),
                    expect_type: expect_type.as_deref(),
                    expect_type_present: value_flag_present(rest, "--expect-type"),
                    expect_formula1: expect_formula1.as_deref(),
                    expect_formula1_present: value_flag_present(rest, "--expect-formula1"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
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
        [family, group, subgroup, verb, file, rest @ ..]
            if family == "xlsx"
                && group == "workbook"
                && subgroup == "metadata"
                && verb == "update" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--title",
                    "--subject",
                    "--creator",
                    "--keywords",
                    "--description",
                    "--last-modified-by",
                    "--category",
                    "--company",
                    "--manager",
                    "--calc-mode",
                    "--expect-title",
                    "--expect-subject",
                    "--expect-creator",
                    "--expect-keywords",
                    "--expect-description",
                    "--expect-last-modified-by",
                    "--expect-category",
                    "--expect-company",
                    "--expect-manager",
                    "--out",
                    "--backup",
                ],
                &[
                    "--full-calc-on-load",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let title = parse_string_flag(rest, "--title")?;
            let subject = parse_string_flag(rest, "--subject")?;
            let creator = parse_string_flag(rest, "--creator")?;
            let keywords = parse_string_flag(rest, "--keywords")?;
            let description = parse_string_flag(rest, "--description")?;
            let last_modified_by = parse_string_flag(rest, "--last-modified-by")?;
            let category = parse_string_flag(rest, "--category")?;
            let company = parse_string_flag(rest, "--company")?;
            let manager = parse_string_flag(rest, "--manager")?;
            let calc_mode = parse_string_flag(rest, "--calc-mode")?;
            let expect_title = parse_string_flag(rest, "--expect-title")?;
            let expect_subject = parse_string_flag(rest, "--expect-subject")?;
            let expect_creator = parse_string_flag(rest, "--expect-creator")?;
            let expect_keywords = parse_string_flag(rest, "--expect-keywords")?;
            let expect_description = parse_string_flag(rest, "--expect-description")?;
            let expect_last_modified_by = parse_string_flag(rest, "--expect-last-modified-by")?;
            let expect_category = parse_string_flag(rest, "--expect-category")?;
            let expect_company = parse_string_flag(rest, "--expect-company")?;
            let expect_manager = parse_string_flag(rest, "--expect-manager")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let full_calc_on_load = parse_bool_flag(rest, "--full-calc-on-load")?;
            xlsx_workbook_metadata_update(
                file,
                XlsxWorkbookMetadataUpdateOptions {
                    title: title.as_deref(),
                    subject: subject.as_deref(),
                    creator: creator.as_deref(),
                    keywords: keywords.as_deref(),
                    description: description.as_deref(),
                    last_modified_by: last_modified_by.as_deref(),
                    category: category.as_deref(),
                    company: company.as_deref(),
                    manager: manager.as_deref(),
                    calc_mode: calc_mode.as_deref(),
                    full_calc_on_load,
                    expect_title: expect_title.as_deref(),
                    expect_subject: expect_subject.as_deref(),
                    expect_creator: expect_creator.as_deref(),
                    expect_keywords: expect_keywords.as_deref(),
                    expect_description: expect_description.as_deref(),
                    expect_last_modified_by: expect_last_modified_by.as_deref(),
                    expect_category: expect_category.as_deref(),
                    expect_company: expect_company.as_deref(),
                    expect_manager: expect_manager.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "cells" && verb == "extract" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?.unwrap_or_else(|| "1".to_string());
            let range = parse_string_flag(rest, "--range")?;
            let max_rows = parse_u32_flag(rest, "--max-rows")?.unwrap_or(1000);
            let max_cells = parse_u32_flag(rest, "--max-cells")?.unwrap_or(0);
            let include_empty = has_flag(rest, "--include-empty");
            xlsx_cells_extract(
                file,
                &sheet,
                range.as_deref(),
                max_rows,
                max_cells,
                include_empty,
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "cells" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--cell",
                    "--ref",
                    "--value",
                    "--formula",
                    "--type",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            let ref_ = parse_string_flag(rest, "--ref")?;
            let value = parse_string_flag(rest, "--value")?;
            let formula = parse_string_flag(rest, "--formula")?;
            let value_type = parse_string_flag(rest, "--type")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let no_validate = has_flag(rest, "--no-validate");
            let in_place = has_flag(rest, "--in-place");
            xlsx_cells_set(
                file,
                XlsxCellsSetOptions {
                    sheet: sheet.as_deref(),
                    cell: cell.as_deref(),
                    ref_: ref_.as_deref(),
                    value: value.as_deref(),
                    formula: formula.as_deref(),
                    value_type: value_type.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    no_validate,
                    in_place,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "freeze" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_freeze_show(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "freeze" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--rows",
                    "--cols",
                    "--expect-state",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let rows = parse_i64_flag(rest, "--rows")?.unwrap_or(0);
            let cols = parse_i64_flag(rest, "--cols")?.unwrap_or(0);
            let expect_state = parse_string_flag(rest, "--expect-state")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_freeze_set(
                file,
                XlsxFreezeMutationOptions {
                    sheet: sheet.as_deref(),
                    rows,
                    cols,
                    expect_state: expect_state.as_deref(),
                    expect_state_present: value_flag_present(rest, "--expect-state"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "freeze" && verb == "clear" =>
        {
            reject_unknown_flags(
                rest,
                &["--sheet", "--expect-state", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let expect_state = parse_string_flag(rest, "--expect-state")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_freeze_clear(
                file,
                XlsxFreezeMutationOptions {
                    sheet: sheet.as_deref(),
                    rows: 0,
                    cols: 0,
                    expect_state: expect_state.as_deref(),
                    expect_state_present: value_flag_present(rest, "--expect-state"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file] if family == "xlsx" && group == "sheets" && verb == "list" => {
            xlsx_sheets_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "sheets" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_sheets_show(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "sheets" && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &["--name", "--after", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let name = parse_string_flag(rest, "--name")?;
            let after = parse_string_flag(rest, "--after")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_sheets_add(
                file,
                XlsxSheetsAddOptions {
                    name: name.as_deref(),
                    after: after.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "sheets" && verb == "rename" =>
        {
            reject_unknown_flags(
                rest,
                &["--sheet", "--name", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let name = parse_string_flag(rest, "--name")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_sheets_rename(
                file,
                XlsxSheetsRenameOptions {
                    sheet: sheet.as_deref(),
                    name: name.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "sheets" && verb == "move" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet", "--to", "--before", "--after", "--out", "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let to = parse_i64_flag(rest, "--to")?;
            let before = parse_string_flag(rest, "--before")?;
            let after = parse_string_flag(rest, "--after")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_sheets_move(
                file,
                XlsxSheetsMoveOptions {
                    sheet: sheet.as_deref(),
                    to,
                    to_present: value_flag_present(rest, "--to"),
                    before: before.as_deref(),
                    after: after.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "sheets" && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &["--sheet", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_sheets_delete(
                file,
                XlsxSheetsDeleteOptions {
                    sheet: sheet.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--scope-sheet"], &[])?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            xlsx_names_list(file, scope_sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--name",
                    "--ref",
                    "--sheet",
                    "--range",
                    "--scope-sheet",
                    "--comment",
                    "--out",
                    "--backup",
                ],
                &["--hidden", "--dry-run", "--no-validate", "--in-place"],
            )?;
            let name = parse_string_flag(rest, "--name")?;
            let ref_ = parse_string_flag(rest, "--ref")?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            let comment = parse_string_flag(rest, "--comment")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_names_add(
                file,
                XlsxNameMutationOptions {
                    name: name.as_deref(),
                    new_name: None,
                    ref_: ref_.as_deref(),
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    scope_sheet: scope_sheet.as_deref(),
                    expect_ref: None,
                    hidden: has_flag(rest, "--hidden"),
                    comment: comment.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "update" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--name",
                    "--ref",
                    "--sheet",
                    "--range",
                    "--scope-sheet",
                    "--expect-ref",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let name = parse_string_flag(rest, "--name")?;
            let ref_ = parse_string_flag(rest, "--ref")?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            let expect_ref = parse_string_flag(rest, "--expect-ref")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_names_update(
                file,
                XlsxNameMutationOptions {
                    name: name.as_deref(),
                    new_name: None,
                    ref_: ref_.as_deref(),
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    scope_sheet: scope_sheet.as_deref(),
                    expect_ref: expect_ref.as_deref(),
                    hidden: false,
                    comment: None,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "rename" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--name",
                    "--new-name",
                    "--scope-sheet",
                    "--expect-ref",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let name = parse_string_flag(rest, "--name")?;
            let new_name = parse_string_flag(rest, "--new-name")?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            let expect_ref = parse_string_flag(rest, "--expect-ref")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_names_rename(
                file,
                XlsxNameMutationOptions {
                    name: name.as_deref(),
                    new_name: new_name.as_deref(),
                    ref_: None,
                    sheet: None,
                    range: None,
                    scope_sheet: scope_sheet.as_deref(),
                    expect_ref: expect_ref.as_deref(),
                    hidden: false,
                    comment: None,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--name",
                    "--scope-sheet",
                    "--expect-ref",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let name = parse_string_flag(rest, "--name")?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            let expect_ref = parse_string_flag(rest, "--expect-ref")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_names_delete(
                file,
                XlsxNameMutationOptions {
                    name: name.as_deref(),
                    new_name: None,
                    ref_: None,
                    sheet: None,
                    range: None,
                    scope_sheet: scope_sheet.as_deref(),
                    expect_ref: expect_ref.as_deref(),
                    hidden: false,
                    comment: None,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--name", "--scope-sheet"], &[])?;
            let name = parse_string_flag(rest, "--name")?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            xlsx_names_show(file, name.as_deref().unwrap_or(""), scope_sheet.as_deref())
        }
        [family, group, ..] if family == "xlsx" && group == "tables" => dispatch_xlsx_tables(args),
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}

fn is_data_validations_group(group: &str) -> bool {
    matches!(
        group,
        "data-validations" | "data-validation" | "datavalidations" | "dv"
    )
}

#[allow(clippy::too_many_arguments)]
fn xlsx_data_validation_fields_from_flags<'a>(
    args: &[String],
    validation_type: Option<&'a str>,
    operator: Option<&'a str>,
    formula1: Option<&'a str>,
    formula2: Option<&'a str>,
    list_values: Option<&'a str>,
    list_range: Option<&'a str>,
    input_title: Option<&'a str>,
    input_message: Option<&'a str>,
    error_title: Option<&'a str>,
    error_message: Option<&'a str>,
    error_style: Option<&'a str>,
) -> XlsxDataValidationFields<'a> {
    XlsxDataValidationFields {
        validation_type,
        operator,
        formula1,
        formula2,
        list_values,
        list_range,
        allow_blank: has_flag(args, "--allow-blank"),
        show_input_message: has_flag(args, "--show-input-message"),
        show_error_message: has_flag(args, "--show-error-message"),
        prompt_title: input_title,
        prompt: input_message,
        error_title,
        error: error_message,
        error_style,
        set_type: value_flag_present(args, "--type"),
        set_operator: value_flag_present(args, "--operator"),
        set_formula1: value_flag_present(args, "--formula1"),
        set_formula2: value_flag_present(args, "--formula2"),
        set_list_values: value_flag_present(args, "--list-values"),
        set_list_range: value_flag_present(args, "--list-range"),
        set_allow_blank: value_flag_present(args, "--allow-blank"),
        set_show_input_message: value_flag_present(args, "--show-input-message"),
        set_show_error_message: value_flag_present(args, "--show-error-message"),
        set_prompt_title: value_flag_present(args, "--input-title"),
        set_prompt: value_flag_present(args, "--input-message"),
        set_error_title: value_flag_present(args, "--error-title"),
        set_error: value_flag_present(args, "--error-message"),
        set_error_style: value_flag_present(args, "--error-style"),
    }
}

fn empty_xlsx_data_validation_fields<'a>() -> XlsxDataValidationFields<'a> {
    XlsxDataValidationFields {
        validation_type: None,
        operator: None,
        formula1: None,
        formula2: None,
        list_values: None,
        list_range: None,
        allow_blank: false,
        show_input_message: false,
        show_error_message: false,
        prompt_title: None,
        prompt: None,
        error_title: None,
        error: None,
        error_style: None,
        set_type: false,
        set_operator: false,
        set_formula1: false,
        set_formula2: false,
        set_list_values: false,
        set_list_range: false,
        set_allow_blank: false,
        set_show_input_message: false,
        set_show_error_message: false,
        set_prompt_title: false,
        set_prompt: false,
        set_error_title: false,
        set_error: false,
        set_error_style: false,
    }
}

fn parse_f64_flag(args: &[String], name: &str) -> CliResult<Option<f64>> {
    parse_string_flag(args, name)?
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| CliError::invalid_args(format!("{name} must be a number")))
        })
        .transpose()
}

fn is_hyperlinks_group(group: &str) -> bool {
    matches!(group, "hyperlinks" | "hyperlink" | "links")
}
