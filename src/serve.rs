use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;

use crate::mutation_envelope::attach_cli_mutation_envelope;
use crate::{
    CliError, CliResult, EXIT_SUCCESS, EXIT_UNEXPECTED, json_bool, json_optional_string,
    json_string, package_mutation_temp_path, package_type, validate, validate_exit_code,
};
mod inspect;
mod inspect_namespace;
mod inspect_table;
mod op;
mod op_dispatch;
mod op_namespace;
use inspect::serve_inspect_command;
use op::ServeOp;
use op_dispatch::serve_op_command;

pub(crate) fn run_serve_stdio() -> i32 {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut state = ServeState::default();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "serve read error: {err}");
                return EXIT_UNEXPECTED;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                let response = json_rpc_parse_error(err.to_string());
                if writeln!(
                    stdout,
                    "{}",
                    serde_json::to_string(&response).expect("serialize parse error")
                )
                .is_err()
                {
                    return EXIT_UNEXPECTED;
                }
                if stdout.flush().is_err() {
                    return EXIT_UNEXPECTED;
                }
                continue;
            }
        };
        let response = state.handle_rpc(request);
        if writeln!(
            stdout,
            "{}",
            serde_json::to_string(&response).expect("serialize rpc response")
        )
        .is_err()
        {
            return EXIT_UNEXPECTED;
        }
        if stdout.flush().is_err() {
            return EXIT_UNEXPECTED;
        }
    }
    EXIT_SUCCESS
}

#[derive(Default)]
pub(crate) struct ServeState {
    next_session: usize,
    sessions: BTreeMap<String, ServeSession>,
}

struct ServeSession {
    file: String,
    out: Option<String>,
    in_place: bool,
    backup: Option<String>,
    no_validate: bool,
    dry_run: bool,
    ops_base_dir: Option<String>,
    working: String,
    ops: Vec<ServeOp>,
    op_ids: Vec<Option<String>>,
    op_results: Vec<Value>,
    resolved_args: Vec<Value>,
    named_results: BTreeMap<String, Value>,
}

impl ServeState {
    fn handle_rpc(&mut self, request: Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        match self.handle_method(method, &params) {
            Ok(result) => json!({"id": id, "jsonrpc": "2.0", "result": result}),
            Err(err) => json!({
                "id": id,
                "jsonrpc": "2.0",
                "error": {
                    "code": err.exit_code,
                    "message": err.message,
                    "data": {"type": err.code, "exitCode": err.exit_code},
                },
            }),
        }
    }

    pub(crate) fn handle_method(&mut self, method: &str, params: &Value) -> CliResult<Value> {
        match method {
            "open" => self.serve_open(params),
            "op" => self.serve_op(params),
            "inspect" => self.serve_inspect(params),
            "validate" => self.serve_validate(params),
            "plan" => self.serve_plan(params),
            "commit" => self.serve_commit(params),
            "abort" => self.serve_abort(params),
            _ => Err(CliError::invalid_args(format!(
                "unsupported serve method: {method}"
            ))),
        }
    }

    fn serve_open(&mut self, params: &Value) -> CliResult<Value> {
        let file = json_string(params, "file")?;
        let out = json_optional_string(params, "out");
        let in_place = json_bool(params, "inPlace").unwrap_or(false);
        let backup = json_optional_string(params, "backup");
        let no_validate = json_bool(params, "noValidate").unwrap_or(false);
        let dry_run = params
            .get("dryRun")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let ops_base_dir = json_optional_string(params, "opsBaseDir");
        if out.is_some() && in_place {
            return Err(CliError::invalid_args(
                "cannot specify both out and inPlace",
            ));
        }
        if backup
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
            && !in_place
        {
            return Err(CliError::invalid_args(
                "backup can only be used with inPlace",
            ));
        }
        self.next_session += 1;
        let session_id = format!("rust-session-{}", self.next_session);
        let working = make_working_copy(&file, self.next_session)?;
        self.sessions.insert(
            session_id.clone(),
            ServeSession {
                file: file.clone(),
                out,
                in_place,
                backup,
                no_validate,
                dry_run,
                ops_base_dir,
                working,
                ops: Vec::new(),
                op_ids: Vec::new(),
                op_results: Vec::new(),
                resolved_args: Vec::new(),
                named_results: BTreeMap::new(),
            },
        );
        let package_kind = if Path::new(&file).is_file() {
            package_type(&file)?
        } else {
            package_type_from_extension(&file)?
        };
        Ok(json!({"sessionId": session_id, "type": package_kind}))
    }

