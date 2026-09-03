//! Narrow, deterministic parser boundaries used by the fuzz harnesses.
//!
//! This module deliberately contains no filesystem or process orchestration.

use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::build::{
    BuildFamily, MarkdownConversion, compile_docx_spec, compile_pptx_spec, compile_xlsx_spec,
    load_spec_bytes, markdown_to_spec,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputError {
    pub code: String,
    pub message: String,
}

impl InputError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

pub fn build_spec(source: &[u8]) -> Result<Value, InputError> {
    let (selector, source) = source.split_first().ok_or_else(|| {
        InputError::new(
            "FUZZ_INPUT_EMPTY",
            "build-spec fuzz input needs a family selector byte",
        )
    })?;
    let family = BuildFamily::ALL[usize::from(*selector) % BuildFamily::ALL.len()];
    let spec = load_spec_bytes(family, source).map_err(|error| {
        let first = error.diagnostics.first();
        InputError::new(
            first
                .map(|diagnostic| diagnostic.code.clone())
                .unwrap_or_else(|| "BUILD_SPEC_INVALID".to_string()),
            error.to_string(),
        )
    })?;
    let operations = match family {
        BuildFamily::Pptx => compile_pptx_spec(&spec).map(|build| build.plan.operations_json()),
        BuildFamily::Xlsx => compile_xlsx_spec(&spec).map(|build| build.plan.operations_json()),
        BuildFamily::Docx => compile_docx_spec(&spec).map(|build| build.plan.operations_json()),
    }
    .map_err(|error| {
        let message = error.to_string();
        InputError::new(error.code, message)
    })?;
    Ok(json!({
        "family": family,
        "document": spec.into_document(),
        "operations": operations,
    }))
}

pub fn markdown(source: &[u8]) -> Result<MarkdownConversion, InputError> {
    let (selector, source) = source.split_first().ok_or_else(|| {
        InputError::new(
            "FUZZ_INPUT_EMPTY",
            "Markdown fuzz input needs a family selector byte",
        )
    })?;
    let family = if selector % 2 == 0 {
        BuildFamily::Pptx
    } else {
        BuildFamily::Docx
    };
    let source = std::str::from_utf8(source).map_err(|error| {
        InputError::new(
            "MARKDOWN_INVALID_UTF8",
            format!("Markdown input must be UTF-8: {error}"),
        )
    })?;
    markdown_to_spec(family, source, "fuzz-input.md").map_err(|error| {
        let message = error.to_string();
        InputError::new(error.code, message)
    })
}

pub fn brand(source: &[u8]) -> Result<Value, InputError> {
    crate::brand::parse_brand_kit_bytes_for_fuzz(source)
        .map_err(|error| InputError::new(error.code, error.message))
}

pub fn refs(source: &[u8]) -> Result<Value, InputError> {
    let document: Value = serde_json::from_slice(source).map_err(|error| {
        InputError::new(
            "REF_INPUT_JSON_INVALID",
            format!("invalid $ref fuzz JSON: {error}"),
        )
    })?;
    let mut document = document.as_object().cloned().ok_or_else(|| {
        InputError::new(
            "REF_INPUT_INVALID",
            "$ref fuzz input must be an object with value and results fields",
        )
    })?;
    reject_unknown_fields(&document, &["value", "results"])?;
    let value = document
        .remove("value")
        .ok_or_else(|| InputError::new("REF_INPUT_INVALID", "$ref fuzz input is missing value"))?;
    let results = document.remove("results").ok_or_else(|| {
        InputError::new("REF_INPUT_INVALID", "$ref fuzz input is missing results")
    })?;
    let Value::Object(results) = results else {
        return Err(InputError::new(
            "REF_INPUT_INVALID",
            "$ref fuzz results must be an object keyed by operation id",
        ));
    };
    let results = results.into_iter().collect::<BTreeMap<_, _>>();
    crate::serve::resolve_refs(&value, &results)
        .map_err(|error| InputError::new(error.code, error.message))
}

pub fn image(source: &[u8]) -> Result<Value, InputError> {
    crate::image_pipeline::probe_image(source)
        .map(|probe| {
            json!({
                "nativeWidth": probe.native_width,
                "nativeHeight": probe.native_height,
                "orientedWidth": probe.oriented_width,
                "orientedHeight": probe.oriented_height,
                "exifOrientation": probe.exif_orientation,
            })
        })
        .map_err(|error| InputError::new(error.code, error.message))
}

fn reject_unknown_fields(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), InputError> {
    if let Some(field) = object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(InputError::new(
            "REF_INPUT_INVALID",
            format!("unknown $ref fuzz input field {field:?}"),
        ));
    }
    Ok(())
}
