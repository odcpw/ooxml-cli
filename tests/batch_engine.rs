use serde_json::{Value, json};
use std::fs;
use std::io::Write;
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
fn apply_resolves_named_operation_refs_and_dry_run_returns_the_resolved_plan() {
    let temp = temp_dir("refs");
    let input = temp.join("input.xlsx");
    let output = temp.join("output.xlsx");
    let ops = temp.join("ops.json");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("copy workbook");
    write_ops(
        &ops,
        json!([
            {"id": "data-sheet", "command": "xlsx sheets add", "args": {"name": "Data"}},
            {
                "id": "header-cell",
                "command": "xlsx cells set",
                "args": {
                    "sheet": {"$ref": "data-sheet.destination.name"},
                    "cell": "A1",
                    "value": "Revenue"
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
    assert_eq!(dry_json["plan"][1]["resolvedArgs"]["sheet"], "Data");
    assert!(
        !serde_json::to_string(&dry_json["plan"])
            .unwrap()
            .contains("$ref"),
        "resolved dry-run plan must not retain reference objects"
    );
    assert!(!output.exists(), "dry-run must not publish output");

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
    assert_eq!(applied_json["opsCount"], 2);
    assert_eq!(applied_json["applied"][0]["id"], "data-sheet");
    assert_eq!(applied_json["applied"][1]["id"], "header-cell");

    let readback = run(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        output.to_str().expect("output path"),
        "--sheet",
        "Data",
        "--range",
        "A1",
    ]);
    assert!(readback.status.success());
    assert_eq!(json_stdout(&readback)["values"][0][0], "Revenue");
    let strict = run(&[
        "--json",
        "validate",
        "--strict",
        output.to_str().expect("output path"),
    ]);
    assert!(
        strict.status.success(),
        "strict validation: {}",
        String::from_utf8_lossy(&strict.stderr)
    );

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
    assert_eq!(fs::read(&output).expect("read preserved output"), before);

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
