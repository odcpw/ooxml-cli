mod docx;
mod pptx;
mod xlsx;

use serde_json::{Value, json};

use super::op::ServeOp;
use crate::{
    CliError, CliResult, DispatchBody, GlobalFlags, dispatch, json_string, json_u32,
    pptx_replace_text_in_place,
};

pub(super) fn serve_op_command(working: &str, command: &str, args: &Value) -> CliResult<ServeOp> {
    let op = match command {
        generic_command if is_generic_serve_mutation_command(generic_command) => {
            serve_generic_mutation_op(working, generic_command, args)?
        }
        family_command if family_command.starts_with("xlsx ") => {
            xlsx::serve_xlsx_op(working, family_command, args)?
        }
        family_command if family_command.starts_with("docx ") => {
            docx::serve_docx_op(working, family_command, args)?
        }
        "pptx replace text" => {
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
        family_command if family_command.starts_with("pptx ") => {
            pptx::serve_pptx_op(working, family_command, args)?
        }
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}

fn is_generic_serve_mutation_command(command: &str) -> bool {
    matches!(
        command,
        "repair normalize"
            | "template apply"
            | "xlsx sheets add"
            | "xlsx sheets rename"
            | "xlsx sheets move"
            | "xlsx sheets delete"
            | "xlsx data-validations create"
            | "xlsx data-validations update"
            | "xlsx data-validations delete"
            | "xlsx hyperlinks add"
            | "xlsx hyperlinks update"
            | "xlsx hyperlinks delete"
            | "xlsx names add"
            | "xlsx names update"
            | "xlsx names rename"
            | "xlsx names delete"
            | "xlsx tables create"
            | "xlsx freeze set"
            | "xlsx freeze clear"
            | "docx replace"
            | "docx tables create"
            | "vba create"
            | "vba rebuild"
            | "vba attach"
            | "vba remove"
    )
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
