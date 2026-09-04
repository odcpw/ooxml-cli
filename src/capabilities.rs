use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::agent_aliases::{
    CAPABILITY_OBJECT_KINDS, capability_filter_aliases_json, capability_filter_suggestions,
    command_alias_registry_json, flag_alias_registry_json, is_command_family_filter,
    normalize_capability_filter,
};
use crate::{
    CliError, CliResult, EXIT_FILE_NOT_FOUND, EXIT_INVALID_ARGS, EXIT_RENDER_FAILED, EXIT_SUCCESS,
    EXIT_TARGET_NOT_FOUND, EXIT_UNEXPECTED, EXIT_UNSUPPORTED_TYPE, has_flag, parse_string_flag,
};

pub(crate) fn capability_commands() -> Vec<Value> {
    crate::command_manifest::capability_commands()
}

pub(crate) fn capabilities(args: &[String]) -> CliResult<Value> {
    reject_capabilities_unknown_flags(args)?;
    let requested_filter = parse_string_flag(args, "--for")?;
    let workflows_only = has_flag(args, "--workflows");
    if workflows_only && requested_filter.is_some() {
        return Err(CliError::invalid_args(
            "--workflows cannot be combined with --for; inspect the recipe catalog first, then filter capabilities separately",
        ));
    }
    if workflows_only {
        return Ok(json!({
            "tool": "ooxml",
            "version": env!("CARGO_PKG_VERSION"),
            "contractVersion": crate::recipes::RECIPE_CONTRACT_VERSION,
            "workflows": crate::recipes::recipes_json(),
        }));
    }
    let normalized_filter = requested_filter.as_deref().map(normalize_capability_filter);
    let mut commands = capability_commands();
    if let Some(filter) = normalized_filter.as_deref() {
        commands.retain(|command| capability_matches_filter(command, filter));
    }
    let filter_info = requested_filter.as_ref().map(|requested| {
        let normalized = normalized_filter.as_deref().unwrap_or_default();
        let mut info = json!({
            "requested": requested,
            "normalized": normalized,
            "matchedCommands": commands.len()
        });
        if commands.is_empty() {
            info["suggestions"] = json!(capability_filter_suggestions(normalized));
        }
        info
    });
    let mut notes = vec![
        "Rust implementation surface: commands listed here are implemented in the current ooxml CLI."
            .to_string(),
        "The deprecated legacy implementation is historical reference material only; current proof is Rust-native."
            .to_string(),
    ];
    if let (Some(requested), Some(normalized)) =
        (requested_filter.as_deref(), normalized_filter.as_deref())
    {
        if requested == normalized {
            notes.insert(
                0,
                format!("Filtered by Rust-supported command/object filter \"{normalized}\"."),
            );
        } else {
            notes.insert(
                0,
                format!(
                    "Filtered by Rust-supported command/object filter \"{requested}\" (normalized to \"{normalized}\")."
                ),
            );
        }
        if commands.is_empty() {
            notes.insert(
                1,
                "No commands matched this filter; inspect `filter.suggestions`, `filterAliases`, and `objectKinds` for accepted filters.".to_string(),
            );
        }
    }
    let object_kinds_index = build_object_kinds_index(&commands);
    let mut document = json!({
        "tool": "ooxml",
        "version": env!("CARGO_PKG_VERSION"),
        "contractVersion": "ooxml-cli.agent-capabilities.v4",
        "packageTypes": ["pptx", "xlsx", "docx"],
        "outputModes": [
            "json via --json or --format json",
            "text for text utility commands via --format text",
            "markdown for docx text, pptx extract text, xlsx ranges export, and outline via --format markdown"
        ],
        "globalFlags": [
            {"name": "--format", "argName": "format", "shorthand": "f", "type": "string", "default": "json", "description": "output format: \"json\" by default; \"text\" is accepted only for text utility commands; \"markdown\" is supported by docx text, pptx extract text, xlsx ranges export, and outline"},
            {"name": "--json", "argName": "json", "type": "bool", "default": "false", "description": "emit JSON output"},
            {"name": "--strict", "argName": "strict", "type": "bool", "default": "false", "description": "enable strict validation mode"}
        ],
        "commands": commands,
        "mcp": {
            "transport": "stdio JSON-RPC 2.0",
            "genericTools": ["open", "op", "inspect", "validate", "plan", "commit", "abort"],
            "typedTools": crate::mcp::typed_tool_names(),
            "resources": [
                "resource://capabilities",
                "resource://agent-guide",
                "resource://schema/pptx-build",
                "resource://schema/xlsx-build",
                "resource://schema/docx-build"
            ],
            "contract": "call tools/list for exact schemas; typed build, edit, outline, check, validate, render, find, and replace intents preserve the corresponding CLI envelopes"
        },
        "objectKinds": CAPABILITY_OBJECT_KINDS,
        "filterAliases": capability_filter_aliases_json(),
        "commandAliases": command_alias_registry_json(),
        "flagAliases": flag_alias_registry_json(),
        "objectKindsIndex": object_kinds_index,
        "exitCodes": [
            {"code": EXIT_SUCCESS, "name": "success", "description": "command completed successfully"},
            {"code": EXIT_UNEXPECTED, "name": "unexpected", "description": "unexpected tool or package processing error"},
            {"code": EXIT_INVALID_ARGS, "name": "invalid_args", "description": "invalid command line arguments or incompatible options"},
            {"code": EXIT_FILE_NOT_FOUND, "name": "file_not_found", "description": "input file was not found"},
            {"code": EXIT_UNSUPPORTED_TYPE, "name": "unsupported_type", "description": "input package type is unsupported for the requested command"},
            {"code": EXIT_TARGET_NOT_FOUND, "name": "target_not_found", "description": "requested slide, sheet, table, shape, or macro part was not found"},
            {"code": EXIT_RENDER_FAILED, "name": "render_failed", "description": "rendering or local Office-compatible open check failed"}
        ],
        "errorEnvelope": {
            "appliesTo": "unknown flags, near-miss command tokens, and missing required flags; other errors keep the base code/exitCode/message shape",
            "code": "stable machine-readable error category",
            "exitCode": "documented numeric exit code",
            "message": "what failed",
            "hint": "specific recovery guidance",
            "didYouMean": ["ranked replacement flags or command paths; omitted when empty"],
            "validFlags": [{"flag": "--flag", "use": "--flag <value>"}],
            "helpCommand": "copy-pasteable focused help command",
            "correctedCommand": "copy-pasteable corrected invocation; omitted when correction needs a user choice",
            "channels": {
                "explicitJson": "one JSON object on stdout; diagnostics remain empty",
                "defaultJson": "one JSON object on stderr for backward compatibility",
                "explicitText": "fixed-layout diagnostics on stderr"
            }
        },
        "workflows": crate::recipes::recipes_json(),
        "conventions": [
            "stdout is data; explicit --json invalid-argument errors are structured result data on stdout, while text diagnostics go to stderr",
            "serve/MCP operation commands use op vocabulary without the leading ooxml",
            "mutations should be validated before handing files to users",
            "package outputs are byte-deterministic for identical inputs; SOURCE_DATE_EPOCH sets created and modified core-property timestamps, and timestamps are omitted when it is unset",
            "text styling is suppressed when NO_COLOR or CI is set, TERM=dumb, or stdout is not a TTY"
        ],
        "notes": notes,
    });
    if let Some(filter_info) = filter_info {
        document["filter"] = filter_info;
    }
    Ok(document)
}

