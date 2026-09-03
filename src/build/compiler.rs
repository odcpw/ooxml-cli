use super::loader::BuildSpec;
use super::schema::BuildFamily;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const PLAN_SCHEMA_VERSION: &str = "ooxml-cli.build-plan.v1";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildOperation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub command: String,
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub args: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanNode {
    pub op_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec_id: Option<String>,
    pub result_ref: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledBuildPlan {
    pub schema_version: &'static str,
    pub family: BuildFamily,
    pub operations: Vec<BuildOperation>,
    pub node_map: BTreeMap<String, PlanNode>,
}

impl CompiledBuildPlan {
    pub fn operations_json(&self) -> Value {
        serde_json::to_value(&self.operations).expect("build operations are JSON serializable")
    }
}

pub(super) fn execution_error_with_spec_path(
    plan: &CompiledBuildPlan,
    mut error: crate::CliError,
) -> crate::CliError {
    let Some(operation_index) = operation_index_from_apply_error(&error.message) else {
        return error;
    };
    let Some(operation) = plan.operations.get(operation_index) else {
        return error;
    };
    let Some(op_id) = operation.id.as_deref() else {
        return error;
    };
    let Some(base_path) = plan
        .node_map
        .iter()
        .filter(|(_, node)| node.op_id == op_id)
        .map(|(path, _)| path.as_str())
        .min_by_key(|path| path.matches('/').count())
    else {
        return error;
    };

    let (code, path, suggestion) = match (operation.command.as_str(), error.code) {
        ("pptx new-slide-from-layout", _) => (
            "BUILD_SPEC_VALUE_INVALID",
            format!("{base_path}/layout"),
            "use a layout name reported by `ooxml pptx layouts list <template.pptx>`; built-in layouts include \"Title Slide\" and \"Title Only\"",
        ),
        ("pptx place image", "file_not_found") => (
            "BUILD_SPEC_FILE_READ_FAILED",
            format!("{base_path}/path"),
            "use an existing image path relative to the build spec file",
        ),
        ("xlsx ranges set", "file_not_found") => (
            "BUILD_SPEC_FILE_READ_FAILED",
            format!("{base_path}/dataFile/path"),
            "use an existing data-file path relative to the build spec file",
        ),
        ("docx images insert", "file_not_found") => (
            "BUILD_SPEC_FILE_READ_FAILED",
            format!("{base_path}/image/path"),
            "use an existing image path relative to the build spec file",
        ),
        _ => return error,
    };
    let diagnostic = BuildCompileError {
        code: code.to_string(),
        path,
        op_id: Some(op_id.to_string()),
        message: format!("{}; suggestion: {suggestion}", error.message),
    };
    error.message = serde_json::to_string(&diagnostic).unwrap_or_else(|_| diagnostic.to_string());
    error
}

fn operation_index_from_apply_error(message: &str) -> Option<usize> {
    let remainder = message.strip_prefix("op ")?;
    let (index, _) = remainder.split_once(' ')?;
    index.parse().ok()
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildCompileError {
    pub code: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_id: Option<String>,
    pub message: String,
}

impl fmt::Display for BuildCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for BuildCompileError {}

pub struct BuildCompiler {
    family: BuildFamily,
    operations: Vec<BuildOperation>,
    node_map: BTreeMap<String, PlanNode>,
    ids: BTreeSet<String>,
}

impl BuildCompiler {
    pub fn new(family: BuildFamily) -> Self {
        Self {
            family,
            operations: Vec::new(),
            node_map: BTreeMap::new(),
            ids: BTreeSet::new(),
        }
    }

    pub fn push_operation(
        &mut self,
        node_path: impl Into<String>,
        spec_id: Option<&str>,
        op_id: impl Into<String>,
        command: impl Into<String>,
        args: Map<String, Value>,
        result_path: &str,
    ) -> Result<(), BuildCompileError> {
        let node_path = normalized_node_path(node_path.into());
        let op_id = op_id.into();
        validate_op_id(&op_id)
            .map_err(|message| compile_error(&node_path, Some(&op_id), message))?;
        if self.ids.contains(&op_id) {
            return Err(compile_error(
                &node_path,
                Some(&op_id),
                format!("duplicate build operation id {op_id:?}"),
            ));
        }
        let command = command.into();
        validate_command(self.family, &command)
            .map_err(|message| compile_error(&node_path, Some(&op_id), message))?;
        validate_args(&args).map_err(|message| compile_error(&node_path, Some(&op_id), message))?;
        validate_reference_targets(&args, &self.ids)
            .map_err(|message| compile_error(&node_path, Some(&op_id), message))?;
        let result_ref = operation_reference(&op_id, result_path)
            .map_err(|message| compile_error(&node_path, Some(&op_id), message))?;
        if self.node_map.contains_key(&node_path) {
            return Err(compile_error(
                &node_path,
                Some(&op_id),
                "a compiled operation is already mapped to this spec node",
            ));
        }
        self.ids.insert(op_id.clone());
        self.operations.push(BuildOperation {
            id: Some(op_id.clone()),
            command,
            args,
        });
        self.node_map.insert(
            node_path,
            PlanNode {
                op_id,
                spec_id: spec_id.map(str::to_string),
                result_ref,
            },
        );
        Ok(())
    }

    /// Append an implementation operation that has no corresponding user spec node.
    ///
    /// Family compilers use this for bounded housekeeping such as removing the
    /// seed slide after creating the first requested slide. The operation is
    /// still validated and participates in ordered `$ref` resolution, but it
    /// is deliberately absent from the public spec-node map.
    pub fn push_internal_operation(
        &mut self,
        op_id: impl Into<String>,
        command: impl Into<String>,
        args: Map<String, Value>,
    ) -> Result<(), BuildCompileError> {
        let op_id = op_id.into();
        let node_path = "/";
        validate_op_id(&op_id)
            .map_err(|message| compile_error(node_path, Some(&op_id), message))?;
        if self.ids.contains(&op_id) {
            return Err(compile_error(
                node_path,
                Some(&op_id),
                format!("duplicate build operation id {op_id:?}"),
            ));
        }
        let command = command.into();
        validate_command(self.family, &command)
            .map_err(|message| compile_error(node_path, Some(&op_id), message))?;
        validate_args(&args).map_err(|message| compile_error(node_path, Some(&op_id), message))?;
        validate_reference_targets(&args, &self.ids)
            .map_err(|message| compile_error(node_path, Some(&op_id), message))?;
        self.ids.insert(op_id.clone());
        self.operations.push(BuildOperation {
            id: Some(op_id),
            command,
            args,
        });
        Ok(())
    }

    pub fn map_node(
        &mut self,
        node_path: impl Into<String>,
        spec_id: Option<&str>,
        op_id: &str,
        result_path: &str,
    ) -> Result<(), BuildCompileError> {
        let node_path = normalized_node_path(node_path.into());
        if !self.ids.contains(op_id) {
            return Err(compile_error(
                &node_path,
                Some(op_id),
                format!("cannot map node to unknown build operation {op_id:?}"),
            ));
        }
        if self.node_map.contains_key(&node_path) {
            return Err(compile_error(
                &node_path,
                Some(op_id),
                "a compiled operation is already mapped to this spec node",
            ));
        }
        let result_ref = operation_reference(op_id, result_path)
            .map_err(|message| compile_error(&node_path, Some(op_id), message))?;
        self.node_map.insert(
            node_path,
            PlanNode {
                op_id: op_id.to_string(),
                spec_id: spec_id.map(str::to_string),
                result_ref,
            },
        );
        Ok(())
    }

    pub fn finish(self) -> Result<CompiledBuildPlan, BuildCompileError> {
        Ok(CompiledBuildPlan {
            schema_version: PLAN_SCHEMA_VERSION,
            family: self.family,
            operations: self.operations,
            node_map: self.node_map,
        })
    }
}

pub fn operation_reference(op_id: &str, path: &str) -> Result<Value, String> {
    validate_op_id(op_id)?;
    if path.is_empty()
        || path.starts_with('.')
        || path.ends_with('.')
        || path.split('.').any(str::is_empty)
    {
        return Err(format!(
            "operation reference path {path:?} must contain non-empty dot-separated fields"
        ));
    }
    Ok(json!({"$ref": format!("{op_id}.{path}")}))
}

pub fn compile_minimal_spec(spec: &BuildSpec) -> Result<CompiledBuildPlan, BuildCompileError> {
    let document = spec
        .document()
        .as_object()
        .expect("validated build spec root is an object");
    let mut compiler = BuildCompiler::new(spec.family());
    match spec.family() {
        BuildFamily::Pptx => compile_minimal_pptx(document, &mut compiler)?,
        BuildFamily::Xlsx => compile_minimal_xlsx(document, &mut compiler)?,
        BuildFamily::Docx => compile_minimal_docx(document, &mut compiler)?,
    }
    compiler.finish()
}

fn compile_minimal_pptx(
    document: &Map<String, Value>,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    require_only_minimal_fields(
        document,
        &[
            "schemaVersion",
            "family",
            "theme",
            "themeSeed",
            "template",
            "brand",
            "size",
            "slides",
        ],
        "/",
    )?;
    let slides = document["slides"]
        .as_array()
        .expect("validated slides array");
    require_one_minimal_node(slides.len(), "/slides")?;
    let slide = slides[0].as_object().expect("validated slide object");
    require_only_minimal_fields(slide, &["id", "layout", "title", "subtitle"], "/slides/0")?;
    require_minimal_variant(slide, "layout", "Title Slide", "/slides/0/layout")?;
    let mut args = Map::new();
    copy_string_arg(slide, "title", "title", &mut args);
    copy_string_arg(slide, "subtitle", "subtitle", &mut args);
    for field in ["theme", "themeSeed", "template", "size"] {
        copy_value_arg(document, field, field, &mut args);
    }
    copy_brand_arg(document, &mut args);
    let spec_id = slide.get("id").and_then(Value::as_str);
    compiler.push_operation("/", None, "document", "pptx scaffold", args, "destination")?;
    compiler.map_node(
        "/slides/0",
        spec_id,
        "document",
        "destination.primarySelector",
    )
}

fn compile_minimal_xlsx(
    document: &Map<String, Value>,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    require_only_minimal_fields(
        document,
        &[
            "schemaVersion",
            "family",
            "theme",
            "themeSeed",
            "brand",
            "sheets",
        ],
        "/",
    )?;
    let sheets = document["sheets"]
        .as_array()
        .expect("validated sheets array");
    require_one_minimal_node(sheets.len(), "/sheets")?;
    let sheet = sheets[0].as_object().expect("validated sheet object");
    require_only_minimal_fields(sheet, &["id", "name"], "/sheets/0")?;
    let mut args = Map::new();
    copy_string_arg(sheet, "name", "sheet", &mut args);
    for field in ["theme", "themeSeed"] {
        copy_value_arg(document, field, field, &mut args);
    }
    copy_brand_arg(document, &mut args);
    let spec_id = sheet.get("id").and_then(Value::as_str);
    compiler.push_operation("/", None, "document", "xlsx scaffold", args, "destination")?;
    compiler.map_node(
        "/sheets/0",
        spec_id,
        "document",
        "destination.primarySelector",
    )
}

fn compile_minimal_docx(
    document: &Map<String, Value>,
    compiler: &mut BuildCompiler,
) -> Result<(), BuildCompileError> {
    require_only_minimal_fields(
        document,
        &[
            "schemaVersion",
            "family",
            "theme",
            "themeSeed",
            "template",
            "brand",
            "blocks",
        ],
        "/",
    )?;
    let blocks = document["blocks"]
        .as_array()
        .expect("validated blocks array");
    require_one_minimal_node(blocks.len(), "/blocks")?;
    let block = blocks[0].as_object().expect("validated block object");
    require_only_minimal_fields(block, &["id", "type", "text"], "/blocks/0")?;
    require_minimal_variant(block, "type", "paragraph", "/blocks/0/type")?;
    let mut args = Map::new();
    copy_string_arg(block, "text", "text", &mut args);
    for field in ["theme", "themeSeed", "template"] {
        copy_value_arg(document, field, field, &mut args);
    }
    copy_brand_arg(document, &mut args);
    let spec_id = block.get("id").and_then(Value::as_str);
    compiler.push_operation("/", None, "document", "docx scaffold", args, "destination")?;
    compiler.map_node(
        "/blocks/0",
        spec_id,
        "document",
        "destination.summary.blockHashes.0.index",
    )
}

fn require_one_minimal_node(count: usize, path: &str) -> Result<(), BuildCompileError> {
    if count == 1 {
        return Ok(());
    }
    Err(BuildCompileError {
        code: "BUILD_SPEC_FAMILY_COMPILER_REQUIRED".to_string(),
        path: path.to_string(),
        op_id: None,
        message: format!(
            "the shared core compiler accepts one minimal node; {count} nodes require the dedicated family build compiler"
        ),
    })
}

fn require_only_minimal_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), BuildCompileError> {
    if let Some(field) = object
        .keys()
        .filter(|field| !allowed.contains(&field.as_str()))
        .min()
    {
        let path = if path == "/" {
            format!("/{field}")
        } else {
            format!("{path}/{field}")
        };
        return Err(family_compiler_required(
            &path,
            format!("field {field:?} requires the dedicated family build compiler"),
        ));
    }
    Ok(())
}

fn require_minimal_variant(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
    path: &str,
) -> Result<(), BuildCompileError> {
    let actual = object[field]
        .as_str()
        .expect("validated minimal variant is a string");
    if actual == expected {
        return Ok(());
    }
    Err(family_compiler_required(
        path,
        format!(
            "{field} {actual:?} requires the dedicated family build compiler; the shared minimal compiler supports {expected:?}"
        ),
    ))
}

fn family_compiler_required(path: &str, message: impl Into<String>) -> BuildCompileError {
    BuildCompileError {
        code: "BUILD_SPEC_FAMILY_COMPILER_REQUIRED".to_string(),
        path: path.to_string(),
        op_id: None,
        message: message.into(),
    }
}

fn copy_string_arg(
    source: &Map<String, Value>,
    source_name: &str,
    arg_name: &str,
    target: &mut Map<String, Value>,
) {
    if let Some(value) = source.get(source_name).and_then(Value::as_str) {
        target.insert(arg_name.to_string(), Value::String(value.to_string()));
    }
}

fn copy_value_arg(
    source: &Map<String, Value>,
    source_name: &str,
    arg_name: &str,
    target: &mut Map<String, Value>,
) {
    if let Some(value) = source.get(source_name) {
        target.insert(arg_name.to_string(), value.clone());
    }
}

fn copy_brand_arg(document: &Map<String, Value>, args: &mut Map<String, Value>) {
    let Some(brand) = document.get("brand") else {
        return;
    };
    let value = brand.as_str().map(str::to_string).or_else(|| {
        let brand = brand.as_object()?;
        brand
            .get("path")
            .or_else(|| brand.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    if let Some(value) = value {
        args.insert("brand".to_string(), Value::String(value));
    }
}

fn validate_op_id(op_id: &str) -> Result<(), String> {
    if op_id.is_empty()
        || op_id.len() > 128
        || !op_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':')
        })
    {
        return Err(format!(
            "build operation id {op_id:?} must contain 1-128 ASCII letters, digits, '-', '_' or ':'"
        ));
    }
    Ok(())
}

fn validate_command(family: BuildFamily, command: &str) -> Result<(), String> {
    let normalized = command.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized != command || command.is_empty() {
        return Err("operation command must be a normalized command path".to_string());
    }
    if command.starts_with("ooxml ") || command.split_whitespace().any(|word| word.starts_with('-'))
    {
        return Err(
            "operation command contains only canonical command words, without ooxml or flags"
                .to_string(),
        );
    }
    if command.split_whitespace().next() != Some(family.as_str()) {
        return Err(format!(
            "operation command {command:?} does not belong to the {} build family",
            family
        ));
    }
    Ok(())
}

fn validate_args(args: &Map<String, Value>) -> Result<(), String> {
    for (key, value) in args {
        let normalized = key
            .chars()
            .filter(|character| !matches!(character, '-' | '_'))
            .flat_map(char::to_lowercase)
            .collect::<String>();
        if matches!(
            normalized.as_str(),
            "file"
                | "out"
                | "output"
                | "inplace"
                | "dryrun"
                | "backup"
                | "novalidate"
                | "session"
                | "json"
                | "pretty"
                | "nocolor"
                | "keeptemp"
                | "tempdir"
                | "verbosity"
                | "strict"
                | "help"
                | "h"
                | "o"
                | "v"
        ) {
            return Err(format!(
                "operation arg {key:?} is session-owned and must be omitted from a build plan"
            ));
        }
        validate_reference_shape(value)?;
    }
    Ok(())
}

fn validate_reference_targets(
    args: &Map<String, Value>,
    seen: &BTreeSet<String>,
) -> Result<(), String> {
    for reference in references_in(&Value::Object(args.clone())) {
        let (op_id, _) = split_reference(reference)?;
        if !seen.contains(op_id) {
            return Err(format!(
                "operation reference {reference:?} is unresolved; refs may name only an earlier op"
            ));
        }
    }
    Ok(())
}

fn validate_reference_shape(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(object) if object.contains_key("$ref") => {
            if object.len() != 1 {
                return Err("a $ref leaf must contain exactly the $ref field".to_string());
            }
            let reference = object["$ref"]
                .as_str()
                .ok_or_else(|| "$ref must be a string".to_string())?;
            split_reference(reference).map(|_| ())
        }
        Value::Object(object) => object.values().try_for_each(validate_reference_shape),
        Value::Array(items) => items.iter().try_for_each(validate_reference_shape),
        _ => Ok(()),
    }
}

