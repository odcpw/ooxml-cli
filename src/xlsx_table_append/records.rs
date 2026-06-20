use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;

use crate::xlsx_mutation::{XlsxMatrixCell, parse_xlsx_matrix_cell};
use crate::{CliError, CliResult};

pub(super) fn normalize_xlsx_missing_policy(value: Option<&str>) -> CliResult<String> {
    let raw = value.unwrap_or("reject");
    let normalized = match raw.trim().to_ascii_lowercase().as_str() {
        "" | "reject" => "reject",
        "skip" => "skip",
        "empty-string" => "empty-string",
        _ => {
            return Err(CliError::invalid_args(format!(
                "invalid missing policy {raw:?} (must be reject, skip, or empty-string)"
            )));
        }
    };
    Ok(normalized.to_string())
}

pub(super) fn resolve_xlsx_tables_append_records(
    records: Option<&str>,
    records_file: Option<&str>,
) -> CliResult<Vec<BTreeMap<String, XlsxMatrixCell>>> {
    match (records, records_file) {
        (Some(_), Some(_)) | (None, None) => Err(CliError::invalid_args(
            "must specify exactly one of --records or --records-file",
        )),
        (Some(data), None) => decode_xlsx_append_records(data),
        (None, Some("-")) => {
            let mut data = String::new();
            std::io::stdin()
                .read_to_string(&mut data)
                .map_err(|err| CliError::unexpected(format!("failed to read stdin: {err}")))?;
            decode_xlsx_append_records(&data)
        }
        (None, Some(path)) => {
            let data = fs::read_to_string(path)
                .map_err(|_| CliError::file_not_found(format!("file not found: {path}")))?;
            decode_xlsx_append_records(&data)
        }
    }
}

pub(super) fn xlsx_records_to_rows(
    records: &[BTreeMap<String, XlsxMatrixCell>],
    columns: &[String],
    missing_policy: &str,
    ignore_extra_fields: bool,
) -> CliResult<Vec<Vec<XlsxMatrixCell>>> {
    if records.is_empty() {
        return Err(CliError::invalid_args("records cannot be empty"));
    }
    if columns.is_empty() {
        return Err(CliError::invalid_args(
            "table must have at least one column",
        ));
    }
    let mut column_set = BTreeSet::new();
    for (col_idx, column) in columns.iter().enumerate() {
        if column.trim().is_empty() {
            return Err(CliError::invalid_args(format!(
                "table column {} has a blank name",
                col_idx + 1
            )));
        }
        if !column_set.insert(column.clone()) {
            return Err(CliError::invalid_args(format!(
                "duplicate table column name {column:?}"
            )));
        }
    }

    records
        .iter()
        .enumerate()
        .map(|(row_idx, record)| {
            for key in record.keys() {
                if column_set.contains(key) || ignore_extra_fields {
                    continue;
                }
                return Err(CliError::invalid_args(format!(
                    "records[{row_idx}] contains unknown field {key:?}"
                )));
            }
            columns
                .iter()
                .map(|column| {
                    if let Some(cell) = record.get(column) {
                        return Ok(cell.clone());
                    }
                    match missing_policy {
                        "reject" => Err(CliError::invalid_args(format!(
                            "records[{row_idx}] missing required field {column:?}"
                        ))),
                        "skip" => Ok(XlsxMatrixCell {
                            kind: "empty".to_string(),
                            value: String::new(),
                            formula: String::new(),
                            null: true,
                        }),
                        "empty-string" => Ok(XlsxMatrixCell {
                            kind: "string".to_string(),
                            value: String::new(),
                            formula: String::new(),
                            null: false,
                        }),
                        _ => unreachable!("missing policy validated earlier"),
                    }
                })
                .collect()
        })
        .collect()
}

fn decode_xlsx_append_records(data: &str) -> CliResult<Vec<BTreeMap<String, XlsxMatrixCell>>> {
    let mut values = serde_json::Deserializer::from_str(data).into_iter::<Value>();
    let raw = values
        .next()
        .ok_or_else(|| CliError::invalid_args("invalid JSON records: EOF while parsing a value"))?
        .map_err(|err| CliError::invalid_args(format!("invalid JSON records: {err}")))?;
    if values.next().is_some() {
        return Err(CliError::invalid_args(
            "invalid JSON records: JSON input must contain exactly one value",
        ));
    }
    let records_value = if let Some(object) = raw.as_object() {
        object.get("records").ok_or_else(|| {
            CliError::invalid_args("invalid JSON records: JSON object must contain records")
        })?
    } else {
        &raw
    };
    let records = records_value.as_array().ok_or_else(|| {
        CliError::invalid_args("invalid JSON records: records must be an array of objects")
    })?;
    records
        .iter()
        .enumerate()
        .map(|(row_idx, record)| {
            let object = record.as_object().ok_or_else(|| {
                CliError::invalid_args(format!(
                    "invalid JSON records: records[{row_idx}] must be an object"
                ))
            })?;
            let mut out = BTreeMap::new();
            for (key, raw_cell) in object {
                if key.trim().is_empty() {
                    return Err(CliError::invalid_args(format!(
                        "invalid JSON records: records[{row_idx}] contains a blank field name"
                    )));
                }
                let cell = parse_xlsx_matrix_cell(raw_cell).map_err(|err| {
                    CliError::invalid_args(format!(
                        "invalid JSON records: records[{row_idx}].{key}: {}",
                        err.message
                    ))
                })?;
                out.insert(key.clone(), cell);
            }
            Ok(out)
        })
        .collect()
}
