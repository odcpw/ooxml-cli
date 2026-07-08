use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use super::{
    ImageBatchReplacePlan, ImageBatchSlideResult, ImageReplacePlan, PptxReplaceMutationOptions,
    ReplaceTextFromXlsxRequest, ReplaceTextMapFromXlsxRequest, ShapeTarget, TextMapApplied,
    TextOccurrenceMatch, TextOccurrencePlan, TextOccurrencesRequest, TextTargetReplacePlan,
};
use crate::{
    CliError, CliResult, RelationshipEntry, command_arg,
    copy_zip_with_binary_part_overrides_and_removals, copy_zip_with_part_overrides,
    package_mutation_temp_path, package_type, validate, xml_attr_escape,
};

pub(super) fn text_from_xlsx_result_json(
    file: &str,
    request: &ReplaceTextFromXlsxRequest,
    plan: &TextTargetReplacePlan,
    options: &PptxReplaceMutationOptions,
) -> Value {
    let output = mutation_output_path(file, options);
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    if options.dry_run {
        result.insert("dryRun".to_string(), json!(true));
    }
    result.insert("source".to_string(), request.source.source.clone());
    result.insert(
        "text".to_string(),
        json!({
            "mode": request.mode,
            "formulaMode": request.formula_mode,
            "rowSeparator": request.row_sep,
            "colSeparator": request.col_sep,
            "chars": request.text.chars().count(),
            "value": request.text,
        }),
    );
    result.insert(
        "destination".to_string(),
        text_shape_destination_json(
            &plan.target,
            plan.slide,
            &request.target,
            &plan.text,
            output.as_deref(),
        ),
    );
    add_shape_text_readback_commands(
        &mut result,
        output.as_deref(),
        options.dry_run,
        plan.slide,
        &plan.target.primary_selector,
    );
    Value::Object(result)
}

pub(super) fn plain_text_result_json(
    file: &str,
    requested_target: &str,
    plan: &TextTargetReplacePlan,
    options: &PptxReplaceMutationOptions,
) -> Value {
    let output = mutation_output_path(file, options);
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("mode".to_string(), json!("plain-text"));
    result.insert("newText".to_string(), json!(plan.text));
    result.insert("slideNumber".to_string(), json!(plan.slide));
    result.insert("target".to_string(), json!(requested_target));
    result.insert(
        "destination".to_string(),
        text_shape_destination_json(
            &plan.target,
            plan.slide,
            requested_target,
            &plan.text,
            output.as_deref(),
        ),
    );
    add_shape_text_readback_commands(
        &mut result,
        output.as_deref(),
        options.dry_run,
        plan.slide,
        &plan.target.primary_selector,
    );
    Value::Object(result)
}

pub(super) fn text_map_from_xlsx_result_json(
    file: &str,
    request: &ReplaceTextMapFromXlsxRequest,
    applied: &[TextMapApplied],
    options: &PptxReplaceMutationOptions,
) -> Value {
    let output = mutation_output_path(file, options);
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    if options.dry_run {
        result.insert("dryRun".to_string(), json!(true));
    }
    result.insert("source".to_string(), request.source.source.clone());
    result.insert(
        "map".to_string(),
        json!({
            "mode": request.mode,
            "formulaMode": request.formula_mode,
            "rows": request.source.rows.saturating_sub(1),
            "applied": applied.len(),
            "slideColumn": request.columns.slide,
            "targetColumn": request.columns.target,
            "textColumn": request.columns.text,
        }),
    );
    result.insert(
        "replacements".to_string(),
        Value::Array(
            applied
                .iter()
                .map(|item| text_map_replacement_json(item, output.as_deref(), options.dry_run))
                .collect(),
        ),
    );
    add_output_verification_commands(&mut result, output.as_deref(), options.dry_run);
    Value::Object(result)
}

