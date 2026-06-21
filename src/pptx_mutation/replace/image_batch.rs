use std::collections::BTreeSet;

use crate::{CliError, CliResult};

pub(super) struct ImageBatchSelector {
    pub(super) normalized: String,
    pub(super) unsupported_error: Option<String>,
}

pub(super) fn parse_image_batch_selector(target_selector: &str) -> CliResult<ImageBatchSelector> {
    let trimmed = target_selector.trim();
    if trimmed.is_empty() {
        return Err(CliError::invalid_args(
            "invalid target selector: selector cannot be empty",
        ));
    }
    if let Some(raw) = trimmed.strip_prefix("shape:") {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(CliError::invalid_args(
                "invalid target selector: shape ID selector cannot be empty after 'shape:'",
            ));
        }
        let id = raw.parse::<i64>().map_err(|err| {
            CliError::invalid_args(format!("invalid target selector: invalid shape ID: {err}"))
        })?;
        if id < 0 {
            return Err(CliError::invalid_args(format!(
                "invalid target selector: shape ID must be non-negative, got {id}"
            )));
        }
        return Ok(ImageBatchSelector {
            normalized: format!("shape:{id}"),
            unsupported_error: None,
        });
    }
    if let Some(name) = trimmed.strip_prefix('~') {
        if name.is_empty() {
            return Err(CliError::invalid_args(
                "invalid target selector: shape name selector cannot be empty after ~",
            ));
        }
        return Ok(ImageBatchSelector {
            normalized: format!("~{name}"),
            unsupported_error: None,
        });
    }
    let unsupported_type = if trimmed.starts_with('@') {
        match trimmed.trim_start_matches('@').trim() {
            "*" | "all-placeholders" => "*selectors.WildcardAllPlaceholdersSelector",
            "all-shapes" | "all-shapes-nonph" => "*selectors.WildcardAllShapesSelector",
            "all-pictures" => "*selectors.WildcardAllPicturesSelector",
            "all-tables" => "*selectors.WildcardAllTablesSelector",
            _ => "*selectors.PlaceholderTypeSelector",
        }
    } else if trimmed.starts_with('#') {
        "*selectors.PlaceholderIndexSelector"
    } else if is_image_batch_slide_selector(trimmed) {
        if trimmed.contains(',') || trimmed.contains('-') {
            "*selectors.SlideRangeSelector"
        } else {
            "*selectors.SlideNumberSelector"
        }
    } else {
        "*selectors.PlaceholderKeySelector"
    };
    Ok(ImageBatchSelector {
        normalized: trimmed.to_string(),
        unsupported_error: Some(format!(
            "selector type {unsupported_type} is not supported for image replacement (use shape ID or name)"
        )),
    })
}

fn is_image_batch_slide_selector(value: &str) -> bool {
    value.contains(',')
        || (!value.starts_with('-') && value.contains('-'))
        || value.chars().all(|ch| ch.is_ascii_digit())
}

pub(super) fn parse_image_batch_slide_spec(value: &str) -> Result<Vec<u32>, String> {
    let spec = value.trim();
    if spec.is_empty() {
        return Err("empty specification".to_string());
    }
    let mut slides = Vec::new();
    let mut seen = BTreeSet::<u32>::new();
    for part in spec
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if part.contains('-') {
            let range_parts = part.split('-').collect::<Vec<_>>();
            if range_parts.len() != 2 {
                return Err(format!("invalid range format: {part}"));
            }
            let start_raw = range_parts[0].trim();
            let end_raw = range_parts[1].trim();
            let start = start_raw
                .parse::<i64>()
                .map_err(|_| format!("invalid range start: {start_raw}"))?;
            if start <= 0 {
                return Err(format!("invalid range start: {start_raw}"));
            }
            let end = end_raw
                .parse::<i64>()
                .map_err(|_| format!("invalid range end: {end_raw}"))?;
            if end <= 0 {
                return Err(format!("invalid range end: {end_raw}"));
            }
            if start > end {
                return Err(format!(
                    "range start ({start}) cannot be greater than end ({end})"
                ));
            }
            for slide in start as u32..=end as u32 {
                if seen.insert(slide) {
                    slides.push(slide);
                }
            }
        } else {
            let slide = part
                .parse::<i64>()
                .map_err(|_| format!("invalid slide number: {part}"))?;
            if slide <= 0 {
                return Err(format!("invalid slide number: {part}"));
            }
            let slide = slide as u32;
            if seen.insert(slide) {
                slides.push(slide);
            }
        }
    }
    slides.sort_unstable();
    Ok(slides)
}
