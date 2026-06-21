mod commands;
pub(crate) use commands::capability_commands;

use serde_json::{Value, json};

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
        "The deprecated Go implementation is retained on codex/ooxml-go-reference as a frozen oracle/reference."
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
    let mut document = json!({
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
        "objectKinds": CAPABILITY_OBJECT_KINDS,
        "filterAliases": capability_filter_aliases_json(),
        "objectKindsIndex": {
            "package": ["ooxml inspect", "ooxml validate", "ooxml verify", "ooxml apply", "ooxml convert xlsm-to-xlsx", "ooxml repair normalize", "ooxml docx scaffold", "ooxml docx text", "ooxml pptx scaffold", "ooxml xlsx scaffold", "ooxml xlsx workbook metadata inspect", "ooxml xlsx workbook metadata update", "ooxml vba build-bin", "ooxml vba create", "ooxml vba rebuild", "ooxml vba inspect", "ooxml vba extract-bin", "ooxml vba inspect-bin", "ooxml vba list", "ooxml vba extract", "ooxml vba add-module", "ooxml vba replace-module", "ooxml vba remove-module", "ooxml vba attach", "ooxml vba remove"],
            "template": ["ooxml template", "ooxml template apply", "ooxml template tokens", "ooxml template profile", "ooxml template profile save", "ooxml template profile inspect", "ooxml pptx template", "ooxml pptx template inspect", "ooxml pptx template capture", "ooxml pptx template compile", "ooxml pptx xlsx-bindings plan", "ooxml pptx xlsx-bindings apply"],
            "slide": ["ooxml pptx scaffold", "ooxml pptx slides list", "ooxml pptx slides selectors", "ooxml pptx slides show", "ooxml pptx slides delete", "ooxml pptx slides move", "ooxml pptx slides reorder", "ooxml pptx slides import-slide", "ooxml pptx slides merge", "ooxml pptx clone-slide", "ooxml pptx new-slide-from-layout", "ooxml pptx shapes show", "ooxml pptx shapes get", "ooxml pptx add-textbox", "ooxml pptx place image", "ooxml pptx place table", "ooxml pptx place table-from-xlsx", "ooxml pptx shapes set-bounds", "ooxml pptx shapes delete", "ooxml pptx text set", "ooxml pptx translate export", "ooxml pptx translate apply", "ooxml pptx fields inspect", "ooxml pptx fields set", "ooxml pptx theme update", "ooxml pptx animations list", "ooxml pptx animations add", "ooxml pptx animations remove", "ooxml pptx animations reorder", "ooxml pptx animations prune-stale", "ooxml pptx layouts list", "ooxml pptx layouts show", "ooxml pptx tables show", "ooxml pptx tables set-cell", "ooxml pptx tables delete-row", "ooxml pptx tables insert-row", "ooxml pptx tables delete-col", "ooxml pptx tables insert-col", "ooxml pptx tables update-from-xlsx", "ooxml pptx charts list", "ooxml pptx charts show", "ooxml pptx charts create", "ooxml pptx charts update-data", "ooxml pptx charts set-title", "ooxml pptx charts set-legend", "ooxml pptx charts set-chart-area-fill", "ooxml pptx charts set-plot-area-fill", "ooxml pptx charts set-series-style", "ooxml pptx charts set-axis", "ooxml pptx charts convert-type", "ooxml pptx charts copy-style", "ooxml pptx extract text", "ooxml pptx extract notes", "ooxml pptx extract images", "ooxml pptx extract xml", "ooxml pptx media list", "ooxml pptx media add", "ooxml pptx media replace", "ooxml pptx notes show", "ooxml pptx notes set", "ooxml pptx notes clear", "ooxml pptx comments list", "ooxml pptx replace text", "ooxml pptx replace text-occurrences", "ooxml pptx replace text-from-xlsx", "ooxml pptx replace text-map-from-xlsx", "ooxml pptx replace images", "ooxml pptx xlsx-bindings apply", "ooxml pptx render"],
            "shape": ["ooxml pptx scaffold", "ooxml pptx slides list", "ooxml pptx slides selectors", "ooxml pptx slides show", "ooxml pptx shapes show", "ooxml pptx shapes get", "ooxml pptx add-textbox", "ooxml pptx place image", "ooxml pptx place table", "ooxml pptx place table-from-xlsx", "ooxml pptx shapes set-bounds", "ooxml pptx shapes delete", "ooxml pptx text set", "ooxml pptx translate export", "ooxml pptx translate apply", "ooxml pptx animations list", "ooxml pptx animations add", "ooxml pptx animations remove", "ooxml pptx animations prune-stale", "ooxml pptx layouts set-bounds", "ooxml pptx layouts delete-shape", "ooxml pptx extract text", "ooxml pptx replace text", "ooxml pptx replace text-occurrences", "ooxml pptx replace text-from-xlsx", "ooxml pptx replace text-map-from-xlsx", "ooxml pptx replace images", "ooxml pptx media replace", "ooxml pptx xlsx-bindings apply"],
            "animation": ["ooxml pptx animations list", "ooxml pptx animations add", "ooxml pptx animations remove", "ooxml pptx animations reorder", "ooxml pptx animations prune-stale"],
            "master": ["ooxml pptx slides import-slide", "ooxml pptx slides merge", "ooxml pptx masters list", "ooxml pptx masters show", "ooxml pptx masters import", "ooxml pptx masters add-placeholder", "ooxml pptx layouts import", "ooxml pptx fields inspect", "ooxml pptx fields set", "ooxml pptx extract xml"],
            "layout": ["ooxml pptx slides import-slide", "ooxml pptx slides merge", "ooxml pptx new-slide-from-layout", "ooxml pptx layouts list", "ooxml pptx layouts show", "ooxml pptx layouts import", "ooxml pptx layouts clone", "ooxml pptx layouts rename", "ooxml pptx layouts set-bounds", "ooxml pptx layouts delete-shape", "ooxml pptx layouts add-placeholder", "ooxml pptx masters import", "ooxml pptx extract xml"],
            "placeholder": ["ooxml pptx new-slide-from-layout", "ooxml pptx masters show", "ooxml pptx masters add-placeholder", "ooxml pptx layouts list", "ooxml pptx layouts show", "ooxml pptx layouts set-bounds", "ooxml pptx layouts delete-shape", "ooxml pptx layouts add-placeholder"],
            "sheet": ["ooxml pptx tables update-from-xlsx", "ooxml pptx place table-from-xlsx", "ooxml pptx replace text-from-xlsx", "ooxml pptx replace text-map-from-xlsx", "ooxml pptx xlsx-bindings apply", "ooxml xlsx scaffold", "ooxml xlsx sheets list", "ooxml xlsx sheets show", "ooxml xlsx sheets add", "ooxml xlsx sheets rename", "ooxml xlsx sheets move", "ooxml xlsx sheets delete", "ooxml xlsx colwidths show", "ooxml xlsx colwidths set", "ooxml xlsx rowheights show", "ooxml xlsx rowheights set", "ooxml xlsx rows insert", "ooxml xlsx rows delete", "ooxml xlsx cols insert", "ooxml xlsx cols delete", "ooxml xlsx filters-sorts show", "ooxml xlsx filters-sorts set-autofilter", "ooxml xlsx filters-sorts clear-autofilter", "ooxml xlsx filters-sorts add-column-filter", "ooxml xlsx filters-sorts clear-column-filter", "ooxml xlsx filters-sorts set-sort", "ooxml xlsx filters-sorts clear-sort", "ooxml xlsx comments list", "ooxml xlsx comments add", "ooxml xlsx comments update", "ooxml xlsx comments remove", "ooxml xlsx conditional-formats list", "ooxml xlsx conditional-formats show", "ooxml xlsx conditional-formats add", "ooxml xlsx conditional-formats delete", "ooxml xlsx conditional-formats reorder", "ooxml xlsx data-validations list", "ooxml xlsx data-validations show", "ooxml xlsx data-validations create", "ooxml xlsx data-validations update", "ooxml xlsx data-validations delete", "ooxml xlsx hyperlinks list", "ooxml xlsx hyperlinks show", "ooxml xlsx hyperlinks add", "ooxml xlsx hyperlinks update", "ooxml xlsx hyperlinks delete", "ooxml xlsx ranges export", "ooxml xlsx ranges set", "ooxml xlsx ranges set-format", "ooxml xlsx ranges set-style", "ooxml xlsx cells extract", "ooxml xlsx cells set", "ooxml xlsx cells clear", "ooxml xlsx cells set-batch", "ooxml xlsx freeze show", "ooxml xlsx freeze set", "ooxml xlsx freeze clear", "ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export", "ooxml xlsx tables create", "ooxml xlsx tables append-rows", "ooxml xlsx tables append-records", "ooxml xlsx tables set-column-format", "ooxml xlsx pivots list", "ooxml xlsx pivots show", "ooxml xlsx pivots create", "ooxml xlsx names list", "ooxml xlsx names show", "ooxml xlsx names add", "ooxml xlsx names update", "ooxml xlsx names rename", "ooxml xlsx names delete"],
            "range": ["ooxml pptx tables update-from-xlsx", "ooxml pptx place table-from-xlsx", "ooxml pptx replace text-from-xlsx", "ooxml pptx replace text-map-from-xlsx", "ooxml pptx xlsx-bindings apply", "ooxml pptx charts create", "ooxml xlsx colwidths show", "ooxml xlsx colwidths set", "ooxml xlsx rowheights show", "ooxml xlsx rowheights set", "ooxml xlsx rows insert", "ooxml xlsx rows delete", "ooxml xlsx cols insert", "ooxml xlsx cols delete", "ooxml xlsx filters-sorts show", "ooxml xlsx filters-sorts set-autofilter", "ooxml xlsx filters-sorts clear-autofilter", "ooxml xlsx filters-sorts add-column-filter", "ooxml xlsx filters-sorts clear-column-filter", "ooxml xlsx filters-sorts set-sort", "ooxml xlsx filters-sorts clear-sort", "ooxml xlsx conditional-formats list", "ooxml xlsx conditional-formats show", "ooxml xlsx conditional-formats add", "ooxml xlsx conditional-formats delete", "ooxml xlsx conditional-formats reorder", "ooxml xlsx data-validations list", "ooxml xlsx data-validations show", "ooxml xlsx data-validations create", "ooxml xlsx data-validations update", "ooxml xlsx data-validations delete", "ooxml xlsx hyperlinks list", "ooxml xlsx hyperlinks show", "ooxml xlsx hyperlinks add", "ooxml xlsx hyperlinks update", "ooxml xlsx hyperlinks delete", "ooxml xlsx ranges export", "ooxml xlsx ranges set", "ooxml xlsx ranges set-format", "ooxml xlsx ranges set-style", "ooxml xlsx cells extract", "ooxml xlsx cells clear", "ooxml xlsx cells set-batch", "ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export", "ooxml xlsx tables create", "ooxml xlsx tables append-rows", "ooxml xlsx tables append-records", "ooxml xlsx tables set-column-format", "ooxml xlsx pivots list", "ooxml xlsx pivots show", "ooxml xlsx pivots create", "ooxml xlsx names list", "ooxml xlsx names show", "ooxml xlsx names add", "ooxml xlsx names update", "ooxml xlsx names rename", "ooxml xlsx names delete"],
            "conditional-format": ["ooxml xlsx conditional-formats list", "ooxml xlsx conditional-formats show", "ooxml xlsx conditional-formats add", "ooxml xlsx conditional-formats delete", "ooxml xlsx conditional-formats reorder"],
            "data-validation": ["ooxml xlsx data-validations list", "ooxml xlsx data-validations show", "ooxml xlsx data-validations create", "ooxml xlsx data-validations update", "ooxml xlsx data-validations delete"],
            "cell": ["ooxml xlsx comments list", "ooxml xlsx comments add", "ooxml xlsx comments update", "ooxml xlsx comments remove", "ooxml xlsx hyperlinks list", "ooxml xlsx hyperlinks show", "ooxml xlsx hyperlinks add", "ooxml xlsx hyperlinks update", "ooxml xlsx hyperlinks delete", "ooxml xlsx ranges set", "ooxml xlsx cells set", "ooxml xlsx cells clear", "ooxml xlsx cells set-batch"],
            "hyperlink": ["ooxml pptx text set", "ooxml xlsx hyperlinks list", "ooxml xlsx hyperlinks show", "ooxml xlsx hyperlinks add", "ooxml xlsx hyperlinks update", "ooxml xlsx hyperlinks delete"],
            "chart": ["ooxml pptx charts list", "ooxml pptx charts show", "ooxml pptx charts create", "ooxml pptx charts update-data", "ooxml pptx charts set-title", "ooxml pptx charts set-legend", "ooxml pptx charts set-chart-area-fill", "ooxml pptx charts set-plot-area-fill", "ooxml pptx charts set-series-style", "ooxml pptx charts set-axis", "ooxml pptx charts convert-type", "ooxml pptx charts copy-style", "ooxml xlsx charts list", "ooxml xlsx charts show", "ooxml xlsx charts create", "ooxml xlsx charts update-source", "ooxml xlsx charts set-title", "ooxml xlsx charts set-legend", "ooxml xlsx charts set-chart-area-fill", "ooxml xlsx charts set-plot-area-fill", "ooxml xlsx charts set-series-style", "ooxml xlsx charts convert-type", "ooxml xlsx charts copy-style", "ooxml xlsx charts set-axis"],
            "table": ["ooxml pptx place table", "ooxml pptx place table-from-xlsx", "ooxml pptx tables show", "ooxml pptx tables set-cell", "ooxml pptx tables delete-row", "ooxml pptx tables insert-row", "ooxml pptx tables delete-col", "ooxml pptx tables insert-col", "ooxml pptx tables update-from-xlsx", "ooxml pptx xlsx-bindings apply", "ooxml xlsx filters-sorts show", "ooxml xlsx filters-sorts set-autofilter", "ooxml xlsx filters-sorts clear-autofilter", "ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export", "ooxml xlsx tables create", "ooxml xlsx tables append-rows", "ooxml xlsx tables append-records", "ooxml xlsx tables set-column-format", "ooxml xlsx pivots list", "ooxml xlsx pivots show", "ooxml xlsx pivots create", "ooxml docx tables show", "ooxml docx tables create", "ooxml docx tables set-cell", "ooxml docx tables clear-cell", "ooxml docx tables insert-row", "ooxml docx tables delete-row", "ooxml docx replace", "ooxml docx styles apply"],
            "pivot": ["ooxml xlsx pivots list", "ooxml xlsx pivots show", "ooxml xlsx pivots create"],
            "name": ["ooxml xlsx names list", "ooxml xlsx names show", "ooxml xlsx names add", "ooxml xlsx names update", "ooxml xlsx names rename", "ooxml xlsx names delete"],
            "block": ["ooxml docx blocks", "ooxml docx blocks replace", "ooxml docx blocks delete", "ooxml docx blocks insert-after", "ooxml docx tables show"],
            "paragraph": ["ooxml docx text", "ooxml docx blocks", "ooxml docx blocks replace", "ooxml docx blocks delete", "ooxml docx blocks insert-after", "ooxml docx paragraphs append", "ooxml docx paragraphs insert", "ooxml docx paragraphs set", "ooxml docx paragraphs clear", "ooxml docx replace", "ooxml docx styles apply", "ooxml docx headers show", "ooxml docx headers set-text", "ooxml docx footers show", "ooxml docx footers set-text", "ooxml docx images list", "ooxml docx images insert"],
            "style": ["ooxml pptx text set", "ooxml pptx theme update", "ooxml pptx charts set-title", "ooxml pptx charts set-legend", "ooxml pptx charts set-chart-area-fill", "ooxml pptx charts set-plot-area-fill", "ooxml pptx charts set-series-style", "ooxml pptx charts set-axis", "ooxml pptx charts convert-type", "ooxml pptx charts copy-style", "ooxml xlsx charts set-title", "ooxml xlsx charts set-legend", "ooxml xlsx charts set-chart-area-fill", "ooxml xlsx charts set-plot-area-fill", "ooxml xlsx charts set-series-style", "ooxml xlsx charts copy-style", "ooxml xlsx charts set-axis", "ooxml xlsx ranges set-format", "ooxml xlsx ranges set-style", "ooxml xlsx tables set-column-format", "ooxml docx styles list", "ooxml docx styles show", "ooxml docx styles apply"],
            "theme": ["ooxml pptx slides import-slide", "ooxml pptx slides merge", "ooxml pptx masters import", "ooxml pptx layouts import"],
            "comment": ["ooxml pptx comments list", "ooxml pptx comments add", "ooxml pptx comments edit", "ooxml pptx comments remove", "ooxml xlsx comments list", "ooxml xlsx comments add", "ooxml xlsx comments update", "ooxml xlsx comments remove", "ooxml docx comments list", "ooxml docx comments add", "ooxml docx comments edit", "ooxml docx comments remove"],
            "field": ["ooxml pptx fields inspect", "ooxml pptx fields set", "ooxml docx fields list", "ooxml docx fields insert", "ooxml docx fields set-result"],
            "header": ["ooxml docx headers list", "ooxml docx headers show", "ooxml docx headers set-text", "ooxml docx footers list"],
            "footer": ["ooxml pptx fields inspect", "ooxml pptx fields set", "ooxml docx footers list", "ooxml docx footers show", "ooxml docx footers set-text", "ooxml docx headers list"],
            "image": ["ooxml pptx extract images", "ooxml pptx place image", "ooxml pptx replace images", "ooxml pptx xlsx-bindings apply", "ooxml docx images list", "ooxml docx images replace", "ooxml docx images insert"],
            "media": ["ooxml pptx media list", "ooxml pptx media add", "ooxml pptx media replace"],
            "module": ["ooxml vba build-bin", "ooxml vba create", "ooxml vba rebuild", "ooxml vba inspect", "ooxml vba extract-bin", "ooxml vba inspect-bin", "ooxml vba list", "ooxml vba extract", "ooxml vba add-module", "ooxml vba replace-module", "ooxml vba remove-module", "ooxml vba office-check", "ooxml vba run-smoke", "ooxml vba attach", "ooxml vba remove"]
        },
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
            "--json" => i += 1,
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
                    "; valid flags are --for <filter>, --json, and --format json"
                };
                return Err(CliError::invalid_args(format!(
                    "unknown flag: {flag}{hint}"
                )));
            }
        }
    }
    Ok(())
}
