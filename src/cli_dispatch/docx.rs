mod comments;
mod paragraphs;
mod tables;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

use super::require_docx_block_hash;
use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::docx_authoring::*;
use crate::docx_block_commands::*;
use crate::docx_fields::*;
use crate::docx_headers::*;
use crate::docx_images::*;
use crate::docx_mutation_core::*;
use crate::docx_replace::*;
use crate::docx_styles::*;
use crate::{find_docx_document_part, zip_entry_names};
use comments::dispatch_docx_comments;
use paragraphs::dispatch_docx_paragraphs;
use tables::dispatch_docx_tables;

pub(super) fn dispatch_docx(args: &[String]) -> CliResult<Value> {
    let (dispatch_args, guard) = docx_guard_args(args)?;
    preflight_docx_document_guard(&dispatch_args, &guard)?;
    let mut result = dispatch_docx_inner(&dispatch_args)?;
    enrich_docx_result(&dispatch_args, &guard, &mut result)?;
    Ok(result)
}

fn dispatch_docx_inner(args: &[String]) -> CliResult<Value> {
    match args {
        [cmd, verb, rest @ ..] if cmd == "docx" && verb == "scaffold" => {
            let value_flags = [
                "--out",
                "--text",
                "--text-file",
                "--theme",
                "--theme-seed",
                "--template",
                "--brand",
            ];
            let bool_flags = ["--force", "--no-validate"];
            reject_unknown_flags(rest, &value_flags, &bool_flags)?;
            let output = output_path_arg(rest, &value_flags, &bool_flags, "docx scaffold")?;
            let text = parse_string_flag(rest, "--text")?;
            let text_file = parse_string_flag(rest, "--text-file")?;
            let theme = parse_string_flag(rest, "--theme")?;
            let theme_seed = parse_string_flag(rest, "--theme-seed")?;
            let template = parse_string_flag(rest, "--template")?;
            let brand = parse_string_flag(rest, "--brand")?;
            docx_scaffold(
                &output,
                DocxScaffoldOptions {
                    text: text.as_deref(),
                    text_file: text_file.as_deref(),
                    theme: theme.as_deref(),
                    theme_seed: theme_seed.as_deref(),
                    template: template.as_deref(),
                    brand: brand.as_deref(),
                    force: has_flag(rest, "--force"),
                    no_validate: has_flag(rest, "--no-validate"),
                },
            )
        }
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
                &["--dry-run", "--in-place", "--no-validate", "--create-style"],
            )?;
            let block = parse_i64_flag(rest, "--block")?.unwrap_or(0);
            if block < 1 {
                return Err(CliError::invalid_args("--block must be >= 1"));
            }
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            if !expect_hash.is_empty() {
                require_docx_block_hash(&expect_hash)?;
            }
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
                has_flag(rest, "--create-style"),
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
            if !expect_hash.is_empty() {
                require_docx_block_hash(&expect_hash)?;
            }
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
                &["--dry-run", "--in-place", "--no-validate", "--create-style"],
            )?;
            let block = parse_i64_flag(rest, "--block")?.unwrap_or(0);
            if block < 0 {
                return Err(CliError::invalid_args("--block must be >= 0"));
            }
            let expect_hash_set = flag_present(rest, "--expect-hash");
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            if block > 0 {
                if !expect_hash.is_empty() {
                    require_docx_block_hash(&expect_hash)?;
                }
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
                has_flag(rest, "--create-style"),
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
                &["--dry-run", "--in-place", "--no-validate", "--create-style"],
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
                    create_style: has_flag(rest, "--create-style"),
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
        [cmd, group, ..] if cmd == "docx" && group == "comments" => dispatch_docx_comments(args),
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
                    "--levels",
                    "--out",
                    "--backup",
                ],
                &["--toc", "--dry-run", "--in-place", "--no-validate"],
            )?;
            let location = parse_string_flag(rest, "--location")?;
            let field_code = parse_string_flag(rest, "--field-code")?;
            let result = parse_string_flag(rest, "--result")?;
            let levels = parse_string_flag(rest, "--levels")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let in_place = has_flag(rest, "--in-place");
            let no_validate = has_flag(rest, "--no-validate");
            let mutation = DocxParagraphMutationOptions {
                text: None,
                text_file: None,
                style: "",
                out: out.as_deref(),
                backup: backup.as_deref(),
                dry_run,
                in_place,
                no_validate,
            };
            if has_flag(rest, "--toc") {
                if location.is_some() || field_code.is_some() || result.is_some() {
                    return Err(CliError::invalid_args(
                        "--toc cannot be combined with --location, --field-code, or --result",
                    ));
                }
                docx_fields_insert_toc(file, levels.as_deref().unwrap_or("1-3"), mutation)
            } else {
                if levels.is_some() {
                    return Err(CliError::invalid_args("--levels requires --toc"));
                }
                let location = location.ok_or_else(|| {
                    CliError::invalid_args("--location is required (e.g. body:2 or header1:1)")
                })?;
                let field_code = field_code.ok_or_else(|| {
                    CliError::invalid_args("--field-code is required (e.g. PAGE)")
                })?;
                docx_fields_insert(
                    file,
                    &location,
                    &field_code,
                    result.as_deref().unwrap_or(""),
                    mutation,
                )
            }
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
                &["--page-numbers", "--dry-run", "--in-place", "--no-validate"],
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
            let page_numbers = has_flag(rest, "--page-numbers");
            let text_set = parse_string_flag(rest, "--text")?.is_some();
            let text_file_set = parse_string_flag(rest, "--text-file")?.is_some();
            let text = if page_numbers {
                if group != "footers" || text_set || text_file_set {
                    return Err(CliError::invalid_args(
                        "--page-numbers is footer-only and cannot be combined with --text or --text-file",
                    ));
                }
                "Page 1 of 1".to_string()
            } else {
                resolve_required_docx_table_text(
                    text.as_deref(),
                    text_file.as_deref(),
                    text_set,
                    text_file_set,
                )?
            };
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
                    page_numbers,
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
            if cmd == "docx" && group == "images" && verb == "replace" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--image",
                    "--file",
                    "--expect-hash",
                    "--width",
                    "--height",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let image = parse_string_flag(rest, "--image")?.ok_or_else(|| {
                CliError::invalid_args("--image is required (1-based index or relationship id)")
            })?;
            if image.trim().is_empty() {
                return Err(CliError::invalid_args(
                    "--image is required (1-based index or relationship id)",
                ));
            }
            let image_file = parse_string_flag(rest, "--file")?
                .ok_or_else(|| CliError::invalid_args("--file is required"))?;
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            require_docx_image_hash_format(&expect_hash)?;
            let width = parse_i64_flag(rest, "--width")?.unwrap_or(0);
            let height = parse_i64_flag(rest, "--height")?.unwrap_or(0);
            if width < 0 || height < 0 {
                return Err(CliError::invalid_args(
                    "--width and --height must be >= 0 (EMU)",
                ));
            }
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            docx_images_replace(
                file,
                &image,
                &image_file,
                &expect_hash,
                width,
                height,
                DocxParagraphMutationOptions {
                    text: None,
                    text_file: None,
                    style: "",
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    in_place: has_flag(rest, "--in-place"),
                    no_validate: has_flag(rest, "--no-validate"),
                },
            )
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "images" && verb == "insert" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--after",
                    "--file",
                    "--expect-hash",
                    "--width",
                    "--height",
                    "--caption",
                    "--align",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let after = parse_i64_flag(rest, "--after")?.unwrap_or(0);
            if after < 0 {
                return Err(CliError::invalid_args("--after must be >= 0"));
            }
            let image_file = parse_string_flag(rest, "--file")?
                .ok_or_else(|| CliError::invalid_args("--file is required"))?;
            let width = parse_i64_flag(rest, "--width")?.unwrap_or(0);
            let height = parse_i64_flag(rest, "--height")?.unwrap_or(0);
            if width <= 0 || height <= 0 {
                return Err(CliError::invalid_args(
                    "--width and --height are required and must be > 0 (EMU)",
                ));
            }
            let expect_hash = parse_string_flag(rest, "--expect-hash")?.unwrap_or_default();
            let caption = parse_string_flag(rest, "--caption")?;
            let align = parse_string_flag(rest, "--align")?.unwrap_or_default();
            if after > 0 {
                if !expect_hash.is_empty() {
                    require_docx_block_hash(&expect_hash)?;
                }
            } else if value_flag_present(rest, "--expect-hash") {
                return Err(CliError::invalid_args(
                    "--expect-hash cannot be used with --after 0",
                ));
            }
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            docx_images_insert(
                file,
                DocxImageInsertOptions {
                    after: after as usize,
                    image_file: &image_file,
                    expected_hash: &expect_hash,
                    width,
                    height,
                    caption: caption.as_deref(),
                    align: &align,
                    mutation: DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: out.as_deref(),
                        backup: backup.as_deref(),
                        dry_run: has_flag(rest, "--dry-run"),
                        in_place: has_flag(rest, "--in-place"),
                        no_validate: has_flag(rest, "--no-validate"),
                    },
                },
            )
        }
        [cmd, verb, file, rest @ ..] if cmd == "docx" && verb == "replace" => {
            reject_unknown_flags(
                rest,
                &["--find", "--replace", "--expect-count", "--out", "--backup"],
                &[
                    "--regex",
                    "--match-case",
                    "--whole-word",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            let find = parse_string_flag(rest, "--find")?.unwrap_or_default();
            if !value_flag_present(rest, "--find") || find.is_empty() {
                return Err(CliError::invalid_args(
                    "--find is required and cannot be empty",
                ));
            }
            let replace = parse_string_flag(rest, "--replace")?.unwrap_or_default();
            let expect_count = if value_flag_present(rest, "--expect-count") {
                let value = parse_i64_flag(rest, "--expect-count")?.unwrap_or(0);
                if value < 0 {
                    return Err(CliError::invalid_args("--expect-count must be >= 0"));
                }
                Some(value as usize)
            } else {
                None
            };
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            docx_replace(
                file,
                DocxReplaceOptions {
                    find: &find,
                    replace: &replace,
                    regex: has_flag(rest, "--regex"),
                    match_case: has_flag(rest, "--match-case"),
                    whole_word: has_flag(rest, "--whole-word"),
                    expect_count,
                    mutation: DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: out.as_deref(),
                        backup: backup.as_deref(),
                        dry_run: has_flag(rest, "--dry-run"),
                        in_place: has_flag(rest, "--in-place"),
                        no_validate: has_flag(rest, "--no-validate"),
                    },
                },
            )
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "breaks" && verb == "insert" =>
        {
            reject_unknown_flags(
                rest,
                &["--out", "--backup"],
                &[
                    "--page",
                    "--section",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            docx_break_insert(
                file,
                has_flag(rest, "--page"),
                has_flag(rest, "--section"),
                DocxParagraphMutationOptions {
                    text: None,
                    text_file: None,
                    style: "",
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    in_place: has_flag(rest, "--in-place"),
                    no_validate: has_flag(rest, "--no-validate"),
                },
            )
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "sections" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--section",
                    "--orientation",
                    "--size",
                    "--margins",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let section = parse_i64_flag(rest, "--section")?.unwrap_or(0);
            let orientation = parse_string_flag(rest, "--orientation")?
                .ok_or_else(|| CliError::invalid_args("--orientation is required"))?;
            let size = parse_string_flag(rest, "--size")?
                .ok_or_else(|| CliError::invalid_args("--size is required"))?;
            let margins = parse_string_flag(rest, "--margins")?
                .ok_or_else(|| CliError::invalid_args("--margins is required"))?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            docx_section_set(
                file,
                DocxSectionSetupOptions {
                    section,
                    orientation: &orientation,
                    size: &size,
                    margins: &margins,
                    mutation: DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: out.as_deref(),
                        backup: backup.as_deref(),
                        dry_run: has_flag(rest, "--dry-run"),
                        in_place: has_flag(rest, "--in-place"),
                        no_validate: has_flag(rest, "--no-validate"),
                    },
                },
            )
        }
        [cmd, group, ..] if cmd == "docx" && group == "tables" => dispatch_docx_tables(args),
        [cmd, group, ..] if cmd == "docx" && group == "paragraphs" => {
            dispatch_docx_paragraphs(args)
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}

#[derive(Default)]
struct DocxGuardArgs {
    expected_document_hash: Option<String>,
    require_guard: bool,
}

fn docx_guard_args(args: &[String]) -> CliResult<(Vec<String>, DocxGuardArgs)> {
    let mut dispatch_args = Vec::with_capacity(args.len());
    let mut guard = DocxGuardArgs::default();
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--expect-document-hash" => {
                if guard.expected_document_hash.is_some() {
                    return Err(CliError::invalid_args(
                        "--expect-document-hash may be specified only once",
                    ));
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::invalid_args("--expect-document-hash requires a value")
                })?;
                require_docx_document_hash(value)?;
                guard.expected_document_hash = Some(value.clone());
                index += 2;
            }
            "--require-guard" => {
                if guard.require_guard {
                    return Err(CliError::invalid_args(
                        "--require-guard may be specified only once",
                    ));
                }
                guard.require_guard = true;
                index += 1;
            }
            _ => {
                dispatch_args.push(args[index].clone());
                index += 1;
            }
        }
    }
    Ok((dispatch_args, guard))
}

