use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml docx styles list",
            "list <file>",
            "List DOCX paragraph, character, table, and numbering styles.",
            &["style"],
            false,
            Some("read-only command; generated style handles can be used by mutation commands"),
            vec![flag(
                "--type",
                "type",
                "string",
                "filter by style type: paragraph, character, table, or numbering",
            )],
        ),
        capability_command(
            "ooxml docx styles show",
            "show <file>",
            "Show detailed info for one DOCX style by styleId.",
            &["style"],
            false,
            Some("read-only command; generated style handles can be used by mutation commands"),
            vec![flag("--style", "style", "string", "styleId to show")],
        ),
        capability_command(
            "ooxml docx styles apply",
            "apply <file>",
            "Apply a paragraph, run, or table style to DOCX body content.",
            &["style", "paragraph", "table"],
            true,
            None,
            vec![
                flag(
                    "--index",
                    "index",
                    "int",
                    "1-based body block index for paragraph/run, or 1-based table number for table",
                ),
                flag(
                    "--handle",
                    "handle",
                    "string",
                    "stable DOCX paragraph handle for paragraph/run targets",
                ),
                flag(
                    "--target",
                    "target",
                    "string",
                    "style target: paragraph, run, or table",
                ),
                flag(
                    "--style",
                    "style",
                    "string",
                    "styleId or H:docx/pt:styles/style:n:<styleId> handle",
                ),
                flag(
                    "--expect-hash",
                    "expectHash",
                    "string",
                    "optional sha256 block hash guard from docx blocks",
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
                    "skip style existence/type validation and post-write validation",
                ),
            ],
        ),
    ]
}
