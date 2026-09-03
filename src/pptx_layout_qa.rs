use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Number, Value, json};

use crate::pptx_readback::pptx_resolved_shape_models;
use crate::pptx_readback::shape_model::{Bounds, BoundsSource, Shape};
use crate::text_metrics::{ParagraphMeasure, TextAutofit, measure_text_box_with_autofit};
use crate::{
    CliError, CliResult, append_xml_text_event, attr, attr_exact, command_arg, is_xml_text_event,
    local_name, package_type, relationships, resolve_relationship_target, zip_text,
};

mod fix;

const DEFAULT_SLIDE_WIDTH: i64 = 9_144_000;
const DEFAULT_SLIDE_HEIGHT: i64 = 6_858_000;
const DEFAULT_SAFE_MARGIN: i64 = 228_600;

#[derive(Clone, Default)]
struct LayoutShape {
    id: i64,
    name: String,
    kind: String,
    is_placeholder: bool,
    bounds: Option<Bounds>,
    bounds_source: Option<BoundsSource>,
    text: Option<TextBlock>,
}

#[derive(Clone, Default)]
struct TextBlock {
    paragraphs: Vec<Paragraph>,
    plain_text: String,
    left_inset: Option<i64>,
    right_inset: Option<i64>,
    top_inset: Option<i64>,
    bottom_inset: Option<i64>,
    autofit: TextAutofit,
}

#[derive(Clone, Default)]
struct Paragraph {
    text: String,
    font_sizes: Vec<f64>,
    font_family: String,
    bold: bool,
    left_indent: i64,
    right_indent: i64,
    first_line_indent: i64,
    bullet: bool,
}

struct SlideContext<'a> {
    file: &'a str,
    slide: usize,
    width: i64,
    height: i64,
    safe_margin: i64,
}

pub(crate) fn pptx_validate_layout(file: &str) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }

    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let (slide_width, slide_height) = pptx_slide_size(&presentation);
    let slide_refs = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut slide_reports = Vec::new();
    for (index, (_, rel_id)) in slide_refs.iter().enumerate() {
        let Some(target) = rels.get(rel_id) else {
            return Err(CliError::unexpected(format!(
                "missing relationship {rel_id}"
            )));
        };
        let part = resolve_relationship_target("/ppt/presentation.xml", target);
        let slide_xml = zip_text(file, part.trim_start_matches('/'))?;
        let resolved_shapes = pptx_resolved_shape_models(file, &part, &slide_xml)?;
        let shapes = layout_shapes_from_resolved(&slide_xml, &resolved_shapes);
        slide_reports.push(analyze_slide(
            file,
            index,
            &shapes,
            slide_width,
            slide_height,
            DEFAULT_SAFE_MARGIN,
        ));
    }

    let total_slides = slide_reports.len();
    let mut slides_with_issues = 0;
    let mut slides_with_high_density = 0;
    let mut total_density = 0.0_f64;
    let mut total_text_overflows = 0;
    let mut total_collisions = 0;
    let mut total_off_slide = 0;
    let mut total_safe_margin_violations = 0;
    for report in &slide_reports {
        if report
            .get("hasIssues")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            slides_with_issues += 1;
        }
        if report
            .get("density")
            .and_then(|density| density.get("classification"))
            .and_then(Value::as_str)
            == Some("dense")
        {
            slides_with_high_density += 1;
        }
        total_density += report
            .get("density")
            .and_then(|density| density.get("densityPercentage"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        total_text_overflows += report
            .get("textOverflows")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        total_collisions += report
            .get("collisions")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        total_off_slide += report
            .get("offSlide")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        total_safe_margin_violations += report
            .get("safeMarginViolations")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
    }
    let average_density = if total_slides == 0 {
        0.0
    } else {
        total_density / total_slides as f64
    };

    Ok(json!({
        "file": file,
        "slideReports": slide_reports,
        "totalSlides": total_slides,
        "slidesWithIssues": slides_with_issues,
        "slidesWithHighDensity": slides_with_high_density,
        "averageDensity": json_number(average_density),
        "safeMargin": {
            "emu": DEFAULT_SAFE_MARGIN,
            "inches": 0.25,
        },
        "totalTextOverflows": total_text_overflows,
        "totalCollisions": total_collisions,
        "totalOffSlide": total_off_slide,
        "totalSafeMarginViolations": total_safe_margin_violations,
        "hasIssues": slides_with_issues > 0,
    }))
}

