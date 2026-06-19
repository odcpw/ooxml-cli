use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, NamespaceResolver, ResolveResult};
use quick_xml::{NsReader, Reader};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const EXIT_SUCCESS: i32 = 0;
const EXIT_UNEXPECTED: i32 = 1;
const EXIT_INVALID_ARGS: i32 = 2;
const EXIT_FILE_NOT_FOUND: i32 = 3;
const EXIT_UNSUPPORTED_TYPE: i32 = 4;
const EXIT_TARGET_NOT_FOUND: i32 = 6;
const DOCX_W_NS: &[u8] = b"http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const DOCX_W14_NS: &[u8] = b"http://schemas.microsoft.com/office/word/2010/wordml";

#[derive(Debug)]
struct CliError {
    code: &'static str,
    exit_code: i32,
    message: String,
}

impl CliError {
    fn invalid_args(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_args",
            exit_code: EXIT_INVALID_ARGS,
            message: message.into(),
        }
    }

    fn file_not_found(message: impl Into<String>) -> Self {
        Self {
            code: "file_not_found",
            exit_code: EXIT_FILE_NOT_FOUND,
            message: message.into(),
        }
    }

    fn unexpected(message: impl Into<String>) -> Self {
        Self {
            code: "unexpected",
            exit_code: EXIT_UNEXPECTED,
            message: message.into(),
        }
    }

    fn unsupported_type(message: impl Into<String>) -> Self {
        Self {
            code: "unsupported_type",
            exit_code: EXIT_UNSUPPORTED_TYPE,
            message: message.into(),
        }
    }

    fn target_not_found(message: impl Into<String>) -> Self {
        Self {
            code: "target_not_found",
            exit_code: EXIT_TARGET_NOT_FOUND,
            message: message.into(),
        }
    }
}

type CliResult<T> = Result<T, CliError>;

#[derive(Default)]
struct GlobalFlags {
    json: bool,
    strict: bool,
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("serve") {
        std::process::exit(run_serve_stdio());
    }
    if argv.first().map(String::as_str) == Some("mcp") {
        std::process::exit(run_mcp_stdio());
    }
    match run(&argv) {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string(&value).expect("serialize output")
            );
            std::process::exit(EXIT_SUCCESS);
        }
        Err(err) => {
            let body = json!({
                "error": {
                    "code": err.code,
                    "exitCode": err.exit_code,
                    "message": err.message,
                }
            });
            eprintln!("{}", serde_json::to_string(&body).expect("serialize error"));
            std::process::exit(err.exit_code);
        }
    }
}

fn run_serve_stdio() -> i32 {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut state = ServeState::default();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "serve read error: {err}");
                return EXIT_UNEXPECTED;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "serve JSON parse error: {err}");
                return EXIT_INVALID_ARGS;
            }
        };
        let response = state.handle_rpc(request);
        if writeln!(
            stdout,
            "{}",
            serde_json::to_string(&response).expect("serialize rpc response")
        )
        .is_err()
        {
            return EXIT_UNEXPECTED;
        }
        if stdout.flush().is_err() {
            return EXIT_UNEXPECTED;
        }
    }
    EXIT_SUCCESS
}

fn run_mcp_stdio() -> i32 {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut state = McpState::default();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "mcp read error: {err}");
                return EXIT_UNEXPECTED;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                let _ = writeln!(std::io::stderr(), "mcp JSON parse error: {err}");
                return EXIT_INVALID_ARGS;
            }
        };
        let response = state.handle_rpc(request);
        if writeln!(
            stdout,
            "{}",
            serde_json::to_string(&response).expect("serialize rpc response")
        )
        .is_err()
        {
            return EXIT_UNEXPECTED;
        }
        if stdout.flush().is_err() {
            return EXIT_UNEXPECTED;
        }
    }
    EXIT_SUCCESS
}

#[derive(Default)]
struct McpState {
    engine: ServeState,
}

impl McpState {
    fn handle_rpc(&mut self, request: Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        match self.handle_method(method, &params) {
            Ok(result) => json!({"id": id, "jsonrpc": "2.0", "result": result}),
            Err(err) => json!({
                "id": id,
                "jsonrpc": "2.0",
                "error": {
                    "code": err.exit_code,
                    "message": err.message,
                    "data": {"type": err.code, "exitCode": err.exit_code},
                },
            }),
        }
    }

    fn handle_method(&mut self, method: &str, params: &Value) -> CliResult<Value> {
        match method {
            "initialize" => Ok(json!({
                "capabilities": {"resources": {}, "tools": {}},
                "protocolVersion": "2025-06-18",
                "serverInfo": {"name": "ooxml", "version": "0.0.1"},
            })),
            "tools/list" => Ok(json!({"tools": mcp_tools()})),
            "tools/call" => self.handle_tools_call(params),
            "resources/list" => Ok(json!({"resources": mcp_resources()})),
            "resources/templates/list" => {
                Ok(json!({"resourceTemplates": [mcp_command_resource_template()]}))
            }
            "resources/read" => self.handle_resource_read(params),
            _ => Err(CliError::invalid_args(format!(
                "unsupported MCP method: {method}"
            ))),
        }
    }

    fn handle_tools_call(&mut self, params: &Value) -> CliResult<Value> {
        let name = json_string(params, "name")?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        match name.as_str() {
            "open" => self.call_open(&arguments),
            "op" => self.call_op(&arguments),
            "inspect" => self.call_engine("inspect", &arguments, Vec::new()),
            "validate" => self.call_engine("validate", &arguments, Vec::new()),
            "plan" => self.call_engine("plan", &arguments, Vec::new()),
            "commit" => self.call_commit(&arguments),
            "abort" => self.call_engine("abort", &arguments, Vec::new()),
            _ => Err(CliError::invalid_args(format!("unknown tool: {name}"))),
        }
    }

    fn call_open(&mut self, arguments: &Value) -> CliResult<Value> {
        let payload = self.engine.handle_method("open", arguments)?;
        let session = payload
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| CliError::unexpected("open returned no sessionId"))?;
        let next_actions = vec![
            format!(
                "call op/inspect/validate with session=\"{session}\" (thread this sessionId through every subsequent call)"
            ),
            "call commit to write the output, or abort to discard the working copy".to_string(),
        ];
        Ok(mcp_tool_success("open", payload, next_actions))
    }

    fn call_op(&mut self, arguments: &Value) -> CliResult<Value> {
        let session = json_string(arguments, "session")?;
        let payload = self.engine.handle_method("op", arguments)?;
        let next_actions = vec![
            format!(
                "call inspect with session=\"{session}\" to confirm the change against the working copy"
            ),
            format!("call validate with session=\"{session}\" before committing"),
            format!("call commit with session=\"{session}\" to write the output"),
        ];
        Ok(mcp_tool_success("op", payload, next_actions))
    }

    fn call_commit(&mut self, arguments: &Value) -> CliResult<Value> {
        let payload = self.engine.handle_method("commit", arguments)?;
        let next_actions = payload
            .get("validateCommand")
            .and_then(Value::as_str)
            .map(|command| vec![format!("verify the output: {command}")])
            .unwrap_or_default();
        Ok(mcp_tool_success("commit", payload, next_actions))
    }

    fn call_engine(
        &mut self,
        method: &str,
        arguments: &Value,
        next_actions: Vec<String>,
    ) -> CliResult<Value> {
        let payload = self.engine.handle_method(method, arguments)?;
        Ok(mcp_tool_success(method, payload, next_actions))
    }

    fn handle_resource_read(&self, params: &Value) -> CliResult<Value> {
        let uri = json_string(params, "uri")?;
        let text = match uri.as_str() {
            "resource://capabilities" => serde_json::to_string(&mcp_capabilities_resource())
                .expect("serialize capabilities resource"),
            "resource://agent-guide" => serde_json::to_string(&json!({
                "tool": "ooxml",
                "guide": "Open a session with tools/call open, apply one op at a time, inspect and validate before commit.",
            }))
            .expect("serialize agent guide"),
            _ if uri.starts_with("resource://command/") => serde_json::to_string(
                &mcp_command_resource_for_uri(&uri)?,
            )
            .expect("serialize command resource"),
            _ => {
                return Err(CliError::file_not_found(format!(
                    "unknown MCP resource: {uri}"
                )));
            }
        };
        Ok(json!({
            "contents": [{
                "mimeType": "application/json",
                "text": text,
                "uri": uri,
            }]
        }))
    }
}

fn run(raw_args: &[String]) -> CliResult<Value> {
    let (flags, args) = parse_global_flags(raw_args)?;
    if !flags.json && !has_local_json_format(&args) {
        return Err(CliError::invalid_args(
            "the Rust port currently supports the frozen --json contract slice only",
        ));
    }
    dispatch(&flags, &args)
}

fn parse_global_flags(raw_args: &[String]) -> CliResult<(GlobalFlags, Vec<String>)> {
    let mut flags = GlobalFlags::default();
    let mut args = Vec::new();
    let mut i = 0;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--json" => {
                flags.json = true;
                i += 1;
            }
            "--format" | "-f" => {
                let Some(value) = raw_args.get(i + 1) else {
                    return Err(CliError::invalid_args("--format requires a value"));
                };
                if value != "json" {
                    return Err(CliError::invalid_args(format!(
                        "invalid format: {value} (expected 'text' or 'json')"
                    )));
                }
                flags.json = true;
                i += 2;
            }
            "--strict" => {
                flags.strict = true;
                i += 1;
            }
            _ => {
                args.extend_from_slice(&raw_args[i..]);
                break;
            }
        }
    }
    Ok((flags, args))
}

fn dispatch(flags: &GlobalFlags, args: &[String]) -> CliResult<Value> {
    match args {
        [cmd] if cmd == "version" => Ok(json!({"tool": "ooxml", "version": "0.0.1"})),
        [cmd, rest @ ..] if cmd == "capabilities" => capabilities(rest),
        [cmd, file] if cmd == "inspect" => inspect(file),
        [cmd, file] if cmd == "validate" => validate(file, flags.strict),
        [cmd, file, rest @ ..] if cmd == "verify" => verify(file, rest),
        [cmd, family, file] if cmd == "docx" && family == "text" => docx_text(file),
        [cmd, group, file, rest @ ..] if cmd == "docx" && group == "blocks" => {
            reject_unknown_flags(rest, &["--block"], &["--include-runs"])?;
            let block = parse_i64_flag(rest, "--block")?.unwrap_or(0);
            if block < 0 {
                return Err(CliError::invalid_args("--block must be >= 0"));
            }
            let include_runs = has_flag(rest, "--include-runs");
            docx_blocks_show(file, block as usize, include_runs)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "styles" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--type"], &[])?;
            let style_type = parse_string_flag(rest, "--type")?;
            docx_styles_list(file, style_type.as_deref())
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "styles" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--style"], &[])?;
            let style_id = parse_string_flag(rest, "--style")?
                .ok_or_else(|| CliError::invalid_args("--style is required"))?;
            docx_styles_show(file, &style_id)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "comments" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--comment-id"], &[])?;
            let comment_id = parse_i64_flag(rest, "--comment-id")?;
            docx_comments_list(file, comment_id)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "fields" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--type"], &[])?;
            let field_type = parse_string_flag(rest, "--type")?;
            docx_fields_list(file, field_type.as_deref())
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && (group == "headers" || group == "footers") && verb == "list" =>
        {
            reject_unknown_flags(rest, &[], &[])?;
            docx_headers_footers_list(file)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && (group == "headers" || group == "footers") && verb == "show" =>
        {
            docx_headers_footers_show(file, docx_header_footer_kind(group), rest)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "images" && verb == "list" =>
        {
            reject_unknown_flags(rest, &[], &[])?;
            docx_images_list(file)
        }
        [cmd, group, verb, file, rest @ ..]
            if cmd == "docx" && group == "tables" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--table"], &["--details"])?;
            let table = parse_i64_flag(rest, "--table")?.unwrap_or(0);
            if table < 0 {
                return Err(CliError::invalid_args("--table must be positive"));
            }
            let include_details = has_flag(rest, "--details");
            docx_tables_show(file, table as usize, include_details)
        }
        [family, verb, file, rest @ ..] if family == "pptx" && verb == "render" => {
            pptx_render(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "show" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?.unwrap_or(1);
            pptx_slide_show(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "selectors" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("--slide is required"))?;
            pptx_slide_selectors(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "shapes" && verb == "show" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("required flag(s) \"slide\" not set"))?;
            let include_text = has_flag(rest, "--include-text");
            let include_bounds = has_flag(rest, "--include-bounds");
            pptx_shapes_show(file, slide, include_text, include_bounds)
        }
        [family, group, verb, file] if family == "pptx" && group == "slides" && verb == "list" => {
            pptx_slides_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "export" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--data-format",
                    "--data-out",
                    "--max-cells",
                ],
                &["--include-types", "--include-formulas", "--include-formats"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?.unwrap_or_else(|| "1".to_string());
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            require_json_data_format(data_format.as_deref())?;
            let data_out = parse_string_flag(rest, "--data-out")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let include_types = has_flag(rest, "--include-types");
            let include_formulas = has_flag(rest, "--include-formulas");
            let include_formats = has_flag(rest, "--include-formats");
            xlsx_range_export_with_options(
                file,
                &sheet,
                &range,
                XlsxRangeExportOptions {
                    include_types,
                    include_formulas,
                    include_formats,
                    data_out: data_out.as_deref(),
                    max_cells,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--anchor",
                    "--values",
                    "--values-file",
                    "--data-format",
                    "--null-policy",
                    "--ragged",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &[
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                    "--overwrite-formulas",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?
                .ok_or_else(|| CliError::invalid_args("--sheet is required for range commands"))?;
            let range = parse_string_flag(rest, "--range")?;
            let anchor = parse_string_flag(rest, "--anchor")?;
            let values = parse_string_flag(rest, "--values")?;
            let values_file = parse_string_flag(rest, "--values-file")?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            let null_policy = parse_string_flag(rest, "--null-policy")?;
            let ragged = parse_string_flag(rest, "--ragged")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let no_validate = has_flag(rest, "--no-validate");
            let in_place = has_flag(rest, "--in-place");
            let overwrite_formulas = has_flag(rest, "--overwrite-formulas");
            xlsx_ranges_set(
                file,
                XlsxRangesSetOptions {
                    sheet: &sheet,
                    range: range.as_deref(),
                    anchor: anchor.as_deref(),
                    values: values.as_deref(),
                    values_file: values_file.as_deref(),
                    data_format: data_format.as_deref(),
                    null_policy: null_policy.as_deref(),
                    ragged: ragged.as_deref(),
                    max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    no_validate,
                    in_place,
                    overwrite_formulas,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "set-format" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--preset",
                    "--format-code",
                    "--decimals",
                    "--currency-symbol",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?
                .ok_or_else(|| CliError::invalid_args("--sheet is required for range commands"))?;
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            let preset = parse_string_flag(rest, "--preset")?;
            let format_code = parse_string_flag(rest, "--format-code")?;
            let decimals = parse_i64_flag(rest, "--decimals")?.unwrap_or(2);
            let currency_symbol = parse_string_flag(rest, "--currency-symbol")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let no_validate = has_flag(rest, "--no-validate");
            let in_place = has_flag(rest, "--in-place");
            xlsx_ranges_set_format(
                file,
                XlsxRangesSetFormatOptions {
                    sheet: &sheet,
                    range: &range,
                    preset: preset.as_deref(),
                    format_code: format_code.as_deref(),
                    decimals,
                    currency_symbol: currency_symbol.as_deref(),
                    max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    no_validate,
                    in_place,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "cells" && verb == "extract" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?.unwrap_or_else(|| "1".to_string());
            let range = parse_string_flag(rest, "--range")?;
            let max_rows = parse_u32_flag(rest, "--max-rows")?.unwrap_or(1000);
            let max_cells = parse_u32_flag(rest, "--max-cells")?.unwrap_or(0);
            let include_empty = has_flag(rest, "--include-empty");
            xlsx_cells_extract(
                file,
                &sheet,
                range.as_deref(),
                max_rows,
                max_cells,
                include_empty,
            )
        }
        [family, group, verb, file] if family == "xlsx" && group == "sheets" && verb == "list" => {
            xlsx_sheets_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "sheets" && verb == "show" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_sheets_show(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "tables" && verb == "list" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_tables_list(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "tables" && verb == "show" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?;
            let table = parse_string_flag(rest, "--table")?;
            xlsx_tables_show(file, sheet.as_deref(), table.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "tables" && verb == "export" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--table",
                    "--data-format",
                    "--data-out",
                    "--max-cells",
                ],
                &["--include-types", "--include-formulas"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let table = parse_string_flag(rest, "--table")?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            let data_out = parse_string_flag(rest, "--data-out")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let include_types = has_flag(rest, "--include-types");
            let include_formulas = has_flag(rest, "--include-formulas");
            xlsx_tables_export(
                file,
                sheet.as_deref(),
                table.as_deref(),
                XlsxTableExportOptions {
                    data_format: data_format.as_deref(),
                    data_out: data_out.as_deref(),
                    max_cells,
                    include_types,
                    include_formulas,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text" =>
        {
            pptx_replace_text(file, rest)
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}

fn has_local_json_format(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "--format" && pair[1] == "json")
}

fn parse_string_flag(args: &[String], name: &str) -> CliResult<Option<String>> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            let Some(value) = args.get(i + 1) else {
                return Err(CliError::invalid_args(format!("{name} requires a value")));
            };
            return Ok(Some(value.clone()));
        }
        i += 1;
    }
    Ok(None)
}

fn reject_unknown_flags(
    args: &[String],
    value_flags: &[&str],
    bool_flags: &[&str],
) -> CliResult<()> {
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !arg.starts_with("--") {
            i += 1;
            continue;
        }
        if bool_flags.iter().any(|flag| flag == arg) {
            i += 1;
            continue;
        }
        if value_flags.iter().any(|flag| flag == arg) {
            if args.get(i + 1).is_none() {
                return Err(CliError::invalid_args(format!("{arg} requires a value")));
            }
            i += 2;
            continue;
        }
        return Err(CliError::invalid_args(format!("unknown flag: {arg}")));
    }
    Ok(())
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn capabilities(args: &[String]) -> CliResult<Value> {
    let filter = parse_string_flag(args, "--for")?.map(|value| value.to_ascii_lowercase());
    let mut commands = capability_commands();
    if let Some(filter) = filter.as_deref() {
        commands.retain(|command| capability_matches_filter(command, filter));
    }
    let mut notes = vec![
        "Rust port partial surface: only commands listed here are implemented in the Rust subject."
            .to_string(),
        "Use Go on codex/ooxml-go-reference as the oracle for the full command universe."
            .to_string(),
    ];
    if let Some(filter) = filter.as_deref() {
        notes.insert(
            0,
            format!("Filtered by Rust-supported command/object filter \"{filter}\"."),
        );
    }
    Ok(json!({
        "tool": "ooxml",
        "version": "0.0.1",
        "contractVersion": "ooxml-cli.agent-capabilities.v4",
        "packageTypes": ["pptx", "xlsx", "docx"],
        "outputModes": ["json via --json or --format json"],
        "globalFlags": [
            {"name": "--format", "argName": "format", "shorthand": "f", "type": "string", "default": "text", "description": "output format: \"text\" or \"json\""},
            {"name": "--json", "argName": "json", "type": "bool", "default": "false", "description": "emit JSON output"},
            {"name": "--strict", "argName": "strict", "type": "bool", "default": "false", "description": "enable strict validation mode"}
        ],
        "commands": commands,
        "objectKinds": ["package", "slide", "shape", "sheet", "range", "cell", "table", "style", "comment"],
        "objectKindsIndex": {
            "package": ["ooxml inspect", "ooxml validate", "ooxml verify"],
            "slide": ["ooxml pptx slides list", "ooxml pptx slides selectors", "ooxml pptx slides show", "ooxml pptx shapes show", "ooxml pptx replace text", "ooxml pptx render"],
            "shape": ["ooxml pptx slides list", "ooxml pptx slides selectors", "ooxml pptx slides show", "ooxml pptx shapes show", "ooxml pptx replace text"],
            "sheet": ["ooxml xlsx sheets list", "ooxml xlsx sheets show", "ooxml xlsx ranges export", "ooxml xlsx ranges set", "ooxml xlsx ranges set-format", "ooxml xlsx cells extract", "ooxml xlsx cells set", "ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export"],
            "range": ["ooxml xlsx ranges export", "ooxml xlsx ranges set", "ooxml xlsx ranges set-format", "ooxml xlsx cells extract", "ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export"],
            "cell": ["ooxml xlsx ranges set", "ooxml xlsx cells set"],
            "table": ["ooxml xlsx tables list", "ooxml xlsx tables show", "ooxml xlsx tables export"],
            "style": ["ooxml xlsx ranges set-format", "ooxml docx styles list", "ooxml docx styles show"],
            "comment": ["ooxml docx comments list"]
        },
        "exitCodes": [
            {"code": EXIT_SUCCESS, "name": "success", "description": "command completed successfully"},
            {"code": EXIT_UNEXPECTED, "name": "unexpected", "description": "unexpected tool or package processing error"},
            {"code": EXIT_INVALID_ARGS, "name": "invalid_args", "description": "invalid command line arguments or incompatible options"},
            {"code": EXIT_FILE_NOT_FOUND, "name": "file_not_found", "description": "input file was not found"},
            {"code": EXIT_UNSUPPORTED_TYPE, "name": "unsupported_type", "description": "input package type is unsupported for the requested command"},
            {"code": EXIT_TARGET_NOT_FOUND, "name": "target_not_found", "description": "requested slide, sheet, table, shape, or macro part was not found"}
        ],
        "workflows": [
            {
                "name": "pptx inspect then edit",
                "commands": [
                    "ooxml --json inspect deck.pptx",
                    "ooxml --json pptx slides list deck.pptx",
                    "ooxml --json pptx slides selectors deck.pptx --slide 1",
                    "ooxml --json pptx slides show deck.pptx --slide 1 --include-text",
                    "ooxml --json pptx shapes show deck.pptx --slide 1 --include-text --include-bounds",
                    "ooxml --json pptx replace text deck.pptx --slide 1 --target title --text NEW --out edited.pptx",
                    "ooxml validate --strict edited.pptx"
                ]
            },
            {
                "name": "xlsx inspect then edit",
                "commands": [
                    "ooxml --json xlsx sheets list workbook.xlsx",
                    "ooxml --json xlsx ranges export workbook.xlsx --sheet sheetId:1 --range A1 --include-types",
                    "ooxml --json xlsx ranges set workbook.xlsx --sheet sheetId:1 --range A1:B2 --values '[[\"A\",\"B\"],[1,2]]' --out edited.xlsx",
                    "ooxml --json xlsx ranges set-format workbook.xlsx --sheet Sheet1 --range B2:B20 --preset currency --out edited.xlsx",
                    "serve op command: xlsx cells set"
                ]
            }
        ],
        "conventions": [
            "stdout is data; diagnostics and errors go to stderr",
            "serve/MCP operation commands use op vocabulary without the leading ooxml",
            "mutations should be validated before handing files to users"
        ],
        "notes": notes,
    }))
}

fn capability_matches_filter(command: &Value, filter: &str) -> bool {
    let path = command
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if path.contains(&format!(" {filter} ")) || path.ends_with(&format!(" {filter}")) {
        return true;
    }
    command
        .get("targetObjectKinds")
        .and_then(Value::as_array)
        .map(|kinds| kinds.iter().any(|kind| kind.as_str() == Some(filter)))
        .unwrap_or(false)
}

fn capability_commands() -> Vec<Value> {
    vec![
        capability_command(
            "ooxml version",
            "version",
            "Print the version of ooxml.",
            &[],
            false,
            Some("read-only metadata command"),
            vec![],
        ),
        capability_command(
            "ooxml inspect",
            "inspect <file>",
            "Inspect a supported OOXML package.",
            &["package"],
            false,
            Some("read-only command; use inspect_current_with_ooxml through serve"),
            vec![],
        ),
        capability_command(
            "ooxml validate",
            "validate <file>",
            "Validate an OOXML package.",
            &["package"],
            false,
            Some("read-only validation command"),
            vec![],
        ),
        capability_command(
            "ooxml verify",
            "verify <file>",
            "Validate and compare a package against a baseline where supported.",
            &["package"],
            false,
            Some("read-only verification command"),
            vec![flag(
                "--baseline",
                "baseline",
                "string",
                "baseline file to compare against",
            )],
        ),
        capability_command(
            "ooxml pptx slides list",
            "list <file>",
            "List slides and stable slide selectors.",
            &["slide", "shape"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![],
        ),
        capability_command(
            "ooxml pptx slides selectors",
            "selectors <file> --slide <n>",
            "List targetable selectors for a slide.",
            &["slide", "shape"],
            false,
            Some("read-only command; generated by slides list/show"),
            vec![flag("--slide", "slide", "int", "1-based slide number")],
        ),
        capability_command(
            "ooxml pptx slides show",
            "show <file>",
            "Show slide text and stable selectors.",
            &["slide", "shape"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![
                flag("--slide", "slide", "int", "1-based slide number"),
                flag(
                    "--include-text",
                    "includeText",
                    "bool",
                    "include visible text",
                ),
                flag(
                    "--include-bounds",
                    "includeBounds",
                    "bool",
                    "include shape bounds when available",
                ),
            ],
        ),
        capability_command(
            "ooxml pptx shapes show",
            "show <file> --slide <n>",
            "Show targetable shapes on a slide.",
            &["slide", "shape"],
            false,
            Some("read-only command; generated by slides list/show"),
            vec![
                flag("--slide", "slide", "int", "1-based slide number"),
                flag(
                    "--include-text",
                    "includeText",
                    "bool",
                    "include text preview/content where available",
                ),
                flag(
                    "--include-bounds",
                    "includeBounds",
                    "bool",
                    "include explicit shape bounds where available",
                ),
            ],
        ),
        capability_command(
            "ooxml pptx replace text",
            "text <file>",
            "Replace text in the supported slide target.",
            &["slide", "shape"],
            true,
            None,
            vec![
                flag("--slide", "slide", "int", "1-based slide number"),
                flag(
                    "--target",
                    "target",
                    "string",
                    "shape selector, title for the frozen slice",
                ),
                flag("--text", "text", "string", "replacement text"),
                flag(
                    "--out",
                    "out",
                    "string",
                    "output file path for direct CLI use",
                ),
            ],
        ),
        capability_command(
            "ooxml pptx render",
            "render <file>",
            "Render a PPTX to PDF/thumbnails when local tools are installed.",
            &["slide"],
            false,
            Some("render command is not a mutation op"),
            vec![
                flag("--out", "out", "string", "render output directory"),
                flag("--slides", "slides", "string", "comma-separated slide list"),
                flag("--format", "format", "string", "json"),
            ],
        ),
        capability_command(
            "ooxml xlsx sheets list",
            "list <file>",
            "List workbook sheets and selectors.",
            &["sheet"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![],
        ),
        capability_command(
            "ooxml xlsx sheets show",
            "show <file>",
            "Show worksheet metadata, used ranges, and generated readback commands.",
            &["sheet", "range"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![flag("--sheet", "sheet", "string", "sheet selector")],
        ),
        capability_command(
            "ooxml xlsx tables list",
            "list <file>",
            "List workbook tables",
            &["table", "range", "sheet"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![flag(
                "--sheet",
                "sheet",
                "string",
                "sheet number (1-based) or exact sheet name",
            )],
        ),
        capability_command(
            "ooxml xlsx tables show",
            "show <file>",
            "Show table metadata",
            &["table", "range", "sheet"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![
                flag(
                    "--sheet",
                    "sheet",
                    "string",
                    "sheet number (1-based) or exact sheet name",
                ),
                flag(
                    "--table",
                    "table",
                    "string",
                    "table number, name, or displayName",
                ),
            ],
        ),
        capability_command(
            "ooxml xlsx tables export",
            "export <file>",
            "Export a table as a rectangular JSON matrix.",
            &["table", "range", "sheet"],
            false,
            Some("read-only command; generated by xlsx tables list/show"),
            vec![
                flag(
                    "--sheet",
                    "sheet",
                    "string",
                    "sheet number (1-based) or exact sheet name",
                ),
                flag(
                    "--table",
                    "table",
                    "string",
                    "table number, name, or displayName",
                ),
                flag(
                    "--include-types",
                    "includeTypes",
                    "bool",
                    "include cell types",
                ),
                flag(
                    "--include-formulas",
                    "includeFormulas",
                    "bool",
                    "include formulas",
                ),
                flag(
                    "--data-out",
                    "dataOut",
                    "string",
                    "write JSON matrix data to this file",
                ),
                flag(
                    "--max-cells",
                    "maxCells",
                    "number",
                    "maximum cells to export",
                ),
            ],
        ),
        capability_command(
            "ooxml xlsx ranges export",
            "export <file>",
            "Export decoded worksheet cells from a range.",
            &["sheet", "range"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "A1 range"),
                flag(
                    "--include-types",
                    "includeTypes",
                    "bool",
                    "include cell types",
                ),
                flag(
                    "--include-formulas",
                    "includeFormulas",
                    "bool",
                    "include formulas",
                ),
                flag(
                    "--include-formats",
                    "includeFormats",
                    "bool",
                    "include style and number-format matrices",
                ),
                flag(
                    "--data-out",
                    "dataOut",
                    "string",
                    "write JSON matrix data to this file",
                ),
                flag(
                    "--max-cells",
                    "maxCells",
                    "number",
                    "maximum cells to export",
                ),
            ],
        ),
        capability_command(
            "ooxml xlsx ranges set",
            "set <file>",
            "Set a worksheet range from a rectangular JSON, CSV, or TSV matrix.",
            &["sheet", "range", "cell"],
            false,
            Some("direct CLI mutation is implemented; serve op routing is not wired yet"),
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "A1 range"),
                flag("--anchor", "anchor", "string", "top-left A1 cell"),
                flag("--values", "values", "string", "inline matrix data"),
                flag(
                    "--values-file",
                    "valuesFile",
                    "string",
                    "path to matrix data, or - for stdin",
                ),
                flag(
                    "--data-format",
                    "dataFormat",
                    "string",
                    "matrix data format: json, csv, or tsv",
                ),
                flag(
                    "--null-policy",
                    "nullPolicy",
                    "string",
                    "null handling: skip, clear, or empty-string",
                ),
                flag(
                    "--ragged",
                    "ragged",
                    "string",
                    "ragged handling: reject or fill-empty",
                ),
                flag("--max-cells", "maxCells", "number", "maximum cells to set"),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--overwrite-formulas",
                    "overwriteFormulas",
                    "bool",
                    "allow replacing existing formula cells",
                ),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "accepted for CLI compatibility",
                ),
            ],
        ),
        capability_command(
            "ooxml xlsx ranges set-format",
            "set-format <file>",
            "Apply a practical number format to a worksheet range.",
            &["sheet", "range", "style"],
            false,
            Some("direct CLI mutation is implemented; serve op routing is not wired yet"),
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "A1 range"),
                flag(
                    "--preset",
                    "preset",
                    "string",
                    "format preset: integer, number, currency, percent, date, datetime, text, or general",
                ),
                flag(
                    "--format-code",
                    "formatCode",
                    "string",
                    "custom SpreadsheetML number format code",
                ),
                flag(
                    "--decimals",
                    "decimals",
                    "number",
                    "decimal places for number, currency, and percent presets",
                ),
                flag(
                    "--currency-symbol",
                    "currencySymbol",
                    "string",
                    "currency literal for the currency preset",
                ),
                flag(
                    "--max-cells",
                    "maxCells",
                    "number",
                    "maximum cells to format",
                ),
                flag("--out", "out", "string", "output file path"),
                flag(
                    "--in-place",
                    "inPlace",
                    "bool",
                    "write the input file in place",
                ),
                flag("--backup", "backup", "string", "backup path for --in-place"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "accepted for CLI compatibility",
                ),
            ],
        ),
        capability_command(
            "ooxml xlsx cells extract",
            "extract <file>",
            "Extract decoded worksheet cells with stable cell selectors.",
            &["sheet", "range", "cell"],
            false,
            Some("read-only command; call via inspect in serve/MCP"),
            vec![
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--range", "range", "string", "A1 range"),
                flag("--max-rows", "maxRows", "number", "maximum rows to emit"),
                flag("--max-cells", "maxCells", "number", "maximum cells to emit"),
                flag(
                    "--include-empty",
                    "includeEmpty",
                    "bool",
                    "include empty cells inside output bounds",
                ),
            ],
        ),
        capability_command(
            "ooxml xlsx cells set",
            "set <file>",
            "Set a worksheet cell value.",
            &["sheet", "cell"],
            true,
            None,
            vec![
                flag("--backup", "backup", "string", "backup path"),
                flag("--cell", "cell", "string", "A1 cell reference"),
                flag("--dry-run", "dryRun", "bool", "plan without writing"),
                flag("--formula", "formula", "string", "cell formula"),
                flag("--in-place", "inPlace", "bool", "write in place"),
                flag(
                    "--no-validate",
                    "noValidate",
                    "bool",
                    "skip post-write validation",
                ),
                flag(
                    "--out",
                    "out",
                    "string",
                    "output file path for direct CLI use",
                ),
                flag("--ref", "ref", "string", "A1 cell reference alias"),
                flag("--sheet", "sheet", "string", "sheet selector"),
                flag("--type", "type", "string", "value type"),
                flag("--value", "value", "string", "cell value"),
            ],
        ),
        capability_command(
            "ooxml docx text",
            "text <file>",
            "Extract DOCX paragraph text.",
            &["package"],
            false,
            Some("read-only command"),
            vec![],
        ),
        capability_command(
            "ooxml docx blocks",
            "blocks <file>",
            "Show stable DOCX body blocks with hashes, selectors, paragraph metadata, table cells, and optional runs.",
            &[],
            false,
            Some("read-only command; block hashes and selectors feed hash-guarded DOCX mutations"),
            vec![
                flag(
                    "--block",
                    "block",
                    "int",
                    "1-based body block index to show",
                ),
                flag(
                    "--include-runs",
                    "includeRuns",
                    "bool",
                    "include paragraph run text and basic run properties",
                ),
            ],
        ),
        capability_command(
            "ooxml docx styles list",
            "list <file>",
            "List DOCX paragraph, character, table, and numbering styles.",
            &["style"],
            false,
            Some("read-only command; generated style handles can be used by mutation commands"),
            vec![flag(
                "--type",
                "type",
                "string",
                "filter by style type: paragraph, character, table, or numbering",
            )],
        ),
        capability_command(
            "ooxml docx styles show",
            "show <file>",
            "Show detailed info for one DOCX style by styleId.",
            &["style"],
            false,
            Some("read-only command; generated style handles can be used by mutation commands"),
            vec![flag("--style", "style", "string", "styleId to show")],
        ),
        capability_command(
            "ooxml docx comments list",
            "list <file>",
            "List DOCX comments with stable selectors, hashes, and anchor blocks.",
            &["comment"],
            false,
            Some("read-only command; generated comment handles can be used by mutation commands"),
            vec![flag(
                "--comment-id",
                "commentId",
                "int",
                "show only the comment with this numeric w:id",
            )],
        ),
        capability_command(
            "ooxml docx fields list",
            "list <file>",
            "List all simple/complex fields in document body + headers/footers.",
            &["field"],
            false,
            Some(
                "read-only command; cached field results are stale until Word recalculates fields",
            ),
            vec![flag(
                "--type",
                "type",
                "string",
                "show only fields whose leading instruction keyword matches",
            )],
        ),
        capability_command(
            "ooxml docx headers list",
            "list <file>",
            "List headers and footers defined per section.",
            &["header", "footer"],
            false,
            Some(
                "read-only command; generated header/footer selectors can be pasted into show or set-text",
            ),
            vec![],
        ),
        capability_command(
            "ooxml docx headers show",
            "show <file>",
            "Show header content by type, section, or relationship id.",
            &["header", "paragraph"],
            false,
            Some("read-only command; accepts selectors from docx headers list"),
            vec![
                flag(
                    "--id",
                    "id",
                    "string",
                    "relationship id to resolve directly",
                ),
                flag(
                    "--section",
                    "section",
                    "int",
                    "1-based section index; 0 means the last section",
                ),
                flag(
                    "--selector",
                    "selector",
                    "string",
                    "selector from headers/footers list",
                ),
                flag("--type", "type", "string", "default, first, or even"),
            ],
        ),
        capability_command(
            "ooxml docx footers list",
            "list <file>",
            "List headers and footers defined per section.",
            &["footer", "header"],
            false,
            Some(
                "read-only command; generated header/footer selectors can be pasted into show or set-text",
            ),
            vec![],
        ),
        capability_command(
            "ooxml docx footers show",
            "show <file>",
            "Show footer content by type, section, or relationship id.",
            &["footer", "paragraph"],
            false,
            Some("read-only command; accepts selectors from docx footers list"),
            vec![
                flag(
                    "--id",
                    "id",
                    "string",
                    "relationship id to resolve directly",
                ),
                flag(
                    "--section",
                    "section",
                    "int",
                    "1-based section index; 0 means the last section",
                ),
                flag(
                    "--selector",
                    "selector",
                    "string",
                    "selector from headers/footers list",
                ),
                flag("--type", "type", "string", "default, first, or even"),
            ],
        ),
        capability_command(
            "ooxml docx images list",
            "list <file>",
            "List inline images in a DOCX document.",
            &["image", "paragraph"],
            false,
            Some(
                "read-only command; image records include relationship ids, media parts, dimensions, and block anchors",
            ),
            vec![],
        ),
        capability_command(
            "ooxml docx tables show",
            "show <file>",
            "Show DOCX tables by table index, body block index, dimensions, merged-cell flag, and cell text.",
            &[],
            false,
            Some(
                "read-only command; generated table hashes feed hash-guarded DOCX table mutations",
            ),
            vec![
                flag(
                    "--details",
                    "details",
                    "bool",
                    "include detailed table object in JSON output",
                ),
                flag(
                    "--table",
                    "table",
                    "int",
                    "1-based table number; omitted shows all tables",
                ),
            ],
        ),
    ]
}

fn capability_command(
    path: &str,
    use_text: &str,
    short: &str,
    target_kinds: &[&str],
    op_compatible: bool,
    op_ineligible_reason: Option<&str>,
    local_flags: Vec<Value>,
) -> Value {
    let mut object = Map::new();
    object.insert("path".to_string(), json!(path));
    object.insert("use".to_string(), json!(use_text));
    object.insert("short".to_string(), json!(short));
    object.insert("targetObjectKinds".to_string(), json!(target_kinds));
    object.insert("localFlags".to_string(), Value::Array(local_flags));
    object.insert("opCompatible".to_string(), json!(op_compatible));
    if let Some(reason) = op_ineligible_reason {
        object.insert("opIneligibleReason".to_string(), json!(reason));
    }
    Value::Object(object)
}

fn flag(name: &str, arg_name: &str, flag_type: &str, description: &str) -> Value {
    json!({
        "name": name,
        "argName": arg_name,
        "type": flag_type,
        "description": description,
    })
}

fn parse_u32_flag(args: &[String], name: &str) -> CliResult<Option<u32>> {
    parse_string_flag(args, name)?
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| CliError::invalid_args(format!("{name} must be an integer")))
        })
        .transpose()
}

fn parse_i64_flag(args: &[String], name: &str) -> CliResult<Option<i64>> {
    parse_string_flag(args, name)?
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| CliError::invalid_args(format!("{name} must be an integer")))
        })
        .transpose()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InspectPackageKind {
    Pptx,
    Xlsx,
    Docx,
    Unknown,
}

fn inspect(file: &str) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    match detect_inspect_package_type(file, &entries) {
        InspectPackageKind::Pptx => {
            let presentation = zip_text(file, "ppt/presentation.xml")?;
            let (cx, cy) = pptx_slide_size(&presentation)?;
            Ok(json!({
                "file": file,
                "summary": {
                    "customXmlParts": count_entries(&entries, "customXml/item", ".xml"),
                    "handoutMasters": count_entries(&entries, "ppt/handoutMasters/handoutMaster", ".xml"),
                    "layouts": count_entries(&entries, "ppt/slideLayouts/slideLayout", ".xml"),
                    "masters": count_entries(&entries, "ppt/slideMasters/slideMaster", ".xml"),
                    "mediaAssets": entries.iter().filter(|name| name.starts_with("ppt/media/")).count(),
                    "notesMasters": count_entries(&entries, "ppt/notesMasters/notesMaster", ".xml"),
                    "slideSize": {"cx": cx, "cy": cy, "unit": "emu"},
                    "slides": count_entries(&entries, "ppt/slides/slide", ".xml"),
                    "themes": count_entries(&entries, "ppt/theme/theme", ".xml"),
                },
                "type": "pptx",
            }))
        }
        InspectPackageKind::Xlsx => inspect_xlsx(file, &entries),
        InspectPackageKind::Docx => inspect_docx(file, &entries),
        InspectPackageKind::Unknown => Err(CliError::unsupported_type("unsupported type: unknown")),
    }
}

fn inspect_xlsx(file: &str, entries: &[String]) -> CliResult<Value> {
    let workbook_part = find_xlsx_workbook_part(file, entries)?;
    let workbook = zip_text(file, &workbook_part).map_err(|err| {
        CliError::unexpected(format!(
            "failed to inspect workbook: failed to read workbook part /{}: {}",
            workbook_part, err.message
        ))
    })?;
    let sheets = workbook_sheets(&workbook).map_err(|err| {
        if is_xml_parse_error(&err.message) {
            CliError::unexpected(format!(
                "failed to inspect workbook: failed to read workbook part /{}: failed to parse XML part /{}: {}",
                workbook_part,
                workbook_part,
                go_like_xml_parse_message(&err.message)
            ))
        } else {
            CliError::unexpected(format!(
                "failed to inspect workbook: workbook part /{} {}",
                workbook_part, err.message
            ))
        }
    })?;
    let workbook_rels =
        relationship_entries(file, &relationships_part_for(&workbook_part)).unwrap_or_default();
    let shared_strings_uri = workbook_rels
        .iter()
        .find(|rel| rel.rel_type.ends_with("/sharedStrings"))
        .map(|rel| resolve_relationship_target(&format!("/{workbook_part}"), &rel.target));
    let mut summary = Map::new();
    summary.insert("sheets".to_string(), json!(sheets.len()));
    summary.insert("worksheets".to_string(), json!(0));
    summary.insert("sharedStrings".to_string(), json!(false));
    summary.insert("styles".to_string(), json!(false));
    summary.insert("themes".to_string(), json!(0));
    summary.insert("tables".to_string(), json!(0));
    summary.insert("pivots".to_string(), json!(0));
    summary.insert("pivotCaches".to_string(), json!(0));
    summary.insert("charts".to_string(), json!(0));
    summary.insert("mediaAssets".to_string(), json!(0));
    summary.insert("customXmlParts".to_string(), json!(0));

    for entry in entries {
        let uri = format!("/{entry}");
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        if is_xlsx_worksheet_part(&uri, &content_type) {
            increment_json_count(&mut summary, "worksheets");
        } else if is_xlsx_shared_strings_part(&uri, &content_type) {
            summary.insert("sharedStrings".to_string(), json!(true));
        } else if is_xlsx_styles_part(&uri, &content_type) {
            summary.insert("styles".to_string(), json!(true));
        } else if is_xlsx_theme_part(&uri, &content_type) {
            increment_json_count(&mut summary, "themes");
        } else if is_xlsx_table_part(&uri, &content_type) {
            increment_json_count(&mut summary, "tables");
        } else if is_xlsx_pivot_table_part(&uri, &content_type) {
            increment_json_count(&mut summary, "pivots");
        } else if is_xlsx_pivot_cache_part(&uri, &content_type) {
            increment_json_count(&mut summary, "pivotCaches");
        } else if is_xlsx_chart_part(&uri, &content_type) {
            increment_json_count(&mut summary, "charts");
        } else if is_xlsx_media_part(&uri) {
            increment_json_count(&mut summary, "mediaAssets");
        } else if is_custom_xml_part(&uri) {
            increment_json_count(&mut summary, "customXmlParts");
        }
    }
    if let Some(shared_strings_uri) = shared_strings_uri {
        let count = shared_string_count(file, &shared_strings_uri).unwrap_or_default();
        if count > 0 {
            summary.insert("sharedStringCount".to_string(), json!(count));
        }
    }
    Ok(json!({
        "file": file,
        "summary": Value::Object(summary),
        "type": "xlsx",
    }))
}

fn inspect_docx(file: &str, entries: &[String]) -> CliResult<Value> {
    let document_part = find_docx_document_part(file, entries)?;
    let document = zip_text(file, &document_part).map_err(|_| {
        CliError::unexpected(format!(
            "failed to inspect document: failed to read document part /{}: part /{} not found",
            document_part, document_part
        ))
    })?;
    let counts = docx_body_summary_counts(&document).map_err(|err| {
        if is_xml_parse_error(&err) {
            CliError::unexpected(format!(
                "failed to inspect document: failed to read document part /{}: failed to parse XML part /{}: {}",
                document_part,
                document_part,
                go_like_docx_xml_parse_message(&err)
            ))
        } else {
            CliError::unexpected(format!(
                "failed to inspect document: document part /{} {}",
                document_part, err
            ))
        }
    })?;
    let mut summary = Map::new();
    summary.insert("paragraphs".to_string(), json!(counts.paragraphs));
    summary.insert("tables".to_string(), json!(counts.tables));
    summary.insert("hyperlinks".to_string(), json!(counts.hyperlinks));
    summary.insert("headers".to_string(), json!(0));
    summary.insert("footers".to_string(), json!(0));
    summary.insert("footnotes".to_string(), json!(false));
    summary.insert("endnotes".to_string(), json!(false));
    summary.insert("comments".to_string(), json!(false));
    summary.insert("sections".to_string(), json!(counts.sections));
    summary.insert("styles".to_string(), json!(false));
    summary.insert("numbering".to_string(), json!(false));
    summary.insert("mediaAssets".to_string(), json!(0));
    summary.insert("customXmlParts".to_string(), json!(0));

    for entry in entries {
        let uri = format!("/{entry}");
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        if is_docx_styles_part(&uri, &content_type) {
            summary.insert("styles".to_string(), json!(true));
        } else if is_docx_numbering_part(&uri, &content_type) {
            summary.insert("numbering".to_string(), json!(true));
        } else if is_docx_header_part(&uri, &content_type) {
            increment_json_count(&mut summary, "headers");
        } else if is_docx_footer_part(&uri, &content_type) {
            increment_json_count(&mut summary, "footers");
        } else if is_docx_footnotes_part(&uri, &content_type) {
            summary.insert("footnotes".to_string(), json!(true));
        } else if is_docx_endnotes_part(&uri, &content_type) {
            summary.insert("endnotes".to_string(), json!(true));
        } else if is_docx_comments_part(&uri, &content_type) {
            summary.insert("comments".to_string(), json!(true));
        } else if is_docx_media_part(&uri) {
            increment_json_count(&mut summary, "mediaAssets");
        } else if is_custom_xml_part(&uri) {
            increment_json_count(&mut summary, "customXmlParts");
        }
    }

    Ok(json!({
        "file": file,
        "summary": Value::Object(summary),
        "type": "docx",
    }))
}

fn detect_inspect_package_type(file: &str, entries: &[String]) -> InspectPackageKind {
    for rel in relationship_entries(file, "_rels/.rels").unwrap_or_default() {
        let target_uri = resolve_relationship_target("/", &rel.target);
        let target_content_type = content_type_for_part(file, &target_uri).unwrap_or_default();

        if rel.rel_type.contains("presentationml.presentation") {
            return InspectPackageKind::Pptx;
        }
        if target_content_type.contains("presentationml.presentation")
            || target_uri.starts_with("/ppt/")
        {
            return InspectPackageKind::Pptx;
        }

        if rel.rel_type.contains("wordprocessingml.document") {
            return InspectPackageKind::Docx;
        }
        if target_content_type.contains("wordprocessingml.document")
            || target_uri.starts_with("/word/")
        {
            return InspectPackageKind::Docx;
        }

        if rel.rel_type.contains("spreadsheetml.sheet") {
            return InspectPackageKind::Xlsx;
        }
        if target_content_type.contains("spreadsheetml.sheet") || target_uri.starts_with("/xl/") {
            return InspectPackageKind::Xlsx;
        }
    }

    for entry in entries {
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        if content_type.contains("presentationml") {
            return InspectPackageKind::Pptx;
        }
        if content_type.contains("wordprocessingml") {
            return InspectPackageKind::Docx;
        }
        if content_type.contains("spreadsheetml") {
            return InspectPackageKind::Xlsx;
        }
    }

    InspectPackageKind::Unknown
}

fn find_xlsx_workbook_part(file: &str, entries: &[String]) -> CliResult<String> {
    for rel in relationship_entries(file, "_rels/.rels").unwrap_or_default() {
        if rel.target_mode == "External" {
            continue;
        }
        let target = resolve_relationship_target("/", &rel.target);
        if is_xlsx_workbook_candidate(file, &target) {
            return Ok(target.trim_start_matches('/').to_string());
        }
    }
    for entry in entries {
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        if is_xlsx_workbook_content_type(&content_type) {
            return Ok(entry.clone());
        }
    }
    Err(CliError::unexpected("xlsx workbook part not found"))
}

fn is_xlsx_workbook_candidate(file: &str, uri: &str) -> bool {
    if uri.is_empty() || uri == "/" {
        return false;
    }
    let content_type = content_type_for_part(file, uri).unwrap_or_default();
    is_xlsx_workbook_content_type(&content_type) || uri == "/xl/workbook.xml"
}

fn is_xlsx_workbook_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"
            | "application/vnd.ms-excel.sheet.macroEnabled.main+xml"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml"
            | "application/vnd.ms-excel.addin.macroEnabled.main+xml"
    ) || content_type.contains("spreadsheetml.sheet.main+xml")
        || content_type.contains("spreadsheetml.template.main+xml")
        || content_type.contains("ms-excel.sheet.macroEnabled.main+xml")
        || content_type.contains("ms-excel.addin.macroEnabled.main+xml")
}

fn find_docx_document_part(file: &str, entries: &[String]) -> CliResult<String> {
    for rel in relationship_entries(file, "_rels/.rels").unwrap_or_default() {
        if rel.target_mode == "External" {
            continue;
        }
        let target = resolve_relationship_target("/", &rel.target);
        if rel.rel_type.ends_with("/officeDocument") || is_docx_document_candidate(file, &target) {
            return Ok(target.trim_start_matches('/').to_string());
        }
    }
    for entry in entries {
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        if is_docx_document_content_type(&content_type) {
            return Ok(entry.clone());
        }
    }
    Err(CliError::unexpected("docx main document part not found"))
}

fn is_docx_document_candidate(file: &str, uri: &str) -> bool {
    if uri.is_empty() || uri == "/" {
        return false;
    }
    let content_type = content_type_for_part(file, uri).unwrap_or_default();
    is_docx_document_content_type(&content_type) || uri == "/word/document.xml"
}

fn is_docx_document_content_type(content_type: &str) -> bool {
    content_type
        == "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
        || content_type.contains("wordprocessingml.document.main+xml")
}

fn relationships_part_for(part: &str) -> String {
    let normalized = part.trim_start_matches('/');
    if let Some((dir, name)) = normalized.rsplit_once('/') {
        format!("{dir}/_rels/{name}.rels")
    } else {
        format!("_rels/{normalized}.rels")
    }
}

fn is_xml_parse_error(message: &str) -> bool {
    message.contains("syntax error")
        || message.contains("unexpected EOF")
        || message.contains("not found before end of input")
}

fn go_like_xml_parse_message(message: &str) -> &'static str {
    if message.contains("unexpected EOF") || message.contains("not found before end of input") {
        "XML syntax error on line 1: unexpected EOF"
    } else {
        "XML syntax error"
    }
}

fn go_like_docx_xml_parse_message(message: &str) -> &'static str {
    if message.contains("unexpected EOF") {
        "etree: invalid XML format"
    } else {
        go_like_xml_parse_message(message)
    }
}

fn increment_json_count(summary: &mut Map<String, Value>, key: &str) {
    let next = summary.get(key).and_then(Value::as_u64).unwrap_or_default() + 1;
    summary.insert(key.to_string(), json!(next));
}

fn is_xlsx_worksheet_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"
        || is_xml_data_part(uri) && uri.starts_with("/xl/worksheets/")
}

fn is_xlsx_shared_strings_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"
        || uri == "/xl/sharedStrings.xml"
}

fn is_xlsx_styles_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"
        || uri == "/xl/styles.xml"
}

fn is_xlsx_theme_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.theme+xml"
        || is_xml_data_part(uri) && uri.starts_with("/xl/theme/")
}

fn is_xlsx_table_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"
        || is_xml_data_part(uri) && uri.starts_with("/xl/tables/")
}

fn is_xlsx_pivot_table_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"
        || is_xml_data_part(uri) && uri.starts_with("/xl/pivotTables/")
}

fn is_xlsx_pivot_cache_part(uri: &str, content_type: &str) -> bool {
    content_type
        == "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"
        || is_xml_data_part(uri)
            && uri.starts_with("/xl/pivotCache/")
            && file_name(uri).starts_with("pivotCacheDefinition")
}

fn is_xlsx_chart_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.drawingml.chart+xml"
        || is_xml_data_part(uri)
            && uri.starts_with("/xl/charts/")
            && file_name(uri).starts_with("chart")
}

fn is_xlsx_media_part(uri: &str) -> bool {
    uri.starts_with("/xl/media/") && !uri.contains("/_rels/")
}

fn is_docx_styles_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"
        || uri == "/word/styles.xml"
}

fn is_docx_numbering_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"
        || uri == "/word/numbering.xml"
}

fn is_docx_header_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"
        || is_xml_data_part(uri) && uri.starts_with("/word/header")
}

fn is_docx_footer_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"
        || is_xml_data_part(uri) && uri.starts_with("/word/footer")
}

fn is_docx_footnotes_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"
        || uri == "/word/footnotes.xml"
}

fn is_docx_endnotes_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"
        || uri == "/word/endnotes.xml"
}

fn is_docx_comments_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"
        || uri == "/word/comments.xml"
}

fn is_docx_media_part(uri: &str) -> bool {
    uri.starts_with("/word/media/") && !uri.contains("/_rels/")
}

fn is_custom_xml_part(uri: &str) -> bool {
    is_xml_data_part(uri) && uri.starts_with("/customXml/")
}

fn is_xml_data_part(uri: &str) -> bool {
    uri.ends_with(".xml") && !uri.contains("/_rels/") && !uri.ends_with(".rels")
}

fn file_name(uri: &str) -> &str {
    uri.rsplit('/').next().unwrap_or(uri)
}

#[derive(Default)]
struct DocxBodySummaryCounts {
    paragraphs: usize,
    tables: usize,
    hyperlinks: usize,
    sections: usize,
}

fn docx_body_summary_counts(xml: &str) -> Result<DocxBodySummaryCounts, String> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut counts = DocxBodySummaryCounts::default();
    let mut direct_sections = 0usize;
    let mut descendant_sections = 0usize;
    let mut block_depth: Option<usize> = None;
    let mut saw_document = false;
    let mut saw_body = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.is_empty() && name != "document" {
                    return Err(format!("root is {name:?}, expected document"));
                }
                if stack.is_empty() {
                    saw_document = true;
                }
                let parent = stack.last().map(String::as_str);
                if parent == Some("document") && name == "body" {
                    saw_body = true;
                }
                if parent == Some("body") && name == "p" {
                    counts.paragraphs += 1;
                    block_depth = Some(1);
                } else if parent == Some("body") && name == "tbl" {
                    counts.tables += 1;
                    block_depth = Some(1);
                } else if let Some(depth) = block_depth.as_mut() {
                    *depth += 1;
                }
                if name == "hyperlink" && block_depth.is_some() {
                    counts.hyperlinks += 1;
                }
                if name == "sectPr" && stack_contains(&stack, "body") {
                    descendant_sections += 1;
                    if parent == Some("body") {
                        direct_sections += 1;
                    }
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.is_empty() && name != "document" {
                    return Err(format!("root is {name:?}, expected document"));
                }
                if stack.is_empty() {
                    saw_document = true;
                }
                let parent = stack.last().map(String::as_str);
                if parent == Some("document") && name == "body" {
                    saw_body = true;
                }
                if parent == Some("body") && name == "p" {
                    counts.paragraphs += 1;
                } else if parent == Some("body") && name == "tbl" {
                    counts.tables += 1;
                }
                if name == "hyperlink" && block_depth.is_some() {
                    counts.hyperlinks += 1;
                }
                if name == "sectPr" && stack_contains(&stack, "body") {
                    descendant_sections += 1;
                    if parent == Some("body") {
                        direct_sections += 1;
                    }
                }
            }
            Ok(Event::End(_)) => {
                if let Some(depth) = block_depth.as_mut() {
                    *depth = depth.saturating_sub(1);
                    if *depth == 0 {
                        block_depth = None;
                    }
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(err.to_string()),
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err("unexpected EOF".to_string());
    }
    if !saw_document {
        return Err("has no root element".to_string());
    }
    if !saw_body {
        return Err("body element not found".to_string());
    }
    counts.sections = if direct_sections > 0 {
        direct_sections
    } else {
        descendant_sections
    };
    Ok(counts)
}

fn validate(file: &str, _strict: bool) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    if !entries.iter().any(|name| name == "[Content_Types].xml") {
        return Err(CliError::unexpected("missing [Content_Types].xml"));
    }
    Ok(json!({
        "file": file,
        "status": "valid",
        "summary": {"errors": 0, "info": 0, "warnings": 0},
        "valid": true,
    }))
}

fn pptx_slide_show(file: &str, slide: u32) -> CliResult<Value> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    if slide == 0 || slide as usize > slides.len() {
        return Err(CliError::invalid_args(format!(
            "slide number {slide} is out of range (1-{})",
            slides.len()
        )));
    }

    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let (slide_id, rel_id) = &slides[slide as usize - 1];
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    let part = normalize_ppt_target(target);
    let slide_xml = zip_text(file, &part)?;
    let layout_part = slide_layout_part(file, &part)?;
    let layout_name = layout_part
        .as_ref()
        .and_then(|part| zip_text(file, part).ok())
        .and_then(|xml| layout_display_name(&xml))
        .unwrap_or_else(|| "Title Slide".to_string());
    let layout_number = layout_part
        .as_ref()
        .and_then(|part| trailing_number(part, "slideLayout"))
        .unwrap_or(1);
    let shapes = pptx_shapes(&slide_xml);
    let part_uri = format!("/{}", part);
    let layout_part_uri = layout_part
        .as_ref()
        .map(|part| format!("/{part}"))
        .unwrap_or_else(|| "/ppt/slideLayouts/slideLayout1.xml".to_string());

    Ok(json!({
        "file": file,
        "slides": [{
            "id": format!("slide{slide}"),
            "layoutNumber": layout_number,
            "layoutPartUri": layout_part_uri,
            "layoutReadbackCommand": format!("ooxml --json pptx layouts show {file} --layout {layout_number}"),
            "layoutRef": layout_name,
            "partUri": part_uri,
            "primarySelector": slide.to_string(),
            "readbackCommand": format!("ooxml --json pptx slides show {file} --slide {slide} --include-text --include-bounds"),
            "relationshipId": rel_id,
            "selectors": [
                slide.to_string(),
                format!("part:/{}", part),
                format!("slideId:{slide_id}"),
                format!("rId:{rel_id}"),
            ],
            "selectorsCommand": format!("ooxml --json pptx slides selectors {file} --slide {slide}"),
            "shapes": shapes,
            "shapesCommand": format!("ooxml --json pptx shapes show {file} --slide {slide} --include-text --include-bounds"),
            "slide": slide,
            "slideId": slide_id,
        }],
    }))
}

fn pptx_slides_list(file: &str) -> CliResult<Value> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let mut slide_id_counts = BTreeMap::<u32, usize>::new();
    for (slide_id, _) in &slides {
        if *slide_id != 0 {
            *slide_id_counts.entry(*slide_id).or_default() += 1;
        }
    }
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let values = slides
        .iter()
        .enumerate()
        .map(|(index, (slide_id, rel_id))| {
            let slide_number = index as u32 + 1;
            let target = rels
                .get(rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            let part = normalize_ppt_target(target);
            let slide_xml = zip_text(file, &part)?;
            let (layout_part, notes_part) = slide_layout_and_notes_parts(file, &part)?;
            let layout_xml = layout_part.as_ref().and_then(|part| zip_text(file, part).ok());
            let layout_name = layout_xml
                .as_deref()
                .and_then(layout_display_name)
                .unwrap_or_default();
            let layout_number = layout_xml
                .as_ref()
                .and(layout_part.as_ref())
                .and_then(|part| trailing_number(part, "slideLayout"))
                .unwrap_or(0);
            let (text_shapes, images, tables) = pptx_slide_object_counts(&slide_xml);
            let part_uri = format!("/{part}");
            let layout_part_uri = layout_xml
                .as_ref()
                .and(layout_part.as_ref())
                .map(|part| format!("/{part}"));
            let notes_part_uri = notes_part.as_ref().map(|part| format!("/{part}"));
            let selectors = vec![
                slide_number.to_string(),
                format!("part:{part_uri}"),
                format!("slideId:{slide_id}"),
                format!("rId:{rel_id}"),
            ];
            let mut item = Map::new();
            item.insert("number".to_string(), json!(slide_number));
            item.insert("slideId".to_string(), json!(slide_id));
            item.insert("relationshipId".to_string(), json!(rel_id));
            item.insert("partUri".to_string(), json!(part_uri));
            item.insert("primarySelector".to_string(), json!(slide_number.to_string()));
            if *slide_id != 0 && slide_id_counts.get(slide_id).copied().unwrap_or_default() == 1 {
                item.insert("handle".to_string(), json!(format!("H:pptx/s:{slide_id}")));
            }
            item.insert("selectors".to_string(), json!(selectors));
            item.insert("layout".to_string(), json!(layout_name));
            if layout_number > 0 {
                item.insert("layoutNumber".to_string(), json!(layout_number));
            }
            if let Some(layout_part_uri) = layout_part_uri {
                item.insert("layoutPartUri".to_string(), json!(layout_part_uri));
            }
            if let Some(notes_part_uri) = notes_part_uri {
                item.insert("notesPartUri".to_string(), json!(notes_part_uri));
            }
            item.insert("textShapes".to_string(), json!(text_shapes));
            item.insert("images".to_string(), json!(images));
            item.insert("tables".to_string(), json!(tables));
            item.insert("notes".to_string(), json!(notes_part.is_some()));
            item.insert(
                "readbackCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx slides show {file} --slide {slide_number} --include-text --include-bounds"
                )),
            );
            item.insert(
                "selectorsCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx slides selectors {file} --slide {slide_number}"
                )),
            );
            item.insert(
                "shapesCommand".to_string(),
                json!(format!(
                    "ooxml --json pptx shapes show {file} --slide {slide_number} --include-text --include-bounds"
                )),
            );
            if tables > 0 {
                item.insert(
                    "tablesCommand".to_string(),
                    json!(format!(
                        "ooxml --json pptx tables show {file} --slide {slide_number}"
                    )),
                );
            }
            if layout_number > 0 {
                item.insert(
                    "layoutReadbackCommand".to_string(),
                    json!(format!(
                        "ooxml --json pptx layouts show {file} --layout {layout_number}"
                    )),
                );
            }
            Ok(Value::Object(item))
        })
        .collect::<CliResult<Vec<_>>>()?;
    Ok(json!({"file": file, "slides": values}))
}

fn pptx_slide_selectors(file: &str, slide: u32) -> CliResult<Value> {
    if slide == 0 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let index = slide as usize - 1;
    let (_, rel_id) = slides
        .get(index)
        .ok_or_else(|| CliError::unexpected(format!("slide {slide} not found")))?;
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    let part = normalize_ppt_target(target);
    let slide_xml = zip_text(file, &part)?;
    let (layout_part, _) = slide_layout_and_notes_parts(file, &part)?;
    let layout_xml = layout_part
        .as_ref()
        .and_then(|part| zip_text(file, part).ok());
    let layout_name = layout_xml.as_deref().and_then(layout_display_name);
    let layout_part_uri = layout_xml
        .as_ref()
        .and(layout_part.as_ref())
        .map(|part| format!("/{part}"));

    let mut output = Map::new();
    output.insert("file".to_string(), json!(file));
    output.insert("slide".to_string(), json!(slide));
    output.insert("partUri".to_string(), json!(format!("/{part}")));
    if let Some(layout_name) = layout_name.filter(|name| !name.is_empty()) {
        output.insert("layoutName".to_string(), json!(layout_name));
    }
    if let Some(layout_part_uri) = layout_part_uri {
        output.insert("layoutPartUri".to_string(), json!(layout_part_uri));
    }
    output.insert(
        "targets".to_string(),
        Value::Array(pptx_selector_targets(&slide_xml)),
    );
    Ok(Value::Object(output))
}

fn pptx_shapes_show(
    file: &str,
    slide: u32,
    include_text: bool,
    include_bounds: bool,
) -> CliResult<Value> {
    if slide == 0 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let mut slide_id_counts = BTreeMap::<u32, usize>::new();
    for (slide_id, _) in &slides {
        if *slide_id != 0 {
            *slide_id_counts.entry(*slide_id).or_default() += 1;
        }
    }
    let index = slide as usize - 1;
    let (slide_id, rel_id) = slides.get(index).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide {slide} not found (presentation has {} slides)",
            slides.len()
        ))
    })?;
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    let part = normalize_ppt_target(target);
    let slide_xml = zip_text(file, &part)?;
    let (layout_part, _) = slide_layout_and_notes_parts(file, &part)?;
    let layout_xml = layout_part
        .as_ref()
        .and_then(|part| zip_text(file, part).ok());
    let layout_name = layout_xml.as_deref().and_then(layout_display_name);
    let layout_part_uri = layout_xml
        .as_ref()
        .and(layout_part.as_ref())
        .map(|part| format!("/{part}"));
    let slide_id_unique =
        *slide_id != 0 && slide_id_counts.get(slide_id).copied().unwrap_or_default() == 1;

    let mut output = Map::new();
    output.insert("file".to_string(), json!(file));
    output.insert("slide".to_string(), json!(slide));
    output.insert("partUri".to_string(), json!(format!("/{part}")));
    if let Some(layout_name) = layout_name.filter(|name| !name.is_empty()) {
        output.insert("layoutName".to_string(), json!(layout_name));
    }
    if let Some(layout_part_uri) = layout_part_uri {
        output.insert("layoutPartUri".to_string(), json!(layout_part_uri));
    }
    output.insert(
        "shapes".to_string(),
        Value::Array(pptx_shape_show_entries(
            file,
            &part,
            &slide_xml,
            *slide_id,
            slide_id_unique,
            include_text,
            include_bounds,
        )),
    );
    Ok(Value::Object(output))
}

struct XlsxRangeExportOptions<'a> {
    include_types: bool,
    include_formulas: bool,
    include_formats: bool,
    data_out: Option<&'a str>,
    max_cells: i64,
}

fn xlsx_range_export(file: &str, sheet_selector: &str, range: &str) -> CliResult<Value> {
    xlsx_range_export_with_options(
        file,
        sheet_selector,
        range,
        XlsxRangeExportOptions {
            include_types: true,
            include_formulas: false,
            include_formats: false,
            data_out: None,
            max_cells: 100000,
        },
    )
}

fn xlsx_range_export_with_options(
    file: &str,
    sheet_selector: &str,
    range: &str,
    options: XlsxRangeExportOptions<'_>,
) -> CliResult<Value> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let shared_strings = shared_strings(file).unwrap_or_default();
    let styles = xlsx_styles(file).unwrap_or_default();
    let sheet_xml = zip_text(file, &sheet_part)?;
    let cells = sheet_cells(&sheet_xml, &shared_strings, &styles);
    let bounds = parse_cli_range(range)?;
    check_range_max_cells(range, bounds, options.max_cells)?;
    let mut values = Vec::new();
    let mut types = Vec::new();
    let mut formulas = Vec::new();
    let mut style_indexes = Vec::new();
    let mut number_format_ids = Vec::new();
    let mut number_format_codes = Vec::new();
    let mut formula_count = 0;
    let mut has_format_readback = false;
    for row in bounds.min_row()..=bounds.max_row() {
        let mut row_values = Vec::new();
        let mut row_types = Vec::new();
        let mut row_formulas = Vec::new();
        let mut row_style_indexes = Vec::new();
        let mut row_number_format_ids = Vec::new();
        let mut row_number_format_codes = Vec::new();
        for col in bounds.min_col()..=bounds.max_col() {
            let addr = format!("{}{}", col_name(col), row);
            if let Some(cell) = cells.get(&addr) {
                if cell.has_formula {
                    formula_count += 1;
                }
                row_values.push(cell.matrix_value.clone());
                row_types.push(Value::String(cell.kind.clone()));
                if cell.formula.is_empty() {
                    row_formulas.push(Value::Null);
                } else {
                    row_formulas.push(Value::String(cell.formula.clone()));
                }
                let style_index = cell.style_index.unwrap_or(0);
                let number_format_id = cell.number_format_id.unwrap_or(0);
                let number_format_code = cell.number_format_code.clone().unwrap_or_default();
                let has_cell_format =
                    style_index != 0 || number_format_id != 0 || !number_format_code.is_empty();
                if has_cell_format {
                    has_format_readback = true;
                    row_style_indexes.push(json!(style_index));
                    row_number_format_ids.push(json!(number_format_id));
                    if number_format_code.is_empty() {
                        row_number_format_codes.push(Value::Null);
                    } else {
                        row_number_format_codes.push(Value::String(number_format_code));
                    }
                } else {
                    row_style_indexes.push(Value::Null);
                    row_number_format_ids.push(Value::Null);
                    row_number_format_codes.push(Value::Null);
                }
            } else {
                row_values.push(Value::Null);
                row_types.push(Value::String("empty".to_string()));
                row_formulas.push(Value::Null);
                row_style_indexes.push(Value::Null);
                row_number_format_ids.push(Value::Null);
                row_number_format_codes.push(Value::Null);
            }
        }
        values.push(Value::Array(row_values));
        types.push(Value::Array(row_types));
        formulas.push(Value::Array(row_formulas));
        style_indexes.push(Value::Array(row_style_indexes));
        number_format_ids.push(Value::Array(row_number_format_ids));
        number_format_codes.push(Value::Array(row_number_format_codes));
    }
    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let mut output = Map::new();
    output.insert(
        "cellsExtractCommand".to_string(),
        json!(format!(
            "ooxml --json xlsx cells extract {} --sheet {} --range {}",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range)
        )),
    );
    output.insert("cols".to_string(), json!(cols));
    output.insert("dataFormat".to_string(), json!("json"));
    output.insert("file".to_string(), json!(file));
    output.insert("formulaCount".to_string(), json!(formula_count));
    output.insert("majorDimension".to_string(), json!("rows"));
    output.insert(
        "pptxPlaceTableCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json pptx place table-from-xlsx deck.pptx --workbook {} --sheet {} --range {} --expect-source-range {} --slide 1 --x 0 --y 0 --cx 4000000 --out out.pptx",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range),
            command_arg(range)
        )),
    );
    output.insert(
        "pptxReplaceTextCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json pptx replace text-from-xlsx deck.pptx --workbook {} --sheet {} --range {} --slide 1 --target title --out out.pptx",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range)
        )),
    );
    output.insert(
        "pptxUpdateTableCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json pptx tables update-from-xlsx deck.pptx --workbook {} --sheet {} --range {} --expect-source-range {} --slide 1 --target table:1 --out out.pptx",
            command_arg(file),
            command_arg(&sheet.name),
            command_arg(range),
            command_arg(range)
        )),
    );
    output.insert("primarySelector".to_string(), json!(range));
    output.insert("range".to_string(), json!(range));
    output.insert("rows".to_string(), json!(rows));
    output.insert("selectors".to_string(), json!([range]));
    output.insert("sheet".to_string(), json!(sheet.name));
    output.insert("sheetNumber".to_string(), json!(sheet.position));
    output.insert("truncated".to_string(), json!(false));
    if options.include_types {
        output.insert("types".to_string(), Value::Array(types));
    }
    if options.include_formulas {
        output.insert("formulas".to_string(), Value::Array(formulas));
    }
    if options.include_formats && has_format_readback {
        output.insert("styleIndexes".to_string(), Value::Array(style_indexes));
        output.insert(
            "numberFormatIds".to_string(),
            Value::Array(number_format_ids),
        );
        output.insert(
            "numberFormatCodes".to_string(),
            Value::Array(number_format_codes),
        );
    }
    output.insert(
        "validateCommand".to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(file))),
    );
    output.insert("values".to_string(), Value::Array(values));
    if let Some(data_out) = options.data_out.filter(|data_out| !data_out.is_empty()) {
        output.insert("dataOut".to_string(), json!(data_out));
        let mut data = serde_json::to_vec(&Value::Object(output.clone()))
            .map_err(|err| CliError::unexpected(format!("failed to marshal range JSON: {err}")))?;
        data.push(b'\n');
        fs::write(data_out, data)
            .map_err(|err| CliError::unexpected(format!("failed to write --data-out: {err}")))?;
        output.remove("values");
        output.remove("types");
        output.remove("formulas");
        output.remove("styleIndexes");
        output.remove("numberFormatIds");
        output.remove("numberFormatCodes");
    }
    Ok(Value::Object(output))
}

fn require_json_data_format(data_format: Option<&str>) -> CliResult<()> {
    let data_format = data_format.unwrap_or("json").trim().to_ascii_lowercase();
    if data_format.is_empty() || data_format == "json" {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "unsupported Rust-port data format {data_format:?}; only json is implemented"
        )))
    }
}

fn normalize_xlsx_ranges_set_data_format(data_format: Option<&str>) -> CliResult<String> {
    let normalized = data_format.unwrap_or("json").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" | "json" => Ok("json".to_string()),
        "csv" => Ok("csv".to_string()),
        "tsv" => Ok("tsv".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "invalid data format {data_format:?} (must be json, csv, or tsv)",
        ))),
    }
}

fn check_range_max_cells(range: &str, bounds: RangeBounds, max_cells: i64) -> CliResult<()> {
    if max_cells < 0 {
        return Err(CliError::invalid_args("--max-cells must be >= 0"));
    }
    let rows = i64::from(bounds.row_count());
    let cols = i64::from(bounds.col_count());
    let cell_count = rows.saturating_mul(cols);
    if max_cells > 0 && cell_count > max_cells {
        return Err(CliError::invalid_args(format!(
            "range {range} contains {cell_count} cells, above --max-cells {max_cells}"
        )));
    }
    Ok(())
}

struct XlsxRangesSetOptions<'a> {
    sheet: &'a str,
    range: Option<&'a str>,
    anchor: Option<&'a str>,
    values: Option<&'a str>,
    values_file: Option<&'a str>,
    data_format: Option<&'a str>,
    null_policy: Option<&'a str>,
    ragged: Option<&'a str>,
    max_cells: i64,
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
    in_place: bool,
    overwrite_formulas: bool,
}

#[derive(Clone)]
struct XlsxMatrixCell {
    kind: String,
    value: String,
    formula: String,
    null: bool,
}

struct XlsxRangeSetMatrix {
    range: Option<String>,
    null_policy: Option<String>,
    major_dimension: String,
    rows: Vec<Vec<XlsxMatrixCell>>,
}

#[derive(Default)]
struct XlsxRangeSetStats {
    updated: usize,
    created: usize,
    cleared: usize,
    skipped: usize,
    formula_count: usize,
}

fn xlsx_ranges_set(file: &str, options: XlsxRangesSetOptions<'_>) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let data_format = normalize_xlsx_ranges_set_data_format(options.data_format)?;
    let data = resolve_xlsx_ranges_set_values(options.values, options.values_file)?;
    let mut matrix = parse_xlsx_range_set_matrix(&data, &data_format)?;
    rectangularize_xlsx_matrix(&mut matrix.rows, options.ragged.unwrap_or("reject"))?;
    let null_policy = options
        .null_policy
        .map(ToString::to_string)
        .or_else(|| matrix.null_policy.clone())
        .unwrap_or_else(|| "skip".to_string());
    validate_xlsx_null_policy(&null_policy)?;
    let bounds = resolve_xlsx_ranges_set_bounds(
        options.range,
        options.anchor,
        matrix.range.as_deref(),
        matrix.rows.len(),
        matrix.rows.first().map_or(0, Vec::len),
    )?;
    let range = range_bounds_ref(bounds);
    check_range_max_cells(&range, bounds, options.max_cells)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheet, sheet_part) = resolve_xlsx_sheet_context(file, options.sheet)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (updated_xml, stats) = set_xlsx_range_in_sheet_xml(
        &sheet_xml,
        bounds,
        &matrix.rows,
        &null_policy,
        options.overwrite_formulas,
    )?;

    let output_path = options.out.filter(|value| !value.is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        xlsx_ranges_set_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_override(file, &readback_path, &sheet_part, &updated_xml)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let destination =
        xlsx_range_destination_json(&readback_path, commit_path, &sheet, &sheet_part, &range)?;
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }

    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert(
        "anchor".to_string(),
        json!(format!(
            "{}{}",
            col_name(bounds.start_col),
            bounds.start_row
        )),
    );
    result.insert("range".to_string(), json!(range));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
    result.insert("updated".to_string(), json!(stats.updated));
    result.insert("created".to_string(), json!(stats.created));
    result.insert("cleared".to_string(), json!(stats.cleared));
    result.insert("skipped".to_string(), json!(stats.skipped));
    result.insert("formulaCount".to_string(), json!(stats.formula_count));
    result.insert("dataFormat".to_string(), json!(data_format));
    result.insert("nullPolicy".to_string(), json!(null_policy));
    result.insert("majorDimension".to_string(), json!(matrix.major_dimension));
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_range_mutation_commands(
        &mut result,
        commit_path,
        &format!("sheetId:{}", sheet.sheet_id),
        &range,
    );
    Ok(Value::Object(result))
}

struct XlsxRangesSetFormatOptions<'a> {
    sheet: &'a str,
    range: &'a str,
    preset: Option<&'a str>,
    format_code: Option<&'a str>,
    decimals: i64,
    currency_symbol: Option<&'a str>,
    max_cells: i64,
    out: Option<&'a str>,
    backup: Option<&'a str>,
    dry_run: bool,
    no_validate: bool,
    in_place: bool,
}

#[derive(Clone)]
struct XlsxNumberFormatSpec {
    preset: String,
    format_code: String,
    number_format_id: u32,
    builtin: bool,
}

#[derive(Default)]
struct XlsxRangeFormatStats {
    updated: usize,
    created: usize,
    created_styles: usize,
    style_indexes: BTreeSet<u32>,
}

fn xlsx_ranges_set_format(file: &str, options: XlsxRangesSetFormatOptions<'_>) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    let bounds = parse_cli_range(options.range)?;
    let range = range_bounds_ref(bounds);
    check_range_max_cells(&range, bounds, options.max_cells)?;
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let spec = resolve_xlsx_number_format(
        options.preset,
        options.format_code,
        options.decimals,
        options.currency_symbol,
    )?;

    let (sheet, sheet_part) = resolve_xlsx_sheet_context(file, options.sheet)?;
    let sheet_xml = zip_text(file, &sheet_part)?;
    let (styles_part, rels_override) = resolve_or_add_xlsx_styles_part(file)?;
    let styles_xml = zip_text(file, &styles_part).unwrap_or_else(|_| default_xlsx_styles_xml());
    let (styles_xml, number_format_id) = ensure_xlsx_number_format(styles_xml, &spec)?;
    let (updated_sheet_xml, styles_xml, stats) =
        set_xlsx_range_number_format_xml(&sheet_xml, styles_xml, bounds, number_format_id)?;
    let content_types_xml = ensure_content_type_override(
        zip_text(file, "[Content_Types].xml")?,
        &format!("/{styles_part}"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
    );

    let output_path = options.out.filter(|value| !value.is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        xlsx_ranges_set_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };

    let mut overrides = BTreeMap::new();
    overrides.insert(sheet_part.clone(), updated_sheet_xml);
    overrides.insert(styles_part.clone(), styles_xml);
    overrides.insert("[Content_Types].xml".to_string(), content_types_xml);
    if let Some(rels_xml) = rels_override {
        overrides.insert("xl/_rels/workbook.xml.rels".to_string(), rels_xml);
    }
    copy_zip_with_part_overrides(file, &readback_path, &overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let destination =
        xlsx_range_destination_json(&readback_path, commit_path, &sheet, &sheet_part, &range)?;
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }

    let rows = bounds.row_count();
    let cols = bounds.col_count();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("sheet".to_string(), json!(sheet.name));
    result.insert("sheetNumber".to_string(), json!(sheet.position));
    result.insert("range".to_string(), json!(range));
    result.insert("rows".to_string(), json!(rows));
    result.insert("cols".to_string(), json!(cols));
    if !spec.preset.is_empty() {
        result.insert("preset".to_string(), json!(spec.preset));
    }
    result.insert("formatCode".to_string(), json!(spec.format_code));
    result.insert("numberFormatId".to_string(), json!(number_format_id));
    result.insert("builtin".to_string(), json!(spec.builtin));
    result.insert("updated".to_string(), json!(stats.updated));
    result.insert("created".to_string(), json!(stats.created));
    result.insert("createdStyles".to_string(), json!(stats.created_styles));
    if !stats.style_indexes.is_empty() {
        result.insert(
            "styleIndexes".to_string(),
            json!(stats.style_indexes.into_iter().collect::<Vec<_>>()),
        );
    }
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("destination".to_string(), destination);
    add_xlsx_range_mutation_commands(
        &mut result,
        commit_path,
        &format!("sheetId:{}", sheet.sheet_id),
        &range,
    );
    Ok(Value::Object(result))
}

fn resolve_xlsx_number_format(
    preset: Option<&str>,
    format_code: Option<&str>,
    decimals: i64,
    currency_symbol: Option<&str>,
) -> CliResult<XlsxNumberFormatSpec> {
    let preset = preset.unwrap_or_default().trim().to_ascii_lowercase();
    let format_code = format_code.unwrap_or_default().trim();
    if preset.is_empty() == format_code.is_empty() {
        return Err(CliError::invalid_args(
            "specify exactly one of preset or format code",
        ));
    }
    if !(0..=10).contains(&decimals) {
        return Err(CliError::invalid_args("decimals must be between 0 and 10"));
    }
    if !format_code.is_empty() {
        return Ok(XlsxNumberFormatSpec {
            preset: "custom".to_string(),
            format_code: format_code.to_string(),
            number_format_id: 0,
            builtin: false,
        });
    }
    match preset.as_str() {
        "general" => builtin_xlsx_number_format_spec("general", 0),
        "integer" => builtin_xlsx_number_format_spec("integer", 3),
        "number" => {
            let code = fixed_decimal_format("#,##0", decimals);
            match decimals {
                0 => builtin_xlsx_number_format_spec("number", 3),
                2 => builtin_xlsx_number_format_spec("number", 4),
                _ => custom_xlsx_number_format_spec("number", &code),
            }
        }
        "percent" => {
            let code = format!("{}%", fixed_decimal_format("0", decimals));
            match decimals {
                0 => builtin_xlsx_number_format_spec("percent", 9),
                2 => builtin_xlsx_number_format_spec("percent", 10),
                _ => custom_xlsx_number_format_spec("percent", &code),
            }
        }
        "currency" => {
            let symbol = currency_symbol.unwrap_or("$");
            let code = format!(
                "{}{}",
                xlsx_format_literal(symbol),
                fixed_decimal_format("#,##0", decimals)
            );
            custom_xlsx_number_format_spec("currency", &code)
        }
        "date" => custom_xlsx_number_format_spec("date", "yyyy-mm-dd"),
        "datetime" => custom_xlsx_number_format_spec("datetime", "yyyy-mm-dd h:mm"),
        "text" => builtin_xlsx_number_format_spec("text", 49),
        _ => Err(CliError::invalid_args(format!(
            "invalid preset {:?} (must be integer, number, currency, percent, date, datetime, text, or general)",
            preset
        ))),
    }
}

fn builtin_xlsx_number_format_spec(
    preset: &str,
    number_format_id: u32,
) -> CliResult<XlsxNumberFormatSpec> {
    let code = builtin_num_format_code(number_format_id).ok_or_else(|| {
        CliError::unexpected(format!(
            "unknown built-in number format id {number_format_id}"
        ))
    })?;
    Ok(XlsxNumberFormatSpec {
        preset: preset.to_string(),
        format_code: code.to_string(),
        number_format_id,
        builtin: true,
    })
}

fn custom_xlsx_number_format_spec(preset: &str, code: &str) -> CliResult<XlsxNumberFormatSpec> {
    Ok(XlsxNumberFormatSpec {
        preset: preset.to_string(),
        format_code: code.to_string(),
        number_format_id: 0,
        builtin: false,
    })
}

fn fixed_decimal_format(base: &str, decimals: i64) -> String {
    if decimals == 0 {
        base.to_string()
    } else {
        format!("{base}.{}", "0".repeat(decimals as usize))
    }
}

fn xlsx_format_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn resolve_or_add_xlsx_styles_part(file: &str) -> CliResult<(String, Option<String>)> {
    let rels_part = "xl/_rels/workbook.xml.rels";
    let rels_xml = zip_text(file, rels_part)?;
    let rels = relationship_entries(file, rels_part)?;
    for rel in &rels {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
        {
            return Ok((normalize_xl_target(&rel.target), None));
        }
    }
    let next_id = allocate_relationship_id(&rels);
    let rel = format!(
        r#"<Relationship Id="{next_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#
    );
    let updated = if let Some(pos) = rels_xml.rfind("</Relationships>") {
        let mut out = String::with_capacity(rels_xml.len() + rel.len());
        out.push_str(&rels_xml[..pos]);
        out.push_str(&rel);
        out.push_str(&rels_xml[pos..]);
        out
    } else {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rel}</Relationships>"#
        )
    };
    Ok(("xl/styles.xml".to_string(), Some(updated)))
}

fn allocate_relationship_id(rels: &[RelationshipEntry]) -> String {
    let mut next = 1u32;
    for rel in rels {
        if let Some(suffix) = rel.id.strip_prefix("rId")
            && let Ok(id) = suffix.parse::<u32>()
            && id >= next
        {
            next = id + 1;
        }
    }
    format!("rId{next}")
}

fn ensure_content_type_override(xml: String, part_name: &str, content_type: &str) -> String {
    let normalized = format!("/{}", part_name.trim_start_matches('/'));
    if xml.contains(&format!(r#"PartName="{normalized}""#)) {
        return xml;
    }
    let override_xml = format!(
        r#"<Override PartName="{normalized}" ContentType="{}"/>"#,
        xml_attr_escape(content_type)
    );
    if let Some(pos) = xml.rfind("</Types>") {
        let mut out = String::with_capacity(xml.len() + override_xml.len());
        out.push_str(&xml[..pos]);
        out.push_str(&override_xml);
        out.push_str(&xml[pos..]);
        out
    } else {
        xml
    }
}

fn default_xlsx_styles_xml() -> String {
    r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font/></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#.to_string()
}

fn ensure_xlsx_number_format(
    styles_xml: String,
    spec: &XlsxNumberFormatSpec,
) -> CliResult<(String, u32)> {
    let styles_xml = ensure_xlsx_style_defaults(styles_xml);
    if spec.builtin {
        return Ok((styles_xml, spec.number_format_id));
    }
    for (id, code) in parse_xlsx_num_formats(&styles_xml) {
        if code == spec.format_code {
            return Ok((styles_xml, id));
        }
    }
    let mut next_id = 164u32;
    for (id, _) in parse_xlsx_num_formats(&styles_xml) {
        if id >= next_id {
            next_id = id + 1;
        }
    }
    let num_fmt = format!(
        r#"<numFmt numFmtId="{next_id}" formatCode="{}"/>"#,
        xml_attr_escape(&spec.format_code)
    );
    let updated = if let Some(span) = element_span_by_local_name(&styles_xml, "numFmts") {
        let mut out = String::with_capacity(styles_xml.len() + num_fmt.len());
        out.push_str(&styles_xml[..span.close_start]);
        out.push_str(&num_fmt);
        out.push_str(&styles_xml[span.close_start..]);
        set_collection_count(out, "numFmts", "numFmt")
    } else {
        insert_xlsx_styles_collection(
            &styles_xml,
            "numFmts",
            &format!(r#"<numFmts count="1">{num_fmt}</numFmts>"#),
        )
    };
    Ok((updated, next_id))
}

fn ensure_xlsx_style_defaults(mut styles_xml: String) -> String {
    if !styles_xml.contains("<styleSheet") {
        return default_xlsx_styles_xml();
    }
    let defaults = [
        ("fonts", r#"<fonts count="1"><font/></fonts>"#),
        (
            "fills",
            r#"<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>"#,
        ),
        ("borders", r#"<borders count="1"><border/></borders>"#),
        (
            "cellStyleXfs",
            r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
        ),
        (
            "cellXfs",
            r#"<cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs>"#,
        ),
        (
            "cellStyles",
            r#"<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>"#,
        ),
    ];
    for (name, block) in defaults {
        if element_span_by_local_name(&styles_xml, name).is_none() {
            styles_xml = insert_xlsx_styles_collection(&styles_xml, name, block);
        }
    }
    styles_xml
}

fn insert_xlsx_styles_collection(styles_xml: &str, name: &str, block: &str) -> String {
    let target_order = xlsx_styles_collection_order(name);
    for candidate in [
        "numFmts",
        "fonts",
        "fills",
        "borders",
        "cellStyleXfs",
        "cellXfs",
        "cellStyles",
        "dxfs",
        "tableStyles",
        "colors",
        "extLst",
    ] {
        if xlsx_styles_collection_order(candidate) > target_order
            && let Some(span) = element_span_by_local_name(styles_xml, candidate)
        {
            let mut out = String::with_capacity(styles_xml.len() + block.len());
            out.push_str(&styles_xml[..span.start]);
            out.push_str(block);
            out.push_str(&styles_xml[span.start..]);
            return out;
        }
    }
    if let Some(pos) = styles_xml.rfind("</styleSheet>") {
        let mut out = String::with_capacity(styles_xml.len() + block.len());
        out.push_str(&styles_xml[..pos]);
        out.push_str(block);
        out.push_str(&styles_xml[pos..]);
        out
    } else {
        styles_xml.to_string()
    }
}

fn xlsx_styles_collection_order(name: &str) -> u32 {
    match name {
        "numFmts" => 10,
        "fonts" => 20,
        "fills" => 30,
        "borders" => 40,
        "cellStyleXfs" => 50,
        "cellXfs" => 60,
        "cellStyles" => 70,
        "dxfs" => 80,
        "tableStyles" => 90,
        "colors" => 100,
        "extLst" => 110,
        _ => 1000,
    }
}

#[derive(Clone, Copy)]
struct XmlElementSpan {
    start: usize,
    open_end: usize,
    close_start: usize,
}

fn element_span_by_local_name(xml: &str, wanted: &str) -> Option<XmlElementSpan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == wanted => {
                let open_end = reader.buffer_position() as usize;
                let mut depth = 1usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::Start(e)) if local_name(e.name().as_ref()) == wanted => {
                            depth += 1;
                        }
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == wanted => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(XmlElementSpan {
                                    start: before,
                                    open_end,
                                    close_start: inner_before,
                                });
                            }
                        }
                        Ok(Event::Eof) | Err(_) => return None,
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == wanted => {
                let end = reader.buffer_position() as usize;
                return Some(XmlElementSpan {
                    start: before,
                    open_end: end,
                    close_start: before,
                });
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn parse_xlsx_num_formats(styles_xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(styles_xml);
    reader.config_mut().trim_text(false);
    let mut formats = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "numFmt" =>
            {
                if let (Some(id), Some(code)) = (attr(&e, "numFmtId"), attr(&e, "formatCode"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    formats.push((id, code));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    formats
}

#[derive(Clone)]
struct XlsxXfEntry {
    attrs: BTreeMap<String, String>,
    inner_xml: String,
}

fn parse_xlsx_cell_xfs(styles_xml: &str) -> CliResult<Vec<XlsxXfEntry>> {
    let Some(parent) = element_span_by_local_name(styles_xml, "cellXfs") else {
        return Ok(Vec::new());
    };
    let fragment = &styles_xml[parent.open_end..parent.close_start];
    let base = parent.open_end;
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut entries = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "xf" => {
                let attrs = xml_attrs(&e);
                let open_end = reader.buffer_position() as usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "xf" => {
                            entries.push(XlsxXfEntry {
                                attrs,
                                inner_xml: styles_xml[base + open_end..base + inner_before]
                                    .to_string(),
                            });
                            break;
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("xf has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "xf" => {
                let _ = before;
                entries.push(XlsxXfEntry {
                    attrs: xml_attrs(&e),
                    inner_xml: String::new(),
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(entries)
}

fn ensure_xlsx_cell_style(
    styles_xml: String,
    base_style_index: u32,
    number_format_id: u32,
) -> CliResult<(String, u32, bool)> {
    let styles_xml = ensure_xlsx_style_defaults(styles_xml);
    let xfs = parse_xlsx_cell_xfs(&styles_xml)?;
    let base_index = if (base_style_index as usize) < xfs.len() {
        base_style_index
    } else {
        0
    };
    let base = xfs
        .get(base_index as usize)
        .cloned()
        .unwrap_or_else(default_xlsx_xf_entry);
    if xlsx_xf_num_fmt_id(&base.attrs) == number_format_id {
        return Ok((styles_xml, base_index, false));
    }
    let mut attrs = base.attrs.clone();
    for (key, value) in [
        ("fontId", "0"),
        ("fillId", "0"),
        ("borderId", "0"),
        ("xfId", "0"),
    ] {
        attrs
            .entry(key.to_string())
            .or_insert_with(|| value.to_string());
    }
    attrs.insert("numFmtId".to_string(), number_format_id.to_string());
    attrs.insert("applyNumberFormat".to_string(), "1".to_string());
    let candidate = XlsxXfEntry {
        attrs,
        inner_xml: base.inner_xml,
    };
    let candidate_sig = render_xlsx_xf(&candidate);
    for (index, xf) in xfs.iter().enumerate() {
        if render_xlsx_xf(xf) == candidate_sig {
            return Ok((styles_xml, index as u32, false));
        }
    }
    let Some(parent) = element_span_by_local_name(&styles_xml, "cellXfs") else {
        return Err(CliError::unexpected("styles cellXfs not found"));
    };
    let mut out = String::with_capacity(styles_xml.len() + candidate_sig.len());
    out.push_str(&styles_xml[..parent.close_start]);
    out.push_str(&candidate_sig);
    out.push_str(&styles_xml[parent.close_start..]);
    let out = set_collection_count(out, "cellXfs", "xf");
    Ok((out, xfs.len() as u32, true))
}

fn default_xlsx_xf_entry() -> XlsxXfEntry {
    let mut attrs = BTreeMap::new();
    attrs.insert("numFmtId".to_string(), "0".to_string());
    attrs.insert("fontId".to_string(), "0".to_string());
    attrs.insert("fillId".to_string(), "0".to_string());
    attrs.insert("borderId".to_string(), "0".to_string());
    attrs.insert("xfId".to_string(), "0".to_string());
    XlsxXfEntry {
        attrs,
        inner_xml: String::new(),
    }
}

fn render_xlsx_xf(xf: &XlsxXfEntry) -> String {
    if xf.inner_xml.is_empty() {
        format!("<xf{}/>", render_xml_attrs(&xf.attrs))
    } else {
        format!("<xf{}>{}</xf>", render_xml_attrs(&xf.attrs), xf.inner_xml)
    }
}

fn xlsx_xf_num_fmt_id(attrs: &BTreeMap<String, String>) -> u32 {
    attrs
        .get("numFmtId")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0)
}

fn set_collection_count(xml: String, parent: &str, child: &str) -> String {
    let count = count_children_in_parent(&xml, parent, child);
    let Some(span) = element_span_by_local_name(&xml, parent) else {
        return xml;
    };
    set_start_tag_count_attr(&xml, span, count)
}

fn count_children_in_parent(xml: &str, parent: &str, child: &str) -> usize {
    let Some(span) = element_span_by_local_name(xml, parent) else {
        return 0;
    };
    let fragment = &xml[span.open_end..span.close_start];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut count = 0usize;
    let mut depth = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if depth == 0 && local_name(e.name().as_ref()) == child {
                    count += 1;
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if depth == 0 && local_name(e.name().as_ref()) == child {
                    count += 1;
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    count
}

fn set_start_tag_count_attr(xml: &str, span: XmlElementSpan, count: usize) -> String {
    let open = &xml[span.start..span.open_end];
    let replacement = if let Some(pos) = open.find("count=\"") {
        let value_start = pos + "count=\"".len();
        if let Some(value_end_rel) = open[value_start..].find('"') {
            let value_end = value_start + value_end_rel;
            let mut tag = String::new();
            tag.push_str(&open[..value_start]);
            tag.push_str(&count.to_string());
            tag.push_str(&open[value_end..]);
            tag
        } else {
            open.to_string()
        }
    } else if let Some(pos) = open.rfind("/>") {
        format!("{} count=\"{}\"/>", &open[..pos].trim_end(), count)
    } else if let Some(pos) = open.rfind('>') {
        format!("{} count=\"{}\">", &open[..pos].trim_end(), count)
    } else {
        open.to_string()
    };
    let mut out = String::with_capacity(xml.len() + replacement.len());
    out.push_str(&xml[..span.start]);
    out.push_str(&replacement);
    out.push_str(&xml[span.open_end..]);
    out
}

fn set_xlsx_range_number_format_xml(
    sheet_xml: &str,
    mut styles_xml: String,
    bounds: RangeBounds,
    number_format_id: u32,
) -> CliResult<(String, String, XlsxRangeFormatStats)> {
    let sheet_data = xlsx_sheet_data_span(sheet_xml)?;
    let row_spans = parse_xlsx_row_spans(sheet_xml, sheet_data.as_ref())?;
    let mut stats = XlsxRangeFormatStats::default();
    let mut changed_rows = BTreeMap::<u32, String>::new();
    let mut style_by_base = BTreeMap::<u32, u32>::new();
    let write_bounds = bounds.normalized();
    for row_num in write_bounds.start_row..=write_bounds.end_row {
        let existing_row = row_spans.get(&row_num);
        let mut rendered_cells = existing_row
            .map(|span| {
                span.cells
                    .iter()
                    .map(|(col, cell)| (*col, cell.xml.clone()))
                    .collect::<BTreeMap<u32, String>>()
            })
            .unwrap_or_default();
        let mut row_changed = false;
        for col_num in write_bounds.start_col..=write_bounds.end_col {
            let addr = format!("{}{}", col_name(col_num), row_num);
            let existing_cell = existing_row.and_then(|span| span.cells.get(&col_num));
            let base_style = existing_cell
                .and_then(|cell| cell.attrs.get("s"))
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0);
            let style_index = if let Some(style_index) = style_by_base.get(&base_style).copied() {
                style_index
            } else {
                let (new_styles_xml, style_index, created) =
                    ensure_xlsx_cell_style(styles_xml, base_style, number_format_id)?;
                styles_xml = new_styles_xml;
                if created {
                    stats.created_styles += 1;
                }
                style_by_base.insert(base_style, style_index);
                style_index
            };
            let cell_xml = if let Some(existing_cell) = existing_cell {
                render_xlsx_existing_cell_with_style(&addr, existing_cell, style_index)
            } else {
                let mut attrs = BTreeMap::new();
                attrs.insert("r".to_string(), addr.clone());
                attrs.insert("s".to_string(), style_index.to_string());
                stats.created += 1;
                render_empty_xlsx_cell_with_attrs(&addr, Some(&attrs))
            };
            rendered_cells.insert(col_num, cell_xml);
            stats.updated += 1;
            stats.style_indexes.insert(style_index);
            row_changed = true;
        }
        if row_changed {
            changed_rows.insert(
                row_num,
                render_xlsx_row(row_num, existing_row, rendered_cells),
            );
        }
    }
    let updated =
        rebuild_xlsx_sheet_data(sheet_xml, sheet_data.as_ref(), &row_spans, &changed_rows)?;
    let used_range = xlsx_used_range_from_cell_refs(&updated);
    Ok((
        replace_xlsx_dimension(&updated, used_range.as_deref()),
        styles_xml,
        stats,
    ))
}

fn render_xlsx_existing_cell_with_style(
    addr: &str,
    cell: &XlsxCellSpan,
    style_index: u32,
) -> String {
    let mut attrs = cell.attrs.clone();
    attrs.insert("r".to_string(), addr.to_string());
    attrs.insert("s".to_string(), style_index.to_string());
    if cell.xml.trim_end().ends_with("/>") {
        return render_empty_xlsx_cell_with_attrs(addr, Some(&attrs));
    }
    if let Some(open_end) = cell.xml.find('>') {
        let mut out = format!("<c{}>", render_xml_attrs(&attrs));
        out.push_str(&cell.xml[open_end + 1..]);
        out
    } else {
        render_empty_xlsx_cell_with_attrs(addr, Some(&attrs))
    }
}

fn chrono_like_counter() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn xlsx_ranges_set_temp_path(file: &str) -> String {
    let parent = Path::new(file)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent
        .join(format!(
            ".ooxml-rust-ranges-set-{}-{}.xlsx",
            std::process::id(),
            chrono_like_counter()
        ))
        .to_string_lossy()
        .to_string()
}

fn resolve_xlsx_ranges_set_values(
    values: Option<&str>,
    values_file: Option<&str>,
) -> CliResult<String> {
    match (values, values_file) {
        (Some(_), Some(_)) | (None, None) => Err(CliError::invalid_args(
            "must specify exactly one of --values or --values-file",
        )),
        (Some(values), None) => Ok(values.to_string()),
        (None, Some("-")) => {
            let mut data = String::new();
            std::io::stdin()
                .read_to_string(&mut data)
                .map_err(|err| CliError::unexpected(format!("failed to read stdin: {err}")))?;
            Ok(data)
        }
        (None, Some(path)) => fs::read_to_string(path)
            .map_err(|_| CliError::file_not_found(format!("file not found: {path}"))),
    }
}

fn parse_xlsx_range_set_matrix(data: &str, data_format: &str) -> CliResult<XlsxRangeSetMatrix> {
    match data_format {
        "json" => parse_xlsx_range_set_json_matrix(data),
        "csv" => parse_xlsx_delimited_matrix(data, ','),
        "tsv" => parse_xlsx_delimited_matrix(data, '\t'),
        _ => Err(CliError::invalid_args(format!(
            "invalid data format {data_format:?} (must be json, csv, or tsv)",
        ))),
    }
}

fn parse_xlsx_range_set_json_matrix(data: &str) -> CliResult<XlsxRangeSetMatrix> {
    let raw: Value = serde_json::from_str(data)
        .map_err(|err| CliError::invalid_args(format!("invalid json values: {err}")))?;
    let (range, null_policy, major_dimension, values) = if let Some(object) = raw.as_object() {
        if object.contains_key("values") {
            (
                object
                    .get("range")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                object
                    .get("nullPolicy")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                object
                    .get("majorDimension")
                    .and_then(Value::as_str)
                    .unwrap_or("rows")
                    .to_string(),
                object
                    .get("values")
                    .cloned()
                    .ok_or_else(|| CliError::invalid_args("JSON object must contain values"))?,
            )
        } else {
            (None, None, "rows".to_string(), raw)
        }
    } else {
        (None, None, "rows".to_string(), raw)
    };
    let mut rows = parse_xlsx_matrix_rows(&values)?;
    let major_dimension = match major_dimension.trim().to_ascii_lowercase().as_str() {
        "" | "rows" => "rows".to_string(),
        "columns" => {
            rows = transpose_xlsx_matrix(rows)?;
            "columns".to_string()
        }
        _ => {
            return Err(CliError::invalid_args(
                "majorDimension must be rows or columns",
            ));
        }
    };
    Ok(XlsxRangeSetMatrix {
        range,
        null_policy,
        major_dimension,
        rows,
    })
}

fn parse_xlsx_delimited_matrix(data: &str, delimiter: char) -> CliResult<XlsxRangeSetMatrix> {
    let records = parse_delimited_records(data, delimiter)?;
    let rows = records
        .into_iter()
        .map(|record| {
            record
                .into_iter()
                .map(|value| XlsxMatrixCell {
                    kind: "string".to_string(),
                    value,
                    formula: String::new(),
                    null: false,
                })
                .collect()
        })
        .collect();
    Ok(XlsxRangeSetMatrix {
        range: None,
        null_policy: None,
        major_dimension: "rows".to_string(),
        rows,
    })
}

fn parse_delimited_records(data: &str, delimiter: char) -> CliResult<Vec<Vec<String>>> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut chars = data.chars().peekable();
    let mut in_quotes = false;
    let mut field_started = false;
    let mut just_closed_quote = false;

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                    just_closed_quote = true;
                }
            } else {
                field.push(ch);
            }
            continue;
        }

        if ch == '"' {
            if !field_started {
                in_quotes = true;
                field_started = true;
                continue;
            }
            return Err(CliError::invalid_args(
                "parse error on line 1, column 1: bare \" in non-quoted-field",
            ));
        }

        if ch == delimiter {
            record.push(std::mem::take(&mut field));
            field_started = false;
            just_closed_quote = false;
            continue;
        }

        if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            record.push(std::mem::take(&mut field));
            records.push(std::mem::take(&mut record));
            field_started = false;
            just_closed_quote = false;
            continue;
        }

        if just_closed_quote {
            return Err(CliError::invalid_args(
                "parse error on line 1, column 1: extraneous or missing \" in quoted-field",
            ));
        }
        field_started = true;
        field.push(ch);
    }

    if in_quotes {
        return Err(CliError::invalid_args(
            "parse error on line 1, column 1: extraneous or missing \" in quoted-field",
        ));
    }
    if field_started || !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    Ok(records)
}

fn parse_xlsx_matrix_rows(value: &Value) -> CliResult<Vec<Vec<XlsxMatrixCell>>> {
    let rows = value
        .as_array()
        .ok_or_else(|| CliError::invalid_args("values must be an array of arrays"))?;
    rows.iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let cells = row.as_array().ok_or_else(|| {
                CliError::invalid_args(format!("values[{row_idx}] must be an array"))
            })?;
            cells
                .iter()
                .enumerate()
                .map(|(col_idx, cell)| {
                    parse_xlsx_matrix_cell(cell).map_err(|err| {
                        CliError::invalid_args(format!(
                            "values[{row_idx}][{col_idx}]: {}",
                            err.message
                        ))
                    })
                })
                .collect()
        })
        .collect()
}

fn parse_xlsx_matrix_cell(value: &Value) -> CliResult<XlsxMatrixCell> {
    if value.is_null() {
        return Ok(XlsxMatrixCell {
            kind: "empty".to_string(),
            value: String::new(),
            formula: String::new(),
            null: true,
        });
    }
    if let Some(text) = value.as_str() {
        return Ok(XlsxMatrixCell {
            kind: "string".to_string(),
            value: text.to_string(),
            formula: String::new(),
            null: false,
        });
    }
    if let Some(number) = value.as_number() {
        return Ok(XlsxMatrixCell {
            kind: "number".to_string(),
            value: number.to_string(),
            formula: String::new(),
            null: false,
        });
    }
    if let Some(boolean) = value.as_bool() {
        return Ok(XlsxMatrixCell {
            kind: "boolean".to_string(),
            value: boolean.to_string(),
            formula: String::new(),
            null: false,
        });
    }
    let object = value
        .as_object()
        .ok_or_else(|| CliError::invalid_args("unsupported JSON cell type"))?;
    if let Some(formula) = object.get("formula") {
        let formula = formula
            .as_str()
            .ok_or_else(|| CliError::invalid_args("formula must be a string"))?;
        if formula.trim().is_empty() {
            return Err(CliError::invalid_args("formula cannot be empty"));
        }
        return Ok(XlsxMatrixCell {
            kind: "formula".to_string(),
            value: formula.to_string(),
            formula: formula.to_string(),
            null: false,
        });
    }
    let raw_value = object
        .get("value")
        .ok_or_else(|| CliError::invalid_args("object cell must contain value or formula"))?;
    let mut cell = parse_xlsx_matrix_cell(raw_value)?;
    if let Some(kind) = object.get("type").and_then(Value::as_str) {
        cell.kind = kind.trim().to_ascii_lowercase();
        if cell.kind == "formula" {
            cell.formula = cell.value.clone();
        }
    }
    Ok(cell)
}

fn transpose_xlsx_matrix(rows: Vec<Vec<XlsxMatrixCell>>) -> CliResult<Vec<Vec<XlsxMatrixCell>>> {
    if rows.is_empty() {
        return Ok(rows);
    }
    let cols = rows[0].len();
    if rows.iter().any(|row| row.len() != cols) {
        return Err(CliError::invalid_args(
            "ragged columns matrix cannot be transposed",
        ));
    }
    let mut out = vec![Vec::with_capacity(rows.len()); cols];
    for row in rows {
        for (col_idx, cell) in row.into_iter().enumerate() {
            out[col_idx].push(cell);
        }
    }
    Ok(out)
}

fn rectangularize_xlsx_matrix(rows: &mut Vec<Vec<XlsxMatrixCell>>, ragged: &str) -> CliResult<()> {
    if rows.is_empty() {
        return Err(CliError::invalid_args("values matrix cannot be empty"));
    }
    let cols = rows[0].len();
    let max_cols = rows.iter().map(Vec::len).max().unwrap_or(cols);
    if max_cols == 0 {
        return Err(CliError::invalid_args(
            "values matrix must contain at least one column",
        ));
    }
    match ragged.trim().to_ascii_lowercase().as_str() {
        "" | "reject" => {
            for (idx, row) in rows.iter().enumerate().skip(1) {
                if row.len() != cols {
                    return Err(CliError::invalid_args(format!(
                        "ragged matrix row {} has {} columns, want {}",
                        idx + 1,
                        row.len(),
                        cols
                    )));
                }
            }
        }
        "fill-empty" => {
            for row in rows {
                while row.len() < max_cols {
                    row.push(XlsxMatrixCell {
                        kind: "string".to_string(),
                        value: String::new(),
                        formula: String::new(),
                        null: false,
                    });
                }
            }
        }
        _ => {
            return Err(CliError::invalid_args(
                "invalid ragged mode (must be reject or fill-empty)",
            ));
        }
    }
    Ok(())
}

fn validate_xlsx_null_policy(policy: &str) -> CliResult<()> {
    match policy.trim().to_ascii_lowercase().as_str() {
        "skip" | "clear" | "empty-string" => Ok(()),
        _ => Err(CliError::invalid_args(
            "invalid null policy (must be skip, clear, or empty-string)",
        )),
    }
}

fn resolve_xlsx_ranges_set_bounds(
    range: Option<&str>,
    anchor: Option<&str>,
    input_range: Option<&str>,
    rows: usize,
    cols: usize,
) -> CliResult<RangeBounds> {
    let mut sources = 0;
    if range.is_some_and(|value| !value.trim().is_empty()) {
        sources += 1;
    }
    if anchor.is_some_and(|value| !value.trim().is_empty()) {
        sources += 1;
    }
    if input_range.is_some_and(|value| !value.trim().is_empty()) {
        sources += 1;
    }
    if sources != 1 {
        return Err(CliError::invalid_args(
            "must specify exactly one of --anchor, --range, or JSON input range",
        ));
    }
    if let Some(anchor) = anchor.filter(|value| !value.trim().is_empty()) {
        let (start_col, start_row) = parse_cell_ref(anchor)
            .map_err(|err| CliError::invalid_args(format!("invalid --anchor: {}", err.message)))?;
        let end_col = start_col + cols as u32 - 1;
        let end_row = start_row + rows as u32 - 1;
        return Ok(RangeBounds {
            start_col,
            start_row,
            end_col,
            end_row,
        });
    }
    let range_text = input_range
        .filter(|value| !value.trim().is_empty())
        .or(range)
        .unwrap_or_default();
    let bounds = parse_cli_range(range_text)?;
    let range_rows = bounds.row_count();
    let range_cols = bounds.col_count();
    if range_rows as usize != rows || range_cols as usize != cols {
        return Err(CliError::invalid_args(format!(
            "range {} is {}x{} but values matrix is {}x{}",
            range_text, range_rows, range_cols, rows, cols
        )));
    }
    Ok(bounds)
}

fn validate_xlsx_mutation_output_flags(
    out: Option<&str>,
    in_place: bool,
    backup: Option<&str>,
    dry_run: bool,
) -> CliResult<()> {
    let has_out = out.is_some_and(|value| !value.trim().is_empty());
    let has_backup = backup.is_some_and(|value| !value.trim().is_empty());
    if dry_run && (has_out || in_place) {
        return Err(CliError::invalid_args(
            "--dry-run cannot be combined with --out or --in-place",
        ));
    }
    if dry_run && has_backup {
        return Err(CliError::invalid_args(
            "--backup cannot be used with --dry-run",
        ));
    }
    if !dry_run && !has_out && !in_place {
        return Err(CliError::invalid_args(
            "must specify exactly one of --out, --in-place, or --dry-run",
        ));
    }
    if has_out && in_place {
        return Err(CliError::invalid_args(
            "cannot specify both --out and --in-place",
        ));
    }
    if has_backup && !in_place {
        return Err(CliError::invalid_args(
            "--backup can only be used with --in-place",
        ));
    }
    Ok(())
}

fn resolve_xlsx_sheet_context(
    file: &str,
    sheet_selector: &str,
) -> CliResult<(WorkbookSheet, String)> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    Ok((sheet, sheet_part))
}

fn set_xlsx_range_in_sheet_xml(
    xml: &str,
    bounds: RangeBounds,
    rows: &[Vec<XlsxMatrixCell>],
    null_policy: &str,
    overwrite_formulas: bool,
) -> CliResult<(String, XlsxRangeSetStats)> {
    reject_xlsx_merged_cell_intersection(xml, bounds)?;
    let sheet_data = xlsx_sheet_data_span(xml)?;
    let row_spans = parse_xlsx_row_spans(xml, sheet_data.as_ref())?;

    let mut stats = XlsxRangeSetStats::default();
    let mut changed_rows = BTreeMap::<u32, String>::new();
    let write_bounds = bounds.normalized();
    for (row_offset, row) in rows.iter().enumerate() {
        let row_number = write_bounds.start_row + row_offset as u32;
        let existing_row = row_spans.get(&row_number);
        let mut rendered_cells = existing_row
            .map(|span| {
                span.cells
                    .iter()
                    .map(|(col, cell)| (*col, cell.xml.clone()))
                    .collect::<BTreeMap<u32, String>>()
            })
            .unwrap_or_default();
        let mut row_changed = false;
        for (col_offset, cell) in row.iter().enumerate() {
            let col_number = write_bounds.start_col + col_offset as u32;
            let addr = format!("{}{}", col_name(col_number), row_number);
            let existing_cell = existing_row.and_then(|span| span.cells.get(&col_number));
            if !overwrite_formulas
                && existing_cell.is_some_and(|span| span.has_formula)
                && xlsx_range_cell_touches_existing(cell, null_policy)
            {
                return Err(CliError::invalid_args(format!(
                    "range write would overwrite existing formula: {addr}"
                )));
            }
            if cell.null {
                match null_policy.trim().to_ascii_lowercase().as_str() {
                    "skip" => {
                        stats.skipped += 1;
                    }
                    "clear" => {
                        if let Some(existing_cell) = existing_cell {
                            stats.cleared += 1;
                            row_changed = true;
                            if existing_cell
                                .attrs
                                .get("s")
                                .is_some_and(|value| !value.is_empty())
                            {
                                rendered_cells.insert(
                                    col_number,
                                    render_empty_xlsx_cell_with_attrs(
                                        &addr,
                                        Some(&existing_cell.attrs),
                                    ),
                                );
                            } else {
                                rendered_cells.remove(&col_number);
                            }
                        } else {
                            rendered_cells.remove(&col_number);
                        }
                    }
                    "empty-string" => {
                        let empty = XlsxMatrixCell {
                            kind: "string".to_string(),
                            value: String::new(),
                            formula: String::new(),
                            null: false,
                        };
                        let (rendered, wrote_formula) = render_xlsx_cell_with_attrs(
                            &addr,
                            &empty,
                            existing_cell.map(|span| &span.attrs),
                        )?;
                        rendered_cells.insert(col_number, rendered);
                        row_changed = true;
                        stats.updated += 1;
                        if existing_cell.is_none() {
                            stats.created += 1;
                        }
                        if wrote_formula {
                            stats.formula_count += 1;
                        }
                    }
                    _ => unreachable!("null policy validated earlier"),
                }
                continue;
            }
            let (rendered, wrote_formula) =
                render_xlsx_cell_with_attrs(&addr, cell, existing_cell.map(|span| &span.attrs))?;
            rendered_cells.insert(col_number, rendered);
            row_changed = true;
            if existing_cell.is_none() {
                stats.created += 1;
            }
            if wrote_formula {
                stats.formula_count += 1;
            }
            stats.updated += 1;
        }
        if row_changed {
            changed_rows.insert(
                row_number,
                render_xlsx_row(row_number, existing_row, rendered_cells),
            );
        }
    }
    let updated = rebuild_xlsx_sheet_data(xml, sheet_data.as_ref(), &row_spans, &changed_rows)?;
    let used_range = xlsx_used_range_from_cell_refs(&updated);
    Ok((
        replace_xlsx_dimension(&updated, used_range.as_deref()),
        stats,
    ))
}

fn xlsx_range_cell_touches_existing(cell: &XlsxMatrixCell, null_policy: &str) -> bool {
    !(cell.null && null_policy.trim().eq_ignore_ascii_case("skip"))
}

fn render_xlsx_cell_with_attrs(
    addr: &str,
    cell: &XlsxMatrixCell,
    attrs: Option<&BTreeMap<String, String>>,
) -> CliResult<(String, bool)> {
    let mut attrs = attrs.cloned().unwrap_or_default();
    attrs.insert("r".to_string(), addr.to_string());
    attrs.remove("t");
    let (kind, value) = normalize_xlsx_write_cell(cell)?;
    let (content, wrote_formula) = match kind.as_str() {
        "string" => {
            attrs.insert("t".to_string(), "inlineStr".to_string());
            let space_attr = if needs_xml_space_preserve(&value) {
                " xml:space=\"preserve\""
            } else {
                ""
            };
            (
                format!("<is><t{space_attr}>{}</t></is>", xml_escape(&value)),
                false,
            )
        }
        "number" => (format!("<v>{}</v>", xml_escape(&value)), false),
        "bool" | "boolean" => {
            let value = match cell.value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => "1",
                _ => "0",
            };
            attrs.insert("t".to_string(), "b".to_string());
            (format!("<v>{value}</v>"), false)
        }
        "formula" => (format!("<f>{}</f>", xml_escape(&value)), true),
        _ => unreachable!("cell kind normalized earlier"),
    };
    Ok((
        format!("<c{}>{content}</c>", render_xml_attrs(&attrs)),
        wrote_formula,
    ))
}

fn normalize_xlsx_write_cell(cell: &XlsxMatrixCell) -> CliResult<(String, String)> {
    let kind = if !cell.formula.is_empty() {
        "formula".to_string()
    } else {
        cell.kind.trim().to_ascii_lowercase()
    };
    match kind.as_str() {
        "" | "string" => Ok(("string".to_string(), cell.value.clone())),
        "number" => {
            let literal = cell.value.trim();
            let parsed = literal.parse::<f64>().map_err(|_| {
                CliError::invalid_args(format!("invalid number value {:?}", cell.value))
            })?;
            if !parsed.is_finite() || literal.is_empty() {
                return Err(CliError::invalid_args(format!(
                    "invalid number value {:?}",
                    cell.value
                )));
            }
            Ok(("number".to_string(), literal.to_string()))
        }
        "bool" | "boolean" => match cell.value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(("bool".to_string(), "1".to_string())),
            "false" | "0" => Ok(("bool".to_string(), "0".to_string())),
            _ => Err(CliError::invalid_args(format!(
                "invalid bool value {:?}",
                cell.value
            ))),
        },
        "formula" => {
            let formula = if cell.formula.is_empty() {
                &cell.value
            } else {
                &cell.formula
            };
            let formula = formula.trim().trim_start_matches('=').to_string();
            if formula.is_empty() {
                return Err(CliError::invalid_args("formula value cannot be empty"));
            }
            Ok(("formula".to_string(), formula))
        }
        "auto" => {
            let trimmed = cell.value.trim();
            if trimmed.starts_with('=') {
                return normalize_xlsx_write_cell(&XlsxMatrixCell {
                    kind: "formula".to_string(),
                    value: trimmed.to_string(),
                    formula: trimmed.to_string(),
                    null: false,
                });
            }
            if matches!(trimmed.to_ascii_lowercase().as_str(), "true" | "false") {
                return normalize_xlsx_write_cell(&XlsxMatrixCell {
                    kind: "bool".to_string(),
                    value: trimmed.to_string(),
                    formula: String::new(),
                    null: false,
                });
            }
            if let Ok(parsed) = trimmed.parse::<f64>()
                && parsed.is_finite()
            {
                return normalize_xlsx_write_cell(&XlsxMatrixCell {
                    kind: "number".to_string(),
                    value: trimmed.to_string(),
                    formula: String::new(),
                    null: false,
                });
            }
            Ok(("string".to_string(), cell.value.clone()))
        }
        _ => Err(CliError::invalid_args(format!(
            "invalid cell value type {:?} (must be string, number, bool, formula, or auto)",
            cell.kind
        ))),
    }
}

fn render_empty_xlsx_cell_with_attrs(
    addr: &str,
    attrs: Option<&BTreeMap<String, String>>,
) -> String {
    let mut attrs = attrs.cloned().unwrap_or_default();
    attrs.insert("r".to_string(), addr.to_string());
    attrs.remove("t");
    format!("<c{}/>", render_xml_attrs(&attrs))
}

fn needs_xml_space_preserve(value: &str) -> bool {
    value != value.trim_matches([' ', '\t', '\r', '\n'])
}

fn replace_xlsx_dimension(xml: &str, range: Option<&str>) -> String {
    let dimension = range.map(|range| format!("<dimension ref=\"{range}\"/>"));
    if let Some(start) = xml.find("<dimension")
        && let Some(end) = xml[start..]
            .find("/>")
            .map(|offset| start + offset + "/>".len())
            .or_else(|| xml[start..].find('>').map(|offset| start + offset + 1))
    {
        let mut updated =
            String::with_capacity(xml.len() + dimension.as_ref().map_or(0, String::len));
        updated.push_str(&xml[..start]);
        if let Some(dimension) = dimension.as_deref() {
            updated.push_str(dimension);
        }
        updated.push_str(&xml[end..]);
        return updated;
    }
    if let Some(dimension) = dimension
        && let Some(sheet_data_start) = xml.find("<sheetData")
    {
        let mut updated = String::with_capacity(xml.len() + dimension.len());
        updated.push_str(&xml[..sheet_data_start]);
        updated.push_str(&dimension);
        updated.push_str(&xml[sheet_data_start..]);
        return updated;
    }
    xml.to_string()
}

#[derive(Clone)]
struct XlsxSheetDataSpan {
    start: usize,
    open_end: usize,
    close_start: usize,
    end: usize,
    empty: bool,
}

#[derive(Clone)]
struct XlsxRowSpan {
    row: u32,
    start: usize,
    end: usize,
    attrs: BTreeMap<String, String>,
    cells: BTreeMap<u32, XlsxCellSpan>,
}

#[derive(Clone)]
struct XlsxCellSpan {
    xml: String,
    attrs: BTreeMap<String, String>,
    has_formula: bool,
}

fn xlsx_sheet_data_span(xml: &str) -> CliResult<Option<XlsxSheetDataSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                let open_end = reader.buffer_position() as usize;
                loop {
                    let inner_before = reader.buffer_position() as usize;
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                            return Ok(Some(XlsxSheetDataSpan {
                                start: before,
                                open_end,
                                close_start: inner_before,
                                end: reader.buffer_position() as usize,
                                empty: false,
                            }));
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("sheetData has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "sheetData" => {
                let end = reader.buffer_position() as usize;
                return Ok(Some(XlsxSheetDataSpan {
                    start: before,
                    open_end: end,
                    close_start: end,
                    end,
                    empty: true,
                }));
            }
            Ok(Event::Eof) => return Ok(None),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn parse_xlsx_row_spans(
    xml: &str,
    sheet_data: Option<&XlsxSheetDataSpan>,
) -> CliResult<BTreeMap<u32, XlsxRowSpan>> {
    let Some(sheet_data) = sheet_data.filter(|span| !span.empty) else {
        return Ok(BTreeMap::new());
    };
    let fragment = &xml[sheet_data.open_end..sheet_data.close_start];
    let base = sheet_data.open_end;
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut rows = BTreeMap::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "row" => {
                let Some(row) = attr(&e, "r").and_then(|value| value.parse::<u32>().ok()) else {
                    continue;
                };
                let attrs = xml_attrs(&e);
                loop {
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "row" => {
                            let start = base + before;
                            let end = base + reader.buffer_position() as usize;
                            let row_xml = &xml[start..end];
                            rows.insert(
                                row,
                                XlsxRowSpan {
                                    row,
                                    start,
                                    end,
                                    attrs,
                                    cells: parse_xlsx_cell_spans(row_xml, start)?,
                                },
                            );
                            break;
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("row has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "row" => {
                if let Some(row) = attr(&e, "r").and_then(|value| value.parse::<u32>().ok()) {
                    let start = base + before;
                    let end = base + reader.buffer_position() as usize;
                    rows.insert(
                        row,
                        XlsxRowSpan {
                            row,
                            start,
                            end,
                            attrs: xml_attrs(&e),
                            cells: BTreeMap::new(),
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(rows)
}

fn parse_xlsx_cell_spans(row_xml: &str, base: usize) -> CliResult<BTreeMap<u32, XlsxCellSpan>> {
    let mut reader = Reader::from_str(row_xml);
    reader.config_mut().trim_text(false);
    let mut cells = BTreeMap::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "c" => {
                let Some(addr) = attr(&e, "r") else {
                    continue;
                };
                let (col, _) = parse_cell_ref(&addr)?;
                let attrs = xml_attrs(&e);
                loop {
                    match reader.read_event() {
                        Ok(Event::End(e)) if local_name(e.name().as_ref()) == "c" => {
                            let end = reader.buffer_position() as usize;
                            let xml = row_xml[before..end].to_string();
                            cells.insert(
                                col,
                                XlsxCellSpan {
                                    has_formula: xlsx_cell_xml_has_formula(&xml),
                                    xml,
                                    attrs,
                                },
                            );
                            break;
                        }
                        Ok(Event::Eof) => {
                            return Err(CliError::unexpected("cell has no closing tag"));
                        }
                        Err(err) => return Err(CliError::unexpected(err.to_string())),
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "c" => {
                if let Some(addr) = attr(&e, "r") {
                    let (col, _) = parse_cell_ref(&addr)?;
                    let end = reader.buffer_position() as usize;
                    let xml = row_xml[before..end].to_string();
                    cells.insert(
                        col,
                        XlsxCellSpan {
                            has_formula: false,
                            xml,
                            attrs: xml_attrs(&e),
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    let _ = base;
    Ok(cells)
}

fn xlsx_cell_xml_has_formula(cell_xml: &str) -> bool {
    let mut reader = Reader::from_str(cell_xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "f" => {
                return true;
            }
            Ok(Event::Eof) => return false,
            Err(_) => return false,
            _ => {}
        }
    }
}

fn render_xlsx_row(
    row_number: u32,
    row_span: Option<&XlsxRowSpan>,
    cells: BTreeMap<u32, String>,
) -> String {
    let mut attrs = row_span.map(|span| span.attrs.clone()).unwrap_or_default();
    attrs.insert("r".to_string(), row_number.to_string());
    attrs.remove("spans");
    let mut out = format!("<row{}>", render_xml_attrs(&attrs));
    for cell_xml in cells.into_values() {
        out.push_str(&cell_xml);
    }
    out.push_str("</row>");
    out
}

fn rebuild_xlsx_sheet_data(
    xml: &str,
    sheet_data: Option<&XlsxSheetDataSpan>,
    row_spans: &BTreeMap<u32, XlsxRowSpan>,
    changed_rows: &BTreeMap<u32, String>,
) -> CliResult<String> {
    if changed_rows.is_empty() {
        return Ok(xml.to_string());
    }
    let new_sheet_data = if let Some(sheet_data) = sheet_data.filter(|span| !span.empty) {
        let mut out = String::new();
        out.push_str(&xml[sheet_data.start..sheet_data.open_end]);
        let mut last = sheet_data.open_end;
        let mut emitted = BTreeSet::new();
        let mut rows_by_start = row_spans.values().collect::<Vec<_>>();
        rows_by_start.sort_by_key(|span| span.start);
        for row_span in rows_by_start {
            for (row, row_xml) in changed_rows.range(..row_span.row) {
                if !row_spans.contains_key(row) && emitted.insert(*row) {
                    out.push_str(row_xml);
                }
            }
            out.push_str(&xml[last..row_span.start]);
            if let Some(row_xml) = changed_rows.get(&row_span.row) {
                out.push_str(row_xml);
                emitted.insert(row_span.row);
            } else {
                out.push_str(&xml[row_span.start..row_span.end]);
            }
            last = row_span.end;
        }
        out.push_str(&xml[last..sheet_data.close_start]);
        for (row, row_xml) in changed_rows {
            if emitted.insert(*row) {
                out.push_str(row_xml);
            }
        }
        out.push_str(&xml[sheet_data.close_start..sheet_data.end]);
        out
    } else {
        let mut out = String::from("<sheetData>");
        for row_xml in changed_rows.values() {
            out.push_str(row_xml);
        }
        out.push_str("</sheetData>");
        out
    };
    if let Some(sheet_data) = sheet_data {
        let mut updated = String::with_capacity(xml.len() + new_sheet_data.len());
        updated.push_str(&xml[..sheet_data.start]);
        updated.push_str(&new_sheet_data);
        updated.push_str(&xml[sheet_data.end..]);
        return Ok(updated);
    }
    let insert_at = xml
        .find("</worksheet>")
        .ok_or_else(|| CliError::unexpected("worksheet has no closing tag"))?;
    let mut updated = String::with_capacity(xml.len() + new_sheet_data.len());
    updated.push_str(&xml[..insert_at]);
    updated.push_str(&new_sheet_data);
    updated.push_str(&xml[insert_at..]);
    Ok(updated)
}

fn reject_xlsx_merged_cell_intersection(xml: &str, bounds: RangeBounds) -> CliResult<()> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "mergeCell" =>
            {
                if let Some(merge_ref) = attr(&e, "ref") {
                    let merged = parse_range(&merge_ref)?;
                    if ranges_intersect(bounds, merged) {
                        return Err(CliError::invalid_args(format!(
                            "range write intersects merged cells: {} intersects {}",
                            range_bounds_ref(bounds),
                            range_bounds_ref(merged)
                        )));
                    }
                }
            }
            Ok(Event::Eof) => return Ok(()),
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
}

fn ranges_intersect(a: RangeBounds, b: RangeBounds) -> bool {
    a.min_col() <= b.max_col()
        && a.max_col() >= b.min_col()
        && a.min_row() <= b.max_row()
        && a.max_row() >= b.min_row()
}

fn range_bounds_ref(bounds: RangeBounds) -> String {
    let start = format!("{}{}", col_name(bounds.start_col), bounds.start_row);
    let end = format!("{}{}", col_name(bounds.end_col), bounds.end_row);
    if start == end {
        start
    } else {
        format!("{start}:{end}")
    }
}

fn xlsx_used_range_from_cell_refs(xml: &str) -> Option<String> {
    let mut min_row = u32::MAX;
    let mut max_row = 0;
    let mut min_col = u32::MAX;
    let mut max_col = 0;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "c" => {
                if let Some(addr) = attr(&e, "r")
                    && let Ok((col, row)) = parse_cell_ref(&addr)
                {
                    min_row = min_row.min(row);
                    max_row = max_row.max(row);
                    min_col = min_col.min(col);
                    max_col = max_col.max(col);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    if max_row == 0 {
        None
    } else {
        Some(format!(
            "{}{}:{}{}",
            col_name(min_col),
            min_row,
            col_name(max_col),
            max_row
        ))
    }
}

fn xml_attrs(e: &BytesStart<'_>) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    for attr in e.attributes().with_checks(false).flatten() {
        attrs.insert(
            local_name(attr.key.as_ref()).to_string(),
            decode_xml_text(attr.value.as_ref()),
        );
    }
    attrs
}

fn render_xml_attrs(attrs: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    for (key, value) in attrs {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        out.push_str(&xml_attr_escape(value));
        out.push('"');
    }
    out
}

fn xml_attr_escape(value: &str) -> String {
    xml_escape(value).replace('"', "&quot;")
}

fn xlsx_range_destination_json(
    readback_file: &str,
    destination_file: Option<&str>,
    sheet: &WorkbookSheet,
    sheet_part: &str,
    range: &str,
) -> CliResult<Value> {
    let exported = xlsx_range_export_with_options(
        readback_file,
        &sheet.name,
        range,
        XlsxRangeExportOptions {
            include_types: true,
            include_formulas: true,
            include_formats: true,
            data_out: None,
            max_cells: 0,
        },
    )?;
    let mut destination = Map::new();
    if let Some(file) = destination_file {
        destination.insert("file".to_string(), json!(file));
    }
    destination.insert("sheet".to_string(), json!(sheet.name));
    destination.insert("sheetNumber".to_string(), json!(sheet.position));
    destination.insert(
        "sheetPrimarySelector".to_string(),
        json!(format!("sheetId:{}", sheet.sheet_id)),
    );
    destination.insert(
        "sheetSelectors".to_string(),
        json!(xlsx_sheet_selectors(
            &sheet.name,
            sheet.sheet_id,
            sheet.position,
            &sheet.rel_id,
            &format!("/{sheet_part}")
        )),
    );
    for key in [
        "range",
        "rows",
        "cols",
        "values",
        "types",
        "formulas",
        "styleIndexes",
        "numberFormatIds",
        "numberFormatCodes",
        "formulaCount",
        "truncated",
    ] {
        if let Some(value) = exported.get(key) {
            destination.insert(key.to_string(), value.clone());
        }
    }
    Ok(Value::Object(destination))
}

fn add_xlsx_range_mutation_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    sheet_selector: &str,
    range: &str,
) {
    let target = output_path.unwrap_or("<out.xlsx>");
    let validate_key = if output_path.is_some() {
        "validateCommand"
    } else {
        "validateCommandTemplate"
    };
    let cells_key = if output_path.is_some() {
        "cellsExtractCommand"
    } else {
        "cellsExtractCommandTemplate"
    };
    let ranges_key = if output_path.is_some() {
        "rangesExportCommand"
    } else {
        "rangesExportCommandTemplate"
    };
    result.insert(
        validate_key.to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(target))),
    );
    result.insert(
        cells_key.to_string(),
        json!(format!(
            "ooxml --json xlsx cells extract {} --sheet {} --range {} --include-empty",
            command_arg(target),
            command_arg(sheet_selector),
            command_arg(range)
        )),
    );
    result.insert(
        ranges_key.to_string(),
        json!(format!(
            "ooxml --json xlsx ranges export {} --sheet {} --range {} --include-types --include-formulas --include-formats",
            command_arg(target),
            command_arg(sheet_selector),
            command_arg(range)
        )),
    );
}

fn xlsx_cells_extract(
    file: &str,
    sheet_selector: &str,
    range: Option<&str>,
    max_rows: u32,
    max_cells: u32,
    include_empty: bool,
) -> CliResult<Value> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let part_uri = format!("/{sheet_part}");
    let shared_strings = shared_strings(file).unwrap_or_default();
    let styles = xlsx_styles(file).unwrap_or_default();
    let sheet_xml = zip_text(file, &sheet_part)?;
    let dimension_declared = xlsx_dimension_declared(&sheet_xml);
    let merged_cell_count = xlsx_merged_cell_count(&sheet_xml);
    let all_cells = sheet_cells(&sheet_xml, &shared_strings, &styles);
    let range_bounds = range.map(parse_cli_range).transpose()?;
    let cells = sorted_xlsx_cells(&all_cells, range_bounds);
    let used_range = used_range_for_cells(&cells);
    let row_count = cells
        .iter()
        .map(|cell| cell.row)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let (rows, truncated) = if include_empty {
        build_dense_xlsx_rows(
            &cells,
            range_bounds,
            used_range,
            max_rows,
            max_cells,
            &sheet,
        )
    } else {
        build_sparse_xlsx_rows(&cells, max_rows, max_cells, &sheet)
    };

    let mut sheet_obj = Map::new();
    sheet_obj.insert("number".to_string(), json!(sheet.position));
    sheet_obj.insert("name".to_string(), json!(sheet.name));
    sheet_obj.insert("sheetId".to_string(), json!(sheet.sheet_id.to_string()));
    sheet_obj.insert("state".to_string(), json!("visible"));
    sheet_obj.insert("partUri".to_string(), json!(part_uri));
    sheet_obj.insert(
        "primarySelector".to_string(),
        json!(format!("sheetId:{}", sheet.sheet_id)),
    );
    sheet_obj.insert(
        "selectors".to_string(),
        json!(xlsx_sheet_selectors(
            &sheet.name,
            sheet.sheet_id,
            sheet.position,
            &sheet.rel_id,
            &format!("/{sheet_part}")
        )),
    );
    if let Some(dimension_declared) = dimension_declared.filter(|value| !value.is_empty()) {
        sheet_obj.insert("dimensionDeclared".to_string(), json!(dimension_declared));
    }
    sheet_obj.insert("usedRange".to_string(), used_range_json(used_range));
    sheet_obj.insert("rowCount".to_string(), json!(row_count));
    sheet_obj.insert("cellCount".to_string(), json!(cells.len()));
    sheet_obj.insert("mergedCellCount".to_string(), json!(merged_cell_count));
    if !rows.is_empty() {
        sheet_obj.insert("rows".to_string(), Value::Array(rows));
    }
    if truncated {
        sheet_obj.insert("truncated".to_string(), json!(true));
    }

    Ok(json!({
        "file": file,
        "sheet": Value::Object(sheet_obj),
    }))
}

fn xlsx_sheets_show(file: &str, sheet_selector: Option<&str>) -> CliResult<Value> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let selected = if let Some(selector) = sheet_selector.filter(|selector| !selector.is_empty()) {
        vec![resolve_sheet(&sheets, selector)?]
    } else {
        sheets
    };
    if selected.is_empty() {
        return Err(CliError::invalid_args("workbook has no worksheet sheets"));
    }
    let shared_strings = shared_strings(file).unwrap_or_default();
    let styles = xlsx_styles(file).unwrap_or_default();
    let mut reports = Vec::new();
    for sheet in selected {
        let target = rels.get(&sheet.rel_id).ok_or_else(|| {
            CliError::unexpected(format!("missing relationship {}", sheet.rel_id))
        })?;
        let sheet_part = normalize_xl_target(target);
        let sheet_xml = zip_text(file, &sheet_part)?;
        let cells = sorted_xlsx_cells(&sheet_cells(&sheet_xml, &shared_strings, &styles), None);
        reports.push(xlsx_sheet_show_item(
            file,
            &sheet,
            &sheet_part,
            &sheet_xml,
            &cells,
        ));
    }
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "sheets": reports,
    }))
}

fn xlsx_sheet_show_item(
    file: &str,
    sheet: &WorkbookSheet,
    sheet_part: &str,
    sheet_xml: &str,
    cells: &[XlsxCellEntry],
) -> Value {
    let part_uri = format!("/{sheet_part}");
    let used_range = used_range_for_cells(cells);
    let selector = format!("sheetId:{}", sheet.sheet_id);
    let used_range_ref = used_range_ref(used_range);
    let row_count = cells
        .iter()
        .map(|cell| cell.row)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let mut item = Map::new();
    item.insert("number".to_string(), json!(sheet.position));
    item.insert("name".to_string(), json!(sheet.name));
    item.insert("sheetId".to_string(), json!(sheet.sheet_id.to_string()));
    item.insert("state".to_string(), json!(sheet.state));
    item.insert("partUri".to_string(), json!(part_uri));
    item.insert("primarySelector".to_string(), json!(selector));
    item.insert(
        "selectors".to_string(),
        json!(xlsx_sheet_selectors(
            &sheet.name,
            sheet.sheet_id,
            sheet.position,
            &sheet.rel_id,
            &format!("/{sheet_part}")
        )),
    );
    if let Some(dimension_declared) =
        xlsx_dimension_declared(sheet_xml).filter(|value| !value.is_empty())
    {
        item.insert("dimensionDeclared".to_string(), json!(dimension_declared));
    }
    item.insert("usedRange".to_string(), used_range_json(used_range));
    item.insert("rowCount".to_string(), json!(row_count));
    item.insert("cellCount".to_string(), json!(cells.len()));
    item.insert(
        "mergedCellCount".to_string(),
        json!(xlsx_merged_cell_count(sheet_xml)),
    );
    item.insert(
        "tablesListCommand".to_string(),
        json!(format!(
            "ooxml --json xlsx tables list {} --sheet {}",
            command_arg(file),
            command_arg(&selector)
        )),
    );
    item.insert(
        "setCellCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json xlsx cells set {} --sheet {} --cell A1 --value VALUE --out out.xlsx",
            command_arg(file),
            command_arg(&selector)
        )),
    );
    if let Some(range_ref) = used_range_ref {
        item.insert(
            "cellsExtractCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx cells extract {} --sheet {} --range {}",
                command_arg(file),
                command_arg(&selector),
                command_arg(&range_ref)
            )),
        );
        item.insert(
            "rangesExportCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx ranges export {} --sheet {} --range {} --include-types",
                command_arg(file),
                command_arg(&selector),
                command_arg(&range_ref)
            )),
        );
        item.insert(
            "setRangeCommandTemplate".to_string(),
            json!(format!(
                "ooxml --json xlsx ranges set {} --sheet {} --range {} --data-format json --values-file values.json --out out.xlsx",
                command_arg(file),
                command_arg(&selector),
                command_arg(&range_ref)
            )),
        );
    }
    Value::Object(item)
}

fn xlsx_sheets_list(file: &str) -> CliResult<Value> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let values: Vec<Value> = sheets
        .iter()
        .map(|sheet| {
            let target = rels.get(&sheet.rel_id).cloned().unwrap_or_default();
            let part = normalize_xl_target(&target);
            let part_uri = format!("/{part}");
            let primary_selector = format!("sheetId:{}", sheet.sheet_id);
            json!({
                "number": sheet.position,
                "position": sheet.position,
                "name": sheet.name,
                "sheetId": sheet.sheet_id.to_string(),
                "state": sheet.state,
                "relationshipId": sheet.rel_id,
                "partUri": part_uri,
                "relationshipType": "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet",
                "primarySelector": primary_selector,
                "selectors": xlsx_sheet_selectors(&sheet.name, sheet.sheet_id, sheet.position, &sheet.rel_id, &part_uri),
                "handle": format!("H:xlsx/ws:{}", sheet.sheet_id),
                "showCommand": format!("ooxml --json xlsx sheets show {} --sheet {}", command_arg(file), command_arg(&primary_selector)),
                "tablesListCommand": format!("ooxml --json xlsx tables list {} --sheet {}", command_arg(file), command_arg(&primary_selector)),
            })
        })
        .collect();
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "sheets": values,
    }))
}

fn xlsx_tables_list(file: &str, sheet_selector: Option<&str>) -> CliResult<Value> {
    let tables = xlsx_tables(file, sheet_selector)?;
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "tables": tables.iter().map(|table| xlsx_table_item_json(file, table)).collect::<Vec<_>>(),
    }))
}

fn xlsx_tables_show(
    file: &str,
    sheet_selector: Option<&str>,
    table_selector: Option<&str>,
) -> CliResult<Value> {
    let tables = xlsx_tables(file, sheet_selector)?;
    let table = select_xlsx_table(&tables, table_selector.unwrap_or_default())?;
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {}", command_arg(file)),
        "tables": [xlsx_table_item_json(file, &table)],
    }))
}

struct XlsxTableExportOptions<'a> {
    data_format: Option<&'a str>,
    data_out: Option<&'a str>,
    max_cells: i64,
    include_types: bool,
    include_formulas: bool,
}

fn xlsx_tables_export(
    file: &str,
    sheet_selector: Option<&str>,
    table_selector: Option<&str>,
    options: XlsxTableExportOptions<'_>,
) -> CliResult<Value> {
    require_json_data_format(options.data_format)?;
    let tables = xlsx_tables(file, sheet_selector)?;
    let table = select_xlsx_table(&tables, table_selector.unwrap_or_default())?;
    xlsx_range_export_with_options(
        file,
        &table.sheet,
        &table.range,
        XlsxRangeExportOptions {
            include_types: options.include_types,
            include_formulas: options.include_formulas,
            include_formats: false,
            data_out: options.data_out,
            max_cells: options.max_cells,
        },
    )
}

fn xlsx_tables(file: &str, sheet_selector: Option<&str>) -> CliResult<Vec<XlsxTableRef>> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let workbook_rels = relationship_entries(file, "xl/_rels/workbook.xml.rels")?;
    let selected = if let Some(selector) = sheet_selector.filter(|selector| !selector.is_empty()) {
        vec![resolve_sheet(&sheets, selector)?]
    } else {
        sheets
    };
    let mut tables = Vec::new();
    for sheet in selected {
        let Some(sheet_rel) = workbook_rels.iter().find(|rel| rel.id == sheet.rel_id) else {
            return Err(CliError::unexpected(format!(
                "missing relationship {}",
                sheet.rel_id
            )));
        };
        let sheet_part = normalize_xl_target(&sheet_rel.target);
        if !sheet_rel.rel_type.ends_with("/worksheet") {
            continue;
        }
        let sheet_xml = zip_text(file, &sheet_part)?;
        let table_relationship_ids = xlsx_table_relationship_ids(&sheet_xml)?;
        if table_relationship_ids.is_empty() {
            continue;
        }
        let sheet_rels = relationship_entries(file, &relationships_part_for(&sheet_part))?;
        for relationship_id in table_relationship_ids {
            let Some(table_rel) = sheet_rels.iter().find(|rel| rel.id == relationship_id) else {
                return Err(CliError::unexpected(format!(
                    "worksheet /{sheet_part} table relationship {relationship_id} not found"
                )));
            };
            if table_rel.target_mode == "External" {
                return Err(CliError::unexpected(format!(
                    "worksheet /{sheet_part} table relationship {relationship_id} is external"
                )));
            }
            if !table_rel.rel_type.ends_with("/table") {
                return Err(CliError::unexpected(format!(
                    "worksheet /{sheet_part} relationship {relationship_id} is {}, expected table",
                    table_rel.rel_type
                )));
            }
            let table_part =
                resolve_relationship_target(&format!("/{sheet_part}"), &table_rel.target);
            let table_part = table_part.trim_start_matches('/').to_string();
            let table_xml = zip_text(file, &table_part)?;
            let mut table = parse_xlsx_table_part(&table_xml, &format!("/{table_part}"))?;
            table.number = tables.len() as u32 + 1;
            table.sheet = sheet.name.clone();
            table.sheet_number = sheet.position;
            table.sheet_part_uri = format!("/{sheet_part}");
            table.relationship_id = relationship_id;
            table.part_uri = format!("/{table_part}");
            table.apply_selectors();
            tables.push(table);
        }
    }
    Ok(tables)
}

fn xlsx_table_relationship_ids(sheet_xml: &str) -> CliResult<Vec<String>> {
    let mut reader = Reader::from_str(sheet_xml);
    reader.config_mut().trim_text(true);
    let mut in_table_parts = false;
    let mut ids = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "tableParts" => {
                in_table_parts = true;
            }
            Ok(Event::Empty(e))
                if in_table_parts && local_name(e.name().as_ref()) == "tablePart" =>
            {
                if let Some(id) = attr_exact(&e, "r:id") {
                    ids.push(id);
                }
            }
            Ok(Event::Start(e))
                if in_table_parts && local_name(e.name().as_ref()) == "tablePart" =>
            {
                if let Some(id) = attr_exact(&e, "r:id") {
                    ids.push(id);
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "tableParts" => {
                in_table_parts = false;
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(ids)
}

fn parse_xlsx_table_part(xml: &str, part_uri: &str) -> CliResult<XlsxTableRef> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut table = XlsxTableRef::default();
    let mut saw_table = false;
    let mut in_table_columns = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "table" =>
            {
                saw_table = true;
                table.id = parse_optional_u32(attr(&e, "id").as_deref(), 0);
                table.name = attr(&e, "name").unwrap_or_default();
                table.display_name = attr(&e, "displayName").unwrap_or_else(|| table.name.clone());
                table.range = attr(&e, "ref").unwrap_or_default();
                let bounds = parse_range(&table.range).map_err(|err| {
                    CliError::unexpected(format!(
                        "invalid table ref {:?} in {part_uri}: {}",
                        table.range, err.message
                    ))
                })?;
                table.rows = bounds.row_count();
                table.cols = bounds.col_count();
                table.header_row_count =
                    parse_optional_u32(attr(&e, "headerRowCount").as_deref(), 1);
                table.totals_row_count =
                    parse_optional_u32(attr(&e, "totalsRowCount").as_deref(), 0);
                table.data_row_count = table
                    .rows
                    .saturating_sub(table.header_row_count)
                    .saturating_sub(table.totals_row_count);
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "tableColumns" => {
                in_table_columns = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "tableColumns" => {
                in_table_columns = false;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_table_columns && local_name(e.name().as_ref()) == "tableColumn" =>
            {
                table.columns.push(XlsxTableColumn {
                    id: parse_optional_u32(attr(&e, "id").as_deref(), 0),
                    name: attr(&e, "name").unwrap_or_default(),
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "tableStyleInfo" =>
            {
                table.style_name = attr(&e, "name").unwrap_or_default();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !saw_table {
        return Err(CliError::unexpected(format!(
            "table part {part_uri} root element not found"
        )));
    }
    if table.display_name.is_empty() {
        table.display_name = table.name.clone();
    }
    table.part_uri = part_uri.to_string();
    table.apply_selectors();
    Ok(table)
}

fn xlsx_table_item_json(file: &str, table: &XlsxTableRef) -> Value {
    let mut object = table.to_json_object();
    let table_selector = xlsx_table_selector(table);
    let sheet_selector = xlsx_table_sheet_selector(table);
    object.insert(
        "showCommand".to_string(),
        json!(xlsx_table_show_command(
            file,
            &sheet_selector,
            &table_selector
        )),
    );
    object.insert(
        "exportCommand".to_string(),
        json!(xlsx_table_export_command(
            file,
            &sheet_selector,
            &table_selector
        )),
    );
    object.insert(
        "appendRowsCommandTemplate".to_string(),
        json!(xlsx_table_append_rows_command_template(
            file,
            &sheet_selector,
            &table_selector
        )),
    );
    object.insert(
        "appendRecordsCommandTemplate".to_string(),
        json!(xlsx_table_append_records_command_template(
            file,
            &sheet_selector,
            &table_selector,
            &table.range
        )),
    );
    object.insert(
        "pptxUpdateTableCommandTemplate".to_string(),
        json!(xlsx_pptx_update_table_from_table_template(
            file,
            &sheet_selector,
            &table_selector,
            &table.range
        )),
    );
    object.insert(
        "pptxPlaceTableCommandTemplate".to_string(),
        json!(xlsx_pptx_place_table_from_table_template(
            file,
            &sheet_selector,
            &table_selector,
            &table.range
        )),
    );
    if !table.sheet.is_empty() && !table.range.is_empty() {
        object.insert(
            "pptxReplaceTextCommandTemplate".to_string(),
            json!(xlsx_pptx_replace_text_from_range_template(
                file,
                &table.sheet,
                &table.range
            )),
        );
    }
    Value::Object(object)
}

fn select_xlsx_table(tables: &[XlsxTableRef], selector: &str) -> CliResult<XlsxTableRef> {
    if tables.is_empty() {
        return Err(CliError::invalid_args("workbook has no tables"));
    }
    let selector = selector.trim();
    if selector.is_empty() {
        if tables.len() == 1 {
            return Ok(tables[0].clone());
        }
        return Err(CliError::invalid_args(
            "--table is required when workbook has multiple tables",
        ));
    }
    for table in tables {
        if table
            .selectors
            .iter()
            .any(|candidate| candidate == selector)
        {
            return Ok(table.clone());
        }
    }
    if let Ok(number) = selector.parse::<u32>() {
        if number >= 1 && (number as usize) <= tables.len() {
            return Ok(tables[number as usize - 1].clone());
        }
        return Err(CliError::target_not_found(format!(
            "table {number} is out of range (1-{})",
            tables.len()
        )));
    }
    let candidates = selector_candidates(
        &tables
            .iter()
            .map(|table| (table.primary_selector.as_str(), table.selectors.as_slice()))
            .collect::<Vec<_>>(),
        selector,
        3,
    );
    let mut message = format!("table not found: {selector}");
    if !candidates.is_empty() {
        message.push_str(&format!("; did you mean: {}", candidates.join(", ")));
    }
    message.push_str("; discover with `ooxml --json xlsx tables list <file>`");
    Err(CliError::target_not_found(message))
}

fn xlsx_table_selector(table: &XlsxTableRef) -> String {
    if !table.primary_selector.is_empty() {
        table.primary_selector.clone()
    } else if !table.display_name.is_empty() {
        table.display_name.clone()
    } else if table.number > 0 {
        format!("table:{}", table.number)
    } else {
        "1".to_string()
    }
}

fn xlsx_table_sheet_selector(table: &XlsxTableRef) -> String {
    if !table.sheet.is_empty() {
        table.sheet.clone()
    } else if table.sheet_number > 0 {
        format!("sheet:{}", table.sheet_number)
    } else {
        String::new()
    }
}

fn xlsx_table_show_command(file: &str, sheet_selector: &str, table_selector: &str) -> String {
    xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "tables", "show", file],
        &[("--sheet", sheet_selector), ("--table", table_selector)],
    )
}

fn xlsx_table_export_command(file: &str, sheet_selector: &str, table_selector: &str) -> String {
    let mut command = xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "tables", "export", file],
        &[("--sheet", sheet_selector), ("--table", table_selector)],
    );
    command.push_str(" --include-types");
    command
}

fn xlsx_table_append_rows_command_template(
    file: &str,
    sheet_selector: &str,
    table_selector: &str,
) -> String {
    let mut command = xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "tables", "append-rows", file],
        &[("--sheet", sheet_selector), ("--table", table_selector)],
    );
    command.push_str(" --values-file rows.json --out out.xlsx");
    command
}

fn xlsx_table_append_records_command_template(
    file: &str,
    sheet_selector: &str,
    table_selector: &str,
    expect_range: &str,
) -> String {
    let mut command = xlsx_source_command(
        vec!["ooxml", "--json", "xlsx", "tables", "append-records", file],
        &[
            ("--sheet", sheet_selector),
            ("--table", table_selector),
            ("--expect-range", expect_range),
        ],
    );
    command.push_str(" --records-file records.json --out out.xlsx");
    command
}

fn xlsx_pptx_update_table_from_table_template(
    file: &str,
    sheet_selector: &str,
    table_selector: &str,
    expect_range: &str,
) -> String {
    let mut command = xlsx_source_command(
        vec![
            "ooxml",
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            "deck.pptx",
            "--workbook",
            file,
        ],
        &[
            ("--sheet", sheet_selector),
            ("--table", table_selector),
            ("--expect-source-range", expect_range),
        ],
    );
    command.push_str(" --slide 1 --target table:1 --out out.pptx");
    command
}

fn xlsx_pptx_place_table_from_table_template(
    file: &str,
    sheet_selector: &str,
    table_selector: &str,
    expect_range: &str,
) -> String {
    let mut command = xlsx_source_command(
        vec![
            "ooxml",
            "--json",
            "pptx",
            "place",
            "table-from-xlsx",
            "deck.pptx",
            "--workbook",
            file,
        ],
        &[
            ("--sheet", sheet_selector),
            ("--table", table_selector),
            ("--expect-source-range", expect_range),
        ],
    );
    command.push_str(" --slide 1 --x 0 --y 0 --cx 4000000 --out out.pptx");
    command
}

fn xlsx_pptx_replace_text_from_range_template(
    file: &str,
    sheet_selector: &str,
    range: &str,
) -> String {
    let mut command = xlsx_source_command(
        vec![
            "ooxml",
            "--json",
            "pptx",
            "replace",
            "text-from-xlsx",
            "deck.pptx",
            "--workbook",
            file,
        ],
        &[("--sheet", sheet_selector), ("--range", range)],
    );
    command.push_str(" --slide 1 --target title --out out.pptx");
    command
}

fn xlsx_source_command(args: Vec<&str>, flags: &[(&str, &str)]) -> String {
    let mut args = args.into_iter().map(command_arg).collect::<Vec<_>>();
    for (name, value) in flags {
        if !value.trim().is_empty() {
            args.push((*name).to_string());
            args.push(command_arg(value));
        }
    }
    args.join(" ")
}

fn command_arg(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let needs_quotes = value.chars().any(|ch| {
        matches!(
            ch,
            ' ' | '\t'
                | '\r'
                | '\n'
                | '\''
                | '"'
                | '\\'
                | '$'
                | '`'
                | '<'
                | '>'
                | '|'
                | '&'
                | ';'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '*'
                | '?'
                | '!'
        )
    });
    if !needs_quotes {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn parse_optional_u32(value: Option<&str>, fallback: u32) -> u32 {
    value
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(fallback)
}

fn selector_candidates(
    items: &[(&str, &[String])],
    selector: &str,
    max_count: usize,
) -> Vec<String> {
    let needle = selector.trim().to_ascii_lowercase();
    let mut seen = Vec::<String>::new();
    if !needle.is_empty() {
        for (primary, selectors) in items {
            let matched = primary.to_ascii_lowercase().contains(&needle)
                || selectors
                    .iter()
                    .any(|selector| selector.to_ascii_lowercase().contains(&needle));
            if matched && push_selector_candidate(&mut seen, primary, max_count) {
                return seen;
            }
        }
    }
    if !seen.is_empty() {
        return seen;
    }
    for (primary, _) in items {
        if push_selector_candidate(&mut seen, primary, max_count) {
            break;
        }
    }
    seen
}

fn push_selector_candidate(seen: &mut Vec<String>, primary: &str, max_count: usize) -> bool {
    let primary = primary.trim();
    if primary.is_empty() || seen.iter().any(|existing| existing == primary) {
        return false;
    }
    seen.push(primary.to_string());
    seen.len() >= max_count
}

fn docx_text(file: &str) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "docx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }
    let xml = zip_text(file, "word/document.xml")?;
    let blocks = docx_blocks(&xml);
    Ok(json!({"blocks": blocks, "file": file}))
}

fn docx_blocks_show(file: &str, block: usize, include_runs: bool) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let xml = zip_text(file, &document_part)?;
    let reports = docx_rich_block_reports(&xml, include_runs).map_err(|err| {
        if err.message == "invalid DOCX XML"
            || err.message.starts_with("failed to extract DOCX blocks:")
        {
            CliError::unexpected(format!(
                "failed to extract DOCX blocks: failed to read document part {document_uri}: failed to parse XML part {document_uri}: etree: invalid XML format"
            ))
        } else {
            CliError::unexpected(format!("failed to extract DOCX blocks: {}", err.message))
        }
    })?;
    let blocks: Vec<Value> = if block > 0 {
        reports
            .into_iter()
            .filter(|report| report.index == block)
            .map(docx_rich_block_json)
            .collect()
    } else {
        reports.into_iter().map(docx_rich_block_json).collect()
    };
    if block > 0 && blocks.is_empty() {
        return Err(CliError::target_not_found(format!(
            "target not found: block {block}"
        )));
    }
    Ok(json!({
        "file": file,
        "documentPartUri": document_uri,
        "blocks": blocks,
    }))
}

fn docx_styles_list(file: &str, style_type: Option<&str>) -> CliResult<Value> {
    let style_type = normalize_docx_style_type(style_type)?;
    let (document_part, styles_part) = docx_document_and_styles_parts(file)?;
    let mut styles = Vec::new();
    if let Some(styles_part) = styles_part.as_deref() {
        styles = docx_styles(file, styles_part)?;
        if let Some(style_type) = style_type.as_deref() {
            styles.retain(|style| style.style_type == style_type);
        }
    }
    let counts = docx_style_id_counts(&styles);
    let styles_json: Vec<Value> = styles
        .iter()
        .map(|style| docx_style_json(style, &counts))
        .collect();
    Ok(json!({
        "file": file,
        "documentPartUri": document_part,
        "stylesPartUri": styles_part,
        "count": styles_json.len(),
        "styles": styles_json,
    }))
}

fn docx_styles_show(file: &str, style_id: &str) -> CliResult<Value> {
    let (document_part, styles_part) = docx_document_and_styles_parts(file)?;
    let mut style_json = Value::Null;
    let mut found = false;
    if let Some(styles_part) = styles_part.as_deref() {
        let styles = docx_styles(file, styles_part)?;
        let counts = docx_style_id_counts(&styles);
        if let Some(style) = styles.iter().find(|style| style.style_id == style_id) {
            style_json = docx_style_json(style, &counts);
            found = true;
        }
    }
    Ok(json!({
        "file": file,
        "documentPartUri": document_part,
        "stylesPartUri": styles_part,
        "styleId": style_id,
        "found": found,
        "style": style_json,
    }))
}

fn normalize_docx_style_type(value: Option<&str>) -> CliResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_lowercase();
    match normalized.as_str() {
        "paragraph" | "character" | "table" | "numbering" => Ok(Some(normalized)),
        _ => Err(CliError::invalid_args(
            "--type must be one of paragraph, character, table, numbering",
        )),
    }
}

fn docx_document_and_styles_parts(file: &str) -> CliResult<(String, Option<String>)> {
    let entries = zip_entry_names(file)?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        return Err(CliError::unsupported_type(
            "file is not a DOCX document (detected: unknown)",
        ));
    }
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let styles_uri = find_docx_styles_part(file, &entries, &document_part)?;
    Ok((document_uri, styles_uri))
}

fn find_docx_styles_part(
    file: &str,
    entries: &[String],
    document_part: &str,
) -> CliResult<Option<String>> {
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let rels_part = relationships_part_for(document_part);
    for rel in relationship_entries(file, &rels_part).unwrap_or_default() {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
            || rel.rel_type.ends_with("/styles")
        {
            return Ok(Some(resolve_relationship_target(
                &document_uri,
                &rel.target,
            )));
        }
    }
    for entry in entries {
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        let uri = format!("/{}", entry.trim_start_matches('/'));
        if is_docx_styles_part(&uri, &content_type) {
            return Ok(Some(uri));
        }
    }
    Ok(None)
}

#[derive(Clone, Default)]
struct DocxStyleInfo {
    style_id: String,
    name: String,
    style_type: String,
    default: bool,
    builtin: bool,
    based_on: String,
    next: String,
}

fn docx_styles(file: &str, styles_part: &str) -> CliResult<Vec<DocxStyleInfo>> {
    let xml = zip_text(file, styles_part.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut saw_root = false;
    let mut current: Option<DocxStyleInfo> = None;
    let mut styles = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "styles" {
                        return Err(CliError::unexpected(format!(
                            "styles part {styles_part} root is {name:?}, expected styles"
                        )));
                    }
                } else if name == "style" {
                    current = Some(docx_style_from_element(&e));
                } else {
                    docx_note_style_child(&e, &name, &mut current);
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "styles" {
                        return Err(CliError::unexpected(format!(
                            "styles part {styles_part} root is {name:?}, expected styles"
                        )));
                    }
                } else if name == "style" {
                    styles.push(docx_style_from_element(&e));
                } else {
                    docx_note_style_child(&e, &name, &mut current);
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "style" => {
                if let Some(style) = current.take() {
                    styles.push(style);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !saw_root {
        return Err(CliError::unexpected(format!(
            "styles part {styles_part} has no root element"
        )));
    }
    Ok(styles)
}

fn docx_style_from_element(element: &BytesStart<'_>) -> DocxStyleInfo {
    DocxStyleInfo {
        style_id: attr(element, "styleId").unwrap_or_default(),
        style_type: attr(element, "type").unwrap_or_default(),
        default: docx_on_off_attr(element, "default"),
        builtin: !docx_on_off_attr(element, "customStyle"),
        ..DocxStyleInfo::default()
    }
}

fn docx_note_style_child(
    element: &BytesStart<'_>,
    name: &str,
    current: &mut Option<DocxStyleInfo>,
) {
    let Some(style) = current.as_mut() else {
        return;
    };
    let Some(value) = attr(element, "val") else {
        return;
    };
    match name {
        "name" => style.name = value,
        "basedOn" => style.based_on = value,
        "next" => style.next = value,
        _ => {}
    }
}

fn docx_on_off_attr(element: &BytesStart<'_>, name: &str) -> bool {
    match attr(element, name).as_deref() {
        None => false,
        Some("0" | "false" | "off") => false,
        Some(_) => true,
    }
}

fn docx_style_id_counts(styles: &[DocxStyleInfo]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for style in styles {
        if !style.style_id.is_empty() {
            *counts.entry(style.style_id.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn docx_style_json(style: &DocxStyleInfo, counts: &BTreeMap<String, usize>) -> Value {
    let mut object = Map::new();
    object.insert("styleId".to_string(), json!(style.style_id));
    if !style.name.is_empty() {
        object.insert("name".to_string(), json!(style.name));
    }
    if !style.style_type.is_empty() {
        object.insert("type".to_string(), json!(style.style_type));
    }
    object.insert("default".to_string(), json!(style.default));
    object.insert("builtin".to_string(), json!(style.builtin));
    if !style.based_on.is_empty() {
        object.insert("basedOn".to_string(), json!(style.based_on));
    }
    if !style.next.is_empty() {
        object.insert("next".to_string(), json!(style.next));
    }
    if !style.style_id.is_empty() {
        object.insert("primarySelector".to_string(), json!(style.style_id));
        object.insert("selectors".to_string(), json!([style.style_id]));
        if counts.get(&style.style_id).copied().unwrap_or_default() == 1 {
            object.insert(
                "handle".to_string(),
                json!(format!("H:docx/pt:styles/style:n:{}", style.style_id)),
            );
        }
    }
    Value::Object(object)
}

fn docx_comments_list(file: &str, comment_id: Option<i64>) -> CliResult<Value> {
    let (document_part, comments_part) = docx_document_and_comments_parts(file)?;
    let mut comments = Vec::new();
    if let Some(comments_part) = comments_part.as_deref() {
        comments = docx_comments(file, comments_part, &document_part)?;
    }
    if let Some(comment_id) = comment_id {
        comments.retain(|comment| comment.id == comment_id);
        if comments.is_empty() {
            return Err(CliError::target_not_found(format!(
                "target not found: comment {comment_id}"
            )));
        }
    }
    let counts = docx_comment_id_counts(&comments);
    let comment_values = comments
        .iter()
        .map(|comment| docx_comment_json(comment, &counts))
        .collect::<Vec<_>>();
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("documentPartUri".to_string(), json!(document_part));
    if let Some(comments_part) = comments_part {
        result.insert("commentsPart".to_string(), json!(comments_part));
    }
    result.insert("comments".to_string(), Value::Array(comment_values));
    Ok(Value::Object(result))
}

fn docx_fields_list(file: &str, type_filter: Option<&str>) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let xml = zip_text(file, &document_part).map_err(|err| {
        CliError::unexpected(format!(
            "failed to list fields: failed to read document part {document_uri}: {}",
            err.message
        ))
    })?;
    let mut fields = docx_fields_in_document_xml(&xml, &document_uri)
        .map_err(|err| CliError::unexpected(format!("failed to list fields: {}", err.message)))?;

    for part_uri in docx_header_footer_part_uris(file, &document_part, &document_uri, &xml)? {
        let part_xml = match zip_text(file, part_uri.trim_start_matches('/')) {
            Ok(part_xml) => part_xml,
            Err(_) => continue,
        };
        fields.extend(
            docx_fields_in_header_footer_xml(&part_xml, &part_uri).map_err(|err| {
                CliError::unexpected(format!("failed to list fields: {}", err.message))
            })?,
        );
    }

    for (index, field) in fields.iter_mut().enumerate() {
        field.index = index;
    }
    if let Some(type_filter) = type_filter.filter(|value| !value.is_empty()) {
        let wanted = type_filter.to_ascii_uppercase();
        fields.retain(|field| docx_field_code_base(&field.instruction) == wanted);
    }
    let fields = fields.iter().map(docx_field_json).collect::<Vec<_>>();
    Ok(json!({
        "file": file,
        "documentPartUri": document_uri,
        "fields": fields,
    }))
}

#[derive(Clone, Default)]
struct DocxFieldInfo {
    index: usize,
    part_uri: String,
    block_index: usize,
    block_kind: String,
    field_type: String,
    instruction: String,
    cached_result: String,
    location: String,
    editable: bool,
}

#[derive(Default)]
struct DocxFieldParagraphState {
    part_uri: String,
    block_index: usize,
    block_kind: String,
    location: String,
    editable: bool,
    simple: Option<DocxSimpleFieldState>,
    complex: DocxComplexFieldState,
}

#[derive(Default)]
struct DocxSimpleFieldState {
    instruction: String,
    result: String,
    depth: usize,
    in_t: bool,
}

#[derive(Default)]
struct DocxComplexFieldState {
    in_field: bool,
    after_separator: bool,
    depth: usize,
    instruction: String,
    result: String,
    in_instruction_text: bool,
    in_result_text: bool,
}

fn docx_fields_in_document_xml(xml: &str, document_uri: &str) -> CliResult<Vec<DocxFieldInfo>> {
    let mut reader = NsReader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut fields = Vec::new();
    let mut current: Option<DocxFieldParagraphState> = None;
    let mut body_block_index = 0usize;
    let mut body_table_depth = 0usize;
    let mut current_table_block = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);

                if parent == Some("body") && is_word && name == "p" {
                    body_block_index += 1;
                    current = Some(docx_field_paragraph_state(
                        document_uri,
                        body_block_index,
                        "paragraph",
                        true,
                    ));
                } else if parent == Some("body") && is_word && name == "tbl" {
                    body_block_index += 1;
                    current_table_block = body_block_index;
                    body_table_depth = 1;
                } else if body_table_depth > 0 && is_word && name == "tbl" {
                    body_table_depth += 1;
                } else if body_table_depth > 0
                    && is_word
                    && name == "p"
                    && stack.iter().any(|item| item == "tc")
                {
                    current = Some(docx_field_paragraph_state(
                        document_uri,
                        current_table_block,
                        "table",
                        false,
                    ));
                }

                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_start(paragraph, &e, reader.resolver(), parent, is_word);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if parent == Some("body") && is_word && matches!(name.as_str(), "p" | "tbl") {
                    body_block_index += 1;
                } else if let Some(paragraph) = current.as_mut() {
                    docx_field_note_empty(
                        paragraph,
                        &e,
                        reader.resolver(),
                        parent,
                        is_word,
                        &mut fields,
                    );
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_end(paragraph, &name, &mut fields);
                }
                if name == "p" {
                    current = None;
                } else if name == "tbl" {
                    body_table_depth = body_table_depth.saturating_sub(1);
                    if body_table_depth == 0 {
                        current_table_block = 0;
                    }
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(fields)
}

fn docx_fields_in_header_footer_xml(xml: &str, part_uri: &str) -> CliResult<Vec<DocxFieldInfo>> {
    let mut reader = NsReader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut fields = Vec::new();
    let mut current: Option<DocxFieldParagraphState> = None;
    let mut paragraph_index = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    paragraph_index += 1;
                    current = Some(docx_field_paragraph_state(
                        part_uri,
                        paragraph_index,
                        "paragraph",
                        true,
                    ));
                }
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_start(paragraph, &e, reader.resolver(), parent, is_word);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    paragraph_index += 1;
                } else if let Some(paragraph) = current.as_mut() {
                    docx_field_note_empty(
                        paragraph,
                        &e,
                        reader.resolver(),
                        parent,
                        is_word,
                        &mut fields,
                    );
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_end(paragraph, &name, &mut fields);
                }
                if name == "p" {
                    current = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(fields)
}

fn docx_field_paragraph_state(
    part_uri: &str,
    block_index: usize,
    block_kind: &str,
    editable: bool,
) -> DocxFieldParagraphState {
    DocxFieldParagraphState {
        part_uri: part_uri.to_string(),
        block_index,
        block_kind: block_kind.to_string(),
        location: docx_field_location(part_uri, block_index),
        editable,
        ..DocxFieldParagraphState::default()
    }
}

fn docx_field_note_start(
    paragraph: &mut DocxFieldParagraphState,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    parent: Option<&str>,
    is_word: bool,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if !is_word {
        return;
    }

    if paragraph.simple.is_none() && parent == Some("p") && name == "fldSimple" {
        paragraph.simple = Some(DocxSimpleFieldState {
            instruction: docx_word_attr_ns(element, resolver, b"instr").unwrap_or_default(),
            depth: 1,
            ..DocxSimpleFieldState::default()
        });
        return;
    }

    if let Some(simple) = paragraph.simple.as_mut() {
        simple.depth += 1;
        if name == "t" && element_in_ns(resolver, element, DOCX_W_NS) {
            simple.in_t = true;
        }
        return;
    }

    docx_field_note_complex_start(&mut paragraph.complex, element, resolver);
}

fn docx_field_note_empty(
    paragraph: &mut DocxFieldParagraphState,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    parent: Option<&str>,
    is_word: bool,
    fields: &mut Vec<DocxFieldInfo>,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if !is_word {
        return;
    }
    if paragraph.simple.is_none() && parent == Some("p") && name == "fldSimple" {
        paragraph.simple = Some(DocxSimpleFieldState {
            instruction: docx_word_attr_ns(element, resolver, b"instr").unwrap_or_default(),
            depth: 1,
            ..DocxSimpleFieldState::default()
        });
        docx_emit_simple_field(paragraph, fields);
        return;
    }
    if paragraph.simple.is_some() {
        if name == "t" {
            // Empty w:t contributes no text but still belongs to the current simple field.
        }
        return;
    }
    let field_char_type = if name == "fldChar" {
        docx_word_attr_ns(element, resolver, b"fldCharType")
    } else {
        None
    };
    docx_field_note_complex_start(&mut paragraph.complex, element, resolver);
    if field_char_type.as_deref() == Some("end")
        && paragraph.complex.in_field
        && paragraph.complex.depth == 0
    {
        docx_emit_complex_field(paragraph, fields);
    }
}

fn docx_field_note_complex_start(
    complex: &mut DocxComplexFieldState,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if !element_in_ns(resolver, element, DOCX_W_NS) {
        return;
    }
    match name {
        "fldChar" => {
            let field_char_type =
                docx_word_attr_ns(element, resolver, b"fldCharType").unwrap_or_default();
            match field_char_type.as_str() {
                "begin" => {
                    if !complex.in_field {
                        complex.in_field = true;
                        complex.after_separator = false;
                        complex.depth = 1;
                        complex.instruction.clear();
                        complex.result.clear();
                    } else {
                        complex.depth += 1;
                    }
                }
                "separate" => {
                    if complex.in_field && complex.depth == 1 {
                        complex.after_separator = true;
                    }
                }
                "end" if complex.in_field => {
                    complex.depth = complex.depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        "instrText" if complex.in_field && complex.depth == 1 && !complex.after_separator => {
            complex.in_instruction_text = true;
        }
        "t" if complex.in_field && complex.depth == 1 && complex.after_separator => {
            complex.in_result_text = true;
        }
        _ => {}
    }
}

fn docx_field_note_text(paragraph: &mut DocxFieldParagraphState, text: &str) {
    if let Some(simple) = paragraph.simple.as_mut() {
        if simple.in_t {
            simple.result.push_str(text);
        }
        return;
    }
    let complex = &mut paragraph.complex;
    if complex.in_instruction_text {
        complex.instruction.push_str(text);
    } else if complex.in_result_text {
        complex.result.push_str(text);
    }
}

fn docx_field_note_end(
    paragraph: &mut DocxFieldParagraphState,
    name: &str,
    fields: &mut Vec<DocxFieldInfo>,
) {
    if let Some(simple) = paragraph.simple.as_mut() {
        if name == "t" {
            simple.in_t = false;
        }
        if simple.depth > 0 {
            simple.depth -= 1;
        }
        if simple.depth == 0 || name == "fldSimple" {
            docx_emit_simple_field(paragraph, fields);
        }
        return;
    }

    let complex = &mut paragraph.complex;
    match name {
        "instrText" => complex.in_instruction_text = false,
        "t" => complex.in_result_text = false,
        "fldChar" if complex.in_field && complex.depth == 0 => {
            docx_emit_complex_field(paragraph, fields);
        }
        _ => {}
    }
}

fn docx_emit_simple_field(
    paragraph: &mut DocxFieldParagraphState,
    fields: &mut Vec<DocxFieldInfo>,
) {
    let Some(simple) = paragraph.simple.take() else {
        return;
    };
    fields.push(docx_field_info(
        paragraph,
        "simple",
        simple.instruction.trim(),
        &simple.result,
    ));
}

fn docx_emit_complex_field(
    paragraph: &mut DocxFieldParagraphState,
    fields: &mut Vec<DocxFieldInfo>,
) {
    let instruction = paragraph.complex.instruction.trim().to_string();
    let cached_result = paragraph.complex.result.clone();
    fields.push(docx_field_info(
        paragraph,
        "complex",
        &instruction,
        &cached_result,
    ));
    paragraph.complex = DocxComplexFieldState::default();
}

fn docx_field_info(
    paragraph: &DocxFieldParagraphState,
    field_type: &str,
    instruction: &str,
    cached_result: &str,
) -> DocxFieldInfo {
    DocxFieldInfo {
        part_uri: paragraph.part_uri.clone(),
        block_index: paragraph.block_index,
        block_kind: paragraph.block_kind.clone(),
        field_type: field_type.to_string(),
        instruction: instruction.to_string(),
        cached_result: cached_result.to_string(),
        location: paragraph.location.clone(),
        editable: paragraph.editable,
        ..DocxFieldInfo::default()
    }
}

fn docx_field_json(field: &DocxFieldInfo) -> Value {
    let mut object = Map::new();
    object.insert("index".to_string(), json!(field.index));
    object.insert("partUri".to_string(), json!(field.part_uri));
    object.insert("blockIndex".to_string(), json!(field.block_index));
    object.insert("blockKind".to_string(), json!(field.block_kind));
    object.insert("fieldType".to_string(), json!(field.field_type));
    object.insert("instruction".to_string(), json!(field.instruction));
    object.insert("cachedResult".to_string(), json!(field.cached_result));
    object.insert("location".to_string(), json!(field.location));
    object.insert("isStale".to_string(), json!(true));
    object.insert("editable".to_string(), json!(field.editable));
    Value::Object(object)
}

fn docx_field_code_base(code: &str) -> String {
    code.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase()
}

fn docx_field_location(part_uri: &str, block_index: usize) -> String {
    let prefix = if part_uri.ends_with("/document.xml") {
        "body".to_string()
    } else {
        let name = part_uri.rsplit('/').next().unwrap_or(part_uri);
        name.strip_suffix(".xml").unwrap_or(name).to_string()
    };
    format!("{prefix}:{block_index}")
}

fn docx_header_footer_part_uris(
    file: &str,
    document_part: &str,
    document_uri: &str,
    document_xml: &str,
) -> CliResult<Vec<String>> {
    let rels_part = relationships_part_for(document_part);
    let rel_targets = relationship_entries(file, &rels_part)
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| rel.target_mode != "External")
        .map(|rel| {
            (
                rel.id,
                resolve_relationship_target(document_uri, &rel.target),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut reader = NsReader::from_str(document_xml);
    let mut stack: Vec<String> = Vec::new();
    let mut section_uris = Vec::new();
    let mut seen = BTreeSet::new();
    let mut in_direct_section = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    in_direct_section = true;
                } else if in_direct_section
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                    && let Some(part_uri) =
                        docx_header_footer_ref_part_uri(&e, reader.resolver(), &rel_targets)
                    && seen.insert(part_uri.clone())
                {
                    section_uris.push(part_uri);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    // Empty section properties have no references.
                } else if in_direct_section
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                    && let Some(part_uri) =
                        docx_header_footer_ref_part_uri(&e, reader.resolver(), &rel_targets)
                    && seen.insert(part_uri.clone())
                {
                    section_uris.push(part_uri);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "sectPr" {
                    in_direct_section = false;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(section_uris)
}

fn docx_header_footer_ref_part_uri(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    rel_targets: &BTreeMap<String, String>,
) -> Option<String> {
    let id = attr_prefixed_ns(
        element,
        resolver,
        b"r",
        b"http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        b"id",
    )?;
    rel_targets.get(&id).cloned()
}

fn docx_word_attr_ns(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    wanted_local: &[u8],
) -> Option<String> {
    attr_prefixed_ns(element, resolver, b"w", DOCX_W_NS, wanted_local)
}

fn docx_headers_footers_list(file: &str) -> CliResult<Value> {
    let (document_uri, sections) = docx_header_footer_listing(file)?;
    Ok(json!({
        "file": file,
        "documentPartUri": document_uri,
        "sections": sections,
    }))
}

fn docx_header_footer_listing(file: &str) -> CliResult<(String, Vec<Value>)> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let xml = zip_text(file, &document_part).map_err(|err| {
        CliError::unexpected(format!(
            "failed to list headers/footers: failed to read document part {document_uri}: {}",
            err.message
        ))
    })?;
    let rel_targets = relationship_entries(file, &relationships_part_for(&document_part))
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| rel.target_mode != "External")
        .map(|rel| {
            (
                rel.id,
                resolve_relationship_target(&document_uri, &rel.target),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let sections = docx_header_footer_sections(file, &xml, &rel_targets)?;
    Ok((document_uri, sections))
}

#[derive(Default)]
struct DocxHeaderFooterSectionBuild {
    section_index: usize,
    headers: DocxHeaderFooterSetBuild,
    footers: DocxHeaderFooterSetBuild,
}

#[derive(Default)]
struct DocxHeaderFooterSetBuild {
    default: Option<Value>,
    first: Option<Value>,
    even: Option<Value>,
}

#[derive(Clone, Debug, Default)]
struct DocxHeaderFooterRefInfo {
    kind: String,
    id: String,
    ref_type: String,
    section: i64,
    primary_selector: String,
    selectors: Vec<String>,
    part_uri: String,
}

#[derive(Default)]
struct DocxHeaderFooterSelector {
    kind: String,
    id: String,
    ref_type: String,
    section: i64,
    part_uri: String,
}

fn docx_header_footer_sections(
    file: &str,
    document_xml: &str,
    rel_targets: &BTreeMap<String, String>,
) -> CliResult<Vec<Value>> {
    let mut reader = NsReader::from_str(document_xml);
    let mut stack: Vec<String> = Vec::new();
    let mut sections = Vec::new();
    let mut current = None::<DocxHeaderFooterSectionBuild>;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if current.is_none()
                    && is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    current = Some(DocxHeaderFooterSectionBuild {
                        section_index: sections.len() + 1,
                        ..DocxHeaderFooterSectionBuild::default()
                    });
                } else if let Some(section) = current.as_mut()
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                {
                    docx_note_header_footer_ref(
                        file,
                        section,
                        &e,
                        reader.resolver(),
                        &name,
                        rel_targets,
                    );
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if current.is_none()
                    && is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    let section = DocxHeaderFooterSectionBuild {
                        section_index: sections.len() + 1,
                        ..DocxHeaderFooterSectionBuild::default()
                    };
                    sections.push(docx_header_footer_section_json(section));
                } else if let Some(section) = current.as_mut()
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                {
                    docx_note_header_footer_ref(
                        file,
                        section,
                        &e,
                        reader.resolver(),
                        &name,
                        rel_targets,
                    );
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "sectPr"
                    && let Some(section) = current.take()
                {
                    sections.push(docx_header_footer_section_json(section));
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(sections)
}

fn docx_note_header_footer_ref(
    file: &str,
    section: &mut DocxHeaderFooterSectionBuild,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    name: &str,
    rel_targets: &BTreeMap<String, String>,
) {
    let kind = if name == "footerReference" {
        "footer"
    } else {
        "header"
    };
    let id = attr_bound_ns(
        element,
        resolver,
        b"http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        b"id",
    )
    .unwrap_or_default();
    let ref_type = normalize_docx_header_footer_type(
        attr_bound_ns(element, resolver, DOCX_W_NS, b"type").unwrap_or_default(),
    );
    let part_uri = rel_targets.get(&id).cloned().unwrap_or_default();
    let content_type = if part_uri.is_empty() {
        String::new()
    } else {
        content_type_for_part(file, &part_uri).unwrap_or_default()
    };
    let value = docx_header_footer_ref_json(
        kind,
        &id,
        &ref_type,
        section.section_index,
        &part_uri,
        &content_type,
    );
    let set = if kind == "footer" {
        &mut section.footers
    } else {
        &mut section.headers
    };
    match ref_type.as_str() {
        "first" => set.first = Some(value),
        "even" => set.even = Some(value),
        _ => set.default = Some(value),
    }
}

fn normalize_docx_header_footer_type(value: String) -> String {
    match value.as_str() {
        "first" | "even" => value,
        _ => "default".to_string(),
    }
}

fn docx_header_footer_ref_json(
    kind: &str,
    id: &str,
    ref_type: &str,
    section: usize,
    part_uri: &str,
    content_type: &str,
) -> Value {
    let primary_selector = format!("{kind}:{section}:{ref_type}");
    let mut selectors = vec![primary_selector.clone()];
    if !id.is_empty() {
        selectors.push(format!("id:{id}"));
        selectors.push(id.to_string());
    }
    if !part_uri.is_empty() {
        selectors.push(format!("part:{part_uri}"));
        selectors.push(part_uri.to_string());
    }
    json!({
        "kind": kind,
        "id": id,
        "type": ref_type,
        "section": section,
        "primarySelector": primary_selector,
        "selectors": selectors,
        "partUri": part_uri,
        "contentType": content_type,
    })
}

fn docx_header_footer_section_json(section: DocxHeaderFooterSectionBuild) -> Value {
    json!({
        "sectionIndex": section.section_index,
        "headers": docx_header_footer_set_json(section.headers),
        "footers": docx_header_footer_set_json(section.footers),
    })
}

fn docx_header_footer_set_json(set: DocxHeaderFooterSetBuild) -> Value {
    json!({
        "default": set.default.unwrap_or(Value::Null),
        "first": set.first.unwrap_or(Value::Null),
        "even": set.even.unwrap_or(Value::Null),
    })
}

fn docx_headers_footers_show(file: &str, kind: &str, rest: &[String]) -> CliResult<Value> {
    reject_unknown_flags(rest, &["--id", "--type", "--section", "--selector"], &[])?;
    let id = parse_string_flag(rest, "--id")?.unwrap_or_default();
    let ref_type = parse_string_flag(rest, "--type")?.unwrap_or_else(|| "default".to_string());
    let ref_type = normalize_docx_header_footer_show_type(&ref_type)?;
    let section = parse_i64_flag(rest, "--section")?.unwrap_or(0);
    if section < 0 {
        return Err(CliError::invalid_args(
            "--section must be >= 0 (0 means the last section)",
        ));
    }
    let selector = parse_string_flag(rest, "--selector")?;
    if selector.is_some()
        && (has_flag(rest, "--id") || has_flag(rest, "--type") || has_flag(rest, "--section"))
    {
        return Err(CliError::invalid_args(
            "cannot specify --selector with --id, --type, or --section",
        ));
    }

    let (_document_uri, sections) = docx_header_footer_listing(file)?;
    let target = if let Some(selector) = selector {
        let parsed = parse_docx_header_footer_selector(kind, &selector)?;
        resolve_docx_header_footer_selector(&sections, kind, &parsed)
    } else if !id.is_empty() {
        resolve_docx_header_footer_selector(
            &sections,
            kind,
            &DocxHeaderFooterSelector {
                kind: kind.to_string(),
                id,
                ref_type,
                section,
                ..DocxHeaderFooterSelector::default()
            },
        )
    } else {
        resolve_docx_header_footer_selector(
            &sections,
            kind,
            &DocxHeaderFooterSelector {
                kind: kind.to_string(),
                ref_type,
                section,
                ..DocxHeaderFooterSelector::default()
            },
        )
    }
    .ok_or_else(|| CliError::target_not_found(format!("target not found: {kind}")))?;

    if target.part_uri.is_empty() {
        return Err(CliError::invalid_args(format!(
            "{kind} reference {:?} does not resolve to a part",
            target.id
        )));
    }
    let paragraphs = docx_header_footer_paragraphs(file, &target)?;
    Ok(json!({
        "file": file,
        "kind": target.kind,
        "partUri": target.part_uri,
        "id": target.id,
        "type": target.ref_type,
        "section": target.section,
        "primarySelector": target.primary_selector,
        "selectors": target.selectors,
        "paragraphs": paragraphs,
    }))
}

fn docx_header_footer_kind(group: &str) -> &'static str {
    if group == "footers" {
        "footer"
    } else {
        "header"
    }
}

fn normalize_docx_header_footer_show_type(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "default" => Ok("default".to_string()),
        "first" => Ok("first".to_string()),
        "even" => Ok("even".to_string()),
        _ => Err(CliError::invalid_args(
            "--type must be one of default, first, even",
        )),
    }
}

fn parse_docx_header_footer_selector(
    command_kind: &str,
    raw: &str,
) -> CliResult<DocxHeaderFooterSelector> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(CliError::invalid_args("--selector cannot be empty"));
    }
    let base = split_docx_header_footer_paragraph_selector(raw)?;
    let mut selector = DocxHeaderFooterSelector {
        kind: command_kind.to_string(),
        ref_type: "default".to_string(),
        ..DocxHeaderFooterSelector::default()
    };
    if let Some(id) = base.strip_prefix("id:") {
        if id.is_empty() {
            return Err(CliError::invalid_args(
                "--selector id:<relId> cannot be empty",
            ));
        }
        selector.id = id.to_string();
        return Ok(selector);
    }
    if let Some(part_uri) = base.strip_prefix("part:") {
        if part_uri.is_empty() {
            return Err(CliError::invalid_args(
                "--selector part:<partUri> cannot be empty",
            ));
        }
        selector.part_uri = part_uri.to_string();
        return Ok(selector);
    }
    if base.starts_with('/') {
        selector.part_uri = base.to_string();
        return Ok(selector);
    }
    if base.starts_with("rId") {
        selector.id = base.to_string();
        return Ok(selector);
    }
    if let Some(rest) = base.strip_prefix("section:") {
        let parts = rest.split(':').collect::<Vec<_>>();
        if parts.len() != 3 || parts[1] != "type" {
            return Err(CliError::invalid_args(
                "--selector section form must be section:<n>:type:<default|first|even>",
            ));
        }
        selector.section = parse_positive_i64(parts[0], "selector section")?;
        selector.ref_type = normalize_docx_header_footer_show_type(parts[2])?;
        return Ok(selector);
    }

    let parts = base.split(':').collect::<Vec<_>>();
    if parts.len() == 3 && (parts[0] == "header" || parts[0] == "footer") {
        if parts[0] != command_kind {
            return Err(CliError::invalid_args(format!(
                "--selector kind {:?} does not match {command_kind} command",
                parts[0]
            )));
        }
        selector.kind = parts[0].to_string();
        selector.section = parse_positive_i64(parts[1], "selector section")?;
        selector.ref_type = normalize_docx_header_footer_show_type(parts[2])?;
        return Ok(selector);
    }

    Err(CliError::invalid_args(
        "--selector must be header:<section>:<type>, footer:<section>:<type>, section:<section>:type:<type>, id:<relId>, or part:<partUri>",
    ))
}

fn split_docx_header_footer_paragraph_selector(raw: &str) -> CliResult<&str> {
    for marker in ["/paragraph:", "/p:"] {
        if let Some(index) = raw.rfind(marker) {
            let base = raw[..index].trim();
            let value = raw[index + marker.len()..].trim();
            if base.is_empty() {
                return Err(CliError::invalid_args(
                    "--selector paragraph suffix requires a header/footer selector before it",
                ));
            }
            let _ = parse_positive_i64(value, "selector paragraph")?;
            return Ok(base);
        }
    }
    Ok(raw)
}

fn parse_positive_i64(value: &str, label: &str) -> CliResult<i64> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args(format!("{label} cannot be empty")));
    }
    let parsed = value
        .parse::<i64>()
        .map_err(|_| CliError::invalid_args(format!("{label} must be an integer")))?;
    if parsed < 1 {
        return Err(CliError::invalid_args(format!("{label} must be >= 1")));
    }
    Ok(parsed)
}

fn resolve_docx_header_footer_selector(
    sections: &[Value],
    command_kind: &str,
    selector: &DocxHeaderFooterSelector,
) -> Option<DocxHeaderFooterRefInfo> {
    let kind = if selector.kind.is_empty() {
        command_kind
    } else {
        &selector.kind
    };
    let refs = docx_header_footer_refs(sections, kind);
    if !selector.id.is_empty() {
        return refs
            .into_iter()
            .find(|reference| reference.id == selector.id);
    }
    if !selector.part_uri.is_empty() {
        return refs
            .into_iter()
            .find(|reference| reference.part_uri == selector.part_uri);
    }
    let section = if selector.section > 0 {
        selector.section
    } else {
        sections
            .last()
            .and_then(|section| section["sectionIndex"].as_i64())
            .unwrap_or(0)
    };
    refs.into_iter()
        .find(|reference| reference.section == section && reference.ref_type == selector.ref_type)
}

fn docx_header_footer_refs(sections: &[Value], kind: &str) -> Vec<DocxHeaderFooterRefInfo> {
    let mut refs = Vec::new();
    for section in sections {
        let set = if kind == "footer" {
            &section["footers"]
        } else {
            &section["headers"]
        };
        for ref_type in ["default", "first", "even"] {
            if let Some(reference) = docx_header_footer_ref_info(&set[ref_type]) {
                refs.push(reference);
            }
        }
    }
    refs
}

fn docx_header_footer_ref_info(value: &Value) -> Option<DocxHeaderFooterRefInfo> {
    if value.is_null() {
        return None;
    }
    let selectors = value["selectors"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();
    Some(DocxHeaderFooterRefInfo {
        kind: value["kind"].as_str()?.to_string(),
        id: value["id"].as_str().unwrap_or_default().to_string(),
        ref_type: value["type"].as_str().unwrap_or_default().to_string(),
        section: value["section"].as_i64().unwrap_or_default(),
        primary_selector: value["primarySelector"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        selectors,
        part_uri: value["partUri"].as_str().unwrap_or_default().to_string(),
    })
}

fn docx_header_footer_paragraphs(
    file: &str,
    reference: &DocxHeaderFooterRefInfo,
) -> CliResult<Vec<Value>> {
    let xml = zip_text(file, reference.part_uri.trim_start_matches('/')).map_err(|err| {
        CliError::unexpected(format!(
            "failed to read header/footer part {}: {}",
            reference.part_uri, err.message
        ))
    })?;
    let mut reader = NsReader::from_str(&xml);
    let mut stack = Vec::<String>::new();
    let mut paragraphs = Vec::new();
    let mut current = None::<DocxHeaderFooterParagraphBuild>;
    let mut in_t = false;
    let mut skip_text_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    current = Some(DocxHeaderFooterParagraphBuild::default());
                }
                docx_note_header_footer_paragraph_start(
                    &mut current,
                    &e,
                    reader.resolver(),
                    &stack,
                    is_word,
                    skip_text_depth,
                );
                if is_word && name == "t" {
                    in_t = true;
                }
                if is_word && matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth += 1;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    let paragraph = DocxHeaderFooterParagraphBuild::default();
                    paragraphs.push(docx_header_footer_paragraph_json(
                        paragraphs.len() + 1,
                        paragraph,
                        reference,
                    ));
                } else {
                    docx_note_header_footer_paragraph_start(
                        &mut current,
                        &e,
                        reader.resolver(),
                        &stack,
                        is_word,
                        skip_text_depth,
                    );
                }
            }
            Ok(Event::Text(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph.text.push_str(&xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph
                        .text
                        .push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_t = false;
                } else if matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth = skip_text_depth.saturating_sub(1);
                } else if name == "p"
                    && let Some(paragraph) = current.take()
                {
                    paragraphs.push(docx_header_footer_paragraph_json(
                        paragraphs.len() + 1,
                        paragraph,
                        reference,
                    ));
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(paragraphs)
}

#[derive(Default)]
struct DocxHeaderFooterParagraphBuild {
    style: String,
    text: String,
}

fn docx_note_header_footer_paragraph_start(
    current: &mut Option<DocxHeaderFooterParagraphBuild>,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    stack: &[String],
    is_word: bool,
    skip_text_depth: usize,
) {
    let Some(paragraph) = current.as_mut() else {
        return;
    };
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if is_word
        && name == "pStyle"
        && stack.last().is_some_and(|parent| parent == "pPr")
        && let Some(style) = docx_word_attr_ns(element, resolver, b"val")
    {
        paragraph.style = style;
        return;
    }
    if is_word && skip_text_depth == 0 {
        match name {
            "tab" => paragraph.text.push('\t'),
            "br" | "cr" => paragraph.text.push('\n'),
            "noBreakHyphen" => paragraph.text.push('-'),
            _ => {}
        }
    }
}

fn docx_header_footer_paragraph_json(
    index: usize,
    paragraph: DocxHeaderFooterParagraphBuild,
    reference: &DocxHeaderFooterRefInfo,
) -> Value {
    let primary_selector = if reference.primary_selector.is_empty() {
        String::new()
    } else {
        format!("{}/p:{index}", reference.primary_selector)
    };
    let mut selectors = Vec::new();
    for selector in &reference.selectors {
        selectors.push(format!("{selector}/p:{index}"));
        selectors.push(format!("{selector}/paragraph:{index}"));
    }
    json!({
        "index": index,
        "primarySelector": primary_selector,
        "selectors": selectors,
        "style": paragraph.style,
        "text": paragraph.text,
    })
}

fn docx_images_list(file: &str) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let xml = zip_text(file, &document_part).map_err(|err| {
        CliError::unexpected(format!(
            "failed to extract DOCX images: failed to read document part {document_uri}: {}",
            err.message
        ))
    })?;
    let block_reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        CliError::unexpected(format!("failed to extract DOCX images: {}", err.message))
    })?;
    let block_reports = block_reports
        .into_iter()
        .map(|report| (report.index, report))
        .collect::<BTreeMap<_, _>>();
    let rel_targets = docx_image_relationship_targets(file, &document_part, &document_uri);
    let refs = docx_image_refs_in_document_xml(&xml).map_err(|err| {
        CliError::unexpected(format!("failed to extract DOCX images: {}", err.message))
    })?;

    let mut images = Vec::new();
    for image_ref in refs {
        let Some(block) = block_reports.get(&image_ref.block_index) else {
            continue;
        };
        let index = images.len() + 1;
        let media_uri = rel_targets
            .get(&image_ref.blip_id)
            .cloned()
            .unwrap_or_default();
        let content_type = if media_uri.is_empty() {
            String::new()
        } else {
            content_type_for_part(file, &media_uri).unwrap_or_default()
        };
        let mut image = Map::new();
        image.insert("index".to_string(), json!(index));
        image.insert("id".to_string(), json!(image_ref.blip_id));
        image.insert("primarySelector".to_string(), json!(index.to_string()));
        image.insert("selectors".to_string(), json!([index.to_string()]));
        image.insert("blockIndex".to_string(), json!(image_ref.block_index));
        image.insert(
            "blockId".to_string(),
            json!(format!("body.b{}", image_ref.block_index)),
        );
        image.insert("blockHash".to_string(), json!(block.content_hash));
        image.insert("blipId".to_string(), json!(image_ref.blip_id));
        image.insert("mediaUri".to_string(), json!(media_uri));
        image.insert("contentType".to_string(), json!(content_type));
        image.insert("width".to_string(), json!(image_ref.width));
        image.insert("height".to_string(), json!(image_ref.height));
        images.push(Value::Object(image));
    }

    Ok(json!({
        "file": file,
        "documentPartUri": document_uri,
        "images": images,
    }))
}

#[derive(Default)]
struct DocxImageRef {
    block_index: usize,
    blip_id: String,
    width: i64,
    height: i64,
}

#[derive(Default)]
struct DocxDrawingScan {
    depth: usize,
    container_depth: Option<usize>,
    container_kind: String,
    blip_id: String,
    width: i64,
    height: i64,
    saw_extent: bool,
}

fn docx_image_relationship_targets(
    file: &str,
    document_part: &str,
    document_uri: &str,
) -> BTreeMap<String, String> {
    relationship_entries(file, &relationships_part_for(document_part))
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| rel.target_mode != "External")
        .filter(|rel| {
            rel.rel_type
                == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
                || rel.rel_type.ends_with("/image")
        })
        .map(|rel| {
            (
                rel.id,
                resolve_relationship_target(document_uri, &rel.target),
            )
        })
        .collect()
}

fn docx_image_refs_in_document_xml(xml: &str) -> CliResult<Vec<DocxImageRef>> {
    let mut reader = NsReader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut refs = Vec::new();
    let mut block_index = 0usize;
    let mut current_block = None::<DocxImageBlockKind>;
    let mut body_table_depth = 0usize;
    let mut drawing = None::<DocxDrawingScan>;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if current_block.is_none() && parent == Some("body") && is_word && name == "p" {
                    block_index += 1;
                    current_block = Some(DocxImageBlockKind::Paragraph { index: block_index });
                } else if current_block.is_none()
                    && parent == Some("body")
                    && is_word
                    && name == "tbl"
                {
                    block_index += 1;
                    current_block = Some(DocxImageBlockKind::Table { index: block_index });
                    body_table_depth = 1;
                } else if matches!(current_block, Some(DocxImageBlockKind::Table { .. }))
                    && is_word
                    && name == "tbl"
                {
                    body_table_depth += 1;
                }

                let current_index = current_block.as_ref().map(DocxImageBlockKind::index);
                docx_image_note_start(&mut drawing, &e, reader.resolver(), is_word, current_index);
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if current_block.is_none()
                    && parent == Some("body")
                    && is_word
                    && matches!(name.as_str(), "p" | "tbl")
                {
                    block_index += 1;
                } else {
                    let current_index = current_block.as_ref().map(DocxImageBlockKind::index);
                    docx_image_note_empty(
                        &mut drawing,
                        &e,
                        reader.resolver(),
                        is_word,
                        current_index,
                        &mut refs,
                    );
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let current_index = current_block.as_ref().map(DocxImageBlockKind::index);
                docx_image_note_end(&mut drawing, &name, current_index, &mut refs);

                match current_block {
                    Some(DocxImageBlockKind::Paragraph { .. }) if name == "p" => {
                        current_block = None;
                    }
                    Some(DocxImageBlockKind::Table { .. }) if name == "tbl" => {
                        body_table_depth = body_table_depth.saturating_sub(1);
                        if body_table_depth == 0 {
                            current_block = None;
                        }
                    }
                    _ => {}
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(refs)
}

#[derive(Clone, Copy)]
enum DocxImageBlockKind {
    Paragraph { index: usize },
    Table { index: usize },
}

impl DocxImageBlockKind {
    fn index(&self) -> usize {
        match self {
            DocxImageBlockKind::Paragraph { index } | DocxImageBlockKind::Table { index } => *index,
        }
    }
}

fn docx_image_note_start(
    drawing: &mut Option<DocxDrawingScan>,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    is_word: bool,
    current_block: Option<usize>,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if drawing.is_none() {
        if current_block.is_some() && is_word && name == "drawing" {
            *drawing = Some(DocxDrawingScan {
                depth: 1,
                ..DocxDrawingScan::default()
            });
        }
        return;
    }

    let Some(scan) = drawing.as_mut() else {
        return;
    };
    let event_depth = scan.depth + 1;
    docx_drawing_scan_element(scan, element, resolver, name, event_depth);
    scan.depth += 1;
}

fn docx_image_note_empty(
    drawing: &mut Option<DocxDrawingScan>,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    is_word: bool,
    current_block: Option<usize>,
    refs: &mut Vec<DocxImageRef>,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if drawing.is_none() {
        if current_block.is_some() && is_word && name == "drawing" {
            // Empty w:drawing cannot contain an inline image.
        }
        return;
    }

    let Some(scan) = drawing.as_mut() else {
        return;
    };
    let event_depth = scan.depth + 1;
    docx_drawing_scan_element(scan, element, resolver, name, event_depth);
    if matches!(name, "inline" | "anchor") && scan.container_depth == Some(event_depth) {
        scan.container_depth = None;
    }
    if name == "drawing" {
        docx_finish_drawing(drawing, current_block, refs);
    }
}

fn docx_image_note_end(
    drawing: &mut Option<DocxDrawingScan>,
    name: &str,
    current_block: Option<usize>,
    refs: &mut Vec<DocxImageRef>,
) {
    let Some(scan) = drawing.as_mut() else {
        return;
    };
    if name == "drawing" && scan.depth == 1 {
        docx_finish_drawing(drawing, current_block, refs);
        return;
    }
    if scan.container_depth == Some(scan.depth) && name == scan.container_kind {
        scan.container_depth = None;
    }
    scan.depth = scan.depth.saturating_sub(1);
}

fn docx_drawing_scan_element(
    scan: &mut DocxDrawingScan,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    name: &str,
    event_depth: usize,
) {
    match name {
        "inline" if scan.container_kind != "inline" => {
            scan.container_depth = Some(event_depth);
            scan.container_kind = "inline".to_string();
            scan.blip_id.clear();
            scan.width = 0;
            scan.height = 0;
            scan.saw_extent = false;
        }
        "anchor" if scan.container_kind.is_empty() => {
            scan.container_depth = Some(event_depth);
            scan.container_kind = "anchor".to_string();
        }
        "extent"
            if scan.container_depth == Some(event_depth.saturating_sub(1)) && !scan.saw_extent =>
        {
            scan.width = attr(element, "cx")
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or_default();
            scan.height = attr(element, "cy")
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or_default();
            scan.saw_extent = true;
        }
        "blip" if scan.container_depth.is_some() && scan.blip_id.is_empty() => {
            scan.blip_id = docx_blip_embed_id(element, resolver).unwrap_or_default();
        }
        _ => {}
    }
}

fn docx_finish_drawing(
    drawing: &mut Option<DocxDrawingScan>,
    current_block: Option<usize>,
    refs: &mut Vec<DocxImageRef>,
) {
    let Some(scan) = drawing.take() else {
        return;
    };
    if let Some(block_index) = current_block
        && !scan.blip_id.is_empty()
    {
        refs.push(DocxImageRef {
            block_index,
            blip_id: scan.blip_id,
            width: scan.width,
            height: scan.height,
        });
    }
}

fn docx_blip_embed_id(element: &BytesStart<'_>, resolver: &NamespaceResolver) -> Option<String> {
    attr_exact(element, "embed")
        .or_else(|| {
            attr_prefixed_ns(
                element,
                resolver,
                b"r",
                b"http://schemas.openxmlformats.org/officeDocument/2006/relationships",
                b"embed",
            )
        })
        .or_else(|| attr_exact(element, "r:embed"))
}

fn docx_tables_show(file: &str, table: usize, include_details: bool) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let xml = zip_text(file, &document_part).map_err(|_| {
        CliError::unexpected(format!(
            "failed to read main document: part {document_uri} not found"
        ))
    })?;
    let reports = docx_rich_block_reports(&xml, false).map_err(|err| {
        if err.message == "invalid DOCX XML"
            || err.message.starts_with("failed to extract DOCX blocks:")
        {
            CliError::unexpected(format!(
                "failed to read main document: failed to read document part {document_uri}: failed to parse XML part {document_uri}: etree: invalid XML format"
            ))
        } else {
            CliError::unexpected(format!("failed to read main document: {}", err.message))
        }
    })?;

    let mut table_number = 0usize;
    let mut tables = Vec::new();
    for report in reports.into_iter().filter(|report| report.kind == "table") {
        table_number += 1;
        if table > 0 && table_number != table {
            continue;
        }
        tables.push(docx_table_summary_json(
            file,
            table_number,
            report,
            include_details,
        ));
    }
    if table > 0 && tables.is_empty() {
        return Err(CliError::target_not_found(format!(
            "target not found: table {table}"
        )));
    }

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert(
        "tables".to_string(),
        if tables.is_empty() {
            Value::Null
        } else {
            Value::Array(tables)
        },
    );
    Ok(Value::Object(result))
}

fn docx_table_summary_json(
    file: &str,
    table_number: usize,
    report: DocxRichBlockReport,
    include_details: bool,
) -> Value {
    let rows = report.table_rows;
    let row_count = rows.len();
    let col_count = rows.iter().map(Vec::len).max().unwrap_or_default();
    let mut table = Map::new();
    table.insert("file".to_string(), json!(file));
    table.insert("table".to_string(), json!(table_number));
    table.insert("block".to_string(), json!(report.index));
    table.insert(
        "primarySelector".to_string(),
        json!(table_number.to_string()),
    );
    table.insert("selectors".to_string(), json!([table_number.to_string()]));
    table.insert("contentHash".to_string(), json!(report.content_hash));
    table.insert("rows".to_string(), json!(row_count));
    table.insert("cols".to_string(), json!(col_count));
    table.insert("merged".to_string(), json!(report.table_merged));
    if include_details {
        let detail_rows: Vec<Value> = rows.iter().map(|row| json!({"cells": row})).collect();
        table.insert("tableInfo".to_string(), json!({"rows": detail_rows}));
    } else {
        table.insert("cells".to_string(), json!(rows));
    }
    Value::Object(table)
}

fn docx_document_and_comments_parts(file: &str) -> CliResult<(String, Option<String>)> {
    let entries = zip_entry_names(file)?;
    if detect_inspect_package_type(file, &entries) != InspectPackageKind::Docx {
        return Err(CliError::unsupported_type(
            "file is not a DOCX document (detected: unknown)",
        ));
    }
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let comments_part = find_docx_comments_part(file, &entries, &document_part)?;
    Ok((document_uri, comments_part))
}

fn find_docx_comments_part(
    file: &str,
    entries: &[String],
    document_part: &str,
) -> CliResult<Option<String>> {
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    for rel in
        relationship_entries(file, &relationships_part_for(document_part)).unwrap_or_default()
    {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type
            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"
        {
            let uri = resolve_relationship_target(&document_uri, &rel.target);
            return Ok(zip_entry_exists(entries, &uri).then_some(uri));
        }
    }
    let conventional = "/word/comments.xml";
    Ok(zip_entry_exists(entries, conventional).then(|| conventional.to_string()))
}

fn zip_entry_exists(entries: &[String], uri: &str) -> bool {
    let wanted = format!("/{}", uri.trim_start_matches('/'));
    entries
        .iter()
        .any(|entry| format!("/{}", entry.trim_start_matches('/')) == wanted)
}

#[derive(Clone, Default)]
struct DocxCommentInfo {
    id: i64,
    id_raw: String,
    id_valid: bool,
    author: String,
    date: String,
    initials: String,
    text: String,
    anchored_to_block: usize,
    anchored_to_block_kind: String,
}

#[derive(Default)]
struct DocxCommentBuild {
    info: DocxCommentInfo,
    paragraphs: Vec<String>,
    current_paragraph: Option<String>,
    in_t: bool,
    skip_text_depth: usize,
}

fn docx_comments(
    file: &str,
    comments_part: &str,
    document_part: &str,
) -> CliResult<Vec<DocxCommentInfo>> {
    let xml = zip_text(file, comments_part.trim_start_matches('/'))?;
    let anchors = docx_comment_anchors(file, document_part)?;
    let mut reader = Reader::from_str(&xml);
    let mut saw_root = false;
    let mut stack = Vec::<String>::new();
    let mut current: Option<DocxCommentBuild> = None;
    let mut comments = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "comments" {
                        return Ok(Vec::new());
                    }
                } else if name == "comment"
                    && stack.last().is_some_and(|parent| parent == "comments")
                {
                    current = Some(docx_comment_from_element(&e));
                } else if let Some(comment) = current.as_mut() {
                    docx_note_comment_start(&e, &name, &stack, comment);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    saw_root = true;
                    if name != "comments" {
                        return Ok(Vec::new());
                    }
                } else if name == "comment"
                    && stack.last().is_some_and(|parent| parent == "comments")
                {
                    let mut comment = docx_comment_from_element(&e);
                    docx_finish_comment(&mut comment, &anchors);
                    comments.push(comment.info);
                } else if let Some(comment) = current.as_mut() {
                    docx_note_comment_empty(&e, &name, &stack, comment);
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(comment) = current.as_mut()
                    && comment.in_t
                    && comment.skip_text_depth == 0
                    && let Some(paragraph) = comment.current_paragraph.as_mut()
                {
                    paragraph.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(comment) = current.as_mut()
                    && comment.in_t
                    && comment.skip_text_depth == 0
                    && let Some(paragraph) = comment.current_paragraph.as_mut()
                {
                    paragraph.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(comment) = current.as_mut() {
                    match name.as_str() {
                        "t" => comment.in_t = false,
                        "delText" | "instrText" => {
                            comment.skip_text_depth = comment.skip_text_depth.saturating_sub(1);
                        }
                        "p" => {
                            if let Some(paragraph) = comment.current_paragraph.take() {
                                comment.paragraphs.push(paragraph);
                            }
                        }
                        "comment" => {
                            if let Some(mut comment) = current.take() {
                                docx_finish_comment(&mut comment, &anchors);
                                comments.push(comment.info);
                            }
                        }
                        _ => {}
                    }
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(comments)
}

fn docx_comment_from_element(element: &BytesStart<'_>) -> DocxCommentBuild {
    let id_raw = attr(element, "id").unwrap_or_default();
    let (id, id_valid) = parse_docx_comment_id(&id_raw);
    DocxCommentBuild {
        info: DocxCommentInfo {
            id,
            id_raw,
            id_valid,
            author: attr(element, "author").unwrap_or_default(),
            date: attr(element, "date").unwrap_or_default(),
            initials: attr(element, "initials").unwrap_or_default(),
            ..DocxCommentInfo::default()
        },
        ..DocxCommentBuild::default()
    }
}

fn docx_note_comment_start(
    element: &BytesStart<'_>,
    name: &str,
    stack: &[String],
    comment: &mut DocxCommentBuild,
) {
    if name == "p" && stack.last().is_some_and(|parent| parent == "comment") {
        comment.current_paragraph = Some(String::new());
    }
    docx_note_comment_empty(element, name, stack, comment);
    if name == "t" {
        comment.in_t = true;
    }
    if name == "delText" || name == "instrText" {
        comment.skip_text_depth += 1;
    }
}

fn docx_note_comment_empty(
    _element: &BytesStart<'_>,
    name: &str,
    _stack: &[String],
    comment: &mut DocxCommentBuild,
) {
    let Some(paragraph) = comment.current_paragraph.as_mut() else {
        return;
    };
    match name {
        "tab" => paragraph.push('\t'),
        "br" | "cr" => paragraph.push('\n'),
        "noBreakHyphen" => paragraph.push('-'),
        _ => {}
    }
}

fn docx_finish_comment(
    comment: &mut DocxCommentBuild,
    anchors: &BTreeMap<String, DocxCommentAnchor>,
) {
    comment.info.text = comment.paragraphs.join("\n");
    if let Some(anchor) = anchors.get(&comment.info.id_raw) {
        comment.info.anchored_to_block = anchor.index;
        comment.info.anchored_to_block_kind = anchor.kind.clone();
    }
}

fn parse_docx_comment_id(value: &str) -> (i64, bool) {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return (0, false);
    }
    value
        .parse::<i64>()
        .map(|id| (id, true))
        .unwrap_or((0, false))
}

#[derive(Clone)]
struct DocxCommentAnchor {
    index: usize,
    kind: String,
    tag: String,
    depth: usize,
}

fn docx_comment_anchors(
    file: &str,
    document_part: &str,
) -> CliResult<BTreeMap<String, DocxCommentAnchor>> {
    let xml = zip_text(file, document_part.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut anchors = BTreeMap::<String, DocxCommentAnchor>::new();
    let mut current_block: Option<DocxCommentAnchor> = None;
    let mut block_index = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.last().is_some_and(|parent| parent == "body")
                    && matches!(name.as_str(), "p" | "tbl")
                {
                    block_index += 1;
                    current_block = Some(DocxCommentAnchor {
                        index: block_index,
                        kind: if name == "p" { "paragraph" } else { "table" }.to_string(),
                        tag: name.clone(),
                        depth: stack.len() + 1,
                    });
                }
                if name == "commentRangeStart" {
                    docx_note_comment_anchor(&mut anchors, current_block.as_ref(), &e);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.last().is_some_and(|parent| parent == "body")
                    && matches!(name.as_str(), "p" | "tbl")
                {
                    block_index += 1;
                }
                if name == "commentRangeStart" {
                    docx_note_comment_anchor(&mut anchors, current_block.as_ref(), &e);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current_block
                    .as_ref()
                    .is_some_and(|block| block.depth == stack.len() && block.tag == name)
                {
                    current_block = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(anchors)
}

fn docx_note_comment_anchor(
    anchors: &mut BTreeMap<String, DocxCommentAnchor>,
    current_block: Option<&DocxCommentAnchor>,
    element: &BytesStart<'_>,
) {
    let Some(block) = current_block else {
        return;
    };
    if let Some(id) = attr(element, "id") {
        anchors.entry(id).or_insert_with(|| block.clone());
    }
}

fn docx_comment_id_counts(comments: &[DocxCommentInfo]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for comment in comments {
        if !comment.id_raw.is_empty() {
            *counts.entry(comment.id_raw.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn docx_comment_json(comment: &DocxCommentInfo, counts: &BTreeMap<String, usize>) -> Value {
    let mut object = Map::new();
    object.insert("id".to_string(), json!(comment.id));
    object.insert("author".to_string(), json!(comment.author));
    if !comment.date.is_empty() {
        object.insert("date".to_string(), json!(comment.date));
    }
    if !comment.initials.is_empty() {
        object.insert("initials".to_string(), json!(comment.initials));
    }
    object.insert("text".to_string(), json!(comment.text));
    object.insert(
        "contentHash".to_string(),
        json!(docx_comment_content_hash(
            &comment.author,
            &comment.date,
            &comment.text
        )),
    );
    if comment.anchored_to_block > 0 {
        object.insert(
            "anchoredToBlock".to_string(),
            json!(comment.anchored_to_block),
        );
    }
    if !comment.anchored_to_block_kind.is_empty() {
        object.insert(
            "anchoredToBlockKind".to_string(),
            json!(comment.anchored_to_block_kind),
        );
    }
    if comment.id_valid {
        let selector = comment.id.to_string();
        object.insert("primarySelector".to_string(), json!(selector));
        object.insert("selectors".to_string(), json!([selector]));
        if counts.get(&comment.id_raw).copied().unwrap_or_default() == 1 {
            object.insert(
                "handle".to_string(),
                json!(format!("H:docx/pt:doc/comment:n:{}", comment.id)),
            );
        }
    }
    Value::Object(object)
}

fn docx_comment_content_hash(author: &str, date: &str, text: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(author.as_bytes());
    hash.update([0]);
    hash.update(date.as_bytes());
    hash.update([0]);
    hash.update(text.as_bytes());
    format!("sha256:{:x}", hash.finalize())
}

fn pptx_render(file: &str, args: &[String]) -> CliResult<Value> {
    let out = parse_string_flag(args, "--out")?
        .ok_or_else(|| CliError::invalid_args("--out is required"))?;
    if let Some(format) = parse_string_flag(args, "--format")?
        && format != "json"
    {
        return Err(CliError::invalid_args(
            "pptx render supports --format json only",
        ));
    }
    let slides = parse_slides_flag(args, "--slides")?.unwrap_or_else(|| pptx_all_slides(file));
    let output_dir = PathBuf::from(&out);
    fs::create_dir_all(&output_dir).map_err(|err| CliError::unexpected(err.to_string()))?;
    let pdf_path = if std::env::var_os("OOXML_RUST_MOCK_RENDER").is_some() {
        mock_render_outputs(file, &output_dir, &slides)?
    } else {
        render_with_local_tools(file, &output_dir, &slides)?
    };
    let slide_values: Vec<Value> = slides
        .iter()
        .map(|slide| {
            json!({
                "imagePath": output_dir.join(format!("slide-{slide}.png")).to_string_lossy(),
                "slide": slide,
            })
        })
        .collect();
    Ok(json!({
        "dpi": 144,
        "imageFormat": "png",
        "outputDir": out,
        "pdfPath": pdf_path.to_string_lossy(),
        "slides": slide_values,
        "sourceFile": file,
    }))
}

fn verify(file: &str, args: &[String]) -> CliResult<Value> {
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

#[derive(Default)]
struct ServeState {
    next_session: usize,
    sessions: BTreeMap<String, ServeSession>,
}

struct ServeSession {
    file: String,
    out: Option<String>,
    dry_run: bool,
    working: String,
    ops: Vec<ServeOp>,
}

#[derive(Clone)]
enum ServeOp {
    XlsxCellSet {
        command: String,
        sheet: String,
        cell: String,
        value: String,
        previous_type: String,
        previous_value: Value,
    },
    PptxReplaceText {
        command: String,
        slide: u32,
        target: String,
        text: String,
    },
}

impl ServeOp {
    fn command(&self) -> &str {
        match self {
            ServeOp::XlsxCellSet { command, .. } | ServeOp::PptxReplaceText { command, .. } => {
                command
            }
        }
    }

    fn plan_argv(&self, source_file: &str) -> Value {
        match self {
            ServeOp::XlsxCellSet {
                sheet, cell, value, ..
            } => json!([
                "xlsx",
                "cells",
                "set",
                source_file,
                "--cell",
                cell,
                "--sheet",
                sheet,
                "--value",
                value,
                "--out",
                "<temp.0>",
                "--json",
                "--no-validate",
            ]),
            ServeOp::PptxReplaceText {
                slide,
                target,
                text,
                ..
            } => json!([
                "pptx",
                "replace",
                "text",
                source_file,
                "--slide",
                slide.to_string(),
                "--target",
                target,
                "--text",
                text,
                "--out",
                "<temp.0>",
                "--json",
                "--no-validate",
            ]),
        }
    }

    fn readback(&self, file: &str) -> Value {
        match self {
            ServeOp::XlsxCellSet {
                cell,
                value,
                previous_type,
                previous_value,
                ..
            } => xlsx_cell_set_readback(file, cell, value, previous_type, previous_value),
            ServeOp::PptxReplaceText {
                slide,
                target,
                text,
                ..
            } => pptx_replace_text_readback(file, file, *slide, target, text),
        }
    }
}

impl ServeState {
    fn handle_rpc(&mut self, request: Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        match self.handle_method(method, &params) {
            Ok(result) => json!({"id": id, "jsonrpc": "2.0", "result": result}),
            Err(err) => json!({
                "id": id,
                "jsonrpc": "2.0",
                "error": {
                    "code": err.exit_code,
                    "message": err.message,
                    "data": {"type": err.code, "exitCode": err.exit_code},
                },
            }),
        }
    }

    fn handle_method(&mut self, method: &str, params: &Value) -> CliResult<Value> {
        match method {
            "open" => self.serve_open(params),
            "op" => self.serve_op(params),
            "inspect" => self.serve_inspect(params),
            "validate" => self.serve_validate(params),
            "plan" => self.serve_plan(params),
            "commit" => self.serve_commit(params),
            "abort" => self.serve_abort(params),
            _ => Err(CliError::invalid_args(format!(
                "unsupported serve method: {method}"
            ))),
        }
    }

    fn serve_open(&mut self, params: &Value) -> CliResult<Value> {
        let file = json_string(params, "file")?;
        let out = json_optional_string(params, "out");
        let dry_run = params
            .get("dryRun")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        self.next_session += 1;
        let session_id = format!("rust-session-{}", self.next_session);
        let working = make_working_copy(&file, self.next_session)?;
        self.sessions.insert(
            session_id.clone(),
            ServeSession {
                file: file.clone(),
                out,
                dry_run,
                working,
                ops: Vec::new(),
            },
        );
        Ok(json!({"sessionId": session_id, "type": package_type(&file)?}))
    }

    fn serve_op(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let command = json_string(params, "command")?;
        let args = params
            .get("args")
            .ok_or_else(|| CliError::invalid_args("op args are required"))?;
        let session = self.session_mut(&session_id)?;
        let op = match command.as_str() {
            "xlsx cells set" => {
                let sheet = json_string(args, "sheet")?;
                let cell = json_string(args, "cell")?;
                let value = json_string(args, "value")?;
                let previous = xlsx_cell_read(&session.working, &sheet, &cell)?;
                xlsx_set_cell_string(&session.working, &sheet, &cell, &value)?;
                ServeOp::XlsxCellSet {
                    command: command.clone(),
                    sheet,
                    cell,
                    value,
                    previous_type: previous.kind,
                    previous_value: previous.value,
                }
            }
            "pptx replace text" => {
                let slide = json_u32(args, "slide")?.unwrap_or(1);
                let target = json_string(args, "target")?;
                let text = json_string(args, "text")?;
                pptx_replace_text_in_place(&session.working, slide, &target, &text)?;
                ServeOp::PptxReplaceText {
                    command: command.clone(),
                    slide,
                    target,
                    text,
                }
            }
            _ => {
                return Err(CliError::invalid_args(format!(
                    "unsupported serve op command: {command}"
                )));
            }
        };
        let readback = op.readback(&session.working);
        let index = session.ops.len();
        session.ops.push(op);
        Ok(json!({"command": command, "index": index, "readback": readback}))
    }

    fn serve_inspect(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let command = json_string(params, "command")?;
        let args = params
            .get("args")
            .ok_or_else(|| CliError::invalid_args("inspect args are required"))?;
        let session = self.session(&session_id)?;
        match command.as_str() {
            "xlsx ranges export" => {
                let sheet = json_string(args, "sheet")?;
                let range = json_string(args, "range")?;
                let data_format = json_optional_string(args, "data-format")
                    .or_else(|| json_optional_string(args, "dataFormat"));
                require_json_data_format(data_format.as_deref())?;
                let data_out = json_optional_string(args, "data-out")
                    .or_else(|| json_optional_string(args, "dataOut"));
                let max_cells = json_i64(args, "max-cells")?
                    .or(json_i64(args, "maxCells")?)
                    .unwrap_or(100000);
                let include_types = json_bool(args, "include-types")
                    .or_else(|| json_bool(args, "includeTypes"))
                    .unwrap_or(false);
                let include_formulas = json_bool(args, "include-formulas")
                    .or_else(|| json_bool(args, "includeFormulas"))
                    .unwrap_or(false);
                let include_formats = json_bool(args, "include-formats")
                    .or_else(|| json_bool(args, "includeFormats"))
                    .unwrap_or(false);
                xlsx_range_export_with_options(
                    &session.working,
                    &sheet,
                    &range,
                    XlsxRangeExportOptions {
                        include_types,
                        include_formulas,
                        include_formats,
                        data_out: data_out.as_deref(),
                        max_cells,
                    },
                )
            }
            "xlsx cells extract" => {
                let sheet = json_optional_string(args, "sheet").unwrap_or_else(|| "1".to_string());
                let range = json_optional_string(args, "range");
                let max_rows = json_u32(args, "max-rows")?
                    .or(json_u32(args, "maxRows")?)
                    .unwrap_or(1000);
                let max_cells = json_u32(args, "max-cells")?
                    .or(json_u32(args, "maxCells")?)
                    .unwrap_or(0);
                let include_empty = json_bool(args, "include-empty")
                    .or_else(|| json_bool(args, "includeEmpty"))
                    .unwrap_or(false);
                xlsx_cells_extract(
                    &session.working,
                    &sheet,
                    range.as_deref(),
                    max_rows,
                    max_cells,
                    include_empty,
                )
            }
            "xlsx sheets list" => xlsx_sheets_list(&session.working),
            "xlsx sheets show" => {
                let sheet = json_optional_string(args, "sheet");
                xlsx_sheets_show(&session.working, sheet.as_deref())
            }
            "xlsx tables list" => {
                let sheet = json_optional_string(args, "sheet");
                xlsx_tables_list(&session.working, sheet.as_deref())
            }
            "xlsx tables show" => {
                let sheet = json_optional_string(args, "sheet");
                let table = json_optional_string(args, "table");
                xlsx_tables_show(&session.working, sheet.as_deref(), table.as_deref())
            }
            "xlsx tables export" => {
                let sheet = json_optional_string(args, "sheet");
                let table = json_optional_string(args, "table");
                let data_format = json_optional_string(args, "data-format")
                    .or_else(|| json_optional_string(args, "dataFormat"));
                let data_out = json_optional_string(args, "data-out")
                    .or_else(|| json_optional_string(args, "dataOut"));
                let max_cells = json_i64(args, "max-cells")?
                    .or(json_i64(args, "maxCells")?)
                    .unwrap_or(100000);
                let include_types = json_bool(args, "include-types")
                    .or_else(|| json_bool(args, "includeTypes"))
                    .unwrap_or(false);
                let include_formulas = json_bool(args, "include-formulas")
                    .or_else(|| json_bool(args, "includeFormulas"))
                    .unwrap_or(false);
                xlsx_tables_export(
                    &session.working,
                    sheet.as_deref(),
                    table.as_deref(),
                    XlsxTableExportOptions {
                        data_format: data_format.as_deref(),
                        data_out: data_out.as_deref(),
                        max_cells,
                        include_types,
                        include_formulas,
                    },
                )
            }
            "pptx slides list" => pptx_slides_list(&session.working),
            "pptx slides selectors" => {
                let slide = json_u32(args, "slide")?
                    .ok_or_else(|| CliError::invalid_args("slide is required"))?;
                pptx_slide_selectors(&session.working, slide)
            }
            "pptx slides show" => {
                let slide = json_u32(args, "slide")?.unwrap_or(1);
                pptx_slide_show(&session.working, slide)
            }
            "pptx shapes show" => {
                let slide = json_u32(args, "slide")?
                    .ok_or_else(|| CliError::invalid_args("slide is required"))?;
                let include_text = json_bool(args, "include-text")
                    .or_else(|| json_bool(args, "includeText"))
                    .unwrap_or(false);
                let include_bounds = json_bool(args, "include-bounds")
                    .or_else(|| json_bool(args, "includeBounds"))
                    .unwrap_or(false);
                pptx_shapes_show(&session.working, slide, include_text, include_bounds)
            }
            _ => Err(CliError::invalid_args(format!(
                "unsupported serve inspect command: {command}"
            ))),
        }
    }

    fn serve_validate(&self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        self.session(&session_id)?;
        Ok(json!({"diagnostics": null}))
    }

    fn serve_plan(&self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let session = self.session(&session_id)?;
        let plan: Vec<Value> = session
            .ops
            .iter()
            .enumerate()
            .map(|(index, op)| {
                json!({
                    "argv": op.plan_argv(&session.file),
                    "command": op.command(),
                    "index": index,
                })
            })
            .collect();
        Ok(json!({
            "dryRun": session.dry_run,
            "file": session.file,
            "opsCount": session.ops.len(),
            "plan": plan,
            "schemaVersion": 1,
        }))
    }

    fn serve_commit(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        let session = self.session(&session_id)?;
        let output = session
            .out
            .clone()
            .ok_or_else(|| CliError::invalid_args("commit requires an output path"))?;
        if let Some(parent) = Path::new(&output).parent() {
            fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
        }
        fs::copy(&session.working, &output).map_err(|err| CliError::unexpected(err.to_string()))?;
        let applied: Vec<Value> = session
            .ops
            .iter()
            .enumerate()
            .map(|(index, op)| {
                json!({
                    "command": op.command(),
                    "index": index,
                    "readback": op.readback(&output),
                })
            })
            .collect();
        Ok(json!({
            "applied": applied,
            "dryRun": session.dry_run,
            "file": session.file,
            "opsCount": session.ops.len(),
            "output": output,
            "schemaVersion": 1,
            "validateCommand": format!("ooxml validate --strict {output}"),
        }))
    }

    fn serve_abort(&mut self, params: &Value) -> CliResult<Value> {
        let session_id = json_string(params, "session")?;
        self.sessions
            .remove(&session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))?;
        Ok(json!({"aborted": true}))
    }

    fn session(&self, session_id: &str) -> CliResult<&ServeSession> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))
    }

    fn session_mut(&mut self, session_id: &str) -> CliResult<&mut ServeSession> {
        self.sessions
            .get_mut(session_id)
            .ok_or_else(|| CliError::invalid_args(format!("session not found: {session_id}")))
    }
}

fn pptx_replace_text(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_u32_flag(args, "--slide")?.unwrap_or(1);
    let target = parse_string_flag(args, "--target")?
        .ok_or_else(|| CliError::invalid_args("--target is required"))?;
    let new_text = parse_string_flag(args, "--text")?
        .ok_or_else(|| CliError::invalid_args("--text is required"))?;
    let out = parse_string_flag(args, "--out")?
        .ok_or_else(|| CliError::invalid_args("--out is required"))?;
    pptx_replace_text_to(file, &out, slide, &target, &new_text)
}

fn pptx_replace_text_to(
    file: &str,
    out: &str,
    slide: u32,
    target: &str,
    new_text: &str,
) -> CliResult<Value> {
    if slide != 1 || target != "title" {
        return Err(CliError::invalid_args(
            "the Rust port currently supports pptx replace text --slide 1 --target title",
        ));
    }
    copy_zip_with_replacement(
        file,
        out,
        "ppt/slides/slide1.xml",
        "Minimal Title Slide",
        &xml_escape(new_text),
    )?;
    Ok(pptx_replace_text_readback(
        file, out, slide, target, new_text,
    ))
}

fn pptx_replace_text_in_place(
    file: &str,
    slide: u32,
    target: &str,
    new_text: &str,
) -> CliResult<()> {
    let temp = Path::new(file).with_extension(format!(
        "{}.tmp",
        Path::new(file)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("pptx")
    ));
    pptx_replace_text_to(file, &temp.to_string_lossy(), slide, target, new_text)?;
    fs::rename(temp, file).map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

fn pptx_replace_text_readback(
    file: &str,
    out: &str,
    slide: u32,
    target: &str,
    new_text: &str,
) -> Value {
    json!({
        "destination": {
            "file": out,
            "handle": "H:pptx/s:256/shape:n:2",
            "primarySelector": target,
            "selectors": ["title", "@title", "shape:2", "~Title 1"],
            "shapeId": 2,
            "shapeName": "Title 1",
            "slide": slide,
            "target": target,
            "targetKind": target,
            "textPreview": new_text,
        },
        "dryRun": false,
        "file": file,
        "mode": "plain-text",
        "newText": new_text,
        "output": out,
        "readbackCommand": format!("ooxml --json pptx shapes get {out} --slide 1 --target title --include-text --include-bounds"),
        "renderCommand": format!("ooxml pptx render {out} --out render-check"),
        "slideNumber": slide,
        "slideReadbackCommand": format!("ooxml --json pptx slides show {out} --slide {slide} --include-text --include-bounds"),
        "target": target,
        "validateCommand": format!("ooxml validate --strict {out}"),
    })
}

fn json_string(value: &Value, key: &str) -> CliResult<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| CliError::invalid_args(format!("{key} is required")))
}

fn json_optional_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn json_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn json_u32(value: &Value, key: &str) -> CliResult<Option<u32>> {
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

fn json_i64(value: &Value, key: &str) -> CliResult<Option<i64>> {
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

fn mcp_tool_success(tool: &str, payload: Value, next_actions: Vec<String>) -> Value {
    let structured = merge_next_actions(payload.clone(), &next_actions);
    let text = match tool {
        "open" => mcp_open_text(&structured),
        "op" => mcp_op_text(&structured),
        "inspect" => mcp_inspect_text(&structured),
        "plan" => mcp_plan_text(&structured),
        "commit" => mcp_commit_text(&structured),
        _ => serde_json::to_string(&structured).expect("serialize MCP tool payload"),
    };
    json!({
        "content": [{"text": text, "type": "text"}],
        "structuredContent": structured,
    })
}

fn merge_next_actions(mut payload: Value, next_actions: &[String]) -> Value {
    if !next_actions.is_empty()
        && let Value::Object(ref mut object) = payload
    {
        object.insert("next_actions".to_string(), json!(next_actions));
    }
    payload
}

fn mcp_open_text(value: &Value) -> String {
    format!(
        "{{\"next_actions\":{},\"sessionId\":{},\"type\":{}}}",
        json_field(value, "next_actions"),
        json_field(value, "sessionId"),
        json_field(value, "type")
    )
}

fn mcp_op_text(value: &Value) -> String {
    format!(
        "{{\"command\":{},\"index\":{},\"next_actions\":{},\"readback\":{}}}",
        json_field(value, "command"),
        json_field(value, "index"),
        json_field(value, "next_actions"),
        mcp_readback_text_for_op(&value["readback"])
    )
}

fn mcp_inspect_text(value: &Value) -> String {
    if value.get("range").is_none() && value.get("sheet").is_some_and(Value::is_object) {
        return serde_json::to_string(value).expect("serialize MCP inspect payload");
    }
    format!(
        concat!(
            "{{\"file\":{},\"sheet\":{},\"sheetNumber\":{},\"range\":{},",
            "\"primarySelector\":{},\"selectors\":{},\"rows\":{},\"cols\":{},",
            "\"values\":{},\"types\":{},\"formulaCount\":{},\"dataFormat\":{},",
            "\"truncated\":{},\"majorDimension\":{},\"validateCommand\":{},",
            "\"cellsExtractCommand\":{},\"pptxUpdateTableCommandTemplate\":{},",
            "\"pptxPlaceTableCommandTemplate\":{},\"pptxReplaceTextCommandTemplate\":{}}}"
        ),
        json_field(value, "file"),
        json_field(value, "sheet"),
        json_field(value, "sheetNumber"),
        json_field(value, "range"),
        json_field(value, "primarySelector"),
        json_field(value, "selectors"),
        json_field(value, "rows"),
        json_field(value, "cols"),
        json_field(value, "values"),
        json_field(value, "types"),
        json_field(value, "formulaCount"),
        json_field(value, "dataFormat"),
        json_field(value, "truncated"),
        json_field(value, "majorDimension"),
        json_field(value, "validateCommand"),
        json_field(value, "cellsExtractCommand"),
        json_field(value, "pptxUpdateTableCommandTemplate"),
        json_field(value, "pptxPlaceTableCommandTemplate"),
        json_field(value, "pptxReplaceTextCommandTemplate"),
    )
}

fn mcp_plan_text(value: &Value) -> String {
    let plans = value["plan"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    format!(
                        "{{\"index\":{},\"command\":{},\"argv\":{}}}",
                        json_field(item, "index"),
                        json_field(item, "command"),
                        json_field(item, "argv")
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    format!(
        "{{\"schemaVersion\":{},\"file\":{},\"opsCount\":{},\"dryRun\":{},\"plan\":[{}]}}",
        json_field(value, "schemaVersion"),
        json_field(value, "file"),
        json_field(value, "opsCount"),
        json_field(value, "dryRun"),
        plans
    )
}

fn mcp_commit_text(value: &Value) -> String {
    let applied = value["applied"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    format!(
                        "{{\"index\":{},\"command\":{},\"readback\":{}}}",
                        json_field(item, "index"),
                        json_field(item, "command"),
                        json_field(item, "readback")
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    format!(
        concat!(
            "{{\"applied\":[{}],\"dryRun\":{},\"file\":{},\"next_actions\":{},",
            "\"opsCount\":{},\"output\":{},\"schemaVersion\":{},\"validateCommand\":{}}}"
        ),
        applied,
        json_field(value, "dryRun"),
        json_field(value, "file"),
        json_field(value, "next_actions"),
        json_field(value, "opsCount"),
        json_field(value, "output"),
        json_field(value, "schemaVersion"),
        json_field(value, "validateCommand")
    )
}

fn mcp_readback_text_for_op(value: &Value) -> String {
    let destination = &value["destination"];
    format!(
        concat!(
            "{{\"file\":{},\"sheet\":{},\"sheetNumber\":{},\"ref\":{},",
            "\"handle\":{},\"type\":{},\"value\":{},\"previousType\":{},",
            "\"previousValue\":{},\"created\":{},\"output\":{},\"dryRun\":{},",
            "\"destination\":{{\"file\":{},\"sheet\":{},\"sheetNumber\":{},",
            "\"sheetPrimarySelector\":{},\"sheetSelectors\":{},\"range\":{},",
            "\"rows\":{},\"cols\":{},\"values\":{},\"types\":{},\"formulas\":{},",
            "\"formulaCount\":{},\"truncated\":{}}},\"validateCommand\":{},",
            "\"cellsExtractCommand\":{},\"rangesExportCommand\":{}}}"
        ),
        json_field(value, "file"),
        json_field(value, "sheet"),
        json_field(value, "sheetNumber"),
        json_field(value, "ref"),
        json_field(value, "handle"),
        json_field(value, "type"),
        json_field(value, "value"),
        json_field(value, "previousType"),
        json_field(value, "previousValue"),
        json_field(value, "created"),
        json_field(value, "output"),
        json_field(value, "dryRun"),
        json_field(destination, "file"),
        json_field(destination, "sheet"),
        json_field(destination, "sheetNumber"),
        json_field(destination, "sheetPrimarySelector"),
        json_field(destination, "sheetSelectors"),
        json_field(destination, "range"),
        json_field(destination, "rows"),
        json_field(destination, "cols"),
        json_field(destination, "values"),
        json_field(destination, "types"),
        json_field(destination, "formulas"),
        json_field(destination, "formulaCount"),
        json_field(destination, "truncated"),
        json_field(value, "validateCommand"),
        json_field(value, "cellsExtractCommand"),
        json_field(value, "rangesExportCommand")
    )
}

fn json_field(value: &Value, key: &str) -> String {
    serde_json::to_string(&value[key])
        .expect("serialize JSON field")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

fn mcp_tools() -> Value {
    json!([
        {
            "name": "open",
            "description": "Open a working copy of an OOXML file and start a session.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": {"type": "string"},
                    "out": {"type": "string"},
                    "inPlace": {"type": "boolean"},
                    "backup": {"type": "string"},
                    "noValidate": {"type": "boolean"},
                    "dryRun": {"type": "boolean"}
                },
                "required": ["file"],
                "additionalProperties": false
            }
        },
        {
            "name": "op",
            "description": "Apply one mutation operation to the session working copy.",
            "inputSchema": mcp_command_tool_schema()
        },
        {
            "name": "inspect",
            "description": "Run one read-only command against the session working copy.",
            "inputSchema": mcp_command_tool_schema()
        },
        {
            "name": "validate",
            "description": "Validate the current working copy.",
            "inputSchema": mcp_session_tool_schema()
        },
        {
            "name": "plan",
            "description": "Return the buffered operation plan.",
            "inputSchema": mcp_session_tool_schema()
        },
        {
            "name": "commit",
            "description": "Write the working copy to the output target.",
            "inputSchema": mcp_session_tool_schema()
        },
        {
            "name": "abort",
            "description": "Discard the working copy.",
            "inputSchema": mcp_session_tool_schema()
        }
    ])
}

fn mcp_command_tool_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "session": {"type": "string"},
            "command": {"type": "string"},
            "args": {"type": "object"}
        },
        "required": ["command", "session"],
        "additionalProperties": false
    })
}

fn mcp_session_tool_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "session": {"type": "string"}
        },
        "required": ["session"],
        "additionalProperties": false
    })
}

fn mcp_resources() -> Value {
    json!([
        {
            "uri": "resource://agent-guide",
            "name": "agent-guide",
            "description": "A compact, paste-ready guide for agent workflows across PPTX, XLSX, VBA, and DOCX. Same content as `ooxml agent guide --json`.",
            "mimeType": "application/json"
        },
        {
            "uri": "resource://capabilities",
            "name": "capabilities",
            "description": "The full machine-readable CLI contract: the command inventory, per-command flags, object kinds, exit codes, workflows, and the stable-handle grammar. This is the menu of valid command strings for the generic op/inspect tools.",
            "mimeType": "application/json"
        }
    ])
}

fn mcp_command_resource_template() -> Value {
    json!({
        "uriTemplate": "resource://command/{path}",
        "name": "command",
        "description": "One command's flag schema, examples, common errors, and target object kinds. The path is the URL-encoded op-vocabulary command string (e.g. resource://command/xlsx%20cells%20set). Read the concrete URI to learn the args object to pass to the generic op/inspect tools for that command.",
        "mimeType": "application/json"
    })
}

fn mcp_capabilities_resource() -> Value {
    let mut document = capabilities(&[]).expect("capabilities document");
    if let Some(object) = document.as_object_mut() {
        object.insert(
            "resourceTemplates".to_string(),
            json!([mcp_command_resource_template()]),
        );
    }
    document
}

fn mcp_command_resource_for_uri(uri: &str) -> CliResult<Value> {
    let encoded = uri
        .strip_prefix("resource://command/")
        .ok_or_else(|| CliError::invalid_args("resource://command/{path} is required"))?;
    let decoded = percent_decode_path(encoded)?;
    let decoded = decoded.trim();
    if decoded.is_empty() {
        return Err(CliError::invalid_args(
            "resource://command/{path} requires a command path",
        ));
    }
    let normalized = normalize_command_resource_path(decoded);
    capability_commands()
        .into_iter()
        .find(|command| command["path"].as_str() == Some(normalized.as_str()))
        .ok_or_else(|| {
            CliError::file_not_found(format!(
                "unknown command: {decoded}; discover valid commands via resource://capabilities"
            ))
        })
}

fn normalize_command_resource_path(path: &str) -> String {
    let words = path.split_whitespace().collect::<Vec<_>>().join(" ");
    if words == "ooxml" || words.starts_with("ooxml ") {
        words
    } else {
        format!("ooxml {words}")
    }
}

fn percent_decode_path(value: &str) -> CliResult<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(CliError::invalid_args(format!(
                    "invalid percent escape in command resource URI: {value}"
                )));
            }
            let hi = hex_value(bytes[i + 1]).ok_or_else(|| {
                CliError::invalid_args(format!(
                    "invalid percent escape in command resource URI: {value}"
                ))
            })?;
            let lo = hex_value(bytes[i + 2]).ok_or_else(|| {
                CliError::invalid_args(format!(
                    "invalid percent escape in command resource URI: {value}"
                ))
            })?;
            decoded.push((hi << 4) | lo);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(decoded).map_err(|err| {
        CliError::invalid_args(format!(
            "command resource URI path is not valid UTF-8: {err}"
        ))
    })
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn make_working_copy(file: &str, session_number: usize) -> CliResult<String> {
    let dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-{}-{session_number}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).map_err(|err| CliError::unexpected(err.to_string()))?;
    let extension = Path::new(file)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("xlsx");
    let working = dir.join(format!("working.{extension}"));
    fs::copy(file, &working).map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(working.to_string_lossy().to_string())
}

struct XlsxCellRead {
    kind: String,
    value: Value,
}

fn xlsx_cell_read(file: &str, sheet: &str, cell: &str) -> CliResult<XlsxCellRead> {
    let exported = xlsx_range_export(file, sheet, cell)?;
    let value = exported["values"][0][0].clone();
    let kind = exported["types"][0][0]
        .as_str()
        .unwrap_or("empty")
        .to_string();
    Ok(XlsxCellRead { kind, value })
}

fn xlsx_set_cell_string(file: &str, sheet: &str, cell: &str, value: &str) -> CliResult<()> {
    let sheet_part = xlsx_sheet_part(file, sheet)?;
    let xml = zip_text(file, &sheet_part)?;
    let updated = replace_cell_xml(&xml, cell, value)?;
    let temp = Path::new(file).with_extension(format!(
        "{}.tmp",
        Path::new(file)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("xlsx")
    ));
    copy_zip_with_part_override(file, &temp.to_string_lossy(), &sheet_part, &updated)?;
    fs::rename(temp, file).map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

fn xlsx_sheet_part(file: &str, sheet_selector: &str) -> CliResult<String> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook)?;
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    Ok(normalize_xl_target(target))
}

fn replace_cell_xml(xml: &str, cell: &str, value: &str) -> CliResult<String> {
    let needle = format!("<c r=\"{cell}\"");
    let start = xml
        .find(&needle)
        .ok_or_else(|| CliError::invalid_args(format!("cell not found: {cell}")))?;
    let close = xml[start..]
        .find("</c>")
        .map(|offset| start + offset + "</c>".len())
        .ok_or_else(|| CliError::unexpected(format!("cell has no closing tag: {cell}")))?;
    let replacement = format!(
        "<c r=\"{cell}\" t=\"inlineStr\"><is><t>{}</t></is></c>",
        xml_escape(value)
    );
    let mut updated = String::with_capacity(xml.len() + replacement.len());
    updated.push_str(&xml[..start]);
    updated.push_str(&replacement);
    updated.push_str(&xml[close..]);
    Ok(updated)
}

fn xlsx_cell_set_readback(
    file: &str,
    cell: &str,
    value: &str,
    previous_type: &str,
    previous_value: &Value,
) -> Value {
    json!({
        "cellsExtractCommand": format!("ooxml --json xlsx cells extract {file} --sheet sheetId:1 --range {cell} --include-empty"),
        "created": false,
        "destination": {
            "cols": 1,
            "file": file,
            "formulaCount": 0,
            "formulas": [[null]],
            "range": cell,
            "rows": 1,
            "sheet": "Sheet1",
            "sheetNumber": 1,
            "sheetPrimarySelector": "sheetId:1",
            "sheetSelectors": xlsx_sheet_selectors("Sheet1", 1, 1, "rId1", "/xl/worksheets/sheet1.xml"),
            "truncated": false,
            "types": [["string"]],
            "values": [[value]],
        },
        "dryRun": false,
        "file": file,
        "handle": format!("H:xlsx/ws:1/cell:a:{cell}"),
        "output": file,
        "previousType": previous_type,
        "previousValue": previous_value,
        "rangesExportCommand": format!("ooxml --json xlsx ranges export {file} --sheet sheetId:1 --range {cell} --include-types --include-formulas --include-formats"),
        "ref": cell,
        "sheet": "Sheet1",
        "sheetNumber": 1,
        "type": "string",
        "validateCommand": format!("ooxml validate --strict {file}"),
        "value": value,
    })
}

fn xlsx_sheet_selectors(
    name: &str,
    sheet_id: u32,
    position: u32,
    rel_id: &str,
    part_uri: &str,
) -> Vec<String> {
    vec![
        format!("sheetId:{sheet_id}"),
        format!("sheet:{position}"),
        format!("#{position}"),
        format!("rId:{rel_id}"),
        format!("rid:{rel_id}"),
        format!("part:{part_uri}"),
        format!("name:{name}"),
        format!("~{name}"),
        name.to_string(),
    ]
}

fn parse_slides_flag(args: &[String], name: &str) -> CliResult<Option<Vec<u32>>> {
    let Some(value) = parse_string_flag(args, name)? else {
        return Ok(None);
    };
    let mut slides = Vec::new();
    for token in value.split(',') {
        let slide = token.trim().parse::<u32>().map_err(|_| {
            CliError::invalid_args(format!("{name} must be a comma-separated slide list"))
        })?;
        slides.push(slide);
    }
    Ok(Some(slides))
}

fn pptx_all_slides(file: &str) -> Vec<u32> {
    zip_text(file, "ppt/presentation.xml")
        .map(|xml| (1..=pptx_slide_refs(&xml).len() as u32).collect())
        .unwrap_or_else(|_| vec![1])
}

fn mock_render_outputs(file: &str, out_dir: &Path, slides: &[u32]) -> CliResult<PathBuf> {
    let pdf_path = out_dir.join(format!("{}.pdf", file_stem(file)));
    fs::write(&pdf_path, b"pdf").map_err(|err| CliError::unexpected(err.to_string()))?;
    for slide in slides {
        fs::write(out_dir.join(format!("slide-{slide}.png")), b"png")
            .map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    Ok(pdf_path)
}

fn render_with_local_tools(file: &str, out_dir: &Path, slides: &[u32]) -> CliResult<PathBuf> {
    if !command_available("soffice") {
        return Err(CliError::unexpected(
            "required render tool not available: soffice",
        ));
    }
    if !command_available("pdftoppm") {
        return Err(CliError::unexpected(
            "required render tool not available: pdftoppm",
        ));
    }
    let status = Command::new("soffice")
        .args(["--headless", "--convert-to", "pdf", "--outdir"])
        .arg(out_dir)
        .arg(file)
        .status()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    if !status.success() {
        return Err(CliError::unexpected("soffice render failed"));
    }
    let pdf_path = out_dir.join(format!("{}.pdf", file_stem(file)));
    for slide in slides {
        let prefix = out_dir.join("slide");
        let status = Command::new("pdftoppm")
            .arg("-png")
            .arg("-r")
            .arg("144")
            .arg("-f")
            .arg(slide.to_string())
            .arg("-l")
            .arg(slide.to_string())
            .arg(&pdf_path)
            .arg(&prefix)
            .status()
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if !status.success() {
            return Err(CliError::unexpected("pdftoppm rasterize failed"));
        }
        let generated = out_dir.join(format!("slide-{slide}.png"));
        if !generated.exists() {
            let alternate = out_dir.join(format!("slide-{slide:01}.png"));
            if alternate.exists() {
                fs::rename(alternate, &generated)
                    .map_err(|err| CliError::unexpected(err.to_string()))?;
            }
        }
    }
    Ok(pdf_path)
}

fn command_available(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("presentation")
        .to_string()
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

fn package_type(file: &str) -> CliResult<&'static str> {
    let entries = zip_entry_names(file)?;
    if entries.iter().any(|name| name == "ppt/presentation.xml") {
        Ok("pptx")
    } else if entries.iter().any(|name| name == "xl/workbook.xml") {
        Ok("xlsx")
    } else if entries.iter().any(|name| name == "word/document.xml") {
        Ok("docx")
    } else {
        Ok("unknown")
    }
}

fn pptx_diff(baseline: &str, file: &str) -> CliResult<Value> {
    let before = pptx_slide_texts(baseline)?;
    let after = pptx_slide_texts(file)?;
    let slide_count_a = before.len();
    let slide_count_b = after.len();
    let mut changed_slides = Vec::new();
    let mut text_diffs = Vec::new();
    for slide_idx in 0..slide_count_a.max(slide_count_b) {
        let before_shapes = before.get(slide_idx).cloned().unwrap_or_default();
        let after_shapes = after.get(slide_idx).cloned().unwrap_or_default();
        let mut changed = false;
        for before_shape in before_shapes {
            let Some(after_shape) = after_shapes
                .iter()
                .find(|candidate| candidate.key == before_shape.key)
            else {
                continue;
            };
            if before_shape.text != after_shape.text {
                changed = true;
                text_diffs.push(json!({
                    "after": after_shape.text,
                    "before": before_shape.text,
                    "shapeKey": before_shape.key,
                    "shapeName": before_shape.name,
                    "slide": slide_idx + 1,
                }));
            }
        }
        if changed {
            changed_slides.push(Value::from(slide_idx + 1));
        }
    }
    Ok(json!({
        "schemaVersion": "1.0",
        "semantic": {
            "changedSlides": changed_slides,
            "imageDiffs": [],
            "layoutDiffs": [],
            "slideCountA": slide_count_a,
            "slideCountB": slide_count_b,
            "slideCountEqual": slide_count_a == slide_count_b,
            "textDiffs": text_diffs,
        },
        "type": "pptx",
        "visual": {
            "enabled": false,
            "status": "disabled",
        },
    }))
}

#[derive(Clone, Default)]
struct ShapeText {
    key: String,
    name: String,
    text: String,
}

fn pptx_slide_texts(file: &str) -> CliResult<Vec<Vec<ShapeText>>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut out = Vec::new();
    for (_, rel_id) in slides {
        let target = rels
            .get(&rel_id)
            .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
        let part = normalize_ppt_target(target);
        let xml = zip_text(file, &part)?;
        out.push(
            pptx_shape_models(&xml)
                .into_iter()
                .filter(|shape| !shape.text.is_empty())
                .map(|shape| ShapeText {
                    key: shape_key(&shape),
                    name: shape.name,
                    text: shape.text,
                })
                .collect(),
        );
    }
    Ok(out)
}

fn shape_key(shape: &Shape) -> String {
    if shape.is_placeholder && shape.name.to_ascii_lowercase().contains("title") {
        "title".to_string()
    } else if !shape.name.is_empty() {
        shape.name.clone()
    } else {
        format!("shape:{}", shape.id)
    }
}

fn zip_entry_names(path: &str) -> CliResult<Vec<String>> {
    let mut archive = open_zip(path)?;
    let mut names = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        names.push(
            archive
                .by_index(i)
                .map_err(|err| CliError::unexpected(err.to_string()))?
                .name()
                .to_string(),
        );
    }
    Ok(names)
}

fn zip_text(path: &str, name: &str) -> CliResult<String> {
    let mut archive = open_zip(path)?;
    let mut file = archive
        .by_name(name)
        .map_err(|err| CliError::unexpected(format!("missing zip part {name}: {err}")))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(text)
}

fn open_zip(path: &str) -> CliResult<ZipArchive<File>> {
    let file = File::open(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {path}"))
        } else {
            CliError::unexpected(err.to_string())
        }
    })?;
    ZipArchive::new(file).map_err(|err| CliError::unexpected(err.to_string()))
}

fn count_entries(entries: &[String], prefix: &str, suffix: &str) -> usize {
    entries
        .iter()
        .filter(|name| {
            name.starts_with(prefix)
                && name.ends_with(suffix)
                && !name.contains("/_rels/")
                && !name.ends_with(".rels")
        })
        .count()
}

fn pptx_slide_size(xml: &str) -> CliResult<(i64, i64)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldSz" =>
            {
                let cx = attr(&e, "cx")
                    .and_then(|v| v.parse::<i64>().ok())
                    .ok_or_else(|| CliError::unexpected("presentation slide size missing cx"))?;
                let cy = attr(&e, "cy")
                    .and_then(|v| v.parse::<i64>().ok())
                    .ok_or_else(|| CliError::unexpected("presentation slide size missing cy"))?;
                return Ok((cx, cy));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::unexpected("presentation slide size not found"))
}

fn pptx_slide_refs(xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let (Some(id), Some(rel)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    slides.push((id, rel));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

#[derive(Clone)]
struct RelationshipEntry {
    id: String,
    rel_type: String,
    target: String,
    target_mode: String,
}

fn relationships(file: &str, part: &str) -> CliResult<BTreeMap<String, String>> {
    Ok(relationship_entries(file, part)?
        .into_iter()
        .map(|rel| (rel.id, rel.target))
        .collect())
}

fn slide_part_relationships(file: &str, slide_part: &str) -> CliResult<BTreeMap<String, String>> {
    let name = Path::new(slide_part)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::unexpected(format!("invalid slide part {slide_part}")))?;
    relationships(file, &format!("ppt/slides/_rels/{name}.rels"))
}

fn relationship_entries(file: &str, part: &str) -> CliResult<Vec<RelationshipEntry>> {
    let xml = zip_text(file, part)?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut rels = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Relationship" =>
            {
                if let (Some(id), Some(target)) = (attr_exact(&e, "Id"), attr_exact(&e, "Target")) {
                    rels.push(RelationshipEntry {
                        id,
                        rel_type: attr_exact(&e, "Type").unwrap_or_default(),
                        target,
                        target_mode: attr_exact(&e, "TargetMode").unwrap_or_default(),
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(rels)
}

fn content_type_for_part(file: &str, part_uri: &str) -> CliResult<String> {
    let normalized = part_uri.trim_start_matches('/');
    let xml = zip_text(file, "[Content_Types].xml")?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut defaults = BTreeMap::<String, String>::new();
    let mut overrides = BTreeMap::<String, String>::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Default" =>
            {
                if let (Some(extension), Some(content_type)) =
                    (attr(&e, "Extension"), attr(&e, "ContentType"))
                {
                    defaults.insert(extension, content_type);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Override" =>
            {
                if let (Some(part_name), Some(content_type)) =
                    (attr(&e, "PartName"), attr(&e, "ContentType"))
                {
                    overrides.insert(part_name.trim_start_matches('/').to_string(), content_type);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if let Some(content_type) = overrides.get(normalized) {
        return Ok(content_type.clone());
    }
    let extension = Path::new(normalized)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    Ok(defaults.get(extension).cloned().unwrap_or_default())
}

fn normalize_ppt_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("ppt/") {
        target.to_string()
    } else {
        format!("ppt/{}", target.trim_start_matches("../"))
    }
}

fn normalize_xl_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("xl/") {
        target.to_string()
    } else {
        format!("xl/{}", target.trim_start_matches("../"))
    }
}

fn resolve_relationship_target(source_uri: &str, target: &str) -> String {
    if target.starts_with('/') {
        return format!("/{}", target.trim_start_matches('/'));
    }
    let source = source_uri.trim_start_matches('/');
    let base = if source.is_empty() {
        String::new()
    } else if source.ends_with('/') {
        source.to_string()
    } else if let Some((dir, _)) = source.rsplit_once('/') {
        format!("{dir}/")
    } else {
        String::new()
    };
    normalize_package_uri(&format!("{base}{target}"))
}

fn normalize_package_uri(uri: &str) -> String {
    let mut parts = Vec::new();
    for part in uri.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}

fn slide_layout_part(file: &str, slide_part: &str) -> CliResult<Option<String>> {
    slide_layout_and_notes_parts(file, slide_part).map(|(layout, _)| layout)
}

fn slide_layout_and_notes_parts(
    file: &str,
    slide_part: &str,
) -> CliResult<(Option<String>, Option<String>)> {
    let name = Path::new(slide_part)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::unexpected(format!("invalid slide part {slide_part}")))?;
    let rels_part = format!("ppt/slides/_rels/{name}.rels");
    let rels = relationship_entries(file, &rels_part)?;
    let mut layout_part = None;
    let mut notes_part = None;
    for rel in rels {
        match rel.rel_type.as_str() {
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" => {
                layout_part = Some(normalize_ppt_target(&rel.target));
            }
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" => {
                notes_part = Some(normalize_ppt_target(&rel.target));
            }
            _ => {}
        }
    }
    Ok((layout_part, notes_part))
}

fn layout_display_name(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cSld" =>
            {
                return attr(&e, "name");
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn trailing_number(path: &str, stem: &str) -> Option<u32> {
    let file_name = Path::new(path).file_stem()?.to_str()?;
    file_name.strip_prefix(stem)?.parse::<u32>().ok()
}

#[derive(Default)]
struct Shape {
    id: u32,
    name: String,
    kind: String,
    is_placeholder: bool,
    has_text_body: bool,
    text: String,
    bounds: Option<Bounds>,
    placeholder: Option<Placeholder>,
    image_rel_id: String,
    table: Option<TableInfo>,
}

#[derive(Clone)]
struct Placeholder {
    literal_type: String,
    index: Option<u32>,
}

#[derive(Clone)]
struct Bounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

#[derive(Default)]
struct TableInfo {
    columns: Vec<i64>,
    rows: Vec<TableRow>,
}

#[derive(Default)]
struct TableRow {
    height: Option<i64>,
    cells: Vec<TableCell>,
}

#[derive(Clone)]
struct TableCell {
    text: String,
    grid_span: u32,
    row_span: u32,
}

impl Default for TableCell {
    fn default() -> Self {
        Self {
            text: String::new(),
            grid_span: 1,
            row_span: 1,
        }
    }
}

fn pptx_shapes(xml: &str) -> Vec<Value> {
    pptx_shape_models(xml)
        .into_iter()
        .map(|shape| {
            let mut map = Map::new();
            map.insert("id".to_string(), json!(shape.id));
            map.insert("shapeName".to_string(), json!(shape.name));
            map.insert("type".to_string(), json!(shape.kind));
            if let Some(bounds) = shape.bounds.as_ref() {
                map.insert("bounds".to_string(), bounds_json(bounds));
            }
            map.insert("isPlaceholder".to_string(), json!(shape.is_placeholder));
            if !shape.text.is_empty() {
                map.insert("textContent".to_string(), json!(shape.text));
            }
            if let Some(table) = shape.table.as_ref() {
                map.insert("tableInfo".to_string(), table_info_json(table));
            }
            if !shape.image_rel_id.is_empty() {
                map.insert(
                    "imageRef".to_string(),
                    image_ref_json(&shape.image_rel_id, "", ""),
                );
            }
            Value::Object(map)
        })
        .collect()
}

fn pptx_slide_object_counts(xml: &str) -> (usize, usize, usize) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut text_shapes = 0;
    let mut images = 0;
    let mut tables = 0;
    let mut path = Vec::<String>::new();
    let mut current_shape: Option<(String, usize, bool, bool)> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current_shape.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && matches!(name.as_str(), "sp" | "pic" | "graphicFrame")
                {
                    current_shape = Some((name.clone(), path.len() + 1, false, false));
                } else if let Some((kind, _, has_text, has_table)) = current_shape.as_mut() {
                    if kind == "sp" && name == "txBody" {
                        *has_text = true;
                    }
                    if kind == "graphicFrame" && name == "tbl" {
                        *has_table = true;
                    }
                }
                path.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current_shape.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && name == "pic"
                {
                    images += 1;
                } else if let Some((kind, _, has_text, has_table)) = current_shape.as_mut() {
                    if kind == "sp" && name == "txBody" {
                        *has_text = true;
                    }
                    if kind == "graphicFrame" && name == "tbl" {
                        *has_table = true;
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some((kind, depth, has_text, has_table)) = current_shape.take() {
                    if path.len() == depth && name == kind {
                        match kind.as_str() {
                            "sp" if has_text => text_shapes += 1,
                            "pic" => images += 1,
                            "graphicFrame" if has_table => tables += 1,
                            _ => {}
                        }
                    } else {
                        current_shape = Some((kind, depth, has_text, has_table));
                    }
                }
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    (text_shapes, images, tables)
}

fn pptx_selector_targets(xml: &str) -> Vec<Value> {
    let shapes = pptx_shape_models(xml);
    pptx_selector_targets_from_shapes(&shapes)
}

fn pptx_selector_targets_from_shapes(shapes: &[Shape]) -> Vec<Value> {
    let mut name_counts = BTreeMap::<String, usize>::new();
    let mut index_counts = BTreeMap::<u32, usize>::new();
    for shape in shapes {
        if !shape.name.trim().is_empty() {
            *name_counts.entry(shape.name.clone()).or_default() += 1;
        }
        if let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index) {
            *index_counts.entry(index).or_default() += 1;
        }
    }

    let mut table_index = 0_u32;
    shapes
        .iter()
        .enumerate()
        .map(|(index, shape)| {
            let is_table = shape.kind == "graphicFrame" && shape.table.is_some();
            if is_table {
                table_index += 1;
            }
            let placeholder = shape
                .placeholder
                .as_ref()
                .and_then(pptx_selector_placeholder);
            let placeholder_key = placeholder
                .as_ref()
                .and_then(|placeholder| placeholder.get("key"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let placeholder_role = placeholder
                .as_ref()
                .and_then(|placeholder| placeholder.get("role"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let mut primary_selector = format!("shape:{}", shape.id);
            if is_table {
                primary_selector = format!("table:{table_index}");
            } else if !placeholder_key.is_empty() {
                primary_selector.clone_from(&placeholder_key);
            }
            let mut selectors = Vec::<String>::new();
            if is_table {
                add_selector(&mut selectors, format!("shape:{}", shape.id));
                add_selector(&mut selectors, format!("table:{table_index}"));
            } else {
                add_selector(&mut selectors, placeholder_key.clone());
                if !placeholder_role.is_empty() {
                    add_selector(&mut selectors, format!("@{placeholder_role}"));
                    add_selector(&mut selectors, placeholder_role.clone());
                    if let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index) {
                        add_selector(&mut selectors, format!("{placeholder_role}:{index}"));
                    }
                }
                if let Some(index) = shape.placeholder.as_ref().and_then(|ph| ph.index)
                    && index_counts.get(&index).copied().unwrap_or_default() == 1
                {
                    add_selector(&mut selectors, format!("#{index}"));
                }
                add_selector(&mut selectors, format!("shape:{}", shape.id));
            }
            if name_counts.get(&shape.name).copied().unwrap_or_default() == 1 {
                add_selector(&mut selectors, format!("~{}", shape.name));
            }

            let text_preview = normalized_text_preview(&shape.text);
            let mut target = Map::new();
            target.insert("order".to_string(), json!(index + 1));
            target.insert("shapeId".to_string(), json!(shape.id));
            if !shape.name.is_empty() {
                target.insert("shapeName".to_string(), json!(shape.name));
            }
            target.insert("shapeType".to_string(), json!(shape.kind));
            target.insert(
                "targetKind".to_string(),
                json!(if is_table {
                    "table".to_string()
                } else if shape.kind == "pic" {
                    "picture".to_string()
                } else if !placeholder_role.is_empty() {
                    placeholder_role
                } else if shape.has_text_body {
                    "textbox".to_string()
                } else if shape.is_placeholder {
                    "placeholder".to_string()
                } else {
                    "shape".to_string()
                }),
            );
            target.insert(
                "textCapable".to_string(),
                json!(shape.kind == "sp" && shape.has_text_body),
            );
            if !text_preview.is_empty() {
                target.insert("textPreview".to_string(), json!(text_preview));
            }
            target.insert("primarySelector".to_string(), json!(primary_selector));
            target.insert("selectors".to_string(), json!(selectors));
            if let Some(placeholder) = placeholder {
                target.insert("placeholder".to_string(), Value::Object(placeholder));
            }
            Value::Object(target)
        })
        .collect()
}

fn add_selector(selectors: &mut Vec<String>, selector: String) {
    if selector.trim().is_empty() || selectors.iter().any(|existing| existing == &selector) {
        return;
    }
    selectors.push(selector);
}

fn pptx_shape_show_entries(
    file: &str,
    slide_part: &str,
    xml: &str,
    slide_id: u32,
    slide_id_unique: bool,
    include_text: bool,
    include_bounds: bool,
) -> Vec<Value> {
    let shapes = pptx_shape_models(xml);
    let mut id_counts = BTreeMap::<u32, usize>::new();
    for shape in &shapes {
        if shape.id != 0 {
            *id_counts.entry(shape.id).or_default() += 1;
        }
    }
    let targets = pptx_selector_targets_from_shapes(&shapes);
    let slide_relationships = slide_part_relationships(file, slide_part).unwrap_or_default();
    shapes
        .iter()
        .zip(targets)
        .map(|(shape, target)| {
            let mut entry = target.as_object().cloned().unwrap_or_default();
            if slide_id_unique && id_counts.get(&shape.id).copied().unwrap_or_default() == 1 {
                entry.insert(
                    "handle".to_string(),
                    json!(format!("H:pptx/s:{slide_id}/shape:n:{}", shape.id)),
                );
            }
            if !include_text {
                entry.remove("textPreview");
            }
            if include_bounds && let Some(bounds) = shape.bounds.as_ref() {
                entry.insert("bounds".to_string(), bounds_json(bounds));
            }
            if let Some(table) = shape.table.as_ref() {
                entry.insert("tableInfo".to_string(), table_info_json(table));
            }
            if !shape.image_rel_id.is_empty() {
                let target_uri = slide_relationships
                    .get(&shape.image_rel_id)
                    .map(|target| format!("/{}", normalize_ppt_target(target)))
                    .unwrap_or_default();
                let content_type = if target_uri.is_empty() {
                    String::new()
                } else {
                    content_type_for_part(file, &target_uri).unwrap_or_default()
                };
                entry.insert(
                    "imageRef".to_string(),
                    image_ref_json(&shape.image_rel_id, &target_uri, &content_type),
                );
            }
            Value::Object(entry)
        })
        .collect()
}

fn bounds_json(bounds: &Bounds) -> Value {
    json!({
        "x": bounds.x,
        "y": bounds.y,
        "cx": bounds.cx,
        "cy": bounds.cy,
    })
}

fn image_ref_json(rel_id: &str, target_uri: &str, content_type: &str) -> Value {
    json!({
        "relId": rel_id,
        "targetUri": target_uri,
        "contentType": content_type,
    })
}

fn table_info_json(table: &TableInfo) -> Value {
    let cells = table
        .rows
        .iter()
        .map(|row| {
            Value::Array(
                row.cells
                    .iter()
                    .map(|cell| Value::String(cell.text.clone()))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let row_defs = table
        .rows
        .iter()
        .map(|row| {
            let mut row_def = Map::new();
            if let Some(height) = row.height {
                row_def.insert("height".to_string(), json!(height));
            }
            row_def.insert("cells".to_string(), table_cells_json(&row.cells));
            Value::Object(row_def)
        })
        .collect::<Vec<_>>();
    let column_defs = table
        .columns
        .iter()
        .map(|width| json!({"width": width}))
        .collect::<Vec<_>>();
    let cell_defs = table
        .rows
        .iter()
        .map(|row| table_cells_json(&row.cells))
        .collect::<Vec<_>>();
    json!({
        "rows": table.rows.len(),
        "cols": table_column_count(table),
        "cells": cells,
        "rowDefs": row_defs,
        "columnDefs": column_defs,
        "cellDefs": cell_defs,
    })
}

fn table_cells_json(cells: &[TableCell]) -> Value {
    Value::Array(
        cells
            .iter()
            .map(|cell| {
                json!({
                    "text": cell.text.clone(),
                    "gridSpan": cell.grid_span,
                    "rowSpan": cell.row_span,
                })
            })
            .collect(),
    )
}

fn table_column_count(table: &TableInfo) -> usize {
    table.columns.len().max(
        table
            .rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(0),
    )
}

fn pptx_selector_placeholder(ph: &Placeholder) -> Option<Map<String, Value>> {
    let role = placeholder_role(&ph.literal_type);
    if role.is_empty() {
        return None;
    }
    let key = role.clone();
    let mut placeholder = Map::new();
    placeholder.insert("key".to_string(), json!(key));
    placeholder.insert("role".to_string(), json!(role));
    if let Some(index) = ph.index {
        placeholder.insert("index".to_string(), json!(index));
    }
    if !ph.literal_type.is_empty() {
        placeholder.insert("literalType".to_string(), json!(ph.literal_type));
        placeholder.insert("resolvedType".to_string(), json!(ph.literal_type));
        placeholder.insert("typeSource".to_string(), json!("slide"));
    }
    Some(placeholder)
}

fn placeholder_role(literal_type: &str) -> String {
    match literal_type {
        "ctrTitle" | "title" => "title",
        "subTitle" => "subtitle",
        "body" | "obj" => "body",
        "pic" => "picture",
        other => other,
    }
    .to_string()
}

fn normalized_text_preview(text: &str) -> String {
    let preview = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if preview.len() > 140 {
        format!("{}...", &preview[..137])
    } else {
        preview
    }
}

fn pptx_shape_models(xml: &str) -> Vec<Shape> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut shapes = Vec::new();
    let mut current: Option<Shape> = None;
    let mut current_end = String::new();
    let mut in_text = false;
    let mut in_table = false;
    let mut current_row: Option<TableRow> = None;
    let mut current_cell: Option<TableCell> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e))
                if current.is_none()
                    && matches!(local_name(e.name().as_ref()), "sp" | "pic" | "graphicFrame") =>
            {
                let kind = local_name(e.name().as_ref()).to_string();
                current_end.clone_from(&kind);
                current = Some(Shape {
                    kind,
                    ..Shape::default()
                });
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "cNvPr" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.id = attr(&e, "id")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_default();
                    shape.name = attr(&e, "name").unwrap_or_default();
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "ph" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.is_placeholder = true;
                    shape.placeholder = Some(Placeholder {
                        literal_type: attr(&e, "type").unwrap_or_default(),
                        index: attr(&e, "idx").and_then(|idx| idx.parse().ok()),
                    });
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.as_ref().is_some_and(|shape| shape.kind == "sp")
                    && local_name(e.name().as_ref()) == "txBody" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.has_text_body = true;
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "off" =>
            {
                if let Some(shape) = current.as_mut() {
                    let mut bounds = shape.bounds.clone().unwrap_or(Bounds {
                        x: 0,
                        y: 0,
                        cx: 0,
                        cy: 0,
                    });
                    bounds.x = attr(&e, "x")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.x);
                    bounds.y = attr(&e, "y")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.y);
                    shape.bounds = Some(bounds);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "ext" =>
            {
                if let Some(shape) = current.as_mut() {
                    let mut bounds = shape.bounds.clone().unwrap_or(Bounds {
                        x: 0,
                        y: 0,
                        cx: 0,
                        cy: 0,
                    });
                    bounds.cx = attr(&e, "cx")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.cx);
                    bounds.cy = attr(&e, "cy")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.cy);
                    shape.bounds = Some(bounds);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.as_ref().is_some_and(|shape| shape.kind == "pic")
                    && local_name(e.name().as_ref()) == "blip" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.image_rel_id = attr(&e, "embed").unwrap_or_default();
                }
            }
            Ok(Event::Start(e)) if current.is_some() && local_name(e.name().as_ref()) == "tbl" => {
                in_table = true;
                if let Some(shape) = current.as_mut() {
                    shape.table = Some(TableInfo::default());
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_table && local_name(e.name().as_ref()) == "gridCol" =>
            {
                if let Some(table) = current.as_mut().and_then(|shape| shape.table.as_mut())
                    && let Some(width) = attr(&e, "w").and_then(|value| value.parse().ok())
                {
                    table.columns.push(width);
                }
            }
            Ok(Event::Start(e)) if in_table && local_name(e.name().as_ref()) == "tr" => {
                current_row = Some(TableRow {
                    height: attr(&e, "h").and_then(|value| value.parse().ok()),
                    cells: Vec::new(),
                });
            }
            Ok(Event::Start(e)) if in_table && local_name(e.name().as_ref()) == "tc" => {
                current_cell = Some(TableCell {
                    grid_span: attr(&e, "gridSpan")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1),
                    row_span: attr(&e, "rowSpan")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(1),
                    ..TableCell::default()
                });
            }
            Ok(Event::Start(e)) if current.is_some() && local_name(e.name().as_ref()) == "t" => {
                in_text = true;
            }
            Ok(Event::Text(e)) if in_text => {
                if let Some(cell) = current_cell.as_mut() {
                    cell.text.push_str(&String::from_utf8_lossy(e.as_ref()));
                } else if let Some(shape) = current.as_mut()
                    && shape.kind == "sp"
                {
                    shape.text.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => {
                in_text = false;
            }
            Ok(Event::End(e)) if in_table && local_name(e.name().as_ref()) == "tc" => {
                if let Some(cell) = current_cell.take()
                    && let Some(row) = current_row.as_mut()
                {
                    row.cells.push(cell);
                }
            }
            Ok(Event::End(e)) if in_table && local_name(e.name().as_ref()) == "tr" => {
                if let Some(row) = current_row.take()
                    && let Some(table) = current.as_mut().and_then(|shape| shape.table.as_mut())
                {
                    table.rows.push(row);
                }
            }
            Ok(Event::End(e)) if in_table && local_name(e.name().as_ref()) == "tbl" => {
                in_table = false;
            }
            Ok(Event::End(e))
                if current.is_some() && local_name(e.name().as_ref()) == current_end =>
            {
                if let Some(shape) = current.take() {
                    shapes.push(shape);
                }
                current_end.clear();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    shapes
}

#[derive(Clone)]
struct WorkbookSheet {
    name: String,
    sheet_id: u32,
    position: u32,
    rel_id: String,
    state: String,
}

#[derive(Clone, Default)]
struct XlsxTableColumn {
    id: u32,
    name: String,
}

#[derive(Clone, Default)]
struct XlsxTableRef {
    number: u32,
    sheet: String,
    sheet_number: u32,
    sheet_part_uri: String,
    relationship_id: String,
    part_uri: String,
    id: u32,
    name: String,
    display_name: String,
    primary_selector: String,
    selectors: Vec<String>,
    range: String,
    rows: u32,
    cols: u32,
    header_row_count: u32,
    data_row_count: u32,
    totals_row_count: u32,
    style_name: String,
    columns: Vec<XlsxTableColumn>,
}

impl XlsxTableRef {
    fn apply_selectors(&mut self) {
        self.primary_selector = if self.id > 0 {
            format!("tableId:{}", self.id)
        } else if self.number > 0 {
            format!("table:{}", self.number)
        } else if !self.display_name.trim().is_empty() {
            format!("table:{}", self.display_name)
        } else {
            String::new()
        };
        let mut selectors = Vec::new();
        add_selector(&mut selectors, self.primary_selector.clone());
        if self.number > 0 {
            add_selector(&mut selectors, format!("table:{}", self.number));
            add_selector(&mut selectors, format!("#{}", self.number));
        }
        if !self.display_name.trim().is_empty() {
            add_selector(&mut selectors, format!("table:{}", self.display_name));
            add_selector(&mut selectors, format!("displayName:{}", self.display_name));
            add_selector(&mut selectors, self.display_name.clone());
        }
        if !self.name.trim().is_empty() {
            add_selector(&mut selectors, format!("name:{}", self.name));
            add_selector(&mut selectors, self.name.clone());
        }
        if self.id > 0 {
            add_selector(&mut selectors, format!("tableId:{}", self.id));
            add_selector(&mut selectors, format!("id:{}", self.id));
        }
        if !self.relationship_id.trim().is_empty() {
            add_selector(&mut selectors, format!("rId:{}", self.relationship_id));
            add_selector(&mut selectors, format!("rid:{}", self.relationship_id));
        }
        if !self.part_uri.trim().is_empty() {
            add_selector(&mut selectors, format!("part:{}", self.part_uri));
        }
        self.selectors = selectors;
    }

    fn to_json_object(&self) -> Map<String, Value> {
        let mut object = Map::new();
        object.insert("number".to_string(), json!(self.number));
        object.insert("sheet".to_string(), json!(self.sheet));
        object.insert("sheetNumber".to_string(), json!(self.sheet_number));
        object.insert("sheetPartUri".to_string(), json!(self.sheet_part_uri));
        object.insert("relationshipId".to_string(), json!(self.relationship_id));
        object.insert("partUri".to_string(), json!(self.part_uri));
        object.insert("id".to_string(), json!(self.id));
        if !self.name.is_empty() {
            object.insert("name".to_string(), json!(self.name));
        }
        object.insert("displayName".to_string(), json!(self.display_name));
        if !self.primary_selector.is_empty() {
            object.insert("primarySelector".to_string(), json!(self.primary_selector));
        }
        if !self.selectors.is_empty() {
            object.insert("selectors".to_string(), json!(self.selectors));
        }
        object.insert("range".to_string(), json!(self.range));
        object.insert("rows".to_string(), json!(self.rows));
        object.insert("cols".to_string(), json!(self.cols));
        object.insert("headerRowCount".to_string(), json!(self.header_row_count));
        object.insert("dataRowCount".to_string(), json!(self.data_row_count));
        object.insert("totalsRowCount".to_string(), json!(self.totals_row_count));
        if !self.style_name.is_empty() {
            object.insert("styleName".to_string(), json!(self.style_name));
        }
        if !self.columns.is_empty() {
            object.insert(
                "columns".to_string(),
                json!(
                    self.columns
                        .iter()
                        .map(|column| json!({"id": column.id, "name": column.name}))
                        .collect::<Vec<_>>()
                ),
            );
        }
        object
    }
}

fn workbook_sheets(xml: &str) -> CliResult<Vec<WorkbookSheet>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut sheets = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut saw_workbook = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.is_empty() && name != "workbook" {
                    return Err(CliError::unexpected(format!(
                        "workbook root is {name:?}, expected workbook"
                    )));
                }
                if stack.is_empty() {
                    saw_workbook = true;
                }
                let parent = stack.last().map(String::as_str);
                if parent == Some("sheets") && name == "sheet" {
                    parse_workbook_sheet(&e, &mut sheets)?;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.is_empty() && name != "workbook" {
                    return Err(CliError::unexpected(format!(
                        "workbook root is {name:?}, expected workbook"
                    )));
                }
                if stack.is_empty() {
                    saw_workbook = true;
                }
                let parent = stack.last().map(String::as_str);
                if parent == Some("sheets") && name == "sheet" {
                    parse_workbook_sheet(&e, &mut sheets)?;
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(CliError::unexpected("unexpected EOF"));
    }
    if !saw_workbook {
        return Err(CliError::unexpected("workbook part has no root element"));
    }
    Ok(sheets)
}

fn parse_workbook_sheet(e: &BytesStart<'_>, sheets: &mut Vec<WorkbookSheet>) -> CliResult<()> {
    let position = sheets.len() as u32 + 1;
    if let (Some(name), Some(number), Some(rel_id)) =
        (attr(e, "name"), attr(e, "sheetId"), attr_exact(e, "r:id"))
    {
        let number = number.parse::<u32>().map_err(|_| {
            CliError::unexpected(format!("sheet at position {position} has invalid sheetId"))
        })?;
        sheets.push(WorkbookSheet {
            name,
            sheet_id: number,
            position,
            rel_id,
            state: attr(e, "state").unwrap_or_else(|| "visible".to_string()),
        });
        Ok(())
    } else {
        Err(CliError::unexpected(format!(
            "sheet at position {position} is missing name, sheetId, or r:id"
        )))
    }
}

fn shared_string_count(file: &str, part_uri: &str) -> CliResult<usize> {
    let xml = zip_text(file, part_uri.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut saw_root = false;
    let mut count = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    if name != "sst" {
                        return Err(CliError::unexpected(
                            "shared string table root element not found",
                        ));
                    }
                    saw_root = true;
                } else if name == "si" {
                    count += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    if name != "sst" {
                        return Err(CliError::unexpected(
                            "shared string table root element not found",
                        ));
                    }
                    saw_root = true;
                } else if name == "si" {
                    count += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if saw_root {
        Ok(count)
    } else {
        Err(CliError::unexpected(
            "shared string table root element not found",
        ))
    }
}

fn resolve_sheet(sheets: &[WorkbookSheet], selector: &str) -> CliResult<WorkbookSheet> {
    if let Some(sheet_id) = selector.strip_prefix("sheetId:")
        && let Ok(sheet_id) = sheet_id.parse::<u32>()
        && let Some(sheet) = sheets.iter().find(|sheet| sheet.sheet_id == sheet_id)
    {
        return Ok(sheet.clone());
    }
    if let Some(position) = selector
        .strip_prefix("sheet:")
        .or_else(|| selector.strip_prefix('#'))
        && let Ok(position) = position.parse::<u32>()
        && let Some(sheet) = sheets.iter().find(|sheet| sheet.position == position)
    {
        return Ok(sheet.clone());
    }
    if let Some(name) = selector
        .strip_prefix("name:")
        .or_else(|| selector.strip_prefix('~'))
        && let Some(sheet) = sheets.iter().find(|sheet| sheet.name == name)
    {
        return Ok(sheet.clone());
    }
    if let Some(rel_id) = selector
        .strip_prefix("rId:")
        .or_else(|| selector.strip_prefix("rid:"))
        && let Some(sheet) = sheets.iter().find(|sheet| sheet.rel_id == rel_id)
    {
        return Ok(sheet.clone());
    }
    if let Ok(number) = selector.parse::<u32>()
        && let Some(sheet) = sheets.iter().find(|sheet| sheet.position == number)
    {
        return Ok(sheet.clone());
    }
    sheets
        .iter()
        .find(|sheet| sheet.name == selector)
        .cloned()
        .ok_or_else(|| CliError::invalid_args(format!("sheet not found: {selector}")))
}

fn shared_strings(file: &str) -> CliResult<Vec<String>> {
    let xml = match zip_text(file, "xl/sharedStrings.xml") {
        Ok(xml) => xml,
        Err(_) => return Ok(Vec::new()),
    };
    let mut reader = Reader::from_str(&xml);
    let mut strings = Vec::new();
    let mut current = String::new();
    let mut in_si = false;
    let mut in_t = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "si" => {
                in_si = true;
                current.clear();
            }
            Ok(Event::Start(e)) if in_si && local_name(e.name().as_ref()) == "t" => in_t = true,
            Ok(Event::Text(e)) if in_t => current.push_str(&decode_xml_text(e.as_ref())),
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => in_t = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "si" => {
                strings.push(current.clone());
                in_si = false;
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(strings)
}

#[derive(Clone)]
struct CellValue {
    kind: String,
    matrix_value: Value,
    display_value: String,
    raw_value: String,
    formula: String,
    style_index: Option<u32>,
    number_format_id: Option<u32>,
    number_format_code: Option<String>,
    date_style: bool,
    has_formula: bool,
}

#[derive(Clone, Default)]
struct XlsxStyle {
    number_format_id: Option<u32>,
    number_format_code: Option<String>,
    date_style: bool,
}

#[derive(Clone)]
struct XlsxCellEntry {
    ref_name: String,
    row: u32,
    col: u32,
    value: CellValue,
}

#[derive(Clone, Copy)]
struct UsedRangeSummary {
    min_row: u32,
    max_row: u32,
    min_col: u32,
    max_col: u32,
    empty: bool,
}

fn sheet_cells(
    xml: &str,
    shared_strings: &[String],
    styles: &[XlsxStyle],
) -> BTreeMap<String, CellValue> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut cells = BTreeMap::new();
    let mut current_ref = String::new();
    let mut current_type = String::new();
    let mut current_value = String::new();
    let mut current_inline_text = String::new();
    let mut current_formula = String::new();
    let mut current_style_index: Option<u32> = None;
    let mut in_v = false;
    let mut in_t = false;
    let mut in_formula = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "c" => {
                current_ref = attr(&e, "r").unwrap_or_default();
                current_type = attr(&e, "t").unwrap_or_default();
                current_value.clear();
                current_inline_text.clear();
                current_formula.clear();
                current_style_index = attr(&e, "s").and_then(|value| value.parse::<u32>().ok());
            }
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "c" => {
                let cell_ref = attr(&e, "r").unwrap_or_default();
                if !cell_ref.is_empty() {
                    let cell_type = attr(&e, "t").unwrap_or_default();
                    let style_index = attr(&e, "s").and_then(|value| value.parse::<u32>().ok());
                    let style = style_index
                        .and_then(|index| styles.get(index as usize).cloned())
                        .unwrap_or_default();
                    let (kind, matrix_value, display_value) =
                        decode_xlsx_cell_value(&cell_type, "", "", "", shared_strings, &style);
                    cells.insert(
                        cell_ref,
                        CellValue {
                            kind,
                            matrix_value,
                            display_value,
                            raw_value: String::new(),
                            formula: String::new(),
                            style_index,
                            number_format_id: style.number_format_id,
                            number_format_code: style.number_format_code,
                            date_style: style.date_style,
                            has_formula: false,
                        },
                    );
                }
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "v" => in_v = true,
            Ok(Event::Start(e))
                if current_type == "inlineStr" && local_name(e.name().as_ref()) == "t" =>
            {
                in_t = true;
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "f" => {
                in_formula = true;
            }
            Ok(Event::Text(e)) if in_v => current_value.push_str(&decode_xml_text(e.as_ref())),
            Ok(Event::Text(e)) if in_t => {
                current_inline_text.push_str(&decode_xml_text(e.as_ref()))
            }
            Ok(Event::Text(e)) if in_formula => {
                current_formula.push_str(&decode_xml_text(e.as_ref()))
            }
            Ok(Event::GeneralRef(e)) if in_v => {
                current_value.push_str(&xml_general_ref(e.as_ref()))
            }
            Ok(Event::GeneralRef(e)) if in_t => {
                current_inline_text.push_str(&xml_general_ref(e.as_ref()))
            }
            Ok(Event::GeneralRef(e)) if in_formula => {
                current_formula.push_str(&xml_general_ref(e.as_ref()))
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "v" => in_v = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => in_t = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "f" => in_formula = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "c" => {
                if !current_ref.is_empty() {
                    let style = current_style_index
                        .and_then(|index| styles.get(index as usize).cloned())
                        .unwrap_or_default();
                    let (kind, matrix_value, display_value) = decode_xlsx_cell_value(
                        &current_type,
                        &current_value,
                        &current_inline_text,
                        &current_formula,
                        shared_strings,
                        &style,
                    );
                    let raw_value = if current_type == "inlineStr" {
                        String::new()
                    } else {
                        current_value.clone()
                    };
                    cells.insert(
                        current_ref.clone(),
                        CellValue {
                            kind,
                            matrix_value,
                            display_value,
                            raw_value,
                            formula: current_formula.clone(),
                            style_index: current_style_index,
                            number_format_id: style.number_format_id,
                            number_format_code: style.number_format_code,
                            date_style: style.date_style,
                            has_formula: !current_formula.is_empty(),
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    cells
}

fn decode_xlsx_cell_value(
    cell_type: &str,
    raw: &str,
    inline_text: &str,
    formula: &str,
    shared_strings: &[String],
    style: &XlsxStyle,
) -> (String, Value, String) {
    match cell_type {
        "s" => {
            let idx = raw.parse::<usize>().unwrap_or(usize::MAX);
            let text = shared_strings.get(idx).cloned().unwrap_or_default();
            ("string".to_string(), Value::String(text.clone()), text)
        }
        "inlineStr" => (
            "string".to_string(),
            Value::String(inline_text.to_string()),
            inline_text.to_string(),
        ),
        "str" => (
            "string".to_string(),
            Value::String(raw.to_string()),
            raw.to_string(),
        ),
        "b" => {
            let text = match raw.trim() {
                "1" => "true",
                "0" => "false",
                _ => raw,
            }
            .to_string();
            let matrix = match raw.trim() {
                "1" => Value::Bool(true),
                "0" => Value::Bool(false),
                _ => Value::String(text.clone()),
            };
            ("boolean".to_string(), matrix, text)
        }
        "e" => (
            "error".to_string(),
            Value::String(raw.to_string()),
            raw.to_string(),
        ),
        "d" => (
            "date".to_string(),
            Value::String(raw.to_string()),
            raw.to_string(),
        ),
        "" if raw.is_empty() && formula.is_empty() => {
            ("empty".to_string(), Value::Null, String::new())
        }
        "" if raw.is_empty() && !formula.is_empty() => {
            ("number".to_string(), Value::Null, String::new())
        }
        "" if style.date_style => (
            "date".to_string(),
            Value::String(raw.to_string()),
            raw.to_string(),
        ),
        "" => {
            let matrix = if let Ok(number) = raw.parse::<i64>() {
                json!(number)
            } else if let Ok(number) = raw.parse::<f64>() {
                json!(number)
            } else {
                Value::String(raw.to_string())
            };
            ("number".to_string(), matrix, raw.to_string())
        }
        _ => (
            "unknown".to_string(),
            Value::String(raw.to_string()),
            raw.to_string(),
        ),
    }
}

fn xlsx_styles(file: &str) -> CliResult<Vec<XlsxStyle>> {
    let xml = match zip_text(file, "xl/styles.xml") {
        Ok(xml) => xml,
        Err(_) => return Ok(Vec::new()),
    };
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut custom_formats = BTreeMap::<u32, String>::new();
    let mut styles = Vec::new();
    let mut in_cell_xfs = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "numFmt" =>
            {
                if let (Some(id), Some(code)) = (attr(&e, "numFmtId"), attr(&e, "formatCode"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    custom_formats.insert(id, code);
                }
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "cellXfs" => {
                in_cell_xfs = true;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "cellXfs" => {
                in_cell_xfs = false;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if in_cell_xfs && local_name(e.name().as_ref()) == "xf" =>
            {
                let number_format_id = attr(&e, "numFmtId").and_then(|value| value.parse().ok());
                let number_format_code = number_format_id.and_then(|id| {
                    custom_formats
                        .get(&id)
                        .cloned()
                        .or_else(|| builtin_num_format_code(id).map(ToString::to_string))
                });
                let date_style = number_format_id.is_some_and(is_builtin_date_num_fmt)
                    || number_format_code
                        .as_deref()
                        .is_some_and(is_date_format_code);
                styles.push(XlsxStyle {
                    number_format_id,
                    number_format_code,
                    date_style,
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(styles)
}

fn builtin_num_format_code(id: u32) -> Option<&'static str> {
    match id {
        0 => Some("General"),
        1 => Some("0"),
        2 => Some("0.00"),
        3 => Some("#,##0"),
        4 => Some("#,##0.00"),
        9 => Some("0%"),
        10 => Some("0.00%"),
        14 => Some("m/d/yy"),
        15 => Some("d-mmm-yy"),
        16 => Some("d-mmm"),
        17 => Some("mmm-yy"),
        18 => Some("h:mm AM/PM"),
        19 => Some("h:mm:ss AM/PM"),
        20 => Some("h:mm"),
        21 => Some("h:mm:ss"),
        22 => Some("m/d/yy h:mm"),
        45 => Some("mm:ss"),
        46 => Some("[h]:mm:ss"),
        47 => Some("mmss.0"),
        49 => Some("@"),
        _ => None,
    }
}

fn is_builtin_date_num_fmt(id: u32) -> bool {
    matches!(id, 14..=22 | 45..=47)
}

fn is_date_format_code(code: &str) -> bool {
    let mut cleaned = String::new();
    let mut in_quote = false;
    for ch in code.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            _ if !in_quote => cleaned.push(ch.to_ascii_lowercase()),
            _ => {}
        }
    }
    cleaned.contains('y')
        || cleaned.contains('d')
        || cleaned.contains("h:")
        || cleaned.contains("m/")
}

fn xlsx_dimension_declared(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "dimension" =>
            {
                return attr(&e, "ref");
            }
            Ok(Event::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
    }
}

fn xlsx_merged_cell_count(xml: &str) -> usize {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut count = 0;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "mergeCell" =>
            {
                count += 1;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    count
}

fn sorted_xlsx_cells(
    cells: &BTreeMap<String, CellValue>,
    range: Option<RangeBounds>,
) -> Vec<XlsxCellEntry> {
    let mut entries: Vec<XlsxCellEntry> = cells
        .iter()
        .filter_map(|(ref_name, value)| {
            let (col, row) = parse_cell_ref(ref_name).ok()?;
            if let Some(bounds) = range
                && !range_contains_cell(bounds, col, row)
            {
                return None;
            }
            Some(XlsxCellEntry {
                ref_name: ref_name.clone(),
                row,
                col,
                value: value.clone(),
            })
        })
        .collect();
    entries.sort_by_key(|entry| (entry.row, entry.col));
    entries
}

fn used_range_for_cells(cells: &[XlsxCellEntry]) -> UsedRangeSummary {
    let Some(first) = cells.first() else {
        return UsedRangeSummary {
            min_row: 0,
            max_row: 0,
            min_col: 0,
            max_col: 0,
            empty: true,
        };
    };
    let mut used = UsedRangeSummary {
        min_row: first.row,
        max_row: first.row,
        min_col: first.col,
        max_col: first.col,
        empty: false,
    };
    for cell in cells.iter().skip(1) {
        used.min_row = used.min_row.min(cell.row);
        used.max_row = used.max_row.max(cell.row);
        used.min_col = used.min_col.min(cell.col);
        used.max_col = used.max_col.max(cell.col);
    }
    used
}

fn used_range_json(used: UsedRangeSummary) -> Value {
    if used.empty {
        return json!({
            "rows": 0,
            "cols": 0,
            "empty": true,
        });
    }
    json!({
        "ref": format!(
            "{}{}:{}{}",
            col_name(used.min_col),
            used.min_row,
            col_name(used.max_col),
            used.max_row
        ),
        "minRow": used.min_row,
        "maxRow": used.max_row,
        "minCol": used.min_col,
        "maxCol": used.max_col,
        "rows": used.max_row - used.min_row + 1,
        "cols": used.max_col - used.min_col + 1,
        "empty": false,
    })
}

fn used_range_ref(used: UsedRangeSummary) -> Option<String> {
    if used.empty {
        None
    } else {
        Some(format!(
            "{}{}:{}{}",
            col_name(used.min_col),
            used.min_row,
            col_name(used.max_col),
            used.max_row
        ))
    }
}

fn build_sparse_xlsx_rows(
    cells: &[XlsxCellEntry],
    max_rows: u32,
    max_cells: u32,
    sheet: &WorkbookSheet,
) -> (Vec<Value>, bool) {
    let mut rows = Vec::<Value>::new();
    let mut row_cells = Vec::<Value>::new();
    let mut current_row = None::<u32>;
    let mut truncated = false;

    for (emitted_cells, cell) in cells.iter().enumerate() {
        if max_cells > 0 && emitted_cells as u32 >= max_cells {
            truncated = true;
            break;
        }
        if current_row != Some(cell.row) {
            if let Some(row_number) = current_row {
                rows.push(json!({"number": row_number, "cells": row_cells}));
                row_cells = Vec::new();
            }
            if max_rows > 0 && rows.len() as u32 >= max_rows {
                truncated = true;
                break;
            }
            current_row = Some(cell.row);
        }
        row_cells.push(xlsx_cell_json(
            &cell.ref_name,
            cell.row,
            cell.col,
            &cell.value,
            sheet,
        ));
    }

    if let Some(row_number) = current_row
        && !row_cells.is_empty()
    {
        rows.push(json!({"number": row_number, "cells": row_cells}));
    }
    (rows, truncated)
}

fn build_dense_xlsx_rows(
    cells: &[XlsxCellEntry],
    range: Option<RangeBounds>,
    used: UsedRangeSummary,
    max_rows: u32,
    max_cells: u32,
    sheet: &WorkbookSheet,
) -> (Vec<Value>, bool) {
    let Some((min_col, min_row, max_col, max_row)) = output_xlsx_bounds(range, used) else {
        return (Vec::new(), false);
    };
    let max_cells = if max_cells == 0 { 10_000 } else { max_cells };
    let by_ref: BTreeMap<String, &XlsxCellEntry> = cells
        .iter()
        .map(|cell| (cell.ref_name.clone(), cell))
        .collect();
    let mut rows = Vec::new();
    let mut emitted_cells = 0u32;
    let mut truncated = false;

    for row in min_row..=max_row {
        if max_rows > 0 && rows.len() as u32 >= max_rows {
            truncated = true;
            break;
        }
        let mut row_cells = Vec::new();
        for col in min_col..=max_col {
            if max_cells > 0 && emitted_cells >= max_cells {
                truncated = true;
                break;
            }
            let ref_name = format!("{}{}", col_name(col), row);
            let cell_value;
            let value = if let Some(cell) = by_ref.get(&ref_name) {
                &cell.value
            } else {
                cell_value = CellValue {
                    kind: "empty".to_string(),
                    matrix_value: Value::Null,
                    display_value: String::new(),
                    raw_value: String::new(),
                    formula: String::new(),
                    style_index: None,
                    number_format_id: None,
                    number_format_code: None,
                    date_style: false,
                    has_formula: false,
                };
                &cell_value
            };
            row_cells.push(xlsx_cell_json(&ref_name, row, col, value, sheet));
            emitted_cells += 1;
        }
        rows.push(json!({"number": row, "cells": row_cells}));
        if truncated {
            break;
        }
    }
    (rows, truncated)
}

fn output_xlsx_bounds(
    range: Option<RangeBounds>,
    used: UsedRangeSummary,
) -> Option<(u32, u32, u32, u32)> {
    if let Some(range) = range {
        return Some((
            range.start_col,
            range.start_row,
            range.end_col,
            range.end_row,
        ));
    }
    if used.empty {
        None
    } else {
        Some((used.min_col, used.min_row, used.max_col, used.max_row))
    }
}

fn xlsx_cell_json(
    ref_name: &str,
    row: u32,
    col: u32,
    value: &CellValue,
    sheet: &WorkbookSheet,
) -> Value {
    let mut object = Map::new();
    object.insert("ref".to_string(), json!(ref_name));
    object.insert(
        "handle".to_string(),
        json!(format!("H:xlsx/ws:{}/cell:a:{ref_name}", sheet.sheet_id)),
    );
    object.insert("primarySelector".to_string(), json!(ref_name));
    object.insert("selectors".to_string(), json!([ref_name]));
    object.insert("row".to_string(), json!(row));
    object.insert("col".to_string(), json!(col));
    object.insert("column".to_string(), json!(col_name(col)));
    object.insert("type".to_string(), json!(value.kind));
    if !value.display_value.is_empty() {
        object.insert("value".to_string(), json!(value.display_value));
    }
    if !value.raw_value.is_empty() {
        object.insert("rawValue".to_string(), json!(value.raw_value));
    }
    if !value.formula.is_empty() {
        object.insert("formula".to_string(), json!(value.formula));
    }
    if let Some(style_index) = value.style_index.filter(|style_index| *style_index > 0) {
        object.insert("styleIndex".to_string(), json!(style_index));
    }
    if let Some(number_format_id) = value
        .number_format_id
        .filter(|number_format_id| *number_format_id > 0)
    {
        object.insert("numberFormatId".to_string(), json!(number_format_id));
    }
    if let Some(number_format_code) = value
        .number_format_code
        .as_ref()
        .filter(|number_format_code| !number_format_code.is_empty())
    {
        object.insert("numberFormatCode".to_string(), json!(number_format_code));
    }
    if value.date_style {
        object.insert("dateStyle".to_string(), json!(true));
    }
    Value::Object(object)
}

fn range_contains_cell(bounds: RangeBounds, col: u32, row: u32) -> bool {
    col >= bounds.min_col()
        && col <= bounds.max_col()
        && row >= bounds.min_row()
        && row <= bounds.max_row()
}

#[derive(Clone, Copy)]
struct RangeBounds {
    start_col: u32,
    start_row: u32,
    end_col: u32,
    end_row: u32,
}

impl RangeBounds {
    fn min_col(self) -> u32 {
        self.start_col.min(self.end_col)
    }

    fn max_col(self) -> u32 {
        self.start_col.max(self.end_col)
    }

    fn min_row(self) -> u32 {
        self.start_row.min(self.end_row)
    }

    fn max_row(self) -> u32 {
        self.start_row.max(self.end_row)
    }

    fn row_count(self) -> u32 {
        self.max_row() - self.min_row() + 1
    }

    fn col_count(self) -> u32 {
        self.max_col() - self.min_col() + 1
    }

    fn normalized(self) -> RangeBounds {
        RangeBounds {
            start_col: self.min_col(),
            start_row: self.min_row(),
            end_col: self.max_col(),
            end_row: self.max_row(),
        }
    }
}

fn parse_range(range: &str) -> CliResult<RangeBounds> {
    let range = range.trim();
    if range.is_empty() {
        return Err(CliError::invalid_args("range reference cannot be empty"));
    }
    let parts = range.split(':').collect::<Vec<_>>();
    if parts.len() > 2 {
        return Err(CliError::invalid_args(format!(
            "invalid range reference {range:?}"
        )));
    }
    let start = parts[0];
    let end = parts.get(1).copied().unwrap_or(start);
    if parts.len() == 2 && end.trim().is_empty() {
        return Err(CliError::invalid_args("range end cannot be empty"));
    }
    let (start_col, start_row) = parse_cell_ref(start)
        .map_err(|err| CliError::invalid_args(format!("invalid range start: {}", err.message)))?;
    let (end_col, end_row) = parse_cell_ref(end)
        .map_err(|err| CliError::invalid_args(format!("invalid range end: {}", err.message)))?;
    Ok(RangeBounds {
        start_col,
        start_row,
        end_col,
        end_row,
    })
}

fn parse_cli_range(range: &str) -> CliResult<RangeBounds> {
    parse_range(range)
        .map_err(|err| CliError::invalid_args(format!("invalid --range: {}", err.message)))
}

fn parse_cell_ref(cell: &str) -> CliResult<(u32, u32)> {
    let cell = cell.trim();
    if cell.is_empty() {
        return Err(CliError::invalid_args("cell reference cannot be empty"));
    }
    let mut rest = cell;
    if let Some(after_abs_col) = rest.strip_prefix('$') {
        rest = after_abs_col;
        if rest.is_empty() {
            return Err(CliError::invalid_args("missing column in cell reference"));
        }
    }
    let col_len = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    if col_len == 0 {
        return Err(CliError::invalid_args("missing column in cell reference"));
    }
    let mut col = 0u32;
    for ch in rest[..col_len].chars() {
        col = col * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        if col > 16_384 {
            return Err(CliError::invalid_args(format!(
                "column {:?} out of XLSX bounds A-XFD",
                &rest[..col_len]
            )));
        }
    }
    rest = &rest[col_len..];
    if rest.is_empty() {
        return Err(CliError::invalid_args("missing row in cell reference"));
    }
    if let Some(after_abs_row) = rest.strip_prefix('$') {
        rest = after_abs_row;
        if rest.is_empty() {
            return Err(CliError::invalid_args("missing row in cell reference"));
        }
    }
    if rest.contains('$') {
        return Err(CliError::invalid_args(
            "invalid absolute marker in row reference",
        ));
    }
    if !rest.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(CliError::invalid_args(format!(
            "invalid row {rest:?} in cell reference"
        )));
    }
    let row = rest
        .parse::<u32>()
        .map_err(|err| CliError::invalid_args(format!("invalid row {rest:?}: {err}")))?;
    if row == 0 || row > 1_048_576 {
        return Err(CliError::invalid_args(format!(
            "row {row} out of XLSX bounds 1-1048576"
        )));
    }
    Ok((col, row))
}

fn col_name(mut col: u32) -> String {
    let mut chars = Vec::new();
    while col > 0 {
        col -= 1;
        chars.push((b'A' + (col % 26) as u8) as char);
        col /= 26;
    }
    chars.iter().rev().collect()
}

#[derive(Default)]
struct DocxParagraphState {
    text: String,
    style: Option<String>,
    para_id: Option<String>,
}

enum DocxParagraphContext {
    Body,
    TableCell,
}

#[derive(Default)]
struct DocxTableState {
    rows: Vec<Vec<String>>,
    current_row: Option<Vec<String>>,
    current_cell: Option<Vec<String>>,
}

fn docx_blocks(xml: &str) -> Vec<Value> {
    let mut reader = Reader::from_str(xml);
    let para_id_counts = docx_para_id_counts(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut blocks = Vec::new();
    let mut current_paragraph: Option<DocxParagraphState> = None;
    let mut paragraph_context: Option<DocxParagraphContext> = None;
    let mut current_table: Option<DocxTableState> = None;
    let mut in_t = false;
    let mut skip_text_depth = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.last().is_some_and(|parent| parent == "body") && name == "tbl" {
                    current_table = Some(DocxTableState::default());
                } else if current_table.is_some() && name == "tr" {
                    if let Some(table) = current_table.as_mut() {
                        table.current_row = Some(Vec::new());
                    }
                } else if current_table.is_some() && name == "tc" {
                    if let Some(table) = current_table.as_mut() {
                        table.current_cell = Some(Vec::new());
                    }
                } else if stack.last().is_some_and(|parent| parent == "body") && name == "p" {
                    current_paragraph = Some(DocxParagraphState {
                        para_id: docx_para_id(&e),
                        ..DocxParagraphState::default()
                    });
                    paragraph_context = Some(DocxParagraphContext::Body);
                } else if current_table.is_some() && name == "p" && stack_contains(&stack, "tc") {
                    current_paragraph = Some(DocxParagraphState {
                        para_id: docx_para_id(&e),
                        ..DocxParagraphState::default()
                    });
                    paragraph_context = Some(DocxParagraphContext::TableCell);
                }

                docx_note_empty_or_start(&e, &name, &mut current_paragraph);
                if name == "t" {
                    in_t = true;
                }
                if name == "delText" || name == "instrText" {
                    skip_text_depth += 1;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                docx_note_empty_or_start(&e, &name, &mut current_paragraph);
            }
            Ok(Event::Text(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current_paragraph.as_mut() {
                    paragraph.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current_paragraph.as_mut() {
                    paragraph
                        .text
                        .push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                match name.as_str() {
                    "t" => in_t = false,
                    "delText" | "instrText" => {
                        skip_text_depth = skip_text_depth.saturating_sub(1);
                    }
                    "p" => {
                        if let (Some(paragraph), Some(context)) =
                            (current_paragraph.take(), paragraph_context.take())
                        {
                            match context {
                                DocxParagraphContext::Body => {
                                    blocks.push(docx_paragraph_block(
                                        blocks.len() + 1,
                                        paragraph,
                                        &para_id_counts,
                                    ));
                                }
                                DocxParagraphContext::TableCell => {
                                    if let Some(cell) = current_table
                                        .as_mut()
                                        .and_then(|table| table.current_cell.as_mut())
                                    {
                                        cell.push(paragraph.text);
                                    }
                                }
                            }
                        }
                    }
                    "tc" => {
                        if let Some(table) = current_table.as_mut()
                            && let Some(cell) = table.current_cell.take()
                            && let Some(row) = table.current_row.as_mut()
                        {
                            row.push(cell.join("\n"));
                        }
                    }
                    "tr" => {
                        if let Some(table) = current_table.as_mut()
                            && let Some(row) = table.current_row.take()
                        {
                            table.rows.push(row);
                        }
                    }
                    "tbl" => {
                        if let Some(table) = current_table.take() {
                            blocks.push(docx_table_block(blocks.len() + 1, table.rows));
                        }
                    }
                    _ => {}
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    blocks
}

fn docx_note_empty_or_start(
    element: &BytesStart<'_>,
    name: &str,
    current_paragraph: &mut Option<DocxParagraphState>,
) {
    let Some(paragraph) = current_paragraph.as_mut() else {
        return;
    };
    match name {
        "pStyle" => {
            if let Some(style) = attr(element, "val").filter(|style| !style.is_empty()) {
                paragraph.style = Some(style);
            }
        }
        "tab" => paragraph.text.push('\t'),
        "br" | "cr" => paragraph.text.push('\n'),
        "noBreakHyphen" => paragraph.text.push('-'),
        _ => {}
    }
}

fn docx_paragraph_block(
    index: usize,
    paragraph: DocxParagraphState,
    para_id_counts: &BTreeMap<String, usize>,
) -> Value {
    let mut block = Map::new();
    block.insert("index".to_string(), json!(index));
    block.insert("kind".to_string(), json!("paragraph"));
    if let Some(style) = paragraph.style {
        block.insert("style".to_string(), json!(style));
    }
    block.insert("text".to_string(), json!(paragraph.text));
    if let Some(para_id) = paragraph.para_id.filter(|para_id| !para_id.is_empty()) {
        let normalized = para_id.trim().to_ascii_uppercase();
        block.insert("paraId".to_string(), json!(para_id));
        if para_id_counts.get(&normalized).copied().unwrap_or_default() == 1 {
            block.insert(
                "handle".to_string(),
                json!(format!("H:docx/pt:doc/para:m:{para_id}")),
            );
        }
    }
    Value::Object(block)
}

fn docx_table_block(index: usize, rows: Vec<Vec<String>>) -> Value {
    let table_rows: Vec<Value> = rows.iter().map(|row| json!({"cells": row})).collect();
    let text = rows
        .iter()
        .map(|row| row.join("\t"))
        .collect::<Vec<_>>()
        .join("\n");
    json!({
        "index": index,
        "kind": "table",
        "table": {"rows": table_rows},
        "text": text,
    })
}

fn stack_contains(stack: &[String], name: &str) -> bool {
    stack.iter().any(|item| item == name)
}

fn docx_para_id_counts(xml: &str) -> BTreeMap<String, usize> {
    let mut reader = Reader::from_str(xml);
    let mut counts = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "p" => {
                if let Some(para_id) = docx_para_id(&e) {
                    *counts.entry(para_id.to_ascii_uppercase()).or_insert(0) += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    counts
}

fn docx_para_id(element: &BytesStart<'_>) -> Option<String> {
    attr(element, "paraId")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn docx_para_id_ns(element: &BytesStart<'_>, resolver: &NamespaceResolver) -> Option<String> {
    attr_prefixed_ns(element, resolver, b"w14", DOCX_W14_NS, b"paraId")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn docx_word_val_ns(element: &BytesStart<'_>, resolver: &NamespaceResolver) -> Option<String> {
    attr_prefixed_ns(element, resolver, b"w", DOCX_W_NS, b"val")
}

fn element_in_ns(resolver: &NamespaceResolver, element: &BytesStart<'_>, ns: &[u8]) -> bool {
    matches!(
        resolver.resolve_element(element.name()),
        (ResolveResult::Bound(Namespace(uri)), _) if uri == ns
    )
}

#[derive(Default)]
struct DocxRichParagraphState {
    text: String,
    style: String,
    para_id: String,
    runs: Vec<DocxRichRunInfo>,
}

enum DocxRichParagraphContext {
    Body,
    TableCell,
}

#[derive(Clone, Default)]
struct DocxRichRunInfo {
    text: String,
    bold: bool,
    italic: bool,
    underline: String,
    color: String,
    size: String,
}

#[derive(Default)]
struct DocxRichRunState {
    info: DocxRichRunInfo,
}

#[derive(Default)]
struct DocxRichTableState {
    rows: Vec<Vec<String>>,
    current_row: Option<Vec<String>>,
    current_cell: Option<Vec<String>>,
    merged: bool,
}

struct DocxRichBlockReport {
    index: usize,
    kind: &'static str,
    text: String,
    style: String,
    para_id: String,
    handle: String,
    content_hash: String,
    runs: Vec<DocxRichRunInfo>,
    table_rows: Vec<Vec<String>>,
    table_merged: bool,
}

fn docx_rich_block_reports(xml: &str, include_runs: bool) -> CliResult<Vec<DocxRichBlockReport>> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let para_id_counts = docx_body_para_id_counts(xml)?;
    let mut stack: Vec<String> = Vec::new();
    let mut word_stack: Vec<bool> = Vec::new();
    let mut blocks = Vec::new();
    let mut current_paragraph: Option<DocxRichParagraphState> = None;
    let mut paragraph_context: Option<DocxRichParagraphContext> = None;
    let mut current_run: Option<DocxRichRunState> = None;
    let mut current_table: Option<DocxRichTableState> = None;
    let mut body_table_depth = 0usize;
    let mut in_t = false;
    let mut skip_text_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let parent_is_word = word_stack.last().copied().unwrap_or(false);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if parent == Some("body") && name == "tbl" {
                    current_table = Some(DocxRichTableState::default());
                    body_table_depth = 1;
                } else if current_table.is_some() && name == "tbl" {
                    body_table_depth += 1;
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tbl")
                    && is_word
                    && name == "tr"
                {
                    if let Some(table) = current_table.as_mut() {
                        table.current_row = Some(Vec::new());
                    }
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tr")
                    && parent_is_word
                    && is_word
                    && name == "tc"
                {
                    if let Some(table) = current_table.as_mut() {
                        table.current_cell = Some(Vec::new());
                    }
                } else if parent == Some("body") && name == "p" {
                    current_paragraph = Some(DocxRichParagraphState {
                        para_id: docx_para_id_ns(&e, reader.resolver()).unwrap_or_default(),
                        ..DocxRichParagraphState::default()
                    });
                    paragraph_context = Some(DocxRichParagraphContext::Body);
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tc")
                    && parent_is_word
                    && is_word
                    && name == "p"
                {
                    current_paragraph = Some(DocxRichParagraphState::default());
                    paragraph_context = Some(DocxRichParagraphContext::TableCell);
                } else if include_runs
                    && matches!(paragraph_context, Some(DocxRichParagraphContext::Body))
                    && parent == Some("p")
                    && is_word
                    && name == "r"
                {
                    current_run = Some(DocxRichRunState::default());
                }
                if current_table.is_some()
                    && is_word
                    && matches!(name.as_str(), "gridSpan" | "vMerge")
                    && let Some(table) = current_table.as_mut()
                {
                    table.merged = true;
                }

                docx_rich_note_empty_or_start(
                    &e,
                    reader.resolver(),
                    &stack,
                    &word_stack,
                    &mut current_paragraph,
                    &mut current_run,
                    skip_text_depth,
                );
                if name == "t" {
                    in_t = true;
                }
                if name == "delText" || name == "instrText" {
                    skip_text_depth += 1;
                }
                stack.push(name);
                word_stack.push(is_word);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let parent_is_word = word_stack.last().copied().unwrap_or(false);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if parent == Some("body") && name == "p" {
                    let paragraph = DocxRichParagraphState {
                        para_id: docx_para_id_ns(&e, reader.resolver()).unwrap_or_default(),
                        ..DocxRichParagraphState::default()
                    };
                    blocks.push(docx_rich_paragraph_report(
                        blocks.len() + 1,
                        paragraph,
                        &para_id_counts,
                    ));
                } else if parent == Some("body") && name == "tbl" {
                    blocks.push(docx_rich_table_report(blocks.len() + 1, Vec::new(), false));
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tr")
                    && parent_is_word
                    && is_word
                    && name == "tc"
                {
                    if let Some(row) = current_table
                        .as_mut()
                        .and_then(|table| table.current_row.as_mut())
                    {
                        row.push(String::new());
                    }
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tbl")
                    && is_word
                    && name == "tr"
                {
                    if let Some(table) = current_table.as_mut() {
                        table.rows.push(Vec::new());
                    }
                } else if current_table.is_some()
                    && body_table_depth == 1
                    && parent == Some("tc")
                    && parent_is_word
                    && is_word
                    && name == "p"
                    && let Some(cell) = current_table
                        .as_mut()
                        .and_then(|table| table.current_cell.as_mut())
                {
                    cell.push(String::new());
                }
                if current_table.is_some()
                    && is_word
                    && matches!(name.as_str(), "gridSpan" | "vMerge")
                    && let Some(table) = current_table.as_mut()
                {
                    table.merged = true;
                }
                docx_rich_note_empty_or_start(
                    &e,
                    reader.resolver(),
                    &stack,
                    &word_stack,
                    &mut current_paragraph,
                    &mut current_run,
                    skip_text_depth,
                );
            }
            Ok(Event::Text(e)) if in_t && skip_text_depth == 0 => {
                docx_rich_append_text(
                    &mut current_paragraph,
                    &mut current_run,
                    &decode_xml_text(e.as_ref()),
                );
            }
            Ok(Event::GeneralRef(e)) if in_t && skip_text_depth == 0 => {
                docx_rich_append_text(
                    &mut current_paragraph,
                    &mut current_run,
                    &xml_general_ref(e.as_ref()),
                );
            }
            Ok(Event::CData(e)) if in_t && skip_text_depth == 0 => {
                docx_rich_append_text(
                    &mut current_paragraph,
                    &mut current_run,
                    &String::from_utf8_lossy(e.as_ref()),
                );
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                match name.as_str() {
                    "t" => in_t = false,
                    "delText" | "instrText" => {
                        skip_text_depth = skip_text_depth.saturating_sub(1);
                    }
                    "r" => {
                        if let Some(run) = current_run.take()
                            && let Some(paragraph) = current_paragraph.as_mut()
                            && docx_rich_run_has_content(&run.info)
                        {
                            paragraph.runs.push(run.info);
                        }
                    }
                    "p" => {
                        if let (Some(paragraph), Some(context)) =
                            (current_paragraph.take(), paragraph_context.take())
                        {
                            match context {
                                DocxRichParagraphContext::Body => {
                                    blocks.push(docx_rich_paragraph_report(
                                        blocks.len() + 1,
                                        paragraph,
                                        &para_id_counts,
                                    ));
                                }
                                DocxRichParagraphContext::TableCell => {
                                    if let Some(cell) = current_table
                                        .as_mut()
                                        .and_then(|table| table.current_cell.as_mut())
                                    {
                                        cell.push(paragraph.text);
                                    }
                                }
                            }
                        }
                    }
                    "tc" => {
                        if body_table_depth == 1
                            && let Some(table) = current_table.as_mut()
                            && let Some(cell) = table.current_cell.take()
                            && let Some(row) = table.current_row.as_mut()
                        {
                            row.push(cell.join("\n"));
                        }
                    }
                    "tr" => {
                        if body_table_depth == 1
                            && let Some(table) = current_table.as_mut()
                            && let Some(row) = table.current_row.take()
                        {
                            table.rows.push(row);
                        }
                    }
                    "tbl" => {
                        if body_table_depth == 1 {
                            body_table_depth = 0;
                            if let Some(table) = current_table.take() {
                                blocks.push(docx_rich_table_report(
                                    blocks.len() + 1,
                                    table.rows,
                                    table.merged,
                                ));
                            }
                        } else if body_table_depth > 1 {
                            body_table_depth -= 1;
                        } else if let Some(table) = current_table.take() {
                            blocks.push(docx_rich_table_report(
                                blocks.len() + 1,
                                table.rows,
                                table.merged,
                            ));
                        }
                    }
                    _ => {}
                }
                stack.pop();
                word_stack.pop();
            }
            Ok(Event::Eof) => {
                if !stack.is_empty() {
                    return Err(CliError::unexpected("invalid DOCX XML"));
                }
                break;
            }
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "failed to extract DOCX blocks: {err}"
                )));
            }
            _ => {}
        }
    }

    Ok(blocks)
}

fn docx_rich_note_empty_or_start(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    stack: &[String],
    word_stack: &[bool],
    current_paragraph: &mut Option<DocxRichParagraphState>,
    current_run: &mut Option<DocxRichRunState>,
    skip_text_depth: usize,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if matches!(name, "tab" | "br" | "cr" | "noBreakHyphen") && skip_text_depth == 0 {
        let text = match name {
            "tab" => "\t",
            "br" | "cr" => "\n",
            "noBreakHyphen" => "-",
            _ => "",
        };
        docx_rich_append_text(current_paragraph, current_run, text);
        return;
    }

    if let Some(paragraph) = current_paragraph.as_mut()
        && name == "pStyle"
        && element_in_ns(resolver, element, DOCX_W_NS)
        && stack.last().is_some_and(|parent| parent == "pPr")
        && word_stack.last().copied().unwrap_or(false)
        && let Some(style) = docx_word_val_ns(element, resolver).filter(|style| !style.is_empty())
    {
        paragraph.style = style;
    }

    if stack.last().is_some_and(|parent| parent == "rPr")
        && word_stack.last().copied().unwrap_or(false)
        && element_in_ns(resolver, element, DOCX_W_NS)
        && let Some(run) = current_run.as_mut()
    {
        docx_rich_note_run_prop(element, resolver, name, &mut run.info);
    }
}

fn docx_rich_note_run_prop(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    name: &str,
    run: &mut DocxRichRunInfo,
) {
    match name {
        "b" => run.bold = docx_word_toggle_enabled(element, resolver),
        "i" => run.italic = docx_word_toggle_enabled(element, resolver),
        "u" => {
            let value = docx_word_val_ns(element, resolver).unwrap_or_else(|| "single".to_string());
            if value != "none" && value != "0" {
                run.underline = value;
            }
        }
        "color" => {
            if let Some(value) = docx_word_val_ns(element, resolver) {
                run.color = value;
            }
        }
        "sz" => {
            if let Some(value) = docx_word_val_ns(element, resolver) {
                run.size = value;
            }
        }
        _ => {}
    }
}

fn docx_word_toggle_enabled(element: &BytesStart<'_>, resolver: &NamespaceResolver) -> bool {
    let Some(value) = docx_word_val_ns(element, resolver) else {
        return true;
    };
    match value.to_ascii_lowercase().as_str() {
        "" | "1" | "true" | "on" => true,
        "0" | "false" | "off" => false,
        _ => true,
    }
}

fn docx_rich_append_text(
    current_paragraph: &mut Option<DocxRichParagraphState>,
    current_run: &mut Option<DocxRichRunState>,
    text: &str,
) {
    if let Some(paragraph) = current_paragraph.as_mut() {
        paragraph.text.push_str(text);
    }
    if let Some(run) = current_run.as_mut() {
        run.info.text.push_str(text);
    }
}

fn docx_rich_run_has_content(run: &DocxRichRunInfo) -> bool {
    !run.text.is_empty()
        || run.bold
        || run.italic
        || !run.underline.is_empty()
        || !run.color.is_empty()
        || !run.size.is_empty()
}

fn docx_rich_paragraph_report(
    index: usize,
    paragraph: DocxRichParagraphState,
    para_id_counts: &BTreeMap<String, usize>,
) -> DocxRichBlockReport {
    let normalized_para_id = paragraph.para_id.trim().to_ascii_uppercase();
    let handle = if !paragraph.para_id.is_empty()
        && para_id_counts
            .get(&normalized_para_id)
            .copied()
            .unwrap_or_default()
            == 1
    {
        format!("H:docx/pt:doc/para:m:{}", paragraph.para_id)
    } else {
        String::new()
    };
    let content_hash = docx_rich_block_content_hash("paragraph", &paragraph.style, &paragraph.text);
    DocxRichBlockReport {
        index,
        kind: "paragraph",
        text: paragraph.text,
        style: paragraph.style,
        para_id: paragraph.para_id,
        handle,
        content_hash,
        runs: paragraph.runs,
        table_rows: Vec::new(),
        table_merged: false,
    }
}

fn docx_rich_table_report(
    index: usize,
    rows: Vec<Vec<String>>,
    merged: bool,
) -> DocxRichBlockReport {
    let text = docx_rich_table_text(&rows);
    let content_hash = docx_rich_block_content_hash("table", "", &text);
    DocxRichBlockReport {
        index,
        kind: "table",
        text,
        style: String::new(),
        para_id: String::new(),
        handle: String::new(),
        content_hash,
        runs: Vec::new(),
        table_rows: rows,
        table_merged: merged,
    }
}

fn docx_rich_block_json(report: DocxRichBlockReport) -> Value {
    let mut block = Map::new();
    block.insert("id".to_string(), json!(format!("body.b{}", report.index)));
    block.insert("index".to_string(), json!(report.index));
    block.insert("kind".to_string(), json!(report.kind));
    block.insert("text".to_string(), json!(report.text));
    block.insert(
        "primarySelector".to_string(),
        json!(report.index.to_string()),
    );
    block.insert("selectors".to_string(), json!([report.index.to_string()]));
    if !report.para_id.is_empty() {
        block.insert("paraId".to_string(), json!(report.para_id));
    }
    if !report.handle.is_empty() {
        block.insert("handle".to_string(), json!(report.handle));
    }
    block.insert("contentHash".to_string(), json!(report.content_hash));
    if report.kind == "paragraph" {
        let mut paragraph = Map::new();
        if !report.style.is_empty() {
            paragraph.insert("style".to_string(), json!(report.style));
        }
        if !report.runs.is_empty() {
            paragraph.insert(
                "runs".to_string(),
                Value::Array(report.runs.into_iter().map(docx_rich_run_json).collect()),
            );
        }
        block.insert("paragraph".to_string(), Value::Object(paragraph));
    } else if report.kind == "table" {
        let rows: Vec<Value> = report
            .table_rows
            .iter()
            .map(|row| {
                let cells: Vec<Value> = row.iter().map(|text| json!({"text": text})).collect();
                json!({"cells": cells})
            })
            .collect();
        block.insert("table".to_string(), json!({"rows": rows}));
    }
    Value::Object(block)
}

fn docx_rich_run_json(run: DocxRichRunInfo) -> Value {
    let mut object = Map::new();
    object.insert("text".to_string(), json!(run.text));
    if run.bold {
        object.insert("bold".to_string(), json!(true));
    }
    if run.italic {
        object.insert("italic".to_string(), json!(true));
    }
    if !run.underline.is_empty() {
        object.insert("underline".to_string(), json!(run.underline));
    }
    if !run.color.is_empty() {
        object.insert("color".to_string(), json!(run.color));
    }
    if !run.size.is_empty() {
        object.insert("size".to_string(), json!(run.size));
    }
    Value::Object(object)
}

fn docx_rich_table_text(rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|row| row.join("\t"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn docx_rich_block_content_hash(kind: &str, style: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.as_bytes());
    hasher.update([0]);
    hasher.update(style.as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn docx_body_para_id_counts(xml: &str) -> CliResult<BTreeMap<String, usize>> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<String> = Vec::new();
    let mut word_stack: Vec<bool> = Vec::new();
    let mut counts = BTreeMap::new();
    let mut saw_root = false;
    let mut saw_body = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.is_empty() {
                    if name != "document" || !is_word {
                        return Err(CliError::unexpected("document root element not found"));
                    }
                    saw_root = true;
                }
                if stack.last().is_some_and(|parent| parent == "document")
                    && word_stack.last().copied().unwrap_or(false)
                    && name == "body"
                    && is_word
                {
                    saw_body = true;
                }
                if stack.last().is_some_and(|parent| parent == "body")
                    && name == "p"
                    && let Some(para_id) = docx_para_id_ns(&e, reader.resolver())
                {
                    *counts.entry(para_id.to_ascii_uppercase()).or_insert(0) += 1;
                }
                stack.push(name);
                word_stack.push(is_word);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.is_empty() {
                    if name != "document" || !is_word {
                        return Err(CliError::unexpected("document root element not found"));
                    }
                    saw_root = true;
                }
                if stack.last().is_some_and(|parent| parent == "document")
                    && word_stack.last().copied().unwrap_or(false)
                    && name == "body"
                    && is_word
                {
                    saw_body = true;
                }
                if stack.last().is_some_and(|parent| parent == "body")
                    && name == "p"
                    && let Some(para_id) = docx_para_id_ns(&e, reader.resolver())
                {
                    *counts.entry(para_id.to_ascii_uppercase()).or_insert(0) += 1;
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
                word_stack.pop();
            }
            Ok(Event::Eof) => {
                if !stack.is_empty() {
                    return Err(CliError::unexpected("invalid DOCX XML"));
                }
                break;
            }
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "failed to extract DOCX blocks: {err}"
                )));
            }
            _ => {}
        }
    }
    if !saw_root {
        return Err(CliError::unexpected("document root element not found"));
    }
    if !saw_body {
        return Err(CliError::unexpected("document body element not found"));
    }
    Ok(counts)
}

fn copy_zip_with_replacement(
    input: &str,
    output: &str,
    part: &str,
    old: &str,
    new: &str,
) -> CliResult<()> {
    if let Some(parent) = Path::new(output).parent() {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let in_file = File::open(input).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut archive =
        ZipArchive::new(in_file).map_err(|err| CliError::unexpected(err.to_string()))?;
    let out_file = File::create(output).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut writer = ZipWriter::new(out_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if entry.is_dir() {
            writer
                .add_directory(entry.name(), options)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
            continue;
        }
        writer
            .start_file(entry.name(), options)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if entry.name() == part {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
            writer
                .write_all(text.replace(old, new).as_bytes())
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        } else {
            std::io::copy(&mut entry, &mut writer)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        }
    }
    writer
        .finish()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

fn copy_zip_with_part_override(input: &str, output: &str, part: &str, text: &str) -> CliResult<()> {
    let mut overrides = BTreeMap::new();
    overrides.insert(part.to_string(), text.to_string());
    copy_zip_with_part_overrides(input, output, &overrides)
}

fn copy_zip_with_part_overrides(
    input: &str,
    output: &str,
    overrides: &BTreeMap<String, String>,
) -> CliResult<()> {
    if let Some(parent) = Path::new(output).parent() {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let in_file = File::open(input).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut archive =
        ZipArchive::new(in_file).map_err(|err| CliError::unexpected(err.to_string()))?;
    let out_file = File::create(output).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut writer = ZipWriter::new(out_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut written = BTreeSet::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if entry.is_dir() {
            writer
                .add_directory(entry.name(), options)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
            continue;
        }
        let name = entry.name().to_string();
        writer
            .start_file(&name, options)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if let Some(text) = overrides.get(&name) {
            writer
                .write_all(text.as_bytes())
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        } else {
            std::io::copy(&mut entry, &mut writer)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        }
        written.insert(name);
    }
    for (name, text) in overrides {
        if written.contains(name) {
            continue;
        }
        writer
            .start_file(name, options)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        writer
            .write_all(text.as_bytes())
            .map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    writer
        .finish()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

fn attr(e: &BytesStart<'_>, wanted_local: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local_name(a.key.as_ref()) == wanted_local {
            Some(decode_xml_text(a.value.as_ref()))
        } else {
            None
        }
    })
}

fn attr_prefixed_ns(
    e: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    wanted_prefix: &[u8],
    wanted_ns: &[u8],
    wanted_local: &[u8],
) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        let raw = a.key.as_ref();
        let colon = raw.iter().position(|byte| *byte == b':')?;
        if &raw[..colon] != wanted_prefix || &raw[colon + 1..] != wanted_local {
            return None;
        }
        let (resolved, local) = resolver.resolve_attribute(a.key);
        if matches!(resolved, ResolveResult::Bound(Namespace(uri)) if uri == wanted_ns)
            && local.as_ref() == wanted_local
        {
            Some(decode_xml_text(a.value.as_ref()))
        } else {
            None
        }
    })
}

fn attr_bound_ns(
    e: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    wanted_ns: &[u8],
    wanted_local: &[u8],
) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        let (resolved, local) = resolver.resolve_attribute(a.key);
        if matches!(resolved, ResolveResult::Bound(Namespace(uri)) if uri == wanted_ns)
            && local.as_ref() == wanted_local
        {
            Some(decode_xml_text(a.value.as_ref()))
        } else {
            None
        }
    })
}

fn attr_exact(e: &BytesStart<'_>, wanted: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if String::from_utf8_lossy(a.key.as_ref()) == wanted {
            Some(decode_xml_text(a.value.as_ref()))
        } else {
            None
        }
    })
}

fn local_name(name: &[u8]) -> &str {
    let raw = std::str::from_utf8(name).unwrap_or("");
    raw.rsplit_once(':').map(|(_, local)| local).unwrap_or(raw)
}

fn decode_xml_text(bytes: &[u8]) -> String {
    xml_unescape(&String::from_utf8_lossy(bytes))
}

fn xml_general_ref(bytes: &[u8]) -> String {
    match bytes {
        b"quot" => "\"".to_string(),
        b"apos" => "'".to_string(),
        b"lt" => "<".to_string(),
        b"gt" => ">".to_string(),
        b"amp" => "&".to_string(),
        _ => format!("&{};", String::from_utf8_lossy(bytes)),
    }
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
