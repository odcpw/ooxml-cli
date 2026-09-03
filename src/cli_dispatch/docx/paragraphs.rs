use serde_json::Value;

use crate::cli_args::{
    flag_present, has_flag, parse_i64_flag, parse_string_flag, reject_unknown_flags,
};
use crate::cli_core::{CliError, CliResult};
use crate::docx_mutation_core::DocxParagraphMutationOptions;
use crate::docx_paragraph_commands::{
    DocxParagraphInsertOptions, docx_paragraphs_append, docx_paragraphs_clear,
    docx_paragraphs_insert, docx_paragraphs_set, resolve_required_docx_paragraph_set_text,
};

pub(super) fn dispatch_docx_paragraphs(args: &[String]) -> CliResult<Value> {
    match args {
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "paragraphs" && verb == "append" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--text",
                    "--text-file",
                    "--style",
                    "--list",
                    "--level",
                    "--out",
                    "--backup",
                ],
                &[
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                    "--create-style",
                    "--restart",
                ],
            )?;
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let style = parse_string_flag(rest, "--style")?.unwrap_or_default();
            let list = parse_string_flag(rest, "--list")?;
            let level = parse_list_level(rest)?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_paragraphs_append(
                file,
                list.as_deref(),
                level,
                has_flag(rest, "--restart"),
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
                has_flag(rest, "--create-style"),
            )
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "paragraphs" && verb == "insert" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--after",
                    "--insert-after",
                    "--expect-hash",
                    "--text",
                    "--text-file",
                    "--style",
                    "--list",
                    "--level",
                    "--out",
                    "--backup",
                ],
                &[
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                    "--create-style",
                    "--restart",
                ],
            )?;
            let after = parse_i64_flag(rest, "--after")?;
            let insert_after_alias = parse_i64_flag(rest, "--insert-after")?;
            if after.is_some() && insert_after_alias.is_some() {
                return Err(CliError::invalid_args(
                    "cannot specify both --after and --insert-after",
                ));
            }
            let insert_after = after.or(insert_after_alias).unwrap_or(0);
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            if !expect_hash.is_empty() {
                crate::require_docx_block_hash(&expect_hash)?;
            }
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let style = parse_string_flag(rest, "--style")?.unwrap_or_default();
            let list = parse_string_flag(rest, "--list")?;
            let level = parse_list_level(rest)?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_paragraphs_insert(
                file,
                DocxParagraphInsertOptions {
                    insert_after,
                    expected_hash: &expect_hash,
                    list: list.as_deref(),
                    level,
                    restart: has_flag(rest, "--restart"),
                    mutation: DocxParagraphMutationOptions {
                        text: text.as_deref(),
                        text_file: text_file.as_deref(),
                        style: &style,
                        out: out.as_deref(),
                        backup: backup.as_deref(),
                        dry_run,
                        in_place,
                        no_validate,
                    },
                    create_style: has_flag(rest, "--create-style"),
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
                    "--expect-hash",
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
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            if !expect_hash.is_empty() {
                crate::require_docx_block_hash(&expect_hash)?;
            }
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
            if cmd == "docx" && group == "paragraphs" && verb == "clear" =>
        {
            reject_unknown_flags(
                rest,
                &["--index", "--handle", "--expect-hash", "--out", "--backup"],
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
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            if !expect_hash.is_empty() {
                crate::require_docx_block_hash(&expect_hash)?;
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

fn parse_list_level(args: &[String]) -> CliResult<u32> {
    let level = parse_i64_flag(args, "--level")?.unwrap_or(0);
    u32::try_from(level).map_err(|_| CliError::invalid_args("--level must be 0, 1, or 2"))
}
