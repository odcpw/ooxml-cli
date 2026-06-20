use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml docx fields list",
            "list <file>",
            "List all simple/complex fields in document body + headers/footers.",
            &["field"],
            false,
            Some(
                "read-only command; cached field results are stale until Word recalculates fields",
            ),
            vec![flag(
                "--type",
                "type",
                "string",
                "show only fields whose leading instruction keyword matches",
            )],
        ),
        capability_command(
            "ooxml docx fields insert",
            "insert <file>",
            "Insert a simple DOCX field into a body, header, or footer paragraph.",
            &["field", "paragraph"],
            true,
            None,
            vec![
                flag(
                    "--location",
                    "location",
                    "string",
                    "target part:block location, e.g. body:2 or header1:1",
                ),
                flag(
                    "--field-code",
                    "fieldCode",
                    "string",
                    "field instruction, e.g. PAGE",
                ),
                flag("--result", "result", "string", "initial cached result text"),
                flag("--out", "out", "string", "output file path"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
            ],
        ),
        capability_command(
            "ooxml docx fields set-result",
            "set-result <file>",
            "Set the cached result text of a simple or complex DOCX field.",
            &["field", "paragraph"],
            true,
            None,
            vec![
                flag(
                    "--selector",
                    "selector",
                    "string",
                    "field selector part:block:field, e.g. body:1:0",
                ),
                flag("--result", "result", "string", "new cached result text"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256 of instruction plus cached result",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag("--in-place", "inPlace", "bool", "write in place"),
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
