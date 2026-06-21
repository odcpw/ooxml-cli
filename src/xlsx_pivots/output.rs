use serde_json::{Value, json};

use super::model::XlsxPivotRef;
use crate::{CliError, CliResult, command_arg, selector_candidates, xlsx_source_command};

pub(super) fn xlsx_pivot_item_json(file: &str, pivot: &XlsxPivotRef) -> Value {
    let mut object = pivot.to_json_object();
    let pivot_selector = xlsx_pivot_selector(pivot);
    let sheet_selector = xlsx_pivot_sheet_selector(pivot);
    object.insert(
        "showCommand".to_string(),
        json!(xlsx_source_command(
            vec!["ooxml", "--json", "xlsx", "pivots", "show", file],
            &[("--sheet", &sheet_selector), ("--pivot", &pivot_selector)]
        )),
    );
    if let Some(cache) = &pivot.cache
        && !cache.source.sheet.is_empty()
        && !cache.source.range.is_empty()
    {
        object.insert(
            "sourceExportCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx ranges export {} --sheet {} --range {} --include-types",
                command_arg(file),
                command_arg(&cache.source.sheet),
                command_arg(&cache.source.range),
            )),
        );
    }
    Value::Object(object)
}

fn xlsx_pivot_selector(pivot: &XlsxPivotRef) -> String {
    if !pivot.primary_selector.is_empty() {
        pivot.primary_selector.clone()
    } else if !pivot.name.is_empty() {
        pivot.name.clone()
    } else if pivot.number > 0 {
        format!("pivot:{}", pivot.number)
    } else {
        "1".to_string()
    }
}

fn xlsx_pivot_sheet_selector(pivot: &XlsxPivotRef) -> String {
    if !pivot.sheet.is_empty() {
        pivot.sheet.clone()
    } else if pivot.sheet_number > 0 {
        pivot.sheet_number.to_string()
    } else {
        "1".to_string()
    }
}

pub(super) fn select_xlsx_pivot(
    pivots: &[XlsxPivotRef],
    selector: &str,
) -> CliResult<XlsxPivotRef> {
    if pivots.is_empty() {
        return Err(CliError::invalid_args("workbook has no pivots"));
    }
    let selector = selector.trim();
    if selector.is_empty() {
        if pivots.len() == 1 {
            return Ok(pivots[0].clone());
        }
        return Err(CliError::invalid_args(
            "--pivot is required when workbook has multiple pivots",
        ));
    }
    let matches = pivots
        .iter()
        .filter(|pivot| {
            pivot
                .selectors
                .iter()
                .any(|candidate| candidate == selector)
        })
        .cloned()
        .collect::<Vec<_>>();
    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }
    if matches.len() > 1 {
        let selectors = matches
            .iter()
            .map(xlsx_pivot_selector)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(CliError::invalid_args(format!(
            "pivot selector {selector:?} matched multiple pivots ({selectors}); use a more specific selector"
        )));
    }
    if let Ok(number) = selector.parse::<usize>() {
        if (1..=pivots.len()).contains(&number) {
            return Ok(pivots[number - 1].clone());
        }
        return Err(CliError::target_not_found(format!(
            "pivot {number} is out of range (1-{})",
            pivots.len()
        )));
    }
    let candidates = pivots
        .iter()
        .map(|pivot| (pivot.primary_selector.as_str(), pivot.selectors.as_slice()))
        .collect::<Vec<_>>();
    let suggestions = selector_candidates(&candidates, selector, 5);
    let hint = if suggestions.is_empty() {
        String::new()
    } else {
        format!("; did you mean: {}", suggestions.join(", "))
    };
    Err(CliError::target_not_found(format!(
        "pivot not found: {selector}{hint}; discover with `ooxml --json xlsx pivots list <file>`"
    )))
}
