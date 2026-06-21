use serde_json::{Map, Value, json};
use std::path::Path;

use crate::command_arg;

pub(super) fn add_layout_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    layout_selector: &str,
) {
    let target = output_path.unwrap_or("<out.pptx>");
    let suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    result.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx layouts show {} --layout {}",
            command_arg(target),
            command_arg(layout_selector)
        )),
    );
    result.insert(
        format!("layoutsListCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx layouts list {}",
            command_arg(target)
        )),
    );
    result.insert(
        format!("validateCommand{suffix}"),
        json!(format!("ooxml validate --strict {}", command_arg(target))),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(target)
        )),
    );
}

pub(super) fn add_master_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    master: usize,
) {
    let target = output_path.unwrap_or("<out.pptx>");
    let suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    result.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx masters show {} --master {master}",
            command_arg(target)
        )),
    );
    result.insert(
        format!("mastersListCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx masters list {}",
            command_arg(target)
        )),
    );
    result.insert(
        format!("validateCommand{suffix}"),
        json!(format!("ooxml validate --strict {}", command_arg(target))),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(target)
        )),
    );
}

pub(super) fn output_basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}
