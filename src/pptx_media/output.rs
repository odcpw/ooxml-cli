use serde_json::{Map, Value, json};

use crate::command_arg;

use super::{AddMutation, ReplaceMutation};

pub(super) fn media_add_result_json(
    file: &str,
    output: Option<&str>,
    dry_run: bool,
    mutation: &AddMutation,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("action".to_string(), json!("pptx.media.add"));
    result.insert("slide".to_string(), json!(mutation.slide_number));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    result.insert("kind".to_string(), json!(mutation.kind));
    result.insert("mediaPartUri".to_string(), json!(mutation.media_uri));
    result.insert(
        "mediaContentType".to_string(),
        json!(mutation.media_content_type),
    );
    result.insert("posterPartUri".to_string(), json!(mutation.poster_uri));
    result.insert(
        "mediaRelationshipId".to_string(),
        json!(mutation.media_rel_id),
    );
    result.insert("avRelationshipId".to_string(), json!(mutation.av_rel_id));
    result.insert(
        "posterRelationshipId".to_string(),
        json!(mutation.poster_rel_id),
    );
    result.insert("playTrigger".to_string(), json!(mutation.play_trigger));
    result.insert(
        "posterSynthesized".to_string(),
        json!(mutation.poster_synthesized),
    );
    result.insert("emitPlayCmd".to_string(), json!(mutation.emit_play_cmd));
    result.insert(
        "renderUnconfirmed".to_string(),
        json!(mutation.emit_play_cmd),
    );
    add_media_readback_commands(&mut result, output, dry_run, mutation.slide_number);
    result
}

pub(super) fn media_replace_result_json(
    file: &str,
    output: Option<&str>,
    dry_run: bool,
    mutation: &ReplaceMutation,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("action".to_string(), json!("pptx.media.replace"));
    result.insert("slide".to_string(), json!(mutation.slide_number));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    result.insert("oldKind".to_string(), json!(mutation.old_kind));
    result.insert("newKind".to_string(), json!(mutation.new_kind));
    result.insert("oldMediaUri".to_string(), json!(mutation.old_media_uri));
    result.insert("newMediaUri".to_string(), json!(mutation.new_media_uri));
    result.insert(
        "oldContentType".to_string(),
        json!(mutation.old_content_type),
    );
    result.insert(
        "newContentType".to_string(),
        json!(mutation.new_content_type),
    );
    result.insert(
        "posterReplaced".to_string(),
        json!(mutation.poster_replaced),
    );
    add_media_readback_commands(&mut result, output, dry_run, mutation.slide_number);
    result
}

fn add_media_readback_commands(
    result: &mut Map<String, Value>,
    output: Option<&str>,
    dry_run: bool,
    slide: u32,
) {
    let target = output.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    result.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx media list {} --slide {slide}",
            command_arg(target)
        )),
    );
    result.insert(
        format!("slideReadbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {slide} --include-text --include-bounds",
            command_arg(target)
        )),
    );
    result.insert(
        format!("validateCommand{suffix}"),
        json!(format!("ooxml validate --strict {}", command_arg(target))),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(target)
        )),
    );
}
