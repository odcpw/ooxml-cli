use serde_json::{Value, json};
use std::fs;

use super::super::super::op::{ServeOp, push_serve_plan_string_flag};
use crate::{
    CliError, CliResult, DocxCommentEditSpec, DocxParagraphMutationOptions, current_utc_rfc3339,
    docx_comments_add, docx_comments_edit, docx_comments_remove, json_i64, json_optional_string,
};

pub(super) fn serve_docx_comments_op(
    working: &str,
    command: &str,
    args: &Value,
) -> CliResult<ServeOp> {
    let op = match command {
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
                working,
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
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
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
            let text_file_set = args.get("text-file").is_some() || args.get("textFile").is_some();
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
                working,
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
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
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
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
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
                working,
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
