use serde_json::{Map, Value, json};

use super::slide_parts::pptx_slide_part_refs;
use super::{
    Shape, TableCell, TableInfo, bounds_json, pptx_selector_targets_from_shapes, pptx_shape_models,
};
use crate::{CliError, CliResult, add_selector, package_type, zip_text};
pub(crate) fn pptx_tables_show(
    file: &str,
    slide: u32,
    table_id: u32,
    target: Option<&str>,
    include_details: bool,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let slides = pptx_slide_part_refs(file)?;
    if slide == 0 || slide as usize > slides.len() {
        return Err(CliError::invalid_args(format!(
            "slide number {slide} out of range (1-{})",
            slides.len()
        )));
    }
    let slide_ref = &slides[slide as usize - 1];
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let shapes = pptx_shape_models(&slide_xml);
    let targets = pptx_selector_targets_from_shapes(&shapes);
    let resolved_table_id = pptx_resolve_table_target(&shapes, &targets, target)?;
    let wanted_table_id = if table_id > 0 {
        Some(table_id)
    } else {
        resolved_table_id
    };
    let tables = pptx_table_summaries(slide, &shapes, &targets, wanted_table_id, include_details);
    if let Some(wanted_table_id) = wanted_table_id
        && tables.is_empty()
    {
        return Err(CliError::target_not_found(format!(
            "target not found: table shape ID {wanted_table_id} on slide {slide}"
        )));
    }
    Ok(json!({
        "file": file,
        "slide": slide,
        "tables": tables,
    }))
}

fn pptx_resolve_table_target(
    shapes: &[Shape],
    targets: &[Value],
    target: Option<&str>,
) -> CliResult<Option<u32>> {
    let target = target.map(str::trim).unwrap_or_default();
    if target.is_empty() || target == "@all-tables" {
        return Ok(None);
    }
    for (shape, target_value) in shapes.iter().zip(targets) {
        let primary = target_value
            .get("primarySelector")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let selectors = target_value
            .get("selectors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str);
        if primary == target || selectors.clone().any(|selector| selector == target) {
            if shape.kind == "graphicFrame" && shape.table.is_some() {
                return Ok(Some(shape.id));
            }
            return Err(CliError::invalid_args(format!(
                "target {target:?} resolves to {primary}, not a table"
            )));
        }
    }
    Err(CliError::target_not_found(format!(
        "target not found: target not found: {target} (available selectors: {})",
        pptx_available_shape_selectors(targets).join(", ")
    )))
}

fn pptx_available_shape_selectors(targets: &[Value]) -> Vec<String> {
    let mut selectors = Vec::new();
    add_selector(&mut selectors, "@all-shapes".to_string());
    add_selector(&mut selectors, "@all-shapes-nonph".to_string());
    add_selector(&mut selectors, "@all-tables".to_string());
    for target in targets {
        if let Some(items) = target.get("selectors").and_then(Value::as_array) {
            for item in items {
                if let Some(selector) = item.as_str() {
                    add_selector(&mut selectors, selector.to_string());
                }
            }
        }
    }
    selectors
}

fn pptx_table_summaries(
    slide: u32,
    shapes: &[Shape],
    targets: &[Value],
    table_id: Option<u32>,
    include_details: bool,
) -> Vec<Value> {
    shapes
        .iter()
        .zip(targets)
        .filter(|(shape, _target)| shape.kind == "graphicFrame" && shape.table.is_some())
        .filter(|(shape, _target)| table_id.is_none_or(|table_id| shape.id == table_id))
        .map(|(shape, target)| pptx_table_summary(slide, shape, target, include_details))
        .collect()
}

fn pptx_table_summary(slide: u32, shape: &Shape, target: &Value, include_details: bool) -> Value {
    let table = shape.table.as_ref().expect("table summary requires table");
    let cells = table
        .rows
        .iter()
        .map(|row| {
            Value::Array(
                row.cells
                    .iter()
                    .map(|cell| Value::String(cell.text.clone()))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let mut summary = Map::new();
    summary.insert("slide".to_string(), json!(slide));
    summary.insert("shapeId".to_string(), json!(shape.id));
    summary.insert("shapeName".to_string(), json!(shape.name));
    summary.insert(
        "targetKind".to_string(),
        target
            .get("targetKind")
            .cloned()
            .unwrap_or_else(|| json!("table")),
    );
    summary.insert(
        "primarySelector".to_string(),
        target
            .get("primarySelector")
            .cloned()
            .unwrap_or_else(|| json!(format!("shape:{}", shape.id))),
    );
    summary.insert(
        "selectors".to_string(),
        target
            .get("selectors")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    summary.insert("rows".to_string(), json!(table.rows.len()));
    summary.insert("cols".to_string(), json!(table_column_count(table)));
    summary.insert("cells".to_string(), Value::Array(cells));
    if let Some(bounds) = shape.bounds.as_ref() {
        summary.insert("bounds".to_string(), bounds_json(bounds));
    }
    if include_details {
        summary.insert("tableInfo".to_string(), table_info_json(table));
    }
    Value::Object(summary)
}

pub(super) fn table_info_json(table: &TableInfo) -> Value {
    let cells = table
        .rows
        .iter()
        .map(|row| {
            Value::Array(
                row.cells
                    .iter()
                    .map(|cell| Value::String(cell.text.clone()))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let row_defs = table
        .rows
        .iter()
        .map(|row| {
            let mut row_def = Map::new();
            if let Some(height) = row.height {
                row_def.insert("height".to_string(), json!(height));
            }
            row_def.insert("cells".to_string(), table_cells_json(&row.cells));
            Value::Object(row_def)
        })
        .collect::<Vec<_>>();
    let column_defs = table
        .columns
        .iter()
        .map(|width| json!({"width": width}))
        .collect::<Vec<_>>();
    let cell_defs = table
        .rows
        .iter()
        .map(|row| table_cells_json(&row.cells))
        .collect::<Vec<_>>();
    json!({
        "rows": table.rows.len(),
        "cols": table_column_count(table),
        "cells": cells,
        "rowDefs": row_defs,
        "columnDefs": column_defs,
        "cellDefs": cell_defs,
    })
}

fn table_cells_json(cells: &[TableCell]) -> Value {
    Value::Array(
        cells
            .iter()
            .map(|cell| {
                json!({
                    "text": cell.text.clone(),
                    "gridSpan": cell.grid_span,
                    "rowSpan": cell.row_span,
                })
            })
            .collect(),
    )
}

fn table_column_count(table: &TableInfo) -> usize {
    table.columns.len().max(
        table
            .rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(0),
    )
}
