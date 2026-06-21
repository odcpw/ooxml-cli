use serde_json::{Map, Value, json};

use crate::{WorkbookSheet, command_arg};

use super::{XlsxCommentInfo, XlsxCommentsSheet};

pub(super) fn mutation_base_result(
    file: &str,
    context: &XlsxCommentsSheet,
    comment: &XlsxCommentInfo,
    output: Option<&str>,
    dry_run: bool,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(context.sheet.name));
    result.insert("sheetNumber".to_string(), json!(context.sheet.position));
    result.insert("commentId".to_string(), json!(comment.id));
    let handle = xlsx_comment_handle(&context.sheet, &context.sheets, &comment.anchored_to_cell);
    if !handle.is_empty() {
        result.insert("handle".to_string(), json!(handle.clone()));
    }
    result.insert(
        "primarySelector".to_string(),
        json!(xlsx_comment_primary_selector(&handle, comment.id)),
    );
    result.insert(
        "selectors".to_string(),
        json!(xlsx_comment_selectors(
            &handle,
            comment.id,
            &comment.anchored_to_cell
        )),
    );
    if let Some(output) = output {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result
}

pub(super) fn comment_json(
    comment: &XlsxCommentInfo,
    sheet: &WorkbookSheet,
    sheets: &[WorkbookSheet],
) -> Value {
    let handle = xlsx_comment_handle(sheet, sheets, &comment.anchored_to_cell);
    let mut item = Map::new();
    item.insert("id".to_string(), json!(comment.id));
    item.insert("author".to_string(), json!(comment.author));
    item.insert("text".to_string(), json!(comment.text));
    item.insert("contentHash".to_string(), json!(comment.content_hash));
    item.insert(
        "anchoredToCell".to_string(),
        json!(comment.anchored_to_cell),
    );
    if let Some(row) = comment.anchored_to_cell_row {
        item.insert("anchoredToCellRow".to_string(), json!(row));
    }
    if let Some(col) = comment.anchored_to_cell_column {
        item.insert("anchoredToCellColumn".to_string(), json!(col));
    }
    if !handle.is_empty() {
        item.insert("handle".to_string(), json!(handle.clone()));
    }
    item.insert(
        "primarySelector".to_string(),
        json!(xlsx_comment_primary_selector(&handle, comment.id)),
    );
    item.insert(
        "selectors".to_string(),
        json!(xlsx_comment_selectors(
            &handle,
            comment.id,
            &comment.anchored_to_cell
        )),
    );
    Value::Object(item)
}

fn xlsx_comment_handle(sheet: &WorkbookSheet, sheets: &[WorkbookSheet], cell: &str) -> String {
    if cell.trim().is_empty() {
        return String::new();
    }
    let count = sheets
        .iter()
        .filter(|candidate| candidate.sheet_id == sheet.sheet_id)
        .count();
    if count != 1 {
        return String::new();
    }
    format!("H:xlsx/ws:{}/comment:a:{cell}", sheet.sheet_id)
}

fn xlsx_comment_primary_selector(handle: &str, comment_id: i64) -> String {
    if !handle.trim().is_empty() {
        handle.to_string()
    } else if comment_id >= 0 {
        comment_id.to_string()
    } else {
        String::new()
    }
}

fn xlsx_comment_selectors(handle: &str, comment_id: i64, cell: &str) -> Vec<String> {
    let mut selectors = Vec::new();
    if !handle.trim().is_empty() {
        selectors.push(handle.to_string());
    }
    if comment_id >= 0 {
        selectors.push(comment_id.to_string());
    }
    if !cell.trim().is_empty() {
        selectors.push(cell.to_string());
    }
    selectors
}

pub(super) fn xlsx_comments_list_command(file: &str, sheet: &WorkbookSheet) -> String {
    format!(
        "ooxml --json xlsx comments list {} --sheet {}",
        command_arg(file),
        command_arg(&format!("sheetId:{}", sheet.sheet_id))
    )
}

pub(super) fn add_comment_readback_commands(
    result: &mut Map<String, Value>,
    output: Option<&str>,
    sheet: &WorkbookSheet,
) {
    if let Some(output) = output {
        result.insert(
            "validateCommand".to_string(),
            json!(format!("ooxml validate --strict {}", command_arg(output))),
        );
        result.insert(
            "listCommand".to_string(),
            json!(xlsx_comments_list_command(output, sheet)),
        );
    }
}
