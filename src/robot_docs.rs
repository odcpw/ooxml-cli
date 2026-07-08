use serde_json::{Value, json};

use crate::agent_aliases::{
    CAPABILITY_COMMAND_FAMILY_FILTERS, CAPABILITY_OBJECT_KINDS, capability_filter_alias_strings,
};
use crate::cli_dispatch::{DispatchBody, DispatchOutput};
use crate::{CliError, CliResult, EXIT_SUCCESS, GlobalFlags, has_flag, reject_unknown_flags};

pub(crate) fn robot_docs(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    match args {
        [sub, rest @ ..] if sub == "guide" => guide(flags, rest),
        [] => guide(flags, &[]),
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: robot-docs {}",
            args.join(" ")
        ))),
    }
}

pub(crate) fn agent_alias(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    match args {
        [sub, rest @ ..] if sub == "guide" => guide(flags, rest),
        [sub, rest @ ..] if sub == "triage" => Ok(DispatchOutput {
            body: DispatchBody::Json(crate::agent_triage::agent_triage(rest)?),
            exit_code: EXIT_SUCCESS,
        }),
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: agent {}",
            args.join(" ")
        ))),
    }
}

fn guide(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    reject_unknown_flags(args, &["--format"], &["--json"])?;
    let value = guide_json();
    if wants_json(flags, args) {
        Ok(DispatchOutput {
            body: DispatchBody::Json(value),
            exit_code: EXIT_SUCCESS,
        })
    } else {
        Ok(DispatchOutput {
            body: DispatchBody::Text(guide_text(&value)),
            exit_code: EXIT_SUCCESS,
        })
    }
}

fn wants_json(flags: &GlobalFlags, args: &[String]) -> bool {
    flags.json
        || has_flag(args, "--json")
        || args
            .windows(2)
            .any(|pair| (pair[0] == "--format" || pair[0] == "-f") && pair[1] == "json")
        || args
            .iter()
            .any(|arg| arg == "--format=json" || arg == "-f=json")
}

