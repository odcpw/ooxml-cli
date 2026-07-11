use serde_json::{Value, json};

use super::super::super::op::{ServeOp, push_serve_plan_bool_flag, push_serve_plan_string_flag};
use crate::command_manifest::DocxCommandId;
use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, DocxStyleApplyOptions, DocxStyleTarget,
    docx_styles_apply, json_bool, json_i64, json_optional_string, normalize_docx_style_target,
    require_docx_block_hash,
};

pub(super) fn serve_docx_styles_op(
    working: &str,
    command_id: DocxCommandId,
    command: &str,
    args: &Value,
) -> CliResult<ServeOp> {
    let op = match command_id {
        DocxCommandId::StylesApply => {
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
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}
