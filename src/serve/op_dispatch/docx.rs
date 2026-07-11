use serde_json::Value;

mod blocks;
mod comments;
mod fields;
mod headers_footers;
mod paragraphs;
mod styles;
mod tables;

use super::super::op::ServeOp;
use crate::command_manifest::DocxCommandId;
use crate::{CliError, CliResult};

pub(super) fn serve_docx_op(
    working: &str,
    command_id: DocxCommandId,
    command: &str,
    args: &Value,
) -> CliResult<ServeOp> {
    let op = match command_id {
        DocxCommandId::HeadersSetText | DocxCommandId::FootersSetText => {
            headers_footers::serve_docx_headers_footers_op(working, command_id, command, args)?
        }
        DocxCommandId::FieldsInsert | DocxCommandId::FieldsSetResult => {
            fields::serve_docx_fields_op(working, command_id, command, args)?
        }
        DocxCommandId::ParagraphsAppend
        | DocxCommandId::ParagraphsInsert
        | DocxCommandId::ParagraphsSet
        | DocxCommandId::ParagraphsClear => {
            paragraphs::serve_docx_paragraphs_op(working, command_id, command, args)?
        }
        DocxCommandId::StylesApply => {
            styles::serve_docx_styles_op(working, command_id, command, args)?
        }
        DocxCommandId::BlocksReplace
        | DocxCommandId::BlocksDelete
        | DocxCommandId::BlocksInsertAfter => {
            blocks::serve_docx_blocks_op(working, command_id, command, args)?
        }
        DocxCommandId::CommentsAdd
        | DocxCommandId::CommentsEdit
        | DocxCommandId::CommentsRemove => {
            comments::serve_docx_comments_op(working, command_id, command, args)?
        }
        DocxCommandId::TablesSetCell
        | DocxCommandId::TablesClearCell
        | DocxCommandId::TablesInsertRow
        | DocxCommandId::TablesDeleteRow => {
            tables::serve_docx_tables_op(working, command_id, command, args)?
        }
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}
