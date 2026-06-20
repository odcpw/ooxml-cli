mod cells;
mod charts;
mod comments;
mod data_validations;
mod dimensions;
mod filters_sorts;
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
use self::data_validations::dispatch_xlsx_data_validations;
use self::dimensions::dispatch_xlsx_dimensions;
use self::filters_sorts::dispatch_xlsx_filters_sorts;
use self::freeze::dispatch_xlsx_freeze;
use self::hyperlinks::dispatch_xlsx_hyperlinks;
use self::names::dispatch_xlsx_names;
use self::pivots::dispatch_xlsx_pivots;
use self::ranges::dispatch_xlsx_ranges;
use self::sheets::dispatch_xlsx_sheets;
use self::tables::dispatch_xlsx_tables;
use self::workbook::dispatch_xlsx_workbook;
use crate::cli_core::{CliError, CliResult};

pub(super) fn dispatch_xlsx(args: &[String]) -> CliResult<Value> {
    match args {
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
        [family, group, ..] if family == "xlsx" && group == "filters-sorts" => {
            dispatch_xlsx_filters_sorts(args)
        }
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