fn preflight_docx_document_guard(args: &[String], guard: &DocxGuardArgs) -> CliResult<()> {
    if !docx_is_mutation(args) {
        if guard.require_guard || guard.expected_document_hash.is_some() {
            return Err(CliError::invalid_args(
                "DOCX guard flags are accepted only by mutation commands",
            ));
        }
        return Ok(());
    }

    let has_block_hash = docx_is_block_addressed_mutation(args)
        && value_after(args, "--expect-hash").is_some_and(|value| !value.is_empty());
    if guard.require_guard && !has_block_hash && guard.expected_document_hash.is_none() {
        return Err(CliError::invalid_args(
            "--require-guard requires --expect-hash or --expect-document-hash",
        ));
    }
    let Some(expected) = guard.expected_document_hash.as_deref() else {
        return Ok(());
    };
    let source = docx_source_path(args).ok_or_else(|| {
        CliError::invalid_args("could not resolve DOCX source for --expect-document-hash")
    })?;
    let actual = docx_document_hash(source)?;
    if expected != actual {
        return Err(CliError::invalid_args(format!(
            "document hash mismatch: expected {expected} but found {actual}"
        )));
    }
    Ok(())
}

fn enrich_docx_result(args: &[String], guard: &DocxGuardArgs, result: &mut Value) -> CliResult<()> {
    let Some(object) = result.as_object_mut() else {
        return Ok(());
    };
    if docx_is_block_addressed_mutation(args)
        && guard.expected_document_hash.is_none()
        && value_after(args, "--expect-hash").is_none_or(str::is_empty)
    {
        push_docx_guard_warning(object);
    }

    let Some(path) = docx_result_path(args, object) else {
        return Ok(());
    };
    let entries = zip_entry_names(&path)?;
    let document_part = find_docx_document_part(&path, &entries)?;
    let bytes = crate::zip_bytes(&path, &document_part)?;
    let xml = String::from_utf8(bytes)
        .map_err(|_| CliError::unexpected("DOCX document part is not UTF-8 XML"))?;
    let (document_hash, block_hashes) = docx_hash_readback(&xml)?;
    object.insert("documentHash".to_string(), json!(document_hash));
    object.insert("blockHashes".to_string(), block_hashes);
    Ok(())
}

