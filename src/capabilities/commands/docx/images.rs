use serde_json::Value;

use super::super::capability_command;

pub(super) fn commands() -> Vec<Value> {
    vec![capability_command(
        "ooxml docx images list",
        "list <file>",
        "List inline images in a DOCX document.",
        &["image", "paragraph"],
        false,
        Some(
            "read-only command; image records include relationship ids, media parts, dimensions, and block anchors",
        ),
        vec![],
    )]
}
