use serde_json::{Map, Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct RpcProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl RpcProcess {
    fn spawn(mode: &str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
            .arg(mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("spawn {mode}: {error}"));
        let stdin = child.stdin.take().expect("RPC stdin");
        let stdout = BufReader::new(child.stdout.take().expect("RPC stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn call(&mut self, id: i64, method: &str, params: Value) -> Value {
        let request = json!({
            "id": id,
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        writeln!(
            self.stdin,
            "{}",
            serde_json::to_string(&request).expect("serialize RPC request")
        )
        .expect("write RPC request");
        self.stdin.flush().expect("flush RPC request");
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read RPC response");
        assert!(!line.trim().is_empty(), "empty {method} response");
        serde_json::from_str(&line).expect("parse RPC response")
    }

    fn finish(mut self) {
        drop(self.stdin);
        assert!(self.child.wait().expect("wait for RPC process").success());
    }
}

fn direct_json(args: &[&str]) -> Value {
    direct_json_with_exit(args, 0)
}

fn direct_json_with_exit(args: &[&str], expected_exit: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run direct CLI");
    assert_eq!(
        output.status.code(),
        Some(expected_exit),
        "direct CLI exit: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "direct CLI emitted stderr");
    serde_json::from_slice(&output.stdout).expect("direct CLI JSON")
}

fn open_session(process: &mut RpcProcess, mcp: bool, id: i64, file: &str) -> String {
    let response = if mcp {
        process.call(
            id,
            "tools/call",
            json!({"name": "open", "arguments": {"file": file}}),
        )
    } else {
        process.call(id, "open", json!({"file": file}))
    };
    assert!(response.get("error").is_none(), "open failed: {response}");
    let result = if mcp {
        &response["result"]["structuredContent"]
    } else {
        &response["result"]
    };
    result["sessionId"].as_str().expect("sessionId").to_string()
}

fn inspect(
    process: &mut RpcProcess,
    mcp: bool,
    id: i64,
    session: &str,
    command: &str,
    args: Value,
) -> Value {
    let response = if mcp {
        process.call(
            id,
            "tools/call",
            json!({
                "name": "inspect",
                "arguments": {"session": session, "command": command, "args": args},
            }),
        )
    } else {
        process.call(
            id,
            "inspect",
            json!({"session": session, "command": command, "args": args}),
        )
    };
    assert!(
        response.get("error").is_none(),
        "{command} inspect failed: {response}"
    );
    if mcp {
        response["result"]["structuredContent"].clone()
    } else {
        response["result"].clone()
    }
}

fn replace_string(value: &mut Value, needle: &str, replacement: &str) {
    match value {
        Value::String(text) => *text = text.replace(needle, replacement),
        Value::Array(items) => {
            for item in items {
                replace_string(item, needle, replacement);
            }
        }
        Value::Object(fields) => {
            for field in fields.values_mut() {
                replace_string(field, needle, replacement);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn canonical_working_path(mut value: Value) -> Value {
    let path = value["file"]
        .as_str()
        .expect("inspect envelope file")
        .to_string();
    replace_string(&mut value, &path, "<working-file>");
    value
}

#[test]
fn serve_and_mcp_outline_and_check_equal_the_direct_cli_and_each_other() {
    let file = "testdata/pptx/title-content/presentation.pptx";
    let mut serve = RpcProcess::spawn("serve");
    let mut mcp = RpcProcess::spawn("mcp");
    let serve_session = open_session(&mut serve, false, 1, file);
    let mcp_session = open_session(&mut mcp, true, 2, file);

    let serve_outline = inspect(
        &mut serve,
        false,
        3,
        &serve_session,
        "outline",
        json!({"depth": 2, "slide": 1, "textPreview": 24}),
    );
    let mcp_outline = inspect(
        &mut mcp,
        true,
        4,
        &mcp_session,
        "outline",
        json!({"depth": 2, "slide": 1, "textPreview": 24}),
    );
    let serve_outline_file = serve_outline["file"].as_str().expect("Serve outline file");
    let mcp_outline_file = mcp_outline["file"].as_str().expect("MCP outline file");
    assert_eq!(
        serve_outline,
        direct_json(&[
            "--json",
            "outline",
            serve_outline_file,
            "--depth",
            "2",
            "--slide",
            "1",
            "--text-preview",
            "24",
        ])
    );
    assert_eq!(
        mcp_outline,
        direct_json(&[
            "--json",
            "outline",
            mcp_outline_file,
            "--depth",
            "2",
            "--slide",
            "1",
            "--text-preview",
            "24",
        ])
    );
    assert_eq!(
        canonical_working_path(serve_outline),
        canonical_working_path(mcp_outline),
        "Serve and MCP outline envelopes"
    );

    let serve_check = inspect(
        &mut serve,
        false,
        5,
        &serve_session,
        "check",
        json!({"openxml-sdk": "skip"}),
    );
    let mcp_check = inspect(
        &mut mcp,
        true,
        6,
        &mcp_session,
        "check",
        json!({"openxml-sdk": "skip"}),
    );
    let serve_check_file = serve_check["file"].as_str().expect("Serve check file");
    let mcp_check_file = mcp_check["file"].as_str().expect("MCP check file");
    assert_eq!(
        serve_check,
        direct_json(&["--json", "check", serve_check_file, "--openxml-sdk", "skip",])
    );
    assert_eq!(
        mcp_check,
        direct_json(&["--json", "check", mcp_check_file, "--openxml-sdk", "skip",])
    );
    assert_eq!(
        canonical_working_path(serve_check),
        canonical_working_path(mcp_check),
        "Serve and MCP check envelopes"
    );

    serve.finish();
    mcp.finish();
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn object_schema(value: &Value) -> Value {
    Value::Object(
        value
            .as_object()
            .expect("schema source object")
            .iter()
            .map(|(key, value)| (key.clone(), json!(value_type(value))))
            .collect::<Map<_, _>>(),
    )
}

fn first_nested_item<'a>(value: &'a Value, parent: &str, child: &str) -> &'a Value {
    value[parent]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|item| item[child].as_array().into_iter().flatten())
        .next()
        .unwrap_or_else(|| panic!("missing {parent}.{child} schema sample"))
}

fn schema_contract(command: &str) -> Value {
    if command == "outline" {
        let envelope = direct_json(&[
            "--json",
            "outline",
            "testdata/pptx/chart-simple/presentation.pptx",
        ]);
        json!({
            "envelope": object_schema(&envelope),
            "summary": object_schema(&envelope["summary"]),
            "slide": object_schema(&envelope["slides"][0]),
            "shape": object_schema(first_nested_item(&envelope, "slides", "shapes")),
            "chart": object_schema(first_nested_item(&envelope, "slides", "charts")),
        })
    } else {
        let envelope = direct_json_with_exit(
            &[
                "--json",
                "check",
                "testdata/xlsx/invalid/pivot-table-parts.xlsx",
                "--openxml-sdk",
                "skip",
            ],
            5,
        );
        json!({
            "envelope": object_schema(&envelope),
            "proofLevel": object_schema(&envelope["proofLevel"]),
            "checks": object_schema(&envelope["checks"]),
            "summary": object_schema(&envelope["summary"]),
            "finding": object_schema(&envelope["findings"][0]),
        })
    }
}

fn assert_or_update_schema_golden(directory: &str, actual: &Value) {
    let path = Path::new("testdata/golden")
        .join(directory)
        .join("envelope-schema.json");
    let mut bytes = serde_json::to_vec_pretty(actual).expect("serialize schema golden");
    bytes.push(b'\n');
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        fs::write(&path, &bytes).expect("write schema golden");
        return;
    }
    assert_eq!(
        bytes,
        fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
        "{}",
        path.display()
    );
}

#[test]
fn every_outline_and_check_json_golden_is_lf_only() {
    for directory in ["testdata/golden/outline", "testdata/golden/check"] {
        for entry in fs::read_dir(directory).expect("read golden directory") {
            let path = entry.expect("read golden entry").path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path).expect("read JSON golden");
            assert!(
                !bytes.contains(&b'\r'),
                "{} contains CR bytes",
                path.display()
            );
            assert!(bytes.ends_with(b"\n"), "{} must end in LF", path.display());
        }
    }
}

#[test]
fn outline_and_check_envelope_schemas_match_platform_independent_goldens() {
    assert_or_update_schema_golden("outline", &schema_contract("outline"));
    assert_or_update_schema_golden("check", &schema_contract("check"));
}
