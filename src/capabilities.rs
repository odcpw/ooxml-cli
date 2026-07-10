mod commands;
pub(crate) use commands::capability_commands;

use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::agent_aliases::{
    CAPABILITY_OBJECT_KINDS, capability_filter_aliases_json, capability_filter_suggestions,
    is_command_family_filter, normalize_capability_filter,
};
use crate::{
    CliError, CliResult, EXIT_FILE_NOT_FOUND, EXIT_INVALID_ARGS, EXIT_RENDER_FAILED, EXIT_SUCCESS,
    EXIT_TARGET_NOT_FOUND, EXIT_UNEXPECTED, EXIT_UNSUPPORTED_TYPE, parse_string_flag,
};

pub(crate) fn capabilities(args: &[String]) -> CliResult<Value> {
    reject_capabilities_unknown_flags(args)?;
    let requested_filter = parse_string_flag(args, "--for")?;
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
        "outputModes": ["json via --json or --format json"],
        "globalFlags": [
            {"name": "--format", "argName": "format", "shorthand": "f", "type": "string", "default": "json", "description": "output format: \"json\"; \"text\" is accepted only for text utility commands"},
            {"name": "--json", "argName": "json", "type": "bool", "default": "false", "description": "emit JSON output"},
            {"name": "--strict", "argName": "strict", "type": "bool", "default": "false", "description": "enable strict validation mode"}
        ],
        "commands": commands,
        "objectKinds": CAPABILITY_OBJECT_KINDS,
        "filterAliases": capability_filter_aliases_json(),
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
        "workflows": [
            {
                "name": "pptx inspect then edit",
                "commands": [
                    "ooxml --json inspect deck.pptx",
                    "ooxml --json pptx slides list deck.pptx",
                    "ooxml --json pptx slides selectors deck.pptx --slide 1",
                    "ooxml --json pptx slides show deck.pptx --slide 1 --include-text",
                    "ooxml --json pptx shapes show deck.pptx --slide 1 --include-text --include-bounds",
                    "ooxml --json pptx shapes get deck.pptx --slide 1 --target title --include-text --include-bounds",
                    "ooxml --json pptx add-textbox deck.pptx --slide 1 --text 'New callout' --x 914400 --y 914400 --cx 3000000 --cy 600000 --out edited.pptx",
                    "ooxml --json pptx place image deck.pptx --slide 1 --image logo.png --x 914400 --y 1700000 --cx 1200000 --cy 600000 --out edited.pptx",
                    "ooxml --json pptx shapes set-bounds deck.pptx --slide 1 --target title --bounds 914400,914400,6000000,1000000 --out edited.pptx",
                    "ooxml --json pptx replace text deck.pptx --slide 1 --target title --text NEW --out edited.pptx",
                    "ooxml validate --strict edited.pptx"
                ]
            },
            {
                "name": "xlsx inspect then edit",
                "commands": [
                    "ooxml --json xlsx sheets list workbook.xlsx",
                    "ooxml --json xlsx ranges export workbook.xlsx --sheet sheetId:1 --range A1 --include-types",
                    "ooxml --json xlsx ranges set workbook.xlsx --sheet sheetId:1 --range A1:B2 --values '[[\"A\",\"B\"],[1,2]]' --out edited.xlsx",
                    "ooxml --json xlsx colwidths set workbook.xlsx --sheet sheetId:1 --range B:D --width 18 --out edited.xlsx",
                    "ooxml --json xlsx rowheights set workbook.xlsx --sheet sheetId:1 --range 2:5 --height 24 --out edited.xlsx",
                    "ooxml --json xlsx ranges set-format workbook.xlsx --sheet Sheet1 --range B2:B20 --preset currency --out edited.xlsx",
                    "ooxml --json apply workbook.xlsx --ops ops.json --out edited.xlsx",
                    "serve op commands: pptx tables set-cell/delete-row/insert-row/delete-col/insert-col/update-from-xlsx, xlsx cells set, xlsx ranges set, xlsx ranges set-format, xlsx comments add, xlsx comments update, xlsx comments remove, xlsx tables append-rows, xlsx tables append-records, xlsx workbook metadata update"
                ]
            }
        ],
        "conventions": [
            "stdout is data; diagnostics and errors go to stderr",
            "serve/MCP operation commands use op vocabulary without the leading ooxml",
            "mutations should be validated before handing files to users"
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
            "--json" | "--strict" => i += 1,
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
                    "; valid flags are --for <filter>, --json, --strict, and --format json"
                };
                return Err(CliError::invalid_args(format!(
                    "unknown flag: {flag}{hint}"
                )));
            }
        }
    }
    Ok(())
}
