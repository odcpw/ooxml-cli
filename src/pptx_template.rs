use serde_json::{Map, Value, json};
use std::fs;

use crate::{CliError, CliResult};

pub(crate) fn pptx_template_inspect(manifest_path: &str) -> CliResult<Value> {
    let text = fs::read_to_string(manifest_path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {manifest_path}"))
        } else {
            CliError::unexpected(format!("failed to read manifest file: {err}"))
        }
    })?;
    let manifest: Value = serde_json::from_str(&text)
        .map_err(|err| CliError::unexpected(format!("failed to parse manifest: {err}")))?;
    validate_manifest(&manifest)
        .map_err(|err| CliError::unexpected(format!("manifest validation failed: {err}")))?;
    Ok(manifest_inspect_json(&manifest))
}

fn manifest_inspect_json(manifest: &Value) -> Value {
    let mut output = Map::new();
    output.insert("name".to_string(), json!(string_field(manifest, "name")));
    output.insert(
        "description".to_string(),
        json!(string_field(manifest, "description")),
    );
    output.insert("version".to_string(), json!(version_string(manifest)));
    output.insert(
        "author".to_string(),
        json!(string_field(manifest, "author")),
    );
    output.insert(
        "organization".to_string(),
        json!(string_field(manifest, "organization")),
    );
    output.insert(
        "createdAt".to_string(),
        manifest.get("createdAt").cloned().unwrap_or(Value::Null),
    );
    output.insert(
        "modifiedAt".to_string(),
        manifest.get("modifiedAt").cloned().unwrap_or(Value::Null),
    );
    output.insert(
        "archetypes".to_string(),
        Value::Array(
            manifest
                .get("archetypes")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(archetype_inspect_json)
                .collect(),
        ),
    );
    Value::Object(output)
}

fn archetype_inspect_json(archetype: &Value) -> Value {
    let mut output = Map::new();
    output.insert("id".to_string(), json!(string_field(archetype, "id")));
    output.insert("name".to_string(), json!(string_field(archetype, "name")));
    output.insert(
        "description".to_string(),
        json!(string_field(archetype, "description")),
    );
    output.insert(
        "layoutName".to_string(),
        json!(string_field(archetype, "layoutName")),
    );
    output.insert(
        "masterName".to_string(),
        json!(string_field(archetype, "masterName")),
    );
    output.insert(
        "slots".to_string(),
        Value::Array(
            archetype
                .get("slots")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(slot_inspect_json)
                .collect(),
        ),
    );
    output.insert(
        "staticShapes".to_string(),
        Value::Array(
            archetype
                .get("staticShapes")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(static_shape_inspect_json)
                .collect(),
        ),
    );
    Value::Object(output)
}

fn slot_inspect_json(slot: &Value) -> Value {
    let mut output = Map::new();
    let kind = string_field(slot, "kind");
    output.insert("id".to_string(), json!(string_field(slot, "id")));
    output.insert("name".to_string(), json!(string_field(slot, "name")));
    output.insert("kind".to_string(), json!(kind));
    output.insert(
        "required".to_string(),
        json!(
            slot.get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        ),
    );
    if let Some(role) = non_empty_string_field(slot, "placeholderRole") {
        output.insert("placeholderRole".to_string(), json!(role));
    }
    if kind == "table" {
        if let Some(rows) = slot.get("tableRows").and_then(Value::as_i64) {
            output.insert("tableRows".to_string(), json!(rows));
        }
        if let Some(cols) = slot.get("tableCols").and_then(Value::as_i64) {
            output.insert("tableCols".to_string(), json!(cols));
        }
    }
    Value::Object(output)
}

fn static_shape_inspect_json(shape: &Value) -> Value {
    let mut output = Map::new();
    output.insert("id".to_string(), json!(string_field(shape, "id")));
    output.insert("name".to_string(), json!(string_field(shape, "name")));
    output.insert("type".to_string(), json!(string_field(shape, "type")));
    Value::Object(output)
}

