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
            Some("read-only command; call via direct CLI in the current Rust slice"),
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
            Some("read-only command; call via direct CLI in the current Rust slice"),
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
            "add <file> --range <sqref> --formula <formula>",
            "Add an expression conditional-formatting rule.",
            &["conditional-format", "range", "sheet"],
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
                "conditional-formatting rule type: expression",
            ),
            flag("--formula", "formula", "string", "expression formula"),
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
