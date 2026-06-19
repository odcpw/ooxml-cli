use serde_json::{Value, json};

use crate::{CliError, CliResult, capabilities, json_field};

pub(crate) fn mcp_tool_success(tool: &str, payload: Value, next_actions: Vec<String>) -> Value {
    let structured = merge_next_actions(payload.clone(), &next_actions);
    let text = match tool {
        "open" => mcp_open_text(&structured),
        "op" => mcp_op_text(&structured),
        "inspect" => mcp_inspect_text(&structured),
        "plan" => mcp_plan_text(&structured),
        "commit" => mcp_commit_text(&structured),
        _ => serde_json::to_string(&structured).expect("serialize MCP tool payload"),
    };
    json!({
        "content": [{"text": text, "type": "text"}],
        "structuredContent": structured,
    })
}

fn merge_next_actions(mut payload: Value, next_actions: &[String]) -> Value {
    if !next_actions.is_empty()
        && let Value::Object(ref mut object) = payload
    {
        object.insert("next_actions".to_string(), json!(next_actions));
    }
    payload
}

fn mcp_open_text(value: &Value) -> String {
    format!(
        "{{\"next_actions\":{},\"sessionId\":{},\"type\":{}}}",
        json_field(value, "next_actions"),
        json_field(value, "sessionId"),
        json_field(value, "type")
    )
}

fn mcp_op_text(value: &Value) -> String {
    format!(
        "{{\"command\":{},\"index\":{},\"next_actions\":{},\"readback\":{}}}",
        json_field(value, "command"),
        json_field(value, "index"),
        json_field(value, "next_actions"),
        mcp_readback_text_for_op(&value["readback"])
    )
}

fn mcp_inspect_text(value: &Value) -> String {
    if value.get("range").is_none() && value.get("sheet").is_some_and(Value::is_object) {
        return serde_json::to_string(value).expect("serialize MCP inspect payload");
    }
    format!(
        concat!(
            "{{\"file\":{},\"sheet\":{},\"sheetNumber\":{},\"range\":{},",
            "\"primarySelector\":{},\"selectors\":{},\"rows\":{},\"cols\":{},",
            "\"values\":{},\"types\":{},\"formulaCount\":{},\"dataFormat\":{},",
            "\"truncated\":{},\"majorDimension\":{},\"validateCommand\":{},",
            "\"cellsExtractCommand\":{},\"pptxUpdateTableCommandTemplate\":{},",
            "\"pptxPlaceTableCommandTemplate\":{},\"pptxReplaceTextCommandTemplate\":{}}}"
        ),
        json_field(value, "file"),
        json_field(value, "sheet"),
        json_field(value, "sheetNumber"),
        json_field(value, "range"),
        json_field(value, "primarySelector"),
        json_field(value, "selectors"),
        json_field(value, "rows"),
        json_field(value, "cols"),
        json_field(value, "values"),
        json_field(value, "types"),
        json_field(value, "formulaCount"),
        json_field(value, "dataFormat"),
        json_field(value, "truncated"),
        json_field(value, "majorDimension"),
        json_field(value, "validateCommand"),
        json_field(value, "cellsExtractCommand"),
        json_field(value, "pptxUpdateTableCommandTemplate"),
        json_field(value, "pptxPlaceTableCommandTemplate"),
        json_field(value, "pptxReplaceTextCommandTemplate"),
    )
}