pub(crate) fn pptx_validate_layout_command(file: &str, args: &[String]) -> CliResult<Value> {
    if args.iter().any(|arg| arg == "--fix" || arg == "--out") {
        return fix::validate_layout_with_fix(file, args);
    }
    pptx_validate_layout(file)
}

fn analyze_slide(
    file: &str,
    slide_index: usize,
    shapes: &[LayoutShape],
    slide_width: i64,
    slide_height: i64,
    safe_margin: i64,
) -> Value {
    let slide_number = slide_index + 1;
    let context = SlideContext {
        file,
        slide: slide_number,
        width: slide_width,
        height: slide_height,
        safe_margin,
    };
    let text_overflows = shapes
        .iter()
        .filter_map(|shape| text_overflow_json(&context, shape))
        .collect::<Vec<_>>();
    let collisions = shape_collisions_json(&context, shapes);
    let off_slide = shapes
        .iter()
        .filter_map(|shape| off_slide_json(&context, shape))
        .collect::<Vec<_>>();
    let safe_margin_violations = shapes
        .iter()
        .filter(|shape| !is_off_slide(shape, slide_width, slide_height))
        .filter_map(|shape| safe_margin_json(&context, shape))
        .collect::<Vec<_>>();
    let issue_count =
        text_overflows.len() + collisions.len() + off_slide.len() + safe_margin_violations.len();
    let mut report = Map::new();
    report.insert("slideIndex".to_string(), json!(slide_index));
    report.insert("slideNumber".to_string(), json!(slide_index + 1));
    if !text_overflows.is_empty() {
        report.insert("textOverflows".to_string(), Value::Array(text_overflows));
    }
    if !collisions.is_empty() {
        report.insert("collisions".to_string(), Value::Array(collisions));
    }
    if !off_slide.is_empty() {
        report.insert("offSlide".to_string(), Value::Array(off_slide));
    }
    if !safe_margin_violations.is_empty() {
        report.insert(
            "safeMarginViolations".to_string(),
            Value::Array(safe_margin_violations),
        );
    }
    report.insert(
        "density".to_string(),
        density_json(shapes, slide_width, slide_height),
    );
    report.insert("hasIssues".to_string(), json!(issue_count > 0));
    report.insert("issueCount".to_string(), json!(issue_count));
    Value::Object(report)
}

fn density_json(shapes: &[LayoutShape], slide_width: i64, slide_height: i64) -> Value {
    let slide_area = slide_width * slide_height;
    let total_area = shapes
        .iter()
        .filter_map(|shape| shape.bounds.as_ref())
        .filter(|bounds| bounds.cx > 0 && bounds.cy > 0)
        .map(|bounds| bounds.cx * bounds.cy)
        .sum::<i64>();
    let mut density = if slide_area > 0 {
        total_area as f64 / slide_area as f64 * 100.0
    } else {
        0.0
    };
    if density > 100.0 {
        density = 100.0;
    }
    json!({
        "totalShapeArea": total_area,
        "slideArea": slide_area,
        "densityPercentage": json_number(density),
        "shapeCount": shapes.len(),
        "classification": density_classification(density),
    })
}

fn density_classification(density: f64) -> &'static str {
    if density < 5.0 {
        "empty"
    } else if density < 30.0 {
        "sparse"
    } else if density < 70.0 {
        "moderate"
    } else {
        "dense"
    }
}

fn shape_collisions_json(context: &SlideContext<'_>, shapes: &[LayoutShape]) -> Vec<Value> {
    let mut collisions = Vec::new();
    for i in 0..shapes.len() {
        for j in i + 1..shapes.len() {
            if let Some(collision) = collision_json(context, shapes, i, j) {
                collisions.push(collision);
            }
        }
    }
    collisions
}