fn text_map_replacement_json(
    applied: &TextMapApplied,
    output: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("sourceRow".to_string(), json!(applied.record.source_row));
    result.insert("slide".to_string(), json!(applied.record.slide));
    result.insert("target".to_string(), json!(applied.record.target));
    result.insert(
        "chars".to_string(),
        json!(applied.record.text.chars().count()),
    );
    result.insert("text".to_string(), json!(applied.record.text));
    result.insert(
        "destination".to_string(),
        text_shape_destination_json(
            &applied.plan.target,
            applied.record.slide,
            &applied.record.target,
            &applied.record.text,
            output,
        ),
    );
    add_shape_text_readback_commands(
        &mut result,
        output,
        dry_run,
        applied.record.slide,
        &applied.plan.target.primary_selector,
    );
    Value::Object(result)
}

fn text_shape_destination_json(
    target: &ShapeTarget,
    slide: u32,
    requested_target: &str,
    text: &str,
    output: Option<&str>,
) -> Value {
    let mut result = Map::new();
    if let Some(output) = output {
        result.insert("file".to_string(), json!(output));
    }
    result.insert("slide".to_string(), json!(slide));
    result.insert("target".to_string(), json!(requested_target));
    result.insert("shapeId".to_string(), json!(target.shape_id));
    if !target.shape_name.is_empty() {
        result.insert("shapeName".to_string(), json!(target.shape_name));
    }
    result.insert("targetKind".to_string(), json!(target.target_kind));
    result.insert(
        "primarySelector".to_string(),
        json!(target.primary_selector),
    );
    if !target.handle.is_empty() {
        result.insert("handle".to_string(), json!(target.handle));
    }
    result.insert("selectors".to_string(), json!(target.selectors));
    result.insert("textPreview".to_string(), json!(text_preview(text)));
    Value::Object(result)
}

fn text_preview(text: &str) -> String {
    let mut preview = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if preview.len() > 140 {
        preview.truncate(137);
        preview.push_str("...");
    }
    preview
}

fn add_shape_text_readback_commands(
    result: &mut Map<String, Value>,
    output: Option<&str>,
    dry_run: bool,
    slide: u32,
    target: &str,
) {
    let command_target = output.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    result.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx shapes get {} --slide {} --target {} --include-text --include-bounds",
            command_arg(command_target),
            slide,
            command_arg(target)
        )),
    );
    result.insert(
        format!("slideReadbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(command_target),
            slide
        )),
    );
    add_output_verification_commands(result, Some(command_target), dry_run);
}

pub(super) fn text_occurrences_result_json(
    file: &str,
    request: &TextOccurrencesRequest,
    plan: &mut TextOccurrencePlan,
    options: &PptxReplaceMutationOptions,
) -> Value {
    let output = mutation_output_path(file, options);
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert(
        "operation".to_string(),
        json!("pptx.replace.text-occurrences"),
    );
    result.insert("matchText".to_string(), json!(request.match_text));
    result.insert("newText".to_string(), json!(request.new_text));
    result.insert("ignoreCase".to_string(), json!(request.ignore_case));
    if !request.for_slides.is_empty() {
        result.insert("forSlides".to_string(), json!(request.for_slides));
    }
    let mut guard = Map::new();
    if let Some(expected) = request.expect_count {
        guard.insert("expectedCount".to_string(), json!(expected));
    }
    guard.insert("actualCount".to_string(), json!(plan.replacement_count));
    if !request.expect_plan_hash.is_empty() {
        guard.insert(
            "expectedPlanHash".to_string(),
            json!(request.expect_plan_hash),
        );
    }
    guard.insert("actualPlanHash".to_string(), json!(plan.plan_hash));
    guard.insert("allowZero".to_string(), json!(request.allow_zero));
    result.insert("staleGuard".to_string(), Value::Object(guard));
    result.insert(
        "summary".to_string(),
        json!({
            "slidesScanned": plan.slides_scanned,
            "targetsScanned": plan.targets_scanned,
            "textNodesScanned": plan.text_nodes_scanned,
            "changedTargetCount": plan.changed_target_count,
            "replacementCount": plan.replacement_count,
        }),
    );
    result.insert(
        "scope".to_string(),
        json!({
            "slides": plan.selected_slides,
            "text": if plan.shape_scoped {
                "slide-visible text nodes under a single shape target (shape-scoped)"
            } else {
                "slide-visible text nodes under published slide targets"
            },
            "splitRunMatches": "not matched; only occurrences contained within one XML text node are replaced",
            "excludedContent": "notes, layouts, masters, comments, charts, and non-slide parts",
            "tableCellsIncluded": true,
            "slideShapesIncluded": true,
        }),
    );
    result.insert(
        "matches".to_string(),
        Value::Array(
            plan.matches
                .iter()
                .map(|item| text_occurrence_match_json(item, output.as_deref(), options.dry_run))
                .collect(),
        ),
    );
    add_output_verification_commands(&mut result, output.as_deref(), options.dry_run);
    Value::Object(result)
}

