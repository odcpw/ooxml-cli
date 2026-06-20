use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::capabilities::capability_commands;
use crate::cli_dispatch::{DispatchBody, DispatchOutput};
use crate::{CliError, CliResult, EXIT_SUCCESS};

const ROOT_SUMMARY: &str = "ooxml is the Rust port of ooxml-cli for proven OOXML automation.";
const ROOT_LONG: &str = "It exposes only the command surface implemented in Rust. Use `ooxml --json capabilities` for the machine-readable inventory.";
const EMPTY_ALIASES: &[&str] = &[];

const GROUP_TOPICS: &[(&[&str], &str, &str, &[&str])] = &[
    (
        &["completion"],
        "Generate shell completion scripts for Rust-supported top-level commands.",
        "Generate shell completion scripts.",
        &[],
    ),
    (
        &["conformance"],
        "Show Rust-supported conformance reports.",
        "Rust currently exposes static conformance coverage. Go's repair-invariant `conformance check` is intentionally unported until Rust can reproduce it without wrapping validation as a placeholder.",
        &[],
    ),
    (
        &["docx"],
        "Work with Rust-supported DOCX document commands.",
        "Commands for inspecting and mutating Word DOCX documents where Rust parity has been proven.",
        &[],
    ),
    (
        &["docx", "comments"],
        "Inspect and mutate DOCX comments.",
        "Commands for listing, adding, editing, and removing document comments.",
        &["comment"],
    ),
    (
        &["docx", "fields"],
        "Inspect and edit DOCX fields.",
        "Commands for listing fields and mutating cached field results.",
        &["field"],
    ),
    (
        &["docx", "headers"],
        "Inspect and edit DOCX headers.",
        "Commands for listing, showing, and setting header text.",
        &["header"],
    ),
    (
        &["docx", "footers"],
        "Inspect and edit DOCX footers.",
        "Commands for listing, showing, and setting footer text.",
        &["footer"],
    ),
    (
        &["docx", "images"],
        "Inspect and mutate DOCX inline images.",
        "Commands for listing, replacing, and inserting DOCX images.",
        &["image"],
    ),
    (
        &["docx", "paragraphs"],
        "Mutate DOCX body paragraphs.",
        "Commands for appending, inserting, setting, and clearing body paragraphs.",
        &["paragraph"],
    ),
    (
        &["docx", "styles"],
        "Inspect and apply DOCX styles.",
        "Commands for style discovery and style application.",
        &["style"],
    ),
    (
        &["docx", "tables"],
        "Inspect and mutate DOCX tables.",
        "Commands for table readback and hash-guarded table mutations.",
        &["table"],
    ),
    (
        &["pptx"],
        "Work with Rust-supported PPTX presentation commands.",
        "Commands for inspecting, modifying, and analyzing PPTX presentations where Rust parity has been proven.",
        &[],
    ),
    (
        &["pptx", "animations"],
        "Inspect and mutate PPTX animations.",
        "Commands for listing and editing per-slide animation timing.",
        &["animation"],
    ),
    (
        &["pptx", "charts"],
        "Inspect and mutate PPTX slide charts.",
        "Commands for chart readback, creation, data updates, styling, and type conversion.",
        &["chart"],
    ),
    (
        &["pptx", "comments"],
        "Inspect and mutate PPTX slide comments.",
        "Commands for legacy slide comments.",
        &["comment"],
    ),
    (
        &["pptx", "extract"],
        "Extract PPTX resources.",
        "Commands for extracting text, notes, images, and XML.",
        &[],
    ),
    (
        &["pptx", "layouts"],
        "Inspect and mutate PPTX slide layouts.",
        "Commands for layout readback and layout-level mutations.",
        &["layout"],
    ),
    (
        &["pptx", "masters"],
        "Inspect and mutate PPTX slide masters.",
        "Commands for master readback and placeholder authoring.",
        &["master"],
    ),
    (
        &["pptx", "media"],
        "Inspect and mutate PPTX media.",
        "Commands for embedded slide audio/video media.",
        &["media"],
    ),
    (
        &["pptx", "notes"],
        "Inspect and mutate PPTX speaker notes.",
        "Commands for showing, setting, and clearing notes.",
        &["note"],
    ),
    (
        &["pptx", "place"],
        "Place content on PPTX slides.",
        "Commands for placing images and tables.",
        &[],
    ),
    (
        &["pptx", "replace"],
        "Replace PPTX content.",
        "Commands for text and image replacement.",
        &[],
    ),
    (
        &["pptx", "shapes"],
        "Inspect and mutate PPTX shapes.",
        "Commands for shape readback, bounds edits, and deletion.",
        &["shape"],
    ),
    (
        &["pptx", "slides"],
        "Inspect and mutate PPTX slides.",
        "Commands for slide readback, selectors, and slide lifecycle mutations.",
        &["slide"],
    ),
    (
        &["pptx", "template"],
        "Inspect PPTX template manifests.",
        "Only template inspect is implemented in Rust today.",
        &[],
    ),
    (
        &["pptx", "text"],
        "Set PPTX text run styling.",
        "Commands for run-level text styling on slide shapes.",
        &[],
    ),
    (
        &["xlsx"],
        "Work with Rust-supported XLSX workbook commands.",
        "Commands for inspecting and mutating XLSX workbooks where Rust parity has been proven.",
        &[],
    ),
    (
        &["xlsx", "cells"],
        "Read and mutate worksheet cells.",
        "Commands for cell extraction, setting, clearing, and batch updates.",
        &["cell"],
    ),
    (
        &["xlsx", "charts"],
        "Inspect and mutate workbook charts.",
        "Commands for workbook chart readback, authoring, source updates, styling, and type conversion.",
        &["chart"],
    ),
    (
        &["xlsx", "cols"],
        "Insert and delete worksheet columns.",
        "Commands for structural column mutations.",
        &["col"],
    ),
    (
        &["xlsx", "colwidths"],
        "Inspect and set worksheet column widths.",
        "Commands for column-width readback and mutation.",
        &["column-width"],
    ),
    (
        &["xlsx", "comments"],
        "Inspect and mutate XLSX comments.",
        "Commands for legacy cell comments.",
        &["comment"],
    ),
    (
        &["xlsx", "data-validations"],
        "Inspect and mutate worksheet data validations.",
        "Commands for data-validation rules.",
        &["data-validation"],
    ),
    (
        &["xlsx", "filters-sorts"],
        "Inspect and mutate worksheet filters and sorts.",
        "Commands for autoFilter and sortState workflows.",
        &[],
    ),
    (
        &["xlsx", "freeze"],
        "Inspect and set worksheet freeze panes.",
        "Commands for freeze-pane readback and mutation.",
        &[],
    ),
    (
        &["xlsx", "hyperlinks"],
        "Inspect and mutate worksheet hyperlinks.",
        "Commands for internal and external worksheet hyperlinks.",
        &["hyperlink"],
    ),
    (
        &["xlsx", "names"],
        "Inspect and mutate workbook defined names.",
        "Commands for workbook-scoped and sheet-local defined names.",
        &["name"],
    ),
    (
        &["xlsx", "pivots"],
        "Inspect and create workbook PivotTables.",
        "Commands for PivotTable readback and authoring.",
        &["pivot"],
    ),
    (
        &["xlsx", "ranges"],
        "Export and set worksheet ranges.",
        "Commands for rectangular range readback, values, and formatting.",
        &["range"],
    ),
    (
        &["xlsx", "rowheights"],
        "Inspect and set worksheet row heights.",
        "Commands for row-height readback and mutation.",
        &["row-height"],
    ),
    (
        &["xlsx", "rows"],
        "Insert and delete worksheet rows.",
        "Commands for structural row mutations.",
        &["row"],
    ),
    (
        &["xlsx", "sheets"],
        "Inspect and mutate workbook sheets.",
        "Commands for sheet readback and sheet lifecycle mutations.",
        &["sheet"],
    ),
    (
        &["xlsx", "tables"],
        "Inspect and mutate workbook tables.",
        "Commands for table readback, export, appends, and table column formatting.",
        &["table"],
    ),
    (
        &["xlsx", "workbook"],
        "Workbook-level operations.",
        "Commands for workbook metadata inspection and mutation.",
        &[],
    ),
    (
        &["xlsx", "workbook", "metadata"],
        "Inspect and update workbook metadata and calc settings.",
        "Commands for workbook core/app properties and calculation settings.",
        &[],
    ),
];

