use serde_json::{Map, Value, json};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::build::{
    BuildFamily, BuildSpec, CompiledBuildPlan, compile_docx_spec, compile_pptx_spec,
    compile_xlsx_spec, load_spec_str, markdown_to_spec,
};
use crate::cli_dispatch::{DispatchBody, DispatchOutput};
use crate::{
    CliError, CliResult, GlobalFlags, OutlineOptions, ServeState, json_bool, json_optional_string,
    json_string, json_u32, mcp_tool_success,
};

const TYPED_TOOL_NAMES: [&str; 10] = [
    "build_presentation",
    "build_workbook",
    "build_document",
    "edit_package",
    "outline_package",
    "check_package",
    "validate_package",
    "render_preview",
    "find_text",
    "replace_text",
];

pub(super) const fn tool_names() -> &'static [&'static str] {
    &TYPED_TOOL_NAMES
}

pub(super) fn is_typed_tool(name: &str) -> bool {
    TYPED_TOOL_NAMES.contains(&name)
}

pub(super) fn tools() -> Vec<Value> {
    vec![
        build_tool("build_presentation", BuildFamily::Pptx),
        build_tool("build_workbook", BuildFamily::Xlsx),
        build_tool("build_document", BuildFamily::Docx),
        json!({
            "name": "edit_package",
            "description": "Apply an ordered batch of manifest-derived mutation operations in one call. Operations may name results with id and consume them through exact {$ref: id.destination.field} objects. The package is staged, strictly validated, and published atomically unless dryRun is true.",
            "inputSchema": edit_package_schema(),
        }),
        json!({
            "name": "outline_package",
            "description": "Orient on a package in one read: a deterministic family-aware tree with stable selectors, handles, previews, and optional slide/sheet/section scope. Equivalent to ooxml outline.",
            "inputSchema": object_schema(
                json!({
                    "file": {"type": "string", "minLength": 1},
                    "depth": {"type": "integer", "minimum": 0, "maximum": 3, "default": 3},
                    "textPreview": {"type": "integer", "minimum": 0, "default": 80},
                    "slide": {"type": "integer", "minimum": 1},
                    "sheet": {"type": "string", "minLength": 1},
                    "section": {"type": "integer", "minimum": 1},
                }),
                &["file"],
            ),
        }),
        json!({
            "name": "check_package",
            "description": "Run the unified structural, strict, schema, design, reference, and optional visual proof recipe. Returns the same proofLevel, summary, findings, fixCommand, and docs contract as ooxml check.",
            "inputSchema": object_schema(
                json!({
                    "file": {"type": "string", "minLength": 1},
                    "openXmlSdk": {"type": "string", "enum": ["auto", "require", "skip"], "default": "auto"},
                    "failOn": {"type": "string", "enum": ["error", "warning"], "default": "error"},
                    "render": {"type": "boolean", "default": false},
                    "fix": {"type":"boolean","default":false,"description":"Apply available fixes atomically; default output is a sibling *.fixed.* package."},
                    "dryRun": {"type":"boolean","default":false},
                    "output": {"type":"string","minLength":1},
                    "inPlace": {"type":"boolean","default":false},
                    "backup": {"type":"string","minLength":1},
                    "maxRounds": {"type":"integer","minimum":1,"maximum":100,"default":8},
                }),
                &["file"],
            ),
        }),
        json!({
            "name": "validate_package",
            "description": "Run strict package validation in one read-only call. Use check_package when schema, reference, design, or visual proof is also required.",
            "inputSchema": object_schema(
                json!({"file": {"type": "string", "minLength": 1}}),
                &["file"],
            ),
        }),
        json!({
            "name": "render_preview",
            "description": "Render a PPTX, XLSX, or DOCX package to PDF and PNG preview pages through the shared renderer. Returns deterministic image paths, or imageBase64 when includeBase64 is requested.",
            "inputSchema": object_schema(
                json!({
                    "file": {"type": "string", "minLength": 1},
                    "out": {"type": "string", "minLength": 1},
                    "dpi": {"type": "integer", "minimum": 1, "maximum": 1200, "default": 144},
                    "pages": {"type": "string", "minLength": 1},
                    "slides": {"type": "string", "minLength": 1},
                    "sheet": {"type": "string", "minLength": 1},
                    "includeBase64": {"type": "boolean", "default": false},
                }),
                &["file", "out"],
            ),
        }),
        json!({
            "name": "find_text",
            "description": "Find text, formulas, or defined names with the same deterministic hit envelope and selectors as ooxml find. Use replace_text to publish a replacement in one call.",
            "inputSchema": object_schema(
                json!({
                    "file": {"type": "string", "minLength": 1},
                    "query": {"type": "string", "minLength": 1},
                    "type": {"type": "string", "enum": ["all", "text", "formula", "name"], "default": "all"},
                    "ignoreCase": {"type": "boolean", "default": false},
                    "regex": {"type": "boolean", "default": false},
                    "max": {"type": "integer", "minimum": 0, "default": 0},
                }),
                &["file", "query"],
            ),
        }),
        json!({
            "name": "replace_text",
            "description": "Find exact text and atomically publish all supported replacements in one call through the shared mutation seam. Returns the same apply envelope as the equivalent ooxml find --apply invocation.",
            "inputSchema": object_schema(
                json!({
                    "file": {"type": "string", "minLength": 1},
                    "query": {"type": "string", "minLength": 1},
                    "replacement": {"type": "string", "minLength": 1},
                    "output": {"type": "string", "minLength": 1},
                    "ignoreCase": {"type": "boolean", "default": false},
                    "regex": {"type": "boolean", "default": false},
                    "noValidate": {"type": "boolean", "default": false},
                    "dryRun": {"type": "boolean", "default": false},
                }),
                &["file", "query", "replacement", "output"],
            ),
        }),
    ]
}

