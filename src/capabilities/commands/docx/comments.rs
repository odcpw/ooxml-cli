use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml docx comments list",
            "list <file>",
            "List DOCX comments with stable selectors, hashes, and anchor blocks.",
            &["comment"],
            false,
            Some("read-only command; generated comment handles can be used by mutation commands"),
            vec![flag(
                "--comment-id",
                "commentId",
                "int",
                "show only the comment with this numeric w:id",
            )],
        ),
        capability_command(
            "ooxml docx comments add",
            "add <file>",
            "Add a DOCX comment anchored to a body paragraph.",
            &["comment"],
            true,
            None,
            vec![
                flag(
                    "--anchor-block",
                    "anchorBlock",
                    "int",
                    "1-based body block index to anchor to (default: first block)",
                ),
                flag("--author", "author", "string", "comment author name"),
                flag(
                    "--initials",
                    "initials",
                    "string",
                    "optional comment author initials",
                ),
                flag(
                    "--date",
                    "date",
                    "string",
                    "RFC3339 timestamp (default: now)",
                ),
                flag("--text", "text", "string", "comment text"),
                flag("--text-file", "textFile", "string", "path to comment text"),
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
            "ooxml docx comments edit",
            "edit <file>",
            "Edit an existing DOCX comment by id or stable handle.",
            &["comment"],
            true,
            None,
            vec![
                flag(
                    "--comment-id",
                    "commentId",
                    "int",
                    "comment id from comments list",
                ),
                flag("--handle", "handle", "string", "stable DOCX comment handle"),
                flag("--text", "text", "string", "new comment text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to new comment text",
                ),
                flag("--author", "author", "string", "new author"),
                flag("--date", "date", "string", "new RFC3339 timestamp"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256 content hash from comments list",
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
            "ooxml docx comments remove",
            "remove <file>",
            "Remove an existing DOCX comment and its range/reference markers.",
            &["comment"],
            true,
            None,
            vec![
                flag(
                    "--comment-id",
                    "commentId",
                    "int",
                    "comment id from comments list",
                ),
                flag("--handle", "handle", "string", "stable DOCX comment handle"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256 content hash from comments list",
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
