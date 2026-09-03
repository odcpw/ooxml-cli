use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::{CliError, CliResult, command_arg, reject_unknown_flags};

pub(crate) const MUTATION_ENVELOPE_SCHEMA_ID: &str =
    "https://ooxml-cli.dev/schemas/mutation-envelope.schema.json";

const MUTATION_ENVELOPE_SCHEMA_JSON: &str = r##"
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://ooxml-cli.dev/schemas/mutation-envelope.schema.json",
  "title": "OOXML Mutation Envelope",
  "description": "Stable proof and destination metadata added to every successful package mutation.",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "file",
    "family",
    "command",
    "destination",
    "changed",
    "readbackCommand",
    "validateCommand",
    "conformanceCommand",
    "checkCommand",
    "warnings",
    "aliasesApplied",
    "validated"
  ],
  "properties": {
    "file": { "type": "string", "minLength": 1 },
    "family": { "enum": ["docx", "xlsx", "pptx", "vba", "package"] },
    "command": { "type": "string", "pattern": "^ooxml " },
    "destination": { "$ref": "#/$defs/destination" },
    "changed": {
      "type": "array",
      "items": { "$ref": "#/$defs/change" }
    },
    "readbackCommand": { "type": "string", "pattern": "^ooxml " },
    "validateCommand": { "type": "string", "pattern": "^ooxml " },
    "conformanceCommand": { "type": "string", "pattern": "^ooxml " },
    "checkCommand": { "type": "string", "pattern": "^ooxml --json check " },
    "renderCommand": { "type": "string", "pattern": "^ooxml " },
    "layoutCheckCommand": { "type": "string", "pattern": "^ooxml " },
    "warnings": { "type": "array", "items": {} },
    "aliasesApplied": { "type": "array", "items": { "type": "object" } },
    "validated": { "type": "boolean" }
  },
  "allOf": [
    {
      "if": { "properties": { "family": { "enum": ["docx", "xlsx", "pptx"] } } },
      "then": { "required": ["renderCommand"] }
    },
    {
      "if": { "properties": { "family": { "const": "pptx" } } },
      "then": { "required": ["layoutCheckCommand"] }
    }
  ],
  "$defs": {
    "destination": {
      "type": "object",
      "additionalProperties": false,
      "required": ["partUri", "primarySelector", "selectors", "handle", "kind", "summary"],
      "properties": {
        "partUri": { "type": "string", "minLength": 1 },
        "primarySelector": { "type": "string", "minLength": 1 },
        "selectors": {
          "type": "array",
          "minItems": 1,
          "uniqueItems": true,
          "items": { "type": "string", "minLength": 1 }
        },
        "handle": { "type": "string", "minLength": 1 },
        "kind": { "type": "string", "minLength": 1 },
        "summary": { "type": "object", "additionalProperties": true }
      }
    },
    "change": {
      "type": "object",
      "additionalProperties": false,
      "required": ["kind", "selector", "handle"],
      "properties": {
        "kind": { "type": "string", "minLength": 1 },
        "selector": { "type": "string", "minLength": 1 },
        "handle": { "type": "string", "minLength": 1 },
        "beforeHash": { "type": "string", "pattern": "^sha256:[0-9a-f]{64}$" },
        "afterHash": { "type": "string", "pattern": "^sha256:[0-9a-f]{64}$" }
      }
    }
  }
}
"##;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MutationEnvelope {
    pub(crate) file: String,
    pub(crate) family: String,
    pub(crate) command: String,
    pub(crate) destination: MutationDestination,
    pub(crate) changed: Vec<MutationChange>,
    pub(crate) readback_command: String,
    pub(crate) validate_command: String,
    pub(crate) conformance_command: String,
    pub(crate) check_command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) render_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) layout_check_command: Option<String>,
    pub(crate) warnings: Vec<Value>,
    pub(crate) aliases_applied: Vec<Value>,
    pub(crate) validated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MutationDestination {
    pub(crate) part_uri: String,
    pub(crate) primary_selector: String,
    pub(crate) selectors: Vec<String>,
    pub(crate) handle: String,
    pub(crate) kind: String,
    pub(crate) summary: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MutationChange {
    pub(crate) kind: String,
    pub(crate) selector: String,
    pub(crate) handle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) before_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) after_hash: Option<String>,
}

