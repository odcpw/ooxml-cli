use serde_json::{Map, Value, json};
use std::fs;

use crate::{
    CliError, CliResult, current_utc_rfc3339, package_type, parse_string_flag, pptx_all_slides,
    pptx_shapes_show, pptx_slides_list, reject_unknown_flags,
};

pub(crate) fn pptx_template_capture(file: &str, args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(
        args,
        &[
            "--author",
            "--description",
            "--name",
            "--organization",
            "--slides",
            "--version",
        ],
        &["--strict-shapes"],
    )?;
    if package_type(file)? != "pptx" {
        return Err(CliError::unsupported_type(
            "template capture supports PPTX/POTX files",
        ));
    }
    let name =
        parse_string_flag(args, "--name")?.unwrap_or_else(|| "Captured Template".to_string());
    let description = parse_string_flag(args, "--description")?.unwrap_or_default();
    let author = parse_string_flag(args, "--author")?.unwrap_or_default();
    let organization = parse_string_flag(args, "--organization")?.unwrap_or_default();
    let version = parse_version(
        parse_string_flag(args, "--version")?
            .as_deref()
            .unwrap_or("1.0.0"),
    )?;
    let slides = parse_capture_slides(parse_string_flag(args, "--slides")?.as_deref(), file)?;
    let timestamp = current_utc_rfc3339();
    let slide_list = pptx_slides_list(file)?;
    let slide_items = slide_list
        .get("slides")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut archetypes = Vec::<Value>::new();
    for slide in slides {
        let slide_meta = slide_items
            .iter()
            .find(|item| item.get("number").and_then(Value::as_u64) == Some(u64::from(slide)))
            .cloned()
            .unwrap_or_else(|| json!({}));
        archetypes.push(capture_archetype(file, slide, &slide_meta)?);
    }
    let mut version_object = Map::new();
    version_object.insert("major".to_string(), json!(version.0));
    version_object.insert("minor".to_string(), json!(version.1));
    version_object.insert("patch".to_string(), json!(version.2));
    version_object.insert("createdAt".to_string(), json!(timestamp));

    let mut manifest = Map::new();
    manifest.insert("manifestVersion".to_string(), json!("1.0.0"));
    manifest.insert("name".to_string(), json!(name));
    manifest.insert("description".to_string(), json!(description));
    manifest.insert("version".to_string(), Value::Object(version_object));
    manifest.insert("createdAt".to_string(), json!(timestamp));
    manifest.insert("modifiedAt".to_string(), json!(timestamp));
    manifest.insert("author".to_string(), json!(author));
    manifest.insert("organization".to_string(), json!(organization));
    manifest.insert("archetypes".to_string(), Value::Array(archetypes));
    manifest.insert("sourceFile".to_string(), json!(file));
    let manifest = Value::Object(manifest);
    validate_manifest(&manifest)
        .map_err(|err| CliError::unexpected(format!("manifest validation failed: {err}")))?;
    Ok(manifest)
}

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

