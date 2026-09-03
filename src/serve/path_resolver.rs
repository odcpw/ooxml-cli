use serde_json::{Map, Value};
use std::path::{Component, Path, PathBuf};

use crate::{CliError, CliResult};

pub(in crate::serve) fn resolve_op_paths(
    command: &str,
    args: &Value,
    base_dir: &Path,
) -> CliResult<Value> {
    let object = args
        .as_object()
        .ok_or_else(|| CliError::invalid_args("op args must be an object"))?;
    object
        .iter()
        .map(|(key, value)| {
            let normalized = normalize_arg_key(key);
            let value = if is_external_path_arg(command, &normalized) {
                resolve_path_value(value, base_dir, &normalized)?
            } else {
                value.clone()
            };
            Ok((key.clone(), value))
        })
        .collect::<CliResult<Map<_, _>>>()
        .map(Value::Object)
}

fn is_external_path_arg(command: &str, key: &str) -> bool {
    if key == "image" && command == "docx images replace" {
        return false;
    }
    matches!(
        key,
        "archetype"
            | "brand"
            | "cells-file"
            | "data"
            | "file"
            | "from"
            | "image"
            | "image-base-dir"
            | "manifest"
            | "new-text-file"
            | "ops"
            | "paragraphs-file"
            | "poster"
            | "profile"
            | "records-file"
            | "source"
            | "source-file"
            | "spec"
            | "template"
            | "text-file"
            | "tokens"
            | "values-file"
            | "workbook"
    )
}

fn resolve_path_value(value: &Value, base_dir: &Path, key: &str) -> CliResult<Value> {
    match value {
        Value::String(text) => resolve_path_string(text, base_dir, key).map(Value::String),
        Value::Array(items) => items
            .iter()
            .map(|item| resolve_path_value(item, base_dir, key))
            .collect::<CliResult<Vec<_>>>()
            .map(Value::Array),
        other => Err(CliError::invalid_args(format!(
            "path-valued op arg {key:?} must be a string or string array, got {other}"
        ))),
    }
}

fn resolve_path_string(text: &str, base_dir: &Path, key: &str) -> CliResult<String> {
    if text == "-" || text.is_empty() || Path::new(text).is_absolute() {
        return Ok(text.to_string());
    }
    let (prefix, path_text) = if key == "paragraphs-file" {
        text.split_once('=')
            .map(|(prefix, path)| (Some(prefix), path))
            .unwrap_or((None, text))
    } else {
        (None, text)
    };
    let relative = Path::new(path_text);
    if relative
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(CliError::invalid_args(format!(
            "path-valued op arg {key:?} escapes the ops directory: {text:?}"
        )));
    }
    let resolved = normalize_join(base_dir, relative);
    let resolved = resolved.to_string_lossy();
    Ok(match prefix {
        Some(prefix) => format!("{prefix}={resolved}"),
        None => resolved.into_owned(),
    })
}

fn normalize_join(base_dir: &Path, relative: &Path) -> PathBuf {
    let mut out = base_dir.to_path_buf();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => out.push(part),
            _ => {}
        }
    }
    out
}

fn normalize_arg_key(key: &str) -> String {
    let key = key.trim().trim_start_matches('-');
    let mut out = String::new();
    for (index, ch) in key.chars().enumerate() {
        if ch == '_' {
            out.push('-');
        } else if ch.is_ascii_uppercase() {
            if index > 0 && !out.ends_with('-') {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch.to_ascii_lowercase());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn resolves_scalar_array_and_assignment_paths_against_ops_directory() {
        let base = PathBuf::from("ops-root").join("spec");
        let title = base.join("copy").join("title.json");
        let body = base.join("copy").join("body.json");
        assert_eq!(
            resolve_op_paths(
                "pptx new-slide-from-layout",
                &json!({
                    "paragraphsFile": ["title=copy/title.json", "body=./copy/body.json"]
                }),
                &base,
            )
            .unwrap(),
            json!({
                "paragraphsFile": [
                    format!("title={}", title.to_string_lossy()),
                    format!("body={}", body.to_string_lossy())
                ]
            })
        );
    }

    #[test]
    fn leaves_non_paths_and_absolute_paths_unchanged() {
        let absolute = std::env::temp_dir().join("replacement.png");
        let args = json!({"image": "rId7", "file": absolute});
        assert_eq!(
            resolve_op_paths("docx images replace", &args, Path::new("ops-root/spec")).unwrap(),
            args
        );
    }

    #[test]
    fn rejects_relative_parent_traversal() {
        let error = resolve_op_paths(
            "pptx place image",
            &json!({"image": "../secret.png"}),
            Path::new("ops-root/spec"),
        )
        .unwrap_err();
        assert!(error.message.contains("escapes the ops directory"));
    }
}
