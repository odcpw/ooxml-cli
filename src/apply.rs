use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, ServeState, command_arg, has_flag, parse_string_flag,
    reject_unknown_flags, validate_xlsx_mutation_output_flags,
};

#[derive(Clone)]
struct ApplyOperation {
    id: Option<String>,
    command: String,
    args: Vec<ApplyArg>,
}

#[derive(Clone)]
struct ApplyArg {
    original_key: String,
    normalized_key: String,
    value: Value,
}

pub(crate) fn apply(file: &str, args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(
        args,
        &["--ops", "--out", "--backup"],
        &["--dry-run", "--in-place", "--no-validate"],
    )?;
    let ops_path = parse_string_flag(args, "--ops")?
        .ok_or_else(|| CliError::invalid_args("--ops is required"))?;
    let ops_data = fs::read(&ops_path)
        .map_err(|_| CliError::file_not_found(format!("file not found: {ops_path}")))?;
    let ops = parse_ops(&ops_data)?;
    if fs::metadata(file).is_err()
        && !ops
            .first()
            .is_some_and(|op| is_scaffold_command(&op.command))
    {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    validate_known_operation_commands(&ops)?;
    validate_handle_safety(&ops)?;

    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = has_flag(args, "--dry-run");
    let in_place = has_flag(args, "--in-place");
    let no_validate = has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;

    let ops_base_dir = Path::new(&ops_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(|err| {
            CliError::unexpected(format!(
                "failed to resolve ops directory for {ops_path}: {err}"
            ))
        })?;

    let mut state = ServeState::default();
    let open = state.handle_method(
        "open",
        &json!({
            "file": file,
            "out": out,
            "inPlace": in_place,
            "backup": backup,
            "noValidate": no_validate,
            "dryRun": dry_run,
            "opsBaseDir": ops_base_dir,
        }),
    )?;
    let session = open["sessionId"]
        .as_str()
        .ok_or_else(|| CliError::unexpected("serve open returned no sessionId"))?;
    for (index, op) in ops.iter().enumerate() {
        let operation = state
            .handle_method(
                "op",
                &json!({
                    "session": session,
                    "id": op.id,
                    "command": op.command,
                    "args": args_object(op),
                }),
            )
            .map_err(|err| CliError {
                code: err.code,
                exit_code: err.exit_code,
                message: format!("op {index} ({}) failed: {}", op.command, err.message),
            });
        if let Err(err) = operation {
            let _ = state.handle_method("abort", &json!({"session": session}));
            return Err(err);
        }
    }
    let plan = if dry_run {
        Some(state.handle_method("plan", &json!({"session": session}))?)
    } else {
        None
    };
    let mut result = match state.handle_method("commit", &json!({ "session": session })) {
        Ok(result) => result,
        Err(err) => {
            let _ = state.handle_method("abort", &json!({"session": session}));
            return Err(err);
        }
    };
    if let Some(plan) = plan
        && let Value::Object(object) = &mut result
    {
        object.insert("plan".to_string(), plan["plan"].clone());
    }
    if let Value::Object(ref mut object) = result
        && let Some(output) = object.get("output").and_then(Value::as_str)
    {
        object.insert(
            "validateCommand".to_string(),
            json!(apply_validate_command(output)),
        );
    }
    Ok(result)
}

fn parse_ops(data: &[u8]) -> CliResult<Vec<ApplyOperation>> {
    let raw: Value = serde_json::from_slice(data)
        .map_err(|err| CliError::invalid_args(format!("invalid ops JSON: {err}")))?;
    let items = raw
        .as_array()
        .ok_or_else(|| CliError::invalid_args("invalid ops JSON: expected operations array"))?;
    let mut ops = Vec::with_capacity(items.len());
    let mut seen_ids = BTreeMap::<String, usize>::new();
    for (index, item) in items.iter().enumerate() {
        let object = item
            .as_object()
            .ok_or_else(|| CliError::invalid_args(format!("op {index}: expected object")))?;
        for key in object.keys() {
            if key != "id" && key != "command" && key != "args" {
                return Err(CliError::invalid_args(format!(
                    "invalid ops JSON: op {index}: unknown field {key:?}"
                )));
            }
        }
        let command = object
            .get("command")
            .and_then(Value::as_str)
            .map(normalize_command)
            .unwrap_or_default();
        if command.is_empty() {
            return Err(CliError::invalid_args(format!(
                "op {index}: missing \"command\""
            )));
        }
        validate_command_words(index, &command)?;
        let id = match object.get("id") {
            None | Some(Value::Null) => None,
            Some(Value::String(id)) => {
                validate_operation_id(index, id)?;
                if let Some(previous) = seen_ids.insert(id.clone(), index) {
                    return Err(CliError::invalid_args(format!(
                        "op {index}: duplicate operation id {id:?}; first used by op {previous}"
                    )));
                }
                Some(id.clone())
            }
            Some(_) => {
                return Err(CliError::invalid_args(format!(
                    "op {index}: id must be a string"
                )));
            }
        };
        let args = parse_op_args(index, object.get("args"))?;
        ops.push(ApplyOperation { id, command, args });
    }
    Ok(ops)
}

fn validate_operation_id(index: usize, id: &str) -> CliResult<()> {
    let valid = !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':'));
    if valid {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "op {index}: invalid operation id {id:?}; use 1-128 ASCII letters, digits, '-', '_', or ':'"
        )))
    }
}

