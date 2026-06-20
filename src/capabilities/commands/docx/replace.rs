use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![capability_command(
        "ooxml docx replace",
        "replace <file>",
        "Find and replace text across DOCX body text.",
        &["paragraph", "table"],
        true,
        None,
        vec![
            flag("--backup", "backup", "string", "backup path for --in-place"),
            flag("--dry-run", "dryRun", "bool", "validate without writing"),
            flag(
                "--expect-count",
                "expectCount",
                "int",
                "expected number of replacements; when set, errors if the actual count differs",
            ),
            flag(
                "--find",
                "find",
                "string",
                "text or regex pattern to find (required)",
            ),
            flag("--in-place", "inPlace", "bool", "write in place"),
            flag(
                "--match-case",
                "matchCase",
                "bool",
                "case-sensitive matching",
            ),
            flag(
                "--no-validate",
                "noValidate",
                "bool",
                "skip post-write validation",
            ),
            flag("--out", "out", "string", "output file path"),
            flag(
                "--regex",
                "regex",
                "bool",
                "treat --find as a regular expression",
            ),
            flag(
                "--replace",
                "replace",
                "string",
                "replacement text (inserted literally)",
            ),
            flag(
                "--whole-word",
                "wholeWord",
                "bool",
                "match whole words only",
            ),
        ],
    )]
}
