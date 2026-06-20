use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Number, Value, json};

use crate::{
    CliError, CliResult, attr, attr_exact, local_name, package_type, relationships,
    resolve_relationship_target, zip_text,
};

const DEFAULT_SLIDE_WIDTH: i64 = 9_144_000;
const DEFAULT_SLIDE_HEIGHT: i64 = 6_858_000;

#[derive(Clone, Default)]
struct Bounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

#[derive(Default)]
struct LayoutShape {
    id: i64,
    name: String,
    bounds: Option<Bounds>,
    text: Option<TextBlock>,
}

#[derive(Default)]
struct TextBlock {
    paragraphs: Vec<Paragraph>,
    plain_text: String,
    top_inset: Option<i64>,
    bottom_inset: Option<i64>,
}

#[derive(Default)]
struct Paragraph {
    text: String,
    font_sizes: Vec<f64>,
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
        slide_reports.push(analyze_slide(
            index,
            &parse_layout_shapes(&slide_xml),
            slide_width,
            slide_height,
        ));
    }

    let total_slides = slide_reports.len();
    let mut slides_with_issues = 0;
    let mut slides_with_high_density = 0;
    let mut total_density = 0.0_f64;
    let mut total_text_overflows = 0;
    let mut total_collisions = 0;
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
        "totalTextOverflows": total_text_overflows,
        "totalCollisions": total_collisions,
        "hasIssues": slides_with_issues > 0,
    }))
}

