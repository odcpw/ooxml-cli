use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml docx tables show",
            "show <file>",
            "Show DOCX tables by table index, body block index, dimensions, merged-cell flag, and cell text.",
            &[],
            false,
            Some(
                "read-only command; call via inspect in serve/MCP; generated table hashes feed hash-guarded DOCX table mutations",
            ),
            vec![
                flag(
                    "--details",
                    "details",
                    "bool",
                    "include detailed table object in JSON output",
                ),
                flag(
                    "--table",
                    "table",
                    "int",
                    "1-based table number; omitted shows all tables",
                ),
            ],
        ),
        capability_command(
            "ooxml docx tables set-cell",
            "set-cell <file>",
            "Set one main-document DOCX table cell's plain text.",
            &["table"],
            true,
            None,
            vec![
                flag("--table", "table", "int", "1-based table number"),
                flag("--row", "row", "int", "1-based table row"),
                flag("--col", "col", "int", "1-based table column"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: table block hash from docx tables show or docx blocks",
                ),
                flag("--text", "text", "string", "replacement cell text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to replacement cell text",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
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
            "ooxml docx tables clear-cell",
            "clear-cell <file>",
            "Clear one main-document DOCX table cell's text.",
            &["table"],
            true,
            None,
            vec![
                flag("--table", "table", "int", "1-based table number"),
                flag("--row", "row", "int", "1-based table row"),
                flag("--col", "col", "int", "1-based table column"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: table block hash from docx tables show or docx blocks",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
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
