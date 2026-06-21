use serde_json::{Map, Value, json};

use super::{AddCommentResult, EditCommentResult, PptxCommentMutationOptions, RemoveCommentResult};
use crate::command_arg;

pub(super) fn add_comment_result_json(
    file: &str,
    result: &AddCommentResult,
    options: &PptxCommentMutationOptions,
) -> Value {
    let mut out = base_mutation_json(file, options, "added");
    let command_target = command_target(file, options);
    let handle = comment_handle(result.slide_id, result.comment_id, result.author_id);
    add_common_comment_result_fields(
        &mut out,
        &command_target,
        options.dry_run,
        result.slide,
        &handle,
        result.comment_id,
        result.author_id,
    );
    out.insert("slide".to_string(), json!(result.slide));
    out.insert("slidePartUri".to_string(), json!(result.slide_part_uri));
    out.insert("commentsPart".to_string(), json!(result.comments_part));
    out.insert("commentId".to_string(), json!(result.comment_id));
    out.insert("authorId".to_string(), json!(result.author_id));
    out.insert("author".to_string(), json!(result.author));
    if !result.initials.is_empty() {
        out.insert("initials".to_string(), json!(result.initials));
    }
    if !result.date.is_empty() {
        out.insert("date".to_string(), json!(result.date));
    }
    out.insert("text".to_string(), json!(result.text));
    out.insert("contentHash".to_string(), json!(result.content_hash));
    out.insert("createdPart".to_string(), json!(result.created_part));
    out.insert(
        "createdRelationship".to_string(),
        json!(result.created_relationship),
    );
    out.insert(
        "createdAuthorsPart".to_string(),
        json!(result.created_authors_part),
    );
    out.insert("createdAuthor".to_string(), json!(result.created_author));
    Value::Object(out)
}

pub(super) fn edit_comment_result_json(
    file: &str,
    result: &EditCommentResult,
    options: &PptxCommentMutationOptions,
) -> Value {
    let mut out = base_mutation_json(file, options, "edited");
    let command_target = command_target(file, options);
    let handle = comment_handle(result.slide_id, result.comment_id, result.author_id);
    add_common_comment_result_fields(
        &mut out,
        &command_target,
        options.dry_run,
        result.slide,
        &handle,
        result.comment_id,
        result.author_id,
    );
    out.insert("slide".to_string(), json!(result.slide));
    out.insert("slidePartUri".to_string(), json!(result.slide_part_uri));
    out.insert("commentsPart".to_string(), json!(result.comments_part));
    out.insert("commentId".to_string(), json!(result.comment_id));
    out.insert("authorId".to_string(), json!(result.author_id));
    out.insert("author".to_string(), json!(result.author));
    if !result.initials.is_empty() {
        out.insert("initials".to_string(), json!(result.initials));
    }
    if !result.date.is_empty() {
        out.insert("date".to_string(), json!(result.date));
    }
    out.insert("text".to_string(), json!(result.text));
    out.insert("contentHash".to_string(), json!(result.content_hash));
    out.insert("previousText".to_string(), json!(result.previous_text));
    out.insert("previousHash".to_string(), json!(result.previous_hash));
    Value::Object(out)
}

pub(super) fn remove_comment_result_json(
    file: &str,
    result: &RemoveCommentResult,
    options: &PptxCommentMutationOptions,
) -> Value {
    let mut out = base_mutation_json(file, options, "removed");
    let command_target = command_target(file, options);
    let handle = comment_handle(result.slide_id, result.comment_id, result.author_id);
    add_common_comment_result_fields(
        &mut out,
        &command_target,
        options.dry_run,
        result.slide,
        &handle,
        result.comment_id,
        result.author_id,
    );
    out.insert("slide".to_string(), json!(result.slide));
    out.insert("slidePartUri".to_string(), json!(result.slide_part_uri));
    out.insert("commentsPart".to_string(), json!(result.comments_part));
    out.insert("commentId".to_string(), json!(result.comment_id));
    out.insert("authorId".to_string(), json!(result.author_id));
    out.insert("previousAuthor".to_string(), json!(result.previous_author));
    out.insert("previousText".to_string(), json!(result.previous_text));
    out.insert("previousHash".to_string(), json!(result.previous_hash));
    out.insert("removedPart".to_string(), json!(result.removed_part));
    Value::Object(out)
}

fn add_common_comment_result_fields(
    out: &mut Map<String, Value>,
    command_target: &str,
    dry_run: bool,
    slide: u32,
    handle: &str,
    comment_id: i64,
    author_id: i64,
) {
    out.insert("dryRun".to_string(), json!(dry_run));
    out.insert("handle".to_string(), json!(handle));
    out.insert("primarySelector".to_string(), json!(handle));
    out.insert(
        "selectors".to_string(),
        json!(comment_selectors(handle, comment_id, author_id)),
    );
    let suffix = if dry_run { "Template" } else { "" };
    out.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx comments list {} --slide {}",
            command_arg(command_target),
            slide
        )),
    );
    out.insert(
        format!("slideReadbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(command_target),
            slide
        )),
    );
    out.insert(
        format!("validateCommand{suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    out.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
}

fn base_mutation_json(
    file: &str,
    options: &PptxCommentMutationOptions,
    operation: &str,
) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if !options.dry_run
        && let Some(output) = mutation_output_path(file, options)
    {
        out.insert("output".to_string(), json!(output));
    }
    out.insert("operation".to_string(), json!(operation));
    out
}

fn command_target(file: &str, options: &PptxCommentMutationOptions) -> String {
    if options.dry_run {
        "<out.pptx>".to_string()
    } else {
        mutation_output_path(file, options).unwrap_or_else(|| file.to_string())
    }
}

fn mutation_output_path(file: &str, options: &PptxCommentMutationOptions) -> Option<String> {
    if options.dry_run {
        None
    } else if options.in_place {
        Some(file.to_string())
    } else {
        options
            .out
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    }
}

fn comment_handle(slide_id: u32, comment_id: i64, author_id: i64) -> String {
    if slide_id > 0 {
        format!("H:pptx/s:{slide_id}/comment:idx:{comment_id}:authorId:{author_id}")
    } else {
        format!("comment:{comment_id}:authorId:{author_id}")
    }
}

fn comment_selectors(handle: &str, comment_id: i64, author_id: i64) -> Vec<String> {
    vec![
        handle.to_string(),
        format!("comment:{comment_id}:authorId:{author_id}"),
        format!("comment:{comment_id}"),
        comment_id.to_string(),
        format!("authorId:{author_id}"),
    ]
}