pub(crate) struct MutationEnvelopeInput {
    pub(crate) file: String,
    pub(crate) family: String,
    pub(crate) command: String,
    pub(crate) destination: MutationDestination,
    pub(crate) changed: Vec<MutationChange>,
    pub(crate) readback_command: String,
    pub(crate) warnings: Vec<Value>,
    pub(crate) aliases_applied: Vec<Value>,
    pub(crate) validated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MutationCommandSpec {
    path: &'static [&'static str],
    destination_kind: &'static str,
    default_part_uri: &'static str,
    readback: ReadbackKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadbackKind {
    Blocks,
    Comments,
    Fields,
    Footers,
    Headers,
    Images,
    Tables,
}

macro_rules! docx_spec {
    ([$($segment:literal),+], $kind:literal, $part:literal, $readback:ident) => {
        MutationCommandSpec {
            path: &[$($segment),+],
            destination_kind: $kind,
            default_part_uri: $part,
            readback: ReadbackKind::$readback,
        }
    };
}

// This table is deliberately data rather than command-name inference. It is the
// review surface for the DOCX adoption stage and is mirrored into the public
// CommandSpec destinationKind rows.
const DOCX_MUTATION_COMMANDS: &[MutationCommandSpec] = &[
    docx_spec!(["docx", "scaffold"], "package", "/", Blocks),
    docx_spec!(
        ["docx", "blocks", "replace"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "blocks", "delete"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "blocks", "insert-after"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "breaks", "insert"],
        "section",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "sections", "set"],
        "section",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "paragraphs", "append"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "paragraphs", "insert"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "paragraphs", "set"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "paragraphs", "clear"],
        "paragraph",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "styles", "apply"],
        "styled-object",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "comments", "add"],
        "comment",
        "/word/comments.xml",
        Comments
    ),
    docx_spec!(
        ["docx", "comments", "edit"],
        "comment",
        "/word/comments.xml",
        Comments
    ),
    docx_spec!(
        ["docx", "comments", "remove"],
        "comment",
        "/word/comments.xml",
        Comments
    ),
    docx_spec!(
        ["docx", "fields", "insert"],
        "field",
        "/word/document.xml",
        Fields
    ),
    docx_spec!(
        ["docx", "fields", "set-result"],
        "field",
        "/word/document.xml",
        Fields
    ),
    docx_spec!(
        ["docx", "headers", "set-text"],
        "header",
        "/word/header1.xml",
        Headers
    ),
    docx_spec!(
        ["docx", "footers", "set-text"],
        "footer",
        "/word/footer1.xml",
        Footers
    ),
    docx_spec!(
        ["docx", "images", "replace"],
        "image",
        "/word/document.xml",
        Images
    ),
    docx_spec!(
        ["docx", "images", "insert"],
        "image",
        "/word/document.xml",
        Images
    ),
    docx_spec!(
        ["docx", "replace"],
        "text-match",
        "/word/document.xml",
        Blocks
    ),
    docx_spec!(
        ["docx", "tables", "create"],
        "table",
        "/word/document.xml",
        Tables
    ),
    docx_spec!(
        ["docx", "tables", "set-style"],
        "table",
        "/word/document.xml",
        Tables
    ),
    docx_spec!(
        ["docx", "tables", "set-cell"],
        "table",
        "/word/document.xml",
        Tables
    ),
    docx_spec!(
        ["docx", "tables", "clear-cell"],
        "table",
        "/word/document.xml",
        Tables
    ),
    docx_spec!(
        ["docx", "tables", "insert-row"],
        "table",
        "/word/document.xml",
        Tables
    ),
    docx_spec!(
        ["docx", "tables", "delete-row"],
        "table",
        "/word/document.xml",
        Tables
    ),
];

pub(crate) fn attach_cli_mutation_envelope(
    args: &[String],
    aliases_applied: Vec<Value>,
    response: &mut Value,
) -> CliResult<()> {
    let Some(spec) = mutation_spec_for_args(args) else {
        return Ok(());
    };
    let file = mutation_destination_file(args, spec, response)?;
    let destination = mutation_destination(spec, response);
    let selector = destination.primary_selector.clone();
    let handle = destination.handle.clone();
    let before_hash = response_hash(response, &["beforeHash", "previousHash"]);
    let after_hash = response_hash(response, &["afterHash", "contentHash"]);
    let warnings = response_warnings(response);
    let validated = response
        .get("validated")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| !args.iter().any(|arg| arg == "--no-validate"));
    MutationEnvelope::from_input(MutationEnvelopeInput {
        file: file.clone(),
        family: "docx".to_string(),
        command: command_for_args(args),
        destination,
        changed: vec![MutationChange {
            kind: spec.destination_kind.to_string(),
            selector,
            handle,
            before_hash,
            after_hash,
        }],
        readback_command: response
            .get("readbackCommand")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| readback_command(spec.readback, &file, response)),
        warnings,
        aliases_applied,
        validated,
    })
    .attach_to(response)
}

