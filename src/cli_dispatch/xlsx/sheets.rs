use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::{
    XlsxSheetsAddOptions, XlsxSheetsDeleteOptions, XlsxSheetsMoveOptions, XlsxSheetsRenameOptions,
    xlsx_sheets_add, xlsx_sheets_delete, xlsx_sheets_list, xlsx_sheets_move, xlsx_sheets_rename,
    xlsx_sheets_show,
};

pub(super) fn dispatch_xlsx_sheets(args: &[String]) -> CliResult<Value> {
    match args {
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
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}
