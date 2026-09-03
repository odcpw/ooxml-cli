use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};

use super::{
    Bounds, DEFAULT_SAFE_MARGIN, LayoutShape, ParagraphMeasure, TextBlock, bounds_intersect,
    bounds_value, json_number, layout_shapes_from_resolved, pptx_slide_refs, pptx_slide_size,
};
use crate::pptx_readback::pptx_resolved_shape_models;
use crate::text_metrics::measure_text_box_with_autofit;
use crate::{
    CliError, CliResult, attr, command_arg, copy_zip_with_part_overrides, local_name,
    parse_string_flag, relationships, resolve_relationship_target, xml_direct_child_ranges,
    zip_text,
};

const MIN_FONT_POINTS: f64 = 12.0;
const FONT_STEP_POINTS: f64 = 0.5;

#[derive(Clone)]
enum FixChange {
    Bounds {
        before: Bounds,
        after: Bounds,
        slot: Option<String>,
    },
    Font {
        bounds: Bounds,
        before_points: f64,
        after_points: f64,
    },
}

#[derive(Clone)]
struct FixAction {
    action: &'static str,
    reason: String,
    slide: usize,
    slide_part: String,
    shape_id: i64,
    shape_name: String,
    change: FixChange,
}

struct FixPlan {
    actions: Vec<FixAction>,
    manual: Vec<Value>,
}

struct LoadedSlide {
    number: usize,
    part: String,
    width: i64,
    height: i64,
    shapes: Vec<LayoutShape>,
}

#[derive(Clone)]
struct GridSlot {
    name: String,
    bounds: Bounds,
}

pub(super) fn validate_layout_with_fix(file: &str, args: &[String]) -> CliResult<Value> {
    let mode = parse_string_flag(args, "--fix")?
        .ok_or_else(|| CliError::invalid_args("--out requires --fix auto"))?;
    let output = parse_string_flag(args, "--out")?;
    match mode.trim().to_ascii_lowercase().as_str() {
        "plan" => {
            if output.is_some() {
                return Err(CliError::invalid_args(
                    "--fix plan is read-only and cannot be combined with --out",
                ));
            }
            let report = super::pptx_validate_layout(file)?;
            let plan = build_fix_plan(file)?;
            Ok(plan_result(report, file, &plan))
        }
        "auto" => {
            let output = output
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| CliError::invalid_args("--fix auto requires --out <file>"))?;
            apply_fix_plan(file, &output)
        }
        other => Err(CliError::invalid_args(format!(
            "invalid --fix {other:?}; accepted values: plan, auto"
        ))),
    }
}

fn plan_result(mut report: Value, file: &str, plan: &FixPlan) -> Value {
    let object = report
        .as_object_mut()
        .expect("layout report is always an object");
    object.insert("fixMode".to_string(), json!("plan"));
    object.insert("dryRun".to_string(), json!(true));
    object.insert("wouldWrite".to_string(), json!(false));
    object.insert("minimumFontPoints".to_string(), json!(MIN_FONT_POINTS));
    object.insert("fixCount".to_string(), json!(plan.actions.len()));
    object.insert("fixPlan".to_string(), actions_json(&plan.actions));
    object.insert("appliedFixes".to_string(), json!([]));
    object.insert(
        "unfixableFindings".to_string(),
        Value::Array(plan.manual.clone()),
    );
    object.insert(
        "applyCommand".to_string(),
        json!(format!(
            "ooxml --json pptx validate-layout {} --fix auto --out {}",
            command_arg(file),
            command_arg(&super::layout_fixed_path(file))
        )),
    );
    report
}

fn apply_fix_plan(file: &str, output: &str) -> CliResult<Value> {
    let before = super::pptx_validate_layout(file)?;
    let plan = build_fix_plan(file)?;
    let overrides = apply_actions_to_parts(file, &plan.actions)?;
    let staged = crate::mutation_staging_path(file, Some(output), "pptx-layout-fix");
    copy_zip_with_part_overrides(file, &staged, &overrides)?;
    crate::validate_owned_mutation_output(&staged)?;
    crate::finish_mutation_output(file, &staged, Some(output), false, None, false)?;
    let after = super::pptx_validate_layout(output)?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("output".to_string(), json!(output));
    result.insert("fixMode".to_string(), json!("auto"));
    result.insert("dryRun".to_string(), json!(false));
    result.insert("minimumFontPoints".to_string(), json!(MIN_FONT_POINTS));
    result.insert("fixCount".to_string(), json!(plan.actions.len()));
    result.insert("fixPlan".to_string(), actions_json(&plan.actions));
    result.insert("appliedFixes".to_string(), actions_json(&plan.actions));
    result.insert("unfixableFindings".to_string(), Value::Array(plan.manual));
    result.insert("before".to_string(), before);
    result.insert("after".to_string(), after);
    result.insert(
        "readbackCommand".to_string(),
        json!(format!(
            "ooxml --json pptx validate-layout {}",
            command_arg(output)
        )),
    );
    result.insert(
        "validateCommand".to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(output))),
    );
    result.insert(
        "renderCommand".to_string(),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(output)
        )),
    );
    Ok(Value::Object(result))
}