pub(crate) fn is_help_request(args: &[String]) -> bool {
    if args.is_empty() {
        return true;
    }
    if matches!(args, [cmd] if cmd == "--help" || cmd == "-h") {
        return true;
    }
    if args.first().map(String::as_str) == Some("help") {
        return true;
    }
    if matches!(args.last().map(String::as_str), Some("--help" | "-h")) {
        return is_known_topic(&args[..args.len() - 1]);
    }
    is_parent_group_path(args)
}

pub(crate) fn help(args: &[String]) -> CliResult<DispatchOutput> {
    let topic = normalize_topic(args);
    let text = if topic_matches(&topic, &["conformance", "check"]) {
        conformance_check_gap_help()
    } else if topic.is_empty() {
        root_help()
    } else if is_group_path(&topic) {
        group_help(&topic)?
    } else if let Some(command) = command_for_topic(&topic) {
        leaf_help(&topic, &command)
    } else {
        return Err(CliError::invalid_args(format!(
            "unknown help topic: {}",
            topic.join(" ")
        )));
    };
    Ok(DispatchOutput {
        body: DispatchBody::Text(text),
        exit_code: EXIT_SUCCESS,
    })
}

fn normalize_topic(args: &[String]) -> Vec<String> {
    let mut topic = args.to_vec();
    if topic.first().map(String::as_str) == Some("help") {
        topic.remove(0);
    }
    topic.retain(|arg| arg != "--help" && arg != "-h");
    topic
}

