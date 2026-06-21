use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml xlsx freeze show",
            "show <file>",
            "Display current worksheet freeze panes state.",
            &["sheet"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![flag("--sheet", "sheet", "string", "sheet selector")],
        ),
        capability_command(
            "ooxml xlsx freeze set",
            "set <file>",
            "Set frozen rows and/or columns on a worksheet.",
            &["sheet"],
            true,
            None,
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--rows", "rows", "number", "number of top rows to freeze"),
                flag(
                    "--cols",
                    "cols",
                    "number",
                    "number of left columns to freeze",
                ),
                flag(
                    "--expect-state",
                    "expectState",
                    "string",
                    "guard: require current state to be none or frozen",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
        ),
        capability_command(
            "ooxml xlsx freeze clear",
            "clear <file>",
            "Remove frozen panes from a worksheet.",
            &["sheet"],
            true,
            None,
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--expect-state",
                    "expectState",
                    "string",
                    "guard: require current state to be none or frozen",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
        ),
    ]
}
