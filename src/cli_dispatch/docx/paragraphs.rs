use serde_json::Value;

use crate::cli_args::{
    flag_present, has_flag, parse_i64_flag, parse_string_flag, reject_unknown_flags,
};
use crate::cli_core::{CliError, CliResult};
use crate::docx_mutation_core::DocxParagraphMutationOptions;
use crate::docx_paragraph_commands::{
    docx_paragraphs_append, docx_paragraphs_clear, docx_paragraphs_insert, docx_paragraphs_set,
    resolve_required_docx_paragraph_set_text,
};

pub(super) fn dispatch_docx_paragraphs(args: &[String]) -> CliResult<Value> {
    match args {
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "paragraphs" && verb == "append" =>
        {
            reject_unknown_flags(
                rest,
                &["--text", "--text-file", "--style", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let style = parse_string_flag(rest, "--style")?.unwrap_or_default();
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_paragraphs_append(
                file,
                DocxParagraphMutationOptions {
                    text: text.as_deref(),
                    text_file: text_file.as_deref(),
                    style: &style,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    in_place,
                    no_validate,
                },
            )
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "paragraphs" && verb == "insert" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--insert-after",
                    "--text",
                    "--text-file",
                    "--style",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let insert_after = parse_i64_flag(rest, "--insert-after")?.unwrap_or(0);
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let style = parse_string_flag(rest, "--style")?.unwrap_or_default();
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_paragraphs_insert(
                file,
                insert_after,
                DocxParagraphMutationOptions {
                    text: text.as_deref(),
                    text_file: text_file.as_deref(),
                    style: &style,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    in_place,
                    no_validate,
                },
            )
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "paragraphs" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--index",
                    "--handle",
                    "--text",
                    "--text-file",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let index = parse_i64_flag(rest, "--index")?.unwrap_or(0);
            let handle = parse_string_flag(rest, "--handle")?;
            let handle_set = flag_present(rest, "--handle");
            let index_set = flag_present(rest, "--index");
            if !handle_set && index < 1 {
                return Err(CliError::invalid_args(
                    "--index must be >= 1 (or pass --handle)",
                ));
            }
            if handle_set && index_set {
                return Err(CliError::invalid_args(
                    "cannot specify both --index and --handle",
                ));
            }
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let replacement = resolve_required_docx_paragraph_set_text(
                text.as_deref(),
                text_file.as_deref(),
                flag_present(rest, "--text"),
                flag_present(rest, "--text-file"),
            )?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_paragraphs_set(
                file,
                index,
                handle.as_deref(),
                &replacement,
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
            if cmd == "docx" && group == "paragraphs" && verb == "clear" =>
        {
            reject_unknown_flags(
                rest,
                &["--index", "--handle", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let index = parse_i64_flag(rest, "--index")?.unwrap_or(0);
            let handle = parse_string_flag(rest, "--handle")?;
            let handle_set = flag_present(rest, "--handle");
            let index_set = flag_present(rest, "--index");
            if !handle_set && index < 1 {
                return Err(CliError::invalid_args(
                    "--index must be >= 1 (or pass --handle)",
                ));
            }
            if handle_set && index_set {
                return Err(CliError::invalid_args(
                    "cannot specify both --index and --handle",
                ));
            }
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_paragraphs_clear(
                file,
                index,
                handle.as_deref(),
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
