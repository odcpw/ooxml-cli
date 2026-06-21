use serde_json::{Map, Value, json};

use super::{
    CloneSlideMutation, DeleteSlideMutation, MoveSlideMutation, NewSlideFromLayoutMutation,
    ReorderSlidesMutation,
};
use crate::{CliError, CliResult, command_arg, pptx_slides_list};

pub(super) fn moved_slide_destination(
    readback_file: &str,
    slide: i64,
    output_path: Option<&str>,
) -> CliResult<Value> {
    clone_slide_destination(readback_file, slide, output_path)
}

pub(super) fn clone_slide_destination(
    readback_file: &str,
    slide: i64,
    file_field: Option<&str>,
) -> CliResult<Value> {
    let list = pptx_slides_list(readback_file)?;
    let item = list
        .get("slides")
        .and_then(Value::as_array)
        .and_then(|slides| slides.get(slide as usize - 1))
        .ok_or_else(|| CliError::unexpected(format!("slide {slide} readback not found")))?;
    let mut out = Map::new();
    if let Some(file_field) = file_field.filter(|value| !value.is_empty()) {
        out.insert("file".to_string(), json!(file_field));
    }
    copy_json_field(item, &mut out, "number");
    copy_json_field(item, &mut out, "partUri");
    copy_non_empty_string_field(item, &mut out, "layout");
    copy_non_empty_string_field(item, &mut out, "layoutPartUri");
    copy_non_empty_string_field(item, &mut out, "notesPartUri");
    copy_json_field(item, &mut out, "textShapes");
    copy_json_field(item, &mut out, "images");
    copy_json_field(item, &mut out, "tables");
    copy_json_field(item, &mut out, "notes");
    Ok(Value::Object(out))
}

fn copy_json_field(source: &Value, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        target.insert(key.to_string(), value.clone());
    }
}

fn copy_non_empty_string_field(source: &Value, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).and_then(Value::as_str)
        && !value.is_empty()
    {
        target.insert(key.to_string(), json!(value));
    }
}

pub(super) fn delete_result_json(
    file: &str,
    mutation: &DeleteSlideMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("deletedSlide".to_string(), json!(mutation.deleted_slide));
    result.insert("removedUri".to_string(), json!(mutation.removed_uri));
    if let Some(removed_notes) = mutation.removed_notes.as_deref() {
        result.insert("removedNotes".to_string(), json!(removed_notes));
    }
    result.insert(
        "remainingSlides".to_string(),
        json!(mutation.remaining_slides),
    );
    add_pptx_slides_mutation_commands(&mut result, output_path);
    Value::Object(result)
}

pub(super) fn clone_slide_result_json(
    file: &str,
    mutation: &CloneSlideMutation,
    output_path: Option<&str>,
    dry_run: bool,
    source: Value,
    destination: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("sourceSlide".to_string(), json!(mutation.source_slide));
    result.insert("insertAfter".to_string(), json!(mutation.insert_after));
    result.insert(
        "slideCountBefore".to_string(),
        json!(mutation.slide_count_before),
    );
    result.insert(
        "slideCountAfter".to_string(),
        json!(mutation.slide_count_after),
    );
    result.insert(
        "newSlideNumber".to_string(),
        json!(mutation.new_slide_number),
    );
    result.insert("newSlideId".to_string(), json!(mutation.new_slide_id));
    result.insert("newSlideUri".to_string(), json!(mutation.new_slide_uri));
    if !mutation.notes_uri.is_empty() {
        result.insert("notesUri".to_string(), json!(mutation.notes_uri));
    }
    result.insert("source".to_string(), source);
    result.insert("destination".to_string(), destination);
    if let Some(output_path) = output_path {
        result.insert(
            "readbackCommand".to_string(),
            json!(slide_readback_command(
                output_path,
                mutation.new_slide_number
            )),
        );
        result.insert(
            "slidesListCommand".to_string(),
            json!(slides_list_command(output_path)),
        );
        result.insert(
            "validateCommand".to_string(),
            json!(validate_command(output_path)),
        );
        result.insert(
            "renderCommand".to_string(),
            json!(render_command(output_path)),
        );
    } else {
        result.insert(
            "readbackCommandTemplate".to_string(),
            json!(slide_readback_command(
                "<out.pptx>",
                mutation.new_slide_number
            )),
        );
        result.insert(
            "slidesListCommandTemplate".to_string(),
            json!(slides_list_command("<out.pptx>")),
        );
    }
    Value::Object(result)
}