fn collision_json(
    context: &SlideContext<'_>,
    shapes: &[LayoutShape],
    shape1_index: usize,
    shape2_index: usize,
) -> Option<Value> {
    let shape1 = &shapes[shape1_index];
    let shape2 = &shapes[shape2_index];
    let bounds1 = shape1.bounds.as_ref()?;
    let bounds2 = shape2.bounds.as_ref()?;
    let shape1_right = bounds1.x + bounds1.cx;
    let shape2_right = bounds2.x + bounds2.cx;
    let shape1_bottom = bounds1.y + bounds1.cy;
    let shape2_bottom = bounds2.y + bounds2.cy;
    if bounds1.x >= shape2_right
        || bounds2.x >= shape1_right
        || bounds1.y >= shape2_bottom
        || bounds2.y >= shape1_bottom
    {
        return None;
    }

    let overlap_left = bounds1.x.max(bounds2.x);
    let overlap_top = bounds1.y.max(bounds2.y);
    let overlap_right = shape1_right.min(shape2_right);
    let overlap_bottom = shape1_bottom.min(shape2_bottom);
    let overlap_area = (overlap_right - overlap_left) * (overlap_bottom - overlap_top);
    let identical = bounds1.x == bounds2.x
        && bounds1.y == bounds2.y
        && bounds1.cx == bounds2.cx
        && bounds1.cy == bounds2.cy;
    if identical {
        return None;
    }
    let area1 = bounds1.cx * bounds1.cy;
    let area2 = bounds2.cx * bounds2.cy;
    let smaller = area1.min(area2);
    let overlap_percentage = if smaller > 0 {
        overlap_area as f64 / smaller as f64 * 100.0
    } else {
        0.0
    };
    if overlap_percentage < 5.0 {
        return None;
    }
    let severity = if overlap_percentage > 50.0 {
        "high"
    } else if overlap_percentage > 20.0 {
        "medium"
    } else {
        "low"
    };
    let suggested = suggested_non_overlapping_bounds(
        shapes,
        shape2_index,
        bounds1,
        context.width,
        context.height,
        context.safe_margin,
    );
    Some(json!({
        "shapeId1": shape1.id,
        "shapeName1": shape1.name,
        "boundsSource1": shape1.bounds_source.map(BoundsSource::as_str),
        "shapeId2": shape2.id,
        "shapeName2": shape2.name,
        "boundsSource2": shape2.bounds_source.map(BoundsSource::as_str),
        "severity": severity,
        "overlapArea": overlap_area,
        "overlapPercentageOfSmaller": json_number(overlap_percentage),
        "shape1Area": area1,
        "shape2Area": area2,
        "isIdenticalBounds": false,
        "reason": "Shapes have overlapping bounding boxes",
        "suggestedBounds": bounds_value(&suggested),
        "fixCommand": set_bounds_fix_command(context.file, context.slide, shape2.id, &suggested),
    }))
}