fn mutation_spec_for_args(args: &[String]) -> Option<&'static MutationCommandSpec> {
    DOCX_MUTATION_COMMANDS.iter().find(|spec| {
        args.len() >= spec.path.len()
            && args
                .iter()
                .zip(spec.path)
                .all(|(actual, expected)| actual == expected)
    })
}

fn mutation_destination_file(
    args: &[String],
    spec: &MutationCommandSpec,
    response: &Value,
) -> CliResult<String> {
    let file = response
        .get("output")
        .and_then(nonempty_string)
        .or_else(|| flag_value(args, "--out"))
        .or_else(|| {
            if spec.path == ["docx", "scaffold"] {
                args.get(spec.path.len())
                    .and_then(|value| (!value.starts_with('-')).then(|| value.to_string()))
            } else {
                None
            }
        })
        .or_else(|| response.get("file").and_then(nonempty_string))
        .or_else(|| args.get(spec.path.len()).cloned())
        .filter(|value| !value.trim().is_empty());
    file.ok_or_else(|| {
        CliError::unexpected(format!(
            "{} succeeded without an addressable destination file",
            spec.path.join(" ")
        ))
    })
}

fn nonempty_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter().enumerate().find_map(|(index, arg)| {
        if arg == name {
            args.get(index + 1).cloned()
        } else {
            arg.strip_prefix(&format!("{name}=")).map(str::to_string)
        }
    })
}

fn mutation_destination(spec: &MutationCommandSpec, response: &Value) -> MutationDestination {
    let part_uri = response
        .get("destination")
        .and_then(|value| value.get("partUri"))
        .and_then(nonempty_string)
        .or_else(|| response.get("partUri").and_then(nonempty_string))
        .unwrap_or_else(|| spec.default_part_uri.to_string());
    let primary_selector = response
        .get("destination")
        .and_then(|value| value.get("primarySelector"))
        .and_then(nonempty_string)
        .or_else(|| response.get("selector").and_then(nonempty_string))
        .unwrap_or_else(|| selector_from_response(spec.destination_kind, response));
    let handle = response
        .get("destination")
        .and_then(|value| value.get("handle"))
        .and_then(nonempty_string)
        .or_else(|| response.get("handle").and_then(nonempty_string))
        .unwrap_or_else(|| {
            format!(
                "H:docx/{}:{}",
                spec.destination_kind,
                primary_selector.replace(':', "/")
            )
        });
    let mut selectors = vec![primary_selector.clone()];
    for candidate in [
        response.get("selector").and_then(nonempty_string),
        response.get("blockId").and_then(nonempty_string),
        Some(handle.clone()),
    ]
    .into_iter()
    .flatten()
    {
        if !selectors.contains(&candidate) {
            selectors.push(candidate);
        }
    }
    MutationDestination {
        part_uri,
        primary_selector,
        selectors,
        handle,
        kind: spec.destination_kind.to_string(),
        summary: response_summary(response),
    }
}

fn selector_from_response(kind: &str, response: &Value) -> String {
    if let Some(comment_id) = response.get("commentId").and_then(Value::as_i64) {
        return format!("comment:{comment_id}");
    }
    if let Some(table) = response.get("table").and_then(Value::as_u64) {
        return format!("table:{table}");
    }
    if let Some(field) = response.get("fieldIndex").and_then(Value::as_u64) {
        return format!("field:{field}");
    }
    if let Some(section) = response.get("section").and_then(Value::as_u64) {
        return format!("section:{section}");
    }
    if let Some(index) = response
        .get("blockIndex")
        .or_else(|| response.get("index"))
        .and_then(Value::as_u64)
    {
        let prefix = if kind == "image" { "image" } else { "block" };
        return format!("{prefix}:{index}");
    }
    match kind {
        "package" => "package".to_string(),
        "header" => "header:1".to_string(),
        "footer" => "footer:1".to_string(),
        other => format!("{other}:document"),
    }
}

