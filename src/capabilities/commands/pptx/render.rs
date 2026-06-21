use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![capability_command(
        "ooxml pptx render",
        "render <file>",
        "Render a PPTX to PDF/thumbnails when local tools are installed.",
        &["slide"],
        false,
        Some("render command is not a mutation op"),
        vec![
            flag("--out", "out", "string", "render output directory"),
            flag("--slides", "slides", "string", "comma-separated slide list"),
            flag("--format", "format", "string", "json"),
        ],
    )]
}