pub(super) fn call(engine: &mut ServeState, name: &str, arguments: &Value) -> Value {
    match call_inner(engine, name, arguments) {
        Ok(payload) => mcp_tool_success(name, payload, next_actions(name)),
        Err(error) => typed_tool_error(name, error),
    }
}

fn call_inner(engine: &mut ServeState, name: &str, arguments: &Value) -> CliResult<Value> {
    reject_unknown_arguments(name, arguments)?;
    match name {
        "build_presentation" => call_build(engine, BuildFamily::Pptx, arguments),
        "build_workbook" => call_build(engine, BuildFamily::Xlsx, arguments),
        "build_document" => call_build(engine, BuildFamily::Docx, arguments),
        "edit_package" => call_edit(engine, arguments),
        "outline_package" => call_outline(arguments),
        "check_package" => call_check(arguments),
        "validate_package" => crate::validate(&json_string(arguments, "file")?, true),
        "render_preview" => call_render(arguments),
        "find_text" => call_find(arguments, false),
        "replace_text" => call_find(arguments, true),
        _ => Err(CliError::invalid_args(format!(
            "unknown typed MCP tool: {name}"
        ))),
    }
}

fn call_build(engine: &mut ServeState, family: BuildFamily, arguments: &Value) -> CliResult<Value> {
    let (spec_source, markdown_source, markdown_warnings) =
        match (arguments.get("spec"), arguments.get("markdown")) {
            (Some(_), Some(_)) | (None, None) => {
                return Err(CliError::invalid_args(
                    "exactly one of spec or markdown is required",
                ));
            }
            (Some(spec), None) => (
                serde_json::to_string(spec).map_err(|error| {
                    CliError::invalid_args(format!("spec must be valid JSON: {error}"))
                })?,
                None,
                Vec::new(),
            ),
            (None, Some(_)) => {
                let markdown = json_string(arguments, "markdown")?;
                let conversion = markdown_to_spec(family, &markdown, "inline.md")
                    .map_err(markdown_conversion_error)?;
                let spec_source = serde_json::to_string(&conversion.spec).map_err(|error| {
                    CliError::invalid_args(format!(
                        "converted Markdown spec must be valid JSON: {error}"
                    ))
                })?;
                (spec_source, Some(markdown), conversion.warnings)
            }
        };
    let spec = load_spec_str(family, &spec_source).map_err(|error| {
        CliError::invalid_args(format!(
            "build spec validation failed: {}",
            serde_json::to_string(&error.diagnostics).expect("serialize build diagnostics")
        ))
    })?;
    let has_output = json_optional_string(arguments, "output").is_some();
    let has_session = json_optional_string(arguments, "session").is_some();
    if has_output == has_session {
        return Err(CliError::invalid_args(
            "exactly one of output or session is required",
        ));
    }
    if has_output {
        return call_family_build(family, &spec_source, markdown_source.as_deref(), arguments);
    }
    if ["check", "dryRun", "force"]
        .iter()
        .any(|field| json_bool(arguments, field).unwrap_or(false))
    {
        return Err(CliError::invalid_args(
            "check, dryRun, and force apply only when output is selected; omit them when applying a build spec to an existing session",
        ));
    }
    let plan = compile_session_plan(family, &spec)?;
    let mut operations = plan.operations_json();
    if let Some(operations) = operations.as_array_mut() {
        for operation in operations {
            operation
                .as_object_mut()
                .expect("compiled build operations are objects")
                .entry("args")
                .or_insert_with(|| json!({}));
        }
    }
    let node_map = serde_json::to_value(&plan.node_map).expect("serialize build node map");
    let session = json_string(arguments, "session")?;
    let applied = apply_operations(engine, &session, &operations)?;
    let mut result = json!({
        "schemaVersion": "ooxml-cli.typed-build.v1",
        "family": family,
        "schemaResource": format!("resource://schema/{}", family.schema_name()),
        "sessionId": session,
        "committed": false,
        "operations": operations,
        "nodeMap": node_map,
        "applied": applied,
    });
    if markdown_source.is_some() {
        result["markdown"] = json!("inline");
    }
    if !markdown_warnings.is_empty() {
        result["warnings"] =
            serde_json::to_value(markdown_warnings).expect("Markdown warnings serialize");
    }
    Ok(result)
}