fn guide_json() -> Value {
    json!({
        "tool": "ooxml",
        "version": env!("CARGO_PKG_VERSION"),
        "principles": [
            "Use --json for machine-readable output whenever a command supports it.",
            "Inspect before mutating and keep readback commands from mutation output.",
            "Prefer stable selectors, handles, hashes, and generated command fields over positional guesses.",
            "Reuse generated command fields for validation and readback.",
            "Use --in-place only when the user explicitly asks to overwrite the input."
        ],
        "sections": [
            {
                "name": "Discovery",
                "commands": [
                    "ooxml --json capabilities",
                    "ooxml --json capabilities --for slides",
                    "ooxml --json capabilities --for conditional-formats",
                    "ooxml --json capabilities --for modules",
                    "ooxml agent-triage",
                    "ooxml robot-docs guide",
                    "ooxml --json inspect <file>",
                    "ooxml --json find <query> <file>",
                    "ooxml --json doctor health"
                ],
                "filters": CAPABILITY_COMMAND_FAMILY_FILTERS
                    .iter()
                    .copied()
                    .chain(CAPABILITY_OBJECT_KINDS.iter().copied())
                    .collect::<Vec<_>>(),
                "filterAliases": capability_filter_alias_strings()
            },
            {
                "name": "Preflight and release proof",
                "commands": [
                    "ooxml validate --strict <file>",
                    "ooxml --json doctor",
                    "ooxml --json doctor capabilities",
                    "ooxml --json conformance coverage",
                    "ooxml --json conformance check <file>",
                    "ooxml --json repair normalize <file> --out <normalized-file>"
                ]
            },
            {
                "name": "PPTX read",
                "commands": [
                    "ooxml --json pptx slides list <file>",
                    "ooxml --json pptx shapes show <file> --slide <n>",
                    "ooxml --json pptx tables show <file> --slide <n>",
                    "ooxml --json pptx charts list <file>",
                    "ooxml --json pptx extract text <file>",
                    "ooxml --json pptx extract notes <file>"
                ]
            },
            {
                "name": "PPTX mutate",
                "commands": [
                    "ooxml --json pptx replace text-occurrences <file> --match-text <old> --new-text <new> --out <file>",
                    "ooxml --json pptx tables set-cell <file> --slide <n> --target <selector> --row <n> --col <n> --text <text> --out <file>",
                    "ooxml --json pptx notes set <file> --slide <n> --text <text> --out <file>",
                    "ooxml --json pptx shapes set-bounds <file> --slide <n> --target <selector> --bounds <json> --out <file>"
                ]
            },
            {
                "name": "XLSX read",
                "commands": [
                    "ooxml --json xlsx sheets list <file>",
                    "ooxml --json xlsx cells extract <file>",
                    "ooxml --json xlsx ranges export <file> --sheet <sheet> --range <range>",
                    "ooxml --json xlsx names list <file>",
                    "ooxml --json xlsx tables list <file>",
                    "ooxml --json xlsx charts list <file>"
                ]
            },
            {
                "name": "XLSX mutate",
                "commands": [
                    "ooxml --json xlsx cells set <file> --sheet <sheet> --cell <cell> --value <value> --out <file>",
                    "ooxml --json xlsx ranges set <file> --sheet <sheet> --range <range> --values <json> --out <file>",
                    "ooxml --json xlsx names add <file> --name <name> --ref <ref> --out <file>",
                    "ooxml --json xlsx tables append-rows <file> --table <table> --values <json> --out <file>"
                ]
            },
            {
                "name": "DOCX read",
                "commands": [
                    "ooxml --json docx text <file>",
                    "ooxml --json docx blocks show <file>",
                    "ooxml --json docx tables show <file>",
                    "ooxml --json docx comments list <file>",
                    "ooxml --json docx images list <file>"
                ]
            },
            {
                "name": "DOCX mutate",
                "commands": [
                    "ooxml --json docx paragraphs append <file> --text <text> --out <file>",
                    "ooxml --json docx paragraphs set <file> --index <n> --text <text> --out <file>",
                    "ooxml --json docx tables create <file> --values <json-matrix> --out <file>",
                    "ooxml --json docx tables set-cell <file> --table <n> --row <n> --col <n> --expect-hash <hash> --text <text> --out <file>",
                    "ooxml --json docx comments add <file> --anchor-block <n> --author <name> --text <text> --out <file>"
                ]
            },
            {
                "name": "VBA package operations",
                "commands": [
                    "ooxml --json vba inspect <file>",
                    "ooxml --json vba build-bin --family xlsx|pptx|docx --source <module.bas> --out <vbaProject.bin>",
                    "ooxml --json vba create <file.xlsx|file.pptx|file.docx> --pure --source <module.bas> --out <file.xlsm|file.pptm|file.docm>",
                    "ooxml --json vba list <file>",
                    "ooxml --json vba extract <file> --out-dir <dir>",
                    "ooxml --json vba attach <file> --bin <vbaProject.bin> --out <file>",
                    "ooxml --json vba remove <file> --out <file>",
                    "ooxml --json vba run-smoke [file.xlsm] --smoke-mode Standard|Class --out-dir <proof-dir>",
                    "ooxml --json convert xlsm-to-xlsx <file.xlsm> --out <file.xlsx>"
                ]
            }
        ],
        "warnings": [
            "Do not run mutation commands without an explicit --out, --dry-run, or user-approved --in-place path.",
            "Do not assume legacy-only commands exist in the Rust CLI; use capabilities before invoking a command family.",
            "Use vba office-check for macro package open proof; on Windows it prefers Microsoft Office COM, but it does not execute macros.",
            "Use vba run-smoke only when the user explicitly asks for macro execution proof; it runs a harmless Excel VBA macro through desktop Office COM."
        ]
    })
}

fn guide_text(value: &Value) -> String {
    let mut out = String::from("OOXML robot guide\n\nPrinciples:\n");
    if let Some(principles) = value["principles"].as_array() {
        for item in principles {
            out.push_str("- ");
            out.push_str(item.as_str().unwrap_or_default());
            out.push('\n');
        }
    }
    if let Some(sections) = value["sections"].as_array() {
        for section in sections {
            out.push('\n');
            out.push_str(section["name"].as_str().unwrap_or_default());
            out.push_str(":\n");
            if let Some(commands) = section["commands"].as_array() {
                for command in commands {
                    out.push_str("- ");
                    out.push_str(command.as_str().unwrap_or_default());
                    out.push('\n');
                }
            }
            if let Some(filters) = section["filters"].as_array()
                && !filters.is_empty()
            {
                out.push_str("  Filters: ");
                out.push_str(
                    &filters
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                out.push('\n');
            }
            if let Some(aliases) = section["filterAliases"].as_array()
                && !aliases.is_empty()
            {
                out.push_str("  Filter aliases: ");
                out.push_str(
                    &aliases
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                out.push('\n');
            }
        }
    }
    if let Some(warnings) = value["warnings"].as_array() {
        out.push_str("\nWarnings:\n");
        for warning in warnings {
            out.push_str("- ");
            out.push_str(warning.as_str().unwrap_or_default());
            out.push('\n');
        }
    }
    out
}