fn response_summary(response: &Value) -> Map<String, Value> {
    let mut summary = Map::new();
    let Some(object) = response.as_object() else {
        return summary;
    };
    for (key, value) in object {
        if matches!(
            key.as_str(),
            "aliasesApplied"
                | "conformanceCommand"
                | "destination"
                | "mutationEnvelope"
                | "readbackCommand"
                | "validateCommand"
                | "warnings"
        ) {
            continue;
        }
        if value.is_null() || value.is_boolean() || value.is_number() || value.is_string() {
            summary.insert(key.clone(), value.clone());
        }
    }
    summary
}

fn response_hash(response: &Value, keys: &[&str]) -> Option<String> {
    let direct = keys.iter().find_map(|key| {
        response
            .get(*key)
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .map(str::to_string)
    });
    direct.or_else(|| {
        let index = response
            .get("blockIndex")
            .or_else(|| response.get("index"))
            .and_then(Value::as_u64)?;
        response
            .get("blockHashes")?
            .as_array()?
            .iter()
            .find(|block| block.get("index").and_then(Value::as_u64) == Some(index))?
            .get("contentHash")?
            .as_str()
            .filter(|hash| is_sha256(hash))
            .map(str::to_string)
    })
}

fn is_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hash| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn response_warnings(response: &Value) -> Vec<Value> {
    let mut warnings = response
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(warning) = response.get("warning") {
        warnings.push(warning.clone());
    }
    warnings
}

