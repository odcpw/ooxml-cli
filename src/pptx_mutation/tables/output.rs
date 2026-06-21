use serde_json::{Map, Value, json};

use crate::{CliError, CliResult, command_arg, pptx_tables_show};

use super::types::{
    DeleteColMutation, DeleteRowMutation, InsertColMutation, InsertRowMutation, SetCellMutation,
    UpdateFromXlsxSource, UpdateMatrixMutation,
};

pub(super) fn read_table_destination(
    readback_path: &str,
    slide: u32,
    table_id: u32,
    output_path: Option<&str>,
) -> CliResult<Value> {
    let show = pptx_tables_show(readback_path, slide, table_id, None, false)?;
    let mut table = show
        .get("tables")
        .and_then(Value::as_array)
        .and_then(|tables| tables.first())
        .cloned()
        .ok_or_else(|| CliError::unexpected("updated table readback missing"))?;
    if let Some(output_path) = output_path
        && let Some(map) = table.as_object_mut()
    {
        map.insert("file".to_string(), json!(output_path));
    }
    Ok(table)
}

pub(super) fn set_cell_result_json(
    file: &str,
    slide: u32,
    row: usize,
    col: usize,
    mutation: &SetCellMutation,
    output_path: Option<&str>,
    destination: &mut Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("slide".to_string(), json!(slide));
    result.insert("tableId".to_string(), json!(mutation.resolved_table_id));
    result.insert("row".to_string(), json!(row));
    result.insert("col".to_string(), json!(col));
    result.insert("text".to_string(), json!(mutation.text));
    result.insert("previousText".to_string(), json!(mutation.previous_text));
    let destination_value = destination.take();
    add_pptx_table_readback_commands(&mut result, output_path, slide, &destination_value);
    result.insert("destination".to_string(), destination_value);
    Value::Object(result)
}

pub(super) fn delete_row_result_json(
    file: &str,
    slide: u32,
    row: usize,
    mutation: &DeleteRowMutation,
    output_path: Option<&str>,
    destination: &mut Value,
) -> Value {
    let rows = destination
        .get("rows")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let cols = destination
        .get("cols")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("slide".to_string(), json!(slide));
    result.insert("tableId".to_string(), json!(mutation.resolved_table_id));
    result.insert("row".to_string(), json!(row));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
    result.insert("cellCount".to_string(), json!(mutation.cell_count));
    let destination_value = destination.take();
    add_pptx_table_readback_commands(&mut result, output_path, slide, &destination_value);
    result.insert("destination".to_string(), destination_value);
    Value::Object(result)
}

pub(super) fn insert_row_result_json(
    file: &str,
    slide: u32,
    at: usize,
    mutation: &InsertRowMutation,
    output_path: Option<&str>,
    destination: &mut Value,
) -> Value {
    let rows = destination
        .get("rows")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let cols = destination
        .get("cols")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("slide".to_string(), json!(slide));
    result.insert("tableId".to_string(), json!(mutation.resolved_table_id));
    result.insert("at".to_string(), json!(at));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
    result.insert("cellCount".to_string(), json!(mutation.cell_count));
    let destination_value = destination.take();
    add_pptx_table_readback_commands(&mut result, output_path, slide, &destination_value);
    result.insert("destination".to_string(), destination_value);
    Value::Object(result)
}

pub(super) fn delete_col_result_json(
    file: &str,
    slide: u32,
    col: usize,
    mutation: &DeleteColMutation,
    output_path: Option<&str>,
    destination: &mut Value,
) -> Value {
    let rows = destination
        .get("rows")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let cols = destination
        .get("cols")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("slide".to_string(), json!(slide));
    result.insert("tableId".to_string(), json!(mutation.resolved_table_id));
    result.insert("col".to_string(), json!(col));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
    result.insert("rowCount".to_string(), json!(mutation.row_count));
    let destination_value = destination.take();
    add_pptx_table_readback_commands(&mut result, output_path, slide, &destination_value);
    result.insert("destination".to_string(), destination_value);
    Value::Object(result)
}

pub(super) fn insert_col_result_json(
    file: &str,
    slide: u32,
    at: usize,
    mutation: &InsertColMutation,
    output_path: Option<&str>,
    destination: &mut Value,
) -> Value {
    let rows = destination
        .get("rows")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let cols = destination
        .get("cols")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("slide".to_string(), json!(slide));
    result.insert("tableId".to_string(), json!(mutation.resolved_table_id));
    result.insert("at".to_string(), json!(at));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
    result.insert("rowCount".to_string(), json!(mutation.row_count));
    result.insert("widthEmu".to_string(), json!(mutation.width_emu));
    let destination_value = destination.take();
    add_pptx_table_readback_commands(&mut result, output_path, slide, &destination_value);
    result.insert("destination".to_string(), destination_value);
    Value::Object(result)
}

pub(super) fn update_from_xlsx_result_json(
    file: &str,
    formula_mode: &str,
    source: UpdateFromXlsxSource,
    mutation: &UpdateMatrixMutation,
    output_path: Option<&str>,
    destination: &mut Value,
    dry_run: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    if dry_run {
        result.insert("dryRun".to_string(), json!(true));
    }
    result.insert("source".to_string(), source.source);
    result.insert(
        "update".to_string(),
        json!({
            "formulaMode": formula_mode,
            "updatedCells": mutation.updated_cells,
            "changedCells": mutation.changed_cells,
        }),
    );
    let destination_value = destination.take();
    let slide = destination_value
        .get("slide")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or_default();
    add_pptx_table_readback_commands(&mut result, output_path, slide, &destination_value);
    result.insert("destination".to_string(), destination_value);
    Value::Object(result)
}

fn add_pptx_table_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    slide: u32,
    destination: &Value,
) {
    let command_target = output_path.unwrap_or("<out.pptx>");
    let target = destination
        .get("primarySelector")
        .and_then(Value::as_str)
        .unwrap_or("table:1");
    let command_suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    result.insert(
        format!("readbackCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx tables show {} --slide {} --target {}",
            command_arg(command_target),
            slide,
            command_arg(target)
        )),
    );
    result.insert(
        format!("slideReadbackCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(command_target),
            slide
        )),
    );
    result.insert(
        format!("validateCommand{command_suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    result.insert(
        format!("renderCommand{command_suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
}
