use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml docx text",
            "text <file>",
            "Extract DOCX paragraph text.",
            &["package"],
            false,
            Some("read-only command"),
            vec![],
        ),
        capability_command(
            "ooxml docx blocks",
            "blocks <file>",
            "Show stable DOCX body blocks with hashes, selectors, paragraph metadata, table cells, and optional runs.",
            &[],
            false,
            Some("read-only command; block hashes and selectors feed hash-guarded DOCX mutations"),
            vec![
                flag(
                    "--block",
                    "block",
                    "int",
                    "1-based body block index to show",
                ),
                flag(
                    "--include-runs",
                    "includeRuns",
                    "bool",
                    "include paragraph run text and basic run properties",
                ),
            ],
        ),
        capability_command(
            "ooxml docx blocks replace",
            "replace <file>",
            "Replace a hash-guarded DOCX body block with a paragraph.",
            &["paragraph"],
            true,
            None,
            vec![
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--block",
                    "block",
                    "int",
                    "1-based body block index from docx blocks",
                ),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: content hash from docx blocks",
                ),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--style",
                    "style",
                    "string",
                    "optional paragraph style ID; default preserves paragraph style when replacing a paragraph",
                ),
                flag("--text", "text", "string", "replacement paragraph text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to replacement paragraph text",
                ),
            ],
        ),
        capability_command(
            "ooxml docx blocks delete",
            "delete <file>",
            "Delete a hash-guarded DOCX body block.",
            &["paragraph", "table"],
            true,
            None,
            vec![
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--block",
                    "block",
                    "int",
                    "1-based body block index from docx blocks",
                ),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: content hash from docx blocks",
                ),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
                flag("--out", "out", "string", "output file path"),
            ],
        ),
        capability_command(
            "ooxml docx blocks insert-after",
            "insert-after <file>",
            "Insert a paragraph after a hash-guarded DOCX body block.",
            &["paragraph"],
            true,
            None,
            vec![
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag(
                    "--block",
                    "block",
                    "int",
                    "1-based body block index from docx blocks; 0 inserts before the first block",
                ),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "expected sha256: content hash from docx blocks when --block is greater than 0",
                ),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
                flag("--out", "out", "string", "output file path"),
                flag("--style", "style", "string", "optional paragraph style ID"),
                flag("--text", "text", "string", "paragraph text"),
                flag(
                    "--text-file",
                    "textFile",
                    "string",
                    "path to paragraph text",
                ),
            ],
        ),
    ]
}
