mod docx;
#[path = "path_resolver.rs"]
mod path_resolver;
mod pptx;
#[path = "ref_resolver.rs"]
mod ref_resolver;
mod xlsx;

use serde_json::{Value, json};
use std::fs;
use std::path::Path;

use super::op::ServeOp;
use super::op_namespace::resolve_serve_mutation_command;
use crate::command_manifest::{
    CommandId, CoreCommandId, DocxCommandId, PptxCommandId, VbaCommandId, XlsxCommandId,
};
use crate::{
    CliError, CliResult, DispatchBody, GlobalFlags, dispatch, json_string, json_u32,
    pptx_replace_text_in_place,
};

pub(super) use path_resolver::resolve_op_paths;
pub(super) use ref_resolver::resolve_refs;

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
        | CommandId::Docx(
            DocxCommandId::Replace
            | DocxCommandId::TablesCreate
            | DocxCommandId::TablesSetStyle
            | DocxCommandId::BreaksInsert
            | DocxCommandId::SectionsSet,
        )
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
        _ => serve_generic_mutation_op(working, command, args)?,
    };
    Ok(op)
}

pub(super) fn serve_generic_mutation_op(
    working: &str,
    command: &str,
    args: &Value,
) -> CliResult<ServeOp> {
    let parsed = generic_op_arguments(command, args)?;
    let mut plan_flags = parsed
        .positionals
        .iter()
        .cloned()
        .map(Value::String)
        .collect::<Vec<_>>();
    plan_flags.extend(parsed.flags.iter().cloned());
    let mut argv = command
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut conversion_output = None;
    match command {
        "find" => {
            argv.extend(parsed.positionals);
            argv.push(working.to_string());
            argv.extend(plan_flags_to_cli_args(&parsed.flags)?);
            if !argv.iter().any(|arg| arg == "--apply") {
                argv.push("--apply".to_string());
            }
            argv.push("--in-place".to_string());
            argv.push("--no-validate".to_string());
        }
        "docx scaffold" | "pptx scaffold" | "xlsx scaffold" => {
            argv.push(working.to_string());
            argv.extend(plan_flags_to_cli_args(&parsed.flags)?);
            if !argv.iter().any(|arg| arg == "--force") {
                argv.push("--force".to_string());
            }
            argv.push("--no-validate".to_string());
        }
        "convert xlsm-to-xlsx" => {
            let converted = Path::new(working)
                .with_extension("converted.xlsx")
                .to_string_lossy()
                .to_string();
            argv.push(working.to_string());
            argv.extend(plan_flags_to_cli_args(&parsed.flags)?);
            argv.push("--out".to_string());
            argv.push(converted.clone());
            argv.push("--no-validate".to_string());
            conversion_output = Some(converted);
        }
        "pptx template compile" => {
            argv.extend(parsed.positionals);
            argv.extend(plan_flags_to_cli_args(&parsed.flags)?);
            argv.push("--out".to_string());
            argv.push(working.to_string());
        }
        _ => {
            argv.push(working.to_string());
            argv.extend(parsed.positionals);
            argv.extend(plan_flags_to_cli_args(&parsed.flags)?);
            argv.push("--in-place".to_string());
            argv.push("--no-validate".to_string());
        }
    }
    let output = dispatch(
        &GlobalFlags {
            json: true,
            format_text: false,
            format_markdown: false,
            strict: false,
        },
        &argv,
    )?;
    let mut readback = match output.body {
        DispatchBody::Json(value) => value,
        DispatchBody::Text(_) => {
            return Err(CliError::unexpected(format!(
                "serve op command returned text output: {command}"
            )));
        }
    };
    if let Some(converted) = conversion_output {
        fs::copy(&converted, working).map_err(|error| {
            CliError::unexpected(format!(
                "failed to adopt converted XLSX into the session stage: {error}"
            ))
        })?;
        let _ = fs::remove_file(&converted);
        readback = replace_json_path(readback, &converted, working);
    }
    Ok(ServeOp::GenericMutationOp {
        command: command.to_string(),
        plan_flags,
        readback_file: working.to_string(),
        readback,
    })
}

