mod commands;
pub(crate) use commands::capability_commands;

use serde_json::{Value, json};

use crate::{
    CliResult, EXIT_FILE_NOT_FOUND, EXIT_INVALID_ARGS, EXIT_SUCCESS, EXIT_TARGET_NOT_FOUND,
    EXIT_UNEXPECTED, EXIT_UNSUPPORTED_TYPE, parse_string_flag,
};

pub(crate) fn capabilities(args: &[String]) -> CliResult<Value> {
    let filter = parse_string_flag(args, "--for")?.map(|value| value.to_ascii_lowercase());
    let mut commands = capability_commands();
    if let Some(filter) = filter.as_deref() {
        commands.retain(|command| capability_matches_filter(command, filter));
    }
    let mut notes = vec![
        "Rust port partial surface: only commands listed here are implemented in the Rust subject."
            .to_string(),
        "Use Go on codex/ooxml-go-reference as the oracle for the full command universe."
            .to_string(),
    ];
    if let Some(filter) = filter.as_deref() {
        notes.insert(
            0,
            format!("Filtered by Rust-supported command/object filter \"{filter}\"."),
        );
    }
    Ok(json!({
        "tool": "ooxml",
        "version": "0.0.1",
        "contractVersion": "ooxml-cli.agent-capabilities.v4",
        "packageTypes": ["pptx", "xlsx", "docx"],
        "outputModes": ["json via --json or --format json"],
        "globalFlags": [
            {"name": "--format", "argName": "format", "shorthand": "f", "type": "string", "default": "text", "description": "output format: \"text\" or \"json\""},
            {"name": "--json", "argName": "json", "type": "bool", "default": "false", "description": "emit JSON output"},
            {"name": "--strict", "argName": "strict", "type": "bool", "default": "false", "description": "enable strict validation mode"}
        ],
        "commands": commands,
        "objectKinds": ["package", "slide", "shape", "master", "layout", "placeholder", "sheet", "range", "cell", "table", "name", "block", "paragraph", "style", "comment", "field", "header", "footer", "image", "module"],
        "objectKindsIndex": {
            "package": ["ooxml inspect", "ooxml validate", "ooxml verify", "ooxml docx text", "ooxml xlsx workbook metadata inspect", "ooxml xlsx workbook metadata update", "ooxml vba inspect", "ooxml vba extract-bin", "ooxml vba attach", "ooxml vba remove"],
            "slide": ["ooxml pptx slides list", "ooxml pptx slides selectors", "ooxml pptx slides show", "ooxml pptx shapes show", "ooxml pptx layouts list", "ooxml pptx layouts show", "ooxml pptx tables show", "ooxml pptx tables set-cell", "ooxml pptx tables delete-row", "ooxml pptx extract text", "ooxml pptx extract notes", "ooxml pptx notes show", "ooxml pptx comments list", "ooxml pptx replace text", "ooxml pptx render"],
            "shape": ["ooxml pptx slides list", "ooxml pptx slides selectors", "ooxml pptx slides show", "ooxml pptx shapes show", "ooxml pptx extract text", "ooxml pptx replace text"],
            "master": ["ooxml pptx masters list", "ooxml pptx masters show"],
            "layout": ["ooxml pptx layouts list", "ooxml pptx layouts show"],
            "placeholder": ["ooxml pptx masters show", "ooxml pptx layouts list", "ooxml pptx layouts show"],
            "sheet": ["ooxml xlsx sheets list", "ooxml xlsx sheets show", "ooxml xlsx colwidths show", "ooxml xlsx rowheights show", "ooxml xlsx filters-sorts show", "ooxml xlsx filters-sorts set-autofilter", "ooxml xlsx comments list", "ooxml xlsx comments add", "ooxml xlsx comments update", "ooxml xlsx comments remove", "ooxml xlsx ranges export", "ooxml xlsx ranges set", "ooxml xlsx ranges set-format", "ooxml xlsx cells extract", "ooxml xlsx cells set", "ooxml xlsx freeze show", "ooxml xlsx freeze set", "ooxml xlsx freeze clear", "ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export", "ooxml xlsx tables append-rows", "ooxml xlsx tables append-records", "ooxml xlsx names list", "ooxml xlsx names show", "ooxml xlsx names add", "ooxml xlsx names update", "ooxml xlsx names rename", "ooxml xlsx names delete"],
            "range": ["ooxml xlsx colwidths show", "ooxml xlsx rowheights show", "ooxml xlsx filters-sorts show", "ooxml xlsx filters-sorts set-autofilter", "ooxml xlsx ranges export", "ooxml xlsx ranges set", "ooxml xlsx ranges set-format", "ooxml xlsx cells extract", "ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export", "ooxml xlsx tables append-rows", "ooxml xlsx tables append-records", "ooxml xlsx names list", "ooxml xlsx names show", "ooxml xlsx names add", "ooxml xlsx names update", "ooxml xlsx names rename", "ooxml xlsx names delete"],
            "cell": ["ooxml xlsx comments list", "ooxml xlsx comments add", "ooxml xlsx comments update", "ooxml xlsx comments remove", "ooxml xlsx ranges set", "ooxml xlsx cells set"],
            "table": ["ooxml pptx tables show", "ooxml pptx tables set-cell", "ooxml pptx tables delete-row", "ooxml xlsx filters-sorts show", "ooxml xlsx filters-sorts set-autofilter", "ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export", "ooxml xlsx tables append-rows", "ooxml xlsx tables append-records", "ooxml docx tables show", "ooxml docx tables set-cell", "ooxml docx tables clear-cell", "ooxml docx tables insert-row", "ooxml docx tables delete-row", "ooxml docx styles apply"],
            "name": ["ooxml xlsx names list", "ooxml xlsx names show", "ooxml xlsx names add", "ooxml xlsx names update", "ooxml xlsx names rename", "ooxml xlsx names delete"],
            "block": ["ooxml docx blocks", "ooxml docx blocks replace", "ooxml docx blocks delete", "ooxml docx blocks insert-after", "ooxml docx tables show"],
            "paragraph": ["ooxml docx text", "ooxml docx blocks", "ooxml docx blocks replace", "ooxml docx blocks delete", "ooxml docx blocks insert-after", "ooxml docx paragraphs append", "ooxml docx paragraphs insert", "ooxml docx paragraphs set", "ooxml docx paragraphs clear", "ooxml docx styles apply", "ooxml docx headers show", "ooxml docx headers set-text", "ooxml docx footers show", "ooxml docx footers set-text", "ooxml docx images list", "ooxml docx images insert"],
            "style": ["ooxml xlsx ranges set-format", "ooxml docx styles list", "ooxml docx styles show", "ooxml docx styles apply"],
            "comment": ["ooxml pptx comments list", "ooxml pptx comments add", "ooxml pptx comments edit", "ooxml pptx comments remove", "ooxml xlsx comments list", "ooxml xlsx comments add", "ooxml xlsx comments update", "ooxml xlsx comments remove", "ooxml docx comments list", "ooxml docx comments add", "ooxml docx comments edit", "ooxml docx comments remove"],
            "field": ["ooxml docx fields list", "ooxml docx fields insert", "ooxml docx fields set-result"],
            "header": ["ooxml docx headers list", "ooxml docx headers show", "ooxml docx headers set-text", "ooxml docx footers list"],
            "footer": ["ooxml docx footers list", "ooxml docx footers show", "ooxml docx footers set-text", "ooxml docx headers list"],
            "image": ["ooxml docx images list", "ooxml docx images replace", "ooxml docx images insert"],
            "module": ["ooxml vba inspect", "ooxml vba extract-bin", "ooxml vba attach", "ooxml vba remove"]
        },
        "exitCodes": [
            {"code": EXIT_SUCCESS, "name": "success", "description": "command completed successfully"},
            {"code": EXIT_UNEXPECTED, "name": "unexpected", "description": "unexpected tool or package processing error"},
            {"code": EXIT_INVALID_ARGS, "name": "invalid_args", "description": "invalid command line arguments or incompatible options"},
            {"code": EXIT_FILE_NOT_FOUND, "name": "file_not_found", "description": "input file was not found"},
            {"code": EXIT_UNSUPPORTED_TYPE, "name": "unsupported_type", "description": "input package type is unsupported for the requested command"},
            {"code": EXIT_TARGET_NOT_FOUND, "name": "target_not_found", "description": "requested slide, sheet, table, shape, or macro part was not found"}
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
                    "ooxml --json xlsx ranges set-format workbook.xlsx --sheet Sheet1 --range B2:B20 --preset currency --out edited.xlsx",
                    "serve op commands: xlsx cells set, xlsx ranges set, xlsx ranges set-format, xlsx comments add, xlsx comments update, xlsx comments remove, xlsx tables append-rows, xlsx tables append-records, xlsx workbook metadata update"
                ]
            }
        ],
        "conventions": [
            "stdout is data; diagnostics and errors go to stderr",
            "serve/MCP operation commands use op vocabulary without the leading ooxml",
            "mutations should be validated before handing files to users"
        ],
        "notes": notes,
    }))
}

fn capability_matches_filter(command: &Value, filter: &str) -> bool {
    let path = command
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if path.contains(&format!(" {filter} ")) || path.ends_with(&format!(" {filter}")) {
        return true;
    }
    command
        .get("targetObjectKinds")
        .and_then(Value::as_array)
        .map(|kinds| kinds.iter().any(|kind| kind.as_str() == Some(filter)))
        .unwrap_or(false)
}
