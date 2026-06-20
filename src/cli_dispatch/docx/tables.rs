use serde_json::Value;

use super::super::require_docx_block_hash;
use crate::cli_args::{
    flag_present, has_flag, parse_i64_flag, parse_string_flag, reject_unknown_flags,
    validate_positive_i64,
};
use crate::cli_core::{CliError, CliResult};
use crate::docx_mutation_core::{DocxParagraphMutationOptions, resolve_required_docx_table_text};
use crate::docx_tables::{
    docx_tables_clear_cell, docx_tables_delete_row, docx_tables_insert_row, docx_tables_set_cell,
    docx_tables_show,
};

pub(super) fn dispatch_docx_tables(args: &[String]) -> CliResult<Value> {
    match args {
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "tables" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--table"], &["--details"])?;
            let table = parse_i64_flag(rest, "--table")?.unwrap_or(0);
            if table < 0 {
                return Err(CliError::invalid_args("--table must be positive"));
            }
            let include_details = has_flag(rest, "--details");
            docx_tables_show(file, table as usize, include_details)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "tables" && verb == "set-cell" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--table",
                    "--row",
                    "--col",
                    "--expect-hash",
                    "--text",
                    "--text-file",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let table = parse_i64_flag(rest, "--table")?.unwrap_or(0);
            let row = parse_i64_flag(rest, "--row")?.unwrap_or(0);
            let col = parse_i64_flag(rest, "--col")?.unwrap_or(0);
            validate_positive_i64(table, "--table")?;
            validate_positive_i64(row, "--row")?;
            validate_positive_i64(col, "--col")?;
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            require_docx_block_hash(&expect_hash)?;
            let text_changed = flag_present(rest, "--text");
            let text_file_changed = flag_present(rest, "--text-file");
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let text = resolve_required_docx_table_text(
                text.as_deref(),
                text_file.as_deref(),
                text_changed,
                text_file_changed,
            )?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_tables_set_cell(
                file,
                table as usize,
                row as usize,
                col as usize,
                &expect_hash,
                &text,
                DocxParagraphMutationOptions {
                    text: None,
                    text_file: None,
                    style: "",
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    in_place,
                    no_validate,
                },
            )
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "tables" && verb == "clear-cell" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--table",
                    "--row",
                    "--col",
                    "--expect-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let table = parse_i64_flag(rest, "--table")?.unwrap_or(0);
            let row = parse_i64_flag(rest, "--row")?.unwrap_or(0);
            let col = parse_i64_flag(rest, "--col")?.unwrap_or(0);
            validate_positive_i64(table, "--table")?;
            validate_positive_i64(row, "--row")?;
            validate_positive_i64(col, "--col")?;
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            require_docx_block_hash(&expect_hash)?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_tables_clear_cell(
                file,
                table as usize,
                row as usize,
                col as usize,
                &expect_hash,
                DocxParagraphMutationOptions {
                    text: None,
                    text_file: None,
                    style: "",
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    in_place,
                    no_validate,
                },
            )
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "tables" && verb == "insert-row" =>
        {
            reject_unknown_flags(
                rest,
                &["--table", "--at", "--expect-hash", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let table = parse_i64_flag(rest, "--table")?.unwrap_or(0);
            let at = parse_i64_flag(rest, "--at")?.unwrap_or(0);
            validate_positive_i64(table, "--table")?;
            validate_positive_i64(at, "--at")?;
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            require_docx_block_hash(&expect_hash)?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_tables_insert_row(
                file,
                table as usize,
                at as usize,
                &expect_hash,
                DocxParagraphMutationOptions {
                    text: None,
                    text_file: None,
                    style: "",
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    in_place,
                    no_validate,
                },
            )
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "tables" && verb == "delete-row" =>
        {
            reject_unknown_flags(
                rest,
                &["--table", "--row", "--expect-hash", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let table = parse_i64_flag(rest, "--table")?.unwrap_or(0);
            let row = parse_i64_flag(rest, "--row")?.unwrap_or(0);
            validate_positive_i64(table, "--table")?;
            validate_positive_i64(row, "--row")?;
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            require_docx_block_hash(&expect_hash)?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_tables_delete_row(
                file,
                table as usize,
                row as usize,
                &expect_hash,
                DocxParagraphMutationOptions {
                    text: None,
                    text_file: None,
                    style: "",
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    in_place,
                    no_validate,
                },
            )
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}
