use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::xlsx_freeze::*;

pub(super) fn dispatch_xlsx_freeze(args: &[String]) -> CliResult<Value> {
    match args {
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
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}