fn references_in(value: &Value) -> Vec<&str> {
    let mut references = Vec::new();
    collect_references(value, &mut references);
    references
}

fn collect_references<'a>(value: &'a Value, references: &mut Vec<&'a str>) {
    match value {
        Value::Object(object) if object.len() == 1 => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                references.push(reference);
            } else {
                for value in object.values() {
                    collect_references(value, references);
                }
            }
        }
        Value::Object(object) => {
            for value in object.values() {
                collect_references(value, references);
            }
        }
        Value::Array(items) => {
            for value in items {
                collect_references(value, references);
            }
        }
        _ => {}
    }
}

fn split_reference(reference: &str) -> Result<(&str, &str), String> {
    let Some((op_id, path)) = reference.split_once('.') else {
        return Err(format!(
            "operation reference {reference:?} must be <op-id>.<path>"
        ));
    };
    validate_op_id(op_id)?;
    if path.is_empty() || path.split('.').any(str::is_empty) {
        return Err(format!(
            "operation reference {reference:?} must contain a non-empty result path"
        ));
    }
    Ok((op_id, path))
}

fn normalized_node_path(path: String) -> String {
    if path.is_empty() {
        "/".to_string()
    } else if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    }
}

fn compile_error(path: &str, op_id: Option<&str>, message: impl Into<String>) -> BuildCompileError {
    BuildCompileError {
        code: "BUILD_SPEC_COMPILE_FAILED".to_string(),
        path: path.to_string(),
        op_id: op_id.map(str::to_string),
        message: message.into(),
    }
}
