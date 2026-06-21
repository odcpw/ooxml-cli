use serde_json::Value;

use super::super::{capability_command, flag};

pub(super) fn commands() -> Vec<Value> {
    vec![capability_command(
        "ooxml pptx diff",
        "diff <baseline> <candidate>",
        "Compare two PPTX presentations",
        &[],
        false,
        Some("read-only package comparison command; not a serve/MCP mutation op"),
        vec![
            flag(
                "--render",
                "render",
                "bool",
                "enable visual diff via rendered slide images",
            ),
            flag("--threshold", "threshold", "float", "visual diff threshold"),
            flag(
                "--out",
                "out",
                "string",
                "output directory for visual diff artifacts",
            ),
        ],
    )]
}