fn build_fix_plan(file: &str) -> CliResult<FixPlan> {
    let mut actions = Vec::new();
    let mut manual = Vec::new();
    for mut slide in load_slides(file)? {
        plan_font_fixes(&slide, &mut actions, &mut manual);
        plan_margin_fixes(&mut slide, &mut actions, &mut manual);
        plan_collision_fixes(file, &mut slide, &mut actions, &mut manual)?;
    }
    Ok(FixPlan { actions, manual })
}

fn load_slides(file: &str) -> CliResult<Vec<LoadedSlide>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let (width, height) = pptx_slide_size(&presentation);
    let slide_refs = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    slide_refs
        .iter()
        .enumerate()
        .map(|(index, (_, rel_id))| {
            let target = rels
                .get(rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            let part = resolve_relationship_target("/ppt/presentation.xml", target);
            let xml = zip_text(file, part.trim_start_matches('/'))?;
            let resolved = pptx_resolved_shape_models(file, &part, &xml)?;
            Ok(LoadedSlide {
                number: index + 1,
                part: part.trim_start_matches('/').to_string(),
                width,
                height,
                shapes: layout_shapes_from_resolved(&xml, &resolved),
            })
        })
        .collect()
}

fn plan_font_fixes(slide: &LoadedSlide, actions: &mut Vec<FixAction>, manual: &mut Vec<Value>) {
    for shape in &slide.shapes {
        let Some(text) = shape.text.as_ref() else {
            continue;
        };
        let Some(bounds) = shape.bounds.as_ref() else {
            continue;
        };
        if !text_overflows_at_cap(text, bounds, None) {
            continue;
        }
        if !shape.is_placeholder {
            manual.push(json!({
                "kind": "text-overflow",
                "slide": slide.number,
                "shapeId": shape.id,
                "shapeName": shape.name,
                "manualSuggestion": "Automatic font shrinking is limited to placeholders; resize the text box or edit its content manually.",
            }));
            continue;
        }
        let before_points = maximum_font_points(text);
        if let Some(after_points) = highest_fitting_font_size(text, bounds, before_points) {
            actions.push(FixAction {
                action: "shrink-placeholder-font",
                reason: "Placeholder text exceeds its resolved bounds".to_string(),
                slide: slide.number,
                slide_part: slide.part.clone(),
                shape_id: shape.id,
                shape_name: shape.name.clone(),
                change: FixChange::Font {
                    bounds: bounds.clone(),
                    before_points,
                    after_points,
                },
            });
        } else {
            manual.push(json!({
                "kind": "text-overflow",
                "slide": slide.number,
                "shapeId": shape.id,
                "shapeName": shape.name,
                "minimumFontPoints": MIN_FONT_POINTS,
                "manualSuggestion": "Text still overflows at the automatic font-size floor; shorten the content or enlarge the placeholder manually.",
            }));
        }
    }
}

fn plan_margin_fixes(
    slide: &mut LoadedSlide,
    actions: &mut Vec<FixAction>,
    manual: &mut Vec<Value>,
) {
    for shape in &mut slide.shapes {
        let Some(before) = shape.bounds.clone() else {
            continue;
        };
        let Some(after) =
            nudge_inside_safe_margin(&before, slide.width, slide.height, DEFAULT_SAFE_MARGIN)
        else {
            manual.push(json!({
                "kind": "safe-margin",
                "slide": slide.number,
                "shapeId": shape.id,
                "shapeName": shape.name,
                "beforeBounds": bounds_value(&before),
                "manualSuggestion": "The shape is larger than the safe area, so it cannot be fixed by a reversible nudge; resize it manually.",
            }));
            continue;
        };
        if same_bounds(&before, &after) {
            continue;
        }
        actions.push(FixAction {
            action: "nudge-inside-safe-margin",
            reason: "Shape crosses the slide safe margin".to_string(),
            slide: slide.number,
            slide_part: slide.part.clone(),
            shape_id: shape.id,
            shape_name: shape.name.clone(),
            change: FixChange::Bounds {
                before: before.clone(),
                after: after.clone(),
                slot: None,
            },
        });
        shape.bounds = Some(after);
    }
}