pub(super) fn new_slide_from_layout_result_json(
    file: &str,
    mutation: &NewSlideFromLayoutMutation,
    output_path: Option<&str>,
    dry_run: bool,
    destination: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("layout".to_string(), json!(mutation.layout));
    if mutation.requested_insert_after > 0 {
        result.insert(
            "insertAfter".to_string(),
            json!(mutation.requested_insert_after),
        );
    }
    result.insert(
        "newSlideNumber".to_string(),
        json!(mutation.new_slide_number),
    );
    result.insert("newSlideId".to_string(), json!(mutation.new_slide_id));
    result.insert("newSlideUri".to_string(), json!(mutation.new_slide_uri));
    result.insert("destination".to_string(), destination);
    if let Some(output_path) = output_path {
        result.insert(
            "readbackCommand".to_string(),
            json!(slide_readback_command(
                output_path,
                mutation.new_slide_number
            )),
        );
        result.insert(
            "slidesListCommand".to_string(),
            json!(slides_list_command(output_path)),
        );
        result.insert(
            "validateCommand".to_string(),
            json!(validate_command(output_path)),
        );
        result.insert(
            "renderCommand".to_string(),
            json!(render_command(output_path)),
        );
    } else {
        result.insert(
            "readbackCommandTemplate".to_string(),
            json!(slide_readback_command(
                "<out.pptx>",
                mutation.new_slide_number
            )),
        );
        result.insert(
            "slidesListCommandTemplate".to_string(),
            json!(slides_list_command("<out.pptx>")),
        );
        result.insert(
            "validateCommandTemplate".to_string(),
            json!(validate_command("<out.pptx>")),
        );
        result.insert(
            "renderCommandTemplate".to_string(),
            json!(render_command("<out.pptx>")),
        );
    }
    Value::Object(result)
}

pub(super) fn move_result_json(
    file: &str,
    mutation: &MoveSlideMutation,
    output_path: Option<&str>,
    dry_run: bool,
    destination: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("slideUri".to_string(), json!(mutation.slide_uri));
    result.insert("fromPosition".to_string(), json!(mutation.from_position));
    result.insert("toPosition".to_string(), json!(mutation.to_position));
    result.insert("isNoOp".to_string(), json!(mutation.is_no_op));
    result.insert("destination".to_string(), destination);
    add_slide_readback_command(&mut result, output_path, mutation.to_position);
    add_pptx_slides_mutation_commands(&mut result, output_path);
    Value::Object(result)
}

pub(super) fn reorder_result_json(
    file: &str,
    mutation: &ReorderSlidesMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("newOrder".to_string(), json!(mutation.new_order));
    result.insert("slideCount".to_string(), json!(mutation.slide_count));
    add_pptx_slides_mutation_commands(&mut result, output_path);
    Value::Object(result)
}

fn add_slide_readback_command(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    slide: i64,
) {
    let command_target = output_path.unwrap_or("<out.pptx>");
    let command_suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    result.insert(
        format!("readbackCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {slide} --include-text --include-bounds",
            command_arg(command_target)
        )),
    );
}

fn add_pptx_slides_mutation_commands(result: &mut Map<String, Value>, output_path: Option<&str>) {
    let command_target = output_path.unwrap_or("<out.pptx>");
    let command_suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    result.insert(
        format!("slidesListCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx slides list {}",
            command_arg(command_target)
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

fn slide_readback_command(file_path: &str, slide: i64) -> String {
    format!(
        "ooxml --json pptx slides show {} --slide {slide} --include-text --include-bounds",
        command_arg(file_path)
    )
}

fn slides_list_command(file_path: &str) -> String {
    format!("ooxml --json pptx slides list {}", command_arg(file_path))
}

fn validate_command(file_path: &str) -> String {
    format!("ooxml validate --strict {}", command_arg(file_path))
}

fn render_command(file_path: &str) -> String {
    format!(
        "ooxml pptx render {} --out render-check",
        command_arg(file_path)
    )
}