fn command_for_args(args: &[String]) -> String {
    std::iter::once("ooxml".to_string())
        .chain(std::iter::once("--json".to_string()))
        .chain(args.iter().map(|arg| command_arg(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn readback_command(kind: ReadbackKind, file: &str, response: &Value) -> String {
    let file = command_arg(file);
    match kind {
        ReadbackKind::Blocks => format!("ooxml --json docx blocks {file}"),
        ReadbackKind::Comments => format!("ooxml --json docx comments list {file}"),
        ReadbackKind::Fields => format!("ooxml --json docx fields list {file}"),
        ReadbackKind::Footers => format!("ooxml --json docx footers list {file}"),
        ReadbackKind::Headers => format!("ooxml --json docx headers list {file}"),
        ReadbackKind::Images => format!("ooxml --json docx images list {file}"),
        ReadbackKind::Tables => response
            .get("table")
            .and_then(Value::as_u64)
            .map(|table| format!("ooxml --json docx tables show {file} --table {table}"))
            .unwrap_or_else(|| format!("ooxml --json docx tables show {file}")),
    }
}

impl MutationEnvelope {
    pub(crate) fn from_input(input: MutationEnvelopeInput) -> Self {
        let file_arg = command_arg(&input.file);
        let render_command = matches!(input.family.as_str(), "docx" | "xlsx" | "pptx").then(|| {
            let render_dir_arg = command_arg(&format!("{}.render", input.file));
            format!("ooxml --json render {file_arg} --out {render_dir_arg}")
        });
        let layout_check_command = (input.family == "pptx")
            .then(|| format!("ooxml --json pptx validate-layout {file_arg}"));
        Self {
            file: input.file,
            family: input.family,
            command: input.command,
            destination: input.destination,
            changed: input.changed,
            readback_command: input.readback_command,
            validate_command: format!("ooxml --json validate --strict {file_arg}"),
            conformance_command: format!("ooxml --json conformance check {file_arg}"),
            check_command: format!("ooxml --json check {file_arg}"),
            render_command,
            layout_check_command,
            warnings: input.warnings,
            aliases_applied: input.aliases_applied,
            validated: input.validated,
        }
    }

    pub(crate) fn attach_to(self, response: &mut Value) -> CliResult<()> {
        let object = response.as_object_mut().ok_or_else(|| {
            CliError::unexpected("mutation result must be a JSON object before envelope attachment")
        })?;
        if object.contains_key("mutationEnvelope") {
            return Err(CliError::unexpected(
                "mutation result already contains mutationEnvelope",
            ));
        }
        object.insert(
            "mutationEnvelope".to_string(),
            serde_json::to_value(self).expect("serialize mutation envelope"),
        );
        Ok(())
    }
}

pub(crate) fn mutation_envelope_schema() -> CliResult<Value> {
    let schema: Value = serde_json::from_str(MUTATION_ENVELOPE_SCHEMA_JSON).map_err(|err| {
        CliError::unexpected(format!(
            "embedded mutation envelope schema is invalid: {err}"
        ))
    })?;
    if schema.get("$id").and_then(Value::as_str) != Some(MUTATION_ENVELOPE_SCHEMA_ID) {
        return Err(CliError::unexpected(
            "embedded mutation envelope schema has an unexpected $id",
        ));
    }
    Ok(schema)
}

pub(crate) fn schema_command(args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(args, &[], &[])?;
    Ok(json!({
        "schema": "mutation-envelope",
        "document": mutation_envelope_schema()?,
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn sample_envelope(family: &str) -> MutationEnvelope {
        MutationEnvelope::from_input(MutationEnvelopeInput {
            file: format!("out.{family}"),
            family: family.to_string(),
            command: format!("ooxml {family} sample mutate"),
            destination: MutationDestination {
                part_uri: "/word/document.xml".to_string(),
                primary_selector: "block:1".to_string(),
                selectors: vec!["block:1".to_string(), "paraId:00112233".to_string()],
                handle: "H:docx/main/para:id:00112233".to_string(),
                kind: "paragraph".to_string(),
                summary: Map::from_iter([("text".to_string(), json!("Hello"))]),
            },
            changed: vec![MutationChange {
                kind: "paragraph".to_string(),
                selector: "block:1".to_string(),
                handle: "H:docx/main/para:id:00112233".to_string(),
                before_hash: None,
                after_hash: Some(format!("sha256:{}", "a".repeat(64))),
            }],
            readback_command: format!("ooxml --json {family} sample show out.{family}"),
            warnings: Vec::new(),
            aliases_applied: Vec::new(),
            validated: true,
        })
    }

    #[test]
    fn schema_pins_required_fields_and_family_specific_proof_commands() {
        let schema = mutation_envelope_schema().expect("schema");
        assert_eq!(schema["$id"], MUTATION_ENVELOPE_SCHEMA_ID);
        assert_eq!(schema["additionalProperties"], false);
        let required = schema["required"].as_array().expect("required array");
        for field in [
            "file",
            "family",
            "command",
            "destination",
            "changed",
            "readbackCommand",
            "validateCommand",
            "conformanceCommand",
            "checkCommand",
            "warnings",
            "aliasesApplied",
            "validated",
        ] {
            assert!(required.iter().any(|value| value == field), "{field}");
        }
        assert_eq!(
            schema["allOf"][1]["then"]["required"][0],
            "layoutCheckCommand"
        );
    }

    #[test]
    fn attachment_preserves_legacy_keys_and_changed_boolean() {
        let mut response = json!({
            "file": "legacy.docx",
            "changed": true,
            "commandSpecificCount": 3,
        });
        sample_envelope("docx")
            .attach_to(&mut response)
            .expect("attach");
        assert_eq!(response["changed"], true);
        assert_eq!(response["commandSpecificCount"], 3);
        assert!(response["mutationEnvelope"]["changed"].is_array());
        assert_eq!(
            response["mutationEnvelope"]["checkCommand"],
            "ooxml --json check out.docx"
        );
        assert!(response["mutationEnvelope"]["renderCommand"].is_string());
        assert!(
            response["mutationEnvelope"]
                .get("layoutCheckCommand")
                .is_none()
        );
    }

    #[test]
    fn pptx_envelope_has_layout_and_render_commands() {
        let value = serde_json::to_value(sample_envelope("pptx")).expect("serialize");
        assert_eq!(
            value["layoutCheckCommand"],
            "ooxml --json pptx validate-layout out.pptx"
        );
        assert_eq!(
            value["renderCommand"],
            "ooxml --json render out.pptx --out out.pptx.render"
        );
        assert_eq!(value["aliasesApplied"], json!([]));
        assert_eq!(value["validated"], true);
    }

    #[test]
    fn docx_adoption_table_has_27_unique_mutating_leaf_commands() {
        let paths = DOCX_MUTATION_COMMANDS
            .iter()
            .map(|spec| spec.path.join(" "))
            .collect::<BTreeSet<_>>();
        assert_eq!(DOCX_MUTATION_COMMANDS.len(), 27);
        assert_eq!(paths.len(), DOCX_MUTATION_COMMANDS.len());
        assert!(paths.contains("docx scaffold"));
        assert!(paths.contains("docx images insert"));
        assert!(paths.contains("docx tables delete-row"));
        assert!(!paths.contains("docx blocks"));
        assert!(!paths.contains("docx styles list"));
    }
}
