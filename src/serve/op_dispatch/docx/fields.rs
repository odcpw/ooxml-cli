use serde_json::Value;

use super::super::super::op::{ServeOp, push_serve_plan_bool_flag, push_serve_plan_string_flag};
use crate::command_manifest::DocxCommandId;
use crate::docx_fields::docx_fields_insert_toc;
use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, docx_fields_insert, docx_fields_set_result,
    json_bool, json_optional_string, json_string,
};

pub(super) fn serve_docx_fields_op(
    working: &str,
    command_id: DocxCommandId,
    command: &str,
    args: &Value,
) -> CliResult<ServeOp> {
    let op = match command_id {
        DocxCommandId::FieldsInsert => {
            let toc = json_bool(args, "toc").unwrap_or(false);
            let location = json_optional_string(args, "location");
            let field_code = json_optional_string(args, "field-code")
                .or_else(|| json_optional_string(args, "fieldCode"));
            let result = json_optional_string(args, "result").unwrap_or_default();
            let result_set = args.get("result").is_some();
            let levels = json_optional_string(args, "levels");
            let mutation = DocxParagraphMutationOptions {
                text: None,
                text_file: None,
                style: "",
                out: None,
                backup: None,
                dry_run: false,
                in_place: true,
                no_validate: true,
            };
            let readback = if toc {
                if location.is_some() || field_code.is_some() || result_set {
                    return Err(CliError::invalid_args(
                        "toc cannot be combined with location, field-code, or result",
                    ));
                }
                docx_fields_insert_toc(working, levels.as_deref().unwrap_or("1-3"), mutation)?
            } else {
                let location = location
                    .as_deref()
                    .ok_or_else(|| CliError::invalid_args("location is required"))?;
                let field_code = field_code
                    .as_deref()
                    .ok_or_else(|| CliError::invalid_args("field-code is required"))?;
                docx_fields_insert(working, location, field_code, &result, mutation)?
            };
            let mut plan_flags = Vec::new();
            push_serve_plan_bool_flag(&mut plan_flags, "--toc", toc.then_some(true));
            push_serve_plan_string_flag(&mut plan_flags, "--levels", levels.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--location", location.as_deref());
            push_serve_plan_string_flag(&mut plan_flags, "--field-code", field_code.as_deref());
            if result_set && !toc {
                push_serve_plan_string_flag(&mut plan_flags, "--result", Some(&result));
            }
            ServeOp::DocxFieldsOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        DocxCommandId::FieldsSetResult => {
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
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}
