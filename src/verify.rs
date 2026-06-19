use serde_json::{Map, Value, json};

use crate::{CliResult, package_type, parse_string_flag, pptx_diff, zip_entry_names};

pub(crate) fn verify(file: &str, args: &[String]) -> CliResult<Value> {
    let baseline = parse_string_flag(args, "--baseline")?;
    let validation = verify_validation(file)?;
    let valid = validation["status"] == "valid";
    let package_type = package_type(file)?;
    let rendered = if package_type == "pptx" {
        json!({
            "enabled": true,
            "reason": "required render tool not available: soffice",
            "status": "unavailable",
        })
    } else {
        json!({
            "enabled": false,
            "reason": "render check applies to PPTX only",
            "status": "skipped",
        })
    };
    let (diff, changes) = if let Some(baseline) = baseline.as_deref() {
        let diff = pptx_diff(baseline, file)?;
        let changes = diff["semantic"]["textDiffs"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default();
        (Some(diff), changes)
    } else {
        (None, 0)
    };
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("rendered".to_string(), rendered);
    result.insert("schemaVersion".to_string(), json!("1.0"));
    result.insert(
        "summary".to_string(),
        json!({
            "baseline": baseline,
            "changes": changes,
            "rendered": false,
            "valid": valid,
        }),
    );
    result.insert("type".to_string(), json!(package_type));
    result.insert("valid".to_string(), json!(valid));
    result.insert("validation".to_string(), validation);
    if let Some(diff) = diff {
        result.insert("diff".to_string(), diff);
    }
    Ok(Value::Object(result))
}

fn verify_validation(file: &str) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    if !entries.iter().any(|name| name == "[Content_Types].xml") {
        return Ok(json!({
            "status": "invalid",
            "summary": {"errors": 1, "info": 0, "warnings": 0},
        }));
    }
    Ok(json!({
        "status": "valid",
        "summary": {"errors": 0, "info": 0, "warnings": 0},
    }))
}