fn compile_session_plan(family: BuildFamily, spec: &BuildSpec) -> CliResult<CompiledBuildPlan> {
    let plan = match family {
        BuildFamily::Pptx => {
            let compiled = compile_pptx_spec(spec).map_err(build_compilation_error)?;
            if !compiled.assets.is_empty() {
                return Err(CliError::invalid_args(
                    "presentation specs with generated chart or table assets require output mode; pass output instead of session",
                ));
            }
            compiled.plan
        }
        BuildFamily::Xlsx => {
            compile_xlsx_spec(spec)
                .map_err(build_compilation_error)?
                .plan
        }
        BuildFamily::Docx => {
            compile_docx_spec(spec)
                .map_err(build_compilation_error)?
                .plan
        }
    };
    Ok(plan)
}

fn build_compilation_error(error: crate::build::BuildCompileError) -> CliError {
    CliError::invalid_args(format!(
        "build spec compilation failed: {}",
        serde_json::to_string(&error).expect("serialize build compilation diagnostic")
    ))
}

fn markdown_conversion_error(error: crate::build::MarkdownError) -> CliError {
    CliError::invalid_args(format!(
        "Markdown conversion failed: {}",
        serde_json::to_string(&error).expect("serialize Markdown diagnostic")
    ))
}

fn call_family_build(
    family: BuildFamily,
    spec_source: &str,
    markdown_source: Option<&str>,
    arguments: &Value,
) -> CliResult<Value> {
    let (source, input_flag, extension, result_field) = match markdown_source {
        Some(markdown) => (markdown, "--from-markdown", "md", "markdown"),
        None => (spec_source, "--spec", "json", "spec"),
    };
    let input_path = temporary_input_path(family.as_str(), extension)?;
    fs::write(&input_path, source).map_err(|error| {
        CliError::unexpected(format!(
            "failed to stage typed {family} build input {}: {error}",
            input_path.display()
        ))
    })?;
    let mut args = vec![
        family.as_str().to_string(),
        "build".to_string(),
        input_flag.to_string(),
        input_path.to_string_lossy().to_string(),
        "--out".to_string(),
        json_string(arguments, "output")?,
    ];
    push_bool_flag(arguments, "check", "--check", &mut args);
    push_bool_flag(arguments, "dryRun", "--dry-run", &mut args);
    push_bool_flag(arguments, "force", "--force", &mut args);
    let result = crate::dispatch(
        &GlobalFlags {
            json: true,
            ..GlobalFlags::default()
        },
        &args,
    )
    .and_then(dispatch_json)
    .map(|mut result| {
        result[result_field] = json!("inline");
        result
    });
    let _ = fs::remove_file(input_path);
    result
}

fn temporary_input_path(family: &str, extension: &str) -> CliResult<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| CliError::unexpected(format!("system clock before epoch: {error}")))?
        .as_nanos();
    let directory = std::env::current_dir().map_err(|error| {
        CliError::unexpected(format!("failed to resolve MCP working directory: {error}"))
    })?;
    Ok(directory.join(format!(
        ".ooxml-mcp-{family}-build-{}-{nanos}.{extension}",
        std::process::id(),
    )))
}

