use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::agent_aliases::{
    CAPABILITY_COMMAND_FAMILY_FILTERS, CAPABILITY_OBJECT_KINDS, capability_filter_alias_strings,
};
use crate::capabilities::capability_commands;
use crate::cli_args::{parse_string_flag, positional_args};
use crate::{CliError, CliResult, reject_unknown_flags};

pub(crate) fn agent_triage(args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(args, &["--format", "--file", "--request"], &["--json"])?;
    if let Some(format) = parse_string_flag(args, "--format")?
        && format != "json"
    {
        return Err(CliError::invalid_args(format!(
            "invalid format: {format} (expected 'json')"
        )));
    }
    let positionals = positional_args(args, &["--format", "--file", "--request"], &["--json"])?;
    if !positionals.is_empty() {
        return Err(CliError::invalid_args(
            "agent-triage does not accept positional arguments; use --file or --request",
        ));
    }
    let file = parse_string_flag(args, "--file")?;
    let request = parse_string_flag(args, "--request")?;
    let detected_family = file.as_deref().and_then(detect_family);
    let recipes = crate::recipes::recipes_for(detected_family, request.as_deref());

    let commands = capability_commands();
    let op_compatible = commands
        .iter()
        .filter(|command| {
            command
                .get("opCompatible")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    let health = crate::doctor::doctor_health_snapshot(None);
    let mut document = json!({
        "tool": "ooxml",
        "version": env!("CARGO_PKG_VERSION"),
        "contractVersion": "ooxml-cli.agent-triage.v1",
        "readOnly": true,
        "quickRef": {
            "summary": "OOXML CLI triage for agents: discover capability filters, inspect before mutating, and validate generated outputs.",
            "topCommands": [
                "ooxml agent-triage",
                "ooxml --json capabilities",
                "ooxml --json capabilities --for <filter>",
                "ooxml --json doctor health",
                "ooxml robot-docs guide",
                "ooxml robot-docs recipes",
                "ooxml validate --strict <file>"
            ]
        },
        "triageInput": {
            "file": file,
            "request": request,
            "detectedFamily": detected_family,
        },
        "recipes": recipes.iter().map(|recipe| json!({
            "name": recipe.name,
            "summary": recipe.summary,
            "families": recipe.families,
            "command": format!("ooxml --json robot-docs recipe {}", recipe.name),
        })).collect::<Vec<_>>(),
        "capabilitySummary": {
            "commands": commands.len(),
            "opCompatibleCommands": op_compatible,
            "readOnlyOrDirectCliCommands": commands.len().saturating_sub(op_compatible),
            "packageTypes": ["pptx", "xlsx", "docx"]
        },
        "discovery": {
            "filters": CAPABILITY_COMMAND_FAMILY_FILTERS
                .iter()
                .copied()
                .chain(CAPABILITY_OBJECT_KINDS.iter().copied())
                .collect::<Vec<_>>(),
            "filterAliases": capability_filter_alias_strings(),
            "examples": [
                "ooxml --json capabilities --for slides",
                "ooxml --json capabilities --for conditional-formats",
                "ooxml --json capabilities --for data-validations",
                "ooxml --json capabilities --for charts",
                "ooxml --json capabilities --for modules"
            ]
        },
        "health": health,
        "recommendations": [
            {
                "id": "discover-surface",
                "title": "Discover the implemented Rust surface before invoking a command family.",
                "command": "ooxml --json capabilities --for <filter>",
                "destructive": false
            },
            {
                "id": "inspect-first",
                "title": "Inspect a package and use stable selectors from readback before mutating.",
                "command": "ooxml --json inspect <file>",
                "destructive": false
            },
            {
                "id": "dry-run-compose",
                "title": "Use --dry-run when composing non-trivial XLSX or PPTX mutations.",
                "command": "ooxml --json <mutation-command> <file> ... --dry-run",
                "destructive": false
            },
            {
                "id": "validate-output",
                "title": "Validate the exact generated package before handing it to a user.",
                "command": "ooxml validate --strict <output-file>",
                "destructive": false
            }
        ],
        "commands": [
            {
                "action": "capabilities-all",
                "command": "ooxml --json capabilities",
                "destructive": false
            },
            {
                "action": "capabilities-filtered",
                "command": "ooxml --json capabilities --for <filter>",
                "destructive": false,
                "requiresInput": ["filter"]
            },
            {
                "action": "doctor-health",
                "command": "ooxml --json doctor health",
                "destructive": false
            },
            {
                "action": "agent-guide",
                "command": "ooxml robot-docs guide",
                "destructive": false
            },
            {
                "action": "validate",
                "command": "ooxml validate --strict <file>",
                "destructive": false,
                "requiresInput": ["file"]
            }
        ],
        "warnings": [
            {
                "code": "explicit-output-required",
                "message": "Mutation commands should use --out, --dry-run, or an explicitly approved --in-place path."
            },
            {
                "code": "rust-surface-only",
                "message": "The Rust CLI exposes only implemented paths; use capabilities rather than assuming legacy-only commands."
            },
            {
                "code": "health-is-advisory",
                "message": "Doctor findings describe local environment proof gaps; agent-triage itself remains read-only and exits successfully."
            }
        ]
    });
    document["dataHash"] = json!(data_hash(&document)?);
    Ok(document)
}

fn detect_family(file: &str) -> Option<&'static str> {
    match std::path::Path::new(file)
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase()
        .as_str()
    {
        "pptx" | "pptm" => Some("pptx"),
        "xlsx" => Some("xlsx"),
        "xlsm" => Some("xlsm"),
        "docx" | "docm" => Some("docx"),
        _ => None,
    }
}

fn data_hash(value: &Value) -> CliResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|err| {
        CliError::unexpected(format!("failed to serialize triage payload: {err}"))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}