fn push_docx_guard_warning(object: &mut Map<String, Value>) {
    let warning = json!({
        "code": "DOCX_GUARD_NOT_PROVIDED",
        "message": "mutation proceeded without a block or document hash guard",
    });
    match object.get_mut("warnings") {
        Some(Value::Array(warnings)) => warnings.push(warning),
        _ => {
            object.insert("warnings".to_string(), json!([warning]));
        }
    }
}

fn docx_result_path(args: &[String], result: &Map<String, Value>) -> Option<String> {
    if !has_flag(args, "--dry-run")
        && let Some(output) = value_after(args, "--out")
        && Path::new(output).is_file()
    {
        return Some(output.to_string());
    }
    if let Some(output) = result.get("output").and_then(Value::as_str)
        && Path::new(output).is_file()
    {
        return Some(output.to_string());
    }
    if let Some(file) = result.get("file").and_then(Value::as_str)
        && Path::new(file).is_file()
    {
        return Some(file.to_string());
    }
    docx_source_path(args)
        .filter(|path| Path::new(path).is_file())
        .map(ToString::to_string)
}

fn docx_document_hash(file: &str) -> CliResult<String> {
    let entries = zip_entry_names(file)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let bytes = crate::zip_bytes(file, &document_part)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn require_docx_document_hash(value: &str) -> CliResult<()> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(CliError::invalid_args(
            "--expect-document-hash must match sha256:<64 lowercase hex chars> from a DOCX readback",
        ));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        return Err(CliError::invalid_args(
            "--expect-document-hash must match sha256:<64 lowercase hex chars> from a DOCX readback",
        ));
    }
    Ok(())
}