fn call_edit(engine: &mut ServeState, arguments: &Value) -> CliResult<Value> {
    let operations = arguments
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::invalid_args("operations must be a non-empty array"))?;
    if operations.is_empty() {
        return Err(CliError::invalid_args(
            "operations must be a non-empty array; discover accepted command and args shapes in the edit_package input schema",
        ));
    }
    let open = json!({
        "file": json_string(arguments, "file")?,
        "out": json_optional_string(arguments, "output"),
        "inPlace": json_bool(arguments, "inPlace").unwrap_or(false),
        "backup": json_optional_string(arguments, "backup"),
        "noValidate": json_bool(arguments, "noValidate").unwrap_or(false),
        "dryRun": json_bool(arguments, "dryRun").unwrap_or(false),
    });
    let session = open_session(engine, &open)?;
    let result = (|| {
        let applied = apply_operations(engine, &session, &Value::Array(operations.clone()))?;
        let plan = engine.handle_method("plan", &json!({"session": session}))?;
        let commit = engine.handle_method("commit", &json!({"session": session}))?;
        Ok(json!({
            "schemaVersion": "ooxml-cli.typed-edit.v1",
            "sessionId": session,
            "applied": applied,
            "plan": plan,
            "commit": commit,
        }))
    })();
    if result.is_err() {
        let _ = engine.handle_method("abort", &json!({"session": session}));
    }
    result
}

fn open_session(engine: &mut ServeState, arguments: &Value) -> CliResult<String> {
    let opened = engine.handle_method("open", arguments)?;
    opened
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| CliError::unexpected("typed MCP open returned no sessionId"))
}

fn apply_operations(
    engine: &mut ServeState,
    session: &str,
    operations: &Value,
) -> CliResult<Vec<Value>> {
    let operations = operations
        .as_array()
        .ok_or_else(|| CliError::invalid_args("operations must be an array"))?;
    let mut applied = Vec::with_capacity(operations.len());
    for (index, operation) in operations.iter().enumerate() {
        let object = operation.as_object().ok_or_else(|| {
            CliError::invalid_args(format!("operations/{index} must be an object"))
        })?;
        if let Some(field) = object
            .keys()
            .find(|field| !matches!(field.as_str(), "id" | "command" | "args"))
        {
            return Err(CliError::invalid_args(format!(
                "unknown argument {field:?} at operations/{index}; valid fields: id, command, args"
            )));
        }
        let command = object
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CliError::invalid_args(format!("operations/{index}/command is required"))
            })?;
        let args = object
            .get("args")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                CliError::invalid_args(format!("operations/{index}/args is required"))
            })?;
        let mut request = json!({
            "session": session,
            "command": command,
            "args": args,
        });
        if let Some(id) = object.get("id") {
            request["id"] = id.clone();
        }
        applied.push(engine.handle_method("op", &request)?);
    }
    Ok(applied)
}

fn call_outline(arguments: &Value) -> CliResult<Value> {
    let file = json_string(arguments, "file")?;
    let sheet = json_optional_string(arguments, "sheet");
    crate::outline(
        &file,
        OutlineOptions {
            depth: json_u32(arguments, "depth")?.unwrap_or(3),
            text_preview: json_u32(arguments, "textPreview")?.unwrap_or(80) as usize,
            slide: json_u32(arguments, "slide")?,
            sheet: sheet.as_deref(),
            section: json_u32(arguments, "section")?,
        },
    )
}