fn text_occurrence_match_json(
    item: &TextOccurrenceMatch,
    output: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("slideNumber".to_string(), json!(item.slide_number));
    result.insert("partUri".to_string(), json!(item.part_uri));
    result.insert("shapeId".to_string(), json!(item.shape_id));
    if !item.shape_name.is_empty() {
        result.insert("shapeName".to_string(), json!(item.shape_name));
    }
    result.insert("targetKind".to_string(), json!(item.target_kind));
    result.insert("primarySelector".to_string(), json!(item.primary_selector));
    result.insert("selectors".to_string(), json!(item.selectors));
    result.insert("textNodeIndex".to_string(), json!(item.text_node_index));
    result.insert("matchCount".to_string(), json!(item.match_count));
    result.insert("beforeText".to_string(), json!(item.before_text));
    result.insert("afterText".to_string(), json!(item.after_text));
    result.insert("sourceHash".to_string(), json!(item.source_hash));
    let command_target = output.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    let readback = if item.target_kind == "table" {
        format!(
            "ooxml --json pptx tables show {} --slide {} --target {}",
            command_arg(command_target),
            item.slide_number,
            command_arg(&item.primary_selector)
        )
    } else {
        format!(
            "ooxml --json pptx shapes get {} --slide {} --target {} --include-text --include-bounds",
            command_arg(command_target),
            item.slide_number,
            command_arg(&item.primary_selector)
        )
    };
    result.insert(format!("readbackCommand{suffix}"), json!(readback));
    add_slide_validate_render_commands(&mut result, command_target, item.slide_number, dry_run);
    Value::Object(result)
}

pub(super) fn image_batch_replace_result_json(
    target_selector: &str,
    plan: &ImageBatchReplacePlan,
) -> Value {
    json!({
        "target": target_selector,
        "totalSlides": plan.slides.len(),
        "successCount": plan.success_count,
        "notFoundCount": plan.not_found_count,
        "errorCount": plan.error_count,
        "results": plan.slides.iter().map(image_batch_slide_result_json).collect::<Vec<_>>(),
    })
}

fn image_batch_slide_result_json(item: &ImageBatchSlideResult) -> Value {
    json!({
        "SlideNumber": item.slide,
        "Success": item.success,
        "NotFound": item.not_found,
        "Error": item.error,
        "Result": item.plan.as_ref().map(|plan| {
            json!({
                "ShapeID": plan.target.shape_id,
                "ShapeName": plan.target.shape_name,
                "OldTargetURI": plan.old_target_uri,
                "OldContentType": plan.old_content_type,
                "NewTargetURI": plan.new_target_uri,
                "NewContentType": plan.new_content_type,
                "RelationshipID": plan.relationship_id,
            })
        }),
    })
}

pub(super) fn image_replace_result_json(
    file: &str,
    target_selector: &str,
    fit_mode: &str,
    plan: &ImageReplacePlan,
    options: &PptxReplaceMutationOptions,
) -> Value {
    let output = mutation_output_path(file, options);
    let destination = image_destination_json(
        &plan.target,
        plan.slide,
        target_selector,
        output.as_deref(),
        &plan.relationship_id,
        &plan.new_target_uri,
        &plan.new_content_type,
    );
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("target".to_string(), json!(target_selector));
    result.insert("fitMode".to_string(), json!(fit_mode));
    result.insert("slideNumber".to_string(), json!(plan.slide));
    result.insert("shapeId".to_string(), json!(plan.target.shape_id));
    result.insert("shapeName".to_string(), json!(plan.target.shape_name));
    result.insert("oldTargetUri".to_string(), json!(plan.old_target_uri));
    result.insert("oldContentType".to_string(), json!(plan.old_content_type));
    result.insert("newTargetUri".to_string(), json!(plan.new_target_uri));
    result.insert("newContentType".to_string(), json!(plan.new_content_type));
    result.insert("relationshipId".to_string(), json!(plan.relationship_id));
    result.insert("destination".to_string(), destination);
    let command_target = output.as_deref().unwrap_or("<out.pptx>");
    let suffix = if options.dry_run { "Template" } else { "" };
    result.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx shapes get {} --slide {} --target {} --include-bounds",
            command_arg(command_target),
            plan.slide,
            command_arg(&plan.target.primary_selector)
        )),
    );
    add_slide_validate_render_commands(&mut result, command_target, plan.slide, options.dry_run);
    Value::Object(result)
}

