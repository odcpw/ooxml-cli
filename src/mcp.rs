use serde_json::{Value, json};
use std::io::{BufRead, Write};

use crate::{
    CliError, CliResult, EXIT_SUCCESS, EXIT_UNEXPECTED, ServeState, json_string,
    mcp_capabilities_resource, mcp_command_resource_for_uri, mcp_command_resource_template,
    mcp_resources, mcp_tool_success, mcp_tools,
};

pub(crate) fn run_mcp_stdio() -> i32 {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut state = McpState::default();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "mcp read error: {err}");
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
        if let Some(response) = state.handle_rpc(request) {
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
    }
    EXIT_SUCCESS
}

#[derive(Default)]
struct McpState {
    engine: ServeState,
}

impl McpState {
    fn handle_rpc(&mut self, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if id.is_none() || method.starts_with("notifications/") {
            return None;
        }
        let id = id.unwrap_or(Value::Null);
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        Some(match self.handle_method(method, &params) {
            Ok(result) => json!({"id": id, "jsonrpc": "2.0", "result": result}),
            Err(err) => json!({
                "id": id,
                "jsonrpc": "2.0",
                "error": {
                    "code": mcp_json_rpc_error_code(&err),
                    "message": err.message,
                    "data": {"type": err.code, "exitCode": err.exit_code},
                },
            }),
        })
    }

    fn handle_method(&mut self, method: &str, params: &Value) -> CliResult<Value> {
        match method {
            "initialize" => Ok(json!({
                "capabilities": {"resources": {}, "tools": {}},
                "protocolVersion": "2025-06-18",
                "serverInfo": {"name": "ooxml", "version": env!("CARGO_PKG_VERSION")},
            })),
            "tools/list" => Ok(json!({"tools": mcp_tools()})),
            "tools/call" => self.handle_tools_call(params),
            "resources/list" => Ok(json!({"resources": mcp_resources()})),
            "resources/templates/list" => {
                Ok(json!({"resourceTemplates": [mcp_command_resource_template()]}))
            }
            "resources/read" => self.handle_resource_read(params),
            _ => Err(CliError::invalid_args(format!(
                "unsupported MCP method: {method}"
            ))),
        }
    }

    fn handle_tools_call(&mut self, params: &Value) -> CliResult<Value> {
        let name = json_string(params, "name")?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        match name.as_str() {
            "open" => self.call_open(&arguments),
            "op" => self.call_op(&arguments),
            "inspect" => self.call_engine("inspect", &arguments, Vec::new()),
            "validate" => self.call_engine("validate", &arguments, Vec::new()),
            "plan" => self.call_engine("plan", &arguments, Vec::new()),
            "commit" => self.call_commit(&arguments),
            "abort" => self.call_engine("abort", &arguments, Vec::new()),
            _ => Err(CliError::invalid_args(format!("unknown tool: {name}"))),
        }
    }

    fn call_open(&mut self, arguments: &Value) -> CliResult<Value> {
        let payload = self.engine.handle_method("open", arguments)?;
        let session = payload
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| CliError::unexpected("open returned no sessionId"))?;
        let next_actions = vec![
            format!(
                "call op/inspect/validate with session=\"{session}\" (thread this sessionId through every subsequent call)"
            ),
            "call commit to write the output, or abort to discard the working copy".to_string(),
        ];
        Ok(mcp_tool_success("open", payload, next_actions))
    }

    fn call_op(&mut self, arguments: &Value) -> CliResult<Value> {
        let session = json_string(arguments, "session")?;
        let payload = self.engine.handle_method("op", arguments)?;
        let next_actions = vec![
            format!(
                "call inspect with session=\"{session}\" to confirm the change against the working copy"
            ),
            format!("call validate with session=\"{session}\" before committing"),
            format!("call commit with session=\"{session}\" to write the output"),
        ];
        Ok(mcp_tool_success("op", payload, next_actions))
    }

    fn call_commit(&mut self, arguments: &Value) -> CliResult<Value> {
        let payload = self.engine.handle_method("commit", arguments)?;
        let next_actions = payload
            .get("validateCommand")
            .and_then(Value::as_str)
            .map(|command| vec![format!("verify the output: {command}")])
            .unwrap_or_default();
        Ok(mcp_tool_success("commit", payload, next_actions))
    }

    fn call_engine(
        &mut self,
        method: &str,
        arguments: &Value,
        next_actions: Vec<String>,
    ) -> CliResult<Value> {
        let payload = self.engine.handle_method(method, arguments)?;
        Ok(mcp_tool_success(method, payload, next_actions))
    }

    fn handle_resource_read(&self, params: &Value) -> CliResult<Value> {
        let uri = json_string(params, "uri")?;
        let text = match uri.as_str() {
            "resource://capabilities" => serde_json::to_string(&mcp_capabilities_resource())
                .expect("serialize capabilities resource"),
            "resource://agent-guide" => serde_json::to_string(&json!({
                "tool": "ooxml",
                "guide": "Open a session with tools/call open, apply one op at a time, inspect and validate before commit.",
            }))
            .expect("serialize agent guide"),
            _ if uri.starts_with("resource://command/") => serde_json::to_string(
                &mcp_command_resource_for_uri(&uri)?,
            )
            .expect("serialize command resource"),
            _ => {
                return Err(CliError::file_not_found(format!(
                    "unknown MCP resource: {uri}"
                )));
            }
        };
        Ok(json!({
            "contents": [{
                "mimeType": "application/json",
                "text": text,
                "uri": uri,
            }]
        }))
    }
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

fn mcp_json_rpc_error_code(err: &CliError) -> i64 {
    if err.code == "invalid_args" && err.message.starts_with("unsupported MCP method:") {
        -32601
    } else if matches!(
        err.code,
        "invalid_args" | "file_not_found" | "unsupported_type" | "target_not_found"
    ) {
        -32602
    } else {
        -32603
    }
}
