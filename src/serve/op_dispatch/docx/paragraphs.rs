use serde_json::{Value, json};

use super::super::super::op::{ServeOp, push_serve_plan_string_flag};
use crate::command_manifest::DocxCommandId;
use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, docx_paragraphs_append,
    docx_paragraphs_clear, docx_paragraphs_insert, docx_paragraphs_set, json_i64,
    json_optional_string, resolve_required_docx_paragraph_set_text,
};

pub(super) fn serve_docx_paragraphs_op(
    working: &str,
    command_id: DocxCommandId,
    command: &str,
    args: &Value,
) -> CliResult<ServeOp> {
    let op = match command_id {
        DocxCommandId::ParagraphsAppend => {
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
        DocxCommandId::ParagraphsInsert => {
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
        DocxCommandId::ParagraphsSet => {
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
        DocxCommandId::ParagraphsClear => {
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
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}