fn mcp_plan_text(value: &Value) -> String {
    let plans = value["plan"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    format!(
                        "{{\"index\":{},\"command\":{},\"argv\":{}}}",
                        json_field(item, "index"),
                        json_field(item, "command"),
                        json_field(item, "argv")
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    format!(
        "{{\"schemaVersion\":{},\"file\":{},\"opsCount\":{},\"dryRun\":{},\"plan\":[{}]}}",
        json_field(value, "schemaVersion"),
        json_field(value, "file"),
        json_field(value, "opsCount"),
        json_field(value, "dryRun"),
        plans
    )
}

fn mcp_commit_text(value: &Value) -> String {
    let applied = value["applied"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    format!(
                        "{{\"index\":{},\"command\":{},\"readback\":{}}}",
                        json_field(item, "index"),
                        json_field(item, "command"),
                        json_field(item, "readback")
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    format!(
        concat!(
            "{{\"applied\":[{}],\"dryRun\":{},\"file\":{},\"next_actions\":{},",
            "\"opsCount\":{},\"output\":{},\"schemaVersion\":{},\"validateCommand\":{}}}"
        ),
        applied,
        json_field(value, "dryRun"),
        json_field(value, "file"),
        json_field(value, "next_actions"),
        json_field(value, "opsCount"),
        json_field(value, "output"),
        json_field(value, "schemaVersion"),
        json_field(value, "validateCommand")
    )
}

fn mcp_readback_text_for_op(value: &Value) -> String {
    let destination = &value["destination"];
    format!(
        concat!(
            "{{\"file\":{},\"sheet\":{},\"sheetNumber\":{},\"ref\":{},",
            "\"handle\":{},\"type\":{},\"value\":{},\"previousType\":{},",
            "\"previousValue\":{},\"created\":{},\"output\":{},\"dryRun\":{},",
            "\"destination\":{{\"file\":{},\"sheet\":{},\"sheetNumber\":{},",
            "\"sheetPrimarySelector\":{},\"sheetSelectors\":{},\"range\":{},",
            "\"rows\":{},\"cols\":{},\"values\":{},\"types\":{},\"formulas\":{},",
            "\"formulaCount\":{},\"truncated\":{}}},\"validateCommand\":{},",
            "\"cellsExtractCommand\":{},\"rangesExportCommand\":{}}}"
        ),
        json_field(value, "file"),
        json_field(value, "sheet"),
        json_field(value, "sheetNumber"),
        json_field(value, "ref"),
        json_field(value, "handle"),
        json_field(value, "type"),
        json_field(value, "value"),
        json_field(value, "previousType"),
        json_field(value, "previousValue"),
        json_field(value, "created"),
        json_field(value, "output"),
        json_field(value, "dryRun"),
        json_field(destination, "file"),
        json_field(destination, "sheet"),
        json_field(destination, "sheetNumber"),
        json_field(destination, "sheetPrimarySelector"),
        json_field(destination, "sheetSelectors"),
        json_field(destination, "range"),
        json_field(destination, "rows"),
        json_field(destination, "cols"),
        json_field(destination, "values"),
        json_field(destination, "types"),
        json_field(destination, "formulas"),
        json_field(destination, "formulaCount"),
        json_field(destination, "truncated"),
        json_field(value, "validateCommand"),
        json_field(value, "cellsExtractCommand"),
        json_field(value, "rangesExportCommand")
    )
}

pub(crate) fn mcp_tools() -> Value {
    json!([
        {
            "name": "open",
            "description": "Open a working copy of an OOXML file and start a session.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": {"type": "string"},
                    "out": {"type": "string"},
                    "inPlace": {"type": "boolean"},
                    "backup": {"type": "string"},
                    "noValidate": {"type": "boolean"},
                    "dryRun": {"type": "boolean"}
                },
                "required": ["file"],
                "additionalProperties": false
            }
        },
        {
            "name": "op",
            "description": "Apply one mutation operation to the session working copy.",
            "inputSchema": mcp_command_tool_schema()
        },
        {
            "name": "inspect",
            "description": "Run one read-only command against the session working copy.",
            "inputSchema": mcp_command_tool_schema()
        },
        {
            "name": "validate",
            "description": "Validate the current working copy.",
            "inputSchema": mcp_session_tool_schema()
        },
        {
            "name": "plan",
            "description": "Return the buffered operation plan.",
            "inputSchema": mcp_session_tool_schema()
        },
        {
            "name": "commit",
            "description": "Write the working copy to the output target.",
            "inputSchema": mcp_session_tool_schema()
        },
        {
            "name": "abort",
            "description": "Discard the working copy.",
            "inputSchema": mcp_session_tool_schema()
        }
    ])
}

fn mcp_command_tool_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "session": {"type": "string"},
            "command": {"type": "string"},
            "args": {"type": "object"}
        },
        "required": ["command", "session"],
        "additionalProperties": false
    })
}