fn docx_source_path(args: &[String]) -> Option<&str> {
    match args {
        [family, command, file, ..]
            if family == "docx" && matches!(command.as_str(), "text" | "replace") =>
        {
            Some(file)
        }
        [family, command, file, ..] if family == "docx" && command == "scaffold" => Some(file),
        [family, _group, _verb, file, ..] if family == "docx" => Some(file),
        _ => None,
    }
}

fn docx_is_mutation(args: &[String]) -> bool {
    match args {
        [family, command, ..] if family == "docx" && command == "scaffold" => false,
        [family, command, ..] if family == "docx" && command == "replace" => true,
        [family, group, verb, ..] if family == "docx" => match group.as_str() {
            "blocks" => matches!(verb.as_str(), "replace" | "delete" | "insert-after"),
            "paragraphs" => matches!(verb.as_str(), "append" | "insert" | "set" | "clear"),
            "breaks" => verb == "insert",
            "sections" => verb == "set",
            "styles" => verb == "apply",
            "comments" => matches!(verb.as_str(), "add" | "edit" | "remove"),
            "fields" => matches!(verb.as_str(), "insert" | "set-result"),
            "headers" | "footers" => verb == "set-text",
            "images" => matches!(verb.as_str(), "replace" | "insert"),
            "tables" => matches!(
                verb.as_str(),
                "create" | "set-style" | "set-cell" | "clear-cell" | "insert-row" | "delete-row"
            ),
            _ => false,
        },
        _ => false,
    }
}

fn docx_is_block_addressed_mutation(args: &[String]) -> bool {
    match args {
        [family, group, verb, ..] if family == "docx" => match group.as_str() {
            "blocks" => matches!(verb.as_str(), "replace" | "delete" | "insert-after"),
            "paragraphs" => matches!(verb.as_str(), "insert" | "set" | "clear"),
            "styles" => verb == "apply",
            "fields" => matches!(verb.as_str(), "insert" | "set-result"),
            "images" => verb == "insert",
            "tables" => matches!(
                verb.as_str(),
                "set-style" | "set-cell" | "clear-cell" | "insert-row" | "delete-row"
            ),
            _ => false,
        },
        _ => false,
    }
}

fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
}

fn require_docx_image_hash_format(value: &str) -> CliResult<()> {
    if value.trim().is_empty() {
        return Ok(());
    }
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(CliError::invalid_args(
            "--expect-hash must match sha256:<64 lowercase hex chars> from docx blocks",
        ));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        return Err(CliError::invalid_args(
            "--expect-hash must match sha256:<64 lowercase hex chars> from docx blocks",
        ));
    }
    Ok(())
}
