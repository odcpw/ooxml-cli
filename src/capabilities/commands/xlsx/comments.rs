use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml xlsx comments list",
            "list <file> [--sheet <sheet>] [--comment-id <id>]",
            "List worksheet comments, authors, selectors, hashes, and anchored cells.",
            &["comment", "sheet", "cell"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag(
                    "--comment-id",
                    "commentId",
                    "int",
                    "only return the comment with this zero-based id",
                ),
            ],
        ),
        capability_command(
            "ooxml xlsx comments add",
            "add <file> --cell <A1> --author <name> [--text <text>]",
            "Add a worksheet cell comment, creating comments and legacy VML drawing parts when needed.",
            &["comment", "sheet", "cell"],
            true,
            None,
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--cell", "cell", "string", "target cell such as C3"),
                flag("--author", "author", "string", "comment author"),
                flag("--text", "text", "string", "comment text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "read comment text from file",
                ),
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
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip strict validation",
                ),
            ],
        ),
        capability_command(
            "ooxml xlsx comments update",
            "update <file> (--handle <handle>|--comment-id <id>)",
            "Update a worksheet comment's text and/or author with optional hash guard.",
            &["comment", "sheet", "cell"],
            true,
            None,
            vec![
                flag(
                    "--sheet",
                    "sheet",
                    "string",
                    "sheet selector used with --comment-id",
                ),
                flag(
                    "--comment-id",
                    "commentId",
                    "int",
                    "zero-based comment id on the selected sheet",
                ),
                flag("--handle", "handle", "string", "published comment handle"),
                flag("--text", "text", "string", "replacement comment text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "read replacement text from file",
                ),
                flag(
                    "--author",
                    "author",
                    "string",
                    "replacement/additional author",
                ),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "guard: expected current comment content hash",
                ),
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
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip strict validation",
                ),
            ],
        ),
        capability_command(
            "ooxml xlsx comments remove",
            "remove <file> (--handle <handle>|--comment-id <id>)",
            "Remove a worksheet comment, cleaning orphaned comments/VML parts when the sheet has no comments left.",
            &["comment", "sheet", "cell"],
            true,
            None,
            vec![
                flag(
                    "--sheet",
                    "sheet",
                    "string",
                    "sheet selector used with --comment-id",
                ),
                flag(
                    "--comment-id",
                    "commentId",
                    "int",
                    "zero-based comment id on the selected sheet",
                ),
                flag("--handle", "handle", "string", "published comment handle"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "guard: expected current comment content hash",
                ),
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
                flag(
                    "--dry-run",
                    "dryRun",
                    "bool",
                    "validate mutation without writing",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip strict validation",
                ),
            ],
        ),
    ]
}