fn image_destination_json(
    target: &ShapeTarget,
    slide: u32,
    requested_target: &str,
    output: Option<&str>,
    rel_id: &str,
    target_uri: &str,
    content_type: &str,
) -> Value {
    let mut result = Map::new();
    if let Some(output) = output {
        result.insert("file".to_string(), json!(output));
    }
    result.insert("slide".to_string(), json!(slide));
    result.insert("target".to_string(), json!(requested_target));
    result.insert("shapeId".to_string(), json!(target.shape_id));
    if !target.shape_name.is_empty() {
        result.insert("shapeName".to_string(), json!(target.shape_name));
    }
    result.insert("targetKind".to_string(), json!(target.target_kind));
    result.insert(
        "primarySelector".to_string(),
        json!(target.primary_selector),
    );
    if !target.handle.is_empty() {
        result.insert("handle".to_string(), json!(target.handle));
    }
    result.insert("selectors".to_string(), json!(target.selectors));
    if let Some(bounds) = target.bounds {
        result.insert(
            "bounds".to_string(),
            json!({"x": bounds.x, "y": bounds.y, "cx": bounds.cx, "cy": bounds.cy}),
        );
    }
    result.insert(
        "imageRef".to_string(),
        json!({
            "relId": rel_id,
            "targetUri": target_uri,
            "contentType": content_type,
        }),
    );
    Value::Object(result)
}

pub(super) fn render_relationships_xml(rels: &[RelationshipEntry]) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for rel in rels {
        out.push_str(&format!(
            r#"<Relationship Id="{}" Type="{}" Target="{}""#,
            xml_attr_escape(&rel.id),
            xml_attr_escape(&rel.rel_type),
            xml_attr_escape(&rel.target)
        ));
        if !rel.target_mode.is_empty() {
            out.push_str(&format!(
                r#" TargetMode="{}""#,
                xml_attr_escape(&rel.target_mode)
            ));
        }
        out.push_str("/>");
    }
    out.push_str("</Relationships>");
    out
}

pub(super) fn write_replace_mutation(
    file: &str,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    options: &PptxReplaceMutationOptions,
) -> CliResult<()> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-replace")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    if binary_overrides.is_empty() {
        copy_zip_with_part_overrides(file, &write_path, text_overrides)?;
    } else {
        copy_zip_with_binary_part_overrides_and_removals(
            file,
            &write_path,
            text_overrides,
            binary_overrides,
            &BTreeSet::new(),
        )?;
    }
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&write_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&write_path, file)
            .or_else(|_| {
                fs::copy(&write_path, file)?;
                fs::remove_file(&write_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

fn mutation_output_path(file: &str, options: &PptxReplaceMutationOptions) -> Option<String> {
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

fn add_output_verification_commands(
    result: &mut Map<String, Value>,
    output: Option<&str>,
    dry_run: bool,
) {
    let command_target = output.unwrap_or("<out.pptx>");
    let suffix = if dry_run { "Template" } else { "" };
    result.insert(
        format!("validateCommand{suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
}

fn add_slide_validate_render_commands(
    result: &mut Map<String, Value>,
    command_target: &str,
    slide: u32,
    dry_run: bool,
) {
    let suffix = if dry_run { "Template" } else { "" };
    result.insert(
        format!("slideReadbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(command_target),
            slide
        )),
    );
    add_output_verification_commands(result, Some(command_target), dry_run);
}

pub(super) fn sha256_string(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

pub(super) fn ensure_pptx(file: &str) -> CliResult<()> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    Ok(())
}