fn is_known_topic(args: &[String]) -> bool {
    args.is_empty()
        || topic_matches(args, &["conformance", "check"])
        || is_group_path(args)
        || command_for_topic(args).is_some()
}

fn is_parent_group_path(args: &[String]) -> bool {
    if !is_group_path(args) {
        return false;
    }
    command_for_topic(args)
        .as_ref()
        .map(is_command_group_capability)
        .unwrap_or(true)
}

fn is_group_path(args: &[String]) -> bool {
    GROUP_TOPICS
        .iter()
        .any(|(path, _, _, _)| path_matches(args, path))
        || capability_commands().into_iter().any(|command| {
            let Some(path) = command["path"].as_str() else {
                return false;
            };
            let words = path
                .strip_prefix("ooxml ")
                .unwrap_or(path)
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            starts_with_topic(&words, args) && words.len() > args.len()
        })
}

fn path_matches(args: &[String], path: &[&str]) -> bool {
    args.len() == path.len()
        && args
            .iter()
            .zip(path.iter())
            .all(|(left, right)| left == right)
}

fn topic_matches(args: &[String], path: &[&str]) -> bool {
    path_matches(args, path)
}

fn group_for_topic(
    topic: &[String],
) -> Option<(&'static str, &'static str, &'static [&'static str])> {
    GROUP_TOPICS
        .iter()
        .find_map(|(path, summary, long, aliases)| {
            if path_matches(topic, path) {
                Some((*summary, *long, *aliases))
            } else {
                None
            }
        })
}

fn root_help() -> String {
    let commands = available_children(&[]);
    let mut out = format!(
        "{ROOT_SUMMARY}\n\n{ROOT_LONG}\n\nUsage:\n  ooxml [flags]\n  ooxml [command]\n\nAvailable Commands:\n"
    );
    out.push_str(&render_children(&commands));
    out.push_str("\nGlobal Flags:\n");
    out.push_str(global_flags_text());
    out.push_str("\nUse \"ooxml help [command]\" for Rust-supported command help.\n");
    out.push_str(
        "Parent/group capability paths are help surfaces; invoke a listed leaf command for work.\n",
    );
    out
}

fn group_help(topic: &[String]) -> CliResult<String> {
    let (summary, long, aliases) = group_for_topic(topic).unwrap_or((
        "Rust-supported command group.",
        "Commands implemented in Rust under this group.",
        EMPTY_ALIASES,
    ));
    let command = topic.join(" ");
    let children = available_children(topic);
    let mut out = format!("{long}\n\nUsage:\n  ooxml {command} [command]\n");
    if !aliases.is_empty() {
        out.push_str("\nAliases:\n  ");
        out.push_str(&topic.last().cloned().unwrap_or_default());
        for alias in aliases {
            out.push_str(", ");
            out.push_str(alias);
        }
        out.push('\n');
    }
    out.push_str("\nAvailable Commands:\n");
    if children.is_empty() {
        out.push_str("  (none promoted in Rust)\n");
    } else {
        out.push_str(&render_children(&children));
    }
    if topic_matches(topic, &["conformance"]) {
        out.push_str("\nIntentionally Unported:\n");
        out.push_str("  check  Go's repair-invariant check is not yet reproduced in Rust; use `ooxml validate --strict <file>` plus `ooxml --json conformance coverage`.\n");
    }
    out.push_str("\nGlobal Flags:\n");
    out.push_str(global_flags_text());
    out.push('\n');
    out.push_str(summary);
    out.push('\n');
    Ok(out)
}