fn plan_collision_fixes(
    file: &str,
    slide: &mut LoadedSlide,
    actions: &mut Vec<FixAction>,
    manual: &mut Vec<Value>,
) -> CliResult<()> {
    let body = crate::cli_dispatch::pptx_slots::body_bounds(file, slide.number as u32)?;
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let (slide_width, slide_height) = pptx_slide_size(&presentation);
    for first in 0..slide.shapes.len() {
        for second in first + 1..slide.shapes.len() {
            if collision_percentage(&slide.shapes[first], &slide.shapes[second]).is_none() {
                continue;
            }
            let left = &slide.shapes[first];
            let right = &slide.shapes[second];
            if left.is_placeholder && right.is_placeholder {
                manual.push(placeholder_overlap_manual(slide.number, left, right));
                continue;
            }
            if right.is_placeholder || !is_cli_created_shape(right) {
                manual.push(json!({
                    "kind": "collision",
                    "slide": slide.number,
                    "shapeId1": left.id,
                    "shapeName1": left.name,
                    "shapeId2": right.id,
                    "shapeName2": right.name,
                    "manualSuggestion": "Automatic collision moves only the later CLI-created non-placeholder shape; move one shape manually.",
                }));
                continue;
            }
            let Some(before) = right.bounds.clone() else {
                continue;
            };
            let slots = layout_grid_slots(body, slide_width, slide_height);
            let Some(slot) = nearest_free_slot(&slots, &slide.shapes, second, &before) else {
                manual.push(json!({
                    "kind": "collision",
                    "slide": slide.number,
                    "shapeId1": left.id,
                    "shapeName1": left.name,
                    "shapeId2": right.id,
                    "shapeName2": right.name,
                    "manualSuggestion": "No free layout-grid slot can hold the later CLI-created shape; rearrange the slide manually.",
                }));
                continue;
            };
            actions.push(FixAction {
                action: "move-to-free-layout-slot",
                reason: format!(
                    "Later CLI-created shape overlaps shape:{}",
                    slide.shapes[first].id
                ),
                slide: slide.number,
                slide_part: slide.part.clone(),
                shape_id: slide.shapes[second].id,
                shape_name: slide.shapes[second].name.clone(),
                change: FixChange::Bounds {
                    before,
                    after: slot.bounds.clone(),
                    slot: Some(slot.name.clone()),
                },
            });
            slide.shapes[second].bounds = Some(slot.bounds);
        }
    }
    Ok(())
}

fn placeholder_overlap_manual(slide: usize, left: &LayoutShape, right: &LayoutShape) -> Value {
    json!({
        "kind": "collision",
        "slide": slide,
        "shapeId1": left.id,
        "shapeName1": left.name,
        "shapeId2": right.id,
        "shapeName2": right.name,
        "autoFixable": false,
        "manualSuggestion": "Both overlapping shapes are placeholders; adjust their layout or master geometry manually because automatic fixes never move overlapping placeholders.",
    })
}

fn maximum_font_points(text: &TextBlock) -> f64 {
    text.paragraphs
        .iter()
        .flat_map(|paragraph| paragraph.font_sizes.iter().copied())
        .fold(18.0_f64, f64::max)
}

fn highest_fitting_font_size(text: &TextBlock, bounds: &Bounds, before_points: f64) -> Option<f64> {
    let floor_step = (MIN_FONT_POINTS / FONT_STEP_POINTS).ceil() as i64;
    let before_step = (before_points / FONT_STEP_POINTS).floor() as i64;
    (floor_step..before_step)
        .rev()
        .map(|step| step as f64 * FONT_STEP_POINTS)
        .find(|candidate| !text_overflows_at_cap(text, bounds, Some(*candidate)))
}

