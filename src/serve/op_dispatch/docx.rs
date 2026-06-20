use serde_json::Value;

mod blocks;
mod comments;
mod fields;
mod headers_footers;
mod paragraphs;
mod styles;
mod tables;

use super::super::op::ServeOp;
use crate::{CliError, CliResult};

pub(super) fn serve_docx_op(working: &str, command: &str, args: &Value) -> CliResult<ServeOp> {
    let op = match command {
        family_command
            if family_command.starts_with("docx headers ")
                || family_command.starts_with("docx footers ") =>
        {
            headers_footers::serve_docx_headers_footers_op(working, family_command, args)?
        }
        family_command if family_command.starts_with("docx fields ") => {
            fields::serve_docx_fields_op(working, family_command, args)?
        }
        family_command if family_command.starts_with("docx paragraphs ") => {
            paragraphs::serve_docx_paragraphs_op(working, family_command, args)?
        }
        family_command if family_command.starts_with("docx styles ") => {
            styles::serve_docx_styles_op(working, family_command, args)?
        }
        family_command if family_command.starts_with("docx blocks ") => {
            blocks::serve_docx_blocks_op(working, family_command, args)?
        }
        family_command if family_command.starts_with("docx comments ") => {
            comments::serve_docx_comments_op(working, family_command, args)?
        }
        family_command if family_command.starts_with("docx tables ") => {
            tables::serve_docx_tables_op(working, family_command, args)?
        }
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}