fn leaf_help(topic: &[String], command: &Value) -> String {
    let path = topic.join(" ");
    let use_text = command["use"].as_str().unwrap_or_default();
    let short = command["short"]
        .as_str()
        .unwrap_or("Rust-supported command.");
    let usage = if use_text.is_empty() { &path } else { use_text };
    let mut out = format!("{short}\n\nUsage:\n  ooxml {usage}\n");
    if let Some(flags) = command["localFlags"].as_array()
        && !flags.is_empty()
    {
        out.push_str("\nFlags:\n");
        for flag in flags {
            let name = flag["name"].as_str().unwrap_or_default();
            let description = flag["description"].as_str().unwrap_or_default();
            out.push_str(&format!("  {name:<24} {description}\n"));
        }
    }
    out.push_str("\nGlobal Flags:\n");
    out.push_str(global_flags_text());
    out
}

fn conformance_check_gap_help() -> String {
    "Go `ooxml conformance check` runs package-open, repo-validation, repair-invariants, and optional office-open checks.\n\nThe Rust port does not yet reproduce the repair-invariant stage, so this command remains intentionally unimplemented and unadvertised.\n\nUse:\n  ooxml validate --strict <file>\n  ooxml --json conformance coverage\n".to_string()
}

fn available_children(topic: &[String]) -> Vec<(String, String)> {
    let mut children = BTreeMap::<String, String>::new();
    let mut seen = BTreeSet::<String>::new();
    for command in capability_commands() {
        let Some(path) = command["path"].as_str() else {
            continue;
        };
        let words = path
            .strip_prefix("ooxml ")
            .unwrap_or(path)
            .split_whitespace()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if !starts_with_topic(&words, topic) || words.len() <= topic.len() {
            continue;
        }
        let child = words[topic.len()].clone();
        if !seen.insert(child.clone()) {
            continue;
        }
        let child_path = topic
            .iter()
            .cloned()
            .chain(std::iter::once(child.clone()))
            .collect::<Vec<_>>();
        let description = group_for_topic(&child_path)
            .map(|(summary, _, _)| summary.to_string())
            .unwrap_or_else(|| {
                command["short"]
                    .as_str()
                    .unwrap_or("Rust-supported command.")
                    .to_string()
            });
        children.insert(child, description);
    }
    if topic.is_empty() {
        children.insert(
            "help".to_string(),
            "Show Rust-supported command help.".to_string(),
        );
    }
    children.into_iter().collect()
}

fn starts_with_topic(words: &[String], topic: &[String]) -> bool {
    words.len() >= topic.len()
        && words
            .iter()
            .zip(topic.iter())
            .all(|(left, right)| left == right)
}

fn command_for_topic(topic: &[String]) -> Option<Value> {
    let wanted = if topic.is_empty() {
        return None;
    } else {
        format!("ooxml {}", topic.join(" "))
    };
    capability_commands()
        .into_iter()
        .find(|command| command["path"].as_str() == Some(wanted.as_str()))
}

fn is_command_group_capability(command: &Value) -> bool {
    command["opCompatible"].as_bool() == Some(false)
        && command["localFlags"]
            .as_array()
            .map(Vec::is_empty)
            .unwrap_or(true)
        && command["targetObjectKinds"]
            .as_array()
            .map(Vec::is_empty)
            .unwrap_or(true)
}

fn render_children(children: &[(String, String)]) -> String {
    let width = children
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0)
        .max(4);
    let mut out = String::new();
    for (name, description) in children {
        out.push_str(&format!("  {name:<width$}  {description}\n"));
    }
    out
}

fn global_flags_text() -> &'static str {
    "  -f, --format json          emit JSON output for JSON-capable commands\n      --json                 emit JSON output\n      --strict               enable strict validation mode\n"
}
