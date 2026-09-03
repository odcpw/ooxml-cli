use serde_json::Value;

use crate::{CliResult, OutlineOptions, json_optional_string, json_u32, outline};

pub(super) fn serve_outline(working: &str, args: &Value) -> CliResult<Value> {
    let sheet = json_optional_string(args, "sheet");
    outline(
        working,
        OutlineOptions {
            depth: json_u32(args, "depth")?.unwrap_or(3),
            text_preview: json_u32(args, "text-preview")?
                .or(json_u32(args, "textPreview")?)
                .unwrap_or(80) as usize,
            slide: json_u32(args, "slide")?,
            sheet: sheet.as_deref(),
            section: json_u32(args, "section")?,
        },
    )
}
