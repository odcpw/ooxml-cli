use serde_json::{Value, json};

use super::super::super::op::{ServeOp, push_serve_plan_string_flag};
use crate::command_manifest::DocxCommandId;
use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, docx_tables_clear_cell,
    docx_tables_delete_row, docx_tables_insert_row, docx_tables_set_cell, json_i64,
    json_optional_string, require_docx_block_hash, resolve_required_docx_table_text,
    validate_positive_i64,
};

pub(super) fn serve_docx_tables_op(
    working: &str,
    command_id: DocxCommandId,
    command: &str,
    args: &Value,
) -> CliResult<ServeOp> {
    let op = match command_id {
        DocxCommandId::TablesSetCell => {
            let table = json_i64(args, "table")?
                .ok_or_else(|| CliError::invalid_args("table is required"))?;
            let row =
                json_i64(args, "row")?.ok_or_else(|| CliError::invalid_args("row is required"))?;
            let col =
                json_i64(args, "col")?.ok_or_else(|| CliError::invalid_args("col is required"))?;
            validate_positive_i64(table, "--table")?;
            validate_positive_i64(row, "--row")?;
            validate_positive_i64(col, "--col")?;
            let expect_hash = json_optional_string(args, "expect-hash")
                .or_else(|| json_optional_string(args, "expectHash"))
                .unwrap_or_default();
            require_docx_block_hash(&expect_hash)?;
            let text_changed = args.get("text").is_some();
            let text_file_changed =
                args.get("text-file").is_some() || args.get("textFile").is_some();
            let text = json_optional_string(args, "text");
            let text_file = json_optional_string(args, "text-file")
                .or_else(|| json_optional_string(args, "textFile"));
            let resolved_text = resolve_required_docx_table_text(
                text.as_deref(),
                text_file.as_deref(),
                text_changed,
                text_file_changed,
            )?;
            let readback = docx_tables_set_cell(
                working,
                table as usize,
                row as usize,
                col as usize,
                &expect_hash,
                &resolved_text,
                DocxParagraphMutationOptions {
                    text: None,
                    text_file: None,
                    style: "",
                    out: None,
                    backup: None,
                    dry_run: false,
                    in_place: true,
                    no_validate: true,
                },
            )?;
            let mut plan_flags = vec![
                json!("--table"),
                json!(table.to_string()),
                json!("--row"),
                json!(row.to_string()),
                json!("--col"),
                json!(col.to_string()),
            ];
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-hash",
                Some(expect_hash.as_str()),
            );
            if text_changed {
                push_serve_plan_string_flag(
                    &mut plan_flags,
                    "--text",
                    Some(resolved_text.as_str()),
                );
            }
            if text_file_changed {
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
            }
            ServeOp::DocxTablesOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        DocxCommandId::TablesClearCell => {
            let table = json_i64(args, "table")?
                .ok_or_else(|| CliError::invalid_args("table is required"))?;
            let row =
                json_i64(args, "row")?.ok_or_else(|| CliError::invalid_args("row is required"))?;
            let col =
                json_i64(args, "col")?.ok_or_else(|| CliError::invalid_args("col is required"))?;
            validate_positive_i64(table, "--table")?;
            validate_positive_i64(row, "--row")?;
            validate_positive_i64(col, "--col")?;
            let expect_hash = json_optional_string(args, "expect-hash")
                .or_else(|| json_optional_string(args, "expectHash"))
                .unwrap_or_default();
            require_docx_block_hash(&expect_hash)?;
            let readback = docx_tables_clear_cell(
                working,
                table as usize,
                row as usize,
                col as usize,
                &expect_hash,
                DocxParagraphMutationOptions {
                    text: None,
                    text_file: None,
                    style: "",
                    out: None,
                    backup: None,
                    dry_run: false,
                    in_place: true,
                    no_validate: true,
                },
            )?;
            let mut plan_flags = vec![
                json!("--table"),
                json!(table.to_string()),
                json!("--row"),
                json!(row.to_string()),
                json!("--col"),
                json!(col.to_string()),
            ];
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-hash",
                Some(expect_hash.as_str()),
            );
            ServeOp::DocxTablesOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        DocxCommandId::TablesInsertRow => {
            let table = json_i64(args, "table")?
                .ok_or_else(|| CliError::invalid_args("table is required"))?;
            let at =
                json_i64(args, "at")?.ok_or_else(|| CliError::invalid_args("at is required"))?;
            validate_positive_i64(table, "--table")?;
            validate_positive_i64(at, "--at")?;
            let expect_hash = json_optional_string(args, "expect-hash")
                .or_else(|| json_optional_string(args, "expectHash"))
                .unwrap_or_default();
            require_docx_block_hash(&expect_hash)?;
            let readback = docx_tables_insert_row(
                working,
                table as usize,
                at as usize,
                &expect_hash,
                DocxParagraphMutationOptions {
                    text: None,
                    text_file: None,
                    style: "",
                    out: None,
                    backup: None,
                    dry_run: false,
                    in_place: true,
                    no_validate: true,
                },
            )?;
            let mut plan_flags = vec![
                json!("--table"),
                json!(table.to_string()),
                json!("--at"),
                json!(at.to_string()),
            ];
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-hash",
                Some(expect_hash.as_str()),
            );
            ServeOp::DocxTablesOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        DocxCommandId::TablesDeleteRow => {
            let table = json_i64(args, "table")?
                .ok_or_else(|| CliError::invalid_args("table is required"))?;
            let row =
                json_i64(args, "row")?.ok_or_else(|| CliError::invalid_args("row is required"))?;
            validate_positive_i64(table, "--table")?;
            validate_positive_i64(row, "--row")?;
            let expect_hash = json_optional_string(args, "expect-hash")
                .or_else(|| json_optional_string(args, "expectHash"))
                .unwrap_or_default();
            require_docx_block_hash(&expect_hash)?;
            let readback = docx_tables_delete_row(
                working,
                table as usize,
                row as usize,
                &expect_hash,
                DocxParagraphMutationOptions {
                    text: None,
                    text_file: None,
                    style: "",
                    out: None,
                    backup: None,
                    dry_run: false,
                    in_place: true,
                    no_validate: true,
                },
            )?;
            let mut plan_flags = vec![
                json!("--table"),
                json!(table.to_string()),
                json!("--row"),
                json!(row.to_string()),
            ];
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--expect-hash",
                Some(expect_hash.as_str()),
            );
            ServeOp::DocxTablesOp {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
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
