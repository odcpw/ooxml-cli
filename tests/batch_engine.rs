use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ooxml-batch-engine-{label}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create batch test directory");
    path
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn run_with_temp_root(args: &[&str], temp_root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .env("TMPDIR", temp_root)
        .output()
        .expect("run ooxml with isolated temp root")
}

fn json_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "parse JSON stdout: {error}; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn json_error(output: &Output) -> Value {
    let channel = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let value: Value = serde_json::from_slice(channel).unwrap_or_else(|error| {
        panic!(
            "parse JSON error envelope: {error}; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    value.get("error").cloned().unwrap_or(value)
}

fn write_ops(path: &Path, operations: Value) {
    fs::write(
        path,
        serde_json::to_vec_pretty(&operations).expect("serialize operations"),
    )
    .expect("write operations");
}

fn sha256(path: &Path) -> String {
    let mut hash = Sha256::new();
    hash.update(fs::read(path).expect("read file for SHA-256"));
    format!("{:x}", hash.finalize())
}

fn assert_strictly_valid(path: &Path, label: &str) {
    let validated = run(&[
        "--json",
        "validate",
        "--strict",
        path.to_str().expect("OOXML output path"),
    ]);
    assert!(
        validated.status.success(),
        "{label} strict validation failed; stdout={}; stderr={}",
        String::from_utf8_lossy(&validated.stdout),
        String::from_utf8_lossy(&validated.stderr)
    );
}

fn compare_or_update_golden(path: &Path, value: &Value) {
    let mut actual = serde_json::to_vec_pretty(value).expect("serialize batch golden");
    actual.push(b'\n');
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create batch golden directory");
        }
        fs::write(path, &actual).expect("update reviewed batch golden");
    }
    assert_eq!(
        fs::read(path).expect("read reviewed batch golden"),
        actual,
        "batch golden drift: {}",
        path.display()
    );
}

fn jsonl_roundtrip(stdin: &mut impl Write, reader: &mut impl BufRead, request: &Value) -> Value {
    writeln!(stdin, "{}", serde_json::to_string(request).unwrap()).expect("write JSONL request");
    stdin.flush().expect("flush JSONL request");
    let mut line = String::new();
    reader.read_line(&mut line).expect("read JSONL response");
    serde_json::from_str(&line).unwrap_or_else(|error| {
        panic!("parse JSONL response ({error}): request={request}; response={line}")
    })
}

