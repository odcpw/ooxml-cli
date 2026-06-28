mod cells;
mod charts;
mod comments;
mod conditional_formatting;
mod data_validations;
mod dimensions;
mod filters_sorts;
mod forms;
mod freeze;
mod hyperlinks;
mod names;
mod pivots;
mod ranges;
mod sheets;
mod tables;
mod workbook;

use serde_json::Value;

use self::cells::dispatch_xlsx_cells;
use self::charts::dispatch_xlsx_charts;
use self::comments::dispatch_xlsx_comments;
use self::conditional_formatting::dispatch_xlsx_conditional_formatting;
use self::data_validations::dispatch_xlsx_data_validations;
use self::dimensions::dispatch_xlsx_dimensions;
use self::filters_sorts::dispatch_xlsx_filters_sorts;
use self::forms::dispatch_xlsx_forms;
use self::freeze::dispatch_xlsx_freeze;
use self::hyperlinks::dispatch_xlsx_hyperlinks;
use self::names::dispatch_xlsx_names;
use self::pivots::dispatch_xlsx_pivots;
use self::ranges::dispatch_xlsx_ranges;
use self::sheets::dispatch_xlsx_sheets;
use self::tables::dispatch_xlsx_tables;
use self::workbook::dispatch_xlsx_workbook;
use crate::cli_args::{has_flag, output_path_arg, parse_string_flag, reject_unknown_flags};
use crate::cli_core::{CliError, CliResult};
use crate::{XlsxScaffoldOptions, xlsx_scaffold};

pub(super) fn dispatch_xlsx(args: &[String]) -> CliResult<Value> {
    match args {
        [family, verb, rest @ ..] if family == "xlsx" && verb == "scaffold" => {
            let value_flags = ["--out", "--sheet"];
            let bool_flags = ["--force", "--no-validate"];
            reject_unknown_flags(rest, &value_flags, &bool_flags)?;
            let output = output_path_arg(rest, &value_flags, &bool_flags, "xlsx scaffold")?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_scaffold(
                &output,
                XlsxScaffoldOptions {
                    sheet: sheet.as_deref(),
                    force: has_flag(rest, "--force"),
                    no_validate: has_flag(rest, "--no-validate"),
                },
            )
        }
        [family, group, ..] if family == "xlsx" && group == "ranges" => dispatch_xlsx_ranges(args),
        [family, group, ..] if family == "xlsx" && group == "workbook" => {
            dispatch_xlsx_workbook(args)
        }
        [family, group, ..] if family == "xlsx" && group == "pivots" => dispatch_xlsx_pivots(args),
        [family, group, ..]
            if family == "xlsx"
                && matches!(
                    group.as_str(),
                    "colwidths"
                        | "rowheights"
                        | "rows"
                        | "row"
                        | "cols"
                        | "col"
                        | "columns"
                        | "column"
                ) =>
        {
            dispatch_xlsx_dimensions(args)
        }
        [family, group, ..] if family == "xlsx" && group == "charts" => dispatch_xlsx_charts(args),
        [family, group, ..] if family == "xlsx" && group == "comments" => {
            dispatch_xlsx_comments(args)
        }
        [family, group, ..]
            if family == "xlsx"
                && matches!(group.as_str(), "hyperlinks" | "hyperlink" | "links") =>
        {
            dispatch_xlsx_hyperlinks(args)
        }
        [family, group, ..]
            if family == "xlsx"
                && matches!(
                    group.as_str(),
                    "data-validations" | "data-validation" | "datavalidations" | "dv"
                ) =>
        {
            dispatch_xlsx_data_validations(args)
        }
        [family, group, ..]
            if family == "xlsx"
                && matches!(
                    group.as_str(),
                    "conditional-formats" | "conditional-formatting" | "conditional-format" | "cf"
                ) =>
        {
            dispatch_xlsx_conditional_formatting(args)
        }
        [family, group, ..] if family == "xlsx" && group == "filters-sorts" => {
            dispatch_xlsx_filters_sorts(args)
        }
        [family, group, ..] if family == "xlsx" && group == "forms" => dispatch_xlsx_forms(args),
        [family, group, ..] if family == "xlsx" && group == "cells" => dispatch_xlsx_cells(args),
        [family, group, ..] if family == "xlsx" && group == "freeze" => dispatch_xlsx_freeze(args),
        [family, group, ..] if family == "xlsx" && group == "sheets" => dispatch_xlsx_sheets(args),
        [family, group, ..]
            if family == "xlsx" && matches!(group.as_str(), "names" | "defined-names") =>
        {
            dispatch_xlsx_names(args)
        }
        [family, group, ..] if family == "xlsx" && group == "tables" => dispatch_xlsx_tables(args),
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}
