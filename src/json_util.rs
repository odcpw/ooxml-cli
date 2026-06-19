use serde_json::Value;

use crate::{CliError, CliResult};

pub(crate) fn json_string(value: &Value, key: &str) -> CliResult<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| CliError::invalid_args(format!("{key} is required")))
}

pub(crate) fn json_optional_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(crate) fn json_optional_serialized(value: &Value, key: &str) -> CliResult<Option<String>> {
    let Some(raw) = value.get(key) else {
        return Ok(None);
    };
    if let Some(text) = raw.as_str() {
        return Ok(Some(text.to_string()));
    }
    serde_json::to_string(raw)
        .map(Some)
        .map_err(|err| CliError::invalid_args(format!("{key} must be valid JSON: {err}")))
}

pub(crate) fn json_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

pub(crate) fn json_u32(value: &Value, key: &str) -> CliResult<Option<u32>> {
    let Some(raw) = value.get(key) else {
        return Ok(None);
    };
    if let Some(number) = raw.as_u64() {
        return u32::try_from(number)
            .map(Some)
            .map_err(|_| CliError::invalid_args(format!("{key} must fit in uint32")));
    }
    if let Some(text) = raw.as_str() {
        return text
            .parse::<u32>()
            .map(Some)
            .map_err(|_| CliError::invalid_args(format!("{key} must be an integer")));
    }
    Err(CliError::invalid_args(format!(
        "{key} must be an integer or integer string"
    )))
}

pub(crate) fn json_i64(value: &Value, key: &str) -> CliResult<Option<i64>> {
    let Some(raw) = value.get(key) else {
        return Ok(None);
    };
    if let Some(number) = raw.as_i64() {
        return Ok(Some(number));
    }
    if let Some(text) = raw.as_str() {
        return text
            .parse::<i64>()
            .map(Some)
            .map_err(|_| CliError::invalid_args(format!("{key} must be an integer")));
    }
    Err(CliError::invalid_args(format!(
        "{key} must be an integer or integer string"
    )))
}

pub(crate) fn json_field(value: &Value, key: &str) -> String {
    serde_json::to_string(&value[key])
        .expect("serialize JSON field")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}