fn run_jsonl_server(mode: &str, requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn JSONL server");
    {
        let stdin = child.stdin.as_mut().expect("server stdin");
        for request in requests {
            writeln!(stdin, "{}", serde_json::to_string(request).unwrap())
                .expect("write server request");
        }
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait for JSONL server");
    assert!(
        output.status.success(),
        "{mode} stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("server UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("server JSON response"))
        .collect()
}

fn op_compatible_commands() -> Vec<String> {
    let output = run(&["--json", "capabilities"]);
    assert!(
        output.status.success(),
        "capabilities failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    json_stdout(&output)["commands"]
        .as_array()
        .expect("capability commands")
        .iter()
        .filter(|command| command["opCompatible"] == true)
        .map(|command| {
            command["path"]
                .as_str()
                .expect("command path")
                .strip_prefix("ooxml ")
                .expect("ooxml command prefix")
                .to_string()
        })
        .collect()
}

fn fixture_for_operation(command: &str) -> &'static str {
    if command.starts_with("pptx ") {
        "testdata/pptx/title-content/presentation.pptx"
    } else if command.starts_with("docx ") {
        "testdata/docx/styled-headings/document.docx"
    } else if command.starts_with("vba ") || command == "convert xlsm-to-xlsx" {
        "testdata/golden/vba-authoring/xlsx-rebuilt/rebuilt.xlsm"
    } else {
        "testdata/xlsx/minimal-workbook/workbook.xlsx"
    }
}

#[test]
fn capabilities_publish_arg_schemas_for_the_150_batchable_package_mutations() {
    let output = run(&["--json", "capabilities"]);
    assert!(output.status.success());
    let capabilities = json_stdout(&output);
    let op_commands = capabilities["commands"]
        .as_array()
        .expect("capability commands")
        .iter()
        .filter(|command| command["opCompatible"] == true)
        .collect::<Vec<_>>();
    assert_eq!(
        op_commands.len(),
        154,
        "150 batchable package mutations plus four existing VBA package ops; top-level apply and conditionally mutating find are not nestable ops"
    );
    let existing_vba_ops = [
        "ooxml vba attach",
        "ooxml vba create",
        "ooxml vba rebuild",
        "ooxml vba remove",
    ];
    let inventory = op_commands
        .iter()
        .filter(|command| {
            !existing_vba_ops.contains(&command["path"].as_str().expect("command path"))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        inventory.len(),
        150,
        "batchable package-mutation denominator"
    );
    for command in inventory {
        let path = command["path"].as_str().expect("command path");
        let schema = command["opArgsSchema"]
            .as_object()
            .unwrap_or_else(|| panic!("missing opArgsSchema for {path}"));
        assert_eq!(schema["type"], "object", "arg schema type for {path}");
        assert_eq!(
            schema["additionalProperties"], false,
            "closed arg schema for {path}"
        );
    }
    for path in ["ooxml apply", "ooxml find"] {
        let command = capabilities["commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["path"] == path)
            .unwrap_or_else(|| panic!("missing capability {path}"));
        assert_eq!(command["opCompatible"], false, "{path} is not nestable");
        assert!(
            command["opIneligibleReason"].is_string(),
            "{path} remediation"
        );
        assert!(command.get("opArgsSchema").is_none(), "{path} op schema");
    }
    let apply = capabilities["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|command| command["path"] == "ooxml apply")
        .expect("apply capability");
    assert!(
        apply["localFlags"]
            .as_array()
            .expect("apply flags")
            .iter()
            .any(|flag| {
                flag["name"] == "--allow-absolute-paths"
                    && flag["argName"] == "allowAbsolutePaths"
                    && flag["type"] == "bool"
            }),
        "apply must advertise the explicit absolute-path opt-in"
    );
}

#[test]
fn all_154_op_compatible_commands_reach_apply_serve_and_mcp_dispatch() {
    let commands = op_compatible_commands();
    assert_eq!(commands.len(), 154, "op-compatible command denominator");
    assert_eq!(
        commands.iter().collect::<BTreeSet<_>>().len(),
        commands.len(),
        "op-compatible commands must be unique"
    );
    for command in &commands {
        assert!(
            Path::new(fixture_for_operation(command)).is_file(),
            "missing package fixture for {command}"
        );
    }

    let temp = temp_dir("all-dispatch");
    let ops = temp.join("one-op.json");
    for command in &commands {
        write_ops(&ops, json!([{"command": command, "args": {}}]));
        let applied = run(&[
            "--json",
            "apply",
            fixture_for_operation(command),
            "--ops",
            ops.to_str().expect("ops path"),
            "--dry-run",
        ]);
        if !applied.status.success() {
            let error = json_error(&applied);
            let message = error["message"].as_str().unwrap_or_default();
            assert!(
                !message.contains("unknown command")
                    && !message.contains("cannot be used as an apply/serve/MCP op")
                    && !message.contains("unsupported serve op command"),
                "apply did not dispatch {command}: error={error}; ops={}",
                fs::read_to_string(&ops).expect("read failing op fixture")
            );
        }
    }

    for mode in ["serve", "mcp"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
            .arg(mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("spawn {mode}: {error}"));
        let mut stdin = child.stdin.take().expect("server stdin");
        let mut reader = BufReader::new(child.stdout.take().expect("server stdout"));
        for (index, command) in commands.iter().enumerate() {
            let request_id = index * 3 + 1;
            let open_arguments = json!({"file": fixture_for_operation(command)});
            let open_request = if mode == "serve" {
                json!({"jsonrpc": "2.0", "id": request_id, "method": "open", "params": open_arguments})
            } else {
                json!({"jsonrpc": "2.0", "id": request_id, "method": "tools/call", "params": {"name": "open", "arguments": open_arguments}})
            };
            let opened = jsonl_roundtrip(&mut stdin, &mut reader, &open_request);
            let open_result = if mode == "serve" {
                &opened["result"]
            } else {
                &opened["result"]["structuredContent"]
            };
            let session = open_result["sessionId"].as_str().unwrap_or_else(|| {
                panic!(
                    "{mode} open failed for {command}: request={open_request}; response={opened}"
                )
            });
            let op_arguments = json!({"session": session, "command": command, "args": {}});
            let op_request = if mode == "serve" {
                json!({"jsonrpc": "2.0", "id": request_id + 1, "method": "op", "params": op_arguments})
            } else {
                json!({"jsonrpc": "2.0", "id": request_id + 1, "method": "tools/call", "params": {"name": "op", "arguments": op_arguments}})
            };
            let response = jsonl_roundtrip(&mut stdin, &mut reader, &op_request);
            let encoded = serde_json::to_string(&response).expect("encode op response");
            assert!(
                !encoded.contains("unsupported serve op command"),
                "{mode} did not dispatch {command}: request={op_request}; response={response}"
            );

            let abort_arguments = json!({"session": session});
            let abort_request = if mode == "serve" {
                json!({"jsonrpc": "2.0", "id": request_id + 2, "method": "abort", "params": abort_arguments})
            } else {
                json!({"jsonrpc": "2.0", "id": request_id + 2, "method": "tools/call", "params": {"name": "abort", "arguments": abort_arguments}})
            };
            let aborted = jsonl_roundtrip(&mut stdin, &mut reader, &abort_request);
            assert!(
                !serde_json::to_string(&aborted)
                    .expect("encode abort response")
                    .contains("session not found"),
                "{mode} failed to abort fixture session for {command}: {aborted}"
            );
        }
        drop(stdin);
        assert!(
            child.wait().expect("wait for dispatch server").success(),
            "{mode} dispatch server failed"
        );
    }

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn serve_and_mcp_resolve_refs_through_the_same_session_engine() {
    let temp = temp_dir("serve-mcp-refs");
    let input = temp.join("input.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("copy workbook");

    let tools = run_jsonl_server(
        "mcp",
        &[json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})],
    );
    let op_schema = tools[0]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "op")
        .expect("MCP op tool")["inputSchema"]
        .clone();
    assert_eq!(op_schema["properties"]["id"]["type"], "string");
    assert!(
        op_schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("args"))
    );

    for mode in ["serve", "mcp"] {
        let output = temp.join(format!("{mode}.xlsx"));
        let open_params = json!({"file": input, "out": output});
        let first_op = json!({
            "session": "rust-session-1",
            "id": "data-sheet",
            "command": "xlsx sheets add",
            "args": {"name": "Data"}
        });
        let second_op = json!({
            "session": "rust-session-1",
            "id": "header-cell",
            "command": "xlsx cells set",
            "args": {
                "sheet": {"$ref": "data-sheet.destination.name"},
                "cell": "A1",
                "value": mode
            }
        });
        let requests = if mode == "serve" {
            vec![
                json!({"jsonrpc": "2.0", "id": 1, "method": "open", "params": open_params}),
                json!({"jsonrpc": "2.0", "id": 2, "method": "op", "params": first_op}),
                json!({"jsonrpc": "2.0", "id": 3, "method": "op", "params": second_op}),
                json!({"jsonrpc": "2.0", "id": 4, "method": "commit", "params": {"session": "rust-session-1"}}),
            ]
        } else {
            vec![
                json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "open", "arguments": open_params}}),
                json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "op", "arguments": first_op}}),
                json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "op", "arguments": second_op}}),
                json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "commit", "arguments": {"session": "rust-session-1"}}}),
            ]
        };
        let responses = run_jsonl_server(mode, &requests);
        let second = if mode == "serve" {
            &responses[2]["result"]
        } else {
            &responses[2]["result"]["structuredContent"]
        };
        assert_eq!(second["id"], "header-cell", "named result through {mode}");
        assert_eq!(
            second["resolvedArgs"]["sheet"], "Data",
            "resolved ref through {mode}"
        );
        assert!(output.exists(), "{mode} commit output");
        let strict = run(&[
            "--json",
            "validate",
            "--strict",
            output.to_str().expect("output path"),
        ]);
        assert!(strict.status.success(), "{mode} strict validation");
    }

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn positive_family_fixtures_commit_valid_packages_on_apply_serve_and_mcp() {
    let temp = temp_dir("positive-surfaces");
    let cases = [
        (
            "xlsx",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "xlsx cells set",
            json!({"sheet": "Sheet1", "cell": "A1", "value": "batch surface"}),
        ),
        (
            "pptx",
            "testdata/pptx/title-content/presentation.pptx",
            "pptx replace text",
            json!({"slide": 1, "target": "title", "text": "Batch surface"}),
        ),
        (
            "docx",
            "testdata/docx/styled-headings/document.docx",
            "docx paragraphs set",
            json!({"index": 1, "text": "Batch surface"}),
        ),
    ];

    for (family, fixture, command, args) in &cases {
        let ops = temp.join(format!("apply-{family}.json"));
        let output = temp.join(format!("apply.{family}"));
        write_ops(
            &ops,
            json!([{"id": "positive", "command": command, "args": args}]),
        );
        let applied = run(&[
            "--json",
            "apply",
            fixture,
            "--ops",
            ops.to_str().expect("ops path"),
            "--out",
            output.to_str().expect("output path"),
        ]);
        assert!(
            applied.status.success(),
            "apply {command} failed; plan={}; stdout={}; stderr={}",
            fs::read_to_string(&ops).expect("read positive apply plan"),
            String::from_utf8_lossy(&applied.stdout),
            String::from_utf8_lossy(&applied.stderr)
        );
        let result = json_stdout(&applied);
        assert_eq!(result["opsCount"], 1, "apply {command}: {result}");
        assert_eq!(result["applied"][0]["id"], "positive");
        assert!(
            result["applied"][0]["mutationEnvelope"].is_object(),
            "apply {command} omitted per-op envelope: {result}"
        );
        assert_strictly_valid(&output, &format!("apply {command}"));
    }

    for mode in ["serve", "mcp"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
            .arg(mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("spawn {mode}: {error}"));
        let mut stdin = child.stdin.take().expect("surface stdin");
        let mut reader = BufReader::new(child.stdout.take().expect("surface stdout"));
        for (index, (family, fixture, command, args)) in cases.iter().enumerate() {
            let output = temp.join(format!("{mode}-{index}.{family}"));
            let open_arguments = json!({"file": fixture, "out": output});
            let open_request = if mode == "serve" {
                json!({"jsonrpc":"2.0","id":index * 3 + 1,"method":"open","params":open_arguments})
            } else {
                json!({"jsonrpc":"2.0","id":index * 3 + 1,"method":"tools/call","params":{"name":"open","arguments":open_arguments}})
            };
            let opened = jsonl_roundtrip(&mut stdin, &mut reader, &open_request);
            let open_result = if mode == "serve" {
                &opened["result"]
            } else {
                &opened["result"]["structuredContent"]
            };
            let session = open_result["sessionId"].as_str().unwrap_or_else(|| {
                panic!(
                    "{mode} open failed for {command}: request={open_request}; response={opened}"
                )
            });
            let op_arguments =
                json!({"session":session,"id":"positive","command":command,"args":args});
            let op_request = if mode == "serve" {
                json!({"jsonrpc":"2.0","id":index * 3 + 2,"method":"op","params":op_arguments})
            } else {
                json!({"jsonrpc":"2.0","id":index * 3 + 2,"method":"tools/call","params":{"name":"op","arguments":op_arguments}})
            };
            let mutated = jsonl_roundtrip(&mut stdin, &mut reader, &op_request);
            let op_result = if mode == "serve" {
                &mutated["result"]
            } else {
                &mutated["result"]["structuredContent"]
            };
            assert!(
                op_result["mutationEnvelope"].is_object(),
                "{mode} {command} failed or omitted its envelope: request={op_request}; response={mutated}"
            );

            let commit_arguments = json!({"session":session});
            let commit_request = if mode == "serve" {
                json!({"jsonrpc":"2.0","id":index * 3 + 3,"method":"commit","params":commit_arguments})
            } else {
                json!({"jsonrpc":"2.0","id":index * 3 + 3,"method":"tools/call","params":{"name":"commit","arguments":commit_arguments}})
            };
            let committed = jsonl_roundtrip(&mut stdin, &mut reader, &commit_request);
            let commit_result = if mode == "serve" {
                &committed["result"]
            } else {
                &committed["result"]["structuredContent"]
            };
            assert_eq!(
                commit_result["validated"], true,
                "{mode} {command} commit failed; op={mutated}; commit={committed}"
            );
            assert_eq!(commit_result["opsCount"], 1);
            assert!(commit_result["applied"][0]["mutationEnvelope"].is_object());
            assert_strictly_valid(&output, &format!("{mode} {command}"));
        }
        drop(stdin);
        assert!(child.wait().expect("wait for positive server").success());
    }

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn five_op_ref_batch_pins_the_fully_resolved_dry_run_plan() {
    let temp = temp_dir("five-op-refs");
    let input = temp.join("new-workbook.xlsx");
    let output = temp.join("output.xlsx");
    let ops = temp.join("ops.json");
    write_ops(
        &ops,
        json!([
            {"id": "workbook", "command": "xlsx scaffold", "args": {}},
            {"id": "data-sheet", "command": "xlsx sheets add", "args": {"name": "Data"}},
            {
                "id": "data-range",
                "command": "xlsx ranges set",
                "args": {
                    "sheet": {"$ref": "data-sheet.destination.name"},
                    "range": "A1:B2",
                    "values": "[[\"Metric\",\"Value\"],[\"Revenue\",42]]"
                }
            },
            {
                "id": "data-table",
                "command": "xlsx tables create",
                "args": {
                    "sheet": {"$ref": "data-sheet.destination.name"},
                    "range": {"$ref": "data-range.destination.range"},
                    "table": "DataTable"
                }
            },
            {
                "id": "data-note",
                "command": "xlsx comments add",
                "args": {
                    "sheet": {"$ref": "data-sheet.destination.name"},
                    "cell": "A2",
                    "author": "Batch proof",
                    "text": "Created after the referenced range and table"
                }
            }
        ]),
    );

    let dry = run(&[
        "--json",
        "apply",
        input.to_str().expect("input path"),
        "--ops",
        ops.to_str().expect("ops path"),
        "--dry-run",
    ]);
    assert!(
        dry.status.success(),
        "dry-run stderr: {}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let dry_json = json_stdout(&dry);
    assert_eq!(dry_json["dryRun"], true);
    assert_eq!(dry_json["committed"], false);
    assert_eq!(dry_json["opsCount"], 5);
    assert_eq!(dry_json["plan"][2]["resolvedArgs"]["sheet"], "Data");
    assert_eq!(dry_json["plan"][3]["resolvedArgs"]["range"], "A1:B2");
    assert_eq!(dry_json["plan"][4]["resolvedArgs"]["sheet"], "Data");
    assert!(
        !serde_json::to_string(&dry_json["plan"])
            .unwrap()
            .contains("$ref"),
        "resolved dry-run plan must not retain reference objects"
    );
    assert!(!output.exists(), "dry-run must not publish output");
    let plan_projection = json!({
        "schemaVersion": dry_json["schemaVersion"],
        "dryRun": dry_json["dryRun"],
        "committed": dry_json["committed"],
        "opsCount": dry_json["opsCount"],
        "plan": dry_json["plan"]
            .as_array()
            .expect("dry-run plan")
            .iter()
            .map(|operation| json!({
                "index": operation["index"],
                "id": operation.get("id").cloned().unwrap_or(Value::Null),
                "command": operation["command"],
                "resolvedArgs": operation["resolvedArgs"],
            }))
            .collect::<Vec<_>>(),
    });
    compare_or_update_golden(
        Path::new("testdata/golden/batch-engine/five-op-resolved-plan.json"),
        &plan_projection,
    );

    let applied = run(&[
        "--json",
        "apply",
        input.to_str().expect("input path"),
        "--ops",
        ops.to_str().expect("ops path"),
        "--out",
        output.to_str().expect("output path"),
    ]);
    assert!(
        applied.status.success(),
        "apply stderr: {}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let applied_json = json_stdout(&applied);
    assert_eq!(applied_json["opsCount"], 5);
    assert_eq!(applied_json["applied"][0]["id"], "workbook");
    assert_eq!(applied_json["applied"][1]["id"], "data-sheet");
    assert_eq!(applied_json["applied"][2]["id"], "data-range");
    assert_eq!(applied_json["applied"][3]["id"], "data-table");
    assert_eq!(applied_json["applied"][4]["id"], "data-note");

    let readback = run(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        output.to_str().expect("output path"),
        "--sheet",
        "Data",
        "--range",
        "A1:B2",
    ]);
    assert!(readback.status.success());
    assert_eq!(
        json_stdout(&readback)["values"],
        json!([["Metric", "Value"], ["Revenue", 42]])
    );
    assert_strictly_valid(&output, "five-op referenced workbook");

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn unresolved_ref_discards_the_stage_and_preserves_an_existing_output() {
    let temp = temp_dir("atomic-ref-failure");
    let input = temp.join("input.xlsx");
    let output = temp.join("output.xlsx");
    let ops = temp.join("ops.json");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("copy input");
    fs::copy(&input, &output).expect("seed existing output");
    let before = fs::read(&output).expect("read existing output");
    write_ops(
        &ops,
        json!([
            {"id": "data-sheet", "command": "xlsx sheets add", "args": {"name": "Data"}},
            {
                "command": "xlsx cells set",
                "args": {
                    "sheet": {"$ref": "missing.destination.name"},
                    "cell": "A1",
                    "value": "must not publish"
                }
            }
        ]),
    );

    let failed = run(&[
        "--json",
        "apply",
        input.to_str().expect("input path"),
        "--ops",
        ops.to_str().expect("ops path"),
        "--out",
        output.to_str().expect("output path"),
    ]);
    assert!(!failed.status.success());
    let error = json_error(&failed);
    assert_eq!(error["code"], "invalid_args");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("unresolved $ref")
    );
    let message = error["message"].as_str().expect("reference error message");
    assert!(
        message.contains("op 1 (xlsx cells set) failed"),
        "{message}"
    );
    assert!(message.contains("missing.destination.name"), "{message}");
    assert!(message.contains("operation id \"missing\""), "{message}");
    assert_eq!(fs::read(&output).expect("read preserved output"), before);

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn self_and_cyclic_refs_fail_before_publish_with_named_operations() {
    let temp = temp_dir("cyclic-refs");
    let input = temp.join("input.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("copy input");

    let cases = [
        (
            "self",
            json!([{
                "id": "self",
                "command": "xlsx cells set",
                "args": {
                    "sheet": {"$ref": "self.destination.name"},
                    "cell": "A1",
                    "value": "must not publish"
                }
            }]),
            "operation id \"self\" has not completed",
        ),
        (
            "cycle",
            json!([
                {
                    "id": "first",
                    "command": "xlsx cells set",
                    "args": {
                        "sheet": {"$ref": "second.destination.name"},
                        "cell": "A1",
                        "value": "first"
                    }
                },
                {
                    "id": "second",
                    "command": "xlsx cells set",
                    "args": {
                        "sheet": {"$ref": "first.destination.name"},
                        "cell": "A2",
                        "value": "second"
                    }
                }
            ]),
            "operation id \"second\" has not completed",
        ),
    ];

    for (label, operations, expected) in cases {
        let ops = temp.join(format!("{label}.json"));
        let output = temp.join(format!("{label}.xlsx"));
        write_ops(&ops, operations);
        let failed = run(&[
            "--json",
            "apply",
            input.to_str().expect("input path"),
            "--ops",
            ops.to_str().expect("ops path"),
            "--out",
            output.to_str().expect("output path"),
        ]);
        assert!(
            !failed.status.success(),
            "{label} reference unexpectedly passed"
        );
        let error = json_error(&failed);
        let message = error["message"].as_str().expect("reference error message");
        assert!(message.contains(expected), "{label}: {error}");
        assert!(!output.exists(), "{label} reference published output");
    }

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn middle_op_failure_preserves_output_and_in_place_hashes_and_cleans_stages() {
    let temp = temp_dir("atomic-middle-failure");
    let child_temp = temp.join("child-temp");
    fs::create_dir_all(&child_temp).expect("create isolated child temp root");
    let input = temp.join("input.xlsx");
    let output = temp.join("existing-output.xlsx");
    let ops = temp.join("ops.json");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("copy input");
    fs::copy(&input, &output).expect("seed existing output");
    write_ops(
        &ops,
        json!([
            {
                "id": "first",
                "command": "xlsx cells set",
                "args": {"sheet": "Sheet1", "cell": "A1", "value": "staged only"}
            },
            {
                "id": "bad-selector",
                "command": "xlsx cells set",
                "args": {"sheet": "Missing sheet", "cell": "A1", "value": "fail"}
            },
            {
                "id": "never-runs",
                "command": "xlsx cells set",
                "args": {"sheet": "Sheet1", "cell": "A2", "value": "unreachable"}
            }
        ]),
    );
    let input_hash = sha256(&input);
    let output_hash = sha256(&output);

    let failed_output = run_with_temp_root(
        &[
            "--json",
            "apply",
            input.to_str().expect("input path"),
            "--ops",
            ops.to_str().expect("ops path"),
            "--out",
            output.to_str().expect("output path"),
        ],
        &child_temp,
    );
    assert!(!failed_output.status.success());
    let output_error = json_error(&failed_output);
    assert!(
        output_error["message"]
            .as_str()
            .is_some_and(|message| message.contains("op 1 (xlsx cells set) failed")),
        "output failure did not name the middle op: error={output_error}; plan={}",
        fs::read_to_string(&ops).expect("read failing plan")
    );
    assert_eq!(
        sha256(&input),
        input_hash,
        "source changed during --out failure"
    );
    assert_eq!(sha256(&output), output_hash, "existing output was replaced");
    assert_eq!(
        fs::read_dir(&child_temp)
            .expect("read isolated child temp root")
            .count(),
        0,
        "failed --out batch leaked a session stage"
    );

    let failed_in_place = run_with_temp_root(
        &[
            "--json",
            "apply",
            input.to_str().expect("input path"),
            "--ops",
            ops.to_str().expect("ops path"),
            "--in-place",
        ],
        &child_temp,
    );
    assert!(!failed_in_place.status.success());
    assert_eq!(
        sha256(&input),
        input_hash,
        "failed --in-place batch changed the input package"
    );
    assert_eq!(
        fs::read_dir(&child_temp)
            .expect("read isolated child temp root")
            .count(),
        0,
        "failed --in-place batch leaked a session stage"
    );

    let implicit_in_place = run(&[
        "--json",
        "apply",
        input.to_str().expect("input path"),
        "--ops",
        ops.to_str().expect("ops path"),
    ]);
    assert!(!implicit_in_place.status.success());
    assert_eq!(
        sha256(&input),
        input_hash,
        "apply mutated without --in-place"
    );
    assert!(
        json_error(&implicit_in_place)["message"]
            .as_str()
            .is_some_and(|message| message.contains("--out") && message.contains("--in-place")),
        "missing explicit destination error: {}",
        json_error(&implicit_in_place)
    );

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn relative_input_paths_are_ops_local_and_cannot_escape_the_ops_directory() {
    let temp = temp_dir("path-safety");
    let project = temp.join("project");
    fs::create_dir_all(&project).expect("create project directory");
    fs::copy("testdata/test_image.png", project.join("asset.png")).expect("copy image");
    fs::copy("testdata/test_image.png", temp.join("outside.png")).expect("copy outside image");
    let output = temp.join("deck.pptx");
    let safe_ops = project.join("safe.json");
    write_ops(
        &safe_ops,
        json!([
            {"id": "deck", "command": "pptx scaffold", "args": {"title": "Batch deck"}},
            {
                "command": "pptx place image",
                "args": {
                    "slide": 1,
                    "image": "asset.png",
                    "x": 914400,
                    "y": 1600000,
                    "cx": 1200000,
                    "cy": 700000
                }
            }
        ]),
    );
    let virtual_input = temp.join("new-session.pptx");
    let safe = run(&[
        "--json",
        "apply",
        virtual_input.to_str().expect("virtual input"),
        "--ops",
        safe_ops.to_str().expect("safe ops"),
        "--out",
        output.to_str().expect("output path"),
    ]);
    assert!(
        safe.status.success(),
        "safe relative path stderr: {}",
        String::from_utf8_lossy(&safe.stderr)
    );
    assert!(output.exists());
    let strict = run(&[
        "--json",
        "validate",
        "--strict",
        output.to_str().expect("output path"),
    ]);
    assert!(strict.status.success());

    let unsafe_ops = project.join("unsafe.json");
    write_ops(
        &unsafe_ops,
        json!([
            {"command": "pptx scaffold", "args": {"title": "Unsafe"}},
            {
                "command": "pptx place image",
                "args": {
                    "slide": 1,
                    "image": "../outside.png",
                    "x": 0,
                    "y": 0,
                    "cx": 1000000,
                    "cy": 1000000
                }
            }
        ]),
    );
    let unsafe_output = temp.join("unsafe.pptx");
    let unsafe_run = run(&[
        "--json",
        "apply",
        virtual_input.to_str().expect("virtual input"),
        "--ops",
        unsafe_ops.to_str().expect("unsafe ops"),
        "--out",
        unsafe_output.to_str().expect("unsafe output"),
    ]);
    assert!(!unsafe_run.status.success());
    assert!(
        json_error(&unsafe_run)["message"]
            .as_str()
            .unwrap()
            .contains("escapes the ops directory")
    );
    assert!(!unsafe_output.exists());

    let unsafe_with_opt_in = run(&[
        "--json",
        "apply",
        virtual_input.to_str().expect("virtual input"),
        "--ops",
        unsafe_ops.to_str().expect("unsafe ops"),
        "--out",
        unsafe_output.to_str().expect("unsafe output"),
        "--allow-absolute-paths",
    ]);
    assert!(
        !unsafe_with_opt_in.status.success(),
        "absolute-path opt-in must not permit relative parent traversal"
    );
    assert!(!unsafe_output.exists());

    let absolute_ops = project.join("absolute.json");
    let absolute_asset = project.join("asset.png");
    write_ops(
        &absolute_ops,
        json!([
            {"command": "pptx scaffold", "args": {"title": "Absolute path opt-in"}},
            {
                "command": "pptx place image",
                "args": {
                    "slide": 1,
                    "image": absolute_asset,
                    "x": 0,
                    "y": 0,
                    "cx": 1000000,
                    "cy": 1000000
                }
            }
        ]),
    );
    let absolute_output = temp.join("absolute.pptx");
    let absolute_denied = run(&[
        "--json",
        "apply",
        virtual_input.to_str().expect("virtual input"),
        "--ops",
        absolute_ops.to_str().expect("absolute ops"),
        "--out",
        absolute_output.to_str().expect("absolute output"),
    ]);
    assert!(!absolute_denied.status.success());
    assert!(
        json_error(&absolute_denied)["message"]
            .as_str()
            .is_some_and(|message| message.contains("--allow-absolute-paths")),
        "absolute path denial: {}",
        json_error(&absolute_denied)
    );
    assert!(!absolute_output.exists());

    let absolute_allowed = run(&[
        "--json",
        "apply",
        virtual_input.to_str().expect("virtual input"),
        "--ops",
        absolute_ops.to_str().expect("absolute ops"),
        "--out",
        absolute_output.to_str().expect("absolute output"),
        "--allow-absolute-paths",
    ]);
    assert!(
        absolute_allowed.status.success(),
        "absolute path opt-in failed; stdout={}; stderr={}",
        String::from_utf8_lossy(&absolute_allowed.stdout),
        String::from_utf8_lossy(&absolute_allowed.stderr)
    );
    assert_strictly_valid(&absolute_output, "absolute-path opt-in deck");

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn one_apply_process_scaffolds_fills_and_charts_a_strictly_valid_deck() {
    let temp = temp_dir("pptx-build-batch");
    let virtual_input = temp.join("new-deck.pptx");
    let output = temp.join("built-deck.pptx");
    let ops = temp.join("deck-ops.json");
    write_ops(
        &ops,
        json!([
            {
                "id": "deck",
                "command": "pptx scaffold",
                "args": {"title": "Quarterly Review", "subtitle": "Batch-built"}
            },
            {
                "id": "results-slide",
                "command": "pptx new-slide-from-layout",
                "args": {
                    "layout": "Title and Content",
                    "setText": ["title=Results", "body=Revenue grew across both regions"]
                }
            },
            {
                "id": "results-chart",
                "command": "pptx charts create",
                "args": {
                    "slide": {"$ref": "results-slide.destination.slide"},
                    "type": "bar",
                    "title": "Revenue",
                    "valuesJson": "[[\"\",\"North\",\"South\"],[\"Q1\",10,20],[\"Q2\",15,25]]",
                    "slot": "body"
                }
            }
        ]),
    );

    let applied = run(&[
        "--json",
        "apply",
        virtual_input.to_str().expect("virtual deck path"),
        "--ops",
        ops.to_str().expect("ops path"),
        "--out",
        output.to_str().expect("output path"),
    ]);
    assert!(
        applied.status.success(),
        "single-process deck batch stderr: {}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let result = json_stdout(&applied);
    assert_eq!(result["opsCount"], 3);
    assert_eq!(result["applied"].as_array().unwrap().len(), 3);
    assert!(
        result["applied"]
            .as_array()
            .unwrap()
            .iter()
            .all(|operation| operation["mutationEnvelope"].is_object()),
        "each operation must retain its ordered MutationEnvelope"
    );
    assert_eq!(
        result["applied"][2]["resolvedArgs"]["slide"], 2,
        "chart must target the slide created by the preceding named op"
    );
    assert_eq!(result["validated"], true);
    assert!(output.is_file(), "the batch publishes one final package");

    let strict = run(&[
        "--json",
        "validate",
        "--strict",
        output.to_str().expect("output path"),
    ]);
    assert!(
        strict.status.success(),
        "strict validation stderr: {}",
        String::from_utf8_lossy(&strict.stderr)
    );
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn output_extension_changing_conversion_runs_inside_the_session_stage() {
    let temp = temp_dir("convert-op");
    let input = temp.join("macro.xlsm");
    let output = temp.join("macro-free.xlsx");
    let ops = temp.join("convert.json");
    fs::copy(
        "testdata/golden/vba-authoring/xlsx-rebuilt/rebuilt.xlsm",
        &input,
    )
    .expect("copy XLSM fixture");
    write_ops(
        &ops,
        json!([{"id": "conversion", "command": "convert xlsm-to-xlsx", "args": {}}]),
    );

    let applied = run(&[
        "--json",
        "apply",
        input.to_str().expect("input path"),
        "--ops",
        ops.to_str().expect("ops path"),
        "--out",
        output.to_str().expect("output path"),
    ]);
    assert!(
        applied.status.success(),
        "conversion op stderr: {}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let result = json_stdout(&applied);
    assert_eq!(result["applied"][0]["id"], "conversion");
    assert_eq!(result["validated"], true);
    let strict = run(&[
        "--json",
        "validate",
        "--strict",
        output.to_str().expect("output path"),
    ]);
    assert!(strict.status.success(), "converted XLSX strict validation");
    let _ = fs::remove_dir_all(temp);
}
