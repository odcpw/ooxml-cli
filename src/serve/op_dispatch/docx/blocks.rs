use serde_json::{Value, json};

use super::super::super::op::{ServeOp, push_serve_plan_string_flag};
use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, docx_blocks_delete,
    docx_blocks_insert_after, docx_blocks_replace, json_i64, json_optional_string,
    require_docx_block_hash,
};

pub(super) fn serve_docx_blocks_op(
    working: &str,
    command: &str,
    args: &Value,
) -> CliResult<ServeOp> {
    let op = match command {
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
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}
