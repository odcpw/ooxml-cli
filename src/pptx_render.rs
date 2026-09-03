use serde_json::Value;

use crate::CliResult;
use crate::render::render_command_for_family;

pub(crate) fn pptx_render(file: &str, args: &[String]) -> CliResult<Value> {
    render_command_for_family(file, args, Some("pptx"))
}
