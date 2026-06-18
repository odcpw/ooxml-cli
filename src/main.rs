use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
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
            let sheet = parse_string_flag(rest, "--sheet")?.unwrap_or_else(|| "1".to_string());
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            xlsx_range_export(file, &sheet, &range)
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
        "objectKinds": ["package", "slide", "shape", "sheet", "range", "cell", "style"],
        "objectKindsIndex": {
            "package": ["ooxml inspect", "ooxml validate", "ooxml verify"],
            "slide": ["ooxml pptx slides list", "ooxml pptx slides selectors", "ooxml pptx slides show", "ooxml pptx shapes show", "ooxml pptx replace text", "ooxml pptx render"],
            "shape": ["ooxml pptx slides list", "ooxml pptx slides selectors", "ooxml pptx slides show", "ooxml pptx shapes show", "ooxml pptx replace text"],
            "sheet": ["ooxml xlsx sheets list", "ooxml xlsx ranges export", "ooxml xlsx cells extract", "ooxml xlsx cells set"],
            "range": ["ooxml xlsx ranges export", "ooxml xlsx cells extract"],
            "cell": ["ooxml xlsx cells set"],
            "style": []
        },
        "exitCodes": [
            {"code": EXIT_SUCCESS, "name": "success", "description": "command completed successfully"},
            {"code": EXIT_UNEXPECTED, "name": "unexpected", "description": "unexpected tool or package processing error"},
            {"code": EXIT_INVALID_ARGS, "name": "invalid_args", "description": "invalid command line arguments or incompatible options"},
            {"code": EXIT_FILE_NOT_FOUND, "name": "file_not_found", "description": "input file was not found"}
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

fn inspect(file: &str) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    if entries.iter().any(|name| name == "ppt/presentation.xml") {
        let presentation = zip_text(file, "ppt/presentation.xml")?;
        let (cx, cy) = pptx_slide_size(&presentation)?;
        return Ok(json!({
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
        }));
    }
    Err(CliError::invalid_args(format!(
        "unsupported file type for inspect: {file}"
    )))
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

fn xlsx_range_export(file: &str, sheet_selector: &str, range: &str) -> CliResult<Value> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook);
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
    let bounds = parse_range(range)?;
    let mut values = Vec::new();
    let mut types = Vec::new();
    let mut formula_count = 0;
    for row in bounds.start_row..=bounds.end_row {
        let mut row_values = Vec::new();
        let mut row_types = Vec::new();
        for col in bounds.start_col..=bounds.end_col {
            let addr = format!("{}{}", col_name(col), row);
            if let Some(cell) = cells.get(&addr) {
                if cell.has_formula {
                    formula_count += 1;
                }
                row_values.push(cell.matrix_value.clone());
                row_types.push(Value::String(cell.kind.clone()));
            } else {
                row_values.push(Value::Null);
                row_types.push(Value::String("empty".to_string()));
            }
        }
        values.push(Value::Array(row_values));
        types.push(Value::Array(row_types));
    }
    let rows = bounds.end_row - bounds.start_row + 1;
    let cols = bounds.end_col - bounds.start_col + 1;
    Ok(json!({
        "cellsExtractCommand": format!("ooxml --json xlsx cells extract {file} --sheet {} --range {range}", sheet.name),
        "cols": cols,
        "dataFormat": "json",
        "file": file,
        "formulaCount": formula_count,
        "majorDimension": "rows",
        "pptxPlaceTableCommandTemplate": format!("ooxml --json pptx place table-from-xlsx deck.pptx --workbook {file} --sheet {} --range {range} --expect-source-range {range} --slide 1 --x 0 --y 0 --cx 4000000 --out out.pptx", sheet.name),
        "pptxReplaceTextCommandTemplate": format!("ooxml --json pptx replace text-from-xlsx deck.pptx --workbook {file} --sheet {} --range {range} --slide 1 --target title --out out.pptx", sheet.name),
        "pptxUpdateTableCommandTemplate": format!("ooxml --json pptx tables update-from-xlsx deck.pptx --workbook {file} --sheet {} --range {range} --expect-source-range {range} --slide 1 --target table:1 --out out.pptx", sheet.name),
        "primarySelector": range,
        "range": range,
        "rows": rows,
        "selectors": [range],
        "sheet": sheet.name,
        "sheetNumber": sheet.position,
        "truncated": false,
        "types": types,
        "validateCommand": format!("ooxml validate --strict {file}"),
        "values": values,
    }))
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
    let sheets = workbook_sheets(&workbook);
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
    let range_bounds = range.map(parse_range).transpose()?;
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
    let sheets = workbook_sheets(&workbook);
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
        "validateCommand": format!("ooxml validate --strict {file}"),
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
            "ooxml --json xlsx tables list {file} --sheet {selector}"
        )),
    );
    item.insert(
        "setCellCommandTemplate".to_string(),
        json!(format!(
            "ooxml --json xlsx cells set {file} --sheet {selector} --cell A1 --value VALUE --out out.xlsx"
        )),
    );
    if let Some(range_ref) = used_range_ref {
        item.insert(
            "cellsExtractCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx cells extract {file} --sheet {selector} --range {range_ref}"
            )),
        );
        item.insert(
            "rangesExportCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx ranges export {file} --sheet {selector} --range {range_ref} --include-types"
            )),
        );
        item.insert(
            "setRangeCommandTemplate".to_string(),
            json!(format!(
                "ooxml --json xlsx ranges set {file} --sheet {selector} --range {range_ref} --data-format json --values-file values.json --out out.xlsx"
            )),
        );
    }
    Value::Object(item)
}

