mod commands;
pub(crate) use commands::capability_commands;

use serde_json::{Value, json};

use crate::{
    CliResult, EXIT_FILE_NOT_FOUND, EXIT_INVALID_ARGS, EXIT_RENDER_FAILED, EXIT_SUCCESS,
    EXIT_TARGET_NOT_FOUND, EXIT_UNEXPECTED, EXIT_UNSUPPORTED_TYPE, parse_string_flag,
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
        "objectKinds": ["package", "slide", "shape", "animation", "master", "layout", "placeholder", "sheet", "range", "data-validation", "cell", "hyperlink", "table", "name", "block", "paragraph", "style", "comment", "chart", "field", "header", "footer", "image", "media", "module"],
        "objectKindsIndex": {
            "package": ["ooxml inspect", "ooxml validate", "ooxml verify", "ooxml apply", "ooxml docx text", "ooxml xlsx workbook metadata inspect", "ooxml xlsx workbook metadata update", "ooxml vba create", "ooxml vba inspect", "ooxml vba extract-bin", "ooxml vba inspect-bin", "ooxml vba list", "ooxml vba extract", "ooxml vba attach", "ooxml vba remove"],
            "slide": ["ooxml pptx slides list", "ooxml pptx slides selectors", "ooxml pptx slides show", "ooxml pptx slides delete", "ooxml pptx slides move", "ooxml pptx slides reorder", "ooxml pptx shapes show", "ooxml pptx shapes get", "ooxml pptx shapes set-bounds", "ooxml pptx shapes delete", "ooxml pptx animations list", "ooxml pptx animations add", "ooxml pptx animations remove", "ooxml pptx animations reorder", "ooxml pptx animations prune-stale", "ooxml pptx layouts list", "ooxml pptx layouts show", "ooxml pptx tables show", "ooxml pptx tables set-cell", "ooxml pptx tables delete-row", "ooxml pptx tables insert-row", "ooxml pptx tables delete-col", "ooxml pptx tables insert-col", "ooxml pptx tables update-from-xlsx", "ooxml pptx charts list", "ooxml pptx charts show", "ooxml pptx charts set-title", "ooxml pptx charts set-legend", "ooxml pptx charts set-chart-area-fill", "ooxml pptx charts set-plot-area-fill", "ooxml pptx charts set-series-style", "ooxml pptx extract text", "ooxml pptx extract notes", "ooxml pptx extract images", "ooxml pptx extract xml", "ooxml pptx media list", "ooxml pptx media add", "ooxml pptx media replace", "ooxml pptx notes show", "ooxml pptx notes set", "ooxml pptx notes clear", "ooxml pptx comments list", "ooxml pptx replace text", "ooxml pptx replace text-occurrences", "ooxml pptx replace text-from-xlsx", "ooxml pptx replace text-map-from-xlsx", "ooxml pptx replace images", "ooxml pptx render"],
            "shape": ["ooxml pptx slides list", "ooxml pptx slides selectors", "ooxml pptx slides show", "ooxml pptx shapes show", "ooxml pptx shapes get", "ooxml pptx shapes set-bounds", "ooxml pptx shapes delete", "ooxml pptx animations list", "ooxml pptx animations add", "ooxml pptx animations remove", "ooxml pptx animations prune-stale", "ooxml pptx layouts set-bounds", "ooxml pptx layouts delete-shape", "ooxml pptx extract text", "ooxml pptx replace text", "ooxml pptx replace text-occurrences", "ooxml pptx replace text-from-xlsx", "ooxml pptx replace text-map-from-xlsx", "ooxml pptx replace images", "ooxml pptx media replace"],
            "animation": ["ooxml pptx animations list", "ooxml pptx animations add", "ooxml pptx animations remove", "ooxml pptx animations reorder", "ooxml pptx animations prune-stale"],
            "master": ["ooxml pptx masters list", "ooxml pptx masters show", "ooxml pptx extract xml"],
            "layout": ["ooxml pptx layouts list", "ooxml pptx layouts show", "ooxml pptx layouts rename", "ooxml pptx layouts set-bounds", "ooxml pptx layouts delete-shape", "ooxml pptx layouts add-placeholder", "ooxml pptx extract xml"],
            "placeholder": ["ooxml pptx masters show", "ooxml pptx layouts list", "ooxml pptx layouts show", "ooxml pptx layouts set-bounds", "ooxml pptx layouts delete-shape", "ooxml pptx layouts add-placeholder"],
            "sheet": ["ooxml pptx tables update-from-xlsx", "ooxml pptx replace text-from-xlsx", "ooxml pptx replace text-map-from-xlsx", "ooxml xlsx sheets list", "ooxml xlsx sheets show", "ooxml xlsx sheets add", "ooxml xlsx sheets rename", "ooxml xlsx sheets move", "ooxml xlsx sheets delete", "ooxml xlsx colwidths show", "ooxml xlsx colwidths set", "ooxml xlsx rowheights show", "ooxml xlsx rowheights set", "ooxml xlsx rows insert", "ooxml xlsx rows delete", "ooxml xlsx cols insert", "ooxml xlsx cols delete", "ooxml xlsx filters-sorts show", "ooxml xlsx filters-sorts set-autofilter", "ooxml xlsx filters-sorts clear-autofilter", "ooxml xlsx filters-sorts add-column-filter", "ooxml xlsx filters-sorts clear-column-filter", "ooxml xlsx filters-sorts set-sort", "ooxml xlsx filters-sorts clear-sort", "ooxml xlsx comments list", "ooxml xlsx comments add", "ooxml xlsx comments update", "ooxml xlsx comments remove", "ooxml xlsx data-validations list", "ooxml xlsx data-validations show", "ooxml xlsx data-validations create", "ooxml xlsx data-validations update", "ooxml xlsx data-validations delete", "ooxml xlsx hyperlinks list", "ooxml xlsx hyperlinks show", "ooxml xlsx hyperlinks add", "ooxml xlsx hyperlinks update", "ooxml xlsx hyperlinks delete", "ooxml xlsx ranges export", "ooxml xlsx ranges set", "ooxml xlsx ranges set-format", "ooxml xlsx ranges set-style", "ooxml xlsx cells extract", "ooxml xlsx cells set", "ooxml xlsx cells clear", "ooxml xlsx cells set-batch", "ooxml xlsx freeze show", "ooxml xlsx freeze set", "ooxml xlsx freeze clear", "ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export", "ooxml xlsx tables append-rows", "ooxml xlsx tables append-records", "ooxml xlsx names list", "ooxml xlsx names show", "ooxml xlsx names add", "ooxml xlsx names update", "ooxml xlsx names rename", "ooxml xlsx names delete"],
            "range": ["ooxml pptx tables update-from-xlsx", "ooxml pptx replace text-from-xlsx", "ooxml pptx replace text-map-from-xlsx", "ooxml xlsx colwidths show", "ooxml xlsx colwidths set", "ooxml xlsx rowheights show", "ooxml xlsx rowheights set", "ooxml xlsx rows insert", "ooxml xlsx rows delete", "ooxml xlsx cols insert", "ooxml xlsx cols delete", "ooxml xlsx filters-sorts show", "ooxml xlsx filters-sorts set-autofilter", "ooxml xlsx filters-sorts clear-autofilter", "ooxml xlsx filters-sorts add-column-filter", "ooxml xlsx filters-sorts clear-column-filter", "ooxml xlsx filters-sorts set-sort", "ooxml xlsx filters-sorts clear-sort", "ooxml xlsx data-validations list", "ooxml xlsx data-validations show", "ooxml xlsx data-validations create", "ooxml xlsx data-validations update", "ooxml xlsx data-validations delete", "ooxml xlsx hyperlinks list", "ooxml xlsx hyperlinks show", "ooxml xlsx hyperlinks add", "ooxml xlsx hyperlinks update", "ooxml xlsx hyperlinks delete", "ooxml xlsx ranges export", "ooxml xlsx ranges set", "ooxml xlsx ranges set-format", "ooxml xlsx ranges set-style", "ooxml xlsx cells extract", "ooxml xlsx cells clear", "ooxml xlsx cells set-batch", "ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export", "ooxml xlsx tables append-rows", "ooxml xlsx tables append-records", "ooxml xlsx names list", "ooxml xlsx names show", "ooxml xlsx names add", "ooxml xlsx names update", "ooxml xlsx names rename", "ooxml xlsx names delete"],
            "data-validation": ["ooxml xlsx data-validations list", "ooxml xlsx data-validations show", "ooxml xlsx data-validations create", "ooxml xlsx data-validations update", "ooxml xlsx data-validations delete"],
            "cell": ["ooxml xlsx comments list", "ooxml xlsx comments add", "ooxml xlsx comments update", "ooxml xlsx comments remove", "ooxml xlsx hyperlinks list", "ooxml xlsx hyperlinks show", "ooxml xlsx hyperlinks add", "ooxml xlsx hyperlinks update", "ooxml xlsx hyperlinks delete", "ooxml xlsx ranges set", "ooxml xlsx cells set", "ooxml xlsx cells clear", "ooxml xlsx cells set-batch"],
            "hyperlink": ["ooxml xlsx hyperlinks list", "ooxml xlsx hyperlinks show", "ooxml xlsx hyperlinks add", "ooxml xlsx hyperlinks update", "ooxml xlsx hyperlinks delete"],
            "chart": ["ooxml pptx charts list", "ooxml pptx charts show", "ooxml pptx charts set-title", "ooxml pptx charts set-legend", "ooxml pptx charts set-chart-area-fill", "ooxml pptx charts set-plot-area-fill", "ooxml pptx charts set-series-style", "ooxml xlsx charts list", "ooxml xlsx charts set-title", "ooxml xlsx charts set-legend", "ooxml xlsx charts set-chart-area-fill", "ooxml xlsx charts set-plot-area-fill", "ooxml xlsx charts set-series-style"],
            "table": ["ooxml pptx tables show", "ooxml pptx tables set-cell", "ooxml pptx tables delete-row", "ooxml pptx tables insert-row", "ooxml pptx tables delete-col", "ooxml pptx tables insert-col", "ooxml pptx tables update-from-xlsx", "ooxml xlsx filters-sorts show", "ooxml xlsx filters-sorts set-autofilter", "ooxml xlsx filters-sorts clear-autofilter", "ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export", "ooxml xlsx tables append-rows", "ooxml xlsx tables append-records", "ooxml docx tables show", "ooxml docx tables set-cell", "ooxml docx tables clear-cell", "ooxml docx tables insert-row", "ooxml docx tables delete-row", "ooxml docx styles apply"],
            "name": ["ooxml xlsx names list", "ooxml xlsx names show", "ooxml xlsx names add", "ooxml xlsx names update", "ooxml xlsx names rename", "ooxml xlsx names delete"],
            "block": ["ooxml docx blocks", "ooxml docx blocks replace", "ooxml docx blocks delete", "ooxml docx blocks insert-after", "ooxml docx tables show"],
            "paragraph": ["ooxml docx text", "ooxml docx blocks", "ooxml docx blocks replace", "ooxml docx blocks delete", "ooxml docx blocks insert-after", "ooxml docx paragraphs append", "ooxml docx paragraphs insert", "ooxml docx paragraphs set", "ooxml docx paragraphs clear", "ooxml docx styles apply", "ooxml docx headers show", "ooxml docx headers set-text", "ooxml docx footers show", "ooxml docx footers set-text", "ooxml docx images list", "ooxml docx images insert"],
            "style": ["ooxml pptx charts set-title", "ooxml pptx charts set-legend", "ooxml pptx charts set-chart-area-fill", "ooxml pptx charts set-plot-area-fill", "ooxml pptx charts set-series-style", "ooxml xlsx charts set-title", "ooxml xlsx charts set-legend", "ooxml xlsx charts set-chart-area-fill", "ooxml xlsx charts set-plot-area-fill", "ooxml xlsx charts set-series-style", "ooxml xlsx ranges set-format", "ooxml xlsx ranges set-style", "ooxml docx styles list", "ooxml docx styles show", "ooxml docx styles apply"],
            "comment": ["ooxml pptx comments list", "ooxml pptx comments add", "ooxml pptx comments edit", "ooxml pptx comments remove", "ooxml xlsx comments list", "ooxml xlsx comments add", "ooxml xlsx comments update", "ooxml xlsx comments remove", "ooxml docx comments list", "ooxml docx comments add", "ooxml docx comments edit", "ooxml docx comments remove"],
            "field": ["ooxml docx fields list", "ooxml docx fields insert", "ooxml docx fields set-result"],
            "header": ["ooxml docx headers list", "ooxml docx headers show", "ooxml docx headers set-text", "ooxml docx footers list"],
            "footer": ["ooxml docx footers list", "ooxml docx footers show", "ooxml docx footers set-text", "ooxml docx headers list"],
            "image": ["ooxml pptx extract images", "ooxml pptx replace images", "ooxml docx images list", "ooxml docx images replace", "ooxml docx images insert"],
            "media": ["ooxml pptx media list", "ooxml pptx media add", "ooxml pptx media replace"],
            "module": ["ooxml vba create", "ooxml vba inspect", "ooxml vba extract-bin", "ooxml vba inspect-bin", "ooxml vba list", "ooxml vba extract", "ooxml vba office-check", "ooxml vba attach", "ooxml vba remove"]
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
