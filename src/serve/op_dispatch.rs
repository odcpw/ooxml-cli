mod docx;
mod pptx;
mod xlsx;

use serde_json::{Value, json};

use super::op::ServeOp;
use super::op_namespace::resolve_serve_mutation_command;
use crate::command_manifest::{
    CommandId, CoreCommandId, DocxCommandId, PptxCommandId, VbaCommandId, XlsxCommandId,
};
use crate::{
    CliError, CliResult, DispatchBody, GlobalFlags, dispatch, json_string, json_u32,
    pptx_replace_text_in_place,
};

pub(super) fn serve_op_command(working: &str, command: &str, args: &Value) -> CliResult<ServeOp> {
    let Some(command_id) = resolve_serve_mutation_command(command) else {
        return Err(CliError::invalid_args(format!(
            "unsupported serve op command: {command}"
        )));
    };
    let op = match command_id {
        CommandId::Core(CoreCommandId::RepairNormalize | CoreCommandId::TemplateApply)
        | CommandId::Xlsx(
            XlsxCommandId::SheetsAdd
            | XlsxCommandId::SheetsRename
            | XlsxCommandId::SheetsMove
            | XlsxCommandId::SheetsDelete
            | XlsxCommandId::DataValidationsCreate
            | XlsxCommandId::DataValidationsUpdate
            | XlsxCommandId::DataValidationsDelete
            | XlsxCommandId::HyperlinksAdd
            | XlsxCommandId::HyperlinksUpdate
            | XlsxCommandId::HyperlinksDelete
            | XlsxCommandId::NamesAdd
            | XlsxCommandId::NamesUpdate
            | XlsxCommandId::NamesRename
            | XlsxCommandId::NamesDelete
            | XlsxCommandId::TablesCreate
            | XlsxCommandId::FreezeSet
            | XlsxCommandId::FreezeClear,
        )
        | CommandId::Docx(DocxCommandId::Replace | DocxCommandId::TablesCreate)
        | CommandId::Vba(
            VbaCommandId::Create
            | VbaCommandId::Rebuild
            | VbaCommandId::Attach
            | VbaCommandId::Remove,
        ) => serve_generic_mutation_op(working, command, args)?,
        CommandId::Xlsx(id) => xlsx::serve_xlsx_op(working, id, command, args)?,
        CommandId::Docx(id) => docx::serve_docx_op(working, id, command, args)?,
        CommandId::Pptx(PptxCommandId::ReplaceText) => {
            let slide = json_u32(args, "slide")?.unwrap_or(1);
            let target = json_string(args, "target")?;
            let text = json_string(args, "text")?;
            let readback = pptx_replace_text_in_place(working, slide, &target, &text)?;
            let plan_flags = vec![
                json!("--slide"),
                json!(slide.to_string()),
                json!("--target"),
                json!(target),
                json!("--text"),
                json!(text),
            ];
            ServeOp::PptxReplaceOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        CommandId::Pptx(id) => pptx::serve_pptx_op(working, id, command, args)?,
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}

fn serve_generic_mutation_op(working: &str, command: &str, args: &Value) -> CliResult<ServeOp> {
    let plan_flags = json_args_to_plan_flags(args)?;
    let mut argv = command
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    argv.push(working.to_string());
    argv.extend(plan_flags_to_cli_args(&plan_flags)?);
    argv.push("--in-place".to_string());
    argv.push("--no-validate".to_string());
    let output = dispatch(
        &GlobalFlags {
            json: true,
            format_text: false,
            strict: false,
        },
        &argv,
    )?;
    let readback = match output.body {
        DispatchBody::Json(value) => value,
        DispatchBody::Text(_) => {
            return Err(CliError::unexpected(format!(
                "serve op command returned text output: {command}"
            )));
        }
    };
    Ok(ServeOp::GenericMutationOp {
        command: command.to_string(),
        plan_flags,
        readback_file: working.to_string(),
        readback,
    })
}

fn json_args_to_plan_flags(args: &Value) -> CliResult<Vec<Value>> {
    let object = args
        .as_object()
        .ok_or_else(|| CliError::invalid_args("op args must be an object"))?;
    let mut keys = object.keys().collect::<Vec<_>>();
    keys.sort();
    let mut flags = Vec::new();
    for key in keys {
        if generic_op_ignores_arg(key) {
            continue;
        }
        let flag = json_arg_key_to_flag(key);
        append_json_arg_value(&mut flags, &flag, &object[key])?;
    }
    Ok(flags)
}

fn generic_op_ignores_arg(key: &str) -> bool {
    matches!(
        key,
        "out"
            | "output"
            | "backup"
            | "inPlace"
            | "in-place"
            | "dryRun"
            | "dry-run"
            | "noValidate"
            | "no-validate"
    )
}

fn append_json_arg_value(flags: &mut Vec<Value>, flag: &str, value: &Value) -> CliResult<()> {
    match value {
        Value::Null => {}
        Value::Bool(true) => flags.push(json!(flag)),
        Value::Bool(false) => flags.push(json!(format!("{flag}=false"))),
        Value::String(text) => {
            flags.push(json!(flag));
            flags.push(json!(text));
        }
        Value::Number(number) => {
            flags.push(json!(flag));
            flags.push(json!(number.to_string()));
        }
        Value::Array(values) => {
            for item in values {
                append_json_arg_value(flags, flag, item)?;
            }
        }
        Value::Object(_) => {
            flags.push(json!(flag));
            flags.push(json!(
                serde_json::to_string(value).expect("serialize op arg")
            ));
        }
    }
    Ok(())
}

fn json_arg_key_to_flag(key: &str) -> String {
    let key = key.trim_start_matches('-');
    let mut out = String::from("--");
    for (index, ch) in key.chars().enumerate() {
        if ch == '_' {
            out.push('-');
        } else if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn plan_flags_to_cli_args(flags: &[Value]) -> CliResult<Vec<String>> {
    flags
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .ok_or_else(|| CliError::unexpected("serve plan flag was not a string"))
        })
        .collect()
}
