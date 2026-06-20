use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml docx paragraphs append",
            "append <file>",
            "Append a main document body paragraph, preserving trailing section properties.",
            &["paragraph"],
            true,
            None,
            vec![
                flag("--text", "text", "string", "paragraph text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to paragraph text",
                ),
                flag("--style", "style", "string", "optional paragraph style ID"),
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
            "ooxml docx paragraphs insert",
            "insert <file>",
            "Insert a main document body paragraph after a body block index.",
            &["paragraph"],
            true,
            None,
            vec![
                flag(
                    "--insert-after",
                    "insertAfter",
                    "int",
                    "0 to prepend, or a 1-based body block index",
                ),
                flag("--text", "text", "string", "paragraph text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to paragraph text",
                ),
                flag("--style", "style", "string", "optional paragraph style ID"),
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
            "ooxml docx paragraphs set",
            "set <file>",
            "Replace one main document body paragraph's plain text.",
            &["paragraph"],
            true,
            None,
            vec![
                flag("--index", "index", "int", "1-based body block index"),
                flag(
                    "--handle",
                    "handle",
                    "string",
                    "stable DOCX paragraph handle",
                ),
                flag("--text", "text", "string", "replacement paragraph text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to replacement paragraph text",
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
            "ooxml docx paragraphs clear",
            "clear <file>",
            "Clear one main document body paragraph's text while retaining paragraph metadata.",
            &["paragraph"],
            true,
            None,
            vec![
                flag("--index", "index", "int", "1-based body block index"),
                flag(
                    "--handle",
                    "handle",
                    "string",
                    "stable DOCX paragraph handle",
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
