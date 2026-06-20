use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::{XlsxPivotsCreateOptions, xlsx_pivots_create, xlsx_pivots_list, xlsx_pivots_show};

pub(super) fn dispatch_xlsx_pivots(args: &[String]) -> CliResult<Value> {
    match args {
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "pivots" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--sheet"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_pivots_list(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "pivots" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--pivot"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let pivot = parse_string_flag(rest, "--pivot")?;
            xlsx_pivots_show(file, sheet.as_deref(), pivot.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "pivots" && verb == "create" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--table",
                    "--target-sheet",
                    "--anchor",
                    "--name",
                    "--rows",
                    "--cols",
                    "--filters",
                    "--values",
                    "--expect-source-range",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let table = parse_string_flag(rest, "--table")?;
            let target_sheet = parse_string_flag(rest, "--target-sheet")?;
            let anchor = parse_string_flag(rest, "--anchor")?;
            let name = parse_string_flag(rest, "--name")?;
            let rows = parse_string_flag(rest, "--rows")?;
            let cols = parse_string_flag(rest, "--cols")?;
            let filters = parse_string_flag(rest, "--filters")?;
            let values = parse_string_flag(rest, "--values")?;
            let expect_source_range = parse_string_flag(rest, "--expect-source-range")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_pivots_create(
                file,
                XlsxPivotsCreateOptions {
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    table: table.as_deref(),
                    target_sheet: target_sheet.as_deref(),
                    anchor: anchor.as_deref(),
                    name: name.as_deref(),
                    rows: rows.as_deref(),
                    cols: cols.as_deref(),
                    filters: filters.as_deref(),
                    values: values.as_deref(),
                    expect_source_range: expect_source_range.as_deref(),
                    max_cells,
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