fn build_object_kinds_index(commands: &[Value]) -> Value {
    let mut index = CAPABILITY_OBJECT_KINDS
        .iter()
        .map(|kind| ((*kind).to_string(), BTreeSet::new()))
        .collect::<BTreeMap<String, BTreeSet<String>>>();

    for command in commands {
        let Some(path) = command.get("path").and_then(Value::as_str) else {
            continue;
        };
        let Some(kinds) = command.get("targetObjectKinds").and_then(Value::as_array) else {
            continue;
        };
        for kind in kinds.iter().filter_map(Value::as_str) {
            index
                .entry(kind.to_string())
                .or_default()
                .insert(path.to_string());
        }
    }

    Value::Object(
        index
            .into_iter()
            .map(|(kind, paths)| {
                (
                    kind,
                    Value::Array(paths.into_iter().map(Value::String).collect()),
                )
            })
            .collect(),
    )
}

fn capability_matches_filter(command: &Value, filter: &str) -> bool {
    let path = command
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if is_command_family_filter(filter)
        && (path == format!("ooxml {filter}") || path.starts_with(&format!("ooxml {filter} ")))
    {
        return true;
    }
    if is_path_segment_filter(filter)
        && path
            .split_whitespace()
            .skip(1)
            .any(|segment| segment == filter)
    {
        return true;
    }
    command
        .get("targetObjectKinds")
        .and_then(Value::as_array)
        .map(|kinds| kinds.iter().any(|kind| kind.as_str() == Some(filter)))
        .unwrap_or(false)
}

fn is_path_segment_filter(filter: &str) -> bool {
    matches!(filter, "template")
}

fn reject_capabilities_unknown_flags(args: &[String]) -> CliResult<()> {
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !arg.starts_with('-') {
            i += 1;
            continue;
        }
        let flag = arg.split_once('=').map(|(flag, _)| flag).unwrap_or(arg);
        match flag {
            "--json" | "--strict" | "--workflows" => i += 1,
            "--for" => {
                if arg.contains('=') {
                    i += 1;
                } else if args.get(i + 1).is_some() {
                    i += 2;
                } else {
                    return Err(CliError::invalid_args("--for requires a value"));
                }
            }
            "--format" | "-f" => {
                let value = if let Some((_, value)) = arg.split_once('=') {
                    Some(value)
                } else {
                    args.get(i + 1).map(String::as_str)
                };
                match value {
                    Some("json") => {
                        i += if arg.contains('=') { 1 } else { 2 };
                    }
                    Some(value) => {
                        return Err(CliError::invalid_args(format!(
                            "invalid format: {value} (expected 'json')"
                        )));
                    }
                    None => return Err(CliError::invalid_args("--format requires a value")),
                }
            }
            _ => {
                let hint = if matches!(flag, "--fr" | "--fro" | "--filter") {
                    "; did you mean --for? Try: ooxml --json capabilities --for <filter>"
                } else {
                    "; valid flags are --for <filter>, --workflows, --json, --strict, and --format json"
                };
                return Err(CliError::invalid_args(format!(
                    "unknown flag: {flag}{hint}"
                )));
            }
        }
    }
    Ok(())
}
