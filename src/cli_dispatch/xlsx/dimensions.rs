use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::{
    XlsxColWidthsSetOptions, XlsxColsDeleteOptions, XlsxColsInsertOptions,
    XlsxRowHeightsSetOptions, XlsxRowsDeleteOptions, XlsxRowsInsertOptions, xlsx_cols_delete,
    xlsx_cols_insert, xlsx_colwidths_set, xlsx_colwidths_show, xlsx_rowheights_set,
    xlsx_rowheights_show, xlsx_rows_delete, xlsx_rows_insert,
};

pub(super) fn dispatch_xlsx_dimensions(args: &[String]) -> CliResult<Value> {
    match args {
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
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
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