    fn serve_op(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let command = json_string(params, "command")?;
        let args = params
            .get("args")
            .ok_or_else(|| CliError::invalid_args("op args are required"))?;
        let session = self.session_mut(&session_id)?;
        let op_id = json_optional_string(params, "id");
        if let Some(op_id) = op_id.as_deref() {
            validate_operation_id(op_id)?;
            if session.named_results.contains_key(op_id) {
                return Err(CliError::invalid_args(format!(
                    "duplicate operation id: {op_id:?}"
                )));
            }
        }
        if !Path::new(&session.working).is_file()
            && !(session.ops.is_empty() && is_scaffold_command(&command))
        {
            return Err(CliError::invalid_args(
                "a session opened on a new path must begin with a docx, pptx, or xlsx scaffold op",
            ));
        }
        let mut resolved_args = op_dispatch::resolve_refs(args, &session.named_results)?;
        if let Some(base_dir) = session.ops_base_dir.as_deref() {
            resolved_args =
                op_dispatch::resolve_op_paths(&command, &resolved_args, Path::new(base_dir))?;
        }
        let op = serve_op_command(&session.working, &command, &resolved_args)?;
        let readback = op.readback(&session.working);
        let index = session.ops.len();
        let mutation_envelope =
            serve_mutation_envelope(&op, &session.working, &session.working, &readback, false)?;
        let mut result = json!({
            "command": command,
            "index": index,
            "readback": readback,
            "resolvedArgs": resolved_args,
        });
        if let Some(op_id) = op_id.as_ref()
            && let Value::Object(object) = &mut result
        {
            object.insert("id".to_string(), json!(op_id));
        }
        if let Some(mutation_envelope) = mutation_envelope
            && let Value::Object(object) = &mut result
        {
            object.insert("mutationEnvelope".to_string(), mutation_envelope);
        }
        if let Some(op_id) = op_id.as_ref() {
            session.named_results.insert(op_id.clone(), result.clone());
        }
        session.op_ids.push(op_id);
        session.op_results.push(result.clone());
        session.resolved_args.push(resolved_args);
        session.ops.push(op);
        Ok(result)
    }

    fn serve_inspect(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let command = json_string(params, "command")?;
        let args = params
            .get("args")
            .ok_or_else(|| CliError::invalid_args("inspect args are required"))?;
        let session = self.session(&session_id)?;
        serve_inspect_command(&session.working, &command, args)
    }

    fn serve_validate(&self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let session = self.session(&session_id)?;
        let report = validate(&session.working, true)?;
        Ok(json!({
            "diagnostics": report
                .get("diagnostics")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
        }))
    }

    fn serve_plan(&self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let session = self.session(&session_id)?;
        let plan: Vec<Value> = session
            .ops
            .iter()
            .enumerate()
            .map(|(index, op)| {
                let mut item = json!({
                    "argv": op.plan_argv(&session.file),
                    "command": op.command(),
                    "index": index,
                    "resolvedArgs": session.resolved_args[index],
                });
                if let Some(op_id) = &session.op_ids[index]
                    && let Value::Object(object) = &mut item
                {
                    object.insert("id".to_string(), json!(op_id));
                }
                if let Some(envelope) = session.op_results[index].get("mutationEnvelope")
                    && let Value::Object(object) = &mut item
                {
                    object.insert("mutationEnvelope".to_string(), envelope.clone());
                }
                item
            })
            .collect();
        Ok(json!({
            "dryRun": session.dry_run,
            "file": session.file,
            "opsCount": session.ops.len(),
            "plan": plan,
            "schemaVersion": 1,
        }))
    }

    fn serve_commit(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let session = self.session(&session_id)?;
        let output = if session.in_place {
            session.file.clone()
        } else {
            session
                .out
                .clone()
                .or_else(|| session.dry_run.then(|| session.file.clone()))
                .ok_or_else(|| CliError::invalid_args("commit requires an output path"))?
        };
        if !session.no_validate {
            let validation = validate(&session.working, true)?;
            if validate_exit_code(&validation, true) != EXIT_SUCCESS {
                return Err(CliError::validation_failed(format!(
                    "validation failed for working copy: {}",
                    serde_json::to_string(&validation).expect("serialize validation")
                )));
            }
        }
        if !session.dry_run {
            if session.in_place
                && let Some(backup_path) = session
                    .backup
                    .as_ref()
                    .filter(|value| !value.trim().is_empty())
            {
                if let Some(parent) = Path::new(backup_path).parent() {
                    fs::create_dir_all(parent)
                        .map_err(|err| CliError::unexpected(err.to_string()))?;
                }
                fs::copy(&session.file, backup_path).map_err(|err| {
                    CliError::unexpected(format!("failed to create backup: {err}"))
                })?;
            }
            if let Some(parent) = Path::new(&output).parent() {
                fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
            }
            atomic_copy_to_output(&session.working, &output)?;
        }
        let readback_file = if session.dry_run {
            &session.working
        } else {
            &output
        };
        let applied: Vec<Value> = session
            .ops
            .iter()
            .enumerate()
            .map(|(index, op)| -> CliResult<Value> {
                let readback = op.readback(readback_file);
                let mutation_envelope = serve_mutation_envelope(
                    op,
                    &session.file,
                    readback_file,
                    &readback,
                    !session.no_validate,
                )?;
                let mut applied = json!({
                    "command": op.command(),
                    "index": index,
                    "readback": readback,
                    "resolvedArgs": session.resolved_args[index],
                });
                if let Some(op_id) = &session.op_ids[index]
                    && let Value::Object(object) = &mut applied
                {
                    object.insert("id".to_string(), json!(op_id));
                }
                if let Some(mutation_envelope) = mutation_envelope
                    && let Value::Object(object) = &mut applied
                {
                    object.insert("mutationEnvelope".to_string(), mutation_envelope);
                }
                Ok(applied)
            })
            .collect::<CliResult<_>>()?;
        let mut result = json!({
            "applied": applied,
            "dryRun": session.dry_run,
            "file": session.file,
            "opsCount": session.ops.len(),
            "output": if session.dry_run { Value::Null } else { json!(output.clone()) },
            "schemaVersion": 1,
            "validated": !session.no_validate,
            "validateCommand": if session.dry_run {
                Value::Null
            } else {
                json!(format!("ooxml validate --strict {output}"))
            },
        });
        if session.dry_run
            && let Value::Object(ref mut object) = result
        {
            object.insert("committed".to_string(), json!(false));
            object.insert("plannedOutput".to_string(), json!(output));
        }
        Ok(result)
    }

