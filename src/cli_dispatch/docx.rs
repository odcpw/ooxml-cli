use serde_json::Value;
use std::fs;

use super::require_docx_block_hash;
use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::docx_block_commands::*;
use crate::docx_comments::*;
use crate::docx_fields::*;
use crate::docx_headers::*;
use crate::docx_images::*;
use crate::docx_mutation_core::*;
use crate::docx_paragraph_commands::*;
use crate::docx_styles::*;
use crate::docx_tables::*;
use crate::runtime_util::current_utc_rfc3339;

pub(super) fn dispatch_docx(args: &[String]) -> CliResult<Value> {
    match args {
        [cmd, family, file] if cmd == "docx" && family == "text" => docx_text(file),
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "blocks" && verb == "replace" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--block",
                    "--expect-hash",
                    "--text",
                    "--text-file",
                    "--style",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let block = parse_i64_flag(rest, "--block")?.unwrap_or(0);
            if block < 1 {
                return Err(CliError::invalid_args("--block must be >= 1"));
            }
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            require_docx_block_hash(&expect_hash)?;
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let style = parse_string_flag(rest, "--style")?.unwrap_or_default();
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_blocks_replace(
                file,
                block as usize,
                &expect_hash,
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
            if cmd == "docx" && group == "blocks" && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &["--block", "--expect-hash", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let block = parse_i64_flag(rest, "--block")?.unwrap_or(0);
            if block < 1 {
                return Err(CliError::invalid_args("--block must be >= 1"));
            }
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            require_docx_block_hash(&expect_hash)?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_blocks_delete(
                file,
                block as usize,
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
            if cmd == "docx" && group == "blocks" && verb == "insert-after" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--block",
                    "--expect-hash",
                    "--text",
                    "--text-file",
                    "--style",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let block = parse_i64_flag(rest, "--block")?.unwrap_or(0);
            if block < 0 {
                return Err(CliError::invalid_args("--block must be >= 0"));
            }
            let expect_hash_set = flag_present(rest, "--expect-hash");
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            if block > 0 {
                require_docx_block_hash(&expect_hash)?;
            } else if expect_hash_set {
                return Err(CliError::invalid_args(
                    "--expect-hash cannot be used with --block 0",
                ));
            }
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let style = parse_string_flag(rest, "--style")?.unwrap_or_default();
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_blocks_insert_after(
                file,
                block as usize,
                &expect_hash,
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
        [cmd, group, file, rest @ ..] if cmd == "docx" && group == "blocks" => {
            reject_unknown_flags(rest, &["--block"], &["--include-runs"])?;
            let block = parse_i64_flag(rest, "--block")?.unwrap_or(0);
            if block < 0 {
                return Err(CliError::invalid_args("--block must be >= 0"));
            }
            let include_runs = has_flag(rest, "--include-runs");
            docx_blocks_show(file, block as usize, include_runs)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "styles" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--type"], &[])?;
            let style_type = parse_string_flag(rest, "--type")?;
            docx_styles_list(file, style_type.as_deref())
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "styles" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--style"], &[])?;
            let style_id = parse_string_flag(rest, "--style")?
                .ok_or_else(|| CliError::invalid_args("--style is required"))?;
            docx_styles_show(file, &style_id)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "styles" && verb == "apply" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--index",
                    "--handle",
                    "--target",
                    "--style",
                    "--expect-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let index = parse_i64_flag(rest, "--index")?.unwrap_or(0);
            let handle = parse_string_flag(rest, "--handle")?;
            let handle_set = flag_present(rest, "--handle");
            let index_set = flag_present(rest, "--index");
            if handle_set && index_set {
                return Err(CliError::invalid_args(
                    "cannot specify both --index and --handle",
                ));
            }
            if !handle_set && index < 1 {
                return Err(CliError::invalid_args(
                    "--index must be >= 1 (or pass --handle)",
                ));
            }
            let target_arg = parse_string_flag(rest, "--target")?.unwrap_or_default();
            let target = normalize_docx_style_target(&target_arg)?;
            if handle_set && target == DocxStyleTarget::Table {
                return Err(CliError::invalid_args(
                    "--handle is a paragraph handle; use --index with --target table",
                ));
            }
            let style = parse_string_flag(rest, "--style")?.unwrap_or_default();
            if style.trim().is_empty() {
                return Err(CliError::invalid_args("--style is required"));
            }
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            if !expect_hash.is_empty() {
                require_docx_block_hash(&expect_hash)?;
            }
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_styles_apply(
                file,
                DocxStyleApplyOptions {
                    index,
                    handle: handle.as_deref(),
                    target,
                    style: &style,
                    expected_hash: &expect_hash,
                    validate_style: !no_validate,
                    mutation: DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: out.as_deref(),
                        backup: backup.as_deref(),
                        dry_run,
                        in_place,
                        no_validate,
                    },
                },
            )
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "comments" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--comment-id"], &[])?;
            let comment_id = parse_i64_flag(rest, "--comment-id")?;
            docx_comments_list(file, comment_id)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "comments" && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--anchor-block",
                    "--author",
                    "--initials",
                    "--date",
                    "--text",
                    "--text-file",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let anchor_block = parse_i64_flag(rest, "--anchor-block")?.unwrap_or(0);
            if flag_present(rest, "--anchor-block") && anchor_block < 1 {
                return Err(CliError::invalid_args("--anchor-block must be >= 1"));
            }
            let author = parse_string_flag(rest, "--author")?.unwrap_or_default();
            if author.is_empty() {
                return Err(CliError::invalid_args("--author is required"));
            }
            let initials = parse_string_flag(rest, "--initials")?.unwrap_or_default();
            let date = parse_string_flag(rest, "--date")?.unwrap_or_else(current_utc_rfc3339);
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_comments_add(
                file,
                anchor_block,
                &author,
                &initials,
                &date,
                DocxParagraphMutationOptions {
                    text: text.as_deref(),
                    text_file: text_file.as_deref(),
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
            if cmd == "docx" && group == "comments" && verb == "edit" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--comment-id",
                    "--handle",
                    "--text",
                    "--text-file",
                    "--author",
                    "--date",
                    "--expect-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let comment_id_set = flag_present(rest, "--comment-id");
            let handle_set = flag_present(rest, "--handle");
            if handle_set && comment_id_set {
                return Err(CliError::invalid_args(
                    "cannot specify both --comment-id and --handle",
                ));
            }
            if !handle_set && !comment_id_set {
                return Err(CliError::invalid_args(
                    "--comment-id is required (or pass --handle)",
                ));
            }
            let comment_id = parse_i64_flag(rest, "--comment-id")?.unwrap_or(0);
            if !handle_set && comment_id < 0 {
                return Err(CliError::invalid_args("--comment-id must be >= 0"));
            }
            let text_set = flag_present(rest, "--text");
            let text_file_set = flag_present(rest, "--text-file");
            let author_set = flag_present(rest, "--author");
            let date_set = flag_present(rest, "--date");
            if text_set && text_file_set {
                return Err(CliError::invalid_args(
                    "cannot specify both --text and --text-file",
                ));
            }
            if !text_set && !text_file_set && !author_set && !date_set {
                return Err(CliError::invalid_args(
                    "specify at least one of --text, --text-file, --author, or --date",
                ));
            }
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let resolved_text = if text_file_set {
                let path = text_file.as_deref().unwrap_or_default();
                fs::read(path)
                    .map(|data| String::from_utf8_lossy(&data).to_string())
                    .map_err(|_| CliError::file_not_found(format!("file not found: {path}")))?
            } else {
                text.unwrap_or_default()
            };
            let handle = parse_string_flag(rest, "--handle")?;
            let author = parse_string_flag(rest, "--author")?.unwrap_or_default();
            let date = parse_string_flag(rest, "--date")?.unwrap_or_default();
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_comments_edit(
                file,
                comment_id,
                handle.as_deref(),
                DocxCommentEditSpec {
                    expect_hash: &expect_hash,
                    text: &resolved_text,
                    text_set: text_set || text_file_set,
                    author: &author,
                    author_set,
                    date: &date,
                    date_set,
                },
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
            if cmd == "docx" && group == "comments" && verb == "remove" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--comment-id",
                    "--handle",
                    "--expect-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let comment_id_set = flag_present(rest, "--comment-id");
            let handle_set = flag_present(rest, "--handle");
            if handle_set && comment_id_set {
                return Err(CliError::invalid_args(
                    "cannot specify both --comment-id and --handle",
                ));
            }
            if !handle_set && !comment_id_set {
                return Err(CliError::invalid_args(
                    "--comment-id is required (or pass --handle)",
                ));
            }
            let comment_id = parse_i64_flag(rest, "--comment-id")?.unwrap_or(0);
            if !handle_set && comment_id < 0 {
                return Err(CliError::invalid_args("--comment-id must be >= 0"));
            }
            let handle = parse_string_flag(rest, "--handle")?;
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_comments_remove(
                file,
                comment_id,
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
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "fields" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--type"], &[])?;
            let field_type = parse_string_flag(rest, "--type")?;
            docx_fields_list(file, field_type.as_deref())
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "fields" && verb == "insert" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--location",
                    "--field-code",
                    "--result",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let location = parse_string_flag(rest, "--location")?.ok_or_else(|| {
                CliError::invalid_args("--location is required (e.g. body:2 or header1:1)")
            })?;
            let field_code = parse_string_flag(rest, "--field-code")?
                .ok_or_else(|| CliError::invalid_args("--field-code is required (e.g. PAGE)"))?;
            let result = parse_string_flag(rest, "--result")?.unwrap_or_default();
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_fields_insert(
                file,
                &location,
                &field_code,
                &result,
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
            if cmd == "docx" && group == "fields" && verb == "set-result" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--selector",
                    "--result",
                    "--expect-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let selector = parse_string_flag(rest, "--selector")?.ok_or_else(|| {
                CliError::invalid_args("--selector is required (e.g. body:1:0 or header1:1:0)")
            })?;
            if !value_flag_present(rest, "--result") {
                return Err(CliError::invalid_args("--result is required"));
            }
            let result = parse_string_flag(rest, "--result")?.unwrap_or_default();
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_fields_set_result(
                file,
                &selector,
                &result,
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
            if cmd == "docx" && (group == "headers" || group == "footers") && verb == "list" =>
        {
            reject_unknown_flags(rest, &[], &[])?;
            docx_headers_footers_list(file)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && (group == "headers" || group == "footers") && verb == "show" =>
        {
            docx_headers_footers_show(file, docx_header_footer_kind(group), rest)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx"
                && (group == "headers" || group == "footers")
                && verb == "set-text" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--id",
                    "--type",
                    "--section",
                    "--index",
                    "--selector",
                    "--text",
                    "--text-file",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let id = parse_string_flag(rest, "--id")?.unwrap_or_default();
            let ref_type =
                parse_string_flag(rest, "--type")?.unwrap_or_else(|| "default".to_string());
            let ref_type = normalize_docx_header_footer_show_type(&ref_type)?;
            let section = parse_i64_flag(rest, "--section")?.unwrap_or(0);
            let index = parse_i64_flag(rest, "--index")?.unwrap_or(1);
            if index < 1 {
                return Err(CliError::invalid_args("--index must be >= 1"));
            }
            if section < 0 {
                return Err(CliError::invalid_args(
                    "--section must be >= 0 (0 means the last section)",
                ));
            }
            let selector = parse_string_flag(rest, "--selector")?;
            if selector.is_some()
                && (parse_string_flag(rest, "--id")?.is_some()
                    || parse_string_flag(rest, "--type")?.is_some()
                    || parse_string_flag(rest, "--section")?.is_some())
            {
                return Err(CliError::invalid_args(
                    "cannot specify --selector with --id, --type, or --section",
                ));
            }
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let text = resolve_required_docx_table_text(
                text.as_deref(),
                text_file.as_deref(),
                parse_string_flag(rest, "--text")?.is_some(),
                parse_string_flag(rest, "--text-file")?.is_some(),
            )?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            docx_headers_footers_set_text(
                file,
                docx_header_footer_kind(group),
                DocxHeaderFooterSetTextOptions {
                    id: &id,
                    ref_type: &ref_type,
                    section,
                    index,
                    selector: selector.as_deref(),
                    selector_given: selector.is_some(),
                    index_given: parse_string_flag(rest, "--index")?.is_some(),
                    text: &text,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    in_place,
                    no_validate,
                },
            )
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "images" && verb == "list" =>
        {
            reject_unknown_flags(rest, &[], &[])?;
            docx_images_list(file)
        }
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
