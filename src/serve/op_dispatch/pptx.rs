use serde_json::{Value, json};

use super::super::op::ServeOp;
use crate::{
    CliError, CliResult, json_bool, json_i64,
    pptx_mutation::{
        pptx_notes_clear, pptx_notes_set, pptx_replace_text_occurrences, pptx_shapes_delete,
        pptx_tables_delete_col, pptx_tables_delete_row, pptx_tables_insert_col,
        pptx_tables_insert_row, pptx_tables_set_cell, pptx_tables_update_from_xlsx,
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
        "pptx tables delete-col" => {
            let slide = required_i64(args, "slide")?;
            let col = required_i64(args, "col")?;
            let mut plan_args = pptx_table_target_args(args, slide)?;
            push_cli_flag(&mut plan_args, "--col", &col.to_string());

            finish_pptx_tables_op(working, command, plan_args, pptx_tables_delete_col)?
        }
        "pptx tables insert-col" => {
            let slide = required_i64(args, "slide")?;
            let at = required_i64(args, "at")?;
            let mut plan_args = pptx_table_target_args(args, slide)?;
            push_cli_flag(&mut plan_args, "--at", &at.to_string());
            if let Some(width) = optional_i64_alias(args, "width-emu", "widthEmu")? {
                push_cli_flag(&mut plan_args, "--width-emu", &width.to_string());
            }

            finish_pptx_tables_op(working, command, plan_args, pptx_tables_insert_col)?
        }
        "pptx tables update-from-xlsx" => {
            let slide = required_i64(args, "slide")?;
            let workbook = required_string(args, "workbook")?;
            let mut plan_args = pptx_table_target_args(args, slide)?;
            push_cli_flag(&mut plan_args, "--workbook", &workbook);
            for (json_key, flag_name) in [
                ("sheet", "--sheet"),
                ("range", "--range"),
                ("table", "--table"),
                ("formula-mode", "--formula-mode"),
                ("expect-source-range", "--expect-source-range"),
            ] {
                if let Some(value) = optional_string(args, json_key)?.0 {
                    push_cli_flag(&mut plan_args, flag_name, &value);
                }
            }
            for (json_key, alias, flag_name) in [
                ("formulaMode", "formula-mode", "--formula-mode"),
                (
                    "expectSourceRange",
                    "expect-source-range",
                    "--expect-source-range",
                ),
            ] {
                if optional_string(args, alias)?.1 {
                    continue;
                }
                if let Some(value) = optional_string(args, json_key)?.0 {
                    push_cli_flag(&mut plan_args, flag_name, &value);
                }
            }
            if let Some(max_cells) = optional_i64_alias(args, "max-cells", "maxCells")? {
                push_cli_flag(&mut plan_args, "--max-cells", &max_cells.to_string());
            }

            finish_pptx_tables_op(working, command, plan_args, pptx_tables_update_from_xlsx)?
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
        "pptx shapes delete" => {
            let slide = required_i64(args, "slide")?;
            let target = required_string(args, "target")?;
            let mut plan_args = Vec::new();
            push_cli_flag(&mut plan_args, "--slide", &slide.to_string());
            push_cli_flag(&mut plan_args, "--target", &target);

            finish_pptx_shapes_op(working, command, plan_args, pptx_shapes_delete)?
        }
        "pptx replace text-occurrences" => {
            let match_text = required_string_alias(args, "match-text", "matchText")?;
            let new_text = required_string_alias(args, "new-text", "newText")?;
            let mut plan_args = Vec::new();
            push_cli_flag(&mut plan_args, "--match-text", &match_text);
            push_cli_flag(&mut plan_args, "--new-text", &new_text);
            for (key, alias, flag) in [
                ("for-slides", "forSlides", "--for-slides"),
                ("for-shape", "forShape", "--for-shape"),
                ("expect-count", "expectCount", "--expect-count"),
                ("expect-plan-hash", "expectPlanHash", "--expect-plan-hash"),
            ] {
                if let Some(value) = optional_string_alias(args, key, alias)?.0 {
                    push_cli_flag(&mut plan_args, flag, &value);
                }
            }
            for (key, alias, flag) in [
                ("ignore-case", "ignoreCase", "--ignore-case"),
                ("allow-zero", "allowZero", "--allow-zero"),
            ] {
                if optional_bool_alias(args, key, alias) {
                    plan_args.push(flag.to_string());
                }
            }

            finish_pptx_text_occurrences_op(working, command, plan_args)?
        }
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}

fn finish_pptx_text_occurrences_op(
    working: &str,
    command: &str,
    plan_args: Vec<String>,
) -> CliResult<ServeOp> {
    let mut mutation_args = plan_args.clone();
    mutation_args.push("--in-place".to_string());
    mutation_args.push("--no-validate".to_string());
    let readback = pptx_replace_text_occurrences(working, &mutation_args)?;
    Ok(ServeOp::PptxReplaceOp {
        command: command.to_string(),
        plan_flags: plan_args.into_iter().map(|arg| json!(arg)).collect(),
        readback_file: working.to_string(),
        readback,
    })
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

fn finish_pptx_shapes_op(
    working: &str,
    command: &str,
    plan_args: Vec<String>,
    run: fn(&str, &[String]) -> CliResult<Value>,
) -> CliResult<ServeOp> {
    let mut mutation_args = plan_args.clone();
    mutation_args.push("--in-place".to_string());
    mutation_args.push("--no-validate".to_string());
    let readback = run(working, &mutation_args)?;
    Ok(ServeOp::PptxShapesOp {
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

fn required_string(args: &Value, key: &str) -> CliResult<String> {
    optional_string(args, key)?
        .0
        .ok_or_else(|| CliError::invalid_args(format!("{key} is required")))
}

fn required_string_alias(args: &Value, key: &str, alias: &str) -> CliResult<String> {
    optional_string_alias(args, key, alias)?
        .0
        .ok_or_else(|| CliError::invalid_args(format!("{key} is required")))
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

fn optional_bool_alias(args: &Value, key: &str, alias: &str) -> bool {
    json_bool(args, key)
        .or_else(|| json_bool(args, alias))
        .unwrap_or(false)
}

fn push_cli_flag(args: &mut Vec<String>, name: &str, value: &str) {
    args.push(name.to_string());
    args.push(value.to_string());
}