fn text_overflows_at_cap(text: &TextBlock, bounds: &Bounds, cap: Option<f64>) -> bool {
    let paragraphs = text
        .paragraphs
        .iter()
        .map(|paragraph| {
            let size = paragraph
                .font_sizes
                .iter()
                .copied()
                .fold(18.0_f64, f64::max);
            ParagraphMeasure {
                text: &paragraph.text,
                font_family: if paragraph.font_family.is_empty() {
                    "Aptos"
                } else {
                    &paragraph.font_family
                },
                font_size_points: cap.map_or(size, |cap| size.min(cap)),
                bold: paragraph.bold,
                left_indent_emu: paragraph.left_indent,
                right_indent_emu: paragraph.right_indent,
                first_line_indent_emu: paragraph.first_line_indent,
                bullet: paragraph.bullet,
                line_spacing: 1.0,
            }
        })
        .collect::<Vec<_>>();
    let measurement = measure_text_box_with_autofit(
        &paragraphs,
        bounds.cx,
        bounds.cy,
        text.left_inset.unwrap_or(91_440),
        text.right_inset.unwrap_or(91_440),
        text.top_inset.unwrap_or(45_720),
        text.bottom_inset.unwrap_or(45_720),
        text.autofit,
    );
    let line_height = measurement
        .paragraphs
        .iter()
        .map(|paragraph| paragraph.line_height_emu)
        .max()
        .unwrap_or_default();
    measurement.height_emu - measurement.available_height_emu > line_height / 2
}

fn nudge_inside_safe_margin(
    bounds: &Bounds,
    slide_width: i64,
    slide_height: i64,
    margin: i64,
) -> Option<Bounds> {
    let width = slide_width - margin * 2;
    let height = slide_height - margin * 2;
    if bounds.cx > width || bounds.cy > height {
        return None;
    }
    Some(Bounds {
        x: bounds.x.clamp(margin, slide_width - margin - bounds.cx),
        y: bounds.y.clamp(margin, slide_height - margin - bounds.cy),
        cx: bounds.cx,
        cy: bounds.cy,
    })
}

fn layout_grid_slots(
    body: crate::cli_dispatch::pptx_slots::SlotBounds,
    slide_width: i64,
    slide_height: i64,
) -> Vec<GridSlot> {
    let safe = Bounds {
        x: DEFAULT_SAFE_MARGIN,
        y: DEFAULT_SAFE_MARGIN,
        cx: slide_width - DEFAULT_SAFE_MARGIN * 2,
        cy: slide_height - DEFAULT_SAFE_MARGIN * 2,
    };
    let body_left = body.x.max(DEFAULT_SAFE_MARGIN);
    let body_top = body.y.max(DEFAULT_SAFE_MARGIN);
    let body_right = body
        .x
        .saturating_add(body.cx)
        .min(slide_width - DEFAULT_SAFE_MARGIN);
    let body_bottom = body
        .y
        .saturating_add(body.cy)
        .min(slide_height - DEFAULT_SAFE_MARGIN);
    let body = Bounds {
        x: body_left,
        y: body_top,
        cx: (body_right - body_left).max(0),
        cy: (body_bottom - body_top).max(0),
    };
    let mut slots = Vec::new();
    let mut seen = BTreeSet::new();
    for (prefix, area) in [("layout", body), ("safe", safe)] {
        for rows in 1..=4_i64 {
            for cols in 1..=4_i64 {
                for index in 0..rows * cols {
                    let row = index / cols;
                    let col = index % cols;
                    let x0 = area.x + area.cx * col / cols;
                    let x1 = area.x + area.cx * (col + 1) / cols;
                    let y0 = area.y + area.cy * row / rows;
                    let y1 = area.y + area.cy * (row + 1) / rows;
                    let bounds = Bounds {
                        x: x0,
                        y: y0,
                        cx: x1 - x0,
                        cy: y1 - y0,
                    };
                    if bounds.cx > 0
                        && bounds.cy > 0
                        && seen.insert((bounds.x, bounds.y, bounds.cx, bounds.cy))
                    {
                        slots.push(GridSlot {
                            name: format!("{prefix}-grid:{rows}x{cols}:{}", index + 1),
                            bounds,
                        });
                    }
                }
            }
        }
    }
    slots
}

fn nearest_free_slot(
    slots: &[GridSlot],
    shapes: &[LayoutShape],
    moving_index: usize,
    before: &Bounds,
) -> Option<GridSlot> {
    slots
        .iter()
        .filter(|slot| {
            shapes.iter().enumerate().all(|(index, shape)| {
                index == moving_index
                    || shape
                        .bounds
                        .as_ref()
                        .is_none_or(|bounds| !bounds_intersect(&slot.bounds, bounds))
            })
        })
        .min_by(|left, right| compare_slots(left, right, before))
        .cloned()
}

fn compare_slots(left: &GridSlot, right: &GridSlot, before: &Bounds) -> Ordering {
    slot_distance(&left.bounds, before)
        .cmp(&slot_distance(&right.bounds, before))
        .then_with(|| {
            right
                .bounds
                .cx
                .saturating_mul(right.bounds.cy)
                .cmp(&left.bounds.cx.saturating_mul(left.bounds.cy))
        })
        .then_with(|| left.name.cmp(&right.name))
}

