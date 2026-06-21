use serde_json::{Map, Value, json};

use super::{
    AddAnimationMutation, PruneAnimationMutation, PrunedNode, RemoveAnimationMutation,
    ReorderAnimationMutation,
};
use crate::command_arg;

pub(super) fn add_animation_result(
    file: &str,
    mutation: &AddAnimationMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        out.insert("output".to_string(), json!(output_path));
    }
    out.insert("dryRun".to_string(), json!(dry_run));
    out.insert("action".to_string(), json!("pptx.animations.add"));
    out.insert("slide".to_string(), json!(mutation.slide.number));
    out.insert("shapeId".to_string(), json!(mutation.shape_id));
    out.insert("shapeName".to_string(), json!(mutation.shape_name));
    out.insert("effect".to_string(), json!(mutation.effect));
    out.insert("start".to_string(), json!(mutation.start));
    out.insert(
        "addedEffectIds".to_string(),
        json!(mutation.added_effect_ids),
    );
    out.insert("clickStepId".to_string(), json!(mutation.click_step_id));
    out.insert("createdTiming".to_string(), json!(mutation.created_timing));
    out.insert("byParagraph".to_string(), json!(mutation.by_paragraph));
    if mutation.paragraph_count > 0 {
        out.insert(
            "paragraphCount".to_string(),
            json!(mutation.paragraph_count),
        );
    }
    out.insert(
        "renderUnconfirmed".to_string(),
        json!(matches!(mutation.effect.as_str(), "fade" | "flyIn")),
    );
    add_animation_readback_commands(&mut out, output_path);
    Value::Object(out)
}

pub(super) fn remove_animation_result(
    file: &str,
    mutation: &RemoveAnimationMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        out.insert("output".to_string(), json!(output_path));
    }
    out.insert("dryRun".to_string(), json!(dry_run));
    out.insert("action".to_string(), json!("pptx.animations.remove"));
    out.insert("slide".to_string(), json!(mutation.slide.number));
    out.insert(
        "removedEffectId".to_string(),
        json!(mutation.removed_effect_id),
    );
    out.insert(
        "removedClickStep".to_string(),
        json!(mutation.removed_click_step),
    );
    out.insert("shapeId".to_string(), json!(mutation.shape_id));
    out.insert("shapeName".to_string(), json!(mutation.shape_name));
    add_animation_readback_commands(&mut out, output_path);
    Value::Object(out)
}

pub(super) fn reorder_animation_result(
    file: &str,
    mutation: &ReorderAnimationMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        out.insert("output".to_string(), json!(output_path));
    }
    out.insert("dryRun".to_string(), json!(dry_run));
    out.insert("action".to_string(), json!("pptx.animations.reorder"));
    out.insert("slide".to_string(), json!(mutation.slide.number));
    out.insert("order".to_string(), json!(mutation.order));
    out.insert(
        "clickStepCount".to_string(),
        json!(mutation.click_step_count),
    );
    add_animation_readback_commands(&mut out, output_path);
    Value::Object(out)
}

pub(super) fn prune_animation_result(
    file: &str,
    slide: u32,
    mutation: &PruneAnimationMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        out.insert("output".to_string(), json!(output_path));
    }
    out.insert("dryRun".to_string(), json!(dry_run));
    out.insert("action".to_string(), json!("pptx.animations.prune-stale"));
    out.insert("slide".to_string(), json!(slide));
    out.insert(
        "pruned".to_string(),
        Value::Array(mutation.pruned.iter().map(pruned_node_json).collect()),
    );
    out.insert("prunedCount".to_string(), json!(mutation.pruned.len()));
    add_animation_readback_commands(&mut out, output_path);
    Value::Object(out)
}

fn pruned_node_json(node: &PrunedNode) -> Value {
    let mut out = Map::new();
    out.insert("slide".to_string(), json!(node.slide));
    out.insert("kind".to_string(), json!(node.kind));
    if node.effect_id != 0 {
        out.insert("effectId".to_string(), json!(node.effect_id));
    }
    out.insert("spid".to_string(), json!(node.spid));
    out.insert("staleReason".to_string(), json!(node.stale_reason));
    Value::Object(out)
}

fn add_animation_readback_commands(out: &mut Map<String, Value>, output_path: Option<&str>) {
    let command_target = output_path.unwrap_or("<out.pptx>");
    let suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    out.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx animations list {}",
            command_arg(command_target)
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