fn parse_op_args(index: usize, raw: Option<&Value>) -> CliResult<Vec<ApplyArg>> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    if raw.is_null() {
        return Ok(Vec::new());
    }
    let object = raw
        .as_object()
        .ok_or_else(|| CliError::invalid_args(format!("op {index}: args must be an object")))?;
    let mut seen = BTreeMap::<String, String>::new();
    let mut args = Vec::with_capacity(object.len());
    for (key, value) in object {
        let normalized_key = validate_arg_key_name(index, key)?;
        if is_session_owned_mutation_arg(&normalized_key) {
            return Err(CliError::invalid_args(format!(
                "op {index}: arg {normalized_key:?} is owned by the apply/serve/MCP session; omit it from op args and set it on the outer command or session"
            )));
        }
        if let Some(previous) = seen.insert(normalized_key.clone(), key.clone()) {
            return Err(CliError::invalid_args(format!(
                "op {index}: arg keys {previous:?} and {key:?} both map to flag {normalized_key:?}; pass each flag at most once"
            )));
        }
        args.push(ApplyArg {
            original_key: key.clone(),
            normalized_key,
            value: value.clone(),
        });
    }
    Ok(args)
}

fn validate_command_words(index: usize, command: &str) -> CliResult<()> {
    for word in command.split_whitespace() {
        if word.starts_with('-') {
            return Err(CliError::invalid_args(format!(
                "op {index}: command must contain only command words; put flag {word:?} in args instead"
            )));
        }
    }
    Ok(())
}

fn validate_arg_key_name(index: usize, key: &str) -> CliResult<String> {
    let name = normalize_arg_key_name(key);
    if name.is_empty() {
        return Err(CliError::invalid_args(format!(
            "op {index}: arg key {key:?} must name a flag"
        )));
    }
    if name.contains('=') {
        return Err(CliError::invalid_args(format!(
            "op {index}: arg key {key:?} must be a flag name without '='; put the flag value in the JSON value instead"
        )));
    }
    Ok(name)
}

fn validate_known_operation_commands(ops: &[ApplyOperation]) -> CliResult<()> {
    let commands = crate::capabilities::capability_commands();
    for (index, op) in ops.iter().enumerate() {
        let path = format!("ooxml {}", op.command);
        let Some(command) = commands
            .iter()
            .find(|command| command["path"].as_str() == Some(path.as_str()))
        else {
            return Err(CliError::invalid_args(format!(
                "op {index}: unknown command {:?}; command must be one command path from `ooxml capabilities --json`, with flags and positional values supplied through args",
                op.command
            )));
        };
        if command["opCompatible"].as_bool() != Some(true) {
            let reason = command["opIneligibleReason"]
                .as_str()
                .unwrap_or("it cannot be used as an operation");
            return Err(CliError::invalid_args(format!(
                "op {index}: command {:?} cannot be used as an apply/serve/MCP op: {reason}; use a mutation command whose only positional argument is the package file, and supply every other value through args",
                op.command
            )));
        }
    }
    Ok(())
}