fn validate_manifest(manifest: &Value) -> Result<(), String> {
    if !manifest.is_object() {
        return Err("manifest is nil".to_string());
    }
    if string_field(manifest, "name").is_empty() {
        return Err("manifest must have a non-empty name".to_string());
    }
    if string_field(manifest, "manifestVersion").is_empty() {
        return Err("manifest must have a manifestVersion".to_string());
    }
    let version = manifest
        .get("version")
        .ok_or_else(|| "manifest must have version information".to_string())?;
    validate_version(version).map_err(|err| format!("invalid version: {err}"))?;
    let archetypes = manifest
        .get("archetypes")
        .and_then(Value::as_array)
        .ok_or_else(|| "manifest must have at least one archetype".to_string())?;
    if archetypes.is_empty() {
        return Err("manifest must have at least one archetype".to_string());
    }
    let mut seen = std::collections::BTreeSet::new();
    for (index, archetype) in archetypes.iter().enumerate() {
        let id = string_field(archetype, "id");
        if id.is_empty() {
            return Err(format!("archetype at index {index} has empty ID"));
        }
        if !seen.insert(id.clone()) {
            return Err(format!("duplicate archetype ID: {id}"));
        }
        validate_archetype(archetype).map_err(|err| format!("archetype {id} is invalid: {err}"))?;
    }
    Ok(())
}

fn validate_version(version: &Value) -> Result<(), String> {
    if !version.is_object() {
        return Err("version is nil".to_string());
    }
    let major = version.get("major").and_then(Value::as_i64).unwrap_or(0);
    let minor = version.get("minor").and_then(Value::as_i64).unwrap_or(0);
    let patch = version.get("patch").and_then(Value::as_i64).unwrap_or(0);
    if major < 0 || minor < 0 || patch < 0 {
        return Err(format!(
            "version numbers must be non-negative, got {major}.{minor}.{patch}"
        ));
    }
    if non_empty_string_field(version, "createdAt").is_none() {
        return Err("version must have a createdAt timestamp".to_string());
    }
    Ok(())
}

fn validate_archetype(archetype: &Value) -> Result<(), String> {
    let id = string_field(archetype, "id");
    if id.is_empty() {
        return Err("archetype must have an ID".to_string());
    }
    if string_field(archetype, "name").is_empty() {
        return Err(format!("archetype {id} must have a name"));
    }
    let slots = archetype
        .get("slots")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("archetype {id} must have at least one slot"))?;
    if slots.is_empty() {
        return Err(format!("archetype {id} must have at least one slot"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for (index, slot) in slots.iter().enumerate() {
        let slot_id = string_field(slot, "id");
        if slot_id.is_empty() {
            return Err(format!(
                "archetype {id}: slot at index {index} has empty ID"
            ));
        }
        if !seen.insert(slot_id.clone()) {
            return Err(format!("archetype {id}: duplicate slot ID {slot_id}"));
        }
        validate_slot(slot)
            .map_err(|err| format!("archetype {id}, slot {slot_id} is invalid: {err}"))?;
    }
    Ok(())
}

fn validate_slot(slot: &Value) -> Result<(), String> {
    let id = string_field(slot, "id");
    if id.is_empty() {
        return Err("slot must have an ID".to_string());
    }
    if string_field(slot, "name").is_empty() {
        return Err(format!("slot {id} must have a name"));
    }
    let kind = string_field(slot, "kind");
    if !matches!(
        kind.as_str(),
        "text" | "richText" | "bullets" | "image" | "table" | "notes"
    ) {
        return Err(format!("slot {id}: invalid kind {kind:?}"));
    }
    if let Some(bounds) = slot.get("bounds").and_then(Value::as_object) {
        let cx = bounds.get("cx").and_then(Value::as_i64).unwrap_or(0);
        let cy = bounds.get("cy").and_then(Value::as_i64).unwrap_or(0);
        if cx <= 0 || cy <= 0 {
            return Err(format!(
                "slot {id}: bounds must have positive width and height"
            ));
        }
    }
    if kind == "table" {
        if let Some(rows) = slot.get("tableRows").and_then(Value::as_i64)
            && rows <= 0
        {
            return Err(format!("slot {id}: tableRows must be positive"));
        }
        if let Some(cols) = slot.get("tableCols").and_then(Value::as_i64)
            && cols <= 0
        {
            return Err(format!("slot {id}: tableCols must be positive"));
        }
    }
    if let Some(ratio) = slot.get("aspectRatio").and_then(Value::as_f64)
        && ratio <= 0.0
    {
        return Err(format!("slot {id}: aspectRatio must be positive"));
    }
    Ok(())
}

fn version_string(manifest: &Value) -> String {
    let version = manifest.get("version").unwrap_or(&Value::Null);
    format!(
        "{}.{}.{}",
        version.get("major").and_then(Value::as_i64).unwrap_or(0),
        version.get("minor").and_then(Value::as_i64).unwrap_or(0),
        version.get("patch").and_then(Value::as_i64).unwrap_or(0)
    )
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn non_empty_string_field(value: &Value, key: &str) -> Option<String> {
    let value = string_field(value, key);
    if value.is_empty() { None } else { Some(value) }
}
