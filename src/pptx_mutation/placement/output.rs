use serde_json::{Map, Value, json};

use crate::pptx_readback::{pptx_shapes_get, pptx_tables_show};
use crate::{CliError, CliResult, command_arg};

use super::{
    ImageMutation, ImageRequest, PlacementMutationOptions, TableFromXlsxRequest, TableMutation,
    TableRequest, TextboxMutation,
};

pub(super) fn read_shape_destination(
    readback_path: &str,
    slide: u32,
    shape_id: u32,
    output_path: Option<&str>,
    include_text: bool,
) -> CliResult<Value> {
    let target = format!("shape:{shape_id}");
    let get = pptx_shapes_get(readback_path, slide, &target, include_text, true)?;
    let entry = get
        .get("shapes")
        .and_then(Value::as_array)
        .and_then(|shapes| shapes.first())
        .ok_or_else(|| CliError::unexpected("shape readback missing destination"))?;
    Ok(shape_destination_from_entry(
        entry,
        slide,
        output_path,
        &target,
    ))
}

fn shape_destination_from_entry(
    entry: &Value,
    slide: u32,
    file: Option<&str>,
    target: &str,
) -> Value {
    let mut out = Map::new();
    if let Some(file) = file {
        out.insert("file".to_string(), json!(file));
    }
    out.insert("slide".to_string(), json!(slide));
    out.insert("target".to_string(), json!(target));
    copy_json_field(entry, &mut out, "shapeId");
    copy_json_field(entry, &mut out, "shapeName");
    copy_json_field(entry, &mut out, "targetKind");
    copy_json_field(entry, &mut out, "primarySelector");
    copy_json_field(entry, &mut out, "handle");
    copy_json_field(entry, &mut out, "selectors");
    copy_json_field(entry, &mut out, "textPreview");
    copy_json_field(entry, &mut out, "bounds");
    copy_json_field(entry, &mut out, "imageRef");
    Value::Object(out)
}

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

pub(super) fn table_from_xlsx_destination(
    table: &Value,
    request: &TableRequest,
    mutation: &TableMutation,
    output_path: Option<&str>,
) -> Value {
    let mut out = Map::new();
    if let Some(output_path) = output_path {
        out.insert("file".to_string(), json!(output_path));
    }
    out.insert("slide".to_string(), json!(request.slide));
    out.insert("shapeId".to_string(), json!(mutation.shape_id));
    out.insert("shapeName".to_string(), json!(mutation.shape_name));
    copy_json_field(table, &mut out, "primarySelector");
    copy_json_field(table, &mut out, "selectors");
    out.insert("rows".to_string(), json!(mutation.rows));
    out.insert("cols".to_string(), json!(mutation.cols));
    copy_json_field(table, &mut out, "cells");
    out.insert("x".to_string(), json!(request.bounds.x));
    out.insert("y".to_string(), json!(request.bounds.y));
    out.insert("cx".to_string(), json!(mutation.width));
    out.insert("cy".to_string(), json!(mutation.height));
    Value::Object(out)
}

fn copy_json_field(source: &Value, dest: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        dest.insert(key.to_string(), value.clone());
    }
}

pub(super) fn add_textbox_result_json(
    file: &str,
    mutation: &TextboxMutation,
    options: &PlacementMutationOptions,
    output_path: Option<&str>,
    destination: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    result.insert("createdAt".to_string(), json!(crate::current_utc_rfc3339()));
    result.insert("destination".to_string(), destination);
    add_shape_readback_commands(
        &mut result,
        output_path,
        options.dry_run,
        mutation.slide,
        mutation.shape_id,
        true,
    );
    Value::Object(result)
}

pub(super) fn place_table_result_json(mutation: &TableMutation) -> Value {
    json!({
        "shapeId": mutation.shape_id,
        "shapeName": mutation.shape_name,
        "width": mutation.width,
        "height": mutation.height,
        "rows": mutation.rows,
        "cols": mutation.cols,
    })
}

pub(super) fn place_table_from_xlsx_result_json(
    file: &str,
    request: &TableFromXlsxRequest,
    options: &PlacementMutationOptions,
    output_path: Option<&str>,
    destination: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    if options.dry_run {
        result.insert("dryRun".to_string(), json!(true));
    }
    result.insert("source".to_string(), request.source.source.clone());
    add_table_readback_commands(&mut result, output_path, request.table.slide, &destination);
    result.insert("destination".to_string(), destination);
    Value::Object(result)
}

pub(super) fn place_image_result_json(
    file: &str,
    mutation: &ImageMutation,
    request: &ImageRequest,
    options: &PlacementMutationOptions,
    output_path: Option<&str>,
    destination: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("slideNumber".to_string(), json!(mutation.slide));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    result.insert("targetUri".to_string(), json!(mutation.target_uri));
    result.insert("contentType".to_string(), json!(mutation.content_type));
    result.insert(
        "relationshipId".to_string(),
        json!(mutation.relationship_id),
    );
    result.insert("x".to_string(), json!(request.bounds.x));
    result.insert("y".to_string(), json!(request.bounds.y));
    result.insert("cx".to_string(), json!(request.bounds.cx));
    result.insert("cy".to_string(), json!(request.bounds.cy));
    result.insert("fitMode".to_string(), json!(mutation.fit_mode));
    result.insert("destination".to_string(), destination);
    add_shape_readback_commands(
        &mut result,
        output_path,
        options.dry_run,
        mutation.slide,
        mutation.shape_id,
        false,
    );
    Value::Object(result)
}

fn add_table_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    slide: u32,
    destination: &Value,
) {
    let target_file = output_path.unwrap_or("<out.pptx>");
    let target = destination
        .get("primarySelector")
        .and_then(Value::as_str)
        .unwrap_or("table:1");
    let suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    result.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx tables show {} --slide {} --target {}",
            command_arg(target_file),
            slide,
            command_arg(target)
        )),
    );
    result.insert(
        format!("slideReadbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(target_file),
            slide
        )),
    );
    result.insert(
        format!("validateCommand{suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(target_file)
        )),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(target_file)
        )),
    );
}

fn add_shape_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    dry_run: bool,
    slide: u32,
    shape_id: u32,
    include_text: bool,
) {
    let target_file = output_path.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    let mut readback = format!(
        "ooxml --json pptx shapes get {} --slide {slide} --target shape:{shape_id}",
        command_arg(target_file)
    );
    if include_text {
        readback.push_str(" --include-text");
    }
    readback.push_str(" --include-bounds");
    result.insert(format!("readbackCommand{suffix}"), json!(readback));
    result.insert(
        format!("slideReadbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {slide} --include-text --include-bounds",
            command_arg(target_file)
        )),
    );
    result.insert(
        format!("validateCommand{suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(target_file)
        )),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(target_file)
        )),
    );
}
