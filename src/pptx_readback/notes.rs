use serde_json::{Map, Value, json};

use super::slide_parts::{PptxSlidePartRef, pptx_slide_part_refs};
use super::{pptx_shape_models, pptx_text_block_from_paragraphs, slide_layout_and_notes_parts};
use crate::{CliError, CliResult, package_type, parse_u32_flags, zip_text};
pub(crate) fn pptx_extract_notes(file: &str, args: &[String]) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }

    let selected_slides = parse_u32_flags(args, "--slide")?;
    let slides = pptx_slide_part_refs(file)?;
    let mut notes = Vec::new();
    if selected_slides.is_empty() {
        for slide in &slides {
            notes.push(pptx_notes_report(file, slide)?);
        }
    } else {
        for slide_number in selected_slides {
            if slide_number == 0 || slide_number as usize > slides.len() {
                return Err(CliError::invalid_args(format!(
                    "slide number {slide_number} is out of range (1-{})",
                    slides.len()
                )));
            }
            notes.push(pptx_notes_report(file, &slides[slide_number as usize - 1])?);
        }
    }
    Ok(json!({
        "file": file,
        "notes": notes,
    }))
}

pub(crate) fn pptx_notes_show(file: &str, slide: u32) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    if slide == 0 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let slides = pptx_slide_part_refs(file)?;
    let index = slide as usize - 1;
    let slide_ref = slides.get(index).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide {slide} not found (presentation has {} slides)",
            slides.len()
        ))
    })?;
    pptx_notes_report(file, slide_ref)
}

fn pptx_notes_report(file: &str, slide: &PptxSlidePartRef) -> CliResult<Value> {
    let mut report = Map::new();
    report.insert(
        "id".to_string(),
        json!(format!("slide{}-notes", slide.number)),
    );
    report.insert("slide".to_string(), json!(slide.number));
    let (_layout_part, notes_part) = slide_layout_and_notes_parts(file, &slide.part)?;
    let notes = if let Some(part) = notes_part {
        report.insert("partUri".to_string(), json!(format!("/{part}")));
        match zip_text(file, &part) {
            Ok(xml) => pptx_notes_text_block(&xml),
            Err(_) => pptx_empty_notes_block(),
        }
    } else {
        pptx_empty_notes_block()
    };
    report.insert("notes".to_string(), notes);
    Ok(Value::Object(report))
}

fn pptx_empty_notes_block() -> Value {
    json!({
        "paragraphs": [],
        "plainText": "",
    })
}

fn pptx_notes_text_block(xml: &str) -> Value {
    let Some(shape) = pptx_shape_models(xml).into_iter().find(|shape| {
        shape.kind == "sp"
            && shape
                .placeholder
                .as_ref()
                .is_some_and(|placeholder| placeholder.literal_type == "body")
    }) else {
        return pptx_empty_notes_block();
    };
    pptx_text_block_from_paragraphs(&shape.paragraphs, true, false)
}