fn text_overflow_json(context: &SlideContext<'_>, shape: &LayoutShape) -> Option<Value> {
    let text = shape.text.as_ref()?;
    if text.paragraphs.is_empty() {
        return None;
    }
    let bounds = shape.bounds.as_ref()?;
    if bounds.cy <= 0 {
        return None;
    }

    let paragraphs = text
        .paragraphs
        .iter()
        .map(|paragraph| ParagraphMeasure {
            text: &paragraph.text,
            font_family: if paragraph.font_family.is_empty() {
                "Aptos"
            } else {
                &paragraph.font_family
            },
            font_size_points: paragraph
                .font_sizes
                .iter()
                .copied()
                .fold(18.0_f64, f64::max),
            bold: paragraph.bold,
            left_indent_emu: paragraph.left_indent,
            right_indent_emu: paragraph.right_indent,
            first_line_indent_emu: paragraph.first_line_indent,
            bullet: paragraph.bullet,
            line_spacing: 1.0,
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
    let available_height = measurement.available_height_emu;
    let estimated_height = measurement.height_emu;
    let total_lines = measurement.line_count;
    let line_height = measurement
        .paragraphs
        .iter()
        .map(|paragraph| paragraph.line_height_emu)
        .max()
        .unwrap_or_default();
    let overflow_amount = estimated_height - available_height;
    if overflow_amount <= line_height / 2 {
        return None;
    }
    let severity = if overflow_amount > line_height * 3 {
        "high"
    } else if overflow_amount > 0 {
        "low"
    } else {
        "medium"
    };
    let mut suggested = bounds.clone();
    suggested.cy = estimated_height
        + text.top_inset.unwrap_or_default()
        + text.bottom_inset.unwrap_or_default();
    let suggested = fit_bounds_to_safe_area(
        &suggested,
        context.width,
        context.height,
        context.safe_margin,
    );
    Some(json!({
        "shapeId": shape.id,
        "shapeName": shape.name,
        "boundsSource": shape.bounds_source.map(BoundsSource::as_str),
        "severity": severity,
        "estimatedTextHeight": estimated_height,
        "estimatedLineCount": total_lines,
        "availableTextWidth": measurement.available_width_emu,
        "availableHeight": available_height,
        "overflowAmount": overflow_amount,
        "textLength": text.plain_text.len(),
        "paragraphCount": text.paragraphs.len(),
        "averageLineHeight": line_height,
        "metricDataVersion": 1,
        "autofitMode": measurement.autofit_mode,
        "effectiveFontScale": measurement.effective_font_scale,
        "fontSources": measurement.paragraphs.iter().map(|paragraph| paragraph.source_font_family.as_str()).collect::<Vec<_>>(),
        "reason": format!(
            "Text requires ~{estimated_height} EMU height but only {available_height} available ({overflow_amount} EMU overflow)"
        ),
        "suggestedBounds": bounds_value(&suggested),
        "fixCommand": set_bounds_fix_command(context.file, context.slide, shape.id, &suggested),
    }))
}

fn off_slide_json(context: &SlideContext<'_>, shape: &LayoutShape) -> Option<Value> {
    let bounds = shape.bounds.as_ref()?;
    let mut edges = Vec::new();
    if bounds.x < 0 {
        edges.push("left");
    }
    if bounds.y < 0 {
        edges.push("top");
    }
    if bounds.x + bounds.cx > context.width {
        edges.push("right");
    }
    if bounds.y + bounds.cy > context.height {
        edges.push("bottom");
    }
    if edges.is_empty() {
        return None;
    }
    let suggested =
        fit_bounds_to_safe_area(bounds, context.width, context.height, context.safe_margin);
    Some(json!({
        "shapeId": shape.id,
        "shapeName": shape.name,
        "boundsSource": shape.bounds_source.map(BoundsSource::as_str),
        "edges": edges,
        "severity": "high",
        "reason": "Shape extends beyond the slide canvas",
        "suggestedBounds": bounds_value(&suggested),
        "fixCommand": set_bounds_fix_command(context.file, context.slide, shape.id, &suggested),
    }))
}

fn safe_margin_json(context: &SlideContext<'_>, shape: &LayoutShape) -> Option<Value> {
    let bounds = shape.bounds.as_ref()?;
    let mut edges = Vec::new();
    if bounds.x < context.safe_margin {
        edges.push("left");
    }
    if bounds.y < context.safe_margin {
        edges.push("top");
    }
    if bounds.x + bounds.cx > context.width - context.safe_margin {
        edges.push("right");
    }
    if bounds.y + bounds.cy > context.height - context.safe_margin {
        edges.push("bottom");
    }
    if edges.is_empty() {
        return None;
    }
    let suggested =
        fit_bounds_to_safe_area(bounds, context.width, context.height, context.safe_margin);
    Some(json!({
        "shapeId": shape.id,
        "shapeName": shape.name,
        "boundsSource": shape.bounds_source.map(BoundsSource::as_str),
        "edges": edges,
        "marginEmu": context.safe_margin,
        "marginInches": json_number(context.safe_margin as f64 / 914_400.0),
        "severity": "medium",
        "reason": "Shape enters the configured safe-margin area",
        "suggestedBounds": bounds_value(&suggested),
        "fixCommand": set_bounds_fix_command(context.file, context.slide, shape.id, &suggested),
    }))
}

fn is_off_slide(shape: &LayoutShape, slide_width: i64, slide_height: i64) -> bool {
    shape.bounds.as_ref().is_some_and(|bounds| {
        bounds.x < 0
            || bounds.y < 0
            || bounds.x + bounds.cx > slide_width
            || bounds.y + bounds.cy > slide_height
    })
}

fn suggested_non_overlapping_bounds(
    shapes: &[LayoutShape],
    moving_index: usize,
    anchor: &Bounds,
    slide_width: i64,
    slide_height: i64,
    safe_margin: i64,
) -> Bounds {
    let moving = shapes[moving_index]
        .bounds
        .as_ref()
        .expect("collision shape has bounds");
    let right = slide_width - safe_margin;
    let bottom = slide_height - safe_margin;
    let (x, cx) = fit_axis(moving.x, moving.cx, safe_margin, right);
    let (y, cy) = fit_axis(moving.y, moving.cy, safe_margin, bottom);
    let gap = safe_margin;
    let candidates = [
        Bounds {
            x,
            y: safe_margin,
            cx,
            cy: moving.cy.min((anchor.y - gap - safe_margin).max(0)),
        },
        Bounds {
            x,
            y: (anchor.y + anchor.cy + gap).min(bottom),
            cx,
            cy: moving.cy.min((bottom - anchor.y - anchor.cy - gap).max(0)),
        },
        Bounds {
            x: safe_margin,
            y,
            cx: moving.cx.min((anchor.x - gap - safe_margin).max(0)),
            cy,
        },
        Bounds {
            x: (anchor.x + anchor.cx + gap).min(right),
            y,
            cx: moving.cx.min((right - anchor.x - anchor.cx - gap).max(0)),
            cy,
        },
    ];
    candidates
        .into_iter()
        .filter(|candidate| candidate.cx > 0 && candidate.cy > 0)
        .filter(|candidate| {
            shapes.iter().enumerate().all(|(index, shape)| {
                index == moving_index
                    || shape
                        .bounds
                        .as_ref()
                        .is_none_or(|bounds| !bounds_intersect(candidate, bounds))
            })
        })
        .max_by_key(|candidate| candidate.cx.saturating_mul(candidate.cy))
        .unwrap_or_else(|| fit_bounds_to_safe_area(moving, slide_width, slide_height, safe_margin))
}

fn fit_axis(start: i64, size: i64, minimum: i64, maximum: i64) -> (i64, i64) {
    let available = (maximum - minimum).max(0);
    let size = size.max(0).min(available);
    (start.clamp(minimum, maximum - size), size)
}

fn fit_bounds_to_safe_area(
    bounds: &Bounds,
    slide_width: i64,
    slide_height: i64,
    safe_margin: i64,
) -> Bounds {
    let (x, cx) = fit_axis(bounds.x, bounds.cx, safe_margin, slide_width - safe_margin);
    let (y, cy) = fit_axis(bounds.y, bounds.cy, safe_margin, slide_height - safe_margin);
    Bounds { x, y, cx, cy }
}

fn bounds_intersect(left: &Bounds, right: &Bounds) -> bool {
    left.x < right.x + right.cx
        && right.x < left.x + left.cx
        && left.y < right.y + right.cy
        && right.y < left.y + left.cy
}

fn bounds_value(bounds: &Bounds) -> Value {
    json!({
        "x": bounds.x,
        "y": bounds.y,
        "cx": bounds.cx,
        "cy": bounds.cy,
    })
}

fn set_bounds_fix_command(file: &str, slide: usize, shape_id: i64, bounds: &Bounds) -> String {
    format!(
        "ooxml --json pptx shapes set-bounds {} --slide {slide} --target shape:{shape_id} --bounds {},{},{},{} --out {}",
        command_arg(file),
        bounds.x,
        bounds.y,
        bounds.cx,
        bounds.cy,
        command_arg(&layout_fixed_path(file)),
    )
}

fn layout_fixed_path(file: &str) -> String {
    crate::design_check::fixed_output_path(file, "layout-fixed")
}

fn layout_shapes_from_resolved(xml: &str, resolved: &[Shape]) -> Vec<LayoutShape> {
    let mut text_shapes = parse_layout_shapes(xml);
    resolved
        .iter()
        .map(|shape| {
            let text = text_shapes
                .iter_mut()
                .find(|candidate| {
                    candidate.id == i64::from(shape.id) && candidate.name == shape.name
                })
                .and_then(|candidate| candidate.text.take());
            LayoutShape {
                id: i64::from(shape.id),
                name: shape.name.clone(),
                kind: text_shapes
                    .iter()
                    .find(|candidate| {
                        candidate.id == i64::from(shape.id) && candidate.name == shape.name
                    })
                    .map(|candidate| candidate.kind.clone())
                    .unwrap_or_default(),
                is_placeholder: text_shapes
                    .iter()
                    .find(|candidate| {
                        candidate.id == i64::from(shape.id) && candidate.name == shape.name
                    })
                    .is_some_and(|candidate| candidate.is_placeholder),
                bounds: shape.bounds.clone(),
                bounds_source: shape.bounds_source,
                text,
            }
        })
        .collect()
}

fn parse_layout_shapes(xml: &str) -> Vec<LayoutShape> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path = Vec::<String>::new();
    let mut current: Option<LayoutShape> = None;
    let mut current_kind = String::new();
    let mut current_depth = 0_usize;
    let mut in_tx_body = false;
    let mut current_paragraph: Option<Paragraph> = None;
    let mut in_text = false;
    let mut shapes = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && matches!(name.as_str(), "sp" | "pic" | "graphicFrame" | "grpSp")
                {
                    current = Some(LayoutShape {
                        kind: name.clone(),
                        ..LayoutShape::default()
                    });
                    current_kind.clone_from(&name);
                    current_depth = path.len() + 1;
                } else if current.is_some() {
                    parse_shape_start(
                        &e,
                        &name,
                        &mut current,
                        &mut in_tx_body,
                        &mut current_paragraph,
                    );
                }
                path.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && matches!(name.as_str(), "sp" | "pic" | "graphicFrame" | "grpSp")
                {
                    let mut shape = LayoutShape {
                        kind: name.clone(),
                        ..LayoutShape::default()
                    };
                    parse_shape_empty(
                        &e,
                        &name,
                        &mut shape,
                        &mut in_tx_body,
                        &mut current_paragraph,
                    );
                    shapes.push(shape);
                } else if let Some(shape) = current.as_mut() {
                    parse_shape_empty(&e, &name, shape, &mut in_tx_body, &mut current_paragraph);
                }
            }
            Ok(event) if in_text && is_xml_text_event(&event) => {
                if let Some(paragraph) = current_paragraph.as_mut() {
                    append_xml_text_event(&mut paragraph.text, &event);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_text = false;
                }
                if in_tx_body
                    && name == "p"
                    && let Some(paragraph) = current_paragraph.take()
                    && let Some(shape) = current.as_mut()
                {
                    let text = shape.text.get_or_insert_with(TextBlock::default);
                    text.paragraphs.push(paragraph);
                }
                if in_tx_body && name == "txBody" {
                    in_tx_body = false;
                    if let Some(shape) = current.as_mut()
                        && let Some(text) = shape.text.as_mut()
                    {
                        text.plain_text = text
                            .paragraphs
                            .iter()
                            .map(|paragraph| paragraph.text.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");
                    }
                }
                if current.is_some() && path.len() == current_depth && name == current_kind {
                    if let Some(shape) = current.take() {
                        shapes.push(shape);
                    }
                    current_kind.clear();
                    current_depth = 0;
                }
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        if matches!(path.last().map(String::as_str), Some("t")) {
            in_text = true;
        }
    }
    shapes
}

fn parse_shape_start(
    e: &BytesStart<'_>,
    name: &str,
    current: &mut Option<LayoutShape>,
    in_tx_body: &mut bool,
    current_paragraph: &mut Option<Paragraph>,
) {
    if let Some(shape) = current.as_mut() {
        match name {
            "cNvPr" => apply_cnvpr(shape, e),
            "ph" => shape.is_placeholder = true,
            "txBody" => {
                *in_tx_body = true;
                shape.text.get_or_insert_with(TextBlock::default);
            }
            "bodyPr" if *in_tx_body => apply_body_pr(shape, e),
            "spAutoFit" if *in_tx_body => apply_autofit(shape, TextAutofit::ResizeShape),
            "noAutofit" if *in_tx_body => apply_autofit(shape, TextAutofit::None),
            "normAutofit" if *in_tx_body => apply_normal_autofit(shape, e),
            "p" if *in_tx_body => *current_paragraph = Some(Paragraph::default()),
            "pPr" if *in_tx_body => apply_paragraph_properties(current_paragraph, e),
            "buChar" | "buAutoNum" if *in_tx_body => {
                if let Some(paragraph) = current_paragraph.as_mut() {
                    paragraph.bullet = true;
                }
            }
            "buNone" if *in_tx_body => {
                if let Some(paragraph) = current_paragraph.as_mut() {
                    paragraph.bullet = false;
                }
            }
            "br" if *in_tx_body => {
                if let Some(paragraph) = current_paragraph.as_mut() {
                    paragraph.text.push('\n');
                }
            }
            "tab" if *in_tx_body => {
                if let Some(paragraph) = current_paragraph.as_mut() {
                    paragraph.text.push('\t');
                }
            }
            "defRPr" | "rPr" if *in_tx_body => apply_run_properties(current_paragraph, e),
            "latin" if *in_tx_body => apply_font_family(current_paragraph, e),
            _ => {}
        }
    }
}

fn parse_shape_empty(
    e: &BytesStart<'_>,
    name: &str,
    shape: &mut LayoutShape,
    in_tx_body: &mut bool,
    current_paragraph: &mut Option<Paragraph>,
) {
    match name {
        "cNvPr" => apply_cnvpr(shape, e),
        "ph" => shape.is_placeholder = true,
        "txBody" => {
            *in_tx_body = false;
            shape.text.get_or_insert_with(TextBlock::default);
        }
        "bodyPr" if *in_tx_body => apply_body_pr(shape, e),
        "spAutoFit" if *in_tx_body => apply_autofit(shape, TextAutofit::ResizeShape),
        "noAutofit" if *in_tx_body => apply_autofit(shape, TextAutofit::None),
        "normAutofit" if *in_tx_body => apply_normal_autofit(shape, e),
        "p" if *in_tx_body => {
            let text = shape.text.get_or_insert_with(TextBlock::default);
            text.paragraphs.push(Paragraph::default());
        }
        "pPr" if *in_tx_body => apply_paragraph_properties(current_paragraph, e),
        "buChar" | "buAutoNum" if *in_tx_body => {
            if let Some(paragraph) = current_paragraph.as_mut() {
                paragraph.bullet = true;
            }
        }
        "buNone" if *in_tx_body => {
            if let Some(paragraph) = current_paragraph.as_mut() {
                paragraph.bullet = false;
            }
        }
        "br" if *in_tx_body => {
            if let Some(paragraph) = current_paragraph.as_mut() {
                paragraph.text.push('\n');
            }
        }
        "tab" if *in_tx_body => {
            if let Some(paragraph) = current_paragraph.as_mut() {
                paragraph.text.push('\t');
            }
        }
        "defRPr" | "rPr" if *in_tx_body => apply_run_properties(current_paragraph, e),
        "latin" if *in_tx_body => apply_font_family(current_paragraph, e),
        _ => {}
    }
}

fn apply_cnvpr(shape: &mut LayoutShape, e: &BytesStart<'_>) {
    if shape.id == 0 {
        shape.id = attr(e, "id")
            .and_then(|value| value.parse().ok())
            .unwrap_or_default();
    }
    if shape.name.is_empty() {
        shape.name = attr(e, "name").unwrap_or_default();
    }
}

fn apply_body_pr(shape: &mut LayoutShape, e: &BytesStart<'_>) {
    let text = shape.text.get_or_insert_with(TextBlock::default);
    text.left_inset = attr(e, "lIns").and_then(|value| value.parse().ok());
    text.right_inset = attr(e, "rIns").and_then(|value| value.parse().ok());
    text.top_inset = attr(e, "tIns").and_then(|value| value.parse().ok());
    text.bottom_inset = attr(e, "bIns").and_then(|value| value.parse().ok());
}

fn apply_autofit(shape: &mut LayoutShape, autofit: TextAutofit) {
    shape.text.get_or_insert_with(TextBlock::default).autofit = autofit;
}

fn apply_normal_autofit(shape: &mut LayoutShape, e: &BytesStart<'_>) {
    let font_scale = attr(e, "fontScale")
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value / 100_000.0)
        .unwrap_or(1.0);
    let line_spacing_reduction = attr(e, "lnSpcReduction")
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value / 100_000.0)
        .unwrap_or(0.0);
    apply_autofit(
        shape,
        TextAutofit::ShrinkText {
            font_scale,
            line_spacing_reduction,
        },
    );
}

