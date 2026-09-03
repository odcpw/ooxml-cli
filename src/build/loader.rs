use super::schema::{BuildFamily, schema_document};
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};
use std::fmt;
use std::fs;

#[derive(Clone, Debug, PartialEq)]
pub struct BuildSpec {
    family: BuildFamily,
    document: Value,
}

impl BuildSpec {
    pub const fn family(&self) -> BuildFamily {
        self.family
    }

    pub fn document(&self) -> &Value {
        &self.document
    }

    pub fn into_document(self) -> Value {
        self.document
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildSpecDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
    pub did_you_mean: Vec<String>,
    pub valid_fields: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildSpecError {
    pub diagnostics: Vec<BuildSpecDiagnostic>,
}

impl BuildSpecError {
    fn one(code: &str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostics: vec![BuildSpecDiagnostic {
                code: code.to_string(),
                path: path.into(),
                message: message.into(),
                did_you_mean: Vec::new(),
                valid_fields: Vec::new(),
            }],
        }
    }
}

impl fmt::Display for BuildSpecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(first) = self.diagnostics.first() else {
            return formatter.write_str("build spec validation failed");
        };
        write!(formatter, "{}: {}", first.path, first.message)?;
        if self.diagnostics.len() > 1 {
            write!(formatter, " (and {} more)", self.diagnostics.len() - 1)?;
        }
        Ok(())
    }
}

impl std::error::Error for BuildSpecError {}

pub fn load_spec_file(family: BuildFamily, path: &str) -> Result<BuildSpec, BuildSpecError> {
    let bytes = fs::read(path).map_err(|error| {
        BuildSpecError::one(
            "BUILD_SPEC_FILE_READ_FAILED",
            "/",
            format!("could not read build spec {path:?}: {error}"),
        )
    })?;
    load_spec_bytes(family, &bytes)
}

pub fn load_spec_str(family: BuildFamily, source: &str) -> Result<BuildSpec, BuildSpecError> {
    load_spec_bytes(family, source.as_bytes())
}

pub fn load_spec_bytes(family: BuildFamily, source: &[u8]) -> Result<BuildSpec, BuildSpecError> {
    let document: Value = serde_json::from_slice(source).map_err(|error| {
        BuildSpecError::one(
            "BUILD_SPEC_JSON_INVALID",
            "/",
            format!(
                "invalid {} build spec JSON at line {}, column {}: {error}",
                family,
                error.line(),
                error.column()
            ),
        )
    })?;
    let schema = schema_document(family);
    let mut diagnostics = Vec::new();
    validate_value(&schema, &schema, &document, "", &mut diagnostics);
    diagnostics.sort_by(|left, right| {
        (&left.path, &left.code, &left.message).cmp(&(&right.path, &right.code, &right.message))
    });
    diagnostics.dedup();
    if diagnostics.is_empty() {
        Ok(BuildSpec { family, document })
    } else {
        Err(BuildSpecError { diagnostics })
    }
}

fn validate_value(
    root: &Value,
    schema: &Value,
    value: &Value,
    path: &str,
    diagnostics: &mut Vec<BuildSpecDiagnostic>,
) {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        match resolve_local_ref(root, reference) {
            Some(resolved) => validate_value(root, resolved, value, path, diagnostics),
            None => push_diagnostic(
                diagnostics,
                "BUILD_SPEC_SCHEMA_INVALID",
                display_path(path),
                format!("build schema contains unresolved reference {reference:?}"),
            ),
        }
        return;
    }

    if let Some(options) = schema.get("anyOf").and_then(Value::as_array) {
        if options.iter().any(|candidate| {
            let mut candidate_diagnostics = Vec::new();
            validate_value(root, candidate, value, path, &mut candidate_diagnostics);
            candidate_diagnostics.is_empty()
        }) {
            return;
        }
        push_diagnostic(
            diagnostics,
            "BUILD_SPEC_VALUE_INVALID",
            display_path(path),
            "value does not match any accepted schema form",
        );
        return;
    }

    if let Some(expected) = schema.get("const")
        && value != expected
    {
        push_diagnostic(
            diagnostics,
            "BUILD_SPEC_VALUE_INVALID",
            display_path(path),
            format!(
                "expected constant {}, received {}",
                json_label(expected),
                json_label(value)
            ),
        );
        return;
    }

    if let Some(choices) = schema.get("enum").and_then(Value::as_array)
        && !choices.contains(value)
    {
        push_diagnostic(
            diagnostics,
            "BUILD_SPEC_VALUE_INVALID",
            display_path(path),
            format!(
                "expected one of {}, received {}",
                choices
                    .iter()
                    .map(json_label)
                    .collect::<Vec<_>>()
                    .join(", "),
                json_label(value)
            ),
        );
        return;
    }

    if let Some(expected_type) = schema.get("type")
        && !matches_type(value, expected_type)
    {
        push_diagnostic(
            diagnostics,
            "BUILD_SPEC_TYPE_MISMATCH",
            display_path(path),
            format!(
                "expected {}, received {}",
                expected_type_label(expected_type),
                value_type(value)
            ),
        );
        return;
    }

    match value {
        Value::Object(object) => validate_object(root, schema, object, path, diagnostics),
        Value::Array(items) => validate_array(root, schema, items, path, diagnostics),
        Value::String(text) => validate_string(schema, text, path, diagnostics),
        Value::Number(number) => validate_number(schema, number.as_f64(), path, diagnostics),
        _ => {}
    }
}