fn call_check(arguments: &Value) -> CliResult<Value> {
    let file = json_string(arguments, "file")?;
    let args = json!({
        "openxmlSdk": json_optional_string(arguments, "openXmlSdk").unwrap_or_else(|| "auto".to_string()),
        "failOn": json_optional_string(arguments, "failOn").unwrap_or_else(|| "error".to_string()),
        "render": json_bool(arguments, "render").unwrap_or(false),
    });
    let mut argv = Vec::new();
    for (field, flag) in [
        ("fix", "--fix"),
        ("dryRun", "--dry-run"),
        ("inPlace", "--in-place"),
        ("render", "--render"),
    ] {
        if json_bool(arguments, field).unwrap_or(false) {
            argv.push(flag.to_string());
        }
    }
    for (field, flag) in [
        ("output", "--out"),
        ("backup", "--backup"),
        ("openXmlSdk", "--openxml-sdk"),
        ("failOn", "--fail-on"),
    ] {
        push_string_flag(arguments, field, flag, &mut argv);
    }
    push_u32_flag(arguments, "maxRounds", "--max-rounds", &mut argv)?;
    if arguments.get("fix").is_some()
        || ["dryRun", "inPlace", "output", "backup", "maxRounds"]
            .iter()
            .any(|key| arguments.get(key).is_some())
    {
        let result = crate::check::dispatch(&crate::GlobalFlags::default(), &file, &argv)?;
        match result.body {
            crate::cli_dispatch::DispatchBody::Json(report) => Ok(report),
            crate::cli_dispatch::DispatchBody::Text(_) => {
                Err(CliError::unexpected("check returned text to typed MCP"))
            }
        }
    } else {
        crate::check::inspect(&file, &args)
    }
}

fn call_render(arguments: &Value) -> CliResult<Value> {
    let file = json_string(arguments, "file")?;
    let mut args = vec!["--out".to_string(), json_string(arguments, "out")?];
    push_u32_flag(arguments, "dpi", "--dpi", &mut args)?;
    push_string_flag(arguments, "pages", "--pages", &mut args);
    push_string_flag(arguments, "slides", "--slides", &mut args);
    push_string_flag(arguments, "sheet", "--sheet", &mut args);
    let mut rendered = crate::render::render_command(&file, &args)?;
    if json_bool(arguments, "includeBase64").unwrap_or(false) {
        attach_base64_images(&mut rendered)?;
    }
    Ok(rendered)
}

fn call_find(arguments: &Value, replace: bool) -> CliResult<Value> {
    let mut args = vec![
        json_string(arguments, "query")?,
        json_string(arguments, "file")?,
    ];
    push_string_flag(arguments, "type", "--type", &mut args);
    push_u32_flag(arguments, "max", "--max", &mut args)?;
    push_bool_flag(arguments, "ignoreCase", "--ignore-case", &mut args);
    push_bool_flag(arguments, "regex", "--regex", &mut args);
    if replace {
        args.extend([
            "--replace".to_string(),
            json_string(arguments, "replacement")?,
            "--apply".to_string(),
            "--out".to_string(),
            json_string(arguments, "output")?,
        ]);
        push_bool_flag(arguments, "noValidate", "--no-validate", &mut args);
        push_bool_flag(arguments, "dryRun", "--dry-run", &mut args);
    }
    dispatch_json(crate::find::find(
        &GlobalFlags {
            json: true,
            ..GlobalFlags::default()
        },
        &args,
    )?)
}

fn dispatch_json(output: DispatchOutput) -> CliResult<Value> {
    match output.body {
        DispatchBody::Json(value) => Ok(value),
        DispatchBody::Text(_) => Err(CliError::unexpected(
            "typed MCP adapter expected structured CLI output",
        )),
    }
}

fn push_string_flag(arguments: &Value, field: &str, flag: &str, args: &mut Vec<String>) {
    if let Some(value) = json_optional_string(arguments, field) {
        args.extend([flag.to_string(), value]);
    }
}

fn push_u32_flag(
    arguments: &Value,
    field: &str,
    flag: &str,
    args: &mut Vec<String>,
) -> CliResult<()> {
    if let Some(value) = json_u32(arguments, field)? {
        args.extend([flag.to_string(), value.to_string()]);
    }
    Ok(())
}

fn push_bool_flag(arguments: &Value, field: &str, flag: &str, args: &mut Vec<String>) {
    if json_bool(arguments, field).unwrap_or(false) {
        args.push(flag.to_string());
    }
}