fn replace_json_path(value: Value, from: &str, to: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace(from, to)),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| replace_json_path(value, from, to))
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (key, replace_json_path(value, from, to)))
                .collect(),
        ),
        scalar => scalar,
    }
}

struct GenericOpArguments {
    positionals: Vec<String>,
    flags: Vec<Value>,
}

fn generic_op_arguments(command: &str, args: &Value) -> CliResult<GenericOpArguments> {
    let object = args
        .as_object()
        .ok_or_else(|| CliError::invalid_args("op args must be an object"))?;
    let capability_path = format!("ooxml {command}");
    let capability = crate::capabilities::capability_commands()
        .into_iter()
        .find(|candidate| candidate["path"].as_str() == Some(capability_path.as_str()))
        .ok_or_else(|| CliError::invalid_args(format!("unknown op command: {command}")))?;
    let positional_names = generic_positional_arg_names(command);
    let mut positionals = Vec::with_capacity(positional_names.len());
    for name in positional_names {
        let value = object
            .iter()
            .find(|(key, _)| normalize_op_arg_key(key) == *name)
            .map(|(_, value)| value)
            .ok_or_else(|| CliError::invalid_args(format!("{name} is required")))?;
        positionals.push(scalar_op_arg(name, value)?);
    }

    let mut keys = object.keys().collect::<Vec<_>>();
    keys.sort();
    let mut flags = Vec::new();
    for key in keys {
        let normalized = normalize_op_arg_key(key);
        if positional_names.contains(&normalized.as_str()) {
            continue;
        }
        if generic_op_rejects_arg(&normalized) {
            return Err(CliError::invalid_args(format!(
                "op arg {key:?} is owned by the apply/serve/MCP session; omit it from op args"
            )));
        }
        let flag = canonical_op_flag(&capability, &normalized).ok_or_else(|| {
            CliError::invalid_args(format!(
                "unknown op arg {key:?} for {command}; use a local flag name, manifest argName, or registered flag alias"
            ))
        })?;
        append_json_arg_value(&mut flags, &flag, &object[key])?;
    }
    Ok(GenericOpArguments { positionals, flags })
}

fn generic_op_rejects_arg(key: &str) -> bool {
    matches!(
        key,
        "out"
            | "output"
            | "backup"
            | "in-place"
            | "dry-run"
            | "no-validate"
            | "json"
            | "pretty"
            | "strict"
    )
}

fn generic_positional_arg_names(command: &str) -> &'static [&'static str] {
    match command {
        "find" => &["query"],
        "pptx slides delete" => &["slide-number"],
        "pptx slides move" => &["from-position", "to-position"],
        "pptx slides reorder" => &["order"],
        "pptx slides merge" => &["source-file"],
        "pptx template compile" => &["manifest", "spec"],
        _ => &[],
    }
}

fn scalar_op_arg(name: &str, value: &Value) -> CliResult<String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        _ => Err(CliError::invalid_args(format!(
            "{name} must be a string, number, or boolean"
        ))),
    }
}

fn canonical_op_flag(capability: &Value, normalized_key: &str) -> Option<String> {
    capability["localFlags"]
        .as_array()?
        .iter()
        .find_map(|flag| {
            let canonical = flag["name"].as_str()?;
            let canonical_matches = normalize_op_arg_key(canonical) == normalized_key;
            let arg_name_matches = flag["argName"]
                .as_str()
                .is_some_and(|name| normalize_op_arg_key(name) == normalized_key);
            let alias_matches = flag["aliases"].as_array().is_some_and(|aliases| {
                aliases.iter().any(|alias| {
                    alias
                        .as_str()
                        .is_some_and(|name| normalize_op_arg_key(name) == normalized_key)
                })
            });
            (canonical_matches || arg_name_matches || alias_matches).then(|| canonical.to_string())
        })
}

fn normalize_op_arg_key(key: &str) -> String {
    json_arg_key_to_flag(key)
        .trim_start_matches('-')
        .to_string()
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