fn xlsx_sheets_list(file: &str) -> CliResult<Value> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook);
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
                "showCommand": format!("ooxml --json xlsx sheets show {file} --sheet {primary_selector}"),
                "tablesListCommand": format!("ooxml --json xlsx tables list {file} --sheet {primary_selector}"),
            })
        })
        .collect();
    Ok(json!({
        "file": file,
        "validateCommand": format!("ooxml validate --strict {file}"),
        "sheets": values,
    }))
}

fn docx_text(file: &str) -> CliResult<Value> {
    let xml = zip_text(file, "word/document.xml")?;
    let paragraphs = docx_paragraphs(&xml);
    let blocks: Vec<Value> = paragraphs
        .into_iter()
        .enumerate()
        .filter_map(|(idx, text)| {
            if text.is_empty() {
                None
            } else {
                Some(json!({"index": idx + 1, "kind": "paragraph", "text": text}))
            }
        })
        .collect();
    Ok(json!({"blocks": blocks, "file": file}))
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
                xlsx_range_export(&session.working, &sheet, &range)
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
    json!({
        "commands": capability_commands(),
        "packageTypes": ["pptx", "xlsx", "docx"],
        "resourceTemplates": [mcp_command_resource_template()],
        "tool": "ooxml",
        "version": "0.0.1"
    })
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
    let sheets = workbook_sheets(&workbook);
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

fn workbook_sheets(xml: &str) -> Vec<WorkbookSheet> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut sheets = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sheet" =>
            {
                if let (Some(name), Some(number), Some(rel_id)) = (
                    attr(&e, "name"),
                    attr(&e, "sheetId"),
                    attr_exact(&e, "r:id"),
                ) && let Ok(number) = number.parse::<u32>()
                {
                    sheets.push(WorkbookSheet {
                        name,
                        sheet_id: number,
                        position: sheets.len() as u32 + 1,
                        rel_id,
                        state: attr(&e, "state").unwrap_or_else(|| "visible".to_string()),
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    sheets
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
    col >= bounds.start_col
        && col <= bounds.end_col
        && row >= bounds.start_row
        && row <= bounds.end_row
}

#[derive(Clone, Copy)]
struct RangeBounds {
    start_col: u32,
    start_row: u32,
    end_col: u32,
    end_row: u32,
}

fn parse_range(range: &str) -> CliResult<RangeBounds> {
    let mut parts = range.split(':');
    let start = parts
        .next()
        .ok_or_else(|| CliError::invalid_args("range is empty"))?;
    let end = parts.next().unwrap_or(start);
    let (start_col, start_row) = parse_cell_ref(start)?;
    let (end_col, end_row) = parse_cell_ref(end)?;
    Ok(RangeBounds {
        start_col,
        start_row,
        end_col,
        end_row,
    })
}

fn parse_cell_ref(cell: &str) -> CliResult<(u32, u32)> {
    let mut col = 0u32;
    let mut row = String::new();
    for ch in cell.chars() {
        if ch.is_ascii_alphabetic() {
            col = col * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        } else if ch.is_ascii_digit() {
            row.push(ch);
        }
    }
    let row = row
        .parse::<u32>()
        .map_err(|_| CliError::invalid_args(format!("invalid cell reference: {cell}")))?;
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

fn docx_paragraphs(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut in_p = false;
    let mut in_t = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "p" => {
                in_p = true;
                current.clear();
            }
            Ok(Event::Start(e)) if in_p && local_name(e.name().as_ref()) == "t" => in_t = true,
            Ok(Event::Text(e)) if in_t => current.push_str(&String::from_utf8_lossy(e.as_ref())),
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => in_t = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "p" => {
                paragraphs.push(current.clone());
                in_p = false;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    paragraphs
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
            writer
                .write_all(text.as_bytes())
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

fn attr(e: &BytesStart<'_>, wanted_local: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local_name(a.key.as_ref()) == wanted_local {
            Some(String::from_utf8_lossy(a.value.as_ref()).to_string())
        } else {
            None
        }
    })
}

fn attr_exact(e: &BytesStart<'_>, wanted: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if String::from_utf8_lossy(a.key.as_ref()) == wanted {
            Some(String::from_utf8_lossy(a.value.as_ref()).to_string())
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
