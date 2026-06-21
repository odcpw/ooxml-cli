use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml xlsx conditional-formats list",
            "list <file> [--sheet <sheet>] [--range <sqref>]",
            "List worksheet conditional-formatting rules.",
            &["conditional-format", "range", "sheet"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "optional sqref filter"),
            ],
        ),
        capability_command(
            "ooxml xlsx conditional-formats show",
            "show <file> --rule <selector> [--sheet <sheet>]",
            "Show one worksheet conditional-formatting rule.",
            &["conditional-format", "range", "sheet"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--rule",
                    "rule",
                    "string",
                    "rule selector such as cfRule:1, rule:1, block:1/rule:1, priority:1, or sqref:A1:A5",
                ),
            ],
        ),
        capability_command(
            "ooxml xlsx conditional-formats add",
            "add <file> --range <sqref> [--type expression|cell-is|color-scale|data-bar|icon-set]",
            "Add an expression, cellIs, color-scale, data-bar, or icon-set conditional-formatting rule.",
            &["conditional-format", "range", "sheet", "style"],
            true,
            None,
            mutation_flags(true),
        ),
        capability_command(
            "ooxml xlsx conditional-formats delete",
            "delete <file> --rule <selector>",
            "Delete a worksheet conditional-formatting rule.",
            &["conditional-format", "range", "sheet"],
            true,
            None,
            mutation_flags(false),
        ),
    ]
}

fn mutation_flags(include_add_flags: bool) -> Vec<Value> {
    let mut flags = vec![flag("--sheet", "sheet", "string", "sheet selector")];
    if include_add_flags {
        flags.extend([
            flag(
                "--range",
                "range",
                "string",
                "target sqref; space-separated ranges are accepted",
            ),
            flag(
                "--type",
                "type",
                "string",
                "conditional-formatting rule type: expression, cell-is, color-scale, data-bar, or icon-set",
            ),
            flag(
                "--operator",
                "operator",
                "string",
                "cellIs operator: between, notBetween, equal, notEqual, greaterThan, lessThan, greaterThanOrEqual, or lessThanOrEqual",
            ),
            flag(
                "--formula",
                "formula",
                "string",
                "expression formula or first cellIs formula/bound",
            ),
            flag(
                "--formula2",
                "formula2",
                "string",
                "second cellIs formula/bound for between/notBetween",
            ),
            flag(
                "--cfvo",
                "cfvo",
                "string",
                "threshold value: repeat 2 or 3 times for color-scale, exactly 2 times for data-bar, or 3/4/5 times for icon-set based on --icon-set; examples: min, max, num:0, percent:10, or percentile:50",
            ),
            flag(
                "--color",
                "color",
                "string",
                "color hex: repeat 2 or 3 times for color-scale, exactly once for data-bar, and never for icon-set; examples: #F8696B, FFEB84, or FF63BE7B",
            ),
            flag(
                "--icon-set",
                "iconSet",
                "string",
                "icon-set name for --type icon-set; examples: 3TrafficLights1, 4Arrows, or 5Rating",
            ),
            flag("--priority", "priority", "int", "optional cfRule priority"),
            flag(
                "--stop-if-true",
                "stopIfTrue",
                "bool",
                "set stopIfTrue on the rule",
            ),
            flag(
                "--dxf-id",
                "dxfId",
                "int",
                "optional differential style id to reference",
            ),
        ]);
    } else {
        flags.push(flag(
            "--rule",
            "rule",
            "string",
            "rule selector such as cfRule:1, rule:1, block:1/rule:1, priority:1, or sqref:A1:A5",
        ));
    }
    flags.extend([
        flag("--out", "out", "string", "write edited workbook"),
        flag(
            "--in-place",
            "inPlace",
            "bool",
            "edit the workbook in place",
        ),
        flag(
            "--backup",
            "backup",
            "string",
            "backup file path for --in-place",
        ),
        flag("--dry-run", "dryRun", "bool", "validate without writing"),
        flag(
            "--no-validate",
            "noValidate",
            "bool",
            "skip post-write validation",
        ),
    ]);
    flags
}