fn validate_handle_safety(ops: &[ApplyOperation]) -> CliResult<()> {
    let mut shifted_at = None::<(usize, String)>;
    for (index, op) in ops.iter().enumerate() {
        if matches!(
            op.command.as_str(),
            "xlsx rows insert" | "xlsx rows delete" | "xlsx cols insert" | "xlsx cols delete"
        ) {
            shifted_at.get_or_insert((index, op.command.clone()));
            continue;
        }
        let Some((shift_index, shift_command)) = shifted_at.as_ref() else {
            continue;
        };
        if let Some(arg) = op.args.iter().find(|arg| is_address_positional(&arg.value)) {
            return Err(CliError::invalid_args(format!(
                "op {index} ({}) targets an address-positional XLSX handle ({}={:?}) after op {shift_index} ({shift_command}) shifted rows/columns earlier in the same batch; the handle's A1 address may move, risking a silent wrong-cell write. Run the structural edit and the handle op as separate apply invocations (re-resolving the handle against the post-edit file), or target the cell positionally with --sheet/--cell.",
                op.command,
                arg.original_key,
                arg_string(&arg.value)
            )));
        }
    }
    Ok(())
}

fn is_address_positional(value: &Value) -> bool {
    let text = arg_string(value);
    text.starts_with("H:xlsx/ws:") && (text.contains("/cell:") || text.contains("/comment:"))
}

fn args_object(op: &ApplyOperation) -> Value {
    let mut object = Map::new();
    for arg in &op.args {
        object.insert(arg.normalized_key.clone(), serve_arg_value(&arg.value));
    }
    Value::Object(object)
}

fn serve_arg_value(value: &Value) -> Value {
    match value {
        Value::Number(number) => Value::String(number.to_string()),
        Value::Null => Value::String(String::new()),
        other => other.clone(),
    }
}

fn arg_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        other => serde_json::to_string(other).expect("serialize apply arg"),
    }
}

fn normalize_command(command: &str) -> String {
    let mut parts = command.split_whitespace().collect::<Vec<_>>();
    if parts
        .first()
        .is_some_and(|part| part.eq_ignore_ascii_case("ooxml"))
    {
        parts.remove(0);
    }
    parts.join(" ")
}

fn normalize_arg_key_name(key: &str) -> String {
    let name = key.trim().trim_start_matches('-').replace(['_', ' '], "-");
    if name.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut prev_dash = false;
    for (index, ch) in name.chars().enumerate() {
        if ch == '-' {
            if !out.is_empty() && !prev_dash {
                out.push('-');
                prev_dash = true;
            }
            continue;
        }
        if ch.is_ascii_uppercase() {
            if index > 0 && !out.is_empty() && !prev_dash {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch.to_ascii_lowercase());
        }
        prev_dash = false;
    }
    out.trim_matches('-').to_string()
}

fn is_session_owned_mutation_arg(normalized: &str) -> bool {
    matches!(
        normalized,
        "out"
            | "in-place"
            | "inplace"
            | "dry-run"
            | "dryrun"
            | "backup"
            | "no-validate"
            | "novalidate"
            | "output"
            | "json"
            | "pretty"
            | "no-color"
            | "nocolor"
            | "keep-temp"
            | "keeptemp"
            | "temp-dir"
            | "tempdir"
            | "verbosity"
            | "strict"
            | "help"
            | "h"
            | "o"
            | "v"
    )
}

pub(crate) fn apply_validate_command(output: &str) -> String {
    format!("ooxml validate --strict {}", command_arg(output))
}

fn is_scaffold_command(command: &str) -> bool {
    matches!(command, "docx scaffold" | "pptx scaffold" | "xlsx scaffold")
}
