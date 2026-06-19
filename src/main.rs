use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;
use quick_xml::{NsReader, Reader};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

mod capabilities;
mod cli_args;
mod cli_core;
mod command_text;
mod docx_block_readers;
mod docx_fields;
mod docx_images;
mod inspect;
mod json_util;
mod mcp;
mod mcp_support;
mod opc;
mod package_discovery;
mod runtime_util;
mod validation;
mod xlsx_metadata;
mod xlsx_model;
mod xlsx_mutation;
mod xlsx_names;
mod xlsx_ranges;
mod xlsx_sheets;
mod xlsx_tables;
mod xml_util;
mod zip_io;

pub(crate) use cli_args::{
    flag_present, has_flag, parse_bool_flag, parse_i64_flag, parse_string_flag, parse_u32_flag,
    parse_u32_flags, parse_validate_args, reject_unknown_flags, validate_positive_i64,
    value_flag_present,
};
pub(crate) use cli_core::{
    CliError, CliResult, EXIT_FILE_NOT_FOUND, EXIT_INVALID_ARGS, EXIT_PARTIAL_SUCCESS,
    EXIT_SUCCESS, EXIT_TARGET_NOT_FOUND, EXIT_UNEXPECTED, EXIT_UNSUPPORTED_TYPE,
    EXIT_VALIDATION_FAILED, GlobalFlags,
};
pub(crate) use command_text::command_arg;
pub(crate) use docx_block_readers::{
    DocxRichBlockReport, docx_blocks, docx_para_id_ns, docx_rich_block_json,
    docx_rich_block_reports, element_in_ns, stack_contains,
};
pub(crate) use docx_fields::{docx_fields_insert, docx_fields_list, docx_fields_set_result};
pub(crate) use docx_images::docx_images_list;
pub(crate) use inspect::inspect;
pub(crate) use json_util::{
    json_bool, json_field, json_i64, json_optional_serialized, json_optional_string, json_string,
    json_u32,
};
pub(crate) use mcp::run_mcp_stdio;
pub(crate) use mcp_support::{
    mcp_capabilities_resource, mcp_command_resource_for_uri, mcp_command_resource_template,
    mcp_resources, mcp_tool_success, mcp_tools,
};
pub(crate) use opc::{
    RelationshipEntry, add_relationship_to_xml, allocate_relationship_id, content_type_for_part,
    ensure_content_type_override, ensure_package_root_relationship_xml, relationship_entries,
    relationship_source_uri, relationship_target_from_source_to_target, relationships,
    relationships_part_for, resolve_relationship_target,
};
pub(crate) use package_discovery::{
    InspectPackageKind, detect_inspect_package_type, find_docx_document_part,
    find_xlsx_workbook_part, is_custom_xml_part, is_docx_comments_part, is_docx_endnotes_part,
    is_docx_footer_part, is_docx_footnotes_part, is_docx_header_part, is_docx_media_part,
    is_docx_numbering_part, is_docx_styles_part, is_xlsx_chart_part, is_xlsx_media_part,
    is_xlsx_pivot_cache_part, is_xlsx_pivot_table_part, is_xlsx_shared_strings_part,
    is_xlsx_styles_part, is_xlsx_table_part, is_xlsx_theme_part, is_xlsx_worksheet_part,
};
pub(crate) use runtime_util::{
    chrono_like_counter, current_utc_rfc3339, docx_mutation_temp_path, xlsx_ranges_set_temp_path,
};
pub(crate) use validation::{validate, validate_exit_code};
pub(crate) use xlsx_metadata::{
    XlsxWorkbookMetadataUpdateOptions, xlsx_workbook_metadata_inspect,
    xlsx_workbook_metadata_update,
};
pub(crate) use xlsx_model::{
    CellValue, RangeBounds, WorkbookSheet, XlsxCellEntry, build_dense_xlsx_rows,
    build_sparse_xlsx_rows, builtin_num_format_code, col_name, is_xlsx_handle,
    normalize_xlsx_cell_ref, parse_cell_ref, parse_cli_range, parse_range, parse_xlsx_cell_handle,
    resolve_sheet, resolve_sheet_by_sheet_id_unique, shared_strings, sheet_cells,
    sorted_xlsx_cells, used_range_for_cells, used_range_json, used_range_ref, workbook_sheets,
    xlsx_dimension_declared, xlsx_merged_cell_count, xlsx_styles,
};
pub(crate) use xlsx_mutation::{
    XlsxCellsSetOptions, XlsxRangesSetFormatOptions, XlsxRangesSetOptions,
    validate_xlsx_mutation_output_flags, xlsx_cells_set, xlsx_ranges_set, xlsx_ranges_set_format,
};
pub(crate) use xlsx_names::{xlsx_names_list, xlsx_names_show};
pub(crate) use xlsx_ranges::{
    XlsxRangeExportOptions, check_range_max_cells, normalize_xlsx_ranges_set_data_format,
    require_json_data_format, xlsx_range_export, xlsx_range_export_with_options,
};
pub(crate) use xlsx_sheets::{xlsx_cells_extract, xlsx_sheets_list, xlsx_sheets_show};
pub(crate) use xlsx_tables::{
    XlsxTableExportOptions, xlsx_source_command, xlsx_tables_export, xlsx_tables_list,
    xlsx_tables_show,
};
pub(crate) use xml_util::{
    attr, attr_bound_ns, attr_exact, attr_prefixed_ns, decode_local_xml_attrs as xml_attrs,
    decode_xml_attrs as xml_attrs_map, decode_xml_text, local_name, needs_xml_space_preserve,
    remove_xml_span, render_xml_attrs, replace_xml_span, xml_attr_escape, xml_escape,
    xml_general_ref,
};
pub(crate) use zip_io::{
    copy_zip_with_part_override, copy_zip_with_part_overrides,
    copy_zip_with_part_overrides_and_removals, copy_zip_with_replacement, zip_entry_names,
    zip_entry_set, zip_text,
};

const DOCX_W_NS: &[u8] = b"http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const DOCX_W14_NS: &[u8] = b"http://schemas.microsoft.com/office/word/2010/wordml";

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("serve") {
        std::process::exit(run_serve_stdio());
    }
    if argv.first().map(String::as_str) == Some("mcp") {
        std::process::exit(run_mcp_stdio());
    }
    match run(&argv) {
        Ok(output) => {
            println!(
                "{}",
                serde_json::to_string(&output.value).expect("serialize output")
            );
            std::process::exit(output.exit_code);
        }
        Err(err) => {
            let body = json!({
                "error": {
                    "code": err.code,
                    "exitCode": err.exit_code,
                    "message": err.message,
                }
            });
            eprintln!("{}", serde_json::to_string(&body).expect("serialize error"));
            std::process::exit(err.exit_code);
        }
    }
}

struct RunOutput {
    value: Value,
    exit_code: i32,
}

fn run_serve_stdio() -> i32 {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut state = ServeState::default();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "serve read error: {err}");
                return EXIT_UNEXPECTED;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "serve JSON parse error: {err}");
                return EXIT_INVALID_ARGS;
            }
        };
        let response = state.handle_rpc(request);
        if writeln!(
            stdout,
            "{}",
            serde_json::to_string(&response).expect("serialize rpc response")
        )
        .is_err()
        {
            return EXIT_UNEXPECTED;
        }
        if stdout.flush().is_err() {
            return EXIT_UNEXPECTED;
        }
    }
    EXIT_SUCCESS
}

fn run(raw_args: &[String]) -> CliResult<RunOutput> {
    let (flags, args) = parse_global_flags(raw_args)?;
    if !flags.json && !has_local_json_format(&args) && !is_validate_command(&args) {
        return Err(CliError::invalid_args(
            "the Rust port currently supports the frozen --json contract slice only",
        ));
    }
    if let [cmd, rest @ ..] = args.as_slice()
        && cmd == "validate"
    {
        let (file, strict) = parse_validate_args(rest, flags.strict)?;
        let value = validate(file, strict)?;
        let exit_code = validate_exit_code(&value, strict);
        return Ok(RunOutput { value, exit_code });
    }
    dispatch(&flags, &args).map(|value| RunOutput {
        value,
        exit_code: EXIT_SUCCESS,
    })
}

fn parse_global_flags(raw_args: &[String]) -> CliResult<(GlobalFlags, Vec<String>)> {
    let mut flags = GlobalFlags::default();
    let mut args = Vec::new();
    let mut i = 0;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--json" => {
                flags.json = true;
                i += 1;
            }
            "--format" | "-f" => {
                let Some(value) = raw_args.get(i + 1) else {
                    return Err(CliError::invalid_args("--format requires a value"));
                };
                if value != "json" {
                    return Err(CliError::invalid_args(format!(
                        "invalid format: {value} (expected 'text' or 'json')"
                    )));
                }
                flags.json = true;
                i += 2;
            }
            "--strict" => {
                flags.strict = true;
                i += 1;
            }
            _ => {
                args.extend_from_slice(&raw_args[i..]);
                break;
            }
        }
    }
    Ok((flags, args))
}

fn dispatch(flags: &GlobalFlags, args: &[String]) -> CliResult<Value> {
    match args {
        [cmd] if cmd == "version" => Ok(json!({"tool": "ooxml", "version": "0.0.1"})),
        [cmd, rest @ ..] if cmd == "capabilities" => capabilities::capabilities(rest),
        [cmd, file] if cmd == "inspect" => inspect(file),
        [cmd, rest @ ..] if cmd == "validate" => {
            let (file, strict) = parse_validate_args(rest, flags.strict)?;
            validate(file, strict)
        }
        [cmd, file, rest @ ..] if cmd == "verify" => verify(file, rest),
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
        [family, verb, file, rest @ ..] if family == "pptx" && verb == "render" => {
            pptx_render(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "show" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?.unwrap_or(1);
            pptx_slide_show(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "selectors" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("--slide is required"))?;
            pptx_slide_selectors(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "shapes" && verb == "show" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("required flag(s) \"slide\" not set"))?;
            let include_text = has_flag(rest, "--include-text");
            let include_bounds = has_flag(rest, "--include-bounds");
            pptx_shapes_show(file, slide, include_text, include_bounds)
        }
        [family, group, verb, file] if family == "pptx" && group == "slides" && verb == "list" => {
            pptx_slides_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "export" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--data-format",
                    "--data-out",
                    "--max-cells",
                ],
                &["--include-types", "--include-formulas", "--include-formats"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?.unwrap_or_else(|| "1".to_string());
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            require_json_data_format(data_format.as_deref())?;
            let data_out = parse_string_flag(rest, "--data-out")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let include_types = has_flag(rest, "--include-types");
            let include_formulas = has_flag(rest, "--include-formulas");
            let include_formats = has_flag(rest, "--include-formats");
            xlsx_range_export_with_options(
                file,
                &sheet,
                &range,
                XlsxRangeExportOptions {
                    include_types,
                    include_formulas,
                    include_formats,
                    data_out: data_out.as_deref(),
                    max_cells,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--anchor",
                    "--values",
                    "--values-file",
                    "--data-format",
                    "--null-policy",
                    "--ragged",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &[
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                    "--overwrite-formulas",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?
                .ok_or_else(|| CliError::invalid_args("--sheet is required for range commands"))?;
            let range = parse_string_flag(rest, "--range")?;
            let anchor = parse_string_flag(rest, "--anchor")?;
            let values = parse_string_flag(rest, "--values")?;
            let values_file = parse_string_flag(rest, "--values-file")?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            let null_policy = parse_string_flag(rest, "--null-policy")?;
            let ragged = parse_string_flag(rest, "--ragged")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let no_validate = has_flag(rest, "--no-validate");
            let in_place = has_flag(rest, "--in-place");
            let overwrite_formulas = has_flag(rest, "--overwrite-formulas");
            xlsx_ranges_set(
                file,
                XlsxRangesSetOptions {
                    sheet: &sheet,
                    range: range.as_deref(),
                    anchor: anchor.as_deref(),
                    values: values.as_deref(),
                    values_file: values_file.as_deref(),
                    data_format: data_format.as_deref(),
                    null_policy: null_policy.as_deref(),
                    ragged: ragged.as_deref(),
                    max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    no_validate,
                    in_place,
                    overwrite_formulas,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "set-format" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--preset",
                    "--format-code",
                    "--decimals",
                    "--currency-symbol",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?
                .ok_or_else(|| CliError::invalid_args("--sheet is required for range commands"))?;
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            let preset = parse_string_flag(rest, "--preset")?;
            let format_code = parse_string_flag(rest, "--format-code")?;
            let decimals = parse_i64_flag(rest, "--decimals")?.unwrap_or(2);
            let currency_symbol = parse_string_flag(rest, "--currency-symbol")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let no_validate = has_flag(rest, "--no-validate");
            let in_place = has_flag(rest, "--in-place");
            xlsx_ranges_set_format(
                file,
                XlsxRangesSetFormatOptions {
                    sheet: &sheet,
                    range: &range,
                    preset: preset.as_deref(),
                    format_code: format_code.as_deref(),
                    decimals,
                    currency_symbol: currency_symbol.as_deref(),
                    max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    no_validate,
                    in_place,
                },
            )
        }
        [family, group, subgroup, verb, file]
            if family == "xlsx"
                && group == "workbook"
                && subgroup == "metadata"
                && verb == "inspect" =>
        {
            xlsx_workbook_metadata_inspect(file)
        }
        [family, group, subgroup, verb, file, rest @ ..]
            if family == "xlsx"
                && group == "workbook"
                && subgroup == "metadata"
                && verb == "update" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--title",
                    "--subject",
                    "--creator",
                    "--keywords",
                    "--description",
                    "--last-modified-by",
                    "--category",
                    "--company",
                    "--manager",
                    "--calc-mode",
                    "--expect-title",
                    "--expect-subject",
                    "--expect-creator",
                    "--expect-keywords",
                    "--expect-description",
                    "--expect-last-modified-by",
                    "--expect-category",
                    "--expect-company",
                    "--expect-manager",
                    "--out",
                    "--backup",
                ],
                &[
                    "--full-calc-on-load",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let title = parse_string_flag(rest, "--title")?;
            let subject = parse_string_flag(rest, "--subject")?;
            let creator = parse_string_flag(rest, "--creator")?;
            let keywords = parse_string_flag(rest, "--keywords")?;
            let description = parse_string_flag(rest, "--description")?;
            let last_modified_by = parse_string_flag(rest, "--last-modified-by")?;
            let category = parse_string_flag(rest, "--category")?;
            let company = parse_string_flag(rest, "--company")?;
            let manager = parse_string_flag(rest, "--manager")?;
            let calc_mode = parse_string_flag(rest, "--calc-mode")?;
            let expect_title = parse_string_flag(rest, "--expect-title")?;
            let expect_subject = parse_string_flag(rest, "--expect-subject")?;
            let expect_creator = parse_string_flag(rest, "--expect-creator")?;
            let expect_keywords = parse_string_flag(rest, "--expect-keywords")?;
            let expect_description = parse_string_flag(rest, "--expect-description")?;
            let expect_last_modified_by = parse_string_flag(rest, "--expect-last-modified-by")?;
            let expect_category = parse_string_flag(rest, "--expect-category")?;
            let expect_company = parse_string_flag(rest, "--expect-company")?;
            let expect_manager = parse_string_flag(rest, "--expect-manager")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let full_calc_on_load = parse_bool_flag(rest, "--full-calc-on-load")?;
            xlsx_workbook_metadata_update(
                file,
                XlsxWorkbookMetadataUpdateOptions {
                    title: title.as_deref(),
                    subject: subject.as_deref(),
                    creator: creator.as_deref(),
                    keywords: keywords.as_deref(),
                    description: description.as_deref(),
                    last_modified_by: last_modified_by.as_deref(),
                    category: category.as_deref(),
                    company: company.as_deref(),
                    manager: manager.as_deref(),
                    calc_mode: calc_mode.as_deref(),
                    full_calc_on_load,
                    expect_title: expect_title.as_deref(),
                    expect_subject: expect_subject.as_deref(),
                    expect_creator: expect_creator.as_deref(),
                    expect_keywords: expect_keywords.as_deref(),
                    expect_description: expect_description.as_deref(),
                    expect_last_modified_by: expect_last_modified_by.as_deref(),
                    expect_category: expect_category.as_deref(),
                    expect_company: expect_company.as_deref(),
                    expect_manager: expect_manager.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "cells" && verb == "extract" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?.unwrap_or_else(|| "1".to_string());
            let range = parse_string_flag(rest, "--range")?;
            let max_rows = parse_u32_flag(rest, "--max-rows")?.unwrap_or(1000);
            let max_cells = parse_u32_flag(rest, "--max-cells")?.unwrap_or(0);
            let include_empty = has_flag(rest, "--include-empty");
            xlsx_cells_extract(
                file,
                &sheet,
                range.as_deref(),
                max_rows,
                max_cells,
                include_empty,
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "cells" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--cell",
                    "--ref",
                    "--value",
                    "--formula",
                    "--type",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            let ref_ = parse_string_flag(rest, "--ref")?;
            let value = parse_string_flag(rest, "--value")?;
            let formula = parse_string_flag(rest, "--formula")?;
            let value_type = parse_string_flag(rest, "--type")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let no_validate = has_flag(rest, "--no-validate");
            let in_place = has_flag(rest, "--in-place");
            xlsx_cells_set(
                file,
                XlsxCellsSetOptions {
                    sheet: sheet.as_deref(),
                    cell: cell.as_deref(),
                    ref_: ref_.as_deref(),
                    value: value.as_deref(),
                    formula: formula.as_deref(),
                    value_type: value_type.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    no_validate,
                    in_place,
                },
            )
        }
        [family, group, verb, file] if family == "xlsx" && group == "sheets" && verb == "list" => {
            xlsx_sheets_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "sheets" && verb == "show" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_sheets_show(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--scope-sheet"], &[])?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            xlsx_names_list(file, scope_sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--name", "--scope-sheet"], &[])?;
            let name = parse_string_flag(rest, "--name")?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            xlsx_names_show(file, name.as_deref().unwrap_or(""), scope_sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "tables" && verb == "list" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_tables_list(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "tables" && verb == "show" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?;
            let table = parse_string_flag(rest, "--table")?;
            xlsx_tables_show(file, sheet.as_deref(), table.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "tables" && verb == "export" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--table",
                    "--data-format",
                    "--data-out",
                    "--max-cells",
                ],
                &["--include-types", "--include-formulas"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let table = parse_string_flag(rest, "--table")?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            let data_out = parse_string_flag(rest, "--data-out")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let include_types = has_flag(rest, "--include-types");
            let include_formulas = has_flag(rest, "--include-formulas");
            xlsx_tables_export(
                file,
                sheet.as_deref(),
                table.as_deref(),
                XlsxTableExportOptions {
                    data_format: data_format.as_deref(),
                    data_out: data_out.as_deref(),
                    max_cells,
                    include_types,
                    include_formulas,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "extract" && verb == "text" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            pptx_extract_text(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "extract" && verb == "notes" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            pptx_extract_notes(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "notes" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("--slide is required"))?;
            pptx_notes_show(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "comments" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--slide", "--comment-id"], &[])?;
            let slide = parse_i64_flag(rest, "--slide")?;
            let comment_id = parse_i64_flag(rest, "--comment-id")?;
            if let Some(slide) = slide
                && slide < 1
            {
                return Err(CliError::invalid_args("--slide must be >= 1"));
            }
            if comment_id.is_some() && slide.is_none() {
                return Err(CliError::invalid_args("--comment-id requires --slide"));
            }
            pptx_comments_list(file, slide.map(|value| value as u32), comment_id)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "masters" && verb == "list" =>
        {
            reject_unknown_flags(rest, &[], &[])?;
            pptx_masters_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "masters" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--master"], &[])?;
            let master = parse_i64_flag(rest, "--master")?.unwrap_or(1);
            pptx_masters_show(file, master)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--master"], &[])?;
            let master = parse_i64_flag(rest, "--master")?;
            if let Some(master) = master
                && master < 0
            {
                return Err(CliError::invalid_args("--master must be >= 0"));
            }
            pptx_layouts_list(file, master.map(|value| value as u32))
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--layout"], &[])?;
            let layout = parse_string_flag(rest, "--layout")?
                .ok_or_else(|| CliError::invalid_args("--layout flag is required"))?;
            pptx_layouts_show(file, &layout)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide", "--table-id", "--target"], &["--details"])?;
            let slide = parse_i64_flag(rest, "--slide")?.unwrap_or(0);
            if slide < 1 {
                return Err(CliError::invalid_args("--slide must be >= 1"));
            }
            let table_id = parse_i64_flag(rest, "--table-id")?.unwrap_or(0);
            if table_id < 0 {
                return Err(CliError::invalid_args(
                    "--table-id must be a positive integer",
                ));
            }
            let target = parse_string_flag(rest, "--target")?;
            if table_id > 0 && target.as_deref().unwrap_or_default() != "" {
                return Err(CliError::invalid_args(
                    "specify only one of --target or --table-id",
                ));
            }
            pptx_tables_show(
                file,
                slide as u32,
                table_id as u32,
                target.as_deref(),
                has_flag(rest, "--details"),
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text" =>
        {
            pptx_replace_text(file, rest)
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}

fn has_local_json_format(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "--format" && pair[1] == "json")
}

fn is_validate_command(args: &[String]) -> bool {
    matches!(args, [cmd, ..] if cmd == "validate")
}

fn pptx_slide_show(file: &str, slide: u32) -> CliResult<Value> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    if slide == 0 || slide as usize > slides.len() {
        return Err(CliError::invalid_args(format!(
            "slide number {slide} is out of range (1-{})",
            slides.len()
        )));
    }

    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let (slide_id, rel_id) = &slides[slide as usize - 1];
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    let part = normalize_ppt_target(target);
    let slide_xml = zip_text(file, &part)?;
    let layout_part = slide_layout_part(file, &part)?;
    let layout_name = layout_part
        .as_ref()
        .and_then(|part| zip_text(file, part).ok())
        .and_then(|xml| layout_display_name(&xml))
        .unwrap_or_else(|| "Title Slide".to_string());
    let layout_number = layout_part
        .as_ref()
        .and_then(|part| trailing_number(part, "slideLayout"))
        .unwrap_or(1);
    let shapes = pptx_shapes(&slide_xml);
    let part_uri = format!("/{}", part);
    let layout_part_uri = layout_part
        .as_ref()
        .map(|part| format!("/{part}"))
        .unwrap_or_else(|| "/ppt/slideLayouts/slideLayout1.xml".to_string());

    Ok(json!({
        "file": file,
        "slides": [{
            "id": format!("slide{slide}"),
            "layoutNumber": layout_number,
            "layoutPartUri": layout_part_uri,
            "layoutReadbackCommand": format!("ooxml --json pptx layouts show {file} --layout {layout_number}"),
            "layoutRef": layout_name,
            "partUri": part_uri,
            "primarySelector": slide.to_string(),
            "readbackCommand": format!("ooxml --json pptx slides show {file} --slide {slide} --include-text --include-bounds"),
            "relationshipId": rel_id,
            "selectors": [
                slide.to_string(),
                format!("part:/{}", part),
                format!("slideId:{slide_id}"),
                format!("rId:{rel_id}"),
            ],
            "selectorsCommand": format!("ooxml --json pptx slides selectors {file} --slide {slide}"),
            "shapes": shapes,
            "shapesCommand": format!("ooxml --json pptx shapes show {file} --slide {slide} --include-text --include-bounds"),
            "slide": slide,
            "slideId": slide_id,
        }],
    }))
}

fn pptx_slides_list(file: &str) -> CliResult<Value> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let mut slide_id_counts = BTreeMap::<u32, usize>::new();
    for (slide_id, _) in &slides {
        if *slide_id != 0 {
            *slide_id_counts.entry(*slide_id).or_default() += 1;
        }
    }
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let values = slides
        .iter()
        .enumerate()
        .map(|(index, (slide_id, rel_id))| {
            let slide_number = index as u32 + 1;
            let target = rels
                .get(rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            let part = normalize_ppt_target(target);
            let slide_xml = zip_text(file, &part)?;
            let (layout_part, notes_part) = slide_layout_and_notes_parts(file, &part)?;
            let layout_xml = layout_part.as_ref().and_then(|part| zip_text(file, part).ok());
            let layout_name = layout_xml
                .as_deref()
                .and_then(layout_display_name)
                .unwrap_or_default();
            let layout_number = layout_xml
                .as_ref()
                .and(layout_part.as_ref())
                .and_then(|part| trailing_number(part, "slideLayout"))
                .unwrap_or(0);
            let (text_shapes, images, tables) = pptx_slide_object_counts(&slide_xml);
            let part_uri = format!("/{part}");
            let layout_part_uri = layout_xml
                .as_ref()
                .and(layout_part.as_ref())
                .map(|part| format!("/{part}"));
            let notes_part_uri = notes_part.as_ref().map(|part| format!("/{part}"));
            let selectors = vec![
                slide_number.to_string(),
                format!("part:{part_uri}"),
                format!("slideId:{slide_id}"),
                format!("rId:{rel_id}"),
            ];
            let mut item = Map::new();
            item.insert("number".to_string(), json!(slide_number));
            item.insert("slideId".to_string(), json!(slide_id));
            item.insert("relationshipId".to_string(), json!(rel_id));
            item.insert("partUri".to_string(), json!(part_uri));
            item.insert("primarySelector".to_string(), json!(slide_number.to_string()));
            if *slide_id != 0 && slide_id_counts.get(slide_id).copied().unwrap_or_default() == 1 {
                item.insert("handle".to_string(), json!(format!("H:pptx/s:{slide_id}")));
            }
            item.insert("selectors".to_string(), json!(selectors));
            item.insert("layout".to_string(), json!(layout_name));
            if layout_number > 0 {
                item.insert("layoutNumber".to_string(), json!(layout_number));
            }
            if let Some(layout_part_uri) = layout_part_uri {
                item.insert("layoutPartUri".to_string(), json!(layout_part_uri));
            }
            if let Some(notes_part_uri) = notes_part_uri {
                item.insert("notesPartUri".to_string(), json!(notes_part_uri));
            }
            item.insert("textShapes".to_string(), json!(text_shapes));
            item.insert("images".to_string(), json!(images));
            item.insert("tables".to_string(), json!(tables));
            item.insert("notes".to_string(), json!(notes_part.is_some()));
            item.insert(
                "readbackCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx slides show {file} --slide {slide_number} --include-text --include-bounds"
                )),
            );
            item.insert(
                "selectorsCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx slides selectors {file} --slide {slide_number}"
                )),
            );
            item.insert(
                "shapesCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx shapes show {file} --slide {slide_number} --include-text --include-bounds"
                )),
            );
            if tables > 0 {
                item.insert(
                    "tablesCommand".to_string(),
                    json!(format!(
                        "ooxml --json pptx tables show {file} --slide {slide_number}"
                    )),
                );
            }
            if layout_number > 0 {
                item.insert(
                    "layoutReadbackCommand".to_string(),
                    json!(format!(
                        "ooxml --json pptx layouts show {file} --layout {layout_number}"
                    )),
                );
            }
            Ok(Value::Object(item))
        })
        .collect::<CliResult<Vec<_>>>()?;
    Ok(json!({"file": file, "slides": values}))
}

fn pptx_slide_selectors(file: &str, slide: u32) -> CliResult<Value> {
    if slide == 0 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let index = slide as usize - 1;
    let (_, rel_id) = slides
        .get(index)
        .ok_or_else(|| CliError::unexpected(format!("slide {slide} not found")))?;
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    let part = normalize_ppt_target(target);
    let slide_xml = zip_text(file, &part)?;
    let (layout_part, _) = slide_layout_and_notes_parts(file, &part)?;
    let layout_xml = layout_part
        .as_ref()
        .and_then(|part| zip_text(file, part).ok());
    let layout_name = layout_xml.as_deref().and_then(layout_display_name);
    let layout_part_uri = layout_xml
        .as_ref()
        .and(layout_part.as_ref())
        .map(|part| format!("/{part}"));

    let mut output = Map::new();
    output.insert("file".to_string(), json!(file));
    output.insert("slide".to_string(), json!(slide));
    output.insert("partUri".to_string(), json!(format!("/{part}")));
    if let Some(layout_name) = layout_name.filter(|name| !name.is_empty()) {
        output.insert("layoutName".to_string(), json!(layout_name));
    }
    if let Some(layout_part_uri) = layout_part_uri {
        output.insert("layoutPartUri".to_string(), json!(layout_part_uri));
    }
    output.insert(
        "targets".to_string(),
        Value::Array(pptx_selector_targets(&slide_xml)),
    );
    Ok(Value::Object(output))
}

fn pptx_shapes_show(
    file: &str,
    slide: u32,
    include_text: bool,
    include_bounds: bool,
) -> CliResult<Value> {
    if slide == 0 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let mut slide_id_counts = BTreeMap::<u32, usize>::new();
    for (slide_id, _) in &slides {
        if *slide_id != 0 {
            *slide_id_counts.entry(*slide_id).or_default() += 1;
        }
    }
    let index = slide as usize - 1;
    let (slide_id, rel_id) = slides.get(index).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide {slide} not found (presentation has {} slides)",
            slides.len()
        ))
    })?;
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    let part = normalize_ppt_target(target);
    let slide_xml = zip_text(file, &part)?;
    let (layout_part, _) = slide_layout_and_notes_parts(file, &part)?;
    let layout_xml = layout_part
        .as_ref()
        .and_then(|part| zip_text(file, part).ok());
    let layout_name = layout_xml.as_deref().and_then(layout_display_name);
    let layout_part_uri = layout_xml
        .as_ref()
        .and(layout_part.as_ref())
        .map(|part| format!("/{part}"));
    let slide_id_unique =
        *slide_id != 0 && slide_id_counts.get(slide_id).copied().unwrap_or_default() == 1;

    let mut output = Map::new();
    output.insert("file".to_string(), json!(file));
    output.insert("slide".to_string(), json!(slide));
    output.insert("partUri".to_string(), json!(format!("/{part}")));
    if let Some(layout_name) = layout_name.filter(|name| !name.is_empty()) {
        output.insert("layoutName".to_string(), json!(layout_name));
    }
    if let Some(layout_part_uri) = layout_part_uri {
        output.insert("layoutPartUri".to_string(), json!(layout_part_uri));
    }
    output.insert(
        "shapes".to_string(),
        Value::Array(pptx_shape_show_entries(
            file,
            &part,
            &slide_xml,
            *slide_id,
            slide_id_unique,
            include_text,
            include_bounds,
        )),
    );
    Ok(Value::Object(output))
}

fn selector_candidates(
    items: &[(&str, &[String])],
    selector: &str,
    max_count: usize,
) -> Vec<String> {
    let needle = selector.trim().to_ascii_lowercase();
    let mut seen = Vec::<String>::new();
    if !needle.is_empty() {
        for (primary, selectors) in items {
            let matched = primary.to_ascii_lowercase().contains(&needle)
                || selectors
                    .iter()
                    .any(|selector| selector.to_ascii_lowercase().contains(&needle));
            if matched && push_selector_candidate(&mut seen, primary, max_count) {
                return seen;
            }
        }
    }
    if !seen.is_empty() {
        return seen;
    }
    for (primary, _) in items {
        if push_selector_candidate(&mut seen, primary, max_count) {
            break;
        }
    }
    seen
}

fn push_selector_candidate(seen: &mut Vec<String>, primary: &str, max_count: usize) -> bool {
    let primary = primary.trim();
    if primary.is_empty() || seen.iter().any(|existing| existing == primary) {
        return false;
    }
    seen.push(primary.to_string());
    seen.len() >= max_count
}

fn docx_text(file: &str) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "docx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }
    let xml = zip_text(file, "word/document.xml")?;
    let blocks = docx_blocks(&xml);
    Ok(json!({"blocks": blocks, "file": file}))
}

fn docx_blocks_show(file: &str, block: usize, include_runs: bool) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let xml = zip_text(file, &document_part)?;
    let reports = docx_rich_block_reports(&xml, include_runs).map_err(|err| {
        if err.message == "invalid DOCX XML"
            || err.message.starts_with("failed to extract DOCX blocks:")
        {
            CliError::unexpected(format!(
                "failed to extract DOCX blocks: failed to read document part {document_uri}: failed to parse XML part {document_uri}: etree: invalid XML format"
            ))
        } else {
            CliError::unexpected(format!("failed to extract DOCX blocks: {}", err.message))
        }
    })?;
    let blocks: Vec<Value> = if block > 0 {
        reports
            .into_iter()
            .filter(|report| report.index == block)
            .map(docx_rich_block_json)
            .collect()
    } else {
        reports.into_iter().map(docx_rich_block_json).collect()
    };
    if block > 0 && blocks.is_empty() {
        return Err(CliError::target_not_found(format!(
            "target not found: block {block}"
        )));
    }
    Ok(json!({
        "file": file,
        "documentPartUri": document_uri,
        "blocks": blocks,
    }))
}

fn docx_blocks_insert_after(
    file: &str,
    block: usize,
    expected_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    ensure_docx_package_kind(file, &entries)?;

    let text = resolve_optional_docx_paragraph_text(options.text, options.text_file)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let anchor_hash = if block > 0 {
        let anchor = reports
            .get(block - 1)
            .ok_or_else(|| CliError::target_not_found("target not found: block"))?;
        if anchor.content_hash != expected_hash {
            return Err(CliError::invalid_args(format!(
                "block hash mismatch: block {block} expected {expected_hash} but found {}",
                anchor.content_hash
            )));
        }
        anchor.content_hash.clone()
    } else {
        String::new()
    };

    let style = options.style;
    let (updated_xml, index) = insert_docx_body_paragraph_xml(&xml, block, &text, style)?;
    write_docx_mutation_output(file, &document_part, &updated_xml, options)?;
    let updated_reports = docx_rich_block_reports(&updated_xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let inserted = updated_reports
        .get(index - 1)
        .ok_or_else(|| CliError::unexpected("inserted block readback missing"))?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(index));
    result.insert("blockId".to_string(), json!(format!("body.b{index}")));
    result.insert("contentHash".to_string(), json!(inserted.content_hash));
    if !anchor_hash.is_empty() {
        result.insert("anchorHash".to_string(), json!(anchor_hash));
        result.insert("insertAfter".to_string(), json!(block));
    }
    if !style.is_empty() {
        result.insert("style".to_string(), json!(style));
    }
    result.insert("text".to_string(), json!(text));
    Ok(Value::Object(result))
}

fn docx_blocks_replace(
    file: &str,
    block: usize,
    expected_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    ensure_docx_package_kind(file, &entries)?;

    let text = resolve_optional_docx_paragraph_text(options.text, options.text_file)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let previous = reports
        .get(block - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: block"))?;
    if previous.content_hash != expected_hash {
        return Err(CliError::invalid_args(format!(
            "block hash mismatch: block {block} expected {expected_hash} but found {}",
            previous.content_hash
        )));
    }

    let style = if options.style.is_empty() && previous.kind == "paragraph" {
        previous.style.clone()
    } else {
        options.style.to_string()
    };
    let original_body_tag = docx_body_tag(&xml)?;
    let original_prefix = docx_body_prefix(&original_body_tag);
    let working = if original_prefix.is_empty() && !style.is_empty() {
        ensure_docx_word_prefix(&xml)?
    } else {
        xml
    };
    let body_tag = docx_body_tag(&working)?;
    let prefix = docx_body_prefix(&body_tag);
    let ranges = docx_body_block_ranges(&working, &body_tag)?;
    let target_range = ranges
        .get(block - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: block"))?;
    let target_fragment = &working[target_range.start..target_range.end];
    if docx_block_has_section_properties(target_fragment) {
        return Err(CliError::invalid_args(format!(
            "block contains section properties: block {block}"
        )));
    }

    let replacement = render_docx_paragraph(&prefix, &text, &style);
    let mut updated_xml = String::with_capacity(working.len() + replacement.len());
    updated_xml.push_str(&working[..target_range.start]);
    updated_xml.push_str(&replacement);
    updated_xml.push_str(&working[target_range.end..]);

    write_docx_mutation_output(file, &document_part, &updated_xml, options)?;
    let updated_report = docx_rich_block_reports(&updated_xml, true)
        .map_err(|err| {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        })?
        .into_iter()
        .nth(block - 1)
        .ok_or_else(|| CliError::unexpected("replaced block readback missing"))?;
    let content_hash = updated_report.content_hash.clone();
    let destination = docx_rich_block_json(updated_report);

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(block));
    result.insert("blockId".to_string(), json!(format!("body.b{block}")));
    result.insert("contentHash".to_string(), json!(content_hash));
    result.insert("previousKind".to_string(), json!(previous.kind));
    result.insert("previousHash".to_string(), json!(previous.content_hash));
    result.insert("previousText".to_string(), json!(previous.text));
    if !style.is_empty() {
        result.insert("style".to_string(), json!(style));
    }
    result.insert("text".to_string(), json!(text));
    result.insert("destination".to_string(), destination);
    Ok(Value::Object(result))
}

fn docx_blocks_delete(
    file: &str,
    block: usize,
    expected_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    ensure_docx_package_kind(file, &entries)?;

    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let previous = reports
        .get(block - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: block"))?;
    if reports.len() <= 1 {
        return Err(CliError::invalid_args("cannot delete the last body block"));
    }

    let body_tag = docx_body_tag(&xml)?;
    let ranges = docx_body_block_ranges(&xml, &body_tag)?;
    let target_range = ranges
        .get(block - 1)
        .ok_or_else(|| CliError::target_not_found("target not found: block"))?;
    let target_fragment = &xml[target_range.start..target_range.end];
    if docx_block_has_section_properties(target_fragment) {
        return Err(CliError::invalid_args(format!(
            "block contains section properties: block {block}"
        )));
    }
    if previous.content_hash != expected_hash {
        return Err(CliError::invalid_args(format!(
            "block hash mismatch: block {block} expected {expected_hash} but found {}",
            previous.content_hash
        )));
    }

    let mut updated_xml = String::with_capacity(xml.len());
    updated_xml.push_str(&xml[..target_range.start]);
    updated_xml.push_str(&xml[target_range.end..]);

    write_docx_mutation_output(file, &document_part, &updated_xml, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(block));
    result.insert("blockId".to_string(), json!(format!("body.b{block}")));
    result.insert("previousKind".to_string(), json!(previous.kind));
    result.insert("previousHash".to_string(), json!(previous.content_hash));
    result.insert("previousText".to_string(), json!(previous.text));
    Ok(Value::Object(result))
}

fn docx_styles_list(file: &str, style_type: Option<&str>) -> CliResult<Value> {
    let style_type = normalize_docx_style_type(style_type)?;
    let (document_part, styles_part) = docx_document_and_styles_parts(file)?;
    let mut styles = Vec::new();
    if let Some(styles_part) = styles_part.as_deref() {
        styles = docx_styles(file, styles_part)?;
        if let Some(style_type) = style_type.as_deref() {
            styles.retain(|style| style.style_type == style_type);
        }
    }
    let counts = docx_style_id_counts(&styles);
    let styles_json: Vec<Value> = styles
        .iter()
        .map(|style| docx_style_json(style, &counts))
        .collect();
    Ok(json!({
        "file": file,
        "documentPartUri": document_part,
        "stylesPartUri": styles_part,
        "count": styles_json.len(),
        "styles": styles_json,
    }))
}

fn docx_styles_show(file: &str, style_id: &str) -> CliResult<Value> {
    let (document_part, styles_part) = docx_document_and_styles_parts(file)?;
    let mut style_json = Value::Null;
    let mut found = false;
    if let Some(styles_part) = styles_part.as_deref() {
        let styles = docx_styles(file, styles_part)?;
        let counts = docx_style_id_counts(&styles);
        if let Some(style) = styles.iter().find(|style| style.style_id == style_id) {
            style_json = docx_style_json(style, &counts);
            found = true;
        }
    }
    Ok(json!({
        "file": file,
        "documentPartUri": document_part,
        "stylesPartUri": styles_part,
        "styleId": style_id,
        "found": found,
        "style": style_json,
    }))
}

struct DocxStyleApplyOptions<'a> {
    index: i64,
    handle: Option<&'a str>,
    target: DocxStyleTarget,
    style: &'a str,
    expected_hash: &'a str,
    validate_style: bool,
    mutation: DocxParagraphMutationOptions<'a>,
}

fn docx_styles_apply(file: &str, request: DocxStyleApplyOptions<'_>) -> CliResult<Value> {
    let DocxStyleApplyOptions {
        index,
        handle,
        target,
        style,
        expected_hash,
        validate_style,
        mutation: options,
    } = request;
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    ensure_docx_package_kind(file, &entries)?;

    let (document_uri, styles_part) = docx_document_and_styles_parts(file)?;
    let document_part = document_uri.trim_start_matches('/').to_string();
    let styles = if let Some(styles_part) = styles_part.as_deref() {
        docx_styles(file, styles_part)?
    } else {
        Vec::new()
    };

    let mut style_id = style.trim().to_string();
    let mut style_handle = String::new();
    if style_id.starts_with("H:") {
        style_handle = style_id.clone();
        style_id = resolve_docx_style_handle_id(&styles, styles_part.as_deref(), &style_id)?;
    }
    if validate_style {
        validate_docx_style_for_target(&styles, &style_id, target)?;
    }

    let xml = zip_text(file, &document_part)?;
    let target_index = if let Some(handle_arg) = handle.filter(|value| !value.is_empty()) {
        resolve_docx_paragraph_handle_index(&xml, handle_arg)?
    } else {
        index as usize
    };
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;

    let (result_index, block_index, block_kind, previous_style, previous_hash, para_id) =
        match target {
            DocxStyleTarget::Paragraph | DocxStyleTarget::Run => {
                let report = reports.get(target_index.saturating_sub(1)).ok_or_else(|| {
                    CliError::target_not_found(format!(
                        "target not found: {} block {target_index}",
                        target.as_str()
                    ))
                })?;
                if report.kind != "paragraph" {
                    return Err(CliError::invalid_args(format!(
                        "block {target_index} is a table, not a paragraph"
                    )));
                }
                if !expected_hash.is_empty() && expected_hash != report.content_hash {
                    return Err(CliError::invalid_args(format!(
                        "block hash mismatch: block {} expected {} but found {}",
                        report.index, expected_hash, report.content_hash
                    )));
                }
                let previous_style = if target == DocxStyleTarget::Run {
                    let body_tag = docx_body_tag(&xml)?;
                    let blocks = docx_body_block_ranges(&xml, &body_tag)?;
                    let block = blocks.get(target_index - 1).ok_or_else(|| {
                        CliError::target_not_found(format!(
                            "target not found: {} block {target_index}",
                            target.as_str()
                        ))
                    })?;
                    docx_first_run_style(&xml[block.start..block.end])?
                } else {
                    report.style.clone()
                };
                (
                    report.index,
                    report.index,
                    report.kind.to_string(),
                    previous_style,
                    report.content_hash.clone(),
                    report.para_id.clone(),
                )
            }
            DocxStyleTarget::Table => {
                let mut seen = 0usize;
                let mut selected: Option<&DocxRichBlockReport> = None;
                for report in &reports {
                    if report.kind == "table" {
                        seen += 1;
                        if seen == target_index {
                            selected = Some(report);
                            break;
                        }
                    }
                }
                let report = selected.ok_or_else(|| {
                    CliError::target_not_found(format!("target not found: table {target_index}"))
                })?;
                if !expected_hash.is_empty() && expected_hash != report.content_hash {
                    return Err(CliError::invalid_args(format!(
                        "block hash mismatch: block {} expected {} but found {}",
                        report.index, expected_hash, report.content_hash
                    )));
                }
                let body_tag = docx_body_tag(&xml)?;
                let blocks = docx_body_block_ranges(&xml, &body_tag)?;
                let block = blocks.get(report.index - 1).ok_or_else(|| {
                    CliError::target_not_found(format!("target not found: table {target_index}"))
                })?;
                (
                    target_index,
                    report.index,
                    report.kind.to_string(),
                    docx_table_style(&xml[block.start..block.end])?,
                    report.content_hash.clone(),
                    String::new(),
                )
            }
        };

    let updated_xml = apply_docx_style_xml(&xml, target, block_index, &style_id, para_id.trim())?;
    write_docx_mutation_output(file, &document_part, &updated_xml, options)?;

    let updated_reports = docx_rich_block_reports(&updated_xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let updated_report = updated_reports
        .get(block_index - 1)
        .ok_or_else(|| CliError::unexpected("styled block disappeared after mutation"))?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(result_index));
    result.insert("blockIndex".to_string(), json!(block_index));
    result.insert("blockId".to_string(), json!(format!("body.b{block_index}")));
    result.insert("blockKind".to_string(), json!(block_kind));
    result.insert("target".to_string(), json!(target.as_str()));
    if !previous_style.is_empty() {
        result.insert("previousStyle".to_string(), json!(previous_style));
    }
    result.insert("style".to_string(), json!(style_id));
    result.insert(
        "contentHash".to_string(),
        json!(updated_report.content_hash),
    );
    result.insert("previousHash".to_string(), json!(previous_hash));
    if matches!(target, DocxStyleTarget::Paragraph | DocxStyleTarget::Run)
        && !updated_report.para_id.is_empty()
    {
        result.insert(
            "handle".to_string(),
            json!(format!("H:docx/pt:doc/para:m:{}", updated_report.para_id)),
        );
    }
    if !style_handle.is_empty() {
        result.insert("styleHandle".to_string(), json!(style_handle));
    }
    Ok(Value::Object(result))
}

fn normalize_docx_style_type(value: Option<&str>) -> CliResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_lowercase();
    match normalized.as_str() {
        "paragraph" | "character" | "table" | "numbering" => Ok(Some(normalized)),
        _ => Err(CliError::invalid_args(
            "--type must be one of paragraph, character, table, numbering",
        )),
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DocxStyleTarget {
    Paragraph,
    Run,
    Table,
}

impl DocxStyleTarget {
    fn as_str(self) -> &'static str {
        match self {
            DocxStyleTarget::Paragraph => "paragraph",
            DocxStyleTarget::Run => "run",
            DocxStyleTarget::Table => "table",
        }
    }

    fn required_style_type(self) -> &'static str {
        match self {
            DocxStyleTarget::Paragraph => "paragraph",
            DocxStyleTarget::Run => "character",
            DocxStyleTarget::Table => "table",
        }
    }
}

fn normalize_docx_style_target(value: &str) -> CliResult<DocxStyleTarget> {
    match value.trim().to_ascii_lowercase().as_str() {
        "paragraph" => Ok(DocxStyleTarget::Paragraph),
        "run" => Ok(DocxStyleTarget::Run),
        "table" => Ok(DocxStyleTarget::Table),
        _ => Err(CliError::invalid_args(
            "--target must be one of paragraph, run, table",
        )),
    }
}

fn validate_docx_style_for_target(
    styles: &[DocxStyleInfo],
    style_id: &str,
    target: DocxStyleTarget,
) -> CliResult<()> {
    let wanted = target.required_style_type();
    if let Some(style) = styles.iter().find(|style| style.style_id == style_id) {
        if style.style_type != wanted {
            return Err(CliError::invalid_args(format!(
                "style type mismatch: {:?} is a {} style but {} target requires a {} style",
                style_id,
                style.style_type,
                target.as_str(),
                wanted
            )));
        }
        return Ok(());
    }
    let mut candidates: Vec<&str> = styles
        .iter()
        .filter(|style| style.style_type == wanted)
        .map(|style| style.style_id.as_str())
        .collect();
    candidates.sort_unstable();
    let detail = if candidates.is_empty() {
        format!(
            "style not found: {:?} ({}); no {} styles defined",
            style_id, wanted, wanted
        )
    } else {
        format!(
            "style not found: {:?} ({}); available {} styles: [{}]",
            style_id,
            wanted,
            wanted,
            candidates.join(" ")
        )
    };
    Err(CliError::target_not_found(detail))
}

fn resolve_docx_style_handle_id(
    styles: &[DocxStyleInfo],
    styles_part: Option<&str>,
    handle: &str,
) -> CliResult<String> {
    let style_id = parse_docx_style_handle_style_id(handle)?;
    if styles_part.is_none() {
        return Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_SCOPE_STALE,
            "document has no styles part",
            handle,
        ));
    }
    let matches = styles
        .iter()
        .filter(|style| style.style_id == style_id)
        .count();
    match matches {
        0 => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_STALE,
            format!("no style with w:styleId {style_id:?} in document"),
            handle,
        )),
        1 => Ok(style_id),
        count => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_AMBIGUOUS,
            format!("w:styleId {style_id:?} is not unique ({count} styles share it)"),
            handle,
        )),
    }
}

fn require_docx_block_hash(value: &str) -> CliResult<()> {
    if value.trim().is_empty() {
        return Err(CliError::invalid_args("--expect-hash is required"));
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

fn docx_document_and_styles_parts(file: &str) -> CliResult<(String, Option<String>)> {
    let entries = zip_entry_names(file)?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        return Err(CliError::unsupported_type(
            "file is not a DOCX document (detected: unknown)",
        ));
    }
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let styles_uri = find_docx_styles_part(file, &entries, &document_part)?;
    Ok((document_uri, styles_uri))
}

fn find_docx_styles_part(
    file: &str,
    entries: &[String],
    document_part: &str,
) -> CliResult<Option<String>> {
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let rels_part = relationships_part_for(document_part);
    for rel in relationship_entries(file, &rels_part).unwrap_or_default() {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
            || rel.rel_type.ends_with("/styles")
        {
            return Ok(Some(resolve_relationship_target(
                &document_uri,
                &rel.target,
            )));
        }
    }
    for entry in entries {
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        let uri = format!("/{}", entry.trim_start_matches('/'));
        if is_docx_styles_part(&uri, &content_type) {
            return Ok(Some(uri));
        }
    }
    Ok(None)
}

#[derive(Clone, Default)]
struct DocxStyleInfo {
    style_id: String,
    name: String,
    style_type: String,
    default: bool,
    builtin: bool,
    based_on: String,
    next: String,
}

fn docx_styles(file: &str, styles_part: &str) -> CliResult<Vec<DocxStyleInfo>> {
    let xml = zip_text(file, styles_part.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut saw_root = false;
    let mut current: Option<DocxStyleInfo> = None;
    let mut styles = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "styles" {
                        return Err(CliError::unexpected(format!(
                            "styles part {styles_part} root is {name:?}, expected styles"
                        )));
                    }
                } else if name == "style" {
                    current = Some(docx_style_from_element(&e));
                } else {
                    docx_note_style_child(&e, &name, &mut current);
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "styles" {
                        return Err(CliError::unexpected(format!(
                            "styles part {styles_part} root is {name:?}, expected styles"
                        )));
                    }
                } else if name == "style" {
                    styles.push(docx_style_from_element(&e));
                } else {
                    docx_note_style_child(&e, &name, &mut current);
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "style" => {
                if let Some(style) = current.take() {
                    styles.push(style);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !saw_root {
        return Err(CliError::unexpected(format!(
            "styles part {styles_part} has no root element"
        )));
    }
    Ok(styles)
}

fn docx_style_from_element(element: &BytesStart<'_>) -> DocxStyleInfo {
    DocxStyleInfo {
        style_id: attr(element, "styleId").unwrap_or_default(),
        style_type: attr(element, "type").unwrap_or_default(),
        default: docx_on_off_attr(element, "default"),
        builtin: !docx_on_off_attr(element, "customStyle"),
        ..DocxStyleInfo::default()
    }
}

fn docx_note_style_child(
    element: &BytesStart<'_>,
    name: &str,
    current: &mut Option<DocxStyleInfo>,
) {
    let Some(style) = current.as_mut() else {
        return;
    };
    let Some(value) = attr(element, "val") else {
        return;
    };
    match name {
        "name" => style.name = value,
        "basedOn" => style.based_on = value,
        "next" => style.next = value,
        _ => {}
    }
}

fn docx_on_off_attr(element: &BytesStart<'_>, name: &str) -> bool {
    match attr(element, name).as_deref() {
        None => false,
        Some("0" | "false" | "off") => false,
        Some(_) => true,
    }
}

fn docx_style_id_counts(styles: &[DocxStyleInfo]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for style in styles {
        if !style.style_id.is_empty() {
            *counts.entry(style.style_id.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn docx_style_json(style: &DocxStyleInfo, counts: &BTreeMap<String, usize>) -> Value {
    let mut object = Map::new();
    object.insert("styleId".to_string(), json!(style.style_id));
    if !style.name.is_empty() {
        object.insert("name".to_string(), json!(style.name));
    }
    if !style.style_type.is_empty() {
        object.insert("type".to_string(), json!(style.style_type));
    }
    object.insert("default".to_string(), json!(style.default));
    object.insert("builtin".to_string(), json!(style.builtin));
    if !style.based_on.is_empty() {
        object.insert("basedOn".to_string(), json!(style.based_on));
    }
    if !style.next.is_empty() {
        object.insert("next".to_string(), json!(style.next));
    }
    if !style.style_id.is_empty() {
        object.insert("primarySelector".to_string(), json!(style.style_id));
        object.insert("selectors".to_string(), json!([style.style_id]));
        if counts.get(&style.style_id).copied().unwrap_or_default() == 1 {
            object.insert(
                "handle".to_string(),
                json!(format!("H:docx/pt:styles/style:n:{}", style.style_id)),
            );
        }
    }
    Value::Object(object)
}

fn docx_comments_list(file: &str, comment_id: Option<i64>) -> CliResult<Value> {
    let (document_part, comments_part) = docx_document_and_comments_parts(file)?;
    let mut comments = Vec::new();
    if let Some(comments_part) = comments_part.as_deref() {
        comments = docx_comments(file, comments_part, &document_part)?;
    }
    if let Some(comment_id) = comment_id {
        comments.retain(|comment| comment.id == comment_id);
        if comments.is_empty() {
            return Err(CliError::target_not_found(format!(
                "target not found: comment {comment_id}"
            )));
        }
    }
    let counts = docx_comment_id_counts(&comments);
    let comment_values = comments
        .iter()
        .map(|comment| docx_comment_json(comment, &counts))
        .collect::<Vec<_>>();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("documentPartUri".to_string(), json!(document_part));
    if let Some(comments_part) = comments_part {
        result.insert("commentsPart".to_string(), json!(comments_part));
    }
    result.insert("comments".to_string(), Value::Array(comment_values));
    Ok(Value::Object(result))
}

fn docx_comments_add(
    file: &str,
    anchor_block: i64,
    author: &str,
    initials: &str,
    date: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    let text = resolve_optional_docx_paragraph_text(options.text, options.text_file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        let detected = match detect_inspect_package_type(file, &entries) {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let document_xml = zip_text(file, &document_part)?;
    let anchor_index = if anchor_block == 0 {
        1
    } else {
        anchor_block as usize
    };
    let reports = docx_rich_block_reports(&document_xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    if reports.is_empty() {
        return Err(CliError::unexpected(
            "failed to mutate comments: document has no body blocks to anchor a comment to",
        ));
    }
    let report = reports.get(anchor_index.saturating_sub(1)).ok_or_else(|| {
        CliError::invalid_args(format!("comment anchor block out of range: {anchor_index}"))
    })?;
    if report.kind != "paragraph" {
        return Err(CliError::invalid_args(format!(
            "comment anchor block is not a paragraph: block {anchor_index} is {}",
            report.kind
        )));
    }

    let comments_part = docx_comments_part_uri(file, &entries, &document_part)?;
    let comments_part_key = comments_part.trim_start_matches('/').to_string();
    let created_part = !zip_entry_exists(&entries, &comments_part);
    let comments_xml = if created_part {
        docx_comments_template()
    } else {
        zip_text(file, &comments_part_key)?
    };
    let comment_id = docx_next_comment_id(&comments_xml);
    let updated_document_xml =
        insert_docx_comment_markers_xml(&document_xml, anchor_index, comment_id)?;
    let updated_comments_xml =
        append_docx_comment_xml(&comments_xml, comment_id, author, date, initials, &text)?;
    let (rels_part, rels_xml, created_ref) =
        ensure_docx_comments_relationship_xml(file, &document_part, &document_uri, &comments_part)?;
    let content_types_xml = ensure_content_type_override(
        zip_text(file, "[Content_Types].xml")?,
        &comments_part,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml",
    );

    let mut overrides = BTreeMap::new();
    overrides.insert(document_part.clone(), updated_document_xml);
    overrides.insert(comments_part_key, updated_comments_xml);
    overrides.insert("[Content_Types].xml".to_string(), content_types_xml);
    if let Some(rels_xml) = rels_xml {
        overrides.insert(rels_part, rels_xml);
    }
    write_docx_mutation_overrides_output(file, &overrides, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("commentId".to_string(), json!(comment_id));
    result.insert("author".to_string(), json!(author));
    if !date.is_empty() {
        result.insert("date".to_string(), json!(date));
    }
    if !initials.is_empty() {
        result.insert("initials".to_string(), json!(initials));
    }
    result.insert("text".to_string(), json!(text));
    result.insert(
        "contentHash".to_string(),
        json!(docx_comment_content_hash(author, date, &text)),
    );
    result.insert("anchoredToBlock".to_string(), json!(anchor_index));
    result.insert("createdPart".to_string(), json!(created_part));
    result.insert("createdRef".to_string(), json!(created_ref));
    result.insert("operation".to_string(), json!("added"));
    Ok(Value::Object(result))
}

struct DocxCommentEditSpec<'a> {
    expect_hash: &'a str,
    text: &'a str,
    text_set: bool,
    author: &'a str,
    author_set: bool,
    date: &'a str,
    date_set: bool,
}

fn docx_comments_edit(
    file: &str,
    comment_id: i64,
    handle: Option<&str>,
    edit: DocxCommentEditSpec<'_>,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        let detected = match detect_inspect_package_type(file, &entries) {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let comments_part = docx_comments_part_uri(file, &entries, &document_part)?;
    if !zip_entry_exists(&entries, &comments_part) {
        if let Some(handle) = handle.filter(|value| !value.trim().is_empty()) {
            return Err(docx_handle_error(
                EXIT_TARGET_NOT_FOUND,
                HANDLE_SCOPE_STALE,
                "document has no comments part",
                handle,
            ));
        }
        return Err(CliError::target_not_found("target not found: comment"));
    }
    let comments_part_key = comments_part.trim_start_matches('/').to_string();
    let comments_xml = zip_text(file, &comments_part_key)?;
    let target_id = if let Some(handle) = handle.filter(|value| !value.trim().is_empty()) {
        resolve_docx_comment_handle_id(&comments_xml, handle)? as i64
    } else {
        comment_id
    };
    let (updated_comments_xml, before, edited) =
        edit_docx_comment_xml(&comments_xml, target_id, edit)?;

    let mut overrides = BTreeMap::new();
    overrides.insert(comments_part_key, updated_comments_xml);
    write_docx_mutation_overrides_output(file, &overrides, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("commentId".to_string(), json!(target_id));
    result.insert("author".to_string(), json!(edited.author));
    if !edited.date.is_empty() {
        result.insert("date".to_string(), json!(edited.date));
    }
    if !edited.initials.is_empty() {
        result.insert("initials".to_string(), json!(edited.initials));
    }
    result.insert("text".to_string(), json!(edited.text));
    result.insert(
        "contentHash".to_string(),
        json!(docx_comment_content_hash(
            &edited.author,
            &edited.date,
            &edited.text
        )),
    );
    result.insert("previousText".to_string(), json!(before.text));
    result.insert(
        "previousHash".to_string(),
        json!(docx_comment_content_hash(
            &before.author,
            &before.date,
            &before.text
        )),
    );
    result.insert("operation".to_string(), json!("edited"));
    result.insert(
        "handle".to_string(),
        json!(format!("H:docx/pt:doc/comment:n:{target_id}")),
    );
    Ok(Value::Object(result))
}

fn docx_comments_remove(
    file: &str,
    comment_id: i64,
    handle: Option<&str>,
    expect_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        let detected = match detect_inspect_package_type(file, &entries) {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let comments_part = docx_comments_part_uri(file, &entries, &document_part)?;
    if !zip_entry_exists(&entries, &comments_part) {
        if let Some(handle) = handle.filter(|value| !value.trim().is_empty()) {
            return Err(docx_handle_error(
                EXIT_TARGET_NOT_FOUND,
                HANDLE_SCOPE_STALE,
                "document has no comments part",
                handle,
            ));
        }
        return Err(CliError::target_not_found("target not found: comment"));
    }
    let comments_part_key = comments_part.trim_start_matches('/').to_string();
    let comments_xml = zip_text(file, &comments_part_key)?;
    let target_id = if let Some(handle) = handle.filter(|value| !value.trim().is_empty()) {
        resolve_docx_comment_handle_id(&comments_xml, handle)? as i64
    } else {
        comment_id
    };
    let (updated_comments_xml, before) =
        remove_docx_comment_xml(&comments_xml, target_id, expect_hash)?;
    let document_xml = zip_text(file, &document_part)?;
    let (updated_document_xml, range_markers_removed) =
        remove_docx_comment_markers_xml(&document_xml, target_id)?;

    let mut overrides = BTreeMap::new();
    overrides.insert(comments_part_key, updated_comments_xml);
    if range_markers_removed {
        overrides.insert(document_part.clone(), updated_document_xml);
    }
    write_docx_mutation_overrides_output(file, &overrides, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("commentId".to_string(), json!(target_id));
    result.insert("previousAuthor".to_string(), json!(before.author));
    result.insert("previousText".to_string(), json!(before.text));
    result.insert(
        "previousHash".to_string(),
        json!(docx_comment_content_hash(
            &before.author,
            &before.date,
            &before.text
        )),
    );
    result.insert(
        "rangeMarkersRemoved".to_string(),
        json!(range_markers_removed),
    );
    result.insert("operation".to_string(), json!("removed"));
    Ok(Value::Object(result))
}

fn docx_validate_strict_command(file: &str) -> String {
    format!("ooxml validate --strict {file}")
}

fn docx_first_word_attr(fragment: &str, name: &[u8]) -> Option<String> {
    let mut reader = NsReader::from_str(fragment);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                return docx_word_attr_ns(&e, reader.resolver(), name)
                    .or_else(|| attr(&e, std::str::from_utf8(name).ok()?));
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn docx_word_text_descendants(fragment: &str, wanted: &str) -> String {
    let mut reader = NsReader::from_str(fragment);
    let mut text = String::new();
    let mut in_wanted = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if local_name(e.name().as_ref()) == wanted {
                    in_wanted = true;
                }
            }
            Ok(Event::Text(e)) if in_wanted => text.push_str(&decode_xml_text(e.as_ref())),
            Ok(Event::GeneralRef(e)) if in_wanted => text.push_str(&xml_general_ref(e.as_ref())),
            Ok(Event::CData(e)) if in_wanted => text.push_str(&String::from_utf8_lossy(e.as_ref())),
            Ok(Event::End(e)) => {
                if local_name(e.name().as_ref()) == wanted {
                    in_wanted = false;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    text
}

fn xml_fragment_text(fragment: &str) -> String {
    let mut reader = NsReader::from_str(fragment);
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(e)) => text.push_str(&decode_xml_text(e.as_ref())),
            Ok(Event::GeneralRef(e)) => text.push_str(&xml_general_ref(e.as_ref())),
            Ok(Event::CData(e)) => text.push_str(&String::from_utf8_lossy(e.as_ref())),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    text
}

fn docx_header_footer_part_uris(
    file: &str,
    document_part: &str,
    document_uri: &str,
    document_xml: &str,
) -> CliResult<Vec<String>> {
    let rels_part = relationships_part_for(document_part);
    let rel_targets = relationship_entries(file, &rels_part)
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| rel.target_mode != "External")
        .map(|rel| {
            (
                rel.id,
                resolve_relationship_target(document_uri, &rel.target),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut reader = NsReader::from_str(document_xml);
    let mut stack: Vec<String> = Vec::new();
    let mut section_uris = Vec::new();
    let mut seen = BTreeSet::new();
    let mut in_direct_section = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    in_direct_section = true;
                } else if in_direct_section
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                    && let Some(part_uri) =
                        docx_header_footer_ref_part_uri(&e, reader.resolver(), &rel_targets)
                    && seen.insert(part_uri.clone())
                {
                    section_uris.push(part_uri);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    // Empty section properties have no references.
                } else if in_direct_section
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                    && let Some(part_uri) =
                        docx_header_footer_ref_part_uri(&e, reader.resolver(), &rel_targets)
                    && seen.insert(part_uri.clone())
                {
                    section_uris.push(part_uri);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "sectPr" {
                    in_direct_section = false;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(section_uris)
}

fn docx_header_footer_ref_part_uri(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    rel_targets: &BTreeMap<String, String>,
) -> Option<String> {
    let id = attr_prefixed_ns(
        element,
        resolver,
        b"r",
        b"http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        b"id",
    )?;
    rel_targets.get(&id).cloned()
}

fn docx_word_attr_ns(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    wanted_local: &[u8],
) -> Option<String> {
    attr_prefixed_ns(element, resolver, b"w", DOCX_W_NS, wanted_local)
}

fn docx_headers_footers_list(file: &str) -> CliResult<Value> {
    let (document_uri, sections) = docx_header_footer_listing(file)?;
    Ok(json!({
        "file": file,
        "documentPartUri": document_uri,
        "sections": sections,
    }))
}

fn docx_header_footer_listing(file: &str) -> CliResult<(String, Vec<Value>)> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let xml = zip_text(file, &document_part).map_err(|err| {
        CliError::unexpected(format!(
            "failed to list headers/footers: failed to read document part {document_uri}: {}",
            err.message
        ))
    })?;
    let rel_targets = relationship_entries(file, &relationships_part_for(&document_part))
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| rel.target_mode != "External")
        .map(|rel| {
            (
                rel.id,
                resolve_relationship_target(&document_uri, &rel.target),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let sections = docx_header_footer_sections(file, &xml, &rel_targets)?;
    Ok((document_uri, sections))
}

#[derive(Default)]
struct DocxHeaderFooterSectionBuild {
    section_index: usize,
    headers: DocxHeaderFooterSetBuild,
    footers: DocxHeaderFooterSetBuild,
}

#[derive(Default)]
struct DocxHeaderFooterSetBuild {
    default: Option<Value>,
    first: Option<Value>,
    even: Option<Value>,
}

#[derive(Clone, Debug, Default)]
struct DocxHeaderFooterRefInfo {
    kind: String,
    id: String,
    ref_type: String,
    section: i64,
    primary_selector: String,
    selectors: Vec<String>,
    part_uri: String,
}

#[derive(Default)]
struct DocxHeaderFooterSelector {
    kind: String,
    id: String,
    ref_type: String,
    section: i64,
    part_uri: String,
    paragraph_index: i64,
}

fn docx_header_footer_sections(
    file: &str,
    document_xml: &str,
    rel_targets: &BTreeMap<String, String>,
) -> CliResult<Vec<Value>> {
    let mut reader = NsReader::from_str(document_xml);
    let mut stack: Vec<String> = Vec::new();
    let mut sections = Vec::new();
    let mut current = None::<DocxHeaderFooterSectionBuild>;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if current.is_none()
                    && is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    current = Some(DocxHeaderFooterSectionBuild {
                        section_index: sections.len() + 1,
                        ..DocxHeaderFooterSectionBuild::default()
                    });
                } else if let Some(section) = current.as_mut()
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                {
                    docx_note_header_footer_ref(
                        file,
                        section,
                        &e,
                        reader.resolver(),
                        &name,
                        rel_targets,
                    );
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if current.is_none()
                    && is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    let section = DocxHeaderFooterSectionBuild {
                        section_index: sections.len() + 1,
                        ..DocxHeaderFooterSectionBuild::default()
                    };
                    sections.push(docx_header_footer_section_json(section));
                } else if let Some(section) = current.as_mut()
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                {
                    docx_note_header_footer_ref(
                        file,
                        section,
                        &e,
                        reader.resolver(),
                        &name,
                        rel_targets,
                    );
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "sectPr"
                    && let Some(section) = current.take()
                {
                    sections.push(docx_header_footer_section_json(section));
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(sections)
}

fn docx_note_header_footer_ref(
    file: &str,
    section: &mut DocxHeaderFooterSectionBuild,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    name: &str,
    rel_targets: &BTreeMap<String, String>,
) {
    let kind = if name == "footerReference" {
        "footer"
    } else {
        "header"
    };
    let id = attr_bound_ns(
        element,
        resolver,
        b"http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        b"id",
    )
    .unwrap_or_default();
    let ref_type = normalize_docx_header_footer_type(
        attr_bound_ns(element, resolver, DOCX_W_NS, b"type").unwrap_or_default(),
    );
    let part_uri = rel_targets.get(&id).cloned().unwrap_or_default();
    let content_type = if part_uri.is_empty() {
        String::new()
    } else {
        content_type_for_part(file, &part_uri).unwrap_or_default()
    };
    let value = docx_header_footer_ref_json(
        kind,
        &id,
        &ref_type,
        section.section_index,
        &part_uri,
        &content_type,
    );
    let set = if kind == "footer" {
        &mut section.footers
    } else {
        &mut section.headers
    };
    match ref_type.as_str() {
        "first" => set.first = Some(value),
        "even" => set.even = Some(value),
        _ => set.default = Some(value),
    }
}

fn normalize_docx_header_footer_type(value: String) -> String {
    match value.as_str() {
        "first" | "even" => value,
        _ => "default".to_string(),
    }
}

fn docx_header_footer_ref_json(
    kind: &str,
    id: &str,
    ref_type: &str,
    section: usize,
    part_uri: &str,
    content_type: &str,
) -> Value {
    let primary_selector = format!("{kind}:{section}:{ref_type}");
    let mut selectors = vec![primary_selector.clone()];
    if !id.is_empty() {
        selectors.push(format!("id:{id}"));
        selectors.push(id.to_string());
    }
    if !part_uri.is_empty() {
        selectors.push(format!("part:{part_uri}"));
        selectors.push(part_uri.to_string());
    }
    json!({
        "kind": kind,
        "id": id,
        "type": ref_type,
        "section": section,
        "primarySelector": primary_selector,
        "selectors": selectors,
        "partUri": part_uri,
        "contentType": content_type,
    })
}

fn docx_header_footer_section_json(section: DocxHeaderFooterSectionBuild) -> Value {
    json!({
        "sectionIndex": section.section_index,
        "headers": docx_header_footer_set_json(section.headers),
        "footers": docx_header_footer_set_json(section.footers),
    })
}

fn docx_header_footer_set_json(set: DocxHeaderFooterSetBuild) -> Value {
    json!({
        "default": set.default.unwrap_or(Value::Null),
        "first": set.first.unwrap_or(Value::Null),
        "even": set.even.unwrap_or(Value::Null),
    })
}

fn docx_headers_footers_show(file: &str, kind: &str, rest: &[String]) -> CliResult<Value> {
    reject_unknown_flags(rest, &["--id", "--type", "--section", "--selector"], &[])?;
    let id = parse_string_flag(rest, "--id")?.unwrap_or_default();
    let ref_type = parse_string_flag(rest, "--type")?.unwrap_or_else(|| "default".to_string());
    let ref_type = normalize_docx_header_footer_show_type(&ref_type)?;
    let section = parse_i64_flag(rest, "--section")?.unwrap_or(0);
    if section < 0 {
        return Err(CliError::invalid_args(
            "--section must be >= 0 (0 means the last section)",
        ));
    }
    let selector = parse_string_flag(rest, "--selector")?;
    if selector.is_some()
        && (has_flag(rest, "--id") || has_flag(rest, "--type") || has_flag(rest, "--section"))
    {
        return Err(CliError::invalid_args(
            "cannot specify --selector with --id, --type, or --section",
        ));
    }

    let (_document_uri, sections) = docx_header_footer_listing(file)?;
    let target = if let Some(selector) = selector {
        let parsed = parse_docx_header_footer_selector(kind, &selector)?;
        resolve_docx_header_footer_selector(&sections, kind, &parsed)
    } else if !id.is_empty() {
        resolve_docx_header_footer_selector(
            &sections,
            kind,
            &DocxHeaderFooterSelector {
                kind: kind.to_string(),
                id,
                ref_type,
                section,
                ..DocxHeaderFooterSelector::default()
            },
        )
    } else {
        resolve_docx_header_footer_selector(
            &sections,
            kind,
            &DocxHeaderFooterSelector {
                kind: kind.to_string(),
                ref_type,
                section,
                ..DocxHeaderFooterSelector::default()
            },
        )
    }
    .ok_or_else(|| CliError::target_not_found(format!("target not found: {kind}")))?;

    if target.part_uri.is_empty() {
        return Err(CliError::invalid_args(format!(
            "{kind} reference {:?} does not resolve to a part",
            target.id
        )));
    }
    let paragraphs = docx_header_footer_paragraphs(file, &target)?;
    Ok(json!({
        "file": file,
        "kind": target.kind,
        "partUri": target.part_uri,
        "id": target.id,
        "type": target.ref_type,
        "section": target.section,
        "primarySelector": target.primary_selector,
        "selectors": target.selectors,
        "paragraphs": paragraphs,
    }))
}

fn docx_header_footer_kind(group: &str) -> &'static str {
    if group == "footers" {
        "footer"
    } else {
        "header"
    }
}

fn normalize_docx_header_footer_show_type(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "default" => Ok("default".to_string()),
        "first" => Ok("first".to_string()),
        "even" => Ok("even".to_string()),
        _ => Err(CliError::invalid_args(
            "--type must be one of default, first, even",
        )),
    }
}

fn parse_docx_header_footer_selector(
    command_kind: &str,
    raw: &str,
) -> CliResult<DocxHeaderFooterSelector> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(CliError::invalid_args("--selector cannot be empty"));
    }
    let (base, paragraph_index) = split_docx_header_footer_paragraph_selector(raw)?;
    let mut selector = DocxHeaderFooterSelector {
        kind: command_kind.to_string(),
        ref_type: "default".to_string(),
        paragraph_index,
        ..DocxHeaderFooterSelector::default()
    };
    if let Some(id) = base.strip_prefix("id:") {
        if id.is_empty() {
            return Err(CliError::invalid_args(
                "--selector id:<relId> cannot be empty",
            ));
        }
        selector.id = id.to_string();
        return Ok(selector);
    }
    if let Some(part_uri) = base.strip_prefix("part:") {
        if part_uri.is_empty() {
            return Err(CliError::invalid_args(
                "--selector part:<partUri> cannot be empty",
            ));
        }
        selector.part_uri = part_uri.to_string();
        return Ok(selector);
    }
    if base.starts_with('/') {
        selector.part_uri = base.to_string();
        return Ok(selector);
    }
    if base.starts_with("rId") {
        selector.id = base.to_string();
        return Ok(selector);
    }
    if let Some(rest) = base.strip_prefix("section:") {
        let parts = rest.split(':').collect::<Vec<_>>();
        if parts.len() != 3 || parts[1] != "type" {
            return Err(CliError::invalid_args(
                "--selector section form must be section:<n>:type:<default|first|even>",
            ));
        }
        selector.section = parse_positive_i64(parts[0], "selector section")?;
        selector.ref_type = normalize_docx_header_footer_show_type(parts[2])?;
        return Ok(selector);
    }

    let parts = base.split(':').collect::<Vec<_>>();
    if parts.len() == 3 && (parts[0] == "header" || parts[0] == "footer") {
        if parts[0] != command_kind {
            return Err(CliError::invalid_args(format!(
                "--selector kind {:?} does not match {command_kind} command",
                parts[0]
            )));
        }
        selector.kind = parts[0].to_string();
        selector.section = parse_positive_i64(parts[1], "selector section")?;
        selector.ref_type = normalize_docx_header_footer_show_type(parts[2])?;
        return Ok(selector);
    }

    Err(CliError::invalid_args(
        "--selector must be header:<section>:<type>, footer:<section>:<type>, section:<section>:type:<type>, id:<relId>, or part:<partUri>",
    ))
}

fn split_docx_header_footer_paragraph_selector(raw: &str) -> CliResult<(&str, i64)> {
    for marker in ["/paragraph:", "/p:"] {
        if let Some(index) = raw.rfind(marker) {
            let base = raw[..index].trim();
            let value = raw[index + marker.len()..].trim();
            if base.is_empty() {
                return Err(CliError::invalid_args(
                    "--selector paragraph suffix requires a header/footer selector before it",
                ));
            }
            let paragraph_index = parse_positive_i64(value, "selector paragraph")?;
            return Ok((base, paragraph_index));
        }
    }
    Ok((raw, 0))
}

fn parse_positive_i64(value: &str, label: &str) -> CliResult<i64> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args(format!("{label} cannot be empty")));
    }
    let parsed = value
        .parse::<i64>()
        .map_err(|_| CliError::invalid_args(format!("{label} must be an integer")))?;
    if parsed < 1 {
        return Err(CliError::invalid_args(format!("{label} must be >= 1")));
    }
    Ok(parsed)
}

fn resolve_docx_header_footer_selector(
    sections: &[Value],
    command_kind: &str,
    selector: &DocxHeaderFooterSelector,
) -> Option<DocxHeaderFooterRefInfo> {
    let kind = if selector.kind.is_empty() {
        command_kind
    } else {
        &selector.kind
    };
    let refs = docx_header_footer_refs(sections, kind);
    if !selector.id.is_empty() {
        return refs
            .into_iter()
            .find(|reference| reference.id == selector.id);
    }
    if !selector.part_uri.is_empty() {
        return refs
            .into_iter()
            .find(|reference| reference.part_uri == selector.part_uri);
    }
    let section = if selector.section > 0 {
        selector.section
    } else {
        sections
            .last()
            .and_then(|section| section["sectionIndex"].as_i64())
            .unwrap_or(0)
    };
    refs.into_iter()
        .find(|reference| reference.section == section && reference.ref_type == selector.ref_type)
}

fn docx_header_footer_refs(sections: &[Value], kind: &str) -> Vec<DocxHeaderFooterRefInfo> {
    let mut refs = Vec::new();
    for section in sections {
        let set = if kind == "footer" {
            &section["footers"]
        } else {
            &section["headers"]
        };
        for ref_type in ["default", "first", "even"] {
            if let Some(reference) = docx_header_footer_ref_info(&set[ref_type]) {
                refs.push(reference);
            }
        }
    }
    refs
}

fn docx_header_footer_ref_info(value: &Value) -> Option<DocxHeaderFooterRefInfo> {
    if value.is_null() {
        return None;
    }
    let selectors = value["selectors"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();
    Some(DocxHeaderFooterRefInfo {
        kind: value["kind"].as_str()?.to_string(),
        id: value["id"].as_str().unwrap_or_default().to_string(),
        ref_type: value["type"].as_str().unwrap_or_default().to_string(),
        section: value["section"].as_i64().unwrap_or_default(),
        primary_selector: value["primarySelector"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        selectors,
        part_uri: value["partUri"].as_str().unwrap_or_default().to_string(),
    })
}

fn docx_header_footer_paragraphs(
    file: &str,
    reference: &DocxHeaderFooterRefInfo,
) -> CliResult<Vec<Value>> {
    let xml = zip_text(file, reference.part_uri.trim_start_matches('/')).map_err(|err| {
        CliError::unexpected(format!(
            "failed to read header/footer part {}: {}",
            reference.part_uri, err.message
        ))
    })?;
    let mut reader = NsReader::from_str(&xml);
    let mut stack = Vec::<String>::new();
    let mut paragraphs = Vec::new();
    let mut current = None::<DocxHeaderFooterParagraphBuild>;
    let mut in_t = false;
    let mut skip_text_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    current = Some(DocxHeaderFooterParagraphBuild::default());
                }
                docx_note_header_footer_paragraph_start(
                    &mut current,
                    &e,
                    reader.resolver(),
                    &stack,
                    is_word,
                    skip_text_depth,
                );
                if is_word && name == "t" {
                    in_t = true;
                }
                if is_word && matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth += 1;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    let paragraph = DocxHeaderFooterParagraphBuild::default();
                    paragraphs.push(docx_header_footer_paragraph_json(
                        paragraphs.len() + 1,
                        paragraph,
                        reference,
                    ));
                } else {
                    docx_note_header_footer_paragraph_start(
                        &mut current,
                        &e,
                        reader.resolver(),
                        &stack,
                        is_word,
                        skip_text_depth,
                    );
                }
            }
            Ok(Event::Text(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph.text.push_str(&xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph
                        .text
                        .push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_t = false;
                } else if matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth = skip_text_depth.saturating_sub(1);
                } else if name == "p"
                    && let Some(paragraph) = current.take()
                {
                    paragraphs.push(docx_header_footer_paragraph_json(
                        paragraphs.len() + 1,
                        paragraph,
                        reference,
                    ));
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(paragraphs)
}

#[derive(Default)]
struct DocxHeaderFooterParagraphBuild {
    style: String,
    text: String,
}

fn docx_note_header_footer_paragraph_start(
    current: &mut Option<DocxHeaderFooterParagraphBuild>,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    stack: &[String],
    is_word: bool,
    skip_text_depth: usize,
) {
    let Some(paragraph) = current.as_mut() else {
        return;
    };
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if is_word
        && name == "pStyle"
        && stack.last().is_some_and(|parent| parent == "pPr")
        && let Some(style) = docx_word_attr_ns(element, resolver, b"val")
    {
        paragraph.style = style;
        return;
    }
    if is_word && skip_text_depth == 0 {
        match name {
            "tab" => paragraph.text.push('\t'),
            "br" | "cr" => paragraph.text.push('\n'),
            "noBreakHyphen" => paragraph.text.push('-'),
            _ => {}
        }
    }
}

fn docx_header_footer_paragraph_json(
    index: usize,
    paragraph: DocxHeaderFooterParagraphBuild,
    reference: &DocxHeaderFooterRefInfo,
) -> Value {
    let primary_selector = if reference.primary_selector.is_empty() {
        String::new()
    } else {
        format!("{}/p:{index}", reference.primary_selector)
    };
    let mut selectors = Vec::new();
    for selector in &reference.selectors {
        selectors.push(format!("{selector}/p:{index}"));
        selectors.push(format!("{selector}/paragraph:{index}"));
    }
    json!({
        "index": index,
        "primarySelector": primary_selector,
        "selectors": selectors,
        "style": paragraph.style,
        "text": paragraph.text,
    })
}

struct DocxHeaderFooterSetTextOptions<'a> {
    id: &'a str,
    ref_type: &'a str,
    section: i64,
    index: i64,
    selector: Option<&'a str>,
    selector_given: bool,
    index_given: bool,
    text: &'a str,
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

struct DocxHeaderFooterEnsureResult {
    document_xml: String,
    rels_part: Option<String>,
    rels_xml: Option<String>,
    content_types_xml: Option<String>,
    part_xml: Option<String>,
    reference: DocxHeaderFooterRefInfo,
    created_part: bool,
    created_ref: bool,
}

struct DocxHeaderFooterEnsureContext<'a> {
    file: &'a str,
    entries: &'a [String],
    document_part: &'a str,
    document_uri: &'a str,
    document_xml: &'a str,
}

#[derive(Clone, Copy)]
struct DocxSectionRange {
    index: i64,
    start: usize,
    end: usize,
}

fn docx_headers_footers_set_text(
    file: &str,
    kind: &str,
    mut options: DocxHeaderFooterSetTextOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let entries = zip_entry_names(file)?;
    ensure_docx_package_kind(file, &entries)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let document_xml = zip_text(file, &document_part)?;

    let mut created_part = false;
    let mut created_ref = false;
    let mut document_override = None::<String>;
    let mut rels_override = None::<(String, String)>;
    let mut content_types_override = None::<String>;
    let mut part_xml_override = None::<String>;

    let reference = if options.selector_given {
        let selector = options.selector.unwrap_or_default();
        let parsed = parse_docx_header_footer_selector(kind, selector)?;
        if parsed.paragraph_index > 0 {
            if options.index_given && options.index != parsed.paragraph_index {
                return Err(CliError::invalid_args(
                    "--index conflicts with the paragraph index embedded in --selector",
                ));
            }
            options.index = parsed.paragraph_index;
        }
        if !parsed.id.is_empty() || !parsed.part_uri.is_empty() {
            let (_document_uri, sections) = docx_header_footer_listing(file)?;
            resolve_docx_header_footer_selector(&sections, kind, &parsed).ok_or_else(|| {
                CliError::target_not_found(format!("{kind} not found: {selector}"))
            })?
        } else {
            let ensured = ensure_docx_header_footer(
                DocxHeaderFooterEnsureContext {
                    file,
                    entries: &entries,
                    document_part: &document_part,
                    document_uri: &document_uri,
                    document_xml: &document_xml,
                },
                kind,
                &parsed.ref_type,
                parsed.section,
            )?;
            created_part = ensured.created_part;
            created_ref = ensured.created_ref;
            document_override = Some(ensured.document_xml);
            if let (Some(part), Some(xml)) = (ensured.rels_part, ensured.rels_xml) {
                rels_override = Some((part, xml));
            }
            content_types_override = ensured.content_types_xml;
            part_xml_override = ensured.part_xml;
            ensured.reference
        }
    } else if !options.id.is_empty() {
        let (_document_uri, sections) = docx_header_footer_listing(file)?;
        resolve_docx_header_footer_selector(
            &sections,
            kind,
            &DocxHeaderFooterSelector {
                kind: kind.to_string(),
                id: options.id.to_string(),
                ref_type: options.ref_type.to_string(),
                section: options.section,
                ..DocxHeaderFooterSelector::default()
            },
        )
        .ok_or_else(|| CliError::target_not_found(format!("{kind} not found: id:{}", options.id)))?
    } else {
        let ensured = ensure_docx_header_footer(
            DocxHeaderFooterEnsureContext {
                file,
                entries: &entries,
                document_part: &document_part,
                document_uri: &document_uri,
                document_xml: &document_xml,
            },
            kind,
            options.ref_type,
            options.section,
        )?;
        created_part = ensured.created_part;
        created_ref = ensured.created_ref;
        document_override = Some(ensured.document_xml);
        if let (Some(part), Some(xml)) = (ensured.rels_part, ensured.rels_xml) {
            rels_override = Some((part, xml));
        }
        content_types_override = ensured.content_types_xml;
        part_xml_override = ensured.part_xml;
        ensured.reference
    };

    if reference.part_uri.is_empty() {
        return Err(CliError::invalid_args(format!(
            "{kind} reference {:?} does not resolve to a part",
            reference.id
        )));
    }

    let part_name = reference.part_uri.trim_start_matches('/').to_string();
    let part_xml = if let Some(xml) = part_xml_override {
        xml
    } else {
        zip_text(file, &part_name).map_err(|_| {
            CliError::target_not_found(format!("{kind} part not found: {}", reference.part_uri))
        })?
    };
    let mutation = set_docx_header_footer_text_xml(
        &part_xml,
        &reference.part_uri,
        options.index,
        options.text,
    )?;

    let mut overrides = BTreeMap::new();
    if let Some(xml) = document_override.filter(|xml| xml != &document_xml) {
        overrides.insert(document_part.clone(), xml);
    }
    if let Some((part, xml)) = rels_override {
        overrides.insert(part, xml);
    }
    if let Some(xml) = content_types_override {
        overrides.insert("[Content_Types].xml".to_string(), xml);
    }
    overrides.insert(part_name, mutation.xml);

    let output_path = docx_mutation_output_path_for_result(
        file,
        &DocxParagraphMutationOptions {
            text: None,
            text_file: None,
            style: "",
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            in_place: options.in_place,
            no_validate: options.no_validate,
        },
    );
    write_docx_package_mutation_output(
        file,
        &overrides,
        DocxParagraphMutationOptions {
            text: None,
            text_file: None,
            style: "",
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            in_place: options.in_place,
            no_validate: options.no_validate,
        },
    )?;

    let paragraph_primary =
        docx_header_footer_paragraph_primary_selector(&reference.primary_selector, mutation.index);
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output_path.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("kind".to_string(), json!(reference.kind));
    result.insert("partUri".to_string(), json!(reference.part_uri));
    result.insert("id".to_string(), json!(reference.id));
    result.insert("type".to_string(), json!(reference.ref_type));
    result.insert("section".to_string(), json!(reference.section));
    result.insert(
        "primarySelector".to_string(),
        json!(reference.primary_selector),
    );
    result.insert("selectors".to_string(), json!(reference.selectors));
    result.insert("paragraphIndex".to_string(), json!(mutation.index));
    result.insert(
        "paragraphPrimarySelector".to_string(),
        json!(paragraph_primary),
    );
    result.insert(
        "paragraphSelectors".to_string(),
        json!(docx_header_footer_paragraph_selectors(
            &reference.selectors,
            mutation.index
        )),
    );
    result.insert("previousText".to_string(), json!(mutation.previous_text));
    result.insert("text".to_string(), json!(options.text));
    result.insert("createdPart".to_string(), json!(created_part));
    result.insert("createdRef".to_string(), json!(created_ref));
    add_docx_header_footer_readback_commands(
        &mut result,
        output_path.as_deref(),
        &reference.kind,
        &reference.primary_selector,
    );
    Ok(Value::Object(result))
}

fn ensure_docx_header_footer(
    ctx: DocxHeaderFooterEnsureContext<'_>,
    kind: &str,
    ref_type: &str,
    section_index: i64,
) -> CliResult<DocxHeaderFooterEnsureResult> {
    if let Some(section) = select_docx_section_range(ctx.document_xml, section_index)?
        && let Some(id) = docx_header_footer_reference_id(
            &ctx.document_xml[section.start..section.end],
            kind,
            ref_type,
        )
    {
        let rels = relationship_entries(ctx.file, &relationships_part_for(ctx.document_part))
            .unwrap_or_default();
        let part_uri = rels
            .iter()
            .find(|rel| rel.id == id)
            .map(|rel| resolve_relationship_target(ctx.document_uri, &rel.target))
            .unwrap_or_default();
        return Ok(DocxHeaderFooterEnsureResult {
            document_xml: ctx.document_xml.to_string(),
            rels_part: None,
            rels_xml: None,
            content_types_xml: None,
            part_xml: None,
            reference: docx_header_footer_ref_info_from_parts(
                kind,
                &id,
                ref_type,
                section.index,
                &part_uri,
            ),
            created_part: false,
            created_ref: false,
        });
    }

    let mut working = ctx.document_xml.to_string();
    if docx_body_prefix(&docx_body_tag(&working)?).is_empty() {
        working = ensure_docx_word_prefix(&working)?;
    }
    working = ensure_docx_relationship_namespace(&working)?;
    let (mut working, section) = select_or_create_docx_section_range(working, section_index)?;

    let rels_part = relationships_part_for(ctx.document_part);
    let rels = relationship_entries(ctx.file, &rels_part).unwrap_or_default();
    let referenced = docx_referenced_header_footer_rel_ids(&working);
    let mut created_part = false;
    let mut rels_xml = None::<String>;
    let mut content_types_xml = None::<String>;
    let mut part_xml = None::<String>;
    let (id, part_uri) = if let Some((id, part_uri)) =
        unreferenced_docx_header_footer_part(&rels, &referenced, ctx.document_uri, kind)
    {
        (id, part_uri)
    } else {
        let part_uri = allocate_docx_header_footer_part_uri(ctx.entries, kind);
        let id = allocate_relationship_id(&rels);
        let target = relationship_target_from_source_to_target(ctx.document_uri, &part_uri);
        let rel_xml = add_relationship_to_xml(
            zip_text(ctx.file, &rels_part).unwrap_or_else(|_| {
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#
                    .to_string()
            }),
            &id,
            docx_header_footer_relationship_type(kind),
            &target,
        );
        let content_xml = ensure_content_type_override(
            zip_text(ctx.file, "[Content_Types].xml")?,
            &part_uri,
            docx_header_footer_content_type(kind),
        );
        created_part = true;
        rels_xml = Some(rel_xml);
        content_types_xml = Some(content_xml);
        part_xml = Some(docx_header_footer_template(kind));
        (id, part_uri)
    };

    working = insert_docx_header_footer_reference(&working, section, kind, ref_type, &id)?;

    Ok(DocxHeaderFooterEnsureResult {
        document_xml: working,
        rels_part: rels_xml.as_ref().map(|_| rels_part),
        rels_xml,
        content_types_xml,
        part_xml,
        reference: docx_header_footer_ref_info_from_parts(
            kind,
            &id,
            ref_type,
            section.index,
            &part_uri,
        ),
        created_part,
        created_ref: true,
    })
}

fn select_docx_section_range(xml: &str, section_index: i64) -> CliResult<Option<DocxSectionRange>> {
    let sections = docx_section_ranges(xml)?;
    if sections.is_empty() {
        return Ok(None);
    }
    let selected = if section_index <= 0 {
        *sections.last().expect("nonempty sections")
    } else {
        *sections.get(section_index as usize - 1).ok_or_else(|| {
            CliError::unexpected(format!(
                "failed to mutate header: section {section_index} out of range (document has {} sections)",
                sections.len()
            ))
        })?
    };
    Ok(Some(selected))
}

fn select_or_create_docx_section_range(
    mut xml: String,
    section_index: i64,
) -> CliResult<(String, DocxSectionRange)> {
    if let Some(section) = select_docx_section_range(&xml, section_index)? {
        return Ok((xml, section));
    }
    let body_tag = docx_body_tag(&xml)?;
    let prefix = docx_body_prefix(&body_tag);
    let body_close = xml
        .rfind(&format!("</{body_tag}>"))
        .ok_or_else(|| CliError::unexpected("document body element not found"))?;
    let sect_pr = format!("<{}/>", word_xml_tag(&prefix, "sectPr"));
    xml.insert_str(body_close, &sect_pr);
    Ok((
        xml,
        DocxSectionRange {
            index: 1,
            start: body_close,
            end: body_close + sect_pr.len(),
        },
    ))
}

fn docx_section_ranges(xml: &str) -> CliResult<Vec<DocxSectionRange>> {
    let body_tag = docx_body_tag(xml)?;
    let (content_start, content_end) = docx_body_content_bounds(xml, &body_tag)?;
    let mut sections = Vec::new();
    for child in xml_direct_child_ranges(xml, content_start, content_end)? {
        if child.kind == "sectPr" {
            sections.push(DocxSectionRange {
                index: sections.len() as i64 + 1,
                start: child.start,
                end: child.end,
            });
            continue;
        }
        if child.kind != "p" {
            continue;
        }
        let Some(p_pr) = direct_child_range_by_kind(xml, child, "pPr")? else {
            continue;
        };
        let Some(sect_pr) = direct_child_range_by_kind(xml, p_pr, "sectPr")? else {
            continue;
        };
        sections.push(DocxSectionRange {
            index: sections.len() as i64 + 1,
            start: sect_pr.start,
            end: sect_pr.end,
        });
    }
    Ok(sections)
}

fn direct_child_range_by_kind(
    xml: &str,
    range: XmlNamedRange,
    wanted: &str,
) -> CliResult<Option<XmlNamedRange>> {
    let fragment = &xml[range.start..range.end];
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(None);
    }
    Ok(
        xml_direct_child_ranges(xml, range.start + open_end + 1, range.start + close_start)?
            .into_iter()
            .find(|child| child.kind == wanted),
    )
}

fn docx_header_footer_reference_id(fragment: &str, kind: &str, ref_type: &str) -> Option<String> {
    let wanted = format!("{kind}Reference");
    let mut reader = NsReader::from_str(fragment);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == wanted =>
            {
                let actual_type =
                    normalize_docx_header_footer_type(attr(&e, "type").unwrap_or_default());
                if actual_type == ref_type {
                    return attr(&e, "id");
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn docx_referenced_header_footer_rel_ids(xml: &str) -> BTreeSet<String> {
    let mut reader = NsReader::from_str(xml);
    let mut ids = BTreeSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if matches!(
                    local_name(e.name().as_ref()),
                    "headerReference" | "footerReference"
                ) =>
            {
                if let Some(id) = attr(&e, "id") {
                    ids.insert(id);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    ids
}

fn unreferenced_docx_header_footer_part(
    rels: &[RelationshipEntry],
    referenced: &BTreeSet<String>,
    document_uri: &str,
    kind: &str,
) -> Option<(String, String)> {
    let rel_type = docx_header_footer_relationship_type(kind);
    rels.iter()
        .find(|rel| {
            rel.rel_type == rel_type
                && rel.target_mode != "External"
                && !referenced.contains(&rel.id)
        })
        .map(|rel| {
            (
                rel.id.clone(),
                resolve_relationship_target(document_uri, &rel.target),
            )
        })
}

fn allocate_docx_header_footer_part_uri(entries: &[String], kind: &str) -> String {
    let prefix = format!("word/{kind}");
    let mut used = BTreeSet::new();
    for entry in entries {
        let normalized = entry.trim_start_matches('/');
        if !normalized.starts_with(&prefix) || !normalized.ends_with(".xml") {
            continue;
        }
        let number = normalized
            .trim_start_matches(&prefix)
            .trim_end_matches(".xml")
            .parse::<u32>();
        if let Ok(number) = number {
            used.insert(number);
        }
    }
    let mut next = 1;
    while used.contains(&next) {
        next += 1;
    }
    format!("/word/{kind}{next}.xml")
}

fn insert_docx_header_footer_reference(
    xml: &str,
    section: DocxSectionRange,
    kind: &str,
    ref_type: &str,
    id: &str,
) -> CliResult<String> {
    let fragment = &xml[section.start..section.end];
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let prefix = xml_tag_prefix(&tag_name);
    let ref_tag = word_xml_tag(&prefix, &format!("{kind}Reference"));
    let type_attr = if prefix.is_empty() {
        "w:type".to_string()
    } else {
        format!("{prefix}:type")
    };
    let reference = format!(
        r#"<{ref_tag} {type_attr}="{}" r:id="{}"/>"#,
        xml_attr_escape(ref_type),
        xml_attr_escape(id)
    );
    let mut updated = xml_open_tag_from_start(&fragment[..=open_end]);
    if self_closing {
        updated.push_str(&reference);
    } else {
        let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
        let insert_at = children
            .iter()
            .find(|child| child.kind != "headerReference" && child.kind != "footerReference")
            .map(|child| child.start)
            .unwrap_or(close_start);
        updated.push_str(&fragment[open_end + 1..insert_at]);
        updated.push_str(&reference);
        updated.push_str(&fragment[insert_at..close_start]);
    }
    updated.push_str("</");
    updated.push_str(&tag_name);
    updated.push('>');

    let mut out = String::with_capacity(xml.len() + updated.len());
    out.push_str(&xml[..section.start]);
    out.push_str(&updated);
    out.push_str(&xml[section.end..]);
    Ok(out)
}

struct DocxHeaderFooterTextMutation {
    xml: String,
    index: i64,
    previous_text: String,
}

fn set_docx_header_footer_text_xml(
    xml: &str,
    part_uri: &str,
    index: i64,
    text: &str,
) -> CliResult<DocxHeaderFooterTextMutation> {
    let root_tag = docx_header_footer_root_tag(xml, part_uri)?;
    let root_start = xml.find(&format!("<{root_tag}")).ok_or_else(|| {
        CliError::unexpected(format!("part {part_uri} is not a header or footer"))
    })?;
    let root_open_end = xml[root_start..]
        .find('>')
        .map(|offset| root_start + offset)
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let root_self_closing = xml[root_start..=root_open_end].trim_end().ends_with("/>");
    let root_close_start = if root_self_closing {
        root_open_end + 1
    } else {
        xml.rfind(&format!("</{root_tag}>"))
            .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?
    };
    let paragraphs: Vec<XmlNamedRange> = if root_self_closing {
        Vec::new()
    } else {
        xml_direct_child_ranges(xml, root_open_end + 1, root_close_start)?
            .into_iter()
            .filter(|child| child.kind == "p")
            .collect()
    };
    let paragraph = paragraphs.get(index as usize - 1).ok_or_else(|| {
        CliError::target_not_found(format!("target not found: header/footer paragraph {index}"))
    })?;
    let fragment = &xml[paragraph.start..paragraph.end];
    let previous_text = docx_paragraph_fragment_text(fragment);
    let updated_paragraph = replace_docx_header_footer_paragraph_fragment(fragment, text)?;
    let mut out = String::with_capacity(xml.len() + updated_paragraph.len());
    out.push_str(&xml[..paragraph.start]);
    out.push_str(&updated_paragraph);
    out.push_str(&xml[paragraph.end..]);
    Ok(DocxHeaderFooterTextMutation {
        xml: out,
        index,
        previous_text,
    })
}

fn replace_docx_header_footer_paragraph_fragment(fragment: &str, text: &str) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let start_tag = &fragment[..=open_end];
    let prefix = xml_tag_prefix(&tag_name);
    let mut paragraph_properties = String::new();
    let mut run_properties = String::new();
    if !self_closing {
        for child in xml_direct_child_ranges(fragment, open_end + 1, close_start)? {
            match child.kind.as_str() {
                "pPr" if paragraph_properties.is_empty() => {
                    paragraph_properties.push_str(&fragment[child.start..child.end]);
                }
                "r" if run_properties.is_empty() => {
                    if let Some(r_pr) =
                        first_direct_xml_child_by_kind(&fragment[child.start..child.end], "rPr")?
                    {
                        run_properties.push_str(&r_pr);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = xml_open_tag_from_start(start_tag);
    out.push_str(&paragraph_properties);
    let r = word_xml_tag(&prefix, "r");
    out.push('<');
    out.push_str(&r);
    out.push('>');
    out.push_str(&run_properties);
    append_docx_text_children(&mut out, &prefix, text);
    out.push_str("</");
    out.push_str(&r);
    out.push('>');
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok(out)
}

fn docx_paragraph_fragment_text(fragment: &str) -> String {
    let mut reader = NsReader::from_str(fragment);
    let mut text = String::new();
    let mut in_t = false;
    let mut skip_text_depth = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_t = true;
                }
                if matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if skip_text_depth == 0 {
                    match name.as_str() {
                        "tab" => text.push('\t'),
                        "br" | "cr" => text.push('\n'),
                        "noBreakHyphen" => text.push('-'),
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(e)) if in_t && skip_text_depth == 0 => {
                text.push_str(&decode_xml_text(e.as_ref()));
            }
            Ok(Event::GeneralRef(e)) if in_t && skip_text_depth == 0 => {
                text.push_str(&xml_general_ref(e.as_ref()));
            }
            Ok(Event::CData(e)) if in_t && skip_text_depth == 0 => {
                text.push_str(&String::from_utf8_lossy(e.as_ref()));
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_t = false;
                } else if matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth = skip_text_depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    text
}

fn docx_header_footer_root_tag(xml: &str, part_uri: &str) -> CliResult<String> {
    let mut reader = NsReader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if element_in_ns(reader.resolver(), &e, DOCX_W_NS)
                    && matches!(name.as_str(), "hdr" | "ftr")
                {
                    return Ok(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
                return Err(CliError::unexpected(format!(
                    "part {part_uri} is not a header or footer"
                )));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::unexpected(format!(
        "part {part_uri} is not a header or footer"
    )))
}

fn write_docx_package_mutation_output(
    file: &str,
    overrides: &BTreeMap<String, String>,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<()> {
    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        docx_mutation_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_overrides(file, &readback_path, overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

fn docx_header_footer_ref_info_from_parts(
    kind: &str,
    id: &str,
    ref_type: &str,
    section: i64,
    part_uri: &str,
) -> DocxHeaderFooterRefInfo {
    let primary_selector = format!("{kind}:{section}:{ref_type}");
    let mut selectors = vec![primary_selector.clone()];
    if !id.is_empty() {
        selectors.push(format!("id:{id}"));
        selectors.push(id.to_string());
    }
    if !part_uri.is_empty() {
        selectors.push(format!("part:{part_uri}"));
        selectors.push(part_uri.to_string());
    }
    DocxHeaderFooterRefInfo {
        kind: kind.to_string(),
        id: id.to_string(),
        ref_type: ref_type.to_string(),
        section,
        primary_selector,
        selectors,
        part_uri: part_uri.to_string(),
    }
}

fn docx_header_footer_paragraph_primary_selector(selector: &str, index: i64) -> String {
    if selector.is_empty() {
        String::new()
    } else {
        format!("{selector}/p:{index}")
    }
}

fn docx_header_footer_paragraph_selectors(selectors: &[String], index: i64) -> Vec<String> {
    let mut out = Vec::with_capacity(selectors.len() * 2);
    for selector in selectors {
        out.push(format!("{selector}/p:{index}"));
        out.push(format!("{selector}/paragraph:{index}"));
    }
    out
}

fn add_docx_header_footer_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    kind: &str,
    selector: &str,
) {
    let target = output_path.unwrap_or("<out.docx>");
    let validate = format!("ooxml validate --strict {target}");
    let show = docx_header_footer_show_command(target, kind, selector);
    let list = docx_header_footer_list_command(target, kind);
    if output_path.is_some() {
        result.insert("validateCommand".to_string(), json!(validate));
        result.insert("showCommand".to_string(), json!(show));
        result.insert("listCommand".to_string(), json!(list));
    } else {
        result.insert("validateCommandTemplate".to_string(), json!(validate));
        result.insert("showCommandTemplate".to_string(), json!(show));
        result.insert("listCommandTemplate".to_string(), json!(list));
    }
}

fn docx_header_footer_show_command(file: &str, kind: &str, selector: &str) -> String {
    let group = if kind == "footer" {
        "footers"
    } else {
        "headers"
    };
    let mut command = format!("ooxml --json docx {group} show {}", command_arg(file));
    if !selector.trim().is_empty() {
        command.push_str(" --selector ");
        command.push_str(&command_arg(selector));
    }
    command
}

fn docx_header_footer_list_command(file: &str, kind: &str) -> String {
    let group = if kind == "footer" {
        "footers"
    } else {
        "headers"
    };
    format!("ooxml --json docx {group} list {}", command_arg(file))
}

fn ensure_docx_relationship_namespace(xml: &str) -> CliResult<String> {
    if xml
        .contains("xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"")
    {
        return Ok(xml.to_string());
    }
    let document_start = xml
        .find("<w:document")
        .or_else(|| xml.find("<document"))
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let start_end = xml[document_start..]
        .find('>')
        .map(|offset| document_start + offset)
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let mut out = String::with_capacity(xml.len() + 88);
    out.push_str(&xml[..start_end]);
    out.push_str(
        " xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"",
    );
    out.push_str(&xml[start_end..]);
    Ok(out)
}

fn docx_header_footer_template(kind: &str) -> String {
    let tag = if kind == "footer" { "w:ftr" } else { "w:hdr" };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><{tag} xmlns:w="{}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:p/></{tag}>"#,
        String::from_utf8_lossy(DOCX_W_NS)
    )
}

fn docx_header_footer_content_type(kind: &str) -> &'static str {
    if kind == "footer" {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"
    } else {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"
    }
}

fn docx_header_footer_relationship_type(kind: &str) -> &'static str {
    if kind == "footer" {
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer"
    } else {
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header"
    }
}

fn docx_tables_show(file: &str, table: usize, include_details: bool) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let xml = zip_text(file, &document_part).map_err(|_| {
        CliError::unexpected(format!(
            "failed to read main document: part {document_uri} not found"
        ))
    })?;
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        if err.message == "invalid DOCX XML"
            || err.message.starts_with("failed to extract DOCX blocks:")
        {
            CliError::unexpected(format!(
                "failed to read main document: failed to read document part {document_uri}: failed to parse XML part {document_uri}: etree: invalid XML format"
            ))
        } else {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        }
    })?;

    let mut table_number = 0usize;
    let mut tables = Vec::new();
    for report in reports.into_iter().filter(|report| report.kind == "table") {
        table_number += 1;
        if table > 0 && table_number != table {
            continue;
        }
        tables.push(docx_table_summary_json(
            file,
            table_number,
            report,
            include_details,
        ));
    }
    if table > 0 && tables.is_empty() {
        return Err(CliError::target_not_found(format!(
            "target not found: table {table}"
        )));
    }

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert(
        "tables".to_string(),
        if tables.is_empty() {
            Value::Null
        } else {
            Value::Array(tables)
        },
    );
    Ok(Value::Object(result))
}

fn docx_tables_set_cell(
    file: &str,
    table: usize,
    row: usize,
    col: usize,
    expected_hash: &str,
    text: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let mutation = docx_table_cell_text_mutation(file, table, row, col, expected_hash, text)?;
    let output_path = docx_mutation_output_path_for_result(file, &options);
    write_docx_mutation_output(file, &mutation.document_part, &mutation.xml, options)?;

    let mut result = docx_table_cell_mutation_result(file, table, row, col, &mutation, output_path);
    result.insert("text".to_string(), json!(text));
    Ok(Value::Object(result))
}

fn docx_tables_clear_cell(
    file: &str,
    table: usize,
    row: usize,
    col: usize,
    expected_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let mutation = docx_table_cell_text_mutation(file, table, row, col, expected_hash, "")?;
    let output_path = docx_mutation_output_path_for_result(file, &options);
    write_docx_mutation_output(file, &mutation.document_part, &mutation.xml, options)?;

    Ok(Value::Object(docx_table_cell_mutation_result(
        file,
        table,
        row,
        col,
        &mutation,
        output_path,
    )))
}

struct DocxTableCellMutation {
    document_part: String,
    xml: String,
    block: usize,
    content_hash: String,
    previous_hash: String,
    previous_text: String,
    flattened: bool,
}

fn docx_table_cell_text_mutation(
    file: &str,
    table: usize,
    row: usize,
    col: usize,
    expected_hash: &str,
    text: &str,
) -> CliResult<DocxTableCellMutation> {
    let entries = zip_entry_names(file)?;
    ensure_docx_package_kind(file, &entries)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;

    let mut table_seen = 0usize;
    let mut selected_block = 0usize;
    let mut previous_hash = String::new();
    let mut previous_text = String::new();
    for report in reports.iter().filter(|report| report.kind == "table") {
        table_seen += 1;
        if table_seen != table {
            continue;
        }
        selected_block = report.index;
        previous_hash = report.content_hash.clone();
        if previous_hash != expected_hash {
            return Err(CliError::invalid_args(format!(
                "block hash mismatch: block {selected_block} expected {expected_hash} but found {previous_hash}"
            )));
        }
        previous_text = report
            .table_rows
            .get(row - 1)
            .and_then(|cells| cells.get(col - 1))
            .cloned()
            .ok_or_else(|| {
                CliError::target_not_found(format!(
                    "target not found: table {table} cell R{row}C{col}"
                ))
            })?;
        break;
    }
    if selected_block == 0 {
        return Err(CliError::target_not_found(format!(
            "target not found: table {table}"
        )));
    }

    let body_tag = docx_body_tag(&xml)?;
    let ranges = docx_body_block_ranges(&xml, &body_tag)?;
    let table_range = ranges
        .get(selected_block - 1)
        .filter(|range| range.kind == "tbl")
        .ok_or_else(|| CliError::unexpected("selected table block readback missing"))?;
    let table_fragment =
        ensure_docx_table_scaffold_fragment(&xml[table_range.start..table_range.end])?;
    let (updated_table, flattened) =
        set_docx_table_cell_text_fragment(&table_fragment, row, col, text)?;

    let mut updated_xml = String::with_capacity(xml.len() + updated_table.len());
    updated_xml.push_str(&xml[..table_range.start]);
    updated_xml.push_str(&updated_table);
    updated_xml.push_str(&xml[table_range.end..]);

    let updated_report = docx_rich_block_reports(&updated_xml, false)
        .map_err(|err| {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        })?
        .into_iter()
        .find(|report| report.index == selected_block && report.kind == "table")
        .ok_or_else(|| CliError::unexpected("updated table readback missing"))?;

    Ok(DocxTableCellMutation {
        document_part,
        xml: updated_xml,
        block: selected_block,
        content_hash: updated_report.content_hash,
        previous_hash,
        previous_text,
        flattened,
    })
}

fn docx_table_cell_mutation_result(
    file: &str,
    table: usize,
    row: usize,
    col: usize,
    mutation: &DocxTableCellMutation,
    output_path: Option<String>,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("table".to_string(), json!(table));
    result.insert("block".to_string(), json!(mutation.block));
    result.insert("row".to_string(), json!(row));
    result.insert("col".to_string(), json!(col));
    result.insert("contentHash".to_string(), json!(mutation.content_hash));
    result.insert("previousHash".to_string(), json!(mutation.previous_hash));
    result.insert("previousText".to_string(), json!(mutation.previous_text));
    result.insert("flattened".to_string(), json!(mutation.flattened));
    if let Some(output) = output_path.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    add_docx_table_readback_commands(&mut result, output_path.as_deref(), table);
    result
}

fn docx_mutation_output_path_for_result(
    file: &str,
    options: &DocxParagraphMutationOptions<'_>,
) -> Option<String> {
    if options.dry_run {
        None
    } else if options.in_place {
        Some(file.to_string())
    } else {
        options
            .out
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    }
}

fn add_docx_table_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    table: usize,
) {
    let target = output_path.unwrap_or("<out.pptx>");
    let validate = format!("ooxml validate --strict {target}");
    let show = format!(
        "ooxml --json docx tables show {} --table {}",
        command_arg(target),
        table
    );
    let list = format!("ooxml --json docx tables show {}", command_arg(target));
    if output_path.is_some() {
        result.insert("validateCommand".to_string(), json!(validate));
        result.insert("tablesShowCommand".to_string(), json!(show));
        result.insert("tablesListCommand".to_string(), json!(list));
    } else {
        result.insert("validateCommandTemplate".to_string(), json!(validate));
        result.insert("tablesShowCommandTemplate".to_string(), json!(show));
        result.insert("tablesListCommandTemplate".to_string(), json!(list));
    }
}

fn set_docx_table_cell_text_fragment(
    table_fragment: &str,
    row: usize,
    col: usize,
    text: &str,
) -> CliResult<(String, bool)> {
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(table_fragment)?;
    if self_closing {
        return Err(CliError::target_not_found(format!(
            "target not found: table cell R{row}C{col}"
        )));
    }
    let rows: Vec<XmlNamedRange> =
        xml_direct_child_ranges(table_fragment, open_end + 1, close_start)?
            .into_iter()
            .filter(|child| child.kind == "tr")
            .collect();
    let row_range = rows.get(row - 1).ok_or_else(|| {
        CliError::target_not_found(format!("target not found: table cell R{row}C{col}"))
    })?;
    let row_fragment = &table_fragment[row_range.start..row_range.end];
    let (row_open_end, _row_tag_name, row_close_start, row_self_closing) =
        xml_fragment_bounds(row_fragment)?;
    if row_self_closing {
        return Err(CliError::target_not_found(format!(
            "target not found: table cell R{row}C{col}"
        )));
    }
    let cells: Vec<XmlNamedRange> =
        xml_direct_child_ranges(row_fragment, row_open_end + 1, row_close_start)?
            .into_iter()
            .filter(|child| child.kind == "tc")
            .collect();
    let cell_range = cells.get(col - 1).ok_or_else(|| {
        CliError::target_not_found(format!("target not found: table cell R{row}C{col}"))
    })?;
    let cell_fragment = &row_fragment[cell_range.start..cell_range.end];
    let (updated_cell, flattened) = set_docx_table_cell_fragment(cell_fragment, text)?;

    let mut updated_row = String::with_capacity(row_fragment.len() + updated_cell.len());
    updated_row.push_str(&row_fragment[..cell_range.start]);
    updated_row.push_str(&updated_cell);
    updated_row.push_str(&row_fragment[cell_range.end..]);

    let mut updated_table = String::with_capacity(table_fragment.len() + updated_row.len());
    updated_table.push_str(&table_fragment[..row_range.start]);
    updated_table.push_str(&updated_row);
    updated_table.push_str(&table_fragment[row_range.end..]);
    Ok((updated_table, flattened))
}

fn set_docx_table_cell_fragment(cell_fragment: &str, text: &str) -> CliResult<(String, bool)> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(cell_fragment)?;
    let start_tag = &cell_fragment[..=open_end];
    let prefix = xml_tag_prefix(&tag_name);
    let children = if self_closing {
        Vec::new()
    } else {
        xml_direct_child_ranges(cell_fragment, open_end + 1, close_start)?
    };
    let paragraphs: Vec<&XmlNamedRange> =
        children.iter().filter(|child| child.kind == "p").collect();
    let mut flattened = paragraphs.len() > 1;
    for child in &children {
        if child.kind != "tcPr" && (child.kind != "p" || paragraphs.len() > 1) {
            flattened = true;
        }
    }

    let mut paragraph_properties = String::new();
    let mut run_properties = String::new();
    if let Some(first_paragraph) = paragraphs.first() {
        let paragraph_fragment = &cell_fragment[first_paragraph.start..first_paragraph.end];
        if let Some(p_pr) = first_direct_xml_child_by_kind(paragraph_fragment, "pPr")? {
            paragraph_properties = p_pr;
        }
        run_properties = first_docx_run_properties_in_paragraph_fragment(paragraph_fragment)?;
    }

    let mut out = xml_open_tag_from_start(start_tag);
    for child in children.iter().filter(|child| child.kind == "tcPr") {
        out.push_str(&cell_fragment[child.start..child.end]);
    }
    out.push_str(&render_docx_cell_paragraph(
        &prefix,
        text,
        &paragraph_properties,
        &run_properties,
    ));
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok((out, flattened))
}

fn first_docx_run_properties_in_paragraph_fragment(fragment: &str) -> CliResult<String> {
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(String::new());
    }
    for child in xml_direct_child_ranges(fragment, open_end + 1, close_start)? {
        if child.kind == "r" {
            return first_direct_xml_child_by_kind(&fragment[child.start..child.end], "rPr")
                .map(|value| value.unwrap_or_default());
        }
    }
    Ok(String::new())
}

fn render_docx_cell_paragraph(
    prefix: &str,
    text: &str,
    paragraph_properties: &str,
    run_properties: &str,
) -> String {
    let p = word_xml_tag(prefix, "p");
    let mut paragraph = String::new();
    paragraph.push('<');
    paragraph.push_str(&p);
    paragraph.push('>');
    paragraph.push_str(paragraph_properties);
    if !text.is_empty() {
        let r = word_xml_tag(prefix, "r");
        paragraph.push('<');
        paragraph.push_str(&r);
        paragraph.push('>');
        paragraph.push_str(run_properties);
        append_docx_text_children(&mut paragraph, prefix, text);
        paragraph.push_str("</");
        paragraph.push_str(&r);
        paragraph.push('>');
    }
    paragraph.push_str("</");
    paragraph.push_str(&p);
    paragraph.push('>');
    paragraph
}

fn resolve_required_docx_table_text(
    text: Option<&str>,
    text_file: Option<&str>,
    text_changed: bool,
    text_file_changed: bool,
) -> CliResult<String> {
    if text_changed == text_file_changed {
        return Err(CliError::invalid_args(
            "must specify exactly one of --text or --text-file",
        ));
    }
    if text_changed {
        return Ok(text.unwrap_or_default().to_string());
    }
    let path = text_file.unwrap_or_default();
    fs::read(path)
        .map(|data| String::from_utf8_lossy(&data).to_string())
        .map_err(|_| CliError::file_not_found(format!("file not found: {path}")))
}

struct DocxParagraphMutationOptions<'a> {
    text: Option<&'a str>,
    text_file: Option<&'a str>,
    style: &'a str,
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

fn docx_paragraphs_append(
    file: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    let text = resolve_optional_docx_paragraph_text(options.text, options.text_file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let block_count = docx_rich_block_reports(&xml, false)
        .map_err(|err| {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        })?
        .len();
    let updated_xml = append_docx_body_paragraph_xml(&xml, &text, options.style)?;

    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        docx_mutation_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_override(file, &readback_path, &document_part, &updated_xml)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(block_count + 1));
    if !options.style.is_empty() {
        result.insert("style".to_string(), json!(options.style));
    }
    result.insert("text".to_string(), json!(text));
    Ok(Value::Object(result))
}

fn docx_paragraphs_insert(
    file: &str,
    insert_after: i64,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    if insert_after < 0 {
        return Err(CliError::invalid_args("--insert-after must be >= 0"));
    }
    let text = resolve_optional_docx_paragraph_text(options.text, options.text_file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let (updated_xml, index) =
        insert_docx_body_paragraph_xml(&xml, insert_after as usize, &text, options.style)?;

    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        docx_mutation_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_override(file, &readback_path, &document_part, &updated_xml)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(index));
    result.insert("insertAfter".to_string(), json!(insert_after));
    if !options.style.is_empty() {
        result.insert("style".to_string(), json!(options.style));
    }
    result.insert("text".to_string(), json!(text));
    Ok(Value::Object(result))
}

fn docx_paragraphs_set(
    file: &str,
    index: i64,
    handle: Option<&str>,
    replacement: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    ensure_docx_package_kind(file, &entries)?;

    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let target_index = if let Some(handle_arg) = handle.filter(|value| !value.is_empty()) {
        resolve_docx_paragraph_handle_index(&xml, handle_arg)?
    } else {
        index as usize
    };
    let mutation = set_or_clear_docx_body_paragraph_xml(&xml, target_index, Some(replacement))?;
    write_docx_mutation_output(file, &document_part, &mutation.xml, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(mutation.index));
    if !mutation.style.is_empty() {
        result.insert("style".to_string(), json!(mutation.style));
    }
    result.insert("text".to_string(), json!(replacement));
    result.insert("previousText".to_string(), json!(mutation.previous_text));
    result.insert("flattened".to_string(), json!(mutation.flattened));
    if !mutation.handle.is_empty() {
        result.insert("handle".to_string(), json!(mutation.handle));
    }
    Ok(Value::Object(result))
}

fn docx_paragraphs_clear(
    file: &str,
    index: i64,
    handle: Option<&str>,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    ensure_docx_package_kind(file, &entries)?;

    let document_part = find_docx_document_part(file, &entries)?;
    let xml = zip_text(file, &document_part)?;
    let target_index = if let Some(handle_arg) = handle.filter(|value| !value.is_empty()) {
        resolve_docx_paragraph_handle_index(&xml, handle_arg)?
    } else {
        index as usize
    };
    let mutation = set_or_clear_docx_body_paragraph_xml(&xml, target_index, None)?;
    write_docx_mutation_output(file, &document_part, &mutation.xml, options)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("index".to_string(), json!(mutation.index));
    if !mutation.style.is_empty() {
        result.insert("style".to_string(), json!(mutation.style));
    }
    result.insert("previousText".to_string(), json!(mutation.previous_text));
    if !mutation.handle.is_empty() {
        result.insert("handle".to_string(), json!(mutation.handle));
    }
    Ok(Value::Object(result))
}

fn resolve_optional_docx_paragraph_text(
    text: Option<&str>,
    text_file: Option<&str>,
) -> CliResult<String> {
    match (text, text_file) {
        (Some(_), Some(_)) => Err(CliError::invalid_args(
            "cannot specify both --text and --text-file",
        )),
        (Some(value), None) => Ok(value.to_string()),
        (None, Some(path)) => fs::read(path)
            .map(|data| String::from_utf8_lossy(&data).to_string())
            .map_err(|_| CliError::file_not_found(format!("file not found: {path}"))),
        (None, None) => Ok(String::new()),
    }
}

fn resolve_required_docx_paragraph_set_text(
    text: Option<&str>,
    text_file: Option<&str>,
    text_changed: bool,
    text_file_changed: bool,
) -> CliResult<String> {
    if text_changed == text_file_changed {
        return Err(CliError::invalid_args(
            "must specify exactly one of --text or --text-file",
        ));
    }
    if text_changed {
        let value = text.unwrap_or_default();
        if value.is_empty() {
            return Err(CliError::invalid_args(
                "--text cannot be empty; use docx paragraphs clear",
            ));
        }
        return Ok(value.to_string());
    }
    let path = text_file.unwrap_or_default();
    let data =
        fs::read(path).map_err(|_| CliError::file_not_found(format!("file not found: {path}")))?;
    if data.is_empty() {
        return Err(CliError::invalid_args(
            "--text-file cannot be empty; use docx paragraphs clear",
        ));
    }
    Ok(String::from_utf8_lossy(&data).to_string())
}

fn ensure_docx_package_kind(file: &str, entries: &[String]) -> CliResult<()> {
    let package_kind = detect_inspect_package_type(file, entries);
    if package_kind == InspectPackageKind::Docx {
        return Ok(());
    }
    let detected = match package_kind {
        InspectPackageKind::Pptx => "pptx",
        InspectPackageKind::Xlsx => "xlsx",
        InspectPackageKind::Docx => "docx",
        InspectPackageKind::Unknown => package_type(file)?,
    };
    Err(CliError::unsupported_type(format!(
        "file is not a DOCX document (detected: {detected})"
    )))
}

fn write_docx_mutation_output(
    file: &str,
    document_part: &str,
    updated_xml: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<()> {
    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        docx_mutation_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_override(file, &readback_path, document_part, updated_xml)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

struct DocxParagraphXmlMutation {
    xml: String,
    index: usize,
    style: String,
    previous_text: String,
    flattened: bool,
    handle: String,
}

fn set_or_clear_docx_body_paragraph_xml(
    xml: &str,
    target_index: usize,
    replacement: Option<&str>,
) -> CliResult<DocxParagraphXmlMutation> {
    if target_index == 0 {
        return Err(CliError::target_not_found(
            "target not found: paragraph index 0",
        ));
    }
    let reports = docx_rich_block_reports(xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let report = reports.get(target_index - 1).ok_or_else(|| {
        CliError::target_not_found(format!("target not found: paragraph index {target_index}"))
    })?;
    if report.kind != "paragraph" {
        return Err(CliError::invalid_args(format!(
            "block {target_index} is a table, not a paragraph"
        )));
    }

    let mut working = xml.to_string();
    let mut para_id = report.para_id.trim().to_string();
    if para_id.is_empty() {
        working = ensure_docx_w14_namespace(&working)?;
        let existing = docx_all_para_ids(&working)?;
        para_id = mint_docx_para_id(&existing);
    }

    let body_tag = docx_body_tag(&working)?;
    let blocks = docx_body_block_ranges(&working, &body_tag)?;
    let block = blocks.get(target_index - 1).ok_or_else(|| {
        CliError::target_not_found(format!("target not found: paragraph index {target_index}"))
    })?;
    if block.kind != "p" {
        return Err(CliError::invalid_args(format!(
            "block {target_index} is a table, not a paragraph"
        )));
    }
    let fragment = &working[block.start..block.end];
    let (paragraph, flattened) = replace_docx_paragraph_fragment(fragment, &para_id, replacement)?;
    let mut out = String::with_capacity(working.len() + paragraph.len());
    out.push_str(&working[..block.start]);
    out.push_str(&paragraph);
    out.push_str(&working[block.end..]);

    Ok(DocxParagraphXmlMutation {
        xml: out,
        index: target_index,
        style: report.style.clone(),
        previous_text: report.text.clone(),
        flattened,
        handle: format!("H:docx/pt:doc/para:m:{para_id}"),
    })
}

const HANDLE_MALFORMED: &str = "HANDLE_MALFORMED";
const HANDLE_FORMAT_MISMATCH: &str = "HANDLE_FORMAT_MISMATCH";
const HANDLE_SCOPE_STALE: &str = "HANDLE_SCOPE_STALE";
const HANDLE_STALE: &str = "HANDLE_STALE";
const HANDLE_AMBIGUOUS: &str = "HANDLE_AMBIGUOUS";

fn resolve_docx_paragraph_handle_index(xml: &str, handle: &str) -> CliResult<usize> {
    let para_id = parse_docx_paragraph_handle_para_id(handle)?;
    let reports = docx_rich_block_reports(xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to read main document: {}", err.message))
    })?;
    let wanted = para_id.trim().to_ascii_uppercase();
    let matches: Vec<&DocxRichBlockReport> = reports
        .iter()
        .filter(|report| {
            report.kind == "paragraph" && report.para_id.trim().eq_ignore_ascii_case(&wanted)
        })
        .collect();
    match matches.len() {
        0 => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_STALE,
            format!("no paragraph with w14:paraId {para_id:?} in document body"),
            handle,
        )),
        1 => Ok(matches[0].index),
        count => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_AMBIGUOUS,
            format!(
                "w14:paraId {para_id:?} is not unique ({count} paragraphs share it); cannot resolve to a single paragraph"
            ),
            handle,
        )),
    }
}

fn resolve_docx_comment_handle_id(comments_xml: &str, handle: &str) -> CliResult<i64> {
    let comment_id = parse_docx_comment_handle_id(handle)?;
    let spans = docx_comment_element_spans_by_id(comments_xml, comment_id)?;
    match spans.len() {
        0 => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_STALE,
            format!("no comment with w:id {comment_id} in document"),
            handle,
        )),
        1 => Ok(comment_id),
        count => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_AMBIGUOUS,
            format!(
                "w:id {comment_id} is not unique ({count} comments share it); cannot resolve to a single comment"
            ),
            handle,
        )),
    }
}

fn parse_docx_comment_handle_id(handle: &str) -> CliResult<i64> {
    let trimmed = handle.trim();
    let Some(body) = trimmed.strip_prefix("H:") else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "missing handle version prefix \"H:\"",
            handle,
        ));
    };
    let segments: Vec<&str> = body.split('/').collect();
    if segments.len() != 3 {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "handle must be H:docx/<scope>/<class>:<objref>",
            handle,
        ));
    }
    if segments[0].is_empty() {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "empty format tag",
            handle,
        ));
    }
    if segments[0] != "docx" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_FORMAT_MISMATCH,
            format!(
                "handle format tag {:?} does not match package format {:?}",
                segments[0], "docx"
            ),
            handle,
        ));
    }
    let Some((class, objref)) = segments[2].split_once(':') else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("object segment {:?} must be <class>:<objref>", segments[2]),
            handle,
        ));
    };
    if segments[1] != "pt:doc" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!(
                "comment handle scope must be {:?}, got {:?}",
                "pt:doc", segments[1]
            ),
            handle,
        ));
    }
    if class != "comment" {
        return Err(CliError::invalid_args(
            "--handle must be a comment handle (H:docx/pt:doc/comment:n:<id>)",
        ));
    }
    let Some((tag, value)) = objref.split_once(':') else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("comment objref: objref {objref:?} must be n:<id>"),
            handle,
        ));
    };
    if tag != "n" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("comment objref: unsupported objref tag {tag:?} (expected native id \"n\")"),
            handle,
        ));
    }
    if value.is_empty() {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "comment objref: empty native id",
            handle,
        ));
    }
    let id = value.parse::<i64>().map_err(|err| {
        docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("comment objref: invalid native id {value:?}: {err}"),
            handle,
        )
    })?;
    if id < 0 {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("comment objref: native id must be non-negative, got {id}"),
            handle,
        ));
    }
    Ok(id)
}

fn parse_docx_paragraph_handle_para_id(handle: &str) -> CliResult<String> {
    let trimmed = handle.trim();
    let Some(body) = trimmed.strip_prefix("H:") else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "missing handle version prefix \"H:\"",
            handle,
        ));
    };
    let segments: Vec<&str> = body.split('/').collect();
    if segments.len() != 3 {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "handle must be H:docx/<scope>/<class>:<objref>",
            handle,
        ));
    }
    if segments[0].is_empty() {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "empty format tag",
            handle,
        ));
    }
    if segments[0] != "docx" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_FORMAT_MISMATCH,
            format!(
                "handle format tag {:?} does not match package format {:?}",
                segments[0], "docx"
            ),
            handle,
        ));
    }
    let Some((class, objref)) = segments[2].split_once(':') else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("object segment {:?} must be <class>:<objref>", segments[2]),
            handle,
        ));
    };
    if segments[1] != "pt:doc" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!(
                "paragraph handle scope must be {:?}, got {:?}",
                "pt:doc", segments[1]
            ),
            handle,
        ));
    }
    if class != "para" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "expected a paragraph handle",
            handle,
        ));
    }
    let Some((tag, value)) = objref.split_once(':') else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("paragraph objref: objref {objref:?} must be m:<paraId>"),
            handle,
        ));
    };
    if tag != "m" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!(
                "paragraph objref: unsupported objref tag {tag:?} (expected paragraph marker \"m\")"
            ),
            handle,
        ));
    }
    if value.is_empty() {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "paragraph objref: empty paragraph marker",
            handle,
        ));
    }
    Ok(value.to_string())
}

fn parse_docx_style_handle_style_id(handle: &str) -> CliResult<String> {
    let trimmed = handle.trim();
    let Some(body) = trimmed.strip_prefix("H:") else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "missing handle version prefix \"H:\"",
            handle,
        ));
    };
    let segments: Vec<&str> = body.split('/').collect();
    if segments.len() != 3 {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "handle must be H:docx/<scope>/<class>:<objref>",
            handle,
        ));
    }
    if segments[0].is_empty() {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "empty format tag",
            handle,
        ));
    }
    if segments[0] != "docx" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_FORMAT_MISMATCH,
            format!(
                "handle format tag {:?} does not match package format {:?}",
                segments[0], "docx"
            ),
            handle,
        ));
    }
    let Some((class, objref)) = segments[2].split_once(':') else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("object segment {:?} must be <class>:<objref>", segments[2]),
            handle,
        ));
    };
    if segments[1] != "pt:styles" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!(
                "style handle scope must be {:?}, got {:?}",
                "pt:styles", segments[1]
            ),
            handle,
        ));
    }
    if class != "style" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "expected a style handle",
            handle,
        ));
    }
    let Some((tag, value)) = objref.split_once(':') else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("style objref: objref {objref:?} must be n:<value>"),
            handle,
        ));
    };
    if tag != "n" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("style objref: unsupported objref tag {tag:?} (expected native id \"n\")"),
            handle,
        ));
    }
    if value.is_empty() {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "style objref: empty native id",
            handle,
        ));
    }
    Ok(value.to_string())
}

fn docx_handle_error(
    exit_code: i32,
    code: &'static str,
    message: impl Into<String>,
    handle: &str,
) -> CliError {
    CliError {
        code,
        exit_code,
        message: format!("{}: {} (handle {:?})", code, message.into(), handle),
    }
}

fn apply_docx_style_xml(
    xml: &str,
    target: DocxStyleTarget,
    block_index: usize,
    style_id: &str,
    existing_para_id: &str,
) -> CliResult<String> {
    if block_index == 0 {
        return Err(CliError::target_not_found(format!(
            "target not found: {} block 0",
            target.as_str()
        )));
    }
    let mut working = xml.to_string();
    if matches!(target, DocxStyleTarget::Paragraph | DocxStyleTarget::Run)
        && existing_para_id.trim().is_empty()
    {
        working = ensure_docx_w14_namespace(&working)?;
    }
    let body_tag = docx_body_tag(&working)?;
    if !body_tag.contains(':') {
        working = ensure_docx_word_prefix(&working)?;
    }
    let body_tag = docx_body_tag(&working)?;
    let blocks = docx_body_block_ranges(&working, &body_tag)?;
    let block = blocks.get(block_index - 1).ok_or_else(|| {
        CliError::target_not_found(format!(
            "target not found: {} block {block_index}",
            target.as_str()
        ))
    })?;
    let fragment = &working[block.start..block.end];
    let replacement = match target {
        DocxStyleTarget::Paragraph => {
            if block.kind != "p" {
                return Err(CliError::invalid_args(format!(
                    "block {block_index} is a table, not a paragraph"
                )));
            }
            let para_id = docx_style_apply_para_id(&working, existing_para_id)?;
            set_docx_paragraph_style_fragment(fragment, &para_id, style_id)?
        }
        DocxStyleTarget::Run => {
            if block.kind != "p" {
                return Err(CliError::invalid_args(format!(
                    "block {block_index} is a table, not a paragraph"
                )));
            }
            let para_id = docx_style_apply_para_id(&working, existing_para_id)?;
            set_docx_run_style_for_paragraph_fragment(fragment, &para_id, style_id)?
        }
        DocxStyleTarget::Table => {
            if block.kind != "tbl" {
                return Err(CliError::invalid_args(format!(
                    "block {block_index} is a paragraph, not a table"
                )));
            }
            set_docx_table_style_fragment(fragment, style_id)?
        }
    };
    let mut out = String::with_capacity(working.len() + replacement.len());
    out.push_str(&working[..block.start]);
    out.push_str(&replacement);
    out.push_str(&working[block.end..]);
    if matches!(target, DocxStyleTarget::Paragraph | DocxStyleTarget::Run) {
        ensure_docx_body_table_scaffolds_xml(&out)
    } else {
        Ok(out)
    }
}

fn docx_style_apply_para_id(xml: &str, existing_para_id: &str) -> CliResult<String> {
    if !existing_para_id.trim().is_empty() {
        return Ok(existing_para_id.trim().to_string());
    }
    let existing = docx_all_para_ids(xml)?;
    Ok(mint_docx_para_id(&existing))
}

fn set_docx_paragraph_style_fragment(
    fragment: &str,
    para_id: &str,
    style_id: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let start_tag = &fragment[..=open_end];
    let prefix = xml_tag_prefix(&tag_name);
    let open_tag = docx_open_tag_with_para_id(start_tag, para_id);
    let props = render_docx_style_props(&prefix, "pPr", "pStyle", style_id);
    if self_closing {
        return Ok(format!("{open_tag}{props}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    if let Some(child) = children.into_iter().find(|child| child.kind == "pPr") {
        let updated_props =
            set_docx_style_child_in_props(&fragment[child.start..child.end], "pStyle", style_id)?;
        let mut out = String::new();
        out.push_str(&open_tag);
        out.push_str(&fragment[open_end + 1..child.start]);
        out.push_str(&updated_props);
        out.push_str(&fragment[child.end..close_start]);
        out.push_str("</");
        out.push_str(&tag_name);
        out.push('>');
        return Ok(out);
    }
    let mut out = String::new();
    out.push_str(&open_tag);
    out.push_str(&props);
    out.push_str(&fragment[open_end + 1..close_start]);
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok(out)
}

fn set_docx_run_style_for_paragraph_fragment(
    fragment: &str,
    para_id: &str,
    style_id: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let start_tag = &fragment[..=open_end];
    let open_tag = docx_open_tag_with_para_id(start_tag, para_id);
    if self_closing {
        return Ok(format!("{open_tag}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    let mut out = String::new();
    out.push_str(&open_tag);
    let mut cursor = open_end + 1;
    for child in children {
        if child.kind != "r" {
            continue;
        }
        out.push_str(&fragment[cursor..child.start]);
        out.push_str(&set_docx_run_style_fragment(
            &fragment[child.start..child.end],
            style_id,
        )?);
        cursor = child.end;
    }
    out.push_str(&fragment[cursor..close_start]);
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok(out)
}

fn set_docx_run_style_fragment(fragment: &str, style_id: &str) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let prefix = xml_tag_prefix(&tag_name);
    let props = render_docx_style_props(&prefix, "rPr", "rStyle", style_id);
    if self_closing {
        let open = xml_open_tag_from_start(&fragment[..=open_end]);
        return Ok(format!("{open}{props}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    if let Some(child) = children.into_iter().find(|child| child.kind == "rPr") {
        let updated_props =
            set_docx_style_child_in_props(&fragment[child.start..child.end], "rStyle", style_id)?;
        let mut out = String::new();
        out.push_str(&fragment[..child.start]);
        out.push_str(&updated_props);
        out.push_str(&fragment[child.end..]);
        return Ok(out);
    }
    let mut out = String::new();
    out.push_str(&fragment[..open_end + 1]);
    out.push_str(&props);
    out.push_str(&fragment[open_end + 1..]);
    Ok(out)
}

fn set_docx_table_style_fragment(fragment: &str, style_id: &str) -> CliResult<String> {
    let scaffolded = ensure_docx_table_scaffold_fragment(fragment)?;
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(&scaffolded)?;
    if self_closing {
        return Ok(scaffolded);
    }
    let children = xml_direct_child_ranges(&scaffolded, open_end + 1, close_start)?;
    let Some(child) = children.into_iter().find(|child| child.kind == "tblPr") else {
        return Ok(scaffolded);
    };
    let updated_props =
        set_docx_style_child_in_props(&scaffolded[child.start..child.end], "tblStyle", style_id)?;
    let mut out = String::new();
    out.push_str(&scaffolded[..child.start]);
    out.push_str(&updated_props);
    out.push_str(&scaffolded[child.end..]);
    Ok(out)
}

fn set_docx_style_child_in_props(
    props_fragment: &str,
    style_local: &str,
    style_id: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(props_fragment)?;
    let prefix = xml_tag_prefix(&tag_name);
    let style_child = render_docx_style_child(&prefix, style_local, style_id);
    if self_closing {
        let open = xml_open_tag_from_start(&props_fragment[..=open_end]);
        return Ok(format!("{open}{style_child}</{tag_name}>"));
    }
    let children = xml_direct_child_ranges(props_fragment, open_end + 1, close_start)?;
    if let Some(child) = children.into_iter().find(|child| child.kind == style_local) {
        let mut out = String::new();
        out.push_str(&props_fragment[..child.start]);
        out.push_str(&style_child);
        out.push_str(&props_fragment[child.end..]);
        return Ok(out);
    }
    let mut out = String::new();
    out.push_str(&props_fragment[..open_end + 1]);
    out.push_str(&style_child);
    out.push_str(&props_fragment[open_end + 1..]);
    Ok(out)
}

fn render_docx_style_props(
    prefix: &str,
    props_local: &str,
    style_local: &str,
    style_id: &str,
) -> String {
    let props = word_xml_tag(prefix, props_local);
    let mut out = String::new();
    out.push('<');
    out.push_str(&props);
    out.push('>');
    out.push_str(&render_docx_style_child(prefix, style_local, style_id));
    out.push_str("</");
    out.push_str(&props);
    out.push('>');
    out
}

fn render_docx_style_child(prefix: &str, style_local: &str, style_id: &str) -> String {
    let style_tag = word_xml_tag(prefix, style_local);
    let val_attr = if prefix.is_empty() {
        "w:val".to_string()
    } else {
        format!("{prefix}:val")
    };
    format!(
        "<{} {}=\"{}\"/>",
        style_tag,
        val_attr,
        xml_attr_escape(style_id)
    )
}

fn ensure_docx_body_table_scaffolds_xml(xml: &str) -> CliResult<String> {
    let body_tag = docx_body_tag(xml)?;
    let blocks = docx_body_block_ranges(xml, &body_tag)?;
    let mut out = String::with_capacity(xml.len());
    let mut cursor = 0usize;
    for block in blocks {
        if block.kind != "tbl" {
            continue;
        }
        out.push_str(&xml[cursor..block.start]);
        out.push_str(&ensure_docx_table_scaffold_fragment(
            &xml[block.start..block.end],
        )?);
        cursor = block.end;
    }
    out.push_str(&xml[cursor..]);
    Ok(out)
}

fn ensure_docx_table_scaffold_fragment(fragment: &str) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(fragment.to_string());
    }
    let prefix = xml_tag_prefix(&tag_name);
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    let has_tbl_pr = children.iter().any(|child| child.kind == "tblPr");
    let has_tbl_grid = children.iter().any(|child| child.kind == "tblGrid");
    if has_tbl_pr && has_tbl_grid {
        return Ok(fragment.to_string());
    }
    let first_row_start = children
        .iter()
        .find(|child| child.kind == "tr")
        .map(|child| child.start)
        .unwrap_or(open_end + 1);
    let mut scaffold = String::new();
    if !has_tbl_pr {
        scaffold.push_str(&format!("<{}/>", word_xml_tag(&prefix, "tblPr")));
    }
    if !has_tbl_grid {
        scaffold.push_str(&render_docx_tbl_grid(
            &prefix,
            docx_table_max_cols(fragment)?,
        ));
    }
    let mut out = String::new();
    out.push_str(&fragment[..first_row_start]);
    out.push_str(&scaffold);
    out.push_str(&fragment[first_row_start..]);
    Ok(out)
}

fn render_docx_tbl_grid(prefix: &str, cols: usize) -> String {
    let tbl_grid = word_xml_tag(prefix, "tblGrid");
    let grid_col = word_xml_tag(prefix, "gridCol");
    let width_attr = if prefix.is_empty() {
        "w:w".to_string()
    } else {
        format!("{prefix}:w")
    };
    let mut out = format!("<{tbl_grid}>");
    for _ in 0..cols {
        out.push_str(&format!("<{grid_col} {width_attr}=\"0\"/>"));
    }
    out.push_str("</");
    out.push_str(&tbl_grid);
    out.push('>');
    out
}

fn docx_table_max_cols(fragment: &str) -> CliResult<usize> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<String> = Vec::new();
    let mut table_depth = 0usize;
    let mut current_cols = 0usize;
    let mut max_cols = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                if name == "tbl" {
                    table_depth += 1;
                } else if table_depth == 1 && parent == Some("tr") && name == "tc" {
                    current_cols += 1;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                if table_depth == 1 && parent == Some("tr") && name == "tc" {
                    current_cols += 1;
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if table_depth == 1 && name == "tr" {
                    max_cols = max_cols.max(current_cols);
                    current_cols = 0;
                }
                if name == "tbl" && table_depth > 0 {
                    table_depth -= 1;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(max_cols)
}

fn docx_first_run_style(fragment: &str) -> CliResult<String> {
    docx_style_in_fragment(fragment, "rPr", "rStyle")
}

fn docx_table_style(fragment: &str) -> CliResult<String> {
    docx_style_in_fragment(fragment, "tblPr", "tblStyle")
}

fn docx_style_in_fragment(
    fragment: &str,
    property_parent: &str,
    style_local: &str,
) -> CliResult<String> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<String> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                if parent == Some(property_parent)
                    && name == style_local
                    && let Some(style) = attr(&e, "val")
                {
                    return Ok(style);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                if parent == Some(property_parent)
                    && name == style_local
                    && let Some(style) = attr(&e, "val")
                {
                    return Ok(style);
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(String::new())
}

fn xml_fragment_bounds(fragment: &str) -> CliResult<(usize, String, usize, bool)> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let tag_name = xml_token_name(&fragment[1..open_end])
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?
        .to_string();
    let self_closing = fragment[..=open_end].trim_end().ends_with("/>");
    let close_start = if self_closing {
        open_end + 1
    } else {
        let close_tag = format!("</{tag_name}>");
        fragment
            .rfind(&close_tag)
            .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?
    };
    Ok((open_end, tag_name, close_start, self_closing))
}

fn xml_open_tag_from_start(start_tag: &str) -> String {
    if !start_tag.trim_end().ends_with("/>") {
        return start_tag.to_string();
    }
    let slash = start_tag
        .rfind('/')
        .unwrap_or_else(|| start_tag.len().saturating_sub(1));
    let mut open = String::new();
    open.push_str(&start_tag[..slash]);
    open.push('>');
    open
}

fn xml_tag_prefix(tag_name: &str) -> String {
    tag_name
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default()
}

fn append_docx_body_paragraph_xml(xml: &str, text: &str, style: &str) -> CliResult<String> {
    let body_tag = docx_body_tag(xml)?;
    let close_tag = format!("</{body_tag}>");
    if !xml.contains(&close_tag) {
        return Err(CliError::unexpected("document body element not found"));
    }
    let prefix = body_tag
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default();
    let mut working = if prefix.is_empty() && !style.is_empty() {
        ensure_docx_word_prefix(xml)?
    } else {
        xml.to_string()
    };
    let body_close = working.rfind(&close_tag).ok_or_else(|| {
        CliError::unexpected("document body element not found after namespace update")
    })?;
    let insert_at = docx_body_sectpr_start(&working[..body_close], &prefix).unwrap_or(body_close);
    let paragraph = render_docx_paragraph(&prefix, text, style);
    working.insert_str(insert_at, &paragraph);
    Ok(working)
}

fn insert_docx_body_paragraph_xml(
    xml: &str,
    insert_after: usize,
    text: &str,
    style: &str,
) -> CliResult<(String, usize)> {
    let body_tag = docx_body_tag(xml)?;
    let close_tag = format!("</{body_tag}>");
    if !xml.contains(&close_tag) {
        return Err(CliError::unexpected("document body element not found"));
    }
    let prefix = body_tag
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default();
    let mut working = if prefix.is_empty() && !style.is_empty() {
        ensure_docx_word_prefix(xml)?
    } else {
        xml.to_string()
    };
    let body_close = working.rfind(&close_tag).ok_or_else(|| {
        CliError::unexpected("document body element not found after namespace update")
    })?;
    let blocks = docx_body_block_ranges(&working, &body_tag)?;
    let (insert_at, index) = if insert_after == 0 {
        (
            blocks.first().map(|block| block.start).unwrap_or_else(|| {
                docx_body_sectpr_start(&working[..body_close], &prefix).unwrap_or(body_close)
            }),
            1,
        )
    } else {
        let block = blocks.get(insert_after - 1).ok_or_else(|| {
            CliError::target_not_found(format!("target not found: block index {insert_after}"))
        })?;
        (block.end, insert_after + 1)
    };
    let paragraph = render_docx_paragraph(&prefix, text, style);
    working.insert_str(insert_at, &paragraph);
    Ok((working, index))
}

#[derive(Clone, Copy)]
struct XmlRange {
    start: usize,
    end: usize,
    kind: &'static str,
}

fn docx_body_block_ranges(xml: &str, body_tag: &str) -> CliResult<Vec<XmlRange>> {
    let (content_start, content_end) = docx_body_content_bounds(xml, body_tag)?;
    let mut cursor = content_start;
    let mut depth = 0usize;
    let mut active_block_start: Option<usize> = None;
    let mut active_block_kind: &'static str = "";
    let mut blocks = Vec::new();
    while cursor < content_end {
        let Some(relative_start) = xml[cursor..content_end].find('<') else {
            break;
        };
        let tag_start = cursor + relative_start;
        let Some(relative_end) = xml[tag_start..content_end].find('>') else {
            return Err(CliError::unexpected("invalid DOCX XML"));
        };
        let tag_end = tag_start + relative_end;
        let token = &xml[tag_start + 1..tag_end];
        let trimmed = token.trim_start();
        if trimmed.starts_with("!--") || trimmed.starts_with('?') || trimmed.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        let closing = trimmed.starts_with('/');
        if closing {
            if depth > 0 {
                depth -= 1;
                if depth == 0
                    && let Some(start) = active_block_start.take()
                {
                    blocks.push(XmlRange {
                        start,
                        end: tag_end + 1,
                        kind: active_block_kind,
                    });
                    active_block_kind = "";
                }
            }
            cursor = tag_end + 1;
            continue;
        }

        let self_closing = trimmed.trim_end().ends_with('/');
        let name = xml_token_name(trimmed).unwrap_or_default();
        let kind = match local_name(name.as_bytes()) {
            "p" => "p",
            "tbl" => "tbl",
            _ => "",
        };
        let is_body_block = depth == 0 && !kind.is_empty();
        if is_body_block {
            active_block_start = Some(tag_start);
            active_block_kind = kind;
        }
        if self_closing {
            if is_body_block {
                blocks.push(XmlRange {
                    start: tag_start,
                    end: tag_end + 1,
                    kind,
                });
                active_block_start = None;
                active_block_kind = "";
            }
        } else {
            depth += 1;
        }
        cursor = tag_end + 1;
    }
    Ok(blocks)
}

#[derive(Clone)]
struct XmlNamedRange {
    start: usize,
    end: usize,
    kind: String,
}

fn replace_docx_paragraph_fragment(
    fragment: &str,
    para_id: &str,
    replacement: Option<&str>,
) -> CliResult<(String, bool)> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let start_tag = &fragment[..=open_end];
    let tag_name = xml_token_name(&fragment[1..open_end])
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?
        .to_string();
    let prefix = tag_name
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default();
    let self_closing = start_tag.trim_end().ends_with("/>");

    let mut paragraph_properties = String::new();
    let mut run_properties = String::new();
    let mut flattened = false;
    if !self_closing {
        let close_tag = format!("</{tag_name}>");
        let close_start = fragment
            .rfind(&close_tag)
            .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
        for child in xml_direct_child_ranges(fragment, open_end + 1, close_start)? {
            match child.kind.as_str() {
                "pPr" => {
                    if paragraph_properties.is_empty() {
                        paragraph_properties.push_str(&fragment[child.start..child.end]);
                    }
                }
                "r" => {
                    if run_properties.is_empty()
                        && let Some(r_pr) = first_direct_xml_child_by_kind(
                            &fragment[child.start..child.end],
                            "rPr",
                        )?
                    {
                        run_properties.push_str(&r_pr);
                    }
                }
                _ => flattened = true,
            }
        }
    }

    let mut paragraph = docx_open_tag_with_para_id(start_tag, para_id);
    paragraph.push_str(&paragraph_properties);
    if let Some(text) = replacement {
        let r = word_xml_tag(&prefix, "r");
        paragraph.push('<');
        paragraph.push_str(&r);
        paragraph.push('>');
        paragraph.push_str(&run_properties);
        append_docx_text_children(&mut paragraph, &prefix, text);
        paragraph.push_str("</");
        paragraph.push_str(&r);
        paragraph.push('>');
    }
    paragraph.push_str("</");
    paragraph.push_str(&tag_name);
    paragraph.push('>');
    Ok((paragraph, flattened))
}

fn first_direct_xml_child_by_kind(fragment: &str, wanted: &str) -> CliResult<Option<String>> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let start_tag = &fragment[..=open_end];
    if start_tag.trim_end().ends_with("/>") {
        return Ok(None);
    }
    let tag_name = xml_token_name(&fragment[1..open_end])
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let close_tag = format!("</{tag_name}>");
    let close_start = fragment
        .rfind(&close_tag)
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    Ok(
        xml_direct_child_ranges(fragment, open_end + 1, close_start)?
            .into_iter()
            .find(|child| child.kind == wanted)
            .map(|child| fragment[child.start..child.end].to_string()),
    )
}

fn xml_direct_child_ranges(
    xml: &str,
    content_start: usize,
    content_end: usize,
) -> CliResult<Vec<XmlNamedRange>> {
    let mut cursor = content_start;
    let mut depth = 0usize;
    let mut active_start: Option<usize> = None;
    let mut active_kind = String::new();
    let mut ranges = Vec::new();
    while cursor < content_end {
        let Some(relative_start) = xml[cursor..content_end].find('<') else {
            break;
        };
        let tag_start = cursor + relative_start;
        let Some(relative_end) = xml[tag_start..content_end].find('>') else {
            return Err(CliError::unexpected("invalid DOCX XML"));
        };
        let tag_end = tag_start + relative_end;
        let token = &xml[tag_start + 1..tag_end];
        let trimmed = token.trim_start();
        if trimmed.starts_with("!--") || trimmed.starts_with('?') || trimmed.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        if trimmed.starts_with('/') {
            if depth > 0 {
                depth -= 1;
                if depth == 0
                    && let Some(start) = active_start.take()
                {
                    ranges.push(XmlNamedRange {
                        start,
                        end: tag_end + 1,
                        kind: active_kind.clone(),
                    });
                    active_kind.clear();
                }
            }
            cursor = tag_end + 1;
            continue;
        }

        let self_closing = trimmed.trim_end().ends_with('/');
        let name = xml_token_name(trimmed).unwrap_or_default();
        if depth == 0 {
            active_start = Some(tag_start);
            active_kind = local_name(name.as_bytes()).to_string();
        }
        if self_closing {
            if depth == 0 {
                ranges.push(XmlNamedRange {
                    start: tag_start,
                    end: tag_end + 1,
                    kind: active_kind.clone(),
                });
                active_start = None;
                active_kind.clear();
            }
        } else {
            depth += 1;
        }
        cursor = tag_end + 1;
    }
    Ok(ranges)
}

fn docx_open_tag_with_para_id(start_tag: &str, para_id: &str) -> String {
    let mut out = if start_tag.trim_end().ends_with("/>") {
        let slash = start_tag
            .rfind('/')
            .unwrap_or_else(|| start_tag.len().saturating_sub(1));
        let mut open = String::with_capacity(start_tag.len());
        open.push_str(&start_tag[..slash]);
        open.push('>');
        open
    } else {
        start_tag.to_string()
    };
    if !xml_start_tag_has_para_id(&out) {
        insert_xml_start_tag_attr(
            &mut out,
            &format!("w14:paraId=\"{}\"", xml_attr_escape(para_id)),
        );
    }
    out
}

fn xml_start_tag_has_para_id(tag: &str) -> bool {
    tag.contains(":paraId=")
        || tag.contains(" paraId=")
        || tag.contains("\tparaId=")
        || tag.contains("\nparaId=")
}

fn insert_xml_start_tag_attr(tag: &mut String, attr: &str) {
    if let Some(insert_at) = tag.rfind('>') {
        tag.insert_str(insert_at, &format!(" {attr}"));
    }
}

fn ensure_docx_w14_namespace(xml: &str) -> CliResult<String> {
    if xml.contains("xmlns:w14=") {
        return Ok(xml.to_string());
    }
    let document_start = xml
        .find("<w:document")
        .or_else(|| xml.find("<document"))
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let start_end = xml[document_start..]
        .find('>')
        .map(|offset| document_start + offset)
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let mut out = String::with_capacity(xml.len() + 72);
    out.push_str(&xml[..start_end]);
    out.push_str(" xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\"");
    out.push_str(&xml[start_end..]);
    Ok(out)
}

fn docx_all_para_ids(xml: &str) -> CliResult<BTreeSet<String>> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut ids = BTreeSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "p"
                    && element_in_ns(reader.resolver(), &e, DOCX_W_NS) =>
            {
                if let Some(para_id) = docx_para_id_ns(&e, reader.resolver()) {
                    ids.insert(para_id.to_ascii_uppercase());
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(ids)
}

fn mint_docx_para_id(existing: &BTreeSet<String>) -> String {
    for attempt in 0..10_000u32 {
        let raw =
            ((chrono_like_counter() as u64) ^ ((std::process::id() as u64) << 17) ^ attempt as u64)
                & 0x7fff_ffff;
        let candidate = format!("{:08X}", raw as u32);
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
    "00000000".to_string()
}

fn docx_body_content_bounds(xml: &str, body_tag: &str) -> CliResult<(usize, usize)> {
    let body_open = xml
        .find(&format!("<{body_tag}"))
        .ok_or_else(|| CliError::unexpected("document body element not found"))?;
    let content_start = xml[body_open..]
        .find('>')
        .map(|offset| body_open + offset + 1)
        .ok_or_else(|| CliError::unexpected("document body element not found"))?;
    let content_end = xml
        .rfind(&format!("</{body_tag}>"))
        .ok_or_else(|| CliError::unexpected("document body element not found"))?;
    Ok((content_start, content_end))
}

fn xml_token_name(token: &str) -> Option<&str> {
    let token = token.trim_start().trim_start_matches('/');
    if token.is_empty() || token.starts_with('?') || token.starts_with('!') {
        return None;
    }
    let end = token
        .find(|ch: char| ch.is_whitespace() || ch == '/')
        .unwrap_or(token.len());
    Some(&token[..end])
}

fn docx_body_prefix(body_tag: &str) -> String {
    body_tag
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default()
}

fn docx_block_has_section_properties(fragment: &str) -> bool {
    let mut cursor = 0usize;
    while cursor < fragment.len() {
        let Some(relative_start) = fragment[cursor..].find('<') else {
            break;
        };
        let tag_start = cursor + relative_start;
        let Some(relative_end) = fragment[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_end;
        let token = &fragment[tag_start + 1..tag_end];
        if let Some(name) = xml_token_name(token)
            && local_name(name.as_bytes()) == "sectPr"
        {
            return true;
        }
        cursor = tag_end + 1;
    }
    false
}

fn docx_body_tag(xml: &str) -> CliResult<String> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<String> = Vec::new();
    let mut word_stack: Vec<bool> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.last().is_some_and(|parent| parent == "document")
                    && word_stack.last().copied().unwrap_or(false)
                    && name == "body"
                    && is_word
                {
                    return Ok(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
                stack.push(name);
                word_stack.push(is_word);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.last().is_some_and(|parent| parent == "document")
                    && word_stack.last().copied().unwrap_or(false)
                    && name == "body"
                    && is_word
                {
                    return Ok(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
                word_stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "failed to read main document: {err}"
                )));
            }
            _ => {}
        }
    }
    Err(CliError::unexpected("document body element not found"))
}

fn ensure_docx_word_prefix(xml: &str) -> CliResult<String> {
    if xml.contains("xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"") {
        return Ok(xml.to_string());
    }
    let document_start = xml
        .find("<document")
        .or_else(|| xml.find("<w:document"))
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let start_end = xml[document_start..]
        .find('>')
        .map(|offset| document_start + offset)
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let mut out = String::with_capacity(xml.len() + 83);
    out.push_str(&xml[..start_end]);
    out.push_str(" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"");
    out.push_str(&xml[start_end..]);
    Ok(out)
}

fn docx_body_sectpr_start(body_prefix: &str, prefix: &str) -> Option<usize> {
    let tag = if prefix.is_empty() {
        "<sectPr".to_string()
    } else {
        format!("<{prefix}:sectPr")
    };
    body_prefix.rfind(&tag)
}

fn render_docx_paragraph(prefix: &str, text: &str, style: &str) -> String {
    let p = word_xml_tag(prefix, "p");
    let mut paragraph = String::new();
    paragraph.push('<');
    paragraph.push_str(&p);
    paragraph.push('>');
    if !style.is_empty() {
        let p_pr = word_xml_tag(prefix, "pPr");
        let p_style = word_xml_tag(prefix, "pStyle");
        let val_attr = if prefix.is_empty() {
            "w:val".to_string()
        } else {
            format!("{prefix}:val")
        };
        paragraph.push('<');
        paragraph.push_str(&p_pr);
        paragraph.push('>');
        paragraph.push('<');
        paragraph.push_str(&p_style);
        paragraph.push(' ');
        paragraph.push_str(&val_attr);
        paragraph.push_str("=\"");
        paragraph.push_str(&xml_attr_escape(style));
        paragraph.push_str("\"/>");
        paragraph.push_str("</");
        paragraph.push_str(&p_pr);
        paragraph.push('>');
    }
    if !text.is_empty() {
        let r = word_xml_tag(prefix, "r");
        paragraph.push('<');
        paragraph.push_str(&r);
        paragraph.push('>');
        append_docx_text_children(&mut paragraph, prefix, text);
        paragraph.push_str("</");
        paragraph.push_str(&r);
        paragraph.push('>');
    }
    paragraph.push_str("</");
    paragraph.push_str(&p);
    paragraph.push('>');
    paragraph
}

fn append_docx_text_children(out: &mut String, prefix: &str, text: &str) {
    for (line_index, line) in text.split('\n').enumerate() {
        if line_index > 0 {
            let br = word_xml_tag(prefix, "br");
            out.push('<');
            out.push_str(&br);
            out.push_str("/>");
        }
        for (segment_index, segment) in line.split('\t').enumerate() {
            if segment_index > 0 {
                let tab = word_xml_tag(prefix, "tab");
                out.push('<');
                out.push_str(&tab);
                out.push_str("/>");
            }
            if segment.is_empty() {
                continue;
            }
            let t = word_xml_tag(prefix, "t");
            out.push('<');
            out.push_str(&t);
            if needs_docx_space_preserve(segment) {
                out.push_str(" xml:space=\"preserve\"");
            }
            out.push('>');
            out.push_str(&xml_escape(segment));
            out.push_str("</");
            out.push_str(&t);
            out.push('>');
        }
    }
}

fn needs_docx_space_preserve(value: &str) -> bool {
    value != value.trim_matches(|ch| matches!(ch, ' ' | '\t' | '\r' | '\n'))
}

fn word_xml_tag(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn docx_table_summary_json(
    file: &str,
    table_number: usize,
    report: DocxRichBlockReport,
    include_details: bool,
) -> Value {
    let rows = report.table_rows;
    let row_count = rows.len();
    let col_count = rows.iter().map(Vec::len).max().unwrap_or_default();
    let mut table = Map::new();
    table.insert("file".to_string(), json!(file));
    table.insert("table".to_string(), json!(table_number));
    table.insert("block".to_string(), json!(report.index));
    table.insert(
        "primarySelector".to_string(),
        json!(table_number.to_string()),
    );
    table.insert("selectors".to_string(), json!([table_number.to_string()]));
    table.insert("contentHash".to_string(), json!(report.content_hash));
    table.insert("rows".to_string(), json!(row_count));
    table.insert("cols".to_string(), json!(col_count));
    table.insert("merged".to_string(), json!(report.table_merged));
    if include_details {
        let detail_rows: Vec<Value> = rows.iter().map(|row| json!({"cells": row})).collect();
        table.insert("tableInfo".to_string(), json!({"rows": detail_rows}));
    } else {
        table.insert("cells".to_string(), json!(rows));
    }
    Value::Object(table)
}

fn docx_document_and_comments_parts(file: &str) -> CliResult<(String, Option<String>)> {
    let entries = zip_entry_names(file)?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        return Err(CliError::unsupported_type(
            "file is not a DOCX document (detected: unknown)",
        ));
    }
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let comments_part = find_docx_comments_part(file, &entries, &document_part)?;
    Ok((document_uri, comments_part))
}

fn find_docx_comments_part(
    file: &str,
    entries: &[String],
    document_part: &str,
) -> CliResult<Option<String>> {
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    for rel in
        relationship_entries(file, &relationships_part_for(document_part)).unwrap_or_default()
    {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"
        {
            let uri = resolve_relationship_target(&document_uri, &rel.target);
            return Ok(zip_entry_exists(entries, &uri).then_some(uri));
        }
    }
    let conventional = "/word/comments.xml";
    Ok(zip_entry_exists(entries, conventional).then(|| conventional.to_string()))
}

fn docx_comments_part_uri(
    file: &str,
    entries: &[String],
    document_part: &str,
) -> CliResult<String> {
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    for rel in
        relationship_entries(file, &relationships_part_for(document_part)).unwrap_or_default()
    {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"
        {
            return Ok(resolve_relationship_target(&document_uri, &rel.target));
        }
    }
    let conventional = "/word/comments.xml";
    if zip_entry_exists(entries, conventional) {
        return Ok(conventional.to_string());
    }
    Ok(conventional.to_string())
}

fn zip_entry_exists(entries: &[String], uri: &str) -> bool {
    let wanted = format!("/{}", uri.trim_start_matches('/'));
    entries
        .iter()
        .any(|entry| format!("/{}", entry.trim_start_matches('/')) == wanted)
}

fn ensure_docx_comments_relationship_xml(
    file: &str,
    document_part: &str,
    document_uri: &str,
    comments_part: &str,
) -> CliResult<(String, Option<String>, bool)> {
    let rels_part = relationships_part_for(document_part);
    let rels = relationship_entries(file, &rels_part).unwrap_or_default();
    if rels.iter().any(|rel| {
        rel.target_mode != "External"
            && rel.rel_type
                == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"
    }) {
        return Ok((rels_part, None, false));
    }

    let next_id = allocate_relationship_id(&rels);
    let target = relationship_target_from_source_to_target(document_uri, comments_part);
    let rel = format!(
        r#"<Relationship Id="{next_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="{}"/>"#,
        xml_attr_escape(&target)
    );
    let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| {
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#
            .to_string()
    });
    let updated = if let Some(pos) = rels_xml.rfind("</Relationships>") {
        let mut out = String::with_capacity(rels_xml.len() + rel.len());
        out.push_str(&rels_xml[..pos]);
        out.push_str(&rel);
        out.push_str(&rels_xml[pos..]);
        out
    } else {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rel}</Relationships>"#
        )
    };
    Ok((rels_part, Some(updated), true))
}

fn write_docx_mutation_overrides_output(
    file: &str,
    overrides: &BTreeMap<String, String>,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<()> {
    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        docx_mutation_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_overrides(file, &readback_path, overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

#[derive(Clone, Default)]
struct DocxCommentInfo {
    id: i64,
    id_raw: String,
    id_valid: bool,
    author: String,
    date: String,
    initials: String,
    text: String,
    anchored_to_block: usize,
    anchored_to_block_kind: String,
}

#[derive(Default)]
struct DocxCommentBuild {
    info: DocxCommentInfo,
    paragraphs: Vec<String>,
    current_paragraph: Option<String>,
    in_t: bool,
    skip_text_depth: usize,
}

fn docx_comments(
    file: &str,
    comments_part: &str,
    document_part: &str,
) -> CliResult<Vec<DocxCommentInfo>> {
    let xml = zip_text(file, comments_part.trim_start_matches('/'))?;
    let anchors = docx_comment_anchors(file, document_part)?;
    let mut reader = Reader::from_str(&xml);
    let mut saw_root = false;
    let mut stack = Vec::<String>::new();
    let mut current: Option<DocxCommentBuild> = None;
    let mut comments = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "comments" {
                        return Ok(Vec::new());
                    }
                } else if name == "comment"
                    && stack.last().is_some_and(|parent| parent == "comments")
                {
                    current = Some(docx_comment_from_element(&e));
                } else if let Some(comment) = current.as_mut() {
                    docx_note_comment_start(&e, &name, &stack, comment);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "comments" {
                        return Ok(Vec::new());
                    }
                } else if name == "comment"
                    && stack.last().is_some_and(|parent| parent == "comments")
                {
                    let mut comment = docx_comment_from_element(&e);
                    docx_finish_comment(&mut comment, &anchors);
                    comments.push(comment.info);
                } else if let Some(comment) = current.as_mut() {
                    docx_note_comment_empty(&e, &name, &stack, comment);
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(comment) = current.as_mut()
                    && comment.in_t
                    && comment.skip_text_depth == 0
                    && let Some(paragraph) = comment.current_paragraph.as_mut()
                {
                    paragraph.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(comment) = current.as_mut()
                    && comment.in_t
                    && comment.skip_text_depth == 0
                    && let Some(paragraph) = comment.current_paragraph.as_mut()
                {
                    paragraph.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(comment) = current.as_mut() {
                    match name.as_str() {
                        "t" => comment.in_t = false,
                        "delText" | "instrText" => {
                            comment.skip_text_depth = comment.skip_text_depth.saturating_sub(1);
                        }
                        "p" => {
                            if let Some(paragraph) = comment.current_paragraph.take() {
                                comment.paragraphs.push(paragraph);
                            }
                        }
                        "comment" => {
                            if let Some(mut comment) = current.take() {
                                docx_finish_comment(&mut comment, &anchors);
                                comments.push(comment.info);
                            }
                        }
                        _ => {}
                    }
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(comments)
}

fn docx_comment_from_element(element: &BytesStart<'_>) -> DocxCommentBuild {
    let id_raw = attr(element, "id").unwrap_or_default();
    let (id, id_valid) = parse_docx_comment_id(&id_raw);
    DocxCommentBuild {
        info: DocxCommentInfo {
            id,
            id_raw,
            id_valid,
            author: attr(element, "author").unwrap_or_default(),
            date: attr(element, "date").unwrap_or_default(),
            initials: attr(element, "initials").unwrap_or_default(),
            ..DocxCommentInfo::default()
        },
        ..DocxCommentBuild::default()
    }
}

fn docx_note_comment_start(
    element: &BytesStart<'_>,
    name: &str,
    stack: &[String],
    comment: &mut DocxCommentBuild,
) {
    if name == "p" && stack.last().is_some_and(|parent| parent == "comment") {
        comment.current_paragraph = Some(String::new());
    }
    docx_note_comment_empty(element, name, stack, comment);
    if name == "t" {
        comment.in_t = true;
    }
    if name == "delText" || name == "instrText" {
        comment.skip_text_depth += 1;
    }
}

fn docx_note_comment_empty(
    _element: &BytesStart<'_>,
    name: &str,
    _stack: &[String],
    comment: &mut DocxCommentBuild,
) {
    let Some(paragraph) = comment.current_paragraph.as_mut() else {
        return;
    };
    match name {
        "tab" => paragraph.push('\t'),
        "br" | "cr" => paragraph.push('\n'),
        "noBreakHyphen" => paragraph.push('-'),
        _ => {}
    }
}

fn docx_finish_comment(
    comment: &mut DocxCommentBuild,
    anchors: &BTreeMap<String, DocxCommentAnchor>,
) {
    comment.info.text = comment.paragraphs.join("\n");
    if let Some(anchor) = anchors.get(&comment.info.id_raw) {
        comment.info.anchored_to_block = anchor.index;
        comment.info.anchored_to_block_kind = anchor.kind.clone();
    }
}

fn parse_docx_comment_id(value: &str) -> (i64, bool) {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return (0, false);
    }
    value
        .parse::<i64>()
        .map(|id| (id, true))
        .unwrap_or((0, false))
}

#[derive(Clone)]
struct DocxCommentAnchor {
    index: usize,
    kind: String,
    tag: String,
    depth: usize,
}

fn docx_comment_anchors(
    file: &str,
    document_part: &str,
) -> CliResult<BTreeMap<String, DocxCommentAnchor>> {
    let xml = zip_text(file, document_part.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut anchors = BTreeMap::<String, DocxCommentAnchor>::new();
    let mut current_block: Option<DocxCommentAnchor> = None;
    let mut block_index = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.last().is_some_and(|parent| parent == "body")
                    && matches!(name.as_str(), "p" | "tbl")
                {
                    block_index += 1;
                    current_block = Some(DocxCommentAnchor {
                        index: block_index,
                        kind: if name == "p" { "paragraph" } else { "table" }.to_string(),
                        tag: name.clone(),
                        depth: stack.len() + 1,
                    });
                }
                if name == "commentRangeStart" {
                    docx_note_comment_anchor(&mut anchors, current_block.as_ref(), &e);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.last().is_some_and(|parent| parent == "body")
                    && matches!(name.as_str(), "p" | "tbl")
                {
                    block_index += 1;
                }
                if name == "commentRangeStart" {
                    docx_note_comment_anchor(&mut anchors, current_block.as_ref(), &e);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current_block
                    .as_ref()
                    .is_some_and(|block| block.depth == stack.len() && block.tag == name)
                {
                    current_block = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(anchors)
}

fn docx_note_comment_anchor(
    anchors: &mut BTreeMap<String, DocxCommentAnchor>,
    current_block: Option<&DocxCommentAnchor>,
    element: &BytesStart<'_>,
) {
    let Some(block) = current_block else {
        return;
    };
    if let Some(id) = attr(element, "id") {
        anchors.entry(id).or_insert_with(|| block.clone());
    }
}

fn docx_comment_id_counts(comments: &[DocxCommentInfo]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for comment in comments {
        if !comment.id_raw.is_empty() {
            *counts.entry(comment.id_raw.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn docx_comment_json(comment: &DocxCommentInfo, counts: &BTreeMap<String, usize>) -> Value {
    let mut object = Map::new();
    object.insert("id".to_string(), json!(comment.id));
    object.insert("author".to_string(), json!(comment.author));
    if !comment.date.is_empty() {
        object.insert("date".to_string(), json!(comment.date));
    }
    if !comment.initials.is_empty() {
        object.insert("initials".to_string(), json!(comment.initials));
    }
    object.insert("text".to_string(), json!(comment.text));
    object.insert(
        "contentHash".to_string(),
        json!(docx_comment_content_hash(
            &comment.author,
            &comment.date,
            &comment.text
        )),
    );
    if comment.anchored_to_block > 0 {
        object.insert(
            "anchoredToBlock".to_string(),
            json!(comment.anchored_to_block),
        );
    }
    if !comment.anchored_to_block_kind.is_empty() {
        object.insert(
            "anchoredToBlockKind".to_string(),
            json!(comment.anchored_to_block_kind),
        );
    }
    if comment.id_valid {
        let selector = comment.id.to_string();
        object.insert("primarySelector".to_string(), json!(selector));
        object.insert("selectors".to_string(), json!([selector]));
        if counts.get(&comment.id_raw).copied().unwrap_or_default() == 1 {
            object.insert(
                "handle".to_string(),
                json!(format!("H:docx/pt:doc/comment:n:{}", comment.id)),
            );
        }
    }
    Value::Object(object)
}

fn docx_comment_content_hash(author: &str, date: &str, text: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(author.as_bytes());
    hash.update([0]);
    hash.update(date.as_bytes());
    hash.update([0]);
    hash.update(text.as_bytes());
    format!("sha256:{:x}", hash.finalize())
}

#[derive(Clone, Copy)]
struct XmlFullElementSpan {
    start: usize,
    end: usize,
    open_end: usize,
}

fn edit_docx_comment_xml(
    comments_xml: &str,
    target_id: i64,
    edit: DocxCommentEditSpec<'_>,
) -> CliResult<(String, DocxCommentInfo, DocxCommentInfo)> {
    let spans = docx_comment_element_spans_by_id(comments_xml, target_id)?;
    let Some(span) = spans.first().copied() else {
        return Err(CliError::target_not_found("target not found: comment"));
    };
    let fragment = &comments_xml[span.start..span.end];
    let (before, paragraph_count) = docx_comment_info_from_fragment(fragment)?;
    let before_hash = docx_comment_content_hash(&before.author, &before.date, &before.text);
    if !edit.expect_hash.is_empty() && edit.expect_hash != before_hash {
        return Err(CliError::invalid_args(format!(
            "comment hash mismatch: comment {target_id} expected {} but found {before_hash}",
            edit.expect_hash
        )));
    }
    if edit.text_set && paragraph_count > 1 {
        return Err(CliError::unexpected(format!(
            "failed to mutate comments: comment {target_id} has {paragraph_count} paragraphs; editing its text would discard structure (remove and re-add the comment instead)"
        )));
    }

    let mut edited = before.clone();
    if edit.author_set {
        edited.author = edit.author.to_string();
    }
    if edit.date_set {
        edited.date = edit.date.to_string();
    }
    if edit.text_set {
        edited.text = edit.text.to_string();
    }

    let tag_name = xml_token_name(&fragment[1..span.open_end - span.start - 1])
        .ok_or_else(|| CliError::unexpected("invalid comment XML"))?;
    let prefix = xml_tag_prefix(tag_name);
    let rendered = render_docx_comment(
        &prefix,
        target_id,
        &edited.author,
        &edited.date,
        &edited.initials,
        &edited.text,
    );
    let mut out = String::with_capacity(comments_xml.len() + rendered.len());
    out.push_str(&comments_xml[..span.start]);
    out.push_str(&rendered);
    out.push_str(&comments_xml[span.end..]);
    Ok((out, before, edited))
}

fn remove_docx_comment_xml(
    comments_xml: &str,
    target_id: i64,
    expect_hash: &str,
) -> CliResult<(String, DocxCommentInfo)> {
    let spans = docx_comment_element_spans_by_id(comments_xml, target_id)?;
    let Some(span) = spans.first().copied() else {
        return Err(CliError::target_not_found("target not found: comment"));
    };
    let fragment = &comments_xml[span.start..span.end];
    let (before, _) = docx_comment_info_from_fragment(fragment)?;
    let before_hash = docx_comment_content_hash(&before.author, &before.date, &before.text);
    if !expect_hash.is_empty() && expect_hash != before_hash {
        return Err(CliError::invalid_args(format!(
            "comment hash mismatch: comment {target_id} expected {expect_hash} but found {before_hash}"
        )));
    }

    let mut out = String::with_capacity(comments_xml.len().saturating_sub(span.end - span.start));
    out.push_str(&comments_xml[..span.start]);
    out.push_str(&comments_xml[span.end..]);
    Ok((out, before))
}

struct OpenXmlDeleteElement {
    name: String,
    start: usize,
    delete_self: bool,
    contains_target_comment_reference: bool,
}

fn remove_docx_comment_markers_xml(
    document_xml: &str,
    target_id: i64,
) -> CliResult<(String, bool)> {
    let body_tag = docx_body_tag(document_xml)?;
    let (content_start, content_end) = docx_body_content_bounds(document_xml, &body_tag)?;
    let body_xml = &document_xml[content_start..content_end];
    let target = target_id.to_string();
    let mut reader = Reader::from_str(body_xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<OpenXmlDeleteElement>::new();
    let mut ranges = Vec::<(usize, usize)>::new();

    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_target_marker =
                    matches!(name.as_str(), "commentRangeStart" | "commentRangeEnd")
                        && attr(&e, "id").is_some_and(|id| id == target);
                let is_target_reference =
                    name == "commentReference" && attr(&e, "id").is_some_and(|id| id == target);
                let reference_has_run_parent =
                    is_target_reference && mark_nearest_open_word_run(&mut stack);
                stack.push(OpenXmlDeleteElement {
                    name,
                    start: content_start + before,
                    delete_self: is_target_marker
                        || (is_target_reference && !reference_has_run_parent),
                    contains_target_comment_reference: false,
                });
            }
            Ok(Event::Empty(e)) => {
                let after = reader.buffer_position() as usize;
                let name = local_name(e.name().as_ref()).to_string();
                let is_target_marker =
                    matches!(name.as_str(), "commentRangeStart" | "commentRangeEnd")
                        && attr(&e, "id").is_some_and(|id| id == target);
                if is_target_marker {
                    ranges.push((content_start + before, content_start + after));
                    continue;
                }
                let is_target_reference =
                    name == "commentReference" && attr(&e, "id").is_some_and(|id| id == target);
                if is_target_reference && !mark_nearest_open_word_run(&mut stack) {
                    ranges.push((content_start + before, content_start + after));
                }
            }
            Ok(Event::End(_)) => {
                let after = reader.buffer_position() as usize;
                let Some(element) = stack.pop() else {
                    continue;
                };
                if element.delete_self || element.contains_target_comment_reference {
                    ranges.push((element.start, content_start + after));
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    if ranges.is_empty() {
        return Ok((document_xml.to_string(), false));
    }
    Ok((delete_xml_ranges(document_xml, ranges)?, true))
}

fn mark_nearest_open_word_run(stack: &mut [OpenXmlDeleteElement]) -> bool {
    if let Some(run) = stack.iter_mut().rev().find(|element| element.name == "r") {
        run.contains_target_comment_reference = true;
        true
    } else {
        false
    }
}

fn delete_xml_ranges(xml: &str, mut ranges: Vec<(usize, usize)>) -> CliResult<String> {
    ranges.retain(|(start, end)| start < end && *end <= xml.len());
    if ranges.is_empty() {
        return Ok(xml.to_string());
    }
    ranges.sort_by_key(|(start, end)| (*start, std::cmp::Reverse(*end)));
    let mut merged = Vec::<(usize, usize)>::new();
    for (start, end) in ranges {
        if let Some((_, current_end)) = merged.last_mut()
            && start <= *current_end
        {
            *current_end = (*current_end).max(end);
            continue;
        }
        merged.push((start, end));
    }
    let mut out = xml.to_string();
    for (start, end) in merged.into_iter().rev() {
        out.replace_range(start..end, "");
    }
    Ok(out)
}

fn docx_comment_element_spans_by_id(
    comments_xml: &str,
    target_id: i64,
) -> CliResult<Vec<XmlFullElementSpan>> {
    let target = target_id.to_string();
    let mut reader = Reader::from_str(comments_xml);
    reader.config_mut().trim_text(false);
    let mut spans = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "comment" => {
                let open_end = reader.buffer_position() as usize;
                let matches = attr(&e, "id").is_some_and(|id| id == target);
                let mut depth = 1usize;
                loop {
                    match reader.read_event() {
                        Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "comment" => {
                            depth += 1;
                        }
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "comment" => {
                            depth -= 1;
                            if depth == 0 {
                                if matches {
                                    spans.push(XmlFullElementSpan {
                                        start: before,
                                        end: reader.buffer_position() as usize,
                                        open_end,
                                    });
                                }
                                break;
                            }
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("invalid comments XML"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "comment" => {
                if attr(&e, "id").is_some_and(|id| id == target) {
                    spans.push(XmlFullElementSpan {
                        start: before,
                        end: reader.buffer_position() as usize,
                        open_end: reader.buffer_position() as usize,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(spans)
}

fn docx_comment_info_from_fragment(fragment: &str) -> CliResult<(DocxCommentInfo, usize)> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut info = DocxCommentInfo::default();
    let mut paragraphs = Vec::<String>::new();
    let mut current_paragraph: Option<String> = None;
    let mut paragraph_count = 0usize;
    let mut in_t = false;
    let mut skip_text_depth = 0usize;
    let mut saw_comment = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_comment {
                    if name != "comment" {
                        return Err(CliError::unexpected("invalid comment XML"));
                    }
                    saw_comment = true;
                    info = docx_comment_from_element(&e).info;
                } else {
                    if name == "p" && stack.last().is_some_and(|parent| parent == "comment") {
                        current_paragraph = Some(String::new());
                        paragraph_count += 1;
                    }
                    if name == "br"
                        && let Some(paragraph) = current_paragraph.as_mut()
                    {
                        paragraph.push('\n');
                    }
                    if name == "tab"
                        && let Some(paragraph) = current_paragraph.as_mut()
                    {
                        paragraph.push('\t');
                    }
                    if name == "t" {
                        in_t = true;
                    }
                    if matches!(name.as_str(), "delText" | "instrText") {
                        skip_text_depth += 1;
                    }
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_comment {
                    if name != "comment" {
                        return Err(CliError::unexpected("invalid comment XML"));
                    }
                    saw_comment = true;
                    info = docx_comment_from_element(&e).info;
                } else if name == "p" && stack.last().is_some_and(|parent| parent == "comment") {
                    paragraphs.push(String::new());
                    paragraph_count += 1;
                } else if name == "br" {
                    if let Some(paragraph) = current_paragraph.as_mut() {
                        paragraph.push('\n');
                    }
                } else if name == "tab"
                    && let Some(paragraph) = current_paragraph.as_mut()
                {
                    paragraph.push('\t');
                }
            }
            Ok(Event::Text(e)) => {
                if in_t
                    && skip_text_depth == 0
                    && let Some(paragraph) = current_paragraph.as_mut()
                {
                    paragraph.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if in_t
                    && skip_text_depth == 0
                    && let Some(paragraph) = current_paragraph.as_mut()
                {
                    paragraph.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                match name.as_str() {
                    "t" => in_t = false,
                    "delText" | "instrText" => {
                        skip_text_depth = skip_text_depth.saturating_sub(1);
                    }
                    "p" => {
                        if let Some(paragraph) = current_paragraph.take() {
                            paragraphs.push(paragraph);
                        }
                    }
                    _ => {}
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !saw_comment {
        return Err(CliError::unexpected("invalid comment XML"));
    }
    info.text = paragraphs.join("\n");
    Ok((info, paragraph_count))
}

fn docx_comments_template() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"></w:comments>"#
        .to_string()
}

fn docx_next_comment_id(comments_xml: &str) -> i64 {
    let mut reader = Reader::from_str(comments_xml);
    let mut max_id = -1_i64;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "comment" =>
            {
                if let Some(id) = attr(&e, "id").and_then(|value| value.parse::<i64>().ok())
                    && id > max_id
                {
                    max_id = id;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    max_id + 1
}

fn append_docx_comment_xml(
    comments_xml: &str,
    comment_id: i64,
    author: &str,
    date: &str,
    initials: &str,
    text: &str,
) -> CliResult<String> {
    let root_tag = docx_comments_root_tag(comments_xml)?;
    let prefix = xml_tag_prefix(&root_tag);
    let comment = render_docx_comment(&prefix, comment_id, author, date, initials, text);
    let close_tag = format!("</{root_tag}>");
    if let Some(pos) = comments_xml.rfind(&close_tag) {
        let mut out = String::with_capacity(comments_xml.len() + comment.len());
        out.push_str(&comments_xml[..pos]);
        out.push_str(&comment);
        out.push_str(&comments_xml[pos..]);
        return Ok(out);
    }

    let start = comments_xml
        .find(&format!("<{root_tag}"))
        .ok_or_else(|| CliError::unexpected("comments part has no w:comments root"))?;
    let open_end = comments_xml[start..]
        .find('>')
        .map(|offset| start + offset)
        .ok_or_else(|| CliError::unexpected("comments part has no w:comments root"))?;
    let start_tag = &comments_xml[start..=open_end];
    if !start_tag.trim_end().ends_with("/>") {
        return Err(CliError::unexpected(
            "comments part has no closing w:comments tag",
        ));
    }
    let mut out = String::with_capacity(comments_xml.len() + comment.len() + close_tag.len());
    out.push_str(&comments_xml[..start]);
    out.push_str(&xml_open_tag_from_start(start_tag));
    out.push_str(&comment);
    out.push_str(&close_tag);
    out.push_str(&comments_xml[open_end + 1..]);
    Ok(out)
}

fn docx_comments_root_tag(comments_xml: &str) -> CliResult<String> {
    let mut reader = Reader::from_str(comments_xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if local_name(e.name().as_ref()) == "comments" {
                    return Ok(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
                return Err(CliError::unexpected("comments part has no w:comments root"));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::unexpected("comments part has no w:comments root"))
}

fn render_docx_comment(
    prefix: &str,
    comment_id: i64,
    author: &str,
    date: &str,
    initials: &str,
    text: &str,
) -> String {
    let comment = word_xml_tag(prefix, "comment");
    let p = word_xml_tag(prefix, "p");
    let r = word_xml_tag(prefix, "r");
    let mut out = String::new();
    out.push('<');
    out.push_str(&comment);
    out.push(' ');
    out.push_str(&word_attr_name(prefix, "id"));
    out.push_str("=\"");
    out.push_str(&comment_id.to_string());
    out.push_str("\" ");
    out.push_str(&word_attr_name(prefix, "author"));
    out.push_str("=\"");
    out.push_str(&xml_attr_escape(author));
    out.push('"');
    if !date.is_empty() {
        out.push(' ');
        out.push_str(&word_attr_name(prefix, "date"));
        out.push_str("=\"");
        out.push_str(&xml_attr_escape(date));
        out.push('"');
    }
    if !initials.is_empty() {
        out.push(' ');
        out.push_str(&word_attr_name(prefix, "initials"));
        out.push_str("=\"");
        out.push_str(&xml_attr_escape(initials));
        out.push('"');
    }
    out.push('>');
    out.push('<');
    out.push_str(&p);
    out.push('>');
    if !text.is_empty() {
        out.push('<');
        out.push_str(&r);
        out.push('>');
        append_docx_text_children(&mut out, prefix, text);
        out.push_str("</");
        out.push_str(&r);
        out.push('>');
    }
    out.push_str("</");
    out.push_str(&p);
    out.push('>');
    out.push_str("</");
    out.push_str(&comment);
    out.push('>');
    out
}

fn insert_docx_comment_markers_xml(
    document_xml: &str,
    anchor_index: usize,
    comment_id: i64,
) -> CliResult<String> {
    let body_tag = docx_body_tag(document_xml)?;
    let prefix = xml_tag_prefix(&body_tag);
    let working = if prefix.is_empty() {
        ensure_docx_word_prefix(document_xml)?
    } else {
        document_xml.to_string()
    };
    let body_tag = docx_body_tag(&working)?;
    let prefix = xml_tag_prefix(&body_tag);
    let blocks = docx_body_block_ranges(&working, &body_tag)?;
    let block = blocks.get(anchor_index - 1).ok_or_else(|| {
        CliError::invalid_args(format!("comment anchor block out of range: {anchor_index}"))
    })?;
    if block.kind != "p" {
        return Err(CliError::invalid_args(format!(
            "comment anchor block is not a paragraph: block {anchor_index} is table"
        )));
    }
    let fragment = &working[block.start..block.end];
    let updated = insert_docx_comment_markers_in_paragraph(fragment, &prefix, comment_id)?;
    let mut out = String::with_capacity(working.len() + updated.len());
    out.push_str(&working[..block.start]);
    out.push_str(&updated);
    out.push_str(&working[block.end..]);
    Ok(out)
}

fn insert_docx_comment_markers_in_paragraph(
    paragraph: &str,
    prefix: &str,
    comment_id: i64,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(paragraph)?;
    let start_tag = &paragraph[..=open_end];
    let open_tag = xml_open_tag_from_start(start_tag);
    let close_tag = format!("</{tag_name}>");
    let content_start = open_tag.len();
    let normalized = if self_closing {
        format!("{open_tag}{close_tag}")
    } else {
        paragraph.to_string()
    };
    let content_end = if self_closing {
        content_start
    } else {
        close_start
    };
    let children = xml_direct_child_ranges(&normalized, content_start, content_end)?;
    let start_marker = render_docx_comment_range_marker(prefix, "commentRangeStart", comment_id);
    let end_marker = render_docx_comment_range_marker(prefix, "commentRangeEnd", comment_id);
    let reference = render_docx_comment_reference_run(prefix, comment_id);
    let run_children: Vec<&XmlNamedRange> =
        children.iter().filter(|child| child.kind == "r").collect();
    if let (Some(first_run), Some(last_run)) = (run_children.first(), run_children.last()) {
        let mut out = String::with_capacity(
            normalized.len() + start_marker.len() + end_marker.len() + reference.len(),
        );
        out.push_str(&normalized[..first_run.start]);
        out.push_str(&start_marker);
        out.push_str(&normalized[first_run.start..last_run.end]);
        out.push_str(&end_marker);
        out.push_str(&reference);
        out.push_str(&normalized[last_run.end..]);
        return Ok(out);
    }

    let insert_at = children
        .iter()
        .find(|child| child.kind == "pPr")
        .map(|child| child.end)
        .unwrap_or(content_start);
    let mut out = String::with_capacity(
        normalized.len() + start_marker.len() + end_marker.len() + reference.len(),
    );
    out.push_str(&normalized[..insert_at]);
    out.push_str(&start_marker);
    out.push_str(&end_marker);
    out.push_str(&reference);
    out.push_str(&normalized[insert_at..]);
    Ok(out)
}

fn render_docx_comment_range_marker(prefix: &str, local: &str, comment_id: i64) -> String {
    let tag = word_xml_tag(prefix, local);
    format!(
        r#"<{tag} {}="{}"/>"#,
        word_attr_name(prefix, "id"),
        comment_id
    )
}

fn render_docx_comment_reference_run(prefix: &str, comment_id: i64) -> String {
    let r = word_xml_tag(prefix, "r");
    let reference = word_xml_tag(prefix, "commentReference");
    format!(
        r#"<{r}><{reference} {}="{}"/></{r}>"#,
        word_attr_name(prefix, "id"),
        comment_id
    )
}

fn word_attr_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        format!("w:{local}")
    } else {
        format!("{prefix}:{local}")
    }
}

fn pptx_render(file: &str, args: &[String]) -> CliResult<Value> {
    let out = parse_string_flag(args, "--out")?
        .ok_or_else(|| CliError::invalid_args("--out is required"))?;
    if let Some(format) = parse_string_flag(args, "--format")?
        && format != "json"
    {
        return Err(CliError::invalid_args(
            "pptx render supports --format json only",
        ));
    }
    let slides = parse_slides_flag(args, "--slides")?.unwrap_or_else(|| pptx_all_slides(file));
    let output_dir = PathBuf::from(&out);
    fs::create_dir_all(&output_dir).map_err(|err| CliError::unexpected(err.to_string()))?;
    let pdf_path = if std::env::var_os("OOXML_RUST_MOCK_RENDER").is_some() {
        mock_render_outputs(file, &output_dir, &slides)?
    } else {
        render_with_local_tools(file, &output_dir, &slides)?
    };
    let slide_values: Vec<Value> = slides
        .iter()
        .map(|slide| {
            json!({
                "imagePath": output_dir.join(format!("slide-{slide}.png")).to_string_lossy(),
                "slide": slide,
            })
        })
        .collect();
    Ok(json!({
        "dpi": 144,
        "imageFormat": "png",
        "outputDir": out,
        "pdfPath": pdf_path.to_string_lossy(),
        "slides": slide_values,
        "sourceFile": file,
    }))
}

fn verify(file: &str, args: &[String]) -> CliResult<Value> {
    let baseline = parse_string_flag(args, "--baseline")?;
    let validation = verify_validation(file)?;
    let valid = validation["status"] == "valid";
    let package_type = package_type(file)?;
    let rendered = if package_type == "pptx" {
        json!({
            "enabled": true,
            "reason": "required render tool not available: soffice",
            "status": "unavailable",
        })
    } else {
        json!({
            "enabled": false,
            "reason": "render check applies to PPTX only",
            "status": "skipped",
        })
    };
    let (diff, changes) = if let Some(baseline) = baseline.as_deref() {
        let diff = pptx_diff(baseline, file)?;
        let changes = diff["semantic"]["textDiffs"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default();
        (Some(diff), changes)
    } else {
        (None, 0)
    };
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("rendered".to_string(), rendered);
    result.insert("schemaVersion".to_string(), json!("1.0"));
    result.insert(
        "summary".to_string(),
        json!({
            "baseline": baseline,
            "changes": changes,
            "rendered": false,
            "valid": valid,
        }),
    );
    result.insert("type".to_string(), json!(package_type));
    result.insert("valid".to_string(), json!(valid));
    result.insert("validation".to_string(), validation);
    if let Some(diff) = diff {
        result.insert("diff".to_string(), diff);
    }
    Ok(Value::Object(result))
}

#[derive(Default)]
struct ServeState {
    next_session: usize,
    sessions: BTreeMap<String, ServeSession>,
}

struct ServeSession {
    file: String,
    out: Option<String>,
    in_place: bool,
    backup: Option<String>,
    no_validate: bool,
    dry_run: bool,
    working: String,
    ops: Vec<ServeOp>,
}

#[derive(Clone)]
enum ServeOp {
    XlsxCellSet {
        command: String,
        sheet: String,
        cell: String,
        value: String,
        previous_type: String,
        previous_value: Value,
    },
    PptxReplaceText {
        command: String,
        slide: u32,
        target: String,
        text: String,
    },
    XlsxRangeSet {
        command: String,
        sheet: String,
        range: Option<String>,
        anchor: Option<String>,
        values: Option<String>,
        values_file: Option<String>,
        data_format: Option<String>,
        null_policy: Option<String>,
        ragged: Option<String>,
        max_cells: i64,
        overwrite_formulas: bool,
        readback_file: String,
        readback: Value,
    },
    XlsxRangeSetFormat {
        command: String,
        sheet: String,
        range: String,
        preset: Option<String>,
        format_code: Option<String>,
        decimals: i64,
        currency_symbol: Option<String>,
        max_cells: i64,
        readback_file: String,
        readback: Value,
    },
    XlsxWorkbookMetadataUpdate {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxHeaderFooterSetText {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxFieldsOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxBlocksOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxParagraphsOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxStylesOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxTablesOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
    DocxCommentsOp {
        command: String,
        plan_flags: Vec<Value>,
        readback_file: String,
        readback: Value,
    },
}

fn push_serve_plan_string_flag(flags: &mut Vec<Value>, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        flags.push(json!(name));
        flags.push(json!(value));
    }
}

fn push_serve_plan_bool_flag(flags: &mut Vec<Value>, name: &str, value: Option<bool>) {
    match value {
        Some(true) => flags.push(json!(name)),
        Some(false) => flags.push(json!(format!("{name}=false"))),
        None => {}
    }
}

impl ServeOp {
    fn command(&self) -> &str {
        match self {
            ServeOp::XlsxCellSet { command, .. }
            | ServeOp::PptxReplaceText { command, .. }
            | ServeOp::XlsxRangeSet { command, .. }
            | ServeOp::XlsxRangeSetFormat { command, .. }
            | ServeOp::XlsxWorkbookMetadataUpdate { command, .. }
            | ServeOp::DocxHeaderFooterSetText { command, .. }
            | ServeOp::DocxFieldsOp { command, .. }
            | ServeOp::DocxBlocksOp { command, .. }
            | ServeOp::DocxParagraphsOp { command, .. }
            | ServeOp::DocxStylesOp { command, .. }
            | ServeOp::DocxTablesOp { command, .. }
            | ServeOp::DocxCommentsOp { command, .. } => command,
        }
    }

    fn plan_argv(&self, source_file: &str) -> Value {
        match self {
            ServeOp::XlsxCellSet {
                sheet, cell, value, ..
            } => json!([
                "xlsx",
                "cells",
                "set",
                source_file,
                "--cell",
                cell,
                "--sheet",
                sheet,
                "--value",
                value,
                "--out",
                "<temp.0>",
                "--json",
                "--no-validate",
            ]),
            ServeOp::XlsxRangeSet {
                sheet,
                range,
                anchor,
                values,
                values_file,
                data_format,
                null_policy,
                ragged,
                max_cells,
                overwrite_formulas,
                ..
            } => {
                let mut argv = vec![
                    json!("xlsx"),
                    json!("ranges"),
                    json!("set"),
                    json!(source_file),
                    json!("--sheet"),
                    json!(sheet),
                ];
                if let Some(range) = range {
                    argv.push(json!("--range"));
                    argv.push(json!(range));
                }
                if let Some(anchor) = anchor {
                    argv.push(json!("--anchor"));
                    argv.push(json!(anchor));
                }
                if let Some(values) = values {
                    argv.push(json!("--values"));
                    argv.push(json!(values));
                }
                if let Some(values_file) = values_file {
                    argv.push(json!("--values-file"));
                    argv.push(json!(values_file));
                }
                if let Some(data_format) = data_format {
                    argv.push(json!("--data-format"));
                    argv.push(json!(data_format));
                }
                if let Some(null_policy) = null_policy {
                    argv.push(json!("--null-policy"));
                    argv.push(json!(null_policy));
                }
                if let Some(ragged) = ragged {
                    argv.push(json!("--ragged"));
                    argv.push(json!(ragged));
                }
                if *max_cells != 100000 {
                    argv.push(json!("--max-cells"));
                    argv.push(json!(max_cells.to_string()));
                }
                argv.push(json!("--out"));
                argv.push(json!("<temp.0>"));
                argv.push(json!("--json"));
                argv.push(json!("--no-validate"));
                if *overwrite_formulas {
                    argv.push(json!("--overwrite-formulas"));
                }
                Value::Array(argv)
            }
            ServeOp::XlsxRangeSetFormat {
                sheet,
                range,
                preset,
                format_code,
                decimals,
                currency_symbol,
                max_cells,
                ..
            } => {
                let mut argv = vec![
                    json!("xlsx"),
                    json!("ranges"),
                    json!("set-format"),
                    json!(source_file),
                    json!("--sheet"),
                    json!(sheet),
                    json!("--range"),
                    json!(range),
                ];
                if let Some(preset) = preset {
                    argv.push(json!("--preset"));
                    argv.push(json!(preset));
                }
                if let Some(format_code) = format_code {
                    argv.push(json!("--format-code"));
                    argv.push(json!(format_code));
                }
                if *decimals != 2 {
                    argv.push(json!("--decimals"));
                    argv.push(json!(decimals.to_string()));
                }
                if let Some(currency_symbol) = currency_symbol {
                    argv.push(json!("--currency-symbol"));
                    argv.push(json!(currency_symbol));
                }
                if *max_cells != 100000 {
                    argv.push(json!("--max-cells"));
                    argv.push(json!(max_cells.to_string()));
                }
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::XlsxWorkbookMetadataUpdate { plan_flags, .. } => {
                let mut argv = vec![
                    json!("xlsx"),
                    json!("workbook"),
                    json!("metadata"),
                    json!("update"),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxHeaderFooterSetText {
                command,
                plan_flags,
                ..
            } => {
                let parts = command.split_whitespace().collect::<Vec<_>>();
                let group = parts.get(1).copied().unwrap_or("headers");
                let mut argv = vec![
                    json!("docx"),
                    json!(group),
                    json!("set-text"),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxFieldsOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("set-result");
                let mut argv = vec![
                    json!("docx"),
                    json!("fields"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxBlocksOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("replace");
                let mut argv = vec![
                    json!("docx"),
                    json!("blocks"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxParagraphsOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("append");
                let mut argv = vec![
                    json!("docx"),
                    json!("paragraphs"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxStylesOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("apply");
                let mut argv = vec![
                    json!("docx"),
                    json!("styles"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([json!("--out"), json!("<temp.0>"), json!("--json")]);
                Value::Array(argv)
            }
            ServeOp::DocxTablesOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("set-cell");
                let mut argv = vec![
                    json!("docx"),
                    json!("tables"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::DocxCommentsOp {
                command,
                plan_flags,
                ..
            } => {
                let verb = command.split_whitespace().nth(2).unwrap_or("add");
                let mut argv = vec![
                    json!("docx"),
                    json!("comments"),
                    json!(verb),
                    json!(source_file),
                ];
                argv.extend(plan_flags.iter().cloned());
                argv.extend([
                    json!("--out"),
                    json!("<temp.0>"),
                    json!("--json"),
                    json!("--no-validate"),
                ]);
                Value::Array(argv)
            }
            ServeOp::PptxReplaceText {
                slide,
                target,
                text,
                ..
            } => json!([
                "pptx",
                "replace",
                "text",
                source_file,
                "--slide",
                slide.to_string(),
                "--target",
                target,
                "--text",
                text,
                "--out",
                "<temp.0>",
                "--json",
                "--no-validate",
            ]),
        }
    }

    fn readback(&self, file: &str) -> Value {
        match self {
            ServeOp::XlsxCellSet {
                cell,
                value,
                previous_type,
                previous_value,
                ..
            } => xlsx_cell_set_readback(file, cell, value, previous_type, previous_value),
            ServeOp::PptxReplaceText {
                slide,
                target,
                text,
                ..
            } => pptx_replace_text_readback(file, file, *slide, target, text),
            ServeOp::XlsxRangeSet {
                readback_file,
                readback,
                ..
            } => replace_json_string(readback.clone(), readback_file, file),
            ServeOp::XlsxRangeSetFormat {
                readback_file,
                readback,
                ..
            } => replace_json_string(readback.clone(), readback_file, file),
            ServeOp::XlsxWorkbookMetadataUpdate {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxHeaderFooterSetText {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxFieldsOp {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxBlocksOp {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxCommentsOp {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxParagraphsOp {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxStylesOp {
                readback_file,
                readback,
                ..
            }
            | ServeOp::DocxTablesOp {
                readback_file,
                readback,
                ..
            } => replace_json_string(readback.clone(), readback_file, file),
        }
    }
}

fn replace_json_string(value: Value, from: &str, to: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace(from, to)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| replace_json_string(item, from, to))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, replace_json_string(value, from, to)))
                .collect(),
        ),
        other => other,
    }
}

impl ServeState {
    fn handle_rpc(&mut self, request: Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        match self.handle_method(method, &params) {
            Ok(result) => json!({"id": id, "jsonrpc": "2.0", "result": result}),
            Err(err) => json!({
                "id": id,
                "jsonrpc": "2.0",
                "error": {
                    "code": err.exit_code,
                    "message": err.message,
                    "data": {"type": err.code, "exitCode": err.exit_code},
                },
            }),
        }
    }

    fn handle_method(&mut self, method: &str, params: &Value) -> CliResult<Value> {
        match method {
            "open" => self.serve_open(params),
            "op" => self.serve_op(params),
            "inspect" => self.serve_inspect(params),
            "validate" => self.serve_validate(params),
            "plan" => self.serve_plan(params),
            "commit" => self.serve_commit(params),
            "abort" => self.serve_abort(params),
            _ => Err(CliError::invalid_args(format!(
                "unsupported serve method: {method}"
            ))),
        }
    }

    fn serve_open(&mut self, params: &Value) -> CliResult<Value> {
        let file = json_string(params, "file")?;
        let out = json_optional_string(params, "out");
        let in_place = json_bool(params, "inPlace").unwrap_or(false);
        let backup = json_optional_string(params, "backup");
        let no_validate = json_bool(params, "noValidate").unwrap_or(false);
        let dry_run = params
            .get("dryRun")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if out.is_some() && in_place {
            return Err(CliError::invalid_args(
                "cannot specify both out and inPlace",
            ));
        }
        if backup
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
            && !in_place
        {
            return Err(CliError::invalid_args(
                "backup can only be used with inPlace",
            ));
        }
        self.next_session += 1;
        let session_id = format!("rust-session-{}", self.next_session);
        let working = make_working_copy(&file, self.next_session)?;
        self.sessions.insert(
            session_id.clone(),
            ServeSession {
                file: file.clone(),
                out,
                in_place,
                backup,
                no_validate,
                dry_run,
                working,
                ops: Vec::new(),
            },
        );
        Ok(json!({"sessionId": session_id, "type": package_type(&file)?}))
    }

    fn serve_op(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let command = json_string(params, "command")?;
        let args = params
            .get("args")
            .ok_or_else(|| CliError::invalid_args("op args are required"))?;
        let session = self.session_mut(&session_id)?;
        let op = match command.as_str() {
            "xlsx cells set" => {
                let sheet = json_string(args, "sheet")?;
                let cell = json_string(args, "cell")?;
                let value = json_string(args, "value")?;
                let previous = xlsx_cell_read(&session.working, &sheet, &cell)?;
                xlsx_set_cell_string(&session.working, &sheet, &cell, &value)?;
                ServeOp::XlsxCellSet {
                    command: command.clone(),
                    sheet,
                    cell,
                    value,
                    previous_type: previous.kind,
                    previous_value: previous.value,
                }
            }
            "xlsx ranges set" => {
                let sheet = json_string(args, "sheet")?;
                let range = json_optional_string(args, "range");
                let anchor = json_optional_string(args, "anchor");
                let values = json_optional_serialized(args, "values")?;
                let values_file = json_optional_string(args, "values-file")
                    .or_else(|| json_optional_string(args, "valuesFile"));
                let data_format = json_optional_string(args, "data-format")
                    .or_else(|| json_optional_string(args, "dataFormat"));
                let null_policy = json_optional_string(args, "null-policy")
                    .or_else(|| json_optional_string(args, "nullPolicy"));
                let ragged = json_optional_string(args, "ragged");
                let max_cells = json_i64(args, "max-cells")?
                    .or(json_i64(args, "maxCells")?)
                    .unwrap_or(100000);
                let overwrite_formulas = json_bool(args, "overwrite-formulas")
                    .or_else(|| json_bool(args, "overwriteFormulas"))
                    .unwrap_or(false);
                let readback = xlsx_ranges_set(
                    &session.working,
                    XlsxRangesSetOptions {
                        sheet: &sheet,
                        range: range.as_deref(),
                        anchor: anchor.as_deref(),
                        values: values.as_deref(),
                        values_file: values_file.as_deref(),
                        data_format: data_format.as_deref(),
                        null_policy: null_policy.as_deref(),
                        ragged: ragged.as_deref(),
                        max_cells,
                        out: None,
                        backup: None,
                        dry_run: false,
                        no_validate: true,
                        in_place: true,
                        overwrite_formulas,
                    },
                )?;
                ServeOp::XlsxRangeSet {
                    command: command.clone(),
                    sheet,
                    range,
                    anchor,
                    values,
                    values_file,
                    data_format,
                    null_policy,
                    ragged,
                    max_cells,
                    overwrite_formulas,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "xlsx ranges set-format" => {
                let sheet = json_string(args, "sheet")?;
                let range = json_string(args, "range")?;
                let preset = json_optional_string(args, "preset");
                let format_code = json_optional_string(args, "format-code")
                    .or_else(|| json_optional_string(args, "formatCode"));
                let decimals = json_i64(args, "decimals")?.unwrap_or(2);
                let currency_symbol = json_optional_string(args, "currency-symbol")
                    .or_else(|| json_optional_string(args, "currencySymbol"));
                let max_cells = json_i64(args, "max-cells")?
                    .or(json_i64(args, "maxCells")?)
                    .unwrap_or(100000);
                let readback = xlsx_ranges_set_format(
                    &session.working,
                    XlsxRangesSetFormatOptions {
                        sheet: &sheet,
                        range: &range,
                        preset: preset.as_deref(),
                        format_code: format_code.as_deref(),
                        decimals,
                        currency_symbol: currency_symbol.as_deref(),
                        max_cells,
                        out: None,
                        backup: None,
                        dry_run: false,
                        no_validate: true,
                        in_place: true,
                    },
                )?;
                ServeOp::XlsxRangeSetFormat {
                    command: command.clone(),
                    sheet,
                    range,
                    preset,
                    format_code,
                    decimals,
                    currency_symbol,
                    max_cells,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "xlsx workbook metadata update" => {
                let title = json_optional_string(args, "title");
                let subject = json_optional_string(args, "subject");
                let creator = json_optional_string(args, "creator");
                let keywords = json_optional_string(args, "keywords");
                let description = json_optional_string(args, "description");
                let last_modified_by = json_optional_string(args, "last-modified-by")
                    .or_else(|| json_optional_string(args, "lastModifiedBy"));
                let category = json_optional_string(args, "category");
                let company = json_optional_string(args, "company");
                let manager = json_optional_string(args, "manager");
                let calc_mode = json_optional_string(args, "calc-mode")
                    .or_else(|| json_optional_string(args, "calcMode"));
                let full_calc_on_load = json_bool(args, "full-calc-on-load")
                    .or_else(|| json_bool(args, "fullCalcOnLoad"));
                let expect_title = json_optional_string(args, "expect-title")
                    .or_else(|| json_optional_string(args, "expectTitle"));
                let expect_subject = json_optional_string(args, "expect-subject")
                    .or_else(|| json_optional_string(args, "expectSubject"));
                let expect_creator = json_optional_string(args, "expect-creator")
                    .or_else(|| json_optional_string(args, "expectCreator"));
                let expect_keywords = json_optional_string(args, "expect-keywords")
                    .or_else(|| json_optional_string(args, "expectKeywords"));
                let expect_description = json_optional_string(args, "expect-description")
                    .or_else(|| json_optional_string(args, "expectDescription"));
                let expect_last_modified_by = json_optional_string(args, "expect-last-modified-by")
                    .or_else(|| json_optional_string(args, "expectLastModifiedBy"));
                let expect_category = json_optional_string(args, "expect-category")
                    .or_else(|| json_optional_string(args, "expectCategory"));
                let expect_company = json_optional_string(args, "expect-company")
                    .or_else(|| json_optional_string(args, "expectCompany"));
                let expect_manager = json_optional_string(args, "expect-manager")
                    .or_else(|| json_optional_string(args, "expectManager"));
                let readback = xlsx_workbook_metadata_update(
                    &session.working,
                    XlsxWorkbookMetadataUpdateOptions {
                        title: title.as_deref(),
                        subject: subject.as_deref(),
                        creator: creator.as_deref(),
                        keywords: keywords.as_deref(),
                        description: description.as_deref(),
                        last_modified_by: last_modified_by.as_deref(),
                        category: category.as_deref(),
                        company: company.as_deref(),
                        manager: manager.as_deref(),
                        calc_mode: calc_mode.as_deref(),
                        full_calc_on_load,
                        expect_title: expect_title.as_deref(),
                        expect_subject: expect_subject.as_deref(),
                        expect_creator: expect_creator.as_deref(),
                        expect_keywords: expect_keywords.as_deref(),
                        expect_description: expect_description.as_deref(),
                        expect_last_modified_by: expect_last_modified_by.as_deref(),
                        expect_category: expect_category.as_deref(),
                        expect_company: expect_company.as_deref(),
                        expect_manager: expect_manager.as_deref(),
                        out: None,
                        backup: None,
                        dry_run: false,
                        no_validate: true,
                        in_place: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                push_serve_plan_string_flag(&mut plan_flags, "--title", title.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--subject", subject.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--creator", creator.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--keywords", keywords.as_deref());
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--description",
                    description.as_deref(),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--last-modified-by",
                    last_modified_by.as_deref(),
                );
                push_serve_plan_string_flag(&mut plan_flags, "--category", category.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--company", company.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--manager", manager.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--calc-mode", calc_mode.as_deref());
                push_serve_plan_bool_flag(
                    &mut plan_flags,
                    "--full-calc-on-load",
                    full_calc_on_load,
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-title",
                    expect_title.as_deref(),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-subject",
                    expect_subject.as_deref(),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-creator",
                    expect_creator.as_deref(),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-keywords",
                    expect_keywords.as_deref(),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-description",
                    expect_description.as_deref(),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-last-modified-by",
                    expect_last_modified_by.as_deref(),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-category",
                    expect_category.as_deref(),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-company",
                    expect_company.as_deref(),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-manager",
                    expect_manager.as_deref(),
                );
                ServeOp::XlsxWorkbookMetadataUpdate {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx headers set-text" | "docx footers set-text" => {
                let kind = if command.contains("footers") {
                    "footer"
                } else {
                    "header"
                };
                let id = json_optional_string(args, "id").unwrap_or_default();
                let ref_type =
                    json_optional_string(args, "type").unwrap_or_else(|| "default".to_string());
                let ref_type = normalize_docx_header_footer_show_type(&ref_type)?;
                let section_value = json_i64(args, "section")?;
                let section = section_value.unwrap_or(0);
                let index_value = json_i64(args, "index")?;
                let index = index_value.unwrap_or(1);
                let selector = json_optional_string(args, "selector");
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let text_set = args.get("text").is_some();
                let text_file_set =
                    args.get("text-file").is_some() || args.get("textFile").is_some();
                let text = resolve_required_docx_table_text(
                    text.as_deref(),
                    text_file.as_deref(),
                    text_set,
                    text_file_set,
                )?;
                let readback = docx_headers_footers_set_text(
                    &session.working,
                    kind,
                    DocxHeaderFooterSetTextOptions {
                        id: &id,
                        ref_type: &ref_type,
                        section,
                        index,
                        selector: selector.as_deref(),
                        selector_given: selector.is_some(),
                        index_given: index_value.is_some(),
                        text: &text,
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--id",
                    (!id.is_empty()).then_some(id.as_str()),
                );
                if args.get("type").is_some() {
                    push_serve_plan_string_flag(&mut plan_flags, "--type", Some(ref_type.as_str()));
                }
                if let Some(section) = section_value {
                    plan_flags.push(json!("--section"));
                    plan_flags.push(json!(section.to_string()));
                }
                if let Some(index) = index_value {
                    plan_flags.push(json!("--index"));
                    plan_flags.push(json!(index.to_string()));
                }
                push_serve_plan_string_flag(&mut plan_flags, "--selector", selector.as_deref());
                if text_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--text", Some(text.as_str()));
                }
                if text_file_set {
                    push_serve_plan_string_flag(
                        &mut plan_flags,
                        "--text-file",
                        text_file.as_deref(),
                    );
                }
                ServeOp::DocxHeaderFooterSetText {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx fields insert" => {
                let location = json_string(args, "location")?;
                let field_code = json_optional_string(args, "field-code")
                    .or_else(|| json_optional_string(args, "fieldCode"))
                    .ok_or_else(|| CliError::invalid_args("field-code is required"))?;
                let result = json_optional_string(args, "result").unwrap_or_default();
                let result_set = args.get("result").is_some();
                let readback = docx_fields_insert(
                    &session.working,
                    &location,
                    &field_code,
                    &result,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                push_serve_plan_string_flag(&mut plan_flags, "--location", Some(&location));
                push_serve_plan_string_flag(&mut plan_flags, "--field-code", Some(&field_code));
                if result_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--result", Some(&result));
                }
                ServeOp::DocxFieldsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx fields set-result" => {
                let selector = json_string(args, "selector")?;
                if args.get("result").is_none() {
                    return Err(CliError::invalid_args("result is required"));
                }
                let result = json_optional_string(args, "result").unwrap_or_default();
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                let readback = docx_fields_set_result(
                    &session.working,
                    &selector,
                    &result,
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                push_serve_plan_string_flag(&mut plan_flags, "--selector", Some(&selector));
                push_serve_plan_string_flag(&mut plan_flags, "--result", Some(&result));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    (!expect_hash.is_empty()).then_some(expect_hash.as_str()),
                );
                ServeOp::DocxFieldsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx paragraphs append" => {
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let style = json_optional_string(args, "style").unwrap_or_default();
                let readback = docx_paragraphs_append(
                    &session.working,
                    DocxParagraphMutationOptions {
                        text: text.as_deref(),
                        text_file: text_file.as_deref(),
                        style: &style,
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--style",
                    (!style.is_empty()).then_some(style.as_str()),
                );
                ServeOp::DocxParagraphsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx paragraphs insert" => {
                let insert_after = match json_i64(args, "insert-after")? {
                    Some(value) => value,
                    None => json_i64(args, "insertAfter")?.unwrap_or(0),
                };
                if insert_after < 0 {
                    return Err(CliError::invalid_args("--insert-after must be >= 0"));
                }
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let style = json_optional_string(args, "style").unwrap_or_default();
                let readback = docx_paragraphs_insert(
                    &session.working,
                    insert_after,
                    DocxParagraphMutationOptions {
                        text: text.as_deref(),
                        text_file: text_file.as_deref(),
                        style: &style,
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = vec![json!("--insert-after"), json!(insert_after.to_string())];
                push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--style",
                    (!style.is_empty()).then_some(style.as_str()),
                );
                ServeOp::DocxParagraphsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx paragraphs set" => {
                let handle_set = args.get("handle").is_some();
                let index_set = args.get("index").is_some();
                if handle_set && index_set {
                    return Err(CliError::invalid_args(
                        "cannot specify both --index and --handle",
                    ));
                }
                let index = json_i64(args, "index")?.unwrap_or(0);
                if !handle_set && index < 1 {
                    return Err(CliError::invalid_args(
                        "--index must be >= 1 (or pass --handle)",
                    ));
                }
                let text_set = args.get("text").is_some();
                let text_file_set =
                    args.get("text-file").is_some() || args.get("textFile").is_some();
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let resolved_text = resolve_required_docx_paragraph_set_text(
                    text.as_deref(),
                    text_file.as_deref(),
                    text_set,
                    text_file_set,
                )?;
                let handle = json_optional_string(args, "handle");
                let readback = docx_paragraphs_set(
                    &session.working,
                    index,
                    handle.as_deref(),
                    &resolved_text,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                if handle_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
                } else {
                    plan_flags.push(json!("--index"));
                    plan_flags.push(json!(index.to_string()));
                }
                if text_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                }
                if text_file_set {
                    push_serve_plan_string_flag(
                        &mut plan_flags,
                        "--text-file",
                        text_file.as_deref(),
                    );
                }
                ServeOp::DocxParagraphsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx paragraphs clear" => {
                let handle_set = args.get("handle").is_some();
                let index_set = args.get("index").is_some();
                if handle_set && index_set {
                    return Err(CliError::invalid_args(
                        "cannot specify both --index and --handle",
                    ));
                }
                let index = json_i64(args, "index")?.unwrap_or(0);
                if !handle_set && index < 1 {
                    return Err(CliError::invalid_args(
                        "--index must be >= 1 (or pass --handle)",
                    ));
                }
                let handle = json_optional_string(args, "handle");
                let readback = docx_paragraphs_clear(
                    &session.working,
                    index,
                    handle.as_deref(),
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                if handle_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
                } else {
                    plan_flags.push(json!("--index"));
                    plan_flags.push(json!(index.to_string()));
                }
                ServeOp::DocxParagraphsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx styles apply" => {
                let handle_set = args.get("handle").is_some();
                let index_set = args.get("index").is_some();
                if handle_set && index_set {
                    return Err(CliError::invalid_args(
                        "cannot specify both --index and --handle",
                    ));
                }
                let index = json_i64(args, "index")?.unwrap_or(0);
                if !handle_set && index < 1 {
                    return Err(CliError::invalid_args(
                        "--index must be >= 1 (or pass --handle)",
                    ));
                }
                let handle = json_optional_string(args, "handle");
                let target_arg = json_optional_string(args, "target").unwrap_or_default();
                let target = normalize_docx_style_target(&target_arg)?;
                if handle_set && target == DocxStyleTarget::Table {
                    return Err(CliError::invalid_args(
                        "--handle is a paragraph handle; use --index with --target table",
                    ));
                }
                let style = json_optional_string(args, "style").unwrap_or_default();
                if style.trim().is_empty() {
                    return Err(CliError::invalid_args("--style is required"));
                }
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                if !expect_hash.is_empty() {
                    require_docx_block_hash(&expect_hash)?;
                }
                let skip_style_validation = json_bool(args, "no-validate")
                    .or_else(|| json_bool(args, "noValidate"))
                    .unwrap_or(false);
                let readback = docx_styles_apply(
                    &session.working,
                    DocxStyleApplyOptions {
                        index,
                        handle: handle.as_deref(),
                        target,
                        style: &style,
                        expected_hash: &expect_hash,
                        validate_style: !skip_style_validation,
                        mutation: DocxParagraphMutationOptions {
                            text: None,
                            text_file: None,
                            style: "",
                            out: None,
                            backup: None,
                            dry_run: false,
                            in_place: true,
                            no_validate: true,
                        },
                    },
                )?;
                let mut plan_flags = Vec::new();
                if handle_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
                } else {
                    plan_flags.push(json!("--index"));
                    plan_flags.push(json!(index.to_string()));
                }
                push_serve_plan_string_flag(&mut plan_flags, "--target", Some(target.as_str()));
                push_serve_plan_string_flag(&mut plan_flags, "--style", Some(style.as_str()));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    (!expect_hash.is_empty()).then_some(expect_hash.as_str()),
                );
                push_serve_plan_bool_flag(
                    &mut plan_flags,
                    "--no-validate",
                    skip_style_validation.then_some(true),
                );
                ServeOp::DocxStylesOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx blocks replace" => {
                let block = json_i64(args, "block")?
                    .ok_or_else(|| CliError::invalid_args("block is required"))?;
                if block < 1 {
                    return Err(CliError::invalid_args("--block must be >= 1"));
                }
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                require_docx_block_hash(&expect_hash)?;
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let style = json_optional_string(args, "style").unwrap_or_default();
                let readback = docx_blocks_replace(
                    &session.working,
                    block as usize,
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: text.as_deref(),
                        text_file: text_file.as_deref(),
                        style: &style,
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                plan_flags.push(json!("--block"));
                plan_flags.push(json!(block.to_string()));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    Some(expect_hash.as_str()),
                );
                push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--style",
                    (!style.is_empty()).then_some(style.as_str()),
                );
                ServeOp::DocxBlocksOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx blocks delete" => {
                let block = json_i64(args, "block")?
                    .ok_or_else(|| CliError::invalid_args("block is required"))?;
                if block < 1 {
                    return Err(CliError::invalid_args("--block must be >= 1"));
                }
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                require_docx_block_hash(&expect_hash)?;
                let readback = docx_blocks_delete(
                    &session.working,
                    block as usize,
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                plan_flags.push(json!("--block"));
                plan_flags.push(json!(block.to_string()));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    Some(expect_hash.as_str()),
                );
                ServeOp::DocxBlocksOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx blocks insert-after" => {
                let block = json_i64(args, "block")?.unwrap_or(0);
                if block < 0 {
                    return Err(CliError::invalid_args("--block must be >= 0"));
                }
                let expect_hash_set =
                    args.get("expect-hash").is_some() || args.get("expectHash").is_some();
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                if block > 0 {
                    require_docx_block_hash(&expect_hash)?;
                } else if expect_hash_set {
                    return Err(CliError::invalid_args(
                        "--expect-hash cannot be used with --block 0",
                    ));
                }
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let style = json_optional_string(args, "style").unwrap_or_default();
                let readback = docx_blocks_insert_after(
                    &session.working,
                    block as usize,
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: text.as_deref(),
                        text_file: text_file.as_deref(),
                        style: &style,
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                plan_flags.push(json!("--block"));
                plan_flags.push(json!(block.to_string()));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    (!expect_hash.is_empty()).then_some(expect_hash.as_str()),
                );
                push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--style",
                    (!style.is_empty()).then_some(style.as_str()),
                );
                ServeOp::DocxBlocksOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx comments add" => {
                let anchor_block = match json_i64(args, "anchor-block")? {
                    Some(value) => value,
                    None => json_i64(args, "anchorBlock")?.unwrap_or(0),
                };
                if (args.get("anchor-block").is_some() || args.get("anchorBlock").is_some())
                    && anchor_block < 1
                {
                    return Err(CliError::invalid_args("--anchor-block must be >= 1"));
                }
                let author = json_optional_string(args, "author").unwrap_or_default();
                if author.is_empty() {
                    return Err(CliError::invalid_args("--author is required"));
                }
                let initials = json_optional_string(args, "initials").unwrap_or_default();
                let date = json_optional_string(args, "date").unwrap_or_else(current_utc_rfc3339);
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let readback = docx_comments_add(
                    &session.working,
                    anchor_block,
                    &author,
                    &initials,
                    &date,
                    DocxParagraphMutationOptions {
                        text: text.as_deref(),
                        text_file: text_file.as_deref(),
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                if anchor_block > 0 {
                    plan_flags.push(json!("--anchor-block"));
                    plan_flags.push(json!(anchor_block.to_string()));
                }
                push_serve_plan_string_flag(&mut plan_flags, "--author", Some(&author));
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--initials",
                    (!initials.is_empty()).then_some(initials.as_str()),
                );
                push_serve_plan_string_flag(&mut plan_flags, "--date", Some(&date));
                push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
                ServeOp::DocxCommentsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx comments edit" => {
                let comment_id_set =
                    args.get("comment-id").is_some() || args.get("commentId").is_some();
                let handle_set = args.get("handle").is_some();
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
                let comment_id = match json_i64(args, "comment-id")? {
                    Some(value) => value,
                    None => json_i64(args, "commentId")?.unwrap_or(0),
                };
                if !handle_set && comment_id < 0 {
                    return Err(CliError::invalid_args("--comment-id must be >= 0"));
                }
                let text_set = args.get("text").is_some();
                let text_file_set =
                    args.get("text-file").is_some() || args.get("textFile").is_some();
                let author_set = args.get("author").is_some();
                let date_set = args.get("date").is_some();
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
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let resolved_text = if text_file_set {
                    let path = text_file.as_deref().unwrap_or_default();
                    fs::read(path)
                        .map(|data| String::from_utf8_lossy(&data).to_string())
                        .map_err(|_| CliError::file_not_found(format!("file not found: {path}")))?
                } else {
                    text.clone().unwrap_or_default()
                };
                let handle = json_optional_string(args, "handle");
                let author = json_optional_string(args, "author").unwrap_or_default();
                let date = json_optional_string(args, "date").unwrap_or_default();
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                let readback = docx_comments_edit(
                    &session.working,
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
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                if handle_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
                } else {
                    plan_flags.push(json!("--comment-id"));
                    plan_flags.push(json!(comment_id.to_string()));
                }
                if text_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--text", text.as_deref());
                }
                if text_file_set {
                    push_serve_plan_string_flag(
                        &mut plan_flags,
                        "--text-file",
                        text_file.as_deref(),
                    );
                }
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--author",
                    author_set.then_some(author.as_str()),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--date",
                    date_set.then_some(date.as_str()),
                );
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    (!expect_hash.is_empty()).then_some(expect_hash.as_str()),
                );
                ServeOp::DocxCommentsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx comments remove" => {
                let comment_id_set =
                    args.get("comment-id").is_some() || args.get("commentId").is_some();
                let handle_set = args.get("handle").is_some();
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
                let comment_id = match json_i64(args, "comment-id")? {
                    Some(value) => value,
                    None => json_i64(args, "commentId")?.unwrap_or(0),
                };
                if !handle_set && comment_id < 0 {
                    return Err(CliError::invalid_args("--comment-id must be >= 0"));
                }
                let handle = json_optional_string(args, "handle");
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                let readback = docx_comments_remove(
                    &session.working,
                    comment_id,
                    handle.as_deref(),
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = Vec::new();
                if handle_set {
                    push_serve_plan_string_flag(&mut plan_flags, "--handle", handle.as_deref());
                } else {
                    plan_flags.push(json!("--comment-id"));
                    plan_flags.push(json!(comment_id.to_string()));
                }
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    (!expect_hash.is_empty()).then_some(expect_hash.as_str()),
                );
                ServeOp::DocxCommentsOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx tables set-cell" => {
                let table = json_i64(args, "table")?
                    .ok_or_else(|| CliError::invalid_args("table is required"))?;
                let row = json_i64(args, "row")?
                    .ok_or_else(|| CliError::invalid_args("row is required"))?;
                let col = json_i64(args, "col")?
                    .ok_or_else(|| CliError::invalid_args("col is required"))?;
                validate_positive_i64(table, "--table")?;
                validate_positive_i64(row, "--row")?;
                validate_positive_i64(col, "--col")?;
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                require_docx_block_hash(&expect_hash)?;
                let text_changed = args.get("text").is_some();
                let text_file_changed =
                    args.get("text-file").is_some() || args.get("textFile").is_some();
                let text = json_optional_string(args, "text");
                let text_file = json_optional_string(args, "text-file")
                    .or_else(|| json_optional_string(args, "textFile"));
                let resolved_text = resolve_required_docx_table_text(
                    text.as_deref(),
                    text_file.as_deref(),
                    text_changed,
                    text_file_changed,
                )?;
                let readback = docx_tables_set_cell(
                    &session.working,
                    table as usize,
                    row as usize,
                    col as usize,
                    &expect_hash,
                    &resolved_text,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = vec![
                    json!("--table"),
                    json!(table.to_string()),
                    json!("--row"),
                    json!(row.to_string()),
                    json!("--col"),
                    json!(col.to_string()),
                ];
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    Some(expect_hash.as_str()),
                );
                if text_changed {
                    push_serve_plan_string_flag(
                        &mut plan_flags,
                        "--text",
                        Some(resolved_text.as_str()),
                    );
                }
                if text_file_changed {
                    push_serve_plan_string_flag(
                        &mut plan_flags,
                        "--text-file",
                        text_file.as_deref(),
                    );
                }
                ServeOp::DocxTablesOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "docx tables clear-cell" => {
                let table = json_i64(args, "table")?
                    .ok_or_else(|| CliError::invalid_args("table is required"))?;
                let row = json_i64(args, "row")?
                    .ok_or_else(|| CliError::invalid_args("row is required"))?;
                let col = json_i64(args, "col")?
                    .ok_or_else(|| CliError::invalid_args("col is required"))?;
                validate_positive_i64(table, "--table")?;
                validate_positive_i64(row, "--row")?;
                validate_positive_i64(col, "--col")?;
                let expect_hash = json_optional_string(args, "expect-hash")
                    .or_else(|| json_optional_string(args, "expectHash"))
                    .unwrap_or_default();
                require_docx_block_hash(&expect_hash)?;
                let readback = docx_tables_clear_cell(
                    &session.working,
                    table as usize,
                    row as usize,
                    col as usize,
                    &expect_hash,
                    DocxParagraphMutationOptions {
                        text: None,
                        text_file: None,
                        style: "",
                        out: None,
                        backup: None,
                        dry_run: false,
                        in_place: true,
                        no_validate: true,
                    },
                )?;
                let mut plan_flags = vec![
                    json!("--table"),
                    json!(table.to_string()),
                    json!("--row"),
                    json!(row.to_string()),
                    json!("--col"),
                    json!(col.to_string()),
                ];
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--expect-hash",
                    Some(expect_hash.as_str()),
                );
                ServeOp::DocxTablesOp {
                    command: command.clone(),
                    plan_flags,
                    readback_file: session.working.clone(),
                    readback,
                }
            }
            "pptx replace text" => {
                let slide = json_u32(args, "slide")?.unwrap_or(1);
                let target = json_string(args, "target")?;
                let text = json_string(args, "text")?;
                pptx_replace_text_in_place(&session.working, slide, &target, &text)?;
                ServeOp::PptxReplaceText {
                    command: command.clone(),
                    slide,
                    target,
                    text,
                }
            }
            _ => {
                return Err(CliError::invalid_args(format!(
                    "unsupported serve op command: {command}"
                )));
            }
        };
        let readback = op.readback(&session.working);
        let index = session.ops.len();
        session.ops.push(op);
        Ok(json!({"command": command, "index": index, "readback": readback}))
    }

    fn serve_inspect(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let command = json_string(params, "command")?;
        let args = params
            .get("args")
            .ok_or_else(|| CliError::invalid_args("inspect args are required"))?;
        let session = self.session(&session_id)?;
        match command.as_str() {
            "xlsx ranges export" => {
                let sheet = json_string(args, "sheet")?;
                let range = json_string(args, "range")?;
                let data_format = json_optional_string(args, "data-format")
                    .or_else(|| json_optional_string(args, "dataFormat"));
                require_json_data_format(data_format.as_deref())?;
                let data_out = json_optional_string(args, "data-out")
                    .or_else(|| json_optional_string(args, "dataOut"));
                let max_cells = json_i64(args, "max-cells")?
                    .or(json_i64(args, "maxCells")?)
                    .unwrap_or(100000);
                let include_types = json_bool(args, "include-types")
                    .or_else(|| json_bool(args, "includeTypes"))
                    .unwrap_or(false);
                let include_formulas = json_bool(args, "include-formulas")
                    .or_else(|| json_bool(args, "includeFormulas"))
                    .unwrap_or(false);
                let include_formats = json_bool(args, "include-formats")
                    .or_else(|| json_bool(args, "includeFormats"))
                    .unwrap_or(false);
                xlsx_range_export_with_options(
                    &session.working,
                    &sheet,
                    &range,
                    XlsxRangeExportOptions {
                        include_types,
                        include_formulas,
                        include_formats,
                        data_out: data_out.as_deref(),
                        max_cells,
                    },
                )
            }
            "xlsx cells extract" => {
                let sheet = json_optional_string(args, "sheet").unwrap_or_else(|| "1".to_string());
                let range = json_optional_string(args, "range");
                let max_rows = json_u32(args, "max-rows")?
                    .or(json_u32(args, "maxRows")?)
                    .unwrap_or(1000);
                let max_cells = json_u32(args, "max-cells")?
                    .or(json_u32(args, "maxCells")?)
                    .unwrap_or(0);
                let include_empty = json_bool(args, "include-empty")
                    .or_else(|| json_bool(args, "includeEmpty"))
                    .unwrap_or(false);
                xlsx_cells_extract(
                    &session.working,
                    &sheet,
                    range.as_deref(),
                    max_rows,
                    max_cells,
                    include_empty,
                )
            }
            "xlsx sheets list" => xlsx_sheets_list(&session.working),
            "xlsx sheets show" => {
                let sheet = json_optional_string(args, "sheet");
                xlsx_sheets_show(&session.working, sheet.as_deref())
            }
            "xlsx names list" => {
                let scope_sheet = json_optional_string(args, "scope-sheet")
                    .or_else(|| json_optional_string(args, "scopeSheet"));
                xlsx_names_list(&session.working, scope_sheet.as_deref())
            }
            "xlsx names show" => {
                let name = json_string(args, "name")?;
                let scope_sheet = json_optional_string(args, "scope-sheet")
                    .or_else(|| json_optional_string(args, "scopeSheet"));
                xlsx_names_show(&session.working, &name, scope_sheet.as_deref())
            }
            "xlsx tables list" => {
                let sheet = json_optional_string(args, "sheet");
                xlsx_tables_list(&session.working, sheet.as_deref())
            }
            "xlsx tables show" => {
                let sheet = json_optional_string(args, "sheet");
                let table = json_optional_string(args, "table");
                xlsx_tables_show(&session.working, sheet.as_deref(), table.as_deref())
            }
            "xlsx tables export" => {
                let sheet = json_optional_string(args, "sheet");
                let table = json_optional_string(args, "table");
                let data_format = json_optional_string(args, "data-format")
                    .or_else(|| json_optional_string(args, "dataFormat"));
                let data_out = json_optional_string(args, "data-out")
                    .or_else(|| json_optional_string(args, "dataOut"));
                let max_cells = json_i64(args, "max-cells")?
                    .or(json_i64(args, "maxCells")?)
                    .unwrap_or(100000);
                let include_types = json_bool(args, "include-types")
                    .or_else(|| json_bool(args, "includeTypes"))
                    .unwrap_or(false);
                let include_formulas = json_bool(args, "include-formulas")
                    .or_else(|| json_bool(args, "includeFormulas"))
                    .unwrap_or(false);
                xlsx_tables_export(
                    &session.working,
                    sheet.as_deref(),
                    table.as_deref(),
                    XlsxTableExportOptions {
                        data_format: data_format.as_deref(),
                        data_out: data_out.as_deref(),
                        max_cells,
                        include_types,
                        include_formulas,
                    },
                )
            }
            "xlsx workbook metadata inspect" => xlsx_workbook_metadata_inspect(&session.working),
            "docx text" => docx_text(&session.working),
            "docx fields list" => {
                let field_type = json_optional_string(args, "type");
                docx_fields_list(&session.working, field_type.as_deref())
            }
            "docx headers list" | "docx footers list" => {
                docx_headers_footers_list(&session.working)
            }
            "docx headers show" | "docx footers show" => {
                let group = if command.starts_with("docx footers") {
                    "footers"
                } else {
                    "headers"
                };
                let rest = docx_header_footer_show_json_args(args)?;
                docx_headers_footers_show(&session.working, docx_header_footer_kind(group), &rest)
            }
            "docx images list" => docx_images_list(&session.working),
            "docx comments list" => {
                let comment_id = match json_i64(args, "comment-id")? {
                    Some(value) => Some(value),
                    None => json_i64(args, "commentId")?,
                };
                if let Some(comment_id) = comment_id
                    && comment_id < 0
                {
                    return Err(CliError::invalid_args("--comment-id must be >= 0"));
                }
                docx_comments_list(&session.working, comment_id)
            }
            "docx blocks" => {
                let block = json_i64(args, "block")?.unwrap_or(0);
                if block < 0 {
                    return Err(CliError::invalid_args("--block must be >= 0"));
                }
                let include_runs = json_bool(args, "include-runs")
                    .or_else(|| json_bool(args, "includeRuns"))
                    .unwrap_or(false);
                docx_blocks_show(&session.working, block as usize, include_runs)
            }
            "docx styles list" => {
                let style_type = json_optional_string(args, "type");
                docx_styles_list(&session.working, style_type.as_deref())
            }
            "docx styles show" => {
                let style_id = json_string(args, "style")?;
                docx_styles_show(&session.working, &style_id)
            }
            "docx tables show" => {
                let table = json_i64(args, "table")?.unwrap_or(0);
                if table < 0 {
                    return Err(CliError::invalid_args("--table must be >= 0"));
                }
                let details = json_bool(args, "details")
                    .or_else(|| json_bool(args, "includeDetails"))
                    .unwrap_or(false);
                docx_tables_show(&session.working, table as usize, details)
            }
            "pptx slides list" => pptx_slides_list(&session.working),
            "pptx slides selectors" => {
                let slide = json_u32(args, "slide")?
                    .ok_or_else(|| CliError::invalid_args("slide is required"))?;
                pptx_slide_selectors(&session.working, slide)
            }
            "pptx slides show" => {
                let slide = json_u32(args, "slide")?.unwrap_or(1);
                pptx_slide_show(&session.working, slide)
            }
            "pptx extract text" => {
                let rest = pptx_extract_text_json_args(args)?;
                pptx_extract_text(&session.working, &rest)
            }
            "pptx extract notes" => {
                let rest = pptx_extract_text_json_args(args)?;
                pptx_extract_notes(&session.working, &rest)
            }
            "pptx notes show" => {
                let slide = json_u32(args, "slide")?
                    .ok_or_else(|| CliError::invalid_args("slide is required"))?;
                pptx_notes_show(&session.working, slide)
            }
            "pptx comments list" => {
                let slide = json_u32(args, "slide")?;
                let comment_id = match json_i64(args, "comment-id")? {
                    Some(value) => Some(value),
                    None => json_i64(args, "commentId")?,
                };
                if comment_id.is_some() && slide.is_none() {
                    return Err(CliError::invalid_args("--comment-id requires --slide"));
                }
                pptx_comments_list(&session.working, slide, comment_id)
            }
            "pptx masters list" => pptx_masters_list(&session.working),
            "pptx masters show" => {
                let master = json_u32(args, "master")?.unwrap_or(1) as i64;
                pptx_masters_show(&session.working, master)
            }
            "pptx layouts list" => {
                let master = json_u32(args, "master")?;
                pptx_layouts_list(&session.working, master)
            }
            "pptx layouts show" => {
                let layout = json_string(args, "layout")?;
                pptx_layouts_show(&session.working, &layout)
            }
            "pptx tables show" => {
                let slide = json_u32(args, "slide")?
                    .ok_or_else(|| CliError::invalid_args("slide is required"))?;
                let table_id = json_u32(args, "table-id")?
                    .or(json_u32(args, "tableId")?)
                    .unwrap_or(0);
                let target = json_optional_string(args, "target");
                let details = json_bool(args, "details").unwrap_or(false);
                if table_id > 0 && target.as_deref().unwrap_or_default() != "" {
                    return Err(CliError::invalid_args(
                        "specify only one of --target or --table-id",
                    ));
                }
                pptx_tables_show(
                    &session.working,
                    slide,
                    table_id,
                    target.as_deref(),
                    details,
                )
            }
            "pptx shapes show" => {
                let slide = json_u32(args, "slide")?
                    .ok_or_else(|| CliError::invalid_args("slide is required"))?;
                let include_text = json_bool(args, "include-text")
                    .or_else(|| json_bool(args, "includeText"))
                    .unwrap_or(false);
                let include_bounds = json_bool(args, "include-bounds")
                    .or_else(|| json_bool(args, "includeBounds"))
                    .unwrap_or(false);
                pptx_shapes_show(&session.working, slide, include_text, include_bounds)
            }
            _ => Err(CliError::invalid_args(format!(
                "unsupported serve inspect command: {command}"
            ))),
        }
    }

    fn serve_validate(&self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let session = self.session(&session_id)?;
        let report = validate(&session.working, true)?;
        Ok(json!({
            "diagnostics": report
                .get("diagnostics")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
        }))
    }

    fn serve_plan(&self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let session = self.session(&session_id)?;
        let plan: Vec<Value> = session
            .ops
            .iter()
            .enumerate()
            .map(|(index, op)| {
                json!({
                    "argv": op.plan_argv(&session.file),
                    "command": op.command(),
                    "index": index,
                })
            })
            .collect();
        Ok(json!({
            "dryRun": session.dry_run,
            "file": session.file,
            "opsCount": session.ops.len(),
            "plan": plan,
            "schemaVersion": 1,
        }))
    }

    fn serve_commit(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let session = self.session(&session_id)?;
        let output = if session.in_place {
            session.file.clone()
        } else {
            session
                .out
                .clone()
                .ok_or_else(|| CliError::invalid_args("commit requires an output path"))?
        };
        if !session.dry_run {
            if !session.no_validate {
                let validation = validate(&session.working, true)?;
                if validate_exit_code(&validation, true) != EXIT_SUCCESS {
                    return Err(CliError::validation_failed(format!(
                        "validation failed for working copy: {}",
                        serde_json::to_string(&validation).expect("serialize validation")
                    )));
                }
            }
            if session.in_place
                && let Some(backup_path) = session
                    .backup
                    .as_ref()
                    .filter(|value| !value.trim().is_empty())
            {
                if let Some(parent) = Path::new(backup_path).parent() {
                    fs::create_dir_all(parent)
                        .map_err(|err| CliError::unexpected(err.to_string()))?;
                }
                fs::copy(&session.file, backup_path).map_err(|err| {
                    CliError::unexpected(format!("failed to create backup: {err}"))
                })?;
            }
            if let Some(parent) = Path::new(&output).parent() {
                fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
            }
            fs::copy(&session.working, &output)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        }
        let readback_file = if session.dry_run {
            &session.working
        } else {
            &output
        };
        let applied: Vec<Value> = session
            .ops
            .iter()
            .enumerate()
            .map(|(index, op)| {
                json!({
                    "command": op.command(),
                    "index": index,
                    "readback": op.readback(readback_file),
                })
            })
            .collect();
        let mut result = json!({
            "applied": applied,
            "dryRun": session.dry_run,
            "file": session.file,
            "opsCount": session.ops.len(),
            "output": if session.dry_run { Value::Null } else { json!(output.clone()) },
            "schemaVersion": 1,
            "validateCommand": if session.dry_run {
                Value::Null
            } else {
                json!(format!("ooxml validate --strict {output}"))
            },
        });
        if session.dry_run
            && let Value::Object(ref mut object) = result
        {
            object.insert("committed".to_string(), json!(false));
            object.insert("plannedOutput".to_string(), json!(output));
        }
        Ok(result)
    }

    fn serve_abort(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        self.sessions
            .remove(&session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))?;
        Ok(json!({"aborted": true}))
    }

    fn session(&self, session_id: &str) -> CliResult<&ServeSession> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))
    }

    fn session_mut(&mut self, session_id: &str) -> CliResult<&mut ServeSession> {
        self.sessions
            .get_mut(session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))
    }
}

fn pptx_replace_text(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_u32_flag(args, "--slide")?.unwrap_or(1);
    let target = parse_string_flag(args, "--target")?
        .ok_or_else(|| CliError::invalid_args("--target is required"))?;
    let new_text = parse_string_flag(args, "--text")?
        .ok_or_else(|| CliError::invalid_args("--text is required"))?;
    let out = parse_string_flag(args, "--out")?
        .ok_or_else(|| CliError::invalid_args("--out is required"))?;
    pptx_replace_text_to(file, &out, slide, &target, &new_text)
}

fn pptx_replace_text_to(
    file: &str,
    out: &str,
    slide: u32,
    target: &str,
    new_text: &str,
) -> CliResult<Value> {
    if slide != 1 || target != "title" {
        return Err(CliError::invalid_args(
            "the Rust port currently supports pptx replace text --slide 1 --target title",
        ));
    }
    copy_zip_with_replacement(
        file,
        out,
        "ppt/slides/slide1.xml",
        "Minimal Title Slide",
        &xml_escape(new_text),
    )?;
    Ok(pptx_replace_text_readback(
        file, out, slide, target, new_text,
    ))
}

fn pptx_replace_text_in_place(
    file: &str,
    slide: u32,
    target: &str,
    new_text: &str,
) -> CliResult<()> {
    let temp = Path::new(file).with_extension(format!(
        "{}.tmp",
        Path::new(file)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("pptx")
    ));
    pptx_replace_text_to(file, &temp.to_string_lossy(), slide, target, new_text)?;
    fs::rename(temp, file).map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

fn pptx_replace_text_readback(
    file: &str,
    out: &str,
    slide: u32,
    target: &str,
    new_text: &str,
) -> Value {
    json!({
        "destination": {
            "file": out,
            "handle": "H:pptx/s:256/shape:n:2",
            "primarySelector": target,
            "selectors": ["title", "@title", "shape:2", "~Title 1"],
            "shapeId": 2,
            "shapeName": "Title 1",
            "slide": slide,
            "target": target,
            "targetKind": target,
            "textPreview": new_text,
        },
        "dryRun": false,
        "file": file,
        "mode": "plain-text",
        "newText": new_text,
        "output": out,
        "readbackCommand": format!(
            "ooxml --json pptx shapes get {} --slide {slide} --target {} --include-text --include-bounds",
            command_arg(out),
            command_arg(target)
        ),
        "renderCommand": format!("ooxml pptx render {out} --out render-check"),
        "slideNumber": slide,
        "slideReadbackCommand": format!("ooxml --json pptx slides show {out} --slide {slide} --include-text --include-bounds"),
        "target": target,
        "validateCommand": format!("ooxml validate --strict {out}"),
    })
}

fn docx_header_footer_show_json_args(args: &Value) -> CliResult<Vec<String>> {
    let mut rest = Vec::new();
    if let Some(selector) = json_optional_string(args, "selector") {
        rest.push("--selector".to_string());
        rest.push(selector);
    }
    if let Some(id) = json_optional_string(args, "id") {
        rest.push("--id".to_string());
        rest.push(id);
    }
    if let Some(ref_type) = json_optional_string(args, "type") {
        rest.push("--type".to_string());
        rest.push(ref_type);
    }
    if let Some(section) = json_i64(args, "section")? {
        rest.push("--section".to_string());
        rest.push(section.to_string());
    }
    Ok(rest)
}

fn pptx_extract_text_json_args(args: &Value) -> CliResult<Vec<String>> {
    let mut rest = Vec::new();
    if let Some(slide) = json_u32(args, "slide")? {
        rest.push("--slide".to_string());
        rest.push(slide.to_string());
    }
    if let Some(slides) = args.get("slides") {
        let values = slides
            .as_array()
            .ok_or_else(|| CliError::invalid_args("slides must be an array"))?;
        for value in values {
            let slide = if let Some(number) = value.as_u64() {
                u32::try_from(number)
                    .map_err(|_| CliError::invalid_args("slides entries must fit in uint32"))?
            } else if let Some(text) = value.as_str() {
                text.parse::<u32>().map_err(|_| {
                    CliError::invalid_args("slides entries must be integers or integer strings")
                })?
            } else {
                return Err(CliError::invalid_args(
                    "slides entries must be integers or integer strings",
                ));
            };
            rest.push("--slide".to_string());
            rest.push(slide.to_string());
        }
    }
    Ok(rest)
}

fn make_working_copy(file: &str, session_number: usize) -> CliResult<String> {
    let dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-{}-{session_number}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).map_err(|err| CliError::unexpected(err.to_string()))?;
    let extension = Path::new(file)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("xlsx");
    let working = dir.join(format!("working.{extension}"));
    fs::copy(file, &working).map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(working.to_string_lossy().to_string())
}

struct XlsxCellRead {
    kind: String,
    value: Value,
}

fn xlsx_cell_read(file: &str, sheet: &str, cell: &str) -> CliResult<XlsxCellRead> {
    let exported = xlsx_range_export(file, sheet, cell)?;
    let value = exported["values"][0][0].clone();
    let kind = exported["types"][0][0]
        .as_str()
        .unwrap_or("empty")
        .to_string();
    Ok(XlsxCellRead { kind, value })
}

fn xlsx_set_cell_string(file: &str, sheet: &str, cell: &str, value: &str) -> CliResult<()> {
    let sheet_part = xlsx_sheet_part(file, sheet)?;
    let xml = zip_text(file, &sheet_part)?;
    let updated = replace_cell_xml(&xml, cell, value)?;
    let temp = Path::new(file).with_extension(format!(
        "{}.tmp",
        Path::new(file)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("xlsx")
    ));
    copy_zip_with_part_override(file, &temp.to_string_lossy(), &sheet_part, &updated)?;
    fs::rename(temp, file).map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

fn xlsx_sheet_part(file: &str, sheet_selector: &str) -> CliResult<String> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    Ok(normalize_xl_target(target))
}

fn replace_cell_xml(xml: &str, cell: &str, value: &str) -> CliResult<String> {
    let needle = format!("<c r=\"{cell}\"");
    let start = xml
        .find(&needle)
        .ok_or_else(|| CliError::invalid_args(format!("cell not found: {cell}")))?;
    let close = xml[start..]
        .find("</c>")
        .map(|offset| start + offset + "</c>".len())
        .ok_or_else(|| CliError::unexpected(format!("cell has no closing tag: {cell}")))?;
    let replacement = format!(
        "<c r=\"{cell}\" t=\"inlineStr\"><is><t>{}</t></is></c>",
        xml_escape(value)
    );
    let mut updated = String::with_capacity(xml.len() + replacement.len());
    updated.push_str(&xml[..start]);
    updated.push_str(&replacement);
    updated.push_str(&xml[close..]);
    Ok(updated)
}

fn xlsx_cell_set_readback(
    file: &str,
    cell: &str,
    value: &str,
    previous_type: &str,
    previous_value: &Value,
) -> Value {
    json!({
        "cellsExtractCommand": format!("ooxml --json xlsx cells extract {file} --sheet sheetId:1 --range {cell} --include-empty"),
        "created": false,
        "destination": {
            "cols": 1,
            "file": file,
            "formulaCount": 0,
            "formulas": [[null]],
            "range": cell,
            "rows": 1,
            "sheet": "Sheet1",
            "sheetNumber": 1,
            "sheetPrimarySelector": "sheetId:1",
            "sheetSelectors": xlsx_sheet_selectors("Sheet1", 1, 1, "rId1", "/xl/worksheets/sheet1.xml"),
            "truncated": false,
            "types": [["string"]],
            "values": [[value]],
        },
        "dryRun": false,
        "file": file,
        "handle": format!("H:xlsx/ws:1/cell:a:{cell}"),
        "output": file,
        "previousType": previous_type,
        "previousValue": previous_value,
        "rangesExportCommand": format!("ooxml --json xlsx ranges export {file} --sheet sheetId:1 --range {cell} --include-types --include-formulas --include-formats"),
        "ref": cell,
        "sheet": "Sheet1",
        "sheetNumber": 1,
        "type": "string",
        "validateCommand": format!("ooxml validate --strict {file}"),
        "value": value,
    })
}

fn xlsx_sheet_selectors(
    name: &str,
    sheet_id: u32,
    position: u32,
    rel_id: &str,
    part_uri: &str,
) -> Vec<String> {
    vec![
        format!("sheetId:{sheet_id}"),
        format!("sheet:{position}"),
        format!("#{position}"),
        format!("rId:{rel_id}"),
        format!("rid:{rel_id}"),
        format!("part:{part_uri}"),
        format!("name:{name}"),
        format!("~{name}"),
        name.to_string(),
    ]
}

fn parse_slides_flag(args: &[String], name: &str) -> CliResult<Option<Vec<u32>>> {
    let Some(value) = parse_string_flag(args, name)? else {
        return Ok(None);
    };
    let mut slides = Vec::new();
    for token in value.split(',') {
        let slide = token.trim().parse::<u32>().map_err(|_| {
            CliError::invalid_args(format!("{name} must be a comma-separated slide list"))
        })?;
        slides.push(slide);
    }
    Ok(Some(slides))
}

fn pptx_all_slides(file: &str) -> Vec<u32> {
    zip_text(file, "ppt/presentation.xml")
        .map(|xml| (1..=pptx_slide_refs(&xml).len() as u32).collect())
        .unwrap_or_else(|_| vec![1])
}

fn mock_render_outputs(file: &str, out_dir: &Path, slides: &[u32]) -> CliResult<PathBuf> {
    let pdf_path = out_dir.join(format!("{}.pdf", file_stem(file)));
    fs::write(&pdf_path, b"pdf").map_err(|err| CliError::unexpected(err.to_string()))?;
    for slide in slides {
        fs::write(out_dir.join(format!("slide-{slide}.png")), b"png")
            .map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    Ok(pdf_path)
}

fn render_with_local_tools(file: &str, out_dir: &Path, slides: &[u32]) -> CliResult<PathBuf> {
    if !command_available("soffice") {
        return Err(CliError::unexpected(
            "required render tool not available: soffice",
        ));
    }
    if !command_available("pdftoppm") {
        return Err(CliError::unexpected(
            "required render tool not available: pdftoppm",
        ));
    }
    let status = Command::new("soffice")
        .args(["--headless", "--convert-to", "pdf", "--outdir"])
        .arg(out_dir)
        .arg(file)
        .status()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    if !status.success() {
        return Err(CliError::unexpected("soffice render failed"));
    }
    let pdf_path = out_dir.join(format!("{}.pdf", file_stem(file)));
    for slide in slides {
        let prefix = out_dir.join("slide");
        let status = Command::new("pdftoppm")
            .arg("-png")
            .arg("-r")
            .arg("144")
            .arg("-f")
            .arg(slide.to_string())
            .arg("-l")
            .arg(slide.to_string())
            .arg(&pdf_path)
            .arg(&prefix)
            .status()
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if !status.success() {
            return Err(CliError::unexpected("pdftoppm rasterize failed"));
        }
        let generated = out_dir.join(format!("slide-{slide}.png"));
        if !generated.exists() {
            let alternate = out_dir.join(format!("slide-{slide:01}.png"));
            if alternate.exists() {
                fs::rename(alternate, &generated)
                    .map_err(|err| CliError::unexpected(err.to_string()))?;
            }
        }
    }
    Ok(pdf_path)
}

fn command_available(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("presentation")
        .to_string()
}

fn verify_validation(file: &str) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    if !entries.iter().any(|name| name == "[Content_Types].xml") {
        return Ok(json!({
            "status": "invalid",
            "summary": {"errors": 1, "info": 0, "warnings": 0},
        }));
    }
    Ok(json!({
        "status": "valid",
        "summary": {"errors": 0, "info": 0, "warnings": 0},
    }))
}

fn package_type(file: &str) -> CliResult<&'static str> {
    let entries = zip_entry_names(file)?;
    if entries.iter().any(|name| name == "ppt/presentation.xml") {
        Ok("pptx")
    } else if entries.iter().any(|name| name == "xl/workbook.xml") {
        Ok("xlsx")
    } else if entries.iter().any(|name| name == "word/document.xml") {
        Ok("docx")
    } else {
        Ok("unknown")
    }
}

fn pptx_diff(baseline: &str, file: &str) -> CliResult<Value> {
    let before = pptx_slide_texts(baseline)?;
    let after = pptx_slide_texts(file)?;
    let slide_count_a = before.len();
    let slide_count_b = after.len();
    let mut changed_slides = Vec::new();
    let mut text_diffs = Vec::new();
    for slide_idx in 0..slide_count_a.max(slide_count_b) {
        let before_shapes = before.get(slide_idx).cloned().unwrap_or_default();
        let after_shapes = after.get(slide_idx).cloned().unwrap_or_default();
        let mut changed = false;
        for before_shape in before_shapes {
            let Some(after_shape) = after_shapes
                .iter()
                .find(|candidate| candidate.key == before_shape.key)
            else {
                continue;
            };
            if before_shape.text != after_shape.text {
                changed = true;
                text_diffs.push(json!({
                    "after": after_shape.text,
                    "before": before_shape.text,
                    "shapeKey": before_shape.key,
                    "shapeName": before_shape.name,
                    "slide": slide_idx + 1,
                }));
            }
        }
        if changed {
            changed_slides.push(Value::from(slide_idx + 1));
        }
    }
    Ok(json!({
        "schemaVersion": "1.0",
        "semantic": {
            "changedSlides": changed_slides,
            "imageDiffs": [],
            "layoutDiffs": [],
            "slideCountA": slide_count_a,
            "slideCountB": slide_count_b,
            "slideCountEqual": slide_count_a == slide_count_b,
            "textDiffs": text_diffs,
        },
        "type": "pptx",
        "visual": {
            "enabled": false,
            "status": "disabled",
        },
    }))
}

fn pptx_extract_text(file: &str, args: &[String]) -> CliResult<Value> {
    if package_type(file)? != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {})",
            package_type(file)?
        )));
    }

    let selected_slides = parse_u32_flags(args, "--slide")?;
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut values = Vec::new();
    for (index, (_slide_id, rel_id)) in slides.iter().enumerate() {
        let slide_number = index as u32 + 1;
        if !selected_slides.is_empty() && !selected_slides.contains(&slide_number) {
            continue;
        }
        let target = rels
            .get(rel_id)
            .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
        let part = normalize_ppt_target(target);
        let xml = zip_text(file, &part)?;
        values.push(json!({
            "slide": slide_number,
            "shapes": pptx_extract_text_shapes(&xml),
        }));
    }
    Ok(json!({
        "file": file,
        "slides": values,
    }))
}

#[derive(Clone)]
struct PptxSlidePartRef {
    number: u32,
    slide_id: u32,
    part: String,
}

fn pptx_slide_part_refs(file: &str) -> CliResult<Vec<PptxSlidePartRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    slides
        .iter()
        .enumerate()
        .map(|(index, (slide_id, rel_id))| {
            let target = rels
                .get(rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            Ok(PptxSlidePartRef {
                number: index as u32 + 1,
                slide_id: *slide_id,
                part: normalize_ppt_target(target),
            })
        })
        .collect()
}

#[derive(Clone, Default)]
struct PptxCommentAuthor {
    name: String,
    initials: String,
}

#[derive(Clone)]
struct PptxCommentInfo {
    id: i64,
    author_id: i64,
    author: String,
    initials: String,
    date: String,
    text: String,
    content_hash: String,
    handle: String,
    primary_selector: String,
    selectors: Vec<String>,
}

#[derive(Default)]
struct PptxCommentBuild {
    id: i64,
    author_id: i64,
    date: String,
    text: String,
}

fn pptx_comments_list(
    file: &str,
    slide_filter: Option<u32>,
    comment_id: Option<i64>,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    if matches!(slide_filter, Some(0)) {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    if comment_id.is_some() && slide_filter.is_none() {
        return Err(CliError::invalid_args("--comment-id requires --slide"));
    }

    let slides = pptx_slide_part_refs(file)?;
    if let Some(slide) = slide_filter
        && slide as usize > slides.len()
    {
        return Err(CliError::invalid_args(format!(
            "--slide {slide} out of range (presentation has {} slides)",
            slides.len()
        )));
    }

    let entries = zip_entry_names(file)?;
    let authors = pptx_comment_authors(file, &entries)?;
    let mut slide_values = Vec::new();
    for slide in &slides {
        if let Some(wanted) = slide_filter
            && slide.number != wanted
        {
            continue;
        }
        let (mut value, comments) = pptx_slide_comments(file, &entries, &authors, slide)?;
        if let Some(comment_id) = comment_id {
            let filtered = comments
                .iter()
                .filter(|comment| comment.id == comment_id)
                .cloned()
                .collect::<Vec<_>>();
            if filtered.is_empty() {
                return Err(pptx_comment_not_found_error(
                    &comments,
                    slide.number,
                    comment_id,
                ));
            }
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "comments".to_string(),
                    Value::Array(filtered.iter().map(pptx_comment_json).collect()),
                );
            }
        }
        slide_values.push(value);
    }

    Ok(json!({
        "file": file,
        "slides": slide_values,
    }))
}

fn pptx_slide_comments(
    file: &str,
    entries: &[String],
    authors: &BTreeMap<i64, PptxCommentAuthor>,
    slide: &PptxSlidePartRef,
) -> CliResult<(Value, Vec<PptxCommentInfo>)> {
    let mut output = Map::new();
    output.insert("slide".to_string(), json!(slide.number));
    output.insert(
        "slidePartUri".to_string(),
        json!(format!("/{}", slide.part.trim_start_matches('/'))),
    );
    let comments_part = pptx_slide_comments_part(file, entries, &slide.part);
    let comments = if let Some(comments_part) = comments_part.as_deref() {
        output.insert("commentsPart".to_string(), json!(comments_part));
        let xml = zip_text(file, comments_part.trim_start_matches('/'))?;
        pptx_comments_from_xml(&xml, authors, slide.slide_id)
    } else {
        Vec::new()
    };
    output.insert(
        "comments".to_string(),
        Value::Array(comments.iter().map(pptx_comment_json).collect()),
    );
    Ok((Value::Object(output), comments))
}

fn pptx_slide_comments_part(file: &str, entries: &[String], slide_part: &str) -> Option<String> {
    let rels = relationship_entries(file, &relationships_part_for(slide_part)).unwrap_or_default();
    let slide_uri = format!("/{}", slide_part.trim_start_matches('/'));
    for rel in rels {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"
        {
            let uri = resolve_relationship_target(&slide_uri, &rel.target);
            return zip_entry_exists(entries, &uri).then_some(uri);
        }
    }
    None
}

fn pptx_comment_authors(
    file: &str,
    entries: &[String],
) -> CliResult<BTreeMap<i64, PptxCommentAuthor>> {
    let mut authors = BTreeMap::new();
    let Some(part) = pptx_comment_authors_part(file, entries) else {
        return Ok(authors);
    };
    let xml = zip_text(file, part.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cmAuthor" =>
            {
                let id = attr(&e, "id")
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or_default();
                authors.insert(
                    id,
                    PptxCommentAuthor {
                        name: attr(&e, "name").unwrap_or_default(),
                        initials: attr(&e, "initials").unwrap_or_default(),
                    },
                );
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(authors)
}

fn pptx_comment_authors_part(file: &str, entries: &[String]) -> Option<String> {
    let rels = relationship_entries(file, "ppt/_rels/presentation.xml.rels").unwrap_or_default();
    for rel in rels {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/commentAuthors"
        {
            let uri = resolve_relationship_target("/ppt/presentation.xml", &rel.target);
            if zip_entry_exists(entries, &uri) {
                return Some(uri);
            }
            return None;
        }
    }
    let conventional = "/ppt/commentAuthors.xml";
    zip_entry_exists(entries, conventional).then(|| conventional.to_string())
}

fn pptx_comments_from_xml(
    xml: &str,
    authors: &BTreeMap<i64, PptxCommentAuthor>,
    slide_id: u32,
) -> Vec<PptxCommentInfo> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut comments = Vec::new();
    let mut current: Option<PptxCommentBuild> = None;
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "cm" => {
                current = Some(PptxCommentBuild {
                    id: attr(&e, "idx")
                        .and_then(|value| value.parse::<i64>().ok())
                        .unwrap_or_default(),
                    author_id: attr(&e, "authorId")
                        .and_then(|value| value.parse::<i64>().ok())
                        .unwrap_or_default(),
                    date: attr(&e, "dt").unwrap_or_default(),
                    text: String::new(),
                });
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "cm" => {
                comments.push(pptx_comment_from_build(
                    PptxCommentBuild {
                        id: attr(&e, "idx")
                            .and_then(|value| value.parse::<i64>().ok())
                            .unwrap_or_default(),
                        author_id: attr(&e, "authorId")
                            .and_then(|value| value.parse::<i64>().ok())
                            .unwrap_or_default(),
                        date: attr(&e, "dt").unwrap_or_default(),
                        text: String::new(),
                    },
                    authors,
                    slide_id,
                ));
            }
            Ok(Event::Start(e)) if current.is_some() && local_name(e.name().as_ref()) == "text" => {
                in_text = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "text" => {
                in_text = false;
            }
            Ok(Event::Text(e)) if in_text => {
                if let Some(comment) = current.as_mut() {
                    comment.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) if in_text => {
                if let Some(comment) = current.as_mut() {
                    comment.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "cm" => {
                if let Some(comment) = current.take() {
                    comments.push(pptx_comment_from_build(comment, authors, slide_id));
                }
                in_text = false;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    comments
}

fn pptx_comment_from_build(
    comment: PptxCommentBuild,
    authors: &BTreeMap<i64, PptxCommentAuthor>,
    slide_id: u32,
) -> PptxCommentInfo {
    let author = authors.get(&comment.author_id).cloned().unwrap_or_default();
    let content_hash = pptx_comment_content_hash(&author.name, &comment.date, &comment.text);
    let handle = pptx_comment_handle(slide_id, comment.id, comment.author_id);
    let primary_selector = pptx_comment_primary_selector(&handle, comment.id, comment.author_id);
    let selectors = pptx_comment_selectors(&handle, comment.id, comment.author_id);
    PptxCommentInfo {
        id: comment.id,
        author_id: comment.author_id,
        author: author.name,
        initials: author.initials,
        date: comment.date,
        text: comment.text,
        content_hash,
        handle,
        primary_selector,
        selectors,
    }
}

fn pptx_comment_json(comment: &PptxCommentInfo) -> Value {
    let mut output = Map::new();
    output.insert("id".to_string(), json!(comment.id));
    output.insert("authorId".to_string(), json!(comment.author_id));
    if !comment.handle.is_empty() {
        output.insert("handle".to_string(), json!(comment.handle));
    }
    if !comment.primary_selector.is_empty() {
        output.insert(
            "primarySelector".to_string(),
            json!(comment.primary_selector),
        );
    }
    if !comment.selectors.is_empty() {
        output.insert("selectors".to_string(), json!(comment.selectors));
    }
    output.insert("author".to_string(), json!(comment.author));
    if !comment.initials.is_empty() {
        output.insert("initials".to_string(), json!(comment.initials));
    }
    if !comment.date.is_empty() {
        output.insert("date".to_string(), json!(comment.date));
    }
    output.insert("text".to_string(), json!(comment.text));
    output.insert("contentHash".to_string(), json!(comment.content_hash));
    Value::Object(output)
}

fn pptx_comment_content_hash(author: &str, date: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(author.as_bytes());
    hasher.update([0]);
    hasher.update(date.as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn pptx_comment_handle(slide_id: u32, comment_id: i64, author_id: i64) -> String {
    if slide_id == 0 || comment_id < 0 || author_id < 0 {
        return String::new();
    }
    format!("H:pptx/s:{slide_id}/comment:idx:{comment_id}:authorId:{author_id}")
}

fn pptx_comment_primary_selector(handle: &str, comment_id: i64, author_id: i64) -> String {
    if !handle.trim().is_empty() {
        handle.to_string()
    } else {
        format!("comment:{comment_id}:authorId:{author_id}")
    }
}

fn pptx_comment_selectors(handle: &str, comment_id: i64, author_id: i64) -> Vec<String> {
    let mut selectors = Vec::new();
    if !handle.trim().is_empty() {
        selectors.push(handle.to_string());
    }
    selectors.push(format!("comment:{comment_id}:authorId:{author_id}"));
    selectors.push(format!("comment:{comment_id}"));
    selectors.push(comment_id.to_string());
    selectors.push(format!("authorId:{author_id}"));
    selectors
}

fn pptx_comment_not_found_error(
    comments: &[PptxCommentInfo],
    slide: u32,
    comment_id: i64,
) -> CliError {
    let selector = format!("comment:{comment_id}");
    let selector_items = comments
        .iter()
        .map(|comment| {
            (
                comment.primary_selector.as_str(),
                comment.selectors.as_slice(),
            )
        })
        .collect::<Vec<_>>();
    let candidates = selector_candidates(&selector_items, &selector, 3);
    let mut message = format!("comment not found: {selector}");
    if !candidates.is_empty() {
        message.push_str(&format!("; did you mean: {}", candidates.join(", ")));
    }
    message.push_str(&format!(
        "; discover with `ooxml --json pptx comments list <file> --slide {slide}`"
    ));
    CliError::target_not_found(message)
}

#[derive(Clone)]
struct PptxMasterRef {
    part_uri: String,
    layout_uris: Vec<String>,
    theme_uri: String,
}

#[derive(Clone)]
struct PptxLayoutInfo {
    id: String,
    name: String,
    part_uri: String,
    master_id: String,
    theme_uri: String,
    preserve: bool,
    user_drawn: bool,
    placeholders: Vec<Value>,
}

fn pptx_masters_list(file: &str) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let masters = pptx_presentation_masters(file)?;
    let entries = masters
        .iter()
        .enumerate()
        .map(|(index, master)| {
            let number = index + 1;
            let primary = number.to_string();
            let mut entry = Map::new();
            entry.insert("index".to_string(), json!(number));
            entry.insert("uri".to_string(), json!(master.part_uri));
            entry.insert("primarySelector".to_string(), json!(primary.clone()));
            entry.insert("selectors".to_string(), json!([primary]));
            entry.insert("layouts".to_string(), json!(master.layout_uris.len()));
            if !master.theme_uri.is_empty() {
                entry.insert("theme".to_string(), json!(master.theme_uri));
            }
            Value::Object(entry)
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "masters": entries,
    }))
}

fn pptx_masters_show(file: &str, master_number: i64) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let masters = pptx_presentation_masters(file)?;
    if master_number < 1 || master_number as usize > masters.len() {
        return Err(CliError::invalid_args(format!(
            "master {master_number} not found"
        )));
    }
    let master = &masters[master_number as usize - 1];
    let master_xml = zip_text(file, master.part_uri.trim_start_matches('/')).unwrap_or_default();
    let mut output = Map::new();
    output.insert("uri".to_string(), json!(master.part_uri));
    output.insert("index".to_string(), json!(master_number));
    output.insert("layouts".to_string(), json!(master.layout_uris));
    output.insert("layoutCount".to_string(), json!(master.layout_uris.len()));
    if !master.theme_uri.is_empty() {
        output.insert("themeUri".to_string(), json!(master.theme_uri));
        if let Some(theme) = pptx_theme_info(file, &master.theme_uri) {
            output.insert("theme".to_string(), theme.clone());
            if let Some(defaults) = pptx_default_text_style_info(&theme) {
                output.insert("defaultTextStyleInfo".to_string(), defaults);
            }
        }
    }
    output.insert(
        "shapes".to_string(),
        json!(pptx_master_shape_count(&master_xml)),
    );
    let placeholders = pptx_layout_placeholders(&master_xml);
    if !placeholders.is_empty() {
        output.insert("placeholders".to_string(), Value::Array(placeholders));
    }
    let text_styles = pptx_master_text_styles(&master_xml);
    if !text_styles.is_empty() {
        output.insert("textStyles".to_string(), Value::Object(text_styles));
    }
    Ok(Value::Object(output))
}

fn pptx_master_shape_count(xml: &str) -> usize {
    ["<p:sp", "<p:pic", "<p:graphicFrame", "<p:grpSp"]
        .into_iter()
        .map(|needle| xml.matches(needle).count())
        .sum()
}

fn pptx_master_text_styles(xml: &str) -> Map<String, Value> {
    let mut styles = Map::new();
    if xml.contains("title") || xml.contains("Title") {
        styles.insert(
            "title".to_string(),
            json!({
                "placeholderType": "title",
            }),
        );
    }
    if xml.contains("body") || xml.contains("Body") {
        styles.insert(
            "body".to_string(),
            json!({
                "placeholderType": "body",
            }),
        );
    }
    if xml.contains("ctrTitle") || xml.contains("centerTitle") {
        styles.insert(
            "centerTitle".to_string(),
            json!({
                "placeholderType": "centerTitle",
            }),
        );
    }
    if xml.contains("subTitle") || xml.contains("subtitle") {
        styles.insert(
            "subtitle".to_string(),
            json!({
                "placeholderType": "subtitle",
            }),
        );
    }
    styles
}

fn pptx_layouts_list(file: &str, master: Option<u32>) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let mut layouts = pptx_presentation_layouts(file)?;
    if let Some(master) = master
        && master > 0
    {
        let master_id = format!("master-{master}");
        layouts.retain(|layout| layout.master_id == master_id);
    }
    let entries = layouts
        .iter()
        .enumerate()
        .map(|(index, layout)| {
            let number = index + 1;
            let placeholders = layout
                .placeholders
                .iter()
                .filter_map(|placeholder| placeholder.get("key").and_then(Value::as_str))
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let mut entry = Map::new();
            entry.insert("id".to_string(), json!(layout.id));
            entry.insert("number".to_string(), json!(number));
            entry.insert("name".to_string(), json!(layout.name));
            entry.insert("partUri".to_string(), json!(layout.part_uri));
            if !layout.master_id.is_empty() {
                entry.insert("masterId".to_string(), json!(layout.master_id));
            }
            entry.insert("primarySelector".to_string(), json!(number.to_string()));
            entry.insert(
                "selectors".to_string(),
                json!(pptx_layout_selectors(number, &layout.name)),
            );
            entry.insert("placeholderCount".to_string(), json!(placeholders.len()));
            entry.insert("placeholders".to_string(), json!(placeholders));
            Value::Object(entry)
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "file": file,
        "layouts": entries,
    }))
}

fn pptx_layouts_show(file: &str, selector: &str) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let layouts = pptx_presentation_layouts(file)?;
    let layout = pptx_find_layout(&layouts, selector)
        .ok_or_else(|| CliError::invalid_args(format!("layout not found: {selector}")))?;
    let mut output = Map::new();
    output.insert("id".to_string(), json!(layout.id));
    output.insert("name".to_string(), json!(layout.name));
    output.insert("partUri".to_string(), json!(layout.part_uri));
    if !layout.master_id.is_empty() {
        output.insert("masterId".to_string(), json!(layout.master_id));
    }
    if !layout.theme_uri.is_empty() {
        output.insert("themeUri".to_string(), json!(layout.theme_uri));
        if let Some(theme) = pptx_theme_info(file, &layout.theme_uri) {
            output.insert("theme".to_string(), theme.clone());
            if let Some(defaults) = pptx_default_text_style_info(&theme) {
                output.insert("defaultTextStyleInfo".to_string(), defaults);
            }
        }
    }
    output.insert("preserve".to_string(), json!(layout.preserve));
    output.insert("userDrawn".to_string(), json!(layout.user_drawn));
    output.insert(
        "placeholders".to_string(),
        Value::Array(layout.placeholders.clone()),
    );
    Ok(Value::Object(output))
}

fn pptx_presentation_layouts(file: &str) -> CliResult<Vec<PptxLayoutInfo>> {
    let masters = pptx_presentation_masters(file)?;
    let mut master_uri_to_id = BTreeMap::<String, String>::new();
    let mut master_uri_to_theme = BTreeMap::<String, String>::new();
    for (index, master) in masters.iter().enumerate() {
        master_uri_to_id.insert(master.part_uri.clone(), format!("master-{}", index + 1));
        master_uri_to_theme.insert(master.part_uri.clone(), master.theme_uri.clone());
    }

    let mut layouts = Vec::new();
    for master in &masters {
        for layout_uri in &master.layout_uris {
            let layout_part = layout_uri.trim_start_matches('/');
            let xml = zip_text(file, layout_part)?;
            let (name, preserve, user_drawn) = pptx_layout_metadata(&xml);
            let master_part_uri = pptx_layout_master_part(file, layout_uri)?;
            let master_id = master_uri_to_id
                .get(&master_part_uri)
                .cloned()
                .unwrap_or_default();
            let theme_uri = master_uri_to_theme
                .get(&master_part_uri)
                .cloned()
                .unwrap_or_default();
            layouts.push(PptxLayoutInfo {
                id: format!("layout-{}", layouts.len() + 1),
                name,
                part_uri: layout_uri.clone(),
                master_id,
                theme_uri,
                preserve,
                user_drawn,
                placeholders: pptx_layout_placeholders(&xml),
            });
        }
    }
    Ok(layouts)
}

fn pptx_presentation_masters(file: &str) -> CliResult<Vec<PptxMasterRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut reader = Reader::from_str(&presentation);
    reader.config_mut().trim_text(true);
    let mut masters = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldMasterId" =>
            {
                let Some(rel_id) = attr_exact(&e, "r:id") else {
                    continue;
                };
                let Some(target) = rels.get(&rel_id) else {
                    return Err(CliError::unexpected(format!(
                        "relationship {rel_id} not found in presentation.xml.rels"
                    )));
                };
                let master_part_uri = resolve_relationship_target("/ppt/presentation.xml", target);
                let (layout_uris, theme_uri) =
                    pptx_master_layouts_and_theme(file, &master_part_uri);
                masters.push(PptxMasterRef {
                    part_uri: master_part_uri,
                    layout_uris,
                    theme_uri,
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(masters)
}

fn pptx_master_layouts_and_theme(file: &str, master_uri: &str) -> (Vec<String>, String) {
    let rels = relationship_entries(file, &relationships_part_for(master_uri)).unwrap_or_default();
    let mut layout_uris = Vec::new();
    let mut theme_uri = String::new();
    for rel in rels {
        match rel.rel_type.as_str() {
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" => {
                layout_uris.push(resolve_relationship_target(master_uri, &rel.target));
            }
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" => {
                theme_uri = resolve_relationship_target(master_uri, &rel.target);
            }
            _ => {}
        }
    }
    (layout_uris, theme_uri)
}

fn pptx_layout_master_part(file: &str, layout_uri: &str) -> CliResult<String> {
    let rels = relationship_entries(file, &relationships_part_for(layout_uri)).unwrap_or_default();
    for rel in rels {
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster"
        {
            return Ok(resolve_relationship_target(layout_uri, &rel.target));
        }
    }
    Ok(String::new())
}

fn pptx_layout_metadata(xml: &str) -> (String, bool, bool) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut name = String::new();
    let mut preserve = false;
    let mut user_drawn = false;
    let mut saw_root = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if !saw_root {
                    preserve = pptx_truthy_attr(&e, "preserve");
                    user_drawn = pptx_truthy_attr(&e, "userDrawn");
                    saw_root = true;
                }
                if local_name(e.name().as_ref()) == "cSld" {
                    name = attr(&e, "name").unwrap_or_default();
                    if saw_root {
                        break;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    (name, preserve, user_drawn)
}

fn pptx_truthy_attr(e: &BytesStart<'_>, key: &str) -> bool {
    matches!(attr(e, key).as_deref(), Some("1" | "true"))
}

fn pptx_layout_placeholders(xml: &str) -> Vec<Value> {
    pptx_shape_models(xml)
        .into_iter()
        .filter_map(|shape| {
            if !shape.is_placeholder {
                return None;
            }
            let placeholder = shape.placeholder.as_ref()?;
            let literal_type = placeholder.literal_type.as_str();
            let role = pptx_layout_placeholder_role(literal_type);
            let index = placeholder.index.unwrap_or(0);
            let key = if literal_type.is_empty() {
                if shape.id != 0 {
                    format!("shape:{}", shape.id)
                } else {
                    "unknown".to_string()
                }
            } else if placeholder.index.is_some() {
                format!("{literal_type}:{index}")
            } else {
                literal_type.to_string()
            };
            let mut value = Map::new();
            value.insert("key".to_string(), json!(key));
            value.insert("role".to_string(), json!(role));
            value.insert("index".to_string(), json!(index));
            value.insert("shapeName".to_string(), json!(shape.name));
            value.insert("literalType".to_string(), json!(""));
            value.insert("resolvedType".to_string(), json!(""));
            if let Some(bounds) = shape.bounds.as_ref() {
                value.insert(
                    "geometry".to_string(),
                    json!({
                        "bounds": bounds_json(bounds),
                    }),
                );
            }
            Some(Value::Object(value))
        })
        .collect()
}

fn pptx_layout_placeholder_role(literal_type: &str) -> String {
    match literal_type {
        "title" | "ctrTitle" => "title",
        "subTitle" => "subtitle",
        "body" => "body",
        "pic" => "pic",
        "tbl" => "table",
        "chart" => "chart",
        "obj" => "object",
        "dt" => "date",
        "ftr" => "footer",
        "sldNum" => "slideNumber",
        other => other,
    }
    .to_string()
}

fn pptx_layout_selectors(number: usize, name: &str) -> Vec<String> {
    let primary = number.to_string();
    let mut selectors = vec![primary.clone()];
    if !name.is_empty() && name != primary {
        selectors.push(name.to_string());
    }
    selectors
}

fn pptx_find_layout<'a>(
    layouts: &'a [PptxLayoutInfo],
    selector: &str,
) -> Option<&'a PptxLayoutInfo> {
    if let Ok(number) = selector.parse::<usize>()
        && number >= 1
    {
        return layouts.get(number - 1);
    }
    layouts.iter().find(|layout| layout.name == selector)
}

fn pptx_theme_info(file: &str, theme_uri: &str) -> Option<Value> {
    let xml = zip_text(file, theme_uri.trim_start_matches('/')).ok()?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut theme_name = String::new();
    let mut color_scheme = Map::new();
    let mut font_scheme = Map::new();
    let mut in_theme_elements = false;
    let mut in_color_scheme = false;
    let mut in_font_scheme = false;
    let mut current_color = String::new();
    let mut current_font = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                match name.as_str() {
                    "theme" => theme_name = attr(&e, "name").unwrap_or_default(),
                    "themeElements" => in_theme_elements = true,
                    "clrScheme" if in_theme_elements => {
                        in_color_scheme = true;
                        if let Some(value) = attr(&e, "name") {
                            color_scheme.insert("name".to_string(), json!(value));
                        }
                    }
                    "fontScheme" if in_theme_elements => {
                        in_font_scheme = true;
                        if let Some(value) = attr(&e, "name") {
                            font_scheme.insert("name".to_string(), json!(value));
                        }
                    }
                    "dk1" | "lt1" | "dk2" | "lt2" | "accent1" | "accent2" | "accent3"
                    | "accent4" | "accent5" | "accent6" | "hlink" | "folHlink"
                        if in_color_scheme =>
                    {
                        current_color = name;
                    }
                    "majorFont" | "minorFont" if in_font_scheme => current_font = name,
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if in_color_scheme && !current_color.is_empty() {
                    if name == "srgbClr" {
                        if let Some(value) = attr(&e, "val") {
                            pptx_insert_theme_color(&mut color_scheme, &current_color, value);
                        }
                    } else if name == "sysClr"
                        && let Some(value) = attr(&e, "lastClr")
                    {
                        pptx_insert_theme_color(&mut color_scheme, &current_color, value);
                    }
                }
                if in_font_scheme && !current_font.is_empty() {
                    match (current_font.as_str(), name.as_str()) {
                        ("majorFont", "latin") => {
                            if let Some(value) = attr(&e, "typeface") {
                                font_scheme.insert("majorFont".to_string(), json!(value));
                            }
                        }
                        ("minorFont", "latin") => {
                            if let Some(value) = attr(&e, "typeface") {
                                font_scheme.insert("minorFont".to_string(), json!(value));
                            }
                        }
                        ("majorFont", "ea") => {
                            if let Some(value) = attr(&e, "typeface")
                                && !value.is_empty()
                            {
                                font_scheme.insert("eastAsianMajorFont".to_string(), json!(value));
                            }
                        }
                        ("minorFont", "ea") => {
                            if let Some(value) = attr(&e, "typeface")
                                && !value.is_empty()
                            {
                                font_scheme.insert("eastAsianMinorFont".to_string(), json!(value));
                            }
                        }
                        ("majorFont", "cs") => {
                            if let Some(value) = attr(&e, "typeface")
                                && !value.is_empty()
                            {
                                font_scheme
                                    .insert("complexScriptMajorFont".to_string(), json!(value));
                            }
                        }
                        ("minorFont", "cs") => {
                            if let Some(value) = attr(&e, "typeface")
                                && !value.is_empty()
                            {
                                font_scheme
                                    .insert("complexScriptMinorFont".to_string(), json!(value));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => match local_name(e.name().as_ref()) {
                "themeElements" => in_theme_elements = false,
                "clrScheme" => in_color_scheme = false,
                "fontScheme" => in_font_scheme = false,
                "dk1" | "lt1" | "dk2" | "lt2" | "accent1" | "accent2" | "accent3" | "accent4"
                | "accent5" | "accent6" | "hlink" | "folHlink" => {
                    current_color.clear();
                }
                "majorFont" | "minorFont" => current_font.clear(),
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    let mut theme = Map::new();
    if !theme_name.is_empty() {
        theme.insert("name".to_string(), json!(theme_name));
    }
    if !color_scheme.is_empty() {
        theme.insert("colorScheme".to_string(), Value::Object(color_scheme));
    }
    if !font_scheme.is_empty() {
        theme.insert("fontScheme".to_string(), Value::Object(font_scheme));
    }
    Some(Value::Object(theme))
}

fn pptx_insert_theme_color(color_scheme: &mut Map<String, Value>, key: &str, value: String) {
    let json_key = match key {
        "dk1" => "dark1",
        "lt1" => "light1",
        "dk2" => "dark2",
        "lt2" => "light2",
        "hlink" => "hypLink",
        "folHlink" => "folLink",
        other => other,
    };
    color_scheme.insert(json_key.to_string(), json!(value));
}

fn pptx_default_text_style_info(theme: &Value) -> Option<Value> {
    let theme_object = theme.as_object()?;
    let mut info = Map::new();
    if let Some(name) = theme_object.get("name").and_then(Value::as_str)
        && !name.is_empty()
    {
        info.insert("themeName".to_string(), json!(name));
    }
    if let Some(font_scheme) = theme_object.get("fontScheme").and_then(Value::as_object) {
        if let Some(major_font) = font_scheme.get("majorFont").and_then(Value::as_str)
            && !major_font.is_empty()
        {
            info.insert("majorFont".to_string(), json!(major_font));
        }
        if let Some(minor_font) = font_scheme.get("minorFont").and_then(Value::as_str)
            && !minor_font.is_empty()
        {
            info.insert("minorFont".to_string(), json!(minor_font));
        }
    }
    let accents = theme_object
        .get("colorScheme")
        .and_then(Value::as_object)
        .map(|color_scheme| {
            [
                "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
            ]
            .into_iter()
            .filter_map(|key| color_scheme.get(key).and_then(Value::as_str))
            .filter(|value| !value.is_empty())
            .map(|value| json!(value))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !accents.is_empty() {
        info.insert("accentColors".to_string(), Value::Array(accents));
    }
    if info.is_empty() {
        None
    } else {
        Some(Value::Object(info))
    }
}

fn pptx_extract_notes(file: &str, args: &[String]) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }

    let selected_slides = parse_u32_flags(args, "--slide")?;
    let slides = pptx_slide_part_refs(file)?;
    let mut notes = Vec::new();
    if selected_slides.is_empty() {
        for slide in &slides {
            notes.push(pptx_notes_report(file, slide)?);
        }
    } else {
        for slide_number in selected_slides {
            if slide_number == 0 || slide_number as usize > slides.len() {
                return Err(CliError::invalid_args(format!(
                    "slide number {slide_number} is out of range (1-{})",
                    slides.len()
                )));
            }
            notes.push(pptx_notes_report(file, &slides[slide_number as usize - 1])?);
        }
    }
    Ok(json!({
        "file": file,
        "notes": notes,
    }))
}

fn pptx_notes_show(file: &str, slide: u32) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    if slide == 0 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let slides = pptx_slide_part_refs(file)?;
    let index = slide as usize - 1;
    let slide_ref = slides.get(index).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide {slide} not found (presentation has {} slides)",
            slides.len()
        ))
    })?;
    pptx_notes_report(file, slide_ref)
}

fn pptx_notes_report(file: &str, slide: &PptxSlidePartRef) -> CliResult<Value> {
    let mut report = Map::new();
    report.insert(
        "id".to_string(),
        json!(format!("slide{}-notes", slide.number)),
    );
    report.insert("slide".to_string(), json!(slide.number));
    let (_layout_part, notes_part) = slide_layout_and_notes_parts(file, &slide.part)?;
    let notes = if let Some(part) = notes_part {
        report.insert("partUri".to_string(), json!(format!("/{part}")));
        match zip_text(file, &part) {
            Ok(xml) => pptx_notes_text_block(&xml),
            Err(_) => pptx_empty_notes_block(),
        }
    } else {
        pptx_empty_notes_block()
    };
    report.insert("notes".to_string(), notes);
    Ok(Value::Object(report))
}

fn pptx_empty_notes_block() -> Value {
    json!({
        "paragraphs": [],
        "plainText": "",
    })
}

fn pptx_notes_text_block(xml: &str) -> Value {
    let Some(shape) = pptx_shape_models(xml).into_iter().find(|shape| {
        shape.kind == "sp"
            && shape
                .placeholder
                .as_ref()
                .is_some_and(|placeholder| placeholder.literal_type == "body")
    }) else {
        return pptx_empty_notes_block();
    };
    pptx_text_block_from_paragraphs(&shape.paragraphs, true, false)
}

fn pptx_tables_show(
    file: &str,
    slide: u32,
    table_id: u32,
    target: Option<&str>,
    include_details: bool,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let slides = pptx_slide_part_refs(file)?;
    if slide == 0 || slide as usize > slides.len() {
        return Err(CliError::invalid_args(format!(
            "slide number {slide} out of range (1-{})",
            slides.len()
        )));
    }
    let slide_ref = &slides[slide as usize - 1];
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let shapes = pptx_shape_models(&slide_xml);
    let targets = pptx_selector_targets_from_shapes(&shapes);
    let resolved_table_id = pptx_resolve_table_target(&shapes, &targets, target)?;
    let wanted_table_id = if table_id > 0 {
        Some(table_id)
    } else {
        resolved_table_id
    };
    let tables = pptx_table_summaries(slide, &shapes, &targets, wanted_table_id, include_details);
    if let Some(wanted_table_id) = wanted_table_id
        && tables.is_empty()
    {
        return Err(CliError::target_not_found(format!(
            "target not found: table shape ID {wanted_table_id} on slide {slide}"
        )));
    }
    Ok(json!({
        "file": file,
        "slide": slide,
        "tables": tables,
    }))
}

fn pptx_resolve_table_target(
    shapes: &[Shape],
    targets: &[Value],
    target: Option<&str>,
) -> CliResult<Option<u32>> {
    let target = target.map(str::trim).unwrap_or_default();
    if target.is_empty() || target == "@all-tables" {
        return Ok(None);
    }
    for (shape, target_value) in shapes.iter().zip(targets) {
        let primary = target_value
            .get("primarySelector")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let selectors = target_value
            .get("selectors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str);
        if primary == target || selectors.clone().any(|selector| selector == target) {
            if shape.kind == "graphicFrame" && shape.table.is_some() {
                return Ok(Some(shape.id));
            }
            return Err(CliError::invalid_args(format!(
                "target {target:?} resolves to {primary}, not a table"
            )));
        }
    }
    Err(CliError::target_not_found(format!(
        "target not found: target not found: {target} (available selectors: {})",
        pptx_available_shape_selectors(targets).join(", ")
    )))
}

fn pptx_available_shape_selectors(targets: &[Value]) -> Vec<String> {
    let mut selectors = Vec::new();
    add_selector(&mut selectors, "@all-shapes".to_string());
    add_selector(&mut selectors, "@all-shapes-nonph".to_string());
    add_selector(&mut selectors, "@all-tables".to_string());
    for target in targets {
        if let Some(items) = target.get("selectors").and_then(Value::as_array) {
            for item in items {
                if let Some(selector) = item.as_str() {
                    add_selector(&mut selectors, selector.to_string());
                }
            }
        }
    }
    selectors
}

fn pptx_table_summaries(
    slide: u32,
    shapes: &[Shape],
    targets: &[Value],
    table_id: Option<u32>,
    include_details: bool,
) -> Vec<Value> {
    shapes
        .iter()
        .zip(targets)
        .filter(|(shape, _target)| shape.kind == "graphicFrame" && shape.table.is_some())
        .filter(|(shape, _target)| table_id.is_none_or(|table_id| shape.id == table_id))
        .map(|(shape, target)| pptx_table_summary(slide, shape, target, include_details))
        .collect()
}

fn pptx_table_summary(slide: u32, shape: &Shape, target: &Value, include_details: bool) -> Value {
    let table = shape.table.as_ref().expect("table summary requires table");
    let cells = table
        .rows
        .iter()
        .map(|row| {
            Value::Array(
                row.cells
                    .iter()
                    .map(|cell| Value::String(cell.text.clone()))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let mut summary = Map::new();
    summary.insert("slide".to_string(), json!(slide));
    summary.insert("shapeId".to_string(), json!(shape.id));
    summary.insert("shapeName".to_string(), json!(shape.name));
    summary.insert(
        "targetKind".to_string(),
        target
            .get("targetKind")
            .cloned()
            .unwrap_or_else(|| json!("table")),
    );
    summary.insert(
        "primarySelector".to_string(),
        target
            .get("primarySelector")
            .cloned()
            .unwrap_or_else(|| json!(format!("shape:{}", shape.id))),
    );
    summary.insert(
        "selectors".to_string(),
        target
            .get("selectors")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    summary.insert("rows".to_string(), json!(table.rows.len()));
    summary.insert("cols".to_string(), json!(table_column_count(table)));
    summary.insert("cells".to_string(), Value::Array(cells));
    if let Some(bounds) = shape.bounds.as_ref() {
        summary.insert("bounds".to_string(), bounds_json(bounds));
    }
    if include_details {
        summary.insert("tableInfo".to_string(), table_info_json(table));
    }
    Value::Object(summary)
}

fn pptx_extract_text_shapes(xml: &str) -> Vec<Value> {
    let shapes = pptx_shape_models(xml);
    let targets = pptx_selector_targets_from_shapes(&shapes);
    shapes
        .iter()
        .zip(targets)
        .filter(|(shape, _target)| shape.kind == "sp" && shape.has_text_body)
        .map(|(shape, target)| {
            let key = pptx_extract_text_shape_key(shape, &target);
            json!({
                "id": shape.id,
                "name": shape.name,
                "type": shape.kind,
                "key": key,
                "text": pptx_extract_text_body(shape),
            })
        })
        .collect()
}

fn pptx_extract_text_shape_key(shape: &Shape, target: &Value) -> String {
    let lower_name = shape.name.to_ascii_lowercase();
    if lower_name.contains("content placeholder")
        && let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index)
    {
        return format!("body:{index}");
    }
    target
        .get("primarySelector")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn pptx_extract_text_body(shape: &Shape) -> Value {
    pptx_text_block_from_paragraphs(&shape.paragraphs, true, true)
}

fn pptx_text_block_from_paragraphs(
    paragraphs: &[Vec<String>],
    include_body_properties: bool,
    synthesize_empty_paragraph: bool,
) -> Value {
    let paragraphs = if paragraphs.is_empty() && synthesize_empty_paragraph {
        vec![Vec::<String>::new()]
    } else {
        paragraphs.to_vec()
    };
    let paragraph_values = paragraphs
        .iter()
        .map(|runs| {
            let text = runs.join("");
            let mut paragraph = Map::new();
            if !runs.is_empty() {
                paragraph.insert(
                    "runs".to_string(),
                    Value::Array(runs.iter().map(|run| json!({"text": run})).collect()),
                );
            }
            paragraph.insert("text".to_string(), json!(text));
            Value::Object(paragraph)
        })
        .collect::<Vec<_>>();
    let plain_text = paragraphs
        .iter()
        .map(|runs| runs.join(""))
        .collect::<Vec<_>>()
        .join("\n");
    let mut block = Map::new();
    block.insert("paragraphs".to_string(), Value::Array(paragraph_values));
    block.insert("plainText".to_string(), json!(plain_text));
    if include_body_properties {
        block.insert("bodyProperties".to_string(), json!({}));
    }
    Value::Object(block)
}

#[derive(Clone, Default)]
struct ShapeText {
    key: String,
    name: String,
    text: String,
}

fn pptx_slide_texts(file: &str) -> CliResult<Vec<Vec<ShapeText>>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut out = Vec::new();
    for (_, rel_id) in slides {
        let target = rels
            .get(&rel_id)
            .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
        let part = normalize_ppt_target(target);
        let xml = zip_text(file, &part)?;
        out.push(
            pptx_shape_models(&xml)
                .into_iter()
                .filter(|shape| !shape.text.is_empty())
                .map(|shape| ShapeText {
                    key: shape_key(&shape),
                    name: shape.name,
                    text: shape.text,
                })
                .collect(),
        );
    }
    Ok(out)
}

fn shape_key(shape: &Shape) -> String {
    if shape.is_placeholder && shape.name.to_ascii_lowercase().contains("title") {
        "title".to_string()
    } else if !shape.name.is_empty() {
        shape.name.clone()
    } else {
        format!("shape:{}", shape.id)
    }
}

fn pptx_slide_refs(xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let (Some(id), Some(rel)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    slides.push((id, rel));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

fn slide_part_relationships(file: &str, slide_part: &str) -> CliResult<BTreeMap<String, String>> {
    let name = Path::new(slide_part)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::unexpected(format!("invalid slide part {slide_part}")))?;
    relationships(file, &format!("ppt/slides/_rels/{name}.rels"))
}

fn normalize_ppt_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("ppt/") {
        target.to_string()
    } else {
        format!("ppt/{}", target.trim_start_matches("../"))
    }
}

fn normalize_xl_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("xl/") {
        target.to_string()
    } else {
        format!("xl/{}", target.trim_start_matches("../"))
    }
}

fn slide_layout_part(file: &str, slide_part: &str) -> CliResult<Option<String>> {
    slide_layout_and_notes_parts(file, slide_part).map(|(layout, _)| layout)
}

fn slide_layout_and_notes_parts(
    file: &str,
    slide_part: &str,
) -> CliResult<(Option<String>, Option<String>)> {
    let name = Path::new(slide_part)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::unexpected(format!("invalid slide part {slide_part}")))?;
    let rels_part = format!("ppt/slides/_rels/{name}.rels");
    let rels = relationship_entries(file, &rels_part)?;
    let mut layout_part = None;
    let mut notes_part = None;
    for rel in rels {
        match rel.rel_type.as_str() {
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" => {
                layout_part = Some(normalize_ppt_target(&rel.target));
            }
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" => {
                notes_part = Some(normalize_ppt_target(&rel.target));
            }
            _ => {}
        }
    }
    Ok((layout_part, notes_part))
}

fn layout_display_name(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cSld" =>
            {
                return attr(&e, "name");
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn trailing_number(path: &str, stem: &str) -> Option<u32> {
    let file_name = Path::new(path).file_stem()?.to_str()?;
    file_name.strip_prefix(stem)?.parse::<u32>().ok()
}

#[derive(Default)]
struct Shape {
    id: u32,
    name: String,
    kind: String,
    is_placeholder: bool,
    has_text_body: bool,
    text: String,
    paragraphs: Vec<Vec<String>>,
    bounds: Option<Bounds>,
    placeholder: Option<Placeholder>,
    image_rel_id: String,
    table: Option<TableInfo>,
}

#[derive(Clone)]
struct Placeholder {
    literal_type: String,
    index: Option<u32>,
}

#[derive(Clone)]
struct Bounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

#[derive(Default)]
struct TableInfo {
    columns: Vec<i64>,
    rows: Vec<TableRow>,
}

#[derive(Default)]
struct TableRow {
    height: Option<i64>,
    cells: Vec<TableCell>,
}

#[derive(Clone)]
struct TableCell {
    text: String,
    grid_span: u32,
    row_span: u32,
}

impl Default for TableCell {
    fn default() -> Self {
        Self {
            text: String::new(),
            grid_span: 1,
            row_span: 1,
        }
    }
}

fn pptx_shapes(xml: &str) -> Vec<Value> {
    pptx_shape_models(xml)
        .into_iter()
        .map(|shape| {
            let mut map = Map::new();
            map.insert("id".to_string(), json!(shape.id));
            map.insert("shapeName".to_string(), json!(shape.name));
            map.insert("type".to_string(), json!(shape.kind));
            if let Some(bounds) = shape.bounds.as_ref() {
                map.insert("bounds".to_string(), bounds_json(bounds));
            }
            map.insert("isPlaceholder".to_string(), json!(shape.is_placeholder));
            if !shape.text.is_empty() {
                map.insert("textContent".to_string(), json!(shape.text));
            }
            if let Some(table) = shape.table.as_ref() {
                map.insert("tableInfo".to_string(), table_info_json(table));
            }
            if !shape.image_rel_id.is_empty() {
                map.insert(
                    "imageRef".to_string(),
                    image_ref_json(&shape.image_rel_id, "", ""),
                );
            }
            Value::Object(map)
        })
        .collect()
}

fn pptx_slide_object_counts(xml: &str) -> (usize, usize, usize) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut text_shapes = 0;
    let mut images = 0;
    let mut tables = 0;
    let mut path = Vec::<String>::new();
    let mut current_shape: Option<(String, usize, bool, bool)> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current_shape.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && matches!(name.as_str(), "sp" | "pic" | "graphicFrame")
                {
                    current_shape = Some((name.clone(), path.len() + 1, false, false));
                } else if let Some((kind, _, has_text, has_table)) = current_shape.as_mut() {
                    if kind == "sp" && name == "txBody" {
                        *has_text = true;
                    }
                    if kind == "graphicFrame" && name == "tbl" {
                        *has_table = true;
                    }
                }
                path.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current_shape.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && name == "pic"
                {
                    images += 1;
                } else if let Some((kind, _, has_text, has_table)) = current_shape.as_mut() {
                    if kind == "sp" && name == "txBody" {
                        *has_text = true;
                    }
                    if kind == "graphicFrame" && name == "tbl" {
                        *has_table = true;
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some((kind, depth, has_text, has_table)) = current_shape.take() {
                    if path.len() == depth && name == kind {
                        match kind.as_str() {
                            "sp" if has_text => text_shapes += 1,
                            "pic" => images += 1,
                            "graphicFrame" if has_table => tables += 1,
                            _ => {}
                        }
                    } else {
                        current_shape = Some((kind, depth, has_text, has_table));
                    }
                }
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    (text_shapes, images, tables)
}

fn pptx_selector_targets(xml: &str) -> Vec<Value> {
    let shapes = pptx_shape_models(xml);
    pptx_selector_targets_from_shapes(&shapes)
}

fn pptx_selector_targets_from_shapes(shapes: &[Shape]) -> Vec<Value> {
    let mut name_counts = BTreeMap::<String, usize>::new();
    let mut index_counts = BTreeMap::<u32, usize>::new();
    for shape in shapes {
        if !shape.name.trim().is_empty() {
            *name_counts.entry(shape.name.clone()).or_default() += 1;
        }
        if let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index) {
            *index_counts.entry(index).or_default() += 1;
        }
    }

    let mut table_index = 0_u32;
    shapes
        .iter()
        .enumerate()
        .map(|(index, shape)| {
            let is_table = shape.kind == "graphicFrame" && shape.table.is_some();
            if is_table {
                table_index += 1;
            }
            let placeholder = shape
                .placeholder
                .as_ref()
                .and_then(pptx_selector_placeholder);
            let placeholder_key = placeholder
                .as_ref()
                .and_then(|placeholder| placeholder.get("key"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let placeholder_role = placeholder
                .as_ref()
                .and_then(|placeholder| placeholder.get("role"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let mut primary_selector = format!("shape:{}", shape.id);
            if is_table {
                primary_selector = format!("table:{table_index}");
            } else if !placeholder_key.is_empty() {
                primary_selector.clone_from(&placeholder_key);
            }
            let mut selectors = Vec::<String>::new();
            if is_table {
                add_selector(&mut selectors, format!("shape:{}", shape.id));
                add_selector(&mut selectors, format!("table:{table_index}"));
            } else {
                add_selector(&mut selectors, placeholder_key.clone());
                if !placeholder_role.is_empty() {
                    add_selector(&mut selectors, format!("@{placeholder_role}"));
                    add_selector(&mut selectors, placeholder_role.clone());
                    if let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index) {
                        add_selector(&mut selectors, format!("{placeholder_role}:{index}"));
                    }
                }
                if let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index)
                    && index_counts.get(&index).copied().unwrap_or_default() == 1
                {
                    add_selector(&mut selectors, format!("#{index}"));
                }
                add_selector(&mut selectors, format!("shape:{}", shape.id));
            }
            if name_counts.get(&shape.name).copied().unwrap_or_default() == 1 {
                add_selector(&mut selectors, format!("~{}", shape.name));
            }

            let text_preview = normalized_text_preview(&shape.text);
            let mut target = Map::new();
            target.insert("order".to_string(), json!(index + 1));
            target.insert("shapeId".to_string(), json!(shape.id));
            if !shape.name.is_empty() {
                target.insert("shapeName".to_string(), json!(shape.name));
            }
            target.insert("shapeType".to_string(), json!(shape.kind));
            target.insert(
                "targetKind".to_string(),
                json!(if is_table {
                    "table".to_string()
                } else if shape.kind == "pic" {
                    "picture".to_string()
                } else if !placeholder_role.is_empty() {
                    placeholder_role
                } else if shape.has_text_body {
                    "textbox".to_string()
                } else if shape.is_placeholder {
                    "placeholder".to_string()
                } else {
                    "shape".to_string()
                }),
            );
            target.insert(
                "textCapable".to_string(),
                json!(shape.kind == "sp" && shape.has_text_body),
            );
            if !text_preview.is_empty() {
                target.insert("textPreview".to_string(), json!(text_preview));
            }
            target.insert("primarySelector".to_string(), json!(primary_selector));
            target.insert("selectors".to_string(), json!(selectors));
            if let Some(placeholder) = placeholder {
                target.insert("placeholder".to_string(), Value::Object(placeholder));
            }
            Value::Object(target)
        })
        .collect()
}

fn add_selector(selectors: &mut Vec<String>, selector: String) {
    if selector.trim().is_empty() || selectors.iter().any(|existing| existing == &selector) {
        return;
    }
    selectors.push(selector);
}

fn pptx_shape_show_entries(
    file: &str,
    slide_part: &str,
    xml: &str,
    slide_id: u32,
    slide_id_unique: bool,
    include_text: bool,
    include_bounds: bool,
) -> Vec<Value> {
    let shapes = pptx_shape_models(xml);
    let mut id_counts = BTreeMap::<u32, usize>::new();
    for shape in &shapes {
        if shape.id != 0 {
            *id_counts.entry(shape.id).or_default() += 1;
        }
    }
    let targets = pptx_selector_targets_from_shapes(&shapes);
    let slide_relationships = slide_part_relationships(file, slide_part).unwrap_or_default();
    shapes
        .iter()
        .zip(targets)
        .map(|(shape, target)| {
            let mut entry = target.as_object().cloned().unwrap_or_default();
            if slide_id_unique && id_counts.get(&shape.id).copied().unwrap_or_default() == 1 {
                entry.insert(
                    "handle".to_string(),
                    json!(format!("H:pptx/s:{slide_id}/shape:n:{}", shape.id)),
                );
            }
            if !include_text {
                entry.remove("textPreview");
            }
            if include_bounds && let Some(bounds) = shape.bounds.as_ref() {
                entry.insert("bounds".to_string(), bounds_json(bounds));
            }
            if let Some(table) = shape.table.as_ref() {
                entry.insert("tableInfo".to_string(), table_info_json(table));
            }
            if !shape.image_rel_id.is_empty() {
                let target_uri = slide_relationships
                    .get(&shape.image_rel_id)
                    .map(|target| format!("/{}", normalize_ppt_target(target)))
                    .unwrap_or_default();
                let content_type = if target_uri.is_empty() {
                    String::new()
                } else {
                    content_type_for_part(file, &target_uri).unwrap_or_default()
                };
                entry.insert(
                    "imageRef".to_string(),
                    image_ref_json(&shape.image_rel_id, &target_uri, &content_type),
                );
            }
            Value::Object(entry)
        })
        .collect()
}

fn bounds_json(bounds: &Bounds) -> Value {
    json!({
        "x": bounds.x,
        "y": bounds.y,
        "cx": bounds.cx,
        "cy": bounds.cy,
    })
}

fn image_ref_json(rel_id: &str, target_uri: &str, content_type: &str) -> Value {
    json!({
        "relId": rel_id,
        "targetUri": target_uri,
        "contentType": content_type,
    })
}

fn table_info_json(table: &TableInfo) -> Value {
    let cells = table
        .rows
        .iter()
        .map(|row| {
            Value::Array(
                row.cells
                    .iter()
                    .map(|cell| Value::String(cell.text.clone()))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let row_defs = table
        .rows
        .iter()
        .map(|row| {
            let mut row_def = Map::new();
            if let Some(height) = row.height {
                row_def.insert("height".to_string(), json!(height));
            }
            row_def.insert("cells".to_string(), table_cells_json(&row.cells));
            Value::Object(row_def)
        })
        .collect::<Vec<_>>();
    let column_defs = table
        .columns
        .iter()
        .map(|width| json!({"width": width}))
        .collect::<Vec<_>>();
    let cell_defs = table
        .rows
        .iter()
        .map(|row| table_cells_json(&row.cells))
        .collect::<Vec<_>>();
    json!({
        "rows": table.rows.len(),
        "cols": table_column_count(table),
        "cells": cells,
        "rowDefs": row_defs,
        "columnDefs": column_defs,
        "cellDefs": cell_defs,
    })
}

fn table_cells_json(cells: &[TableCell]) -> Value {
    Value::Array(
        cells
            .iter()
            .map(|cell| {
                json!({
                    "text": cell.text.clone(),
                    "gridSpan": cell.grid_span,
                    "rowSpan": cell.row_span,
                })
            })
            .collect(),
    )
}

fn table_column_count(table: &TableInfo) -> usize {
    table.columns.len().max(
        table
            .rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(0),
    )
}

fn pptx_selector_placeholder(ph: &Placeholder) -> Option<Map<String, Value>> {
    let role = placeholder_role(&ph.literal_type);
    if role.is_empty() {
        return None;
    }
    let key = role.clone();
    let mut placeholder = Map::new();
    placeholder.insert("key".to_string(), json!(key));
    placeholder.insert("role".to_string(), json!(role));
    if let Some(index) = ph.index {
        placeholder.insert("index".to_string(), json!(index));
    }
    if !ph.literal_type.is_empty() {
        placeholder.insert("literalType".to_string(), json!(ph.literal_type));
        placeholder.insert("resolvedType".to_string(), json!(ph.literal_type));
        placeholder.insert("typeSource".to_string(), json!("slide"));
    }
    Some(placeholder)
}

fn placeholder_role(literal_type: &str) -> String {
    match literal_type {
        "ctrTitle" | "title" => "title",
        "subTitle" => "subtitle",
        "body" | "obj" => "body",
        "pic" => "picture",
        other => other,
    }
    .to_string()
}

fn normalized_text_preview(text: &str) -> String {
    let preview = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if preview.len() > 140 {
        format!("{}...", &preview[..137])
    } else {
        preview
    }
}

fn pptx_shape_models(xml: &str) -> Vec<Shape> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut shapes = Vec::new();
    let mut current: Option<Shape> = None;
    let mut current_end = String::new();
    let mut in_text = false;
    let mut in_shape_text_body = false;
    let mut in_table = false;
    let mut current_row: Option<TableRow> = None;
    let mut current_cell: Option<TableCell> = None;
    let mut current_paragraph: Option<Vec<String>> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e))
                if current.is_none()
                    && matches!(local_name(e.name().as_ref()), "sp" | "pic" | "graphicFrame") =>
            {
                let kind = local_name(e.name().as_ref()).to_string();
                current_end.clone_from(&kind);
                current = Some(Shape {
                    kind,
                    ..Shape::default()
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "cNvPr" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.id = attr(&e, "id")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_default();
                    shape.name = attr(&e, "name").unwrap_or_default();
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "ph" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.is_placeholder = true;
                    shape.placeholder = Some(Placeholder {
                        literal_type: attr(&e, "type").unwrap_or_default(),
                        index: attr(&e, "idx").and_then(|idx| idx.parse().ok()),
                    });
                }
            }
            Ok(Event::Start(e))
                if current.as_ref().is_some_and(|shape| shape.kind == "sp")
                    && local_name(e.name().as_ref()) == "txBody" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.has_text_body = true;
                }
                in_shape_text_body = true;
            }
            Ok(Event::Empty(e))
                if current.as_ref().is_some_and(|shape| shape.kind == "sp")
                    && local_name(e.name().as_ref()) == "txBody" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.has_text_body = true;
                }
            }
            Ok(Event::Start(e)) if in_shape_text_body && local_name(e.name().as_ref()) == "p" => {
                current_paragraph = Some(Vec::new());
            }
            Ok(Event::Empty(e)) if in_shape_text_body && local_name(e.name().as_ref()) == "p" => {
                if let Some(shape) = current.as_mut() {
                    shape.paragraphs.push(Vec::new());
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "off" =>
            {
                if let Some(shape) = current.as_mut() {
                    let mut bounds = shape.bounds.clone().unwrap_or(Bounds {
                        x: 0,
                        y: 0,
                        cx: 0,
                        cy: 0,
                    });
                    bounds.x = attr(&e, "x")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.x);
                    bounds.y = attr(&e, "y")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.y);
                    shape.bounds = Some(bounds);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "ext" =>
            {
                if let Some(shape) = current.as_mut() {
                    let mut bounds = shape.bounds.clone().unwrap_or(Bounds {
                        x: 0,
                        y: 0,
                        cx: 0,
                        cy: 0,
                    });
                    bounds.cx = attr(&e, "cx")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.cx);
                    bounds.cy = attr(&e, "cy")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.cy);
                    shape.bounds = Some(bounds);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.as_ref().is_some_and(|shape| shape.kind == "pic")
                    && local_name(e.name().as_ref()) == "blip" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.image_rel_id = attr(&e, "embed").unwrap_or_default();
                }
            }
            Ok(Event::Start(e)) if current.is_some() && local_name(e.name().as_ref()) == "tbl" => {
                in_table = true;
                if let Some(shape) = current.as_mut() {
                    shape.table = Some(TableInfo::default());
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_table && local_name(e.name().as_ref()) == "gridCol" =>
            {
                if let Some(table) = current.as_mut().and_then(|shape| shape.table.as_mut())
                    && let Some(width) = attr(&e, "w").and_then(|value| value.parse().ok())
                {
                    table.columns.push(width);
                }
            }
            Ok(Event::Start(e)) if in_table && local_name(e.name().as_ref()) == "tr" => {
                current_row = Some(TableRow {
                    height: attr(&e, "h").and_then(|value| value.parse().ok()),
                    cells: Vec::new(),
                });
            }
            Ok(Event::Start(e)) if in_table && local_name(e.name().as_ref()) == "tc" => {
                current_cell = Some(TableCell {
                    grid_span: attr(&e, "gridSpan")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1),
                    row_span: attr(&e, "rowSpan")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1),
                    ..TableCell::default()
                });
            }
            Ok(Event::Start(e)) if current.is_some() && local_name(e.name().as_ref()) == "t" => {
                in_text = true;
            }
            Ok(Event::Text(e)) if in_text => {
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                if let Some(cell) = current_cell.as_mut() {
                    cell.text.push_str(&text);
                } else if let Some(shape) = current.as_mut()
                    && shape.kind == "sp"
                {
                    shape.text.push_str(&text);
                    if in_shape_text_body && let Some(paragraph) = current_paragraph.as_mut() {
                        paragraph.push(text);
                    }
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => {
                in_text = false;
            }
            Ok(Event::End(e)) if in_shape_text_body && local_name(e.name().as_ref()) == "p" => {
                if let Some(paragraph) = current_paragraph.take()
                    && let Some(shape) = current.as_mut()
                {
                    shape.paragraphs.push(paragraph);
                }
            }
            Ok(Event::End(e))
                if in_shape_text_body && local_name(e.name().as_ref()) == "txBody" =>
            {
                in_shape_text_body = false;
            }
            Ok(Event::End(e)) if in_table && local_name(e.name().as_ref()) == "tc" => {
                if let Some(cell) = current_cell.take()
                    && let Some(row) = current_row.as_mut()
                {
                    row.cells.push(cell);
                }
            }
            Ok(Event::End(e)) if in_table && local_name(e.name().as_ref()) == "tr" => {
                if let Some(row) = current_row.take()
                    && let Some(table) = current.as_mut().and_then(|shape| shape.table.as_mut())
                {
                    table.rows.push(row);
                }
            }
            Ok(Event::End(e)) if in_table && local_name(e.name().as_ref()) == "tbl" => {
                in_table = false;
            }
            Ok(Event::End(e))
                if current.is_some() && local_name(e.name().as_ref()) == current_end =>
            {
                if let Some(shape) = current.take() {
                    shapes.push(shape);
                }
                current_end.clear();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    shapes
}