fn mcp_session_tool_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "session": {"type": "string"}
        },
        "required": ["session"],
        "additionalProperties": false
    })
}

pub(crate) fn mcp_resources() -> Value {
    json!([
        {
            "uri": "resource://agent-guide",
            "name": "agent-guide",
            "description": "A compact, paste-ready guide for agent workflows across PPTX, XLSX, VBA, and DOCX. Same content as `ooxml agent guide --json`.",
            "mimeType": "application/json"
        },
        {
            "uri": "resource://capabilities",
            "name": "capabilities",
            "description": "The full machine-readable CLI contract: the command inventory, per-command flags, object kinds, exit codes, workflows, and the stable-handle grammar. This is the menu of valid command strings for the generic op/inspect tools.",
            "mimeType": "application/json"
        }
    ])
}

pub(crate) fn mcp_command_resource_template() -> Value {
    json!({
        "uriTemplate": "resource://command/{path}",
        "name": "command",
        "description": "One command's flag schema, examples, common errors, and target object kinds. The path is the URL-encoded op-vocabulary command string (e.g. resource://command/xlsx%20cells%20set). Read the concrete URI to learn the args object to pass to the generic op/inspect tools for that command.",
        "mimeType": "application/json"
    })
}

pub(crate) fn mcp_capabilities_resource() -> Value {
    let mut document = capabilities::capabilities(&[]).expect("capabilities document");
    if let Some(object) = document.as_object_mut() {
        object.insert(
            "resourceTemplates".to_string(),
            json!([mcp_command_resource_template()]),
        );
    }
    document
}

pub(crate) fn mcp_command_resource_for_uri(uri: &str) -> CliResult<Value> {
    let encoded = uri
        .strip_prefix("resource://command/")
        .ok_or_else(|| CliError::invalid_args("resource://command/{path} is required"))?;
    let decoded = percent_decode_path(encoded)?;
    let decoded = decoded.trim();
    if decoded.is_empty() {
        return Err(CliError::invalid_args(
            "resource://command/{path} requires a command path",
        ));
    }
    let normalized = normalize_command_resource_path(decoded);
    capabilities::capability_commands()
        .into_iter()
        .find(|command| command["path"].as_str() == Some(normalized.as_str()))
        .ok_or_else(|| {
            CliError::file_not_found(format!(
                "unknown command: {decoded}; discover valid commands via resource://capabilities"
            ))
        })
}

fn normalize_command_resource_path(path: &str) -> String {
    let words = path.split_whitespace().collect::<Vec<_>>().join(" ");
    if words == "ooxml" || words.starts_with("ooxml ") {
        words
    } else {
        format!("ooxml {words}")
    }
}

fn percent_decode_path(value: &str) -> CliResult<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(CliError::invalid_args(format!(
                    "invalid percent escape in command resource URI: {value}"
                )));
            }
            let hi = hex_value(bytes[i + 1]).ok_or_else(|| {
                CliError::invalid_args(format!(
                    "invalid percent escape in command resource URI: {value}"
                ))
            })?;
            let lo = hex_value(bytes[i + 2]).ok_or_else(|| {
                CliError::invalid_args(format!(
                    "invalid percent escape in command resource URI: {value}"
                ))
            })?;
            decoded.push((hi << 4) | lo);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(decoded).map_err(|err| {
        CliError::invalid_args(format!(
            "command resource URI path is not valid UTF-8: {err}"
        ))
    })
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