    fn serve_abort(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        self.sessions
            .remove(&session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))?;
        Ok(json!({"aborted": true}))
    }

    fn session(&self, session_id: &str) -> CliResult<&ServeSession> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))
    }

    fn session_mut(&mut self, session_id: &str) -> CliResult<&mut ServeSession> {
        self.sessions
            .get_mut(session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))
    }
}

fn serve_mutation_envelope(
    op: &ServeOp,
    source_file: &str,
    output_file: &str,
    readback: &Value,
    validated: bool,
) -> CliResult<Option<Value>> {
    let plan_file = if is_scaffold_command(op.command()) {
        output_file
    } else {
        source_file
    };
    let Value::Array(plan) = op.plan_argv(plan_file) else {
        return Err(CliError::unexpected("serve op plan argv must be an array"));
    };
    let mut args = plan
        .into_iter()
        .map(|value| {
            value
                .as_str()
                .map(|text| {
                    if text == "<temp.0>" {
                        output_file.to_string()
                    } else {
                        text.to_string()
                    }
                })
                .ok_or_else(|| CliError::unexpected("serve op argv must contain only strings"))
        })
        .collect::<CliResult<Vec<_>>>()?;
    if validated {
        args.retain(|arg| arg != "--no-validate");
    }
    let mut envelope_source = readback.clone();
    attach_cli_mutation_envelope(&args, Vec::new(), &mut envelope_source)?;
    Ok(envelope_source.get("mutationEnvelope").cloned())
}

fn make_working_copy(file: &str, session_number: usize) -> CliResult<String> {
    let dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-{}-{session_number}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).map_err(|err| CliError::unexpected(err.to_string()))?;
    let extension = Path::new(file)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("xlsx");
    let working = dir.join(format!("working.{extension}"));
    if Path::new(file).exists() {
        if !Path::new(file).is_file() {
            return Err(CliError::invalid_args(format!(
                "session input is not a file: {file}"
            )));
        }
        fs::copy(file, &working).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    Ok(working.to_string_lossy().to_string())
}

fn is_scaffold_command(command: &str) -> bool {
    matches!(command, "docx scaffold" | "pptx scaffold" | "xlsx scaffold")
}

fn package_type_from_extension(file: &str) -> CliResult<&'static str> {
    match Path::new(file)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("docx" | "docm") => Ok("docx"),
        Some("pptx" | "pptm") => Ok("pptx"),
        Some("xlsx" | "xlsm") => Ok("xlsx"),
        _ => Err(CliError::invalid_args(format!(
            "cannot infer OOXML package type from new session path: {file}"
        ))),
    }
}

fn validate_operation_id(id: &str) -> CliResult<()> {
    let valid = !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':'));
    if valid {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "invalid operation id {id:?}; use 1-128 ASCII letters, digits, '-', '_', or ':'"
        )))
    }
}

fn atomic_copy_to_output(source: &str, output: &str) -> CliResult<()> {
    let temp = package_mutation_temp_path(output, "commit");
    fs::copy(source, &temp)
        .map_err(|err| CliError::unexpected(format!("failed to stage commit output: {err}")))?;
    crate::finish_mutation_output(output, &temp, Some(output), false, None, false)
}

fn json_rpc_parse_error(message: String) -> Value {
    json!({
        "id": Value::Null,
        "jsonrpc": "2.0",
        "error": {
            "code": -32700,
            "message": "Parse error",
            "data": {"message": message},
        },
    })
}