fn slot_distance(candidate: &Bounds, before: &Bounds) -> i128 {
    let dx = i128::from(candidate.x) * 2 + i128::from(candidate.cx)
        - i128::from(before.x) * 2
        - i128::from(before.cx);
    let dy = i128::from(candidate.y) * 2 + i128::from(candidate.cy)
        - i128::from(before.y) * 2
        - i128::from(before.cy);
    dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
}

fn collision_percentage(left: &LayoutShape, right: &LayoutShape) -> Option<f64> {
    let left = left.bounds.as_ref()?;
    let right = right.bounds.as_ref()?;
    if same_bounds(left, right) || !bounds_intersect(left, right) {
        return None;
    }
    let overlap_width = (left.x + left.cx).min(right.x + right.cx) - left.x.max(right.x);
    let overlap_height = (left.y + left.cy).min(right.y + right.cy) - left.y.max(right.y);
    let smaller = left
        .cx
        .saturating_mul(left.cy)
        .min(right.cx.saturating_mul(right.cy));
    if smaller <= 0 {
        return None;
    }
    let percentage = overlap_width.saturating_mul(overlap_height) as f64 / smaller as f64 * 100.0;
    (percentage >= 5.0).then_some(percentage)
}

fn is_cli_created_shape(shape: &LayoutShape) -> bool {
    if shape.is_placeholder || !matches!(shape.kind.as_str(), "sp" | "pic" | "graphicFrame") {
        return false;
    }
    let Some((prefix, suffix)) = shape.name.rsplit_once(' ') else {
        return false;
    };
    suffix.parse::<u32>().is_ok()
        && matches!(
            prefix.to_ascii_lowercase().as_str(),
            "chart" | "table" | "picture" | "textbox" | "text box"
        )
}

fn actions_json(actions: &[FixAction]) -> Value {
    Value::Array(actions.iter().map(action_json).collect())
}

fn action_json(action: &FixAction) -> Value {
    let mut value = json!({
        "action": action.action,
        "reason": action.reason,
        "slide": action.slide,
        "partUri": format!("/{}", action.slide_part),
        "shapeId": action.shape_id,
        "shapeName": action.shape_name,
    });
    match &action.change {
        FixChange::Bounds {
            before,
            after,
            slot,
        } => {
            value["beforeBounds"] = bounds_value(before);
            value["afterBounds"] = bounds_value(after);
            if let Some(slot) = slot {
                value["slot"] = json!(slot);
            }
        }
        FixChange::Font {
            bounds,
            before_points,
            after_points,
        } => {
            value["beforeBounds"] = bounds_value(bounds);
            value["afterBounds"] = bounds_value(bounds);
            value["beforeFontSizePoints"] = json_number(*before_points);
            value["afterFontSizePoints"] = json_number(*after_points);
        }
    }
    value
}

fn apply_actions_to_parts(
    file: &str,
    actions: &[FixAction],
) -> CliResult<BTreeMap<String, String>> {
    let mut overrides = BTreeMap::new();
    for action in actions {
        let mut xml = overrides
            .remove(&action.slide_part)
            .map_or_else(|| zip_text(file, &action.slide_part), Ok)?;
        let span = find_shape_span_by_id(&xml, action.shape_id)?.ok_or_else(|| {
            CliError::unexpected(format!("shape:{} disappeared", action.shape_id))
        })?;
        let fragment = &xml[span.start..span.end];
        let updated = match &action.change {
            FixChange::Bounds { after, .. } => {
                set_shape_bounds_fragment(fragment, &span.kind, after)?
            }
            FixChange::Font { after_points, .. } => cap_shape_font_size(fragment, *after_points)?,
        };
        xml.replace_range(span.start..span.end, &updated);
        overrides.insert(action.slide_part.clone(), xml);
    }
    Ok(overrides)
}

#[derive(Clone)]
struct ShapeSpan {
    start: usize,
    end: usize,
    kind: String,
}

