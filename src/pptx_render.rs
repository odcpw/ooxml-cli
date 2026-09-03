use serde_json::Value;

#[path = "render.rs"]
pub(crate) mod shared;

use crate::CliResult;
use shared::render_command_for_family;

pub(crate) fn pptx_render(file: &str, args: &[String]) -> CliResult<Value> {
    render_command_for_family(file, args, Some("pptx"))
}