fn apply_paragraph_properties(current_paragraph: &mut Option<Paragraph>, e: &BytesStart<'_>) {
    let Some(paragraph) = current_paragraph.as_mut() else {
        return;
    };
    paragraph.left_indent = attr(e, "marL")
        .and_then(|value| value.parse().ok())
        .unwrap_or(paragraph.left_indent);
    paragraph.right_indent = attr(e, "marR")
        .and_then(|value| value.parse().ok())
        .unwrap_or(paragraph.right_indent);
    paragraph.first_line_indent = attr(e, "indent")
        .and_then(|value| value.parse().ok())
        .unwrap_or(paragraph.first_line_indent);
}

fn apply_run_properties(current_paragraph: &mut Option<Paragraph>, e: &BytesStart<'_>) {
    let Some(paragraph) = current_paragraph.as_mut() else {
        return;
    };
    if let Some(size) = attr(e, "sz").and_then(|value| value.parse::<f64>().ok()) {
        paragraph.font_sizes.push(size / 100.0);
    }
    if attr(e, "b").as_deref().is_some_and(is_true_xml_value) {
        paragraph.bold = true;
    }
}

fn apply_font_family(current_paragraph: &mut Option<Paragraph>, e: &BytesStart<'_>) {
    let Some(paragraph) = current_paragraph.as_mut() else {
        return;
    };
    if let Some(typeface) = attr(e, "typeface")
        && !typeface.is_empty()
        && !typeface.starts_with('+')
    {
        paragraph.font_family = typeface;
    }
}

fn is_true_xml_value(value: &str) -> bool {
    value == "1" || value.eq_ignore_ascii_case("true")
}

fn pptx_slide_refs(xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let (Some(id), Some(rel)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    slides.push((id, rel));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

fn pptx_slide_size(xml: &str) -> (i64, i64) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldSz" =>
            {
                let cx = attr(&e, "cx")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(DEFAULT_SLIDE_WIDTH);
                let cy = attr(&e, "cy")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(DEFAULT_SLIDE_HEIGHT);
                return (cx, cy);
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    (DEFAULT_SLIDE_WIDTH, DEFAULT_SLIDE_HEIGHT)
}

fn json_number(value: f64) -> Value {
    if value.is_finite() && (value.fract().abs() < f64::EPSILON) {
        json!(value as i64)
    } else {
        Value::Number(Number::from_f64(value).unwrap_or_else(|| Number::from(0)))
    }
}

#[cfg(test)]
mod tests {
    use super::layout_fixed_path;

    #[test]
    fn layout_fixed_path_preserves_forward_slashes() {
        assert_eq!(
            layout_fixed_path(
                "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx"
            ),
            "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.layout-fixed.pptx"
        );
    }
}