fn find_shape_span_by_id(xml: &str, shape_id: i64) -> CliResult<Option<ShapeSpan>> {
    let Some(sp_tree) = find_first_element_span(xml, "spTree")? else {
        return Err(CliError::unexpected("shape tree not found in slide"));
    };
    let (content_start, content_end) = element_content_bounds(&xml[sp_tree.start..sp_tree.end])?;
    for shape in xml_direct_child_ranges(
        xml,
        sp_tree.start + content_start,
        sp_tree.start + content_end,
    )?
    .into_iter()
    .filter(|shape| matches!(shape.kind.as_str(), "sp" | "pic" | "graphicFrame" | "grpSp"))
    {
        let fragment = &xml[shape.start..shape.end];
        if first_cnvpr_id(fragment) == Some(shape_id) {
            return Ok(Some(ShapeSpan {
                start: shape.start,
                end: shape.end,
                kind: shape.kind,
            }));
        }
    }
    Ok(None)
}

fn first_cnvpr_id(fragment: &str) -> Option<i64> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if local_name(element.name().as_ref()) == "cNvPr" =>
            {
                return attr(&element, "id").and_then(|value| value.parse().ok());
            }
            Ok(Event::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
    }
}

#[derive(Clone)]
struct XmlSpan {
    start: usize,
    end: usize,
}

fn find_first_element_span(xml: &str, wanted: &str) -> CliResult<Option<XmlSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut active: Option<(usize, usize)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                if let Some((_, depth)) = active.as_mut() {
                    *depth += 1;
                } else if local_name(element.name().as_ref()) == wanted {
                    active = Some((before, 1));
                }
            }
            Ok(Event::Empty(element)) => {
                if active.is_none() && local_name(element.name().as_ref()) == wanted {
                    return Ok(Some(XmlSpan {
                        start: before,
                        end: reader.buffer_position() as usize,
                    }));
                }
            }
            Ok(Event::End(element)) => {
                if let Some((start, depth)) = active.as_mut() {
                    if *depth == 1 && local_name(element.name().as_ref()) == wanted {
                        return Ok(Some(XmlSpan {
                            start: *start,
                            end: reader.buffer_position() as usize,
                        }));
                    }
                    *depth = depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => return Ok(None),
            Err(error) => return Err(CliError::unexpected(error.to_string())),
            _ => {}
        }
    }
}

fn element_content_bounds(fragment: &str) -> CliResult<(usize, usize)> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    if fragment[..=open_end].trim_end().ends_with("/>") {
        return Ok((open_end + 1, open_end + 1));
    }
    let close_start = fragment
        .rfind("</")
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    Ok((open_end + 1, close_start))
}

fn set_shape_bounds_fragment(fragment: &str, kind: &str, bounds: &Bounds) -> CliResult<String> {
    if kind == "graphicFrame" {
        if let Some(xfrm) = find_first_element_span(fragment, "xfrm")? {
            let updated = set_xfrm_bounds(&fragment[xfrm.start..xfrm.end], bounds)?;
            return Ok(replace_span(fragment, &xfrm, &updated));
        }
        let insert_at = fragment
            .find('>')
            .ok_or_else(|| CliError::unexpected("invalid PPTX shape XML"))?
            + 1;
        return Ok(insert_at_index(
            fragment,
            insert_at,
            &xfrm_xml(bounds, "p:xfrm"),
        ));
    }
    if kind == "grpSp" {
        let group_properties = find_first_element_span(fragment, "grpSpPr")?
            .ok_or_else(|| CliError::unexpected("group shape properties not found"))?;
        let properties_xml = &fragment[group_properties.start..group_properties.end];
        let transform = find_first_element_span(properties_xml, "xfrm")?
            .ok_or_else(|| CliError::unexpected("group shape transform not found"))?;
        let updated_transform =
            set_xfrm_bounds(&properties_xml[transform.start..transform.end], bounds)?;
        let updated_properties = replace_span(properties_xml, &transform, &updated_transform);
        return Ok(replace_span(
            fragment,
            &group_properties,
            &updated_properties,
        ));
    }
    let Some(sp_pr) = find_first_element_span(fragment, "spPr")? else {
        let close = fragment
            .rfind("</")
            .ok_or_else(|| CliError::unexpected("invalid PPTX shape XML"))?;
        return Ok(insert_at_index(
            fragment,
            close,
            &format!("<p:spPr>{}</p:spPr>", xfrm_xml(bounds, "a:xfrm")),
        ));
    };
    let sp_pr_fragment = &fragment[sp_pr.start..sp_pr.end];
    if is_self_closing(sp_pr_fragment) {
        let expanded = expand_self_closing(sp_pr_fragment, &xfrm_xml(bounds, "a:xfrm"))?;
        return Ok(replace_span(fragment, &sp_pr, &expanded));
    }
    let updated_sp_pr = if let Some(xfrm) = find_first_element_span(sp_pr_fragment, "xfrm")? {
        let updated = set_xfrm_bounds(&sp_pr_fragment[xfrm.start..xfrm.end], bounds)?;
        replace_span(sp_pr_fragment, &xfrm, &updated)
    } else {
        let insert_at = sp_pr_fragment
            .find('>')
            .ok_or_else(|| CliError::unexpected("invalid PPTX shape properties XML"))?
            + 1;
        insert_at_index(sp_pr_fragment, insert_at, &xfrm_xml(bounds, "a:xfrm"))
    };
    Ok(replace_span(fragment, &sp_pr, &updated_sp_pr))
}

