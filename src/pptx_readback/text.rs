use super::{
    Shape, normalize_ppt_target, pptx_selector_targets_from_shapes, pptx_shape_models,
    pptx_slide_refs,
};
use crate::{
    CliError, CliResult, json_u32, package_type, parse_u32_flags, relationships, zip_text,
};
use serde_json::{Map, Value, json};

pub(crate) fn pptx_extract_text_json_args(args: &Value) -> CliResult<Vec<String>> {
    let mut rest = Vec::new();
    if let Some(slide) = json_u32(args, "slide")? {
        rest.push("--slide".to_string());
        rest.push(slide.to_string());
    }
    if let Some(slides) = args.get("slides") {
        let values = slides
            .as_array()
            .ok_or_else(|| CliError::invalid_args("slides must be an array"))?;
        for value in values {
            let slide = if let Some(number) = value.as_u64() {
                u32::try_from(number)
                    .map_err(|_| CliError::invalid_args("slides entries must fit in uint32"))?
            } else if let Some(text) = value.as_str() {
                text.parse::<u32>().map_err(|_| {
                    CliError::invalid_args("slides entries must be integers or integer strings")
                })?
            } else {
                return Err(CliError::invalid_args(
                    "slides entries must be integers or integer strings",
                ));
            };
            rest.push("--slide".to_string());
            rest.push(slide.to_string());
        }
    }
    Ok(rest)
}

pub(crate) fn pptx_extract_text(file: &str, args: &[String]) -> CliResult<Value> {
    if package_type(file)? != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {})",
            package_type(file)?
        )));
    }

    let selected_slides = parse_u32_flags(args, "--slide")?;
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut values = Vec::new();
    for (index, (_slide_id, rel_id)) in slides.iter().enumerate() {
        let slide_number = index as u32 + 1;
        if !selected_slides.is_empty() && !selected_slides.contains(&slide_number) {
            continue;
        }
        let target = rels
            .get(rel_id)
            .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
        let part = normalize_ppt_target(target);
        let xml = zip_text(file, &part)?;
        values.push(json!({
            "slide": slide_number,
            "shapes": pptx_extract_text_shapes(&xml),
        }));
    }
    Ok(json!({
        "file": file,
        "slides": values,
    }))
}

fn pptx_extract_text_shapes(xml: &str) -> Vec<Value> {
    let shapes = pptx_shape_models(xml);
    let targets = pptx_selector_targets_from_shapes(&shapes);
    shapes
        .iter()
        .zip(targets)
        .filter(|(shape, _target)| shape.kind == "sp" && shape.has_text_body)
        .map(|(shape, target)| {
            let key = pptx_extract_text_shape_key(shape, &target);
            json!({
                "id": shape.id,
                "name": shape.name,
                "type": shape.kind,
                "key": key,
                "text": pptx_extract_text_body(shape),
            })
        })
        .collect()
}

fn pptx_extract_text_shape_key(shape: &Shape, target: &Value) -> String {
    let lower_name = shape.name.to_ascii_lowercase();
    if lower_name.contains("content placeholder")
        && let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index)
    {
        return format!("body:{index}");
    }
    target
        .get("primarySelector")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn pptx_extract_text_body(shape: &Shape) -> Value {
    pptx_text_block_from_paragraphs(&shape.paragraphs, true, true)
}

pub(super) fn pptx_text_block_from_paragraphs(
    paragraphs: &[Vec<String>],
    include_body_properties: bool,
    synthesize_empty_paragraph: bool,
) -> Value {
    let paragraphs = if paragraphs.is_empty() && synthesize_empty_paragraph {
        vec![Vec::<String>::new()]
    } else {
        paragraphs.to_vec()
    };
    let paragraph_values = paragraphs
        .iter()
        .map(|runs| {
            let text = runs.join("");
            let mut paragraph = Map::new();
            if !runs.is_empty() {
                paragraph.insert(
                    "runs".to_string(),
                    Value::Array(runs.iter().map(|run| json!({"text": run})).collect()),
                );
            }
            paragraph.insert("text".to_string(), json!(text));
            Value::Object(paragraph)
        })
        .collect::<Vec<_>>();
    let plain_text = paragraphs
        .iter()
        .map(|runs| runs.join(""))
        .collect::<Vec<_>>()
        .join("\n");
    let mut block = Map::new();
    block.insert("paragraphs".to_string(), Value::Array(paragraph_values));
    block.insert("plainText".to_string(), json!(plain_text));
    if include_body_properties {
        block.insert("bodyProperties".to_string(), json!({}));
    }
    Value::Object(block)
}

#[derive(Clone, Default)]
pub(super) struct ShapeText {
    pub(super) key: String,
    pub(super) name: String,
    pub(super) text: String,
}

pub(super) fn pptx_slide_texts(file: &str) -> CliResult<Vec<Vec<ShapeText>>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut out = Vec::new();
    for (_, rel_id) in slides {
        let target = rels
            .get(&rel_id)
            .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
        let part = normalize_ppt_target(target);
        let xml = zip_text(file, &part)?;
        out.push(
            pptx_shape_models(&xml)
                .into_iter()
                .filter(|shape| !shape.text.is_empty())
                .map(|shape| ShapeText {
                    key: shape_key(&shape),
                    name: shape.name,
                    text: shape.text,
                })
                .collect(),
        );
    }
    Ok(out)
}

fn shape_key(shape: &Shape) -> String {
    if shape.is_placeholder && shape.name.to_ascii_lowercase().contains("title") {
        "title".to_string()
    } else if !shape.name.is_empty() {
        shape.name.clone()
    } else {
        format!("shape:{}", shape.id)
    }
}
