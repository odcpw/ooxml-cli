use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::{
    XlsxCommentsAddOptions, XlsxCommentsRemoveOptions, XlsxCommentsUpdateOptions,
    xlsx_comments_add, xlsx_comments_list, xlsx_comments_remove, xlsx_comments_update,
};

pub(super) fn dispatch_xlsx_comments(args: &[String]) -> CliResult<Value> {
    match args {
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
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}