fn attach_base64_images(rendered: &mut Value) -> CliResult<()> {
    for key in ["slides", "pages"] {
        let Some(items) = rendered.get_mut(key).and_then(Value::as_array_mut) else {
            continue;
        };
        for item in items {
            let path = item
                .get("imagePath")
                .and_then(Value::as_str)
                .ok_or_else(|| CliError::unexpected("render item has no imagePath"))?;
            let bytes = fs::read(path).map_err(|error| {
                CliError::unexpected(format!("failed to read rendered image {path:?}: {error}"))
            })?;
            item["imageBase64"] = json!(base64(&bytes));
        }
    }
    Ok(())
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[(first >> 2) as usize] as char);
        output.push(TABLE[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn build_tool(name: &str, family: BuildFamily) -> Value {
    let mut properties = Map::from_iter([
        (
            "spec".to_string(),
            crate::build::schema_by_name(family.schema_name())
                .expect("committed build schema is available"),
        ),
        (
            "markdown".to_string(),
            json!({
                "type": "string",
                "minLength": 1,
                "description": "Inline Markdown source for PPTX or DOCX. Relative asset paths resolve from the MCP server working directory. XLSX returns a teaching error because its tabular build spec is the unambiguous input."
            }),
        ),
        (
            "output".to_string(),
            json!({"type": "string", "minLength": 1}),
        ),
        (
            "session".to_string(),
            json!({"type": "string", "minLength": 1}),
        ),
        (
            "dryRun".to_string(),
            json!({"type": "boolean", "default": false}),
        ),
        (
            "check".to_string(),
            json!({"type": "boolean", "default": false}),
        ),
        (
            "force".to_string(),
            json!({"type": "boolean", "default": false}),
        ),
    ]);
    let input_schema = json!({
        "type": "object",
        "properties": Value::Object(std::mem::take(&mut properties)),
        "allOf": [
            {"oneOf": [
                {"required": ["spec"], "not": {"required": ["markdown"]}},
                {"required": ["markdown"], "not": {"required": ["spec"]}},
            ]},
            {"oneOf": [
                {"required": ["output"], "not": {"required": ["session"]}},
                {"required": ["session"], "not": {"required": ["output"]}},
            ]},
        ],
        "additionalProperties": false,
    });
    json!({
        "name": name,
        "description": format!(
            "Build one complete {family} package from the published {} JSON schema in one call. The compiler emits ordered named operations, uses the shared mutation writers, strictly validates before publish, and returns a full outline for follow-up edits.",
            family.schema_name()
        ),
        "inputSchema": input_schema,
    })
}

fn edit_package_schema() -> Value {
    let command_variants = crate::capabilities::capabilities(&[])
        .expect("capabilities document")
        .get("commands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|command| command["opCompatible"] == true)
        .filter_map(|command| {
            let path = command.get("path")?.as_str()?.strip_prefix("ooxml ")?;
            let args = command.get("opArgsSchema")?.clone();
            Some(json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128,
                        "pattern": "^[A-Za-z0-9_:-]+$",
                    },
                    "command": {"const": path},
                    "args": args,
                },
                "required": ["command", "args"],
                "additionalProperties": false,
            }))
        })
        .collect::<Vec<_>>();
    json!({
        "type": "object",
        "properties": {
            "file": {"type": "string", "minLength": 1},
            "output": {"type": "string", "minLength": 1},
            "inPlace": {"type": "boolean", "default": false},
            "backup": {"type": "string", "minLength": 1},
            "noValidate": {"type": "boolean", "default": false},
            "dryRun": {"type": "boolean", "default": false},
            "operations": {"type": "array", "minItems": 1, "items": {"oneOf": command_variants}},
        },
        "required": ["file", "operations"],
        "anyOf": [
            {"required": ["output"]},
            {"properties": {"inPlace": {"const": true}}, "required": ["inPlace"]},
            {"properties": {"dryRun": {"const": true}}, "required": ["dryRun"]},
        ],
        "additionalProperties": false,
    })
}

fn object_schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn reject_unknown_arguments(name: &str, arguments: &Value) -> CliResult<()> {
    let object = arguments.as_object().ok_or_else(|| {
        CliError::invalid_args(format!(
            "{name} arguments must be an object; valid fields: {}",
            valid_fields(name).join(", ")
        ))
    })?;
    if let Some(field) = object
        .keys()
        .find(|field| !valid_fields(name).contains(&field.as_str()))
    {
        let suggestion = nearest_field(field, valid_fields(name));
        let suffix = suggestion
            .map(|candidate| format!("; did you mean {candidate:?}?"))
            .unwrap_or_default();
        return Err(CliError::invalid_args(format!(
            "unknown argument {field:?} for {name}{suffix}; valid fields: {}",
            valid_fields(name).join(", ")
        )));
    }
    Ok(())
}

