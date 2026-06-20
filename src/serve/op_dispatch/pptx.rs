use serde_json::{Value, json};

use super::super::op::ServeOp;
use crate::{
    CliError, CliResult, json_i64,
    pptx_mutation::{
        pptx_notes_clear, pptx_notes_set, pptx_tables_delete_row, pptx_tables_insert_row,
        pptx_tables_set_cell,
    },
};

pub(super) fn serve_pptx_op(working: &str, command: &str, args: &Value) -> CliResult<ServeOp> {
    let op = match command {
        "pptx tables set-cell" => {
            let slide = required_i64(args, "slide")?;
            let row = required_i64(args, "row")?;
            let col = required_i64(args, "col")?;
            let mut plan_args = pptx_table_target_args(args, slide)?;
            push_cli_flag(&mut plan_args, "--row", &row.to_string());
            push_cli_flag(&mut plan_args, "--col", &col.to_string());

            let (text, text_present) = optional_string(args, "text")?;
            let (text_file, text_file_present) =
                optional_string_alias(args, "text-file", "textFile")?;
            if text_present {
                push_cli_flag(
                    &mut plan_args,
                    "--text",
                    text.as_deref().unwrap_or_default(),
                );
            }
            if text_file_present {
                push_cli_flag(
                    &mut plan_args,
                    "--text-file",
                    text_file.as_deref().unwrap_or_default(),
                );
            }

            finish_pptx_tables_op(working, command, plan_args, pptx_tables_set_cell)?
        }
        "pptx tables delete-row" => {
            let slide = required_i64(args, "slide")?;
            let row = required_i64(args, "row")?;
            let mut plan_args = pptx_table_target_args(args, slide)?;
            push_cli_flag(&mut plan_args, "--row", &row.to_string());

            finish_pptx_tables_op(working, command, plan_args, pptx_tables_delete_row)?
        }
        "pptx tables insert-row" => {
            let slide = required_i64(args, "slide")?;
            let at = required_i64(args, "at")?;
            let mut plan_args = pptx_table_target_args(args, slide)?;
            push_cli_flag(&mut plan_args, "--at", &at.to_string());

            finish_pptx_tables_op(working, command, plan_args, pptx_tables_insert_row)?
        }
        "pptx notes set" => {
            let slide = required_i64(args, "slide")?;
            let (text, text_present) = optional_string(args, "text")?;
            if !text_present {
                return Err(CliError::invalid_args("text is required"));
            }
            let mut plan_args = Vec::new();
            push_cli_flag(&mut plan_args, "--slide", &slide.to_string());
            push_cli_flag(
                &mut plan_args,
                "--text",
                text.as_deref().unwrap_or_default(),
            );

            finish_pptx_notes_op(working, command, plan_args, pptx_notes_set)?
        }
        "pptx notes clear" => {
            let slide = required_i64(args, "slide")?;
            let mut plan_args = Vec::new();
            push_cli_flag(&mut plan_args, "--slide", &slide.to_string());

            finish_pptx_notes_op(working, command, plan_args, pptx_notes_clear)?
        }
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}

fn finish_pptx_tables_op(
    working: &str,
    command: &str,
    plan_args: Vec<String>,
    run: fn(&str, &[String]) -> CliResult<Value>,
) -> CliResult<ServeOp> {
    let mut mutation_args = plan_args.clone();
    mutation_args.push("--in-place".to_string());
    mutation_args.push("--no-validate".to_string());
    let readback = run(working, &mutation_args)?;
    Ok(ServeOp::PptxTablesOp {
        command: command.to_string(),
        plan_flags: plan_args.into_iter().map(|arg| json!(arg)).collect(),
        readback_file: working.to_string(),
        readback,
    })
}

fn finish_pptx_notes_op(
    working: &str,
    command: &str,
    plan_args: Vec<String>,
    run: fn(&str, &[String]) -> CliResult<Value>,
) -> CliResult<ServeOp> {
    let mut mutation_args = plan_args.clone();
    mutation_args.push("--in-place".to_string());
    mutation_args.push("--no-validate".to_string());
    let readback = run(working, &mutation_args)?;
    Ok(ServeOp::PptxNotesOp {
        command: command.to_string(),
        plan_flags: plan_args.into_iter().map(|arg| json!(arg)).collect(),
        readback_file: working.to_string(),
        readback,
    })
}

fn pptx_table_target_args(args: &Value, slide: i64) -> CliResult<Vec<String>> {
    let mut plan_args = Vec::new();
    push_cli_flag(&mut plan_args, "--slide", &slide.to_string());

    if let Some(table_id) = optional_i64_alias(args, "table-id", "tableId")? {
        push_cli_flag(&mut plan_args, "--table-id", &table_id.to_string());
    }
    if let Some(target) = optional_string(args, "target")?.0 {
        push_cli_flag(&mut plan_args, "--target", &target);
    }

    Ok(plan_args)
}

fn required_i64(args: &Value, key: &str) -> CliResult<i64> {
    json_i64(args, key)?.ok_or_else(|| CliError::invalid_args(format!("{key} is required")))
}

fn optional_i64_alias(args: &Value, key: &str, alias: &str) -> CliResult<Option<i64>> {
    match json_i64(args, key)? {
        Some(value) => Ok(Some(value)),
        None => json_i64(args, alias),
    }
}

fn optional_string(args: &Value, key: &str) -> CliResult<(Option<String>, bool)> {
    let Some(value) = args.get(key) else {
        return Ok((None, false));
    };
    let text = value
        .as_str()
        .ok_or_else(|| CliError::invalid_args(format!("{key} must be a string")))?;
    Ok((Some(text.to_string()), true))
}

fn optional_string_alias(
    args: &Value,
    key: &str,
    alias: &str,
) -> CliResult<(Option<String>, bool)> {
    let (value, present) = optional_string(args, key)?;
    if present {
        return Ok((value, true));
    }
    optional_string(args, alias)
}

fn push_cli_flag(args: &mut Vec<String>, name: &str, value: &str) {
    args.push(name.to_string());
    args.push(value.to_string());
}