fn analyze_slide(
    slide_index: usize,
    shapes: &[LayoutShape],
    slide_width: i64,
    slide_height: i64,
) -> Value {
    let text_overflows = shapes
        .iter()
        .filter_map(text_overflow_json)
        .collect::<Vec<_>>();
    let collisions = shape_collisions_json(shapes);
    let issue_count = text_overflows.len() + collisions.len();
    let mut report = Map::new();
    report.insert("slideIndex".to_string(), json!(slide_index));
    report.insert("slideNumber".to_string(), json!(slide_index + 1));
    if !text_overflows.is_empty() {
        report.insert("textOverflows".to_string(), Value::Array(text_overflows));
    }
    if !collisions.is_empty() {
        report.insert("collisions".to_string(), Value::Array(collisions));
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

fn shape_collisions_json(shapes: &[LayoutShape]) -> Vec<Value> {
    let mut collisions = Vec::new();
    for i in 0..shapes.len() {
        for j in i + 1..shapes.len() {
            if let Some(collision) = collision_json(&shapes[i], &shapes[j]) {
                collisions.push(collision);
            }
        }
    }
    collisions
}

fn collision_json(shape1: &LayoutShape, shape2: &LayoutShape) -> Option<Value> {
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
    Some(json!({
        "shapeId1": shape1.id,
        "shapeName1": shape1.name,
        "shapeId2": shape2.id,
        "shapeName2": shape2.name,
        "severity": severity,
        "overlapArea": overlap_area,
        "overlapPercentageOfSmaller": json_number(overlap_percentage),
        "shape1Area": area1,
        "shape2Area": area2,
        "isIdenticalBounds": false,
        "reason": "Shapes have overlapping bounding boxes",
    }))
}

fn text_overflow_json(shape: &LayoutShape) -> Option<Value> {
    let text = shape.text.as_ref()?;
    if text.paragraphs.is_empty() {
        return None;
    }
    let bounds = shape.bounds.as_ref()?;
    if bounds.cy <= 0 {
        return None;
    }

    let mut available_height = bounds.cy;
    if let Some(inset) = text.top_inset {
        available_height -= inset;
    }
    if let Some(inset) = text.bottom_inset {
        available_height -= inset;
    }

    let max_font_size = text
        .paragraphs
        .iter()
        .flat_map(|paragraph| paragraph.font_sizes.iter().copied())
        .fold(18.0_f64, f64::max);
    let line_height = (max_font_size * 12_700.0 * 1.3).round() as i64;
    let total_lines = estimate_line_count(&text.paragraphs);
    let estimated_height = line_height * total_lines as i64;
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
    Some(json!({
        "shapeId": shape.id,
        "shapeName": shape.name,
        "severity": severity,
        "estimatedTextHeight": estimated_height,
        "availableHeight": available_height,
        "overflowAmount": overflow_amount,
        "textLength": text.plain_text.len(),
        "paragraphCount": text.paragraphs.len(),
        "averageLineHeight": line_height,
        "reason": format!(
            "Text requires ~{estimated_height} EMU height but only {available_height} available ({overflow_amount} EMU overflow)"
        ),
    }))
}

fn estimate_line_count(paragraphs: &[Paragraph]) -> usize {
    let mut total = 0_usize;
    for paragraph in paragraphs {
        if paragraph.text.is_empty() {
            continue;
        }
        total += 1 + paragraph.text.len() / 40;
    }
    total.max(1)
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
                    current = Some(LayoutShape::default());
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
                    let mut shape = LayoutShape::default();
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
            Ok(Event::Text(e)) if in_text => {
                if let Some(paragraph) = current_paragraph.as_mut() {
                    paragraph
                        .text
                        .push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_text = false;
                }
                if in_tx_body && name == "p" {
                    if let Some(paragraph) = current_paragraph.take()
                        && let Some(shape) = current.as_mut()
                    {
                        let text = shape.text.get_or_insert_with(TextBlock::default);
                        text.paragraphs.push(paragraph);
                    }
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
            "off" => apply_off(shape, e),
            "ext" => apply_ext(shape, e),
            "txBody" => {
                *in_tx_body = true;
                shape.text.get_or_insert_with(TextBlock::default);
            }
            "bodyPr" if *in_tx_body => apply_body_pr(shape, e),
            "p" if *in_tx_body => *current_paragraph = Some(Paragraph::default()),
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
            "defRPr" | "rPr" if *in_tx_body => apply_font_size(current_paragraph, e),
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
        "off" => apply_off(shape, e),
        "ext" => apply_ext(shape, e),
        "txBody" => {
            *in_tx_body = false;
            shape.text.get_or_insert_with(TextBlock::default);
        }
        "bodyPr" if *in_tx_body => apply_body_pr(shape, e),
        "p" if *in_tx_body => {
            let text = shape.text.get_or_insert_with(TextBlock::default);
            text.paragraphs.push(Paragraph::default());
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
        "defRPr" | "rPr" if *in_tx_body => apply_font_size(current_paragraph, e),
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

fn apply_off(shape: &mut LayoutShape, e: &BytesStart<'_>) {
    let bounds = shape.bounds.get_or_insert_with(Bounds::default);
    bounds.x = attr(e, "x")
        .and_then(|value| value.parse().ok())
        .unwrap_or(bounds.x);
    bounds.y = attr(e, "y")
        .and_then(|value| value.parse().ok())
        .unwrap_or(bounds.y);
}

fn apply_ext(shape: &mut LayoutShape, e: &BytesStart<'_>) {
    let bounds = shape.bounds.get_or_insert_with(Bounds::default);
    bounds.cx = attr(e, "cx")
        .and_then(|value| value.parse().ok())
        .unwrap_or(bounds.cx);
    bounds.cy = attr(e, "cy")
        .and_then(|value| value.parse().ok())
        .unwrap_or(bounds.cy);
}

fn apply_body_pr(shape: &mut LayoutShape, e: &BytesStart<'_>) {
    let text = shape.text.get_or_insert_with(TextBlock::default);
    text.top_inset = attr(e, "tIns").and_then(|value| value.parse().ok());
    text.bottom_inset = attr(e, "bIns").and_then(|value| value.parse().ok());
}

fn apply_font_size(current_paragraph: &mut Option<Paragraph>, e: &BytesStart<'_>) {
    let Some(paragraph) = current_paragraph.as_mut() else {
        return;
    };
    if let Some(size) = attr(e, "sz").and_then(|value| value.parse::<f64>().ok()) {
        paragraph.font_sizes.push(size / 100.0);
    }
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