fn nearest_field<'a>(field: &str, valid: &'a [&str]) -> Option<&'a str> {
    valid
        .iter()
        .map(|candidate| {
            (
                crate::cli_args::damerau_levenshtein(field, candidate),
                *candidate,
            )
        })
        .filter(|(distance, candidate)| {
            *distance <= 2 || *distance * 3 <= field.len().max(candidate.len())
        })
        .min()
        .map(|(_, candidate)| candidate)
}

fn typed_tool_error(tool: &str, error: CliError) -> Value {
    let unknown = error
        .message
        .strip_prefix("unknown argument \"")
        .and_then(|rest| rest.split_once('"'))
        .map(|(field, _)| field);
    let did_you_mean = unknown
        .and_then(|field| nearest_field(field, valid_fields(tool)))
        .map(|field| vec![field])
        .unwrap_or_default();
    let mut detail = json!({
        "code": error.code,
        "exitCode": error.exit_code,
        "message": error.message,
        "hint": typed_hint(tool),
        "didYouMean": did_you_mean,
        "validFields": valid_fields(tool),
        "tool": tool,
        "docs": "resource://agent-guide",
    });
    if let Some(schema) = schema_resource(tool) {
        detail["schemaResource"] = json!(schema);
    }
    for prefix in [
        "build spec validation failed: ",
        "build spec compilation failed: ",
        "Markdown conversion failed: ",
    ] {
        if let Some(serialized) = detail["message"]
            .as_str()
            .and_then(|text| text.strip_prefix(prefix))
            && let Ok(diagnostics) = serde_json::from_str::<Value>(serialized)
        {
            detail["diagnostics"] = diagnostics;
        }
    }
    let text = serde_json::to_string(&json!({"error": detail})).expect("serialize typed error");
    json!({
        "content": [{"type": "text", "text": text}],
        "isError": true,
        "structuredContent": {"error": detail},
    })
}

fn next_actions(tool: &str) -> Vec<String> {
    match tool {
        "build_presentation" | "build_workbook" | "build_document" => vec![
            "review structuredContent.outline for stable selectors and handles".to_string(),
            "run check_package on the output before delivery".to_string(),
        ],
        "edit_package" | "replace_text" => vec![
            "inspect the returned readback and mutation envelopes".to_string(),
            "run check_package on the published output".to_string(),
        ],
        "find_text" => vec!["pass the exact query and replacement to replace_text".to_string()],
        "outline_package" => {
            vec!["use returned stable selectors in edit_package operations".to_string()]
        }
        "check_package" => {
            vec!["execute each reviewed finding.fixCommand, then rerun check_package".to_string()]
        }
        _ => Vec::new(),
    }
}

fn typed_hint(tool: &str) -> String {
    match schema_resource(tool) {
        Some(resource) => format!("read {resource}, then retry with only the documented fields"),
        None => format!(
            "call tools/list and inspect the {tool} inputSchema, then retry with only the documented fields"
        ),
    }
}

fn schema_resource(tool: &str) -> Option<&'static str> {
    match tool {
        "build_presentation" => Some("resource://schema/pptx-build"),
        "build_workbook" => Some("resource://schema/xlsx-build"),
        "build_document" => Some("resource://schema/docx-build"),
        _ => None,
    }
}

fn valid_fields(tool: &str) -> &'static [&'static str] {
    match tool {
        "build_presentation" | "build_workbook" | "build_document" => &[
            "spec", "markdown", "output", "session", "check", "dryRun", "force",
        ],
        "edit_package" => &[
            "file",
            "output",
            "inPlace",
            "backup",
            "noValidate",
            "dryRun",
            "operations",
        ],
        "outline_package" => &["file", "depth", "textPreview", "slide", "sheet", "section"],
        "check_package" => &[
            "file",
            "openXmlSdk",
            "failOn",
            "render",
            "fix",
            "dryRun",
            "output",
            "inPlace",
            "backup",
            "maxRounds",
        ],
        "validate_package" => &["file"],
        "render_preview" => &[
            "file",
            "out",
            "dpi",
            "pages",
            "slides",
            "sheet",
            "includeBase64",
        ],
        "find_text" => &["file", "query", "type", "ignoreCase", "regex", "max"],
        "replace_text" => &[
            "file",
            "query",
            "replacement",
            "output",
            "ignoreCase",
            "regex",
            "noValidate",
            "dryRun",
        ],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn base64_padding_matches_rfc_4648_examples() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
    }
}