fn set_xfrm_bounds(fragment: &str, bounds: &Bounds) -> CliResult<String> {
    let mut updated = if is_self_closing(fragment) {
        expand_self_closing(fragment, "")?
    } else {
        fragment.to_string()
    };
    updated = replace_or_insert_child(
        &updated,
        "off",
        &format!(r#"<a:off x="{}" y="{}"/>"#, bounds.x, bounds.y),
    )?;
    replace_or_insert_child(
        &updated,
        "ext",
        &format!(r#"<a:ext cx="{}" cy="{}"/>"#, bounds.cx, bounds.cy),
    )
}

fn replace_or_insert_child(fragment: &str, local: &str, replacement: &str) -> CliResult<String> {
    if let Some(span) = find_first_element_span(fragment, local)? {
        return Ok(replace_span(fragment, &span, replacement));
    }
    let close = fragment
        .rfind("</")
        .ok_or_else(|| CliError::unexpected("invalid PPTX transform XML"))?;
    Ok(insert_at_index(fragment, close, replacement))
}

fn xfrm_xml(bounds: &Bounds, tag: &str) -> String {
    format!(
        r#"<{tag}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></{tag}>"#,
        bounds.x, bounds.y, bounds.cx, bounds.cy
    )
}

fn cap_shape_font_size(fragment: &str, points: f64) -> CliResult<String> {
    if !points.is_finite() || points < MIN_FONT_POINTS {
        return Err(CliError::unexpected("invalid planned font size"));
    }
    let hundredths = (points * 100.0).round() as i64;
    let rewritten = rewrite_existing_font_size_tags(fragment, hundredths);
    Ok(insert_missing_run_properties(&rewritten, hundredths))
}

fn rewrite_existing_font_size_tags(xml: &str, cap: i64) -> String {
    let mut output = String::with_capacity(xml.len());
    let mut cursor = 0;
    while let Some(relative) = xml[cursor..].find('<') {
        let start = cursor + relative;
        output.push_str(&xml[cursor..start]);
        let Some(relative_end) = xml[start..].find('>') else {
            output.push_str(&xml[start..]);
            return output;
        };
        let end = start + relative_end + 1;
        let tag = &xml[start..end];
        let local = opening_tag_local_name(tag);
        if matches!(local, Some("rPr" | "defRPr" | "endParaRPr")) {
            output.push_str(&cap_open_tag_size(tag, cap));
        } else {
            output.push_str(tag);
        }
        cursor = end;
    }
    output.push_str(&xml[cursor..]);
    output
}

fn insert_missing_run_properties(xml: &str, size: i64) -> String {
    let mut output = String::with_capacity(xml.len());
    let mut cursor = 0;
    while let Some(relative) = xml[cursor..].find('<') {
        let start = cursor + relative;
        output.push_str(&xml[cursor..start]);
        let Some(relative_end) = xml[start..].find('>') else {
            output.push_str(&xml[start..]);
            return output;
        };
        let end = start + relative_end + 1;
        let tag = &xml[start..end];
        output.push_str(tag);
        if matches!(opening_tag_local_name(tag), Some("r" | "fld"))
            && !tag.trim_end().ends_with("/>")
        {
            let remainder = xml[end..].trim_start();
            let next_local = remainder
                .find('>')
                .and_then(|next_end| opening_tag_local_name(&remainder[..=next_end]));
            if next_local != Some("rPr") {
                let prefix = opening_tag_prefix(tag).unwrap_or("a");
                output.push_str(&format!("<{prefix}:rPr sz=\"{size}\"/>"));
            }
        }
        cursor = end;
    }
    output.push_str(&xml[cursor..]);
    output
}

fn opening_tag_local_name(tag: &str) -> Option<&str> {
    let name = tag
        .strip_prefix('<')?
        .trim_start()
        .split([' ', '>', '/'])
        .next()?;
    if name.starts_with(['/', '!', '?']) {
        return None;
    }
    Some(name.rsplit(':').next().unwrap_or(name))
}

fn opening_tag_prefix(tag: &str) -> Option<&str> {
    let name = tag
        .strip_prefix('<')?
        .trim_start()
        .split([' ', '>', '/'])
        .next()?;
    name.split_once(':').map(|(prefix, _)| prefix)
}

fn cap_open_tag_size(tag: &str, cap: i64) -> String {
    if let Some((value_start, value_end)) = attribute_value_span(tag, "sz") {
        let current = tag[value_start..value_end].parse::<i64>().unwrap_or(cap);
        let replacement = current.min(cap).to_string();
        let mut output = String::with_capacity(tag.len());
        output.push_str(&tag[..value_start]);
        output.push_str(&replacement);
        output.push_str(&tag[value_end..]);
        return output;
    }
    let insert_at = tag
        .rfind("/>")
        .or_else(|| tag.rfind('>'))
        .unwrap_or(tag.len());
    let mut output = String::with_capacity(tag.len() + 16);
    output.push_str(&tag[..insert_at]);
    output.push_str(&format!(" sz=\"{cap}\""));
    output.push_str(&tag[insert_at..]);
    output
}

fn attribute_value_span(tag: &str, wanted: &str) -> Option<(usize, usize)> {
    let bytes = tag.as_bytes();
    let mut index = 1;
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'>' {
        index += 1;
    }
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let name_start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && !matches!(bytes[index], b'=' | b'>' | b'/')
        {
            index += 1;
        }
        let name = &tag[name_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            index += 1;
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let quote = *bytes.get(index)?;
        if !matches!(quote, b'\'' | b'"') {
            continue;
        }
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if name.rsplit(':').next() == Some(wanted) {
            return Some((value_start, index));
        }
        index += 1;
    }
    None
}

fn is_self_closing(fragment: &str) -> bool {
    fragment
        .find('>')
        .is_some_and(|index| fragment[..=index].trim_end().ends_with("/>"))
}

fn expand_self_closing(fragment: &str, content: &str) -> CliResult<String> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    let open_tag = &fragment[..=open_end];
    let slash = open_tag
        .rfind('/')
        .ok_or_else(|| CliError::unexpected("invalid self-closing PPTX XML"))?;
    let start = open_tag[..slash].trim_end();
    let tag = start
        .trim_start()
        .strip_prefix('<')
        .and_then(|name| name.split_whitespace().next())
        .ok_or_else(|| CliError::unexpected("invalid self-closing PPTX XML"))?;
    Ok(format!("{start}>{content}</{tag}>"))
}

fn replace_span(xml: &str, span: &XmlSpan, replacement: &str) -> String {
    let mut output = String::with_capacity(xml.len() - (span.end - span.start) + replacement.len());
    output.push_str(&xml[..span.start]);
    output.push_str(replacement);
    output.push_str(&xml[span.end..]);
    output
}

fn insert_at_index(xml: &str, index: usize, insertion: &str) -> String {
    let mut output = String::with_capacity(xml.len() + insertion.len());
    output.push_str(&xml[..index]);
    output.push_str(insertion);
    output.push_str(&xml[index..]);
    output
}

fn same_bounds(left: &Bounds, right: &Bounds) -> bool {
    left.x == right.x && left.y == right.y && left.cx == right.cx && left.cy == right.cy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_cap_rewrites_existing_sizes_without_increasing_smaller_runs() {
        let xml = r#"<p:sp><p:txBody><a:p><a:pPr><a:defRPr sz="2000"/></a:pPr><a:r><a:rPr sz="1000"/><a:t>A</a:t></a:r><a:r><a:t>B</a:t></a:r></a:p></p:txBody></p:sp>"#;
        let updated = cap_shape_font_size(xml, 16.0).expect("font cap");
        assert!(updated.contains(r#"<a:defRPr sz="1600"/>"#), "{updated}");
        assert!(updated.contains(r#"<a:rPr sz="1000"/>"#));
        assert!(updated.contains(r#"<a:r><a:rPr sz="1600"/><a:t>B"#));
    }

    #[test]
    fn layout_grid_selection_is_stable() {
        let slots = layout_grid_slots(
            crate::cli_dispatch::pptx_slots::SlotBounds {
                x: 457_200,
                y: 1_371_600,
                cx: 8_229_600,
                cy: 4_800_600,
            },
            9_144_000,
            6_858_000,
        );
        assert_eq!(
            slots.first().map(|slot| slot.name.as_str()),
            Some("layout-grid:1x1:1")
        );
        assert_eq!(slots.len(), 200);
    }
}
