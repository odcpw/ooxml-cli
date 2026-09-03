use serde_json::{Map, Value};
use std::collections::BTreeMap;

use crate::{CliError, CliResult};

pub(in crate::serve) fn resolve_refs(
    value: &Value,
    named_results: &BTreeMap<String, Value>,
) -> CliResult<Value> {
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| resolve_refs(item, named_results))
            .collect::<CliResult<Vec<_>>>()
            .map(Value::Array),
        Value::Object(object) if object.contains_key("$ref") => {
            if object.len() != 1 {
                return Err(CliError::invalid_args(
                    "$ref objects must contain exactly one field",
                ));
            }
            let reference = object["$ref"]
                .as_str()
                .ok_or_else(|| CliError::invalid_args("$ref must be a string"))?;
            resolve_one(reference, named_results)
        }
        Value::Object(object) => object
            .iter()
            .map(|(key, value)| Ok((key.clone(), resolve_refs(value, named_results)?)))
            .collect::<CliResult<Map<_, _>>>()
            .map(Value::Object),
        scalar => Ok(scalar.clone()),
    }
}

fn resolve_one(reference: &str, named_results: &BTreeMap<String, Value>) -> CliResult<Value> {
    let (id, path) = split_reference(reference)?;
    let result = named_results.get(id).ok_or_else(|| {
        CliError::invalid_args(format!(
            "unresolved $ref {reference:?}: operation id {id:?} has not completed"
        ))
    })?;
    let mut result_view = result.clone();
    if let Some(envelope) = result.get("mutationEnvelope")
        && let Some(object) = result_view.as_object_mut()
    {
        object.insert(
            "mutationEnvelope".to_string(),
            envelope_reference_view(envelope),
        );
    }
    let primary = result_view
        .get("mutationEnvelope")
        .cloned()
        .unwrap_or_else(|| result_view.clone());
    if path.is_empty() {
        return Ok(primary);
    }

    let pointer = reference_path_to_pointer(path);
    primary
        .pointer(&pointer)
        .or_else(|| result_view.pointer(&pointer))
        .cloned()
        .ok_or_else(|| {
            CliError::invalid_args(format!(
                "unresolved $ref {reference:?}: path {path:?} is absent from operation {id:?}"
            ))
        })
}

fn split_reference(reference: &str) -> CliResult<(&str, &str)> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Err(CliError::invalid_args("$ref must not be empty"));
    }
    let split = reference.find(['.', '/']).unwrap_or(reference.len());
    let id = &reference[..split];
    if id.is_empty() {
        return Err(CliError::invalid_args(format!(
            "invalid $ref {reference:?}: operation id is empty"
        )));
    }
    let path = reference[split..].trim_start_matches(['.', '/']);
    Ok((id, path))
}

fn reference_path_to_pointer(path: &str) -> String {
    let segments = if path.starts_with('#') {
        path.trim_start_matches('#')
            .trim_start_matches('/')
            .split('/')
    } else if path.contains('/') {
        path.trim_start_matches('/').split('/')
    } else {
        path.split('.')
    };
    format!(
        "/{}",
        segments
            .map(|segment| segment.replace('~', "~0").replace('/', "~1"))
            .collect::<Vec<_>>()
            .join("/")
    )
}

fn envelope_reference_view(envelope: &Value) -> Value {
    let mut envelope = envelope.clone();
    let summary = envelope
        .pointer("/destination/summary")
        .and_then(Value::as_object)
        .cloned();
    if let (Some(summary), Some(destination)) = (
        summary,
        envelope
            .get_mut("destination")
            .and_then(Value::as_object_mut),
    ) {
        for (key, value) in summary {
            destination.entry(key).or_insert(value);
        }
    }
    envelope
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn results() -> BTreeMap<String, Value> {
        BTreeMap::from([(
            "slide-hero".to_string(),
            json!({
                "id": "slide-hero",
                "mutationEnvelope": {
                    "destination": {
                        "primarySelector": "slide:2",
                        "summary": {"slide": 2, "name": "Hero"}
                    }
                },
                "readback": {"slide": {"number": 2}}
            }),
        )])
    }

    #[test]
    fn resolves_recursive_leaves_against_the_envelope_root() {
        let value = json!({
            "slide": {"$ref": "slide-hero.destination.slide"},
            "nested": [{"selector": {"$ref": "slide-hero.destination.primarySelector"}}]
        });
        assert_eq!(
            resolve_refs(&value, &results()).unwrap(),
            json!({"slide": 2, "nested": [{"selector": "slide:2"}]})
        );
    }

    #[test]
    fn accepts_explicit_result_roots_and_json_pointer_paths() {
        assert_eq!(
            resolve_refs(
                &json!({"$ref": "slide-hero/readback/slide/number"}),
                &results()
            )
            .unwrap(),
            json!(2)
        );
        assert_eq!(
            resolve_refs(
                &json!({"$ref": "slide-hero.mutationEnvelope.destination.name"}),
                &results()
            )
            .unwrap(),
            json!("Hero")
        );
    }

    #[test]
    fn rejects_forward_refs_and_mixed_ref_objects() {
        let error =
            resolve_refs(&json!({"$ref": "later.destination.slide"}), &results()).unwrap_err();
        assert!(error.message.contains("has not completed"));

        let error = resolve_refs(
            &json!({"$ref": "slide-hero.destination.slide", "fallback": 1}),
            &results(),
        )
        .unwrap_err();
        assert!(error.message.contains("exactly one field"));
    }
}
