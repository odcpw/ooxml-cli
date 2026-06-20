mod docx;
mod xlsx;

use serde_json::Value;

use super::op::ServeOp;
use crate::{CliError, CliResult, json_string, json_u32, pptx_replace_text_in_place};

pub(super) fn serve_op_command(working: &str, command: &str, args: &Value) -> CliResult<ServeOp> {
    let op = match command {
        family_command if family_command.starts_with("xlsx ") => {
            xlsx::serve_xlsx_op(working, family_command, args)?
        }
        family_command if family_command.starts_with("docx ") => {
            docx::serve_docx_op(working, family_command, args)?
        }
        "pptx replace text" => {
            let slide = json_u32(args, "slide")?.unwrap_or(1);
            let target = json_string(args, "target")?;
            let text = json_string(args, "text")?;
            pptx_replace_text_in_place(working, slide, &target, &text)?;
            ServeOp::PptxReplaceText {
                command: command.to_string(),
                slide,
                target,
                text,
            }
        }
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}
