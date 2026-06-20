use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml docx headers list",
            "list <file>",
            "List headers and footers defined per section.",
            &["header", "footer"],
            false,
            Some(
                "read-only command; generated header/footer selectors can be pasted into show or set-text",
            ),
            vec![],
        ),
        capability_command(
            "ooxml docx headers show",
            "show <file>",
            "Show header content by type, section, or relationship id.",
            &["header", "paragraph"],
            false,
            Some("read-only command; accepts selectors from docx headers list"),
            vec![
                flag(
                    "--id",
                    "id",
                    "string",
                    "relationship id to resolve directly",
                ),
                flag(
                    "--section",
                    "section",
                    "int",
                    "1-based section index; 0 means the last section",
                ),
                flag(
                    "--selector",
                    "selector",
                    "string",
                    "selector from headers/footers list",
                ),
                flag("--type", "type", "string", "default, first, or even"),
            ],
        ),
        capability_command(
            "ooxml docx headers set-text",
            "set-text <file>",
            "Set header paragraph text by index.",
            &["header", "paragraph"],
            true,
            None,
            docx_header_footer_set_text_flags(),
        ),
        capability_command(
            "ooxml docx footers list",
            "list <file>",
            "List headers and footers defined per section.",
            &["footer", "header"],
            false,
            Some(
                "read-only command; generated header/footer selectors can be pasted into show or set-text",
            ),
            vec![],
        ),
        capability_command(
            "ooxml docx footers show",
            "show <file>",
            "Show footer content by type, section, or relationship id.",
            &["footer", "paragraph"],
            false,
            Some("read-only command; accepts selectors from docx footers list"),
            vec![
                flag(
                    "--id",
                    "id",
                    "string",
                    "relationship id to resolve directly",
                ),
                flag(
                    "--section",
                    "section",
                    "int",
                    "1-based section index; 0 means the last section",
                ),
                flag(
                    "--selector",
                    "selector",
                    "string",
                    "selector from headers/footers list",
                ),
                flag("--type", "type", "string", "default, first, or even"),
            ],
        ),
        capability_command(
            "ooxml docx footers set-text",
            "set-text <file>",
            "Set footer paragraph text by index.",
            &["footer", "paragraph"],
            true,
            None,
            docx_header_footer_set_text_flags(),
        ),
    ]
}
fn docx_header_footer_set_text_flags() -> Vec<Value> {
    vec![
        flag(
            "--id",
            "id",
            "string",
            "relationship id to resolve directly",
        ),
        flag("--type", "type", "string", "default, first, or even"),
        flag(
            "--section",
            "section",
            "int",
            "1-based section index; 0 means the last section",
        ),
        flag(
            "--index",
            "index",
            "int",
            "1-based paragraph index within the part",
        ),
        flag(
            "--selector",
            "selector",
            "string",
            "selector from headers/footers list",
        ),
        flag("--text", "text", "string", "replacement text"),
        flag(
            "--text-file",
            "textFile",
            "string",
            "path to replacement text",
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
    ]
}