fn validate_object(
    root: &Value,
    schema: &Value,
    object: &Map<String, Value>,
    path: &str,
    diagnostics: &mut Vec<BuildSpecDiagnostic>,
) {
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for field in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(field) {
                push_diagnostic(
                    diagnostics,
                    "BUILD_SPEC_REQUIRED_FIELD_MISSING",
                    child_path(path, field),
                    format!("required field {field:?} is missing"),
                );
            }
        }
    }
    if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
        let mut valid_fields = properties.keys().cloned().collect::<Vec<_>>();
        valid_fields.sort();
        for field in object
            .keys()
            .filter(|field| !properties.contains_key(*field))
        {
            let did_you_mean = nearest_fields(field, &valid_fields);
            let hint = did_you_mean
                .first()
                .map(|candidate| format!("; did you mean {candidate:?}?"))
                .unwrap_or_default();
            diagnostics.push(BuildSpecDiagnostic {
                code: "BUILD_SPEC_UNKNOWN_FIELD".to_string(),
                path: child_path(path, field),
                message: format!("unknown field {field:?}{hint}"),
                did_you_mean,
                valid_fields: valid_fields.clone(),
            });
        }
    }
    for (field, field_schema) in &properties {
        if let Some(field_value) = object.get(field) {
            validate_value(
                root,
                field_schema,
                field_value,
                &child_path(path, field),
                diagnostics,
            );
        }
    }
}

fn validate_array(
    root: &Value,
    schema: &Value,
    items: &[Value],
    path: &str,
    diagnostics: &mut Vec<BuildSpecDiagnostic>,
) {
    if let Some(minimum) = schema.get("minItems").and_then(Value::as_u64)
        && items.len() < minimum as usize
    {
        push_diagnostic(
            diagnostics,
            "BUILD_SPEC_ARRAY_TOO_SHORT",
            display_path(path),
            format!(
                "expected at least {minimum} item(s), received {}",
                items.len()
            ),
        );
    }
    if let Some(item_schema) = schema.get("items") {
        for (index, item) in items.iter().enumerate() {
            validate_value(
                root,
                item_schema,
                item,
                &child_path(path, &index.to_string()),
                diagnostics,
            );
        }
    }
}

fn validate_string(
    schema: &Value,
    text: &str,
    path: &str,
    diagnostics: &mut Vec<BuildSpecDiagnostic>,
) {
    if let Some(minimum) = schema.get("minLength").and_then(Value::as_u64)
        && text.chars().count() < minimum as usize
    {
        push_diagnostic(
            diagnostics,
            "BUILD_SPEC_STRING_TOO_SHORT",
            display_path(path),
            format!("expected at least {minimum} character(s)"),
        );
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        match Regex::new(pattern) {
            Ok(pattern) if !pattern.is_match(text) => push_diagnostic(
                diagnostics,
                "BUILD_SPEC_VALUE_INVALID",
                display_path(path),
                format!("value {text:?} does not match required pattern {pattern}"),
            ),
            Err(error) => push_diagnostic(
                diagnostics,
                "BUILD_SPEC_SCHEMA_INVALID",
                display_path(path),
                format!("build schema contains invalid pattern {pattern:?}: {error}"),
            ),
            _ => {}
        }
    }
}

fn validate_number(
    schema: &Value,
    number: Option<f64>,
    path: &str,
    diagnostics: &mut Vec<BuildSpecDiagnostic>,
) {
    if let (Some(number), Some(minimum)) = (number, schema.get("minimum").and_then(Value::as_f64))
        && number < minimum
    {
        push_diagnostic(
            diagnostics,
            "BUILD_SPEC_VALUE_INVALID",
            display_path(path),
            format!("value {number} is below minimum {minimum}"),
        );
    }
}

fn resolve_local_ref<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let pointer = reference.strip_prefix('#')?;
    root.pointer(pointer)
}

fn matches_type(value: &Value, expected: &Value) -> bool {
    if let Some(expected) = expected.as_str() {
        return matches_one_type(value, expected);
    }
    expected.as_array().is_some_and(|types| {
        types
            .iter()
            .filter_map(Value::as_str)
            .any(|kind| matches_one_type(value, kind))
    })
}

fn matches_one_type(value: &Value, expected: &str) -> bool {
    match expected {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "number" => value.is_number(),
        "integer" => value
            .as_number()
            .is_some_and(|number| number.is_i64() || number.is_u64()),
        "string" => value.is_string(),
        _ => false,
    }
}

fn expected_type_label(expected: &Value) -> String {
    if let Some(kind) = expected.as_str() {
        return kind.to_string();
    }
    expected
        .as_array()
        .map(|types| {
            types
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" or ")
        })
        .unwrap_or_else(|| "schema value".to_string())
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn nearest_fields(field: &str, valid_fields: &[String]) -> Vec<String> {
    let mut candidates = valid_fields
        .iter()
        .filter_map(|candidate| {
            let distance = crate::cli_args::damerau_levenshtein(field, candidate);
            (distance <= 2 || candidate.starts_with(field) || field.starts_with(candidate))
                .then(|| (distance, candidate.clone()))
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup_by(|left, right| left.1 == right.1);
    candidates
        .into_iter()
        .take(3)
        .map(|(_, candidate)| candidate)
        .collect()
}

fn child_path(parent: &str, segment: &str) -> String {
    format!("{parent}/{}", escape_pointer_segment(segment))
}

fn display_path(path: &str) -> String {
    if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    }
}

fn escape_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn json_label(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unprintable>".to_string())
}

fn push_diagnostic(
    diagnostics: &mut Vec<BuildSpecDiagnostic>,
    code: &str,
    path: String,
    message: impl Into<String>,
) {
    diagnostics.push(BuildSpecDiagnostic {
        code: code.to_string(),
        path,
        message: message.into(),
        did_you_mean: Vec::new(),
        valid_fields: Vec::new(),
    });
}