fn capture_archetype(file: &str, slide: u32, slide_meta: &Value) -> CliResult<Value> {
    let shapes = pptx_shapes_show(file, slide, true, true)?;
    let shape_items = shapes
        .get("shapes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut slots = Vec::<Value>::new();
    let mut static_shapes = Vec::<Value>::new();
    let mut seen_slot_ids = std::collections::BTreeMap::<String, usize>::new();
    for shape in shape_items {
        if let Some(slot) = slot_from_shape(&shape, &mut seen_slot_ids) {
            slots.push(slot);
        } else {
            static_shapes.push(static_shape_from_shape(&shape));
        }
    }
    if slots.is_empty() {
        return Err(CliError::unexpected(format!(
            "template capture failed: failed to capture slide {slide}: slide has no fillable slots or placeholders"
        )));
    }
    let mut archetype = Map::new();
    archetype.insert("id".to_string(), json!(format!("archetype-{slide}")));
    archetype.insert("name".to_string(), json!(format!("Slide {slide}")));
    archetype.insert("description".to_string(), json!(""));
    archetype.insert("slots".to_string(), Value::Array(slots));
    archetype.insert("staticShapes".to_string(), Value::Array(static_shapes));
    archetype.insert(
        "layoutName".to_string(),
        json!(
            slide_meta
                .get("layout")
                .and_then(Value::as_str)
                .or_else(|| shapes.get("layoutName").and_then(Value::as_str))
                .unwrap_or_default()
        ),
    );
    archetype.insert("masterName".to_string(), json!(""));
    archetype.insert("sourceSlideNumber".to_string(), json!(slide));
    Ok(Value::Object(archetype))
}

fn slot_from_shape(
    shape: &Value,
    seen_slot_ids: &mut std::collections::BTreeMap<String, usize>,
) -> Option<Value> {
    let target_kind = shape
        .get("targetKind")
        .and_then(Value::as_str)
        .unwrap_or("");
    let shape_type = shape.get("shapeType").and_then(Value::as_str).unwrap_or("");
    let text_capable = shape
        .get("textCapable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let table_info = shape.get("tableInfo");
    let kind = if table_info.is_some() || target_kind == "table" {
        "table"
    } else if target_kind == "picture" || shape_type == "pic" {
        "image"
    } else if target_kind == "body" {
        "bullets"
    } else if text_capable {
        "text"
    } else {
        return None;
    };
    let placeholder = shape.get("placeholder");
    let placeholder_role = placeholder
        .and_then(|placeholder| placeholder.get("role"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let base_id = placeholder
        .and_then(|placeholder| placeholder.get("key"))
        .and_then(Value::as_str)
        .or_else(|| shape.get("primarySelector").and_then(Value::as_str))
        .map(slot_id_from_selector)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            format!(
                "shape-{}",
                shape.get("shapeId").and_then(Value::as_i64).unwrap_or(0)
            )
        });
    let count = seen_slot_ids.entry(base_id.clone()).or_default();
    *count += 1;
    let slot_id = if *count == 1 {
        base_id
    } else {
        format!("{base_id}-{}", *count)
    };
    let mut slot = Map::new();
    slot.insert("id".to_string(), json!(slot_id));
    slot.insert(
        "name".to_string(),
        json!(slot_name(shape, placeholder_role)),
    );
    slot.insert("kind".to_string(), json!(kind));
    slot.insert(
        "required".to_string(),
        json!(matches!(placeholder_role, "title" | "body")),
    );
    if let Some(key) = placeholder
        .and_then(|placeholder| placeholder.get("key"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        slot.insert("placeholderKey".to_string(), json!(key));
    }
    if !placeholder_role.is_empty() {
        slot.insert("placeholderRole".to_string(), json!(placeholder_role));
    }
    if let Some(bounds) = shape.get("bounds") {
        slot.insert("bounds".to_string(), bounds.clone());
    }
    if kind == "table" {
        if let Some(rows) = table_info
            .and_then(|info| info.get("rows"))
            .and_then(Value::as_array)
        {
            slot.insert("tableRows".to_string(), json!(rows.len()));
        }
        if let Some(cols) = table_info
            .and_then(|info| info.get("cols"))
            .or_else(|| table_info.and_then(|info| info.get("columns")))
            .and_then(Value::as_array)
        {
            slot.insert("tableCols".to_string(), json!(cols.len()));
        }
        if let Some(primary) = shape.get("primarySelector").and_then(Value::as_str) {
            slot.insert("tableId".to_string(), json!(primary));
        }
    }
    if kind == "image"
        && let Some(bounds) = shape.get("bounds")
        && let (Some(cx), Some(cy)) = (
            bounds.get("cx").and_then(Value::as_f64),
            bounds.get("cy").and_then(Value::as_f64),
        )
        && cy > 0.0
    {
        slot.insert("aspectRatio".to_string(), json!(cx / cy));
    }
    Some(Value::Object(slot))
}

fn static_shape_from_shape(shape: &Value) -> Value {
    let mut out = Map::new();
    out.insert(
        "id".to_string(),
        json!(format!(
            "shape-{}",
            shape.get("shapeId").and_then(Value::as_i64).unwrap_or(0)
        )),
    );
    out.insert(
        "name".to_string(),
        json!(shape.get("shapeName").and_then(Value::as_str).unwrap_or("")),
    );
    out.insert(
        "type".to_string(),
        json!(
            shape
                .get("shapeType")
                .and_then(Value::as_str)
                .unwrap_or("shape")
        ),
    );
    if let Some(bounds) = shape.get("bounds") {
        out.insert("bounds".to_string(), bounds.clone());
    }
    Value::Object(out)
}

fn slot_name(shape: &Value, placeholder_role: &str) -> String {
    if !placeholder_role.is_empty() {
        return placeholder_role
            .split(['-', '_', ':'])
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => {
                        format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
    }
    shape
        .get("shapeName")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("Slot")
        .to_string()
}

fn slot_id_from_selector(selector: &str) -> String {
    selector
        .trim_start_matches('@')
        .replace(':', "-")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn parse_version(version: &str) -> CliResult<(i64, i64, i64)> {
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(CliError::invalid_args(
            "--version must be major.minor.patch",
        ));
    }
    let major = parse_version_part(parts[0], "major")?;
    let minor = parse_version_part(parts[1], "minor")?;
    let patch = parse_version_part(parts[2], "patch")?;
    Ok((major, minor, patch))
}

fn parse_version_part(part: &str, label: &str) -> CliResult<i64> {
    let value = part.parse::<i64>().map_err(|_| {
        CliError::invalid_args(format!(
            "--version {label} component must be a non-negative integer"
        ))
    })?;
    if value < 0 {
        return Err(CliError::invalid_args(format!(
            "--version {label} component must be a non-negative integer"
        )));
    }
    Ok(value)
}

fn parse_capture_slides(slides: Option<&str>, file: &str) -> CliResult<Vec<u32>> {
    let all_slides = pptx_all_slides(file);
    let slide_count = all_slides.len() as u32;
    let Some(slides) = slides.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(all_slides);
    };
    let mut out = Vec::new();
    for piece in slides.split(',') {
        let slide = piece.trim().parse::<u32>().map_err(|_| {
            CliError::invalid_args(format!("invalid slide number in --slides: {piece:?}"))
        })?;
        if slide == 0 || slide > slide_count {
            return Err(CliError::invalid_args(format!(
                "slide {slide} is out of range (presentation has {slide_count} slides)"
            )));
        }
        if !out.contains(&slide) {
            out.push(slide);
        }
    }
    Ok(out)
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
