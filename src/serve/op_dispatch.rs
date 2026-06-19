mod docx;
mod xlsx;

use serde_json::Value;

use super::op::ServeOp;
use crate::{CliError, CliResult, json_string, json_u32, pptx_replace_text_in_place};

pub(super) fn serve_op_command(working: &str, command: &str, args: &Value) -> CliResult<ServeOp> {
    let op = match command {
        "xlsx cells set"
        | "xlsx ranges set"
        | "xlsx ranges set-format"
        | "xlsx workbook metadata update" => xlsx::serve_xlsx_op(working, command, args)?,
        "docx headers set-text"
        | "docx footers set-text"
        | "docx fields insert"
        | "docx fields set-result"
        | "docx paragraphs append"
        | "docx paragraphs insert"
        | "docx paragraphs set"
        | "docx paragraphs clear"
        | "docx styles apply"
        | "docx blocks replace"
        | "docx blocks delete"
        | "docx blocks insert-after"
        | "docx comments add"
        | "docx comments edit"
        | "docx comments remove"
        | "docx tables set-cell"
        | "docx tables clear-cell" => docx::serve_docx_op(working, command, args)?,
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
