use serde_json::{Value, json};

mod comments;
mod tables;

use super::super::op::{ServeOp, push_serve_plan_bool_flag, push_serve_plan_string_flag};
use crate::{
    CliError, CliResult, DocxHeaderFooterSetTextOptions, DocxParagraphMutationOptions,
    DocxStyleApplyOptions, DocxStyleTarget, docx_blocks_delete, docx_blocks_insert_after,
    docx_blocks_replace, docx_fields_insert, docx_fields_set_result, docx_headers_footers_set_text,
    docx_paragraphs_append, docx_paragraphs_clear, docx_paragraphs_insert, docx_paragraphs_set,
    docx_styles_apply, json_bool, json_i64, json_optional_string, json_string,
    normalize_docx_header_footer_show_type, normalize_docx_style_target, require_docx_block_hash,
    resolve_required_docx_paragraph_set_text, resolve_required_docx_table_text,
};

pub(super) fn serve_docx_op(working: &str, command: &str, args: &Value) -> CliResult<ServeOp> {
    let op = match command {
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
            let text_file_set = args.get("text-file").is_some() || args.get("textFile").is_some();
            let text = resolve_required_docx_table_text(
                text.as_deref(),
                text_file.as_deref(),
                text_set,
                text_file_set,
            )?;
            let readback = docx_headers_footers_set_text(
                working,
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
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
            }
            ServeOp::DocxHeaderFooterSetText {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
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
                working,
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
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
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
                working,
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
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        "docx paragraphs append" => {
            let text = json_optional_string(args, "text");
            let text_file = json_optional_string(args, "text-file")
                .or_else(|| json_optional_string(args, "textFile"));
            let style = json_optional_string(args, "style").unwrap_or_default();
            let readback = docx_paragraphs_append(
                working,
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
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
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
                working,
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
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
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
            let text_file_set = args.get("text-file").is_some() || args.get("textFile").is_some();
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
                working,
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
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
            }
            ServeOp::DocxParagraphsOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
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
                working,
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
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
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
                working,
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
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
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
                working,
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
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
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
                working,
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
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
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
                working,
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
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        family_command if family_command.starts_with("docx comments ") => {
            comments::serve_docx_comments_op(working, family_command, args)?
        }
        family_command if family_command.starts_with("docx tables ") => {
            tables::serve_docx_tables_op(working, family_command, args)?
        }
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}
