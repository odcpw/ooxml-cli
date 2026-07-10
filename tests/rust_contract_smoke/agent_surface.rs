// MCP and web-smoke agent-surface contract tests live here while shared protocol helpers remain in the parent integration test crate.
use super::*;

#[test]
fn apply_dry_run_plan_matches_rust_baseline() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-apply-dry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let ops = temp_dir.join("ops.json");
    fs::write(
        &ops,
        serde_json::to_string(&serde_json::json!([
            {
                "command": "xlsx cells set",
                "args": {"sheet": "1", "cell": "A1", "value": "apply-contract"}
            }
        ]))
        .expect("ops JSON"),
    )
    .expect("write ops");
    let ops_str = ops.to_str().expect("ops path");
    let args = [
        "--json",
        "apply",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--ops",
        ops_str,
        "--dry-run",
    ];

    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_code, baseline_code, "apply dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "apply dry-run stderr");
    assert_eq!(rust_stdout, baseline_stdout, "apply dry-run stdout");
}

#[test]
fn apply_batch_matches_rust_baseline_and_writes_valid_xlsx() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-apply-run-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let ops = temp_dir.join("ops.json");
    let baseline_out = temp_dir.join("baseline-out.xlsx");
    let rust_out = temp_dir.join("rust-out.xlsx");
    fs::write(
        &ops,
        serde_json::to_string(&serde_json::json!([
            {
                "command": "xlsx cells set",
                "args": {"sheet": "1", "cell": "A1", "value": "apply-contract"}
            }
        ]))
        .expect("ops JSON"),
    )
    .expect("write ops");
    let ops_str = ops.to_str().expect("ops path");
    let baseline_out_str = baseline_out.to_str().expect("baseline out path");
    let rust_out_str = rust_out.to_str().expect("rust out path");
    let baseline_args = [
        "--json",
        "apply",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--ops",
        ops_str,
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "apply",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--ops",
        ops_str,
        "--out",
        rust_out_str,
    ];

    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(
        rust_code, baseline_code,
        "apply run exit; baseline_stderr={baseline_stderr:?}; rust_stderr={rust_stderr:?}"
    );
    assert_eq!(rust_stderr, baseline_stderr, "apply run stderr");
    let baseline_json = baseline_stdout.expect("baseline apply stdout");
    let rust_json = rust_stdout.expect("rust apply stdout");
    assert_eq!(
        scrub_paths(rust_json.clone(), &[(rust_out_str, "[OUT_XLSX]")]),
        scrub_paths(baseline_json, &[(baseline_out_str, "[OUT_XLSX]")]),
        "apply run stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(
        &rust_json["applied"][0]["readback"],
        "rangesExportCommand",
    );

    let (export_code, export_stdout, export_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        rust_out_str,
        "--sheet",
        "1",
        "--range",
        "A1",
        "--include-types",
    ]);
    assert_eq!(export_code, 0, "apply output export exit");
    assert_eq!(export_stderr, None, "apply output export stderr");
    assert_eq!(
        export_stdout.expect("apply output export")["values"],
        serde_json::json!([["apply-contract"]])
    );
}

#[test]
fn apply_rejects_session_owned_nested_args_like_rust_baseline() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-apply-owned-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let ops = temp_dir.join("ops.json");
    fs::write(
        &ops,
        serde_json::to_string(&serde_json::json!([
            {
                "command": "xlsx cells set",
                "args": {
                    "sheet": "1",
                    "cell": "A1",
                    "value": "apply-contract",
                    "out": "nested.xlsx"
                }
            }
        ]))
        .expect("ops JSON"),
    )
    .expect("write ops");
    let ops_str = ops.to_str().expect("ops path");
    let args = [
        "--json",
        "apply",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--ops",
        ops_str,
        "--dry-run",
    ];

    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_code, baseline_code, "apply owned arg exit");
    assert_eq!(rust_stdout, baseline_stdout, "apply owned arg stdout");
    assert_eq!(rust_stderr, baseline_stderr, "apply owned arg stderr");
}

#[test]
fn find_to_ops_emits_apply_compatible_ops_for_xlsx_docx_pptx() {
    let cases = [
        (
            "xlsx",
            [
                "--json",
                "find",
                "Revenue",
                "testdata/xlsx/types-and-formulas/workbook.xlsx",
                "--replace",
                "Income",
                "--to-ops",
            ],
            "xlsx cells set",
            ("cell", "H:xlsx/ws:1/cell:a:B1"),
            ("value", "Income"),
        ),
        (
            "docx",
            [
                "--json",
                "find",
                "marked",
                "testdata/docx/paraid/document.docx",
                "--replace",
                "updated",
                "--to-ops",
            ],
            "docx paragraphs set",
            ("handle", "H:docx/pt:doc/para:m:1A2B3C4D"),
            ("text", "First updated paragraph"),
        ),
        (
            "pptx",
            [
                "--json",
                "find",
                "Content Slide",
                "testdata/pptx/title-content/presentation.pptx",
                "--replace",
                "SURVIVED",
                "--to-ops",
            ],
            "pptx replace text-occurrences",
            ("for-shape", "H:pptx/s:257/shape:n:2"),
            ("new-text", "SURVIVED"),
        ),
    ];

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-find-to-ops-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("find to-ops temp dir");

    for (label, args, command, target, replacement) in cases {
        let (code, stdout, stderr) = run_ooxml_raw(&args);
        assert_eq!(code, 0, "{label} find --to-ops exit; stderr={stderr}");
        let ops: Value = serde_json::from_str(stdout.trim()).expect("ops JSON");
        let ops = ops.as_array().expect("ops array");
        assert_eq!(ops.len(), 1, "{label} ops: {stdout}");
        assert_eq!(ops[0]["command"], command, "{label} command");
        assert_eq!(ops[0]["args"][target.0], target.1, "{label} target arg");
        assert_eq!(
            ops[0]["args"][replacement.0], replacement.1,
            "{label} replacement arg"
        );

        let ops_path = temp_dir.join(format!("{label}-ops.json"));
        fs::write(&ops_path, stdout).expect("write emitted ops");
        let input = args[3];
        let ops_str = ops_path.to_str().expect("ops path");
        let (apply_code, _apply_stdout, apply_stderr) =
            run_ooxml(&["--json", "apply", input, "--ops", ops_str, "--dry-run"]);
        assert_eq!(
            apply_code, 0,
            "{label} emitted ops should be accepted by apply; stderr={apply_stderr:?}"
        );
    }
}

#[test]
fn find_replace_apply_dry_run_matches_rust_baseline_for_xlsx() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-find-apply-dry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("find apply dry temp dir");
    let input = temp_dir.join("input.xlsx");
    fs::copy("testdata/xlsx/types-and-formulas/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path");
    let args = [
        "--json",
        "find",
        "Revenue",
        input_str,
        "--replace",
        "Income",
        "--apply",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline_raw(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml_raw(&args);
    assert_eq!(rust_code, baseline_code, "find apply dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "find apply dry-run stderr");
    let baseline_json: Value = serde_json::from_str(baseline_stdout.trim()).expect("baseline plan");
    let rust_json: Value = serde_json::from_str(rust_stdout.trim()).expect("rust plan");
    assert_eq!(rust_json, baseline_json, "find apply dry-run stdout");
}

#[test]
fn find_replace_apply_saved_outputs_read_back_and_validate() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-find-apply-saved-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("find apply saved temp dir");

    let cases = [
        (
            "xlsx",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
            "Revenue",
            "Income",
            "out.xlsx",
        ),
        (
            "docx",
            "testdata/docx/paraid/document.docx",
            "marked",
            "updated",
            "out.docx",
        ),
        (
            "pptx",
            "testdata/pptx/title-content/presentation.pptx",
            "Content Slide",
            "SURVIVED",
            "out.pptx",
        ),
    ];

    for (label, input, query, replacement, out_name) in cases {
        let out = temp_dir.join(out_name);
        let out_str = out.to_str().expect("out path");
        let (code, stdout, stderr) = run_ooxml_raw(&[
            "--json",
            "find",
            query,
            input,
            "--replace",
            replacement,
            "--apply",
            "--out",
            out_str,
        ]);
        assert_eq!(
            code, 0,
            "{label} find apply saved exit; stdout={stdout}; stderr={stderr}"
        );
        assert!(out.exists(), "{label} output exists");

        let (validate_code, _validate_stdout, validate_stderr) =
            run_ooxml(&["--json", "validate", out_str, "--strict"]);
        assert_eq!(
            validate_code, 0,
            "{label} strict validation exit; stderr={validate_stderr:?}"
        );

        let expected = if label == "docx" {
            "First updated paragraph"
        } else {
            replacement
        };
        let (read_code, read_stdout, read_stderr) =
            run_ooxml(&["--json", "find", expected, out_str]);
        assert_eq!(read_code, 0, "{label} readback exit");
        assert_eq!(read_stderr, None, "{label} readback stderr");
        assert_eq!(
            read_stdout.expect("readback stdout")["totalHits"],
            Value::from(1),
            "{label} replacement readback"
        );
    }
}

#[test]
fn find_compose_invalid_flag_combinations_match_rust_baseline() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "find",
            "Revenue",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
            "--apply",
            "--out",
            "x.xlsx",
        ],
        vec![
            "--json",
            "find",
            "Revenue",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
            "--replace",
            "Income",
        ],
        vec![
            "--json",
            "find",
            "Revenue",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
            "--replace",
            "Income",
            "--to-ops",
            "--apply",
            "--out",
            "x.xlsx",
        ],
        vec![
            "--json",
            "find",
            "Revenue",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
            "--replace",
            "",
            "--apply",
            "--out",
            "x.xlsx",
        ],
    ];
    for args in cases {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(
            rust_code, baseline_code,
            "invalid find flags exit for {args:?}"
        );
        assert_eq!(
            rust_stdout, baseline_stdout,
            "invalid find flags stdout for {args:?}"
        );
        assert_eq!(
            rust_stderr, baseline_stderr,
            "invalid find flags stderr for {args:?}"
        );
    }
}

#[test]
fn frozen_mcp_discovery_and_flow_match_legacy_baseline() {
    let baseline = baseline();
    let temp_dir = std::env::temp_dir().join(format!("ooxml-rust-mcp-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("mcp-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn mcp");
    let mut stdin = child.stdin.take().expect("mcp stdin");
    let stdout = child.stdout.take().expect("mcp stdout");
    let mut reader = BufReader::new(stdout);

    let initialize = rpc_request(
        1,
        "initialize",
        serde_json::json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "rust-contract", "version": "0.0.0"},
        }),
    );
    let initialize_response = serve_roundtrip(&mut stdin, &mut reader, &initialize);
    let tools_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(2, "tools/list", serde_json::json!({})),
    );
    let resources_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(3, "resources/list", serde_json::json!({})),
    );
    let templates_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(4, "resources/templates/list", serde_json::json!({})),
    );
    let command_uri = "resource://command/xlsx%20cells%20set";
    let command_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(5, "resources/read", serde_json::json!({"uri": command_uri})),
    );
    let discovery = serde_json::json!({
        "initialize": initialize_response["result"].clone(),
        "tools": summarize_mcp_tools(&tools_response["result"]),
        "resources": sort_by_string_field(resources_response["result"]["resources"].clone(), "uri"),
        "resourceTemplates": templates_response["result"]["resourceTemplates"].clone(),
        "commandResource": summarize_mcp_command_resource(&command_response["result"], command_uri),
    });
    let mut expected_discovery = baseline["mcp"]["discovery"].clone();
    expected_discovery["initialize"]["serverInfo"]["version"] =
        Value::String(env!("CARGO_PKG_VERSION").to_string());
    assert_eq!(discovery, expected_discovery);

    let mut replacements = vec![
        (input_str.clone(), "[MCP_INPUT_XLSX]".to_string()),
        (output_str.clone(), "[MCP_OUT_XLSX]".to_string()),
    ];
    let mut flow = Vec::new();

    let open = rpc_request(
        6,
        "tools/call",
        serde_json::json!({
            "name": "open",
            "arguments": {"file": input_str, "out": output_str},
        }),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["structuredContent"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();
    replacements.push((session.clone(), "[MCP_SESSION]".to_string()));
    flow.push(flow_item("tools/call", open, open_response, &replacements));

    let op = rpc_request(
        7,
        "tools/call",
        serde_json::json!({
            "name": "op",
            "arguments": {
                "session": session,
                "command": "xlsx cells set",
                "args": {"sheet": "1", "cell": "A1", "value": "mcp-contract"},
            },
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    let working = op_response["result"]["structuredContent"]["readback"]["file"]
        .as_str()
        .expect("working package")
        .to_string();
    replacements.push((working, "[SESSION_WORKING_PACKAGE]".to_string()));
    flow.push(flow_item("tools/call", op, op_response, &replacements));

    let inspect = rpc_request(
        8,
        "tools/call",
        serde_json::json!({
            "name": "inspect",
            "arguments": {
                "session": session,
                "command": "xlsx ranges export",
                "args": {"sheet": "1", "range": "A1", "include-types": true},
            },
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    flow.push(flow_item(
        "tools/call",
        inspect,
        inspect_response,
        &replacements,
    ));

    for (id, name) in [(9, "validate"), (10, "plan"), (11, "commit")] {
        let request = rpc_request(
            id,
            "tools/call",
            serde_json::json!({"name": name, "arguments": {"session": session}}),
        );
        let response = serve_roundtrip(&mut stdin, &mut reader, &request);
        flow.push(flow_item("tools/call", request, response, &replacements));
    }

    let dry_open = rpc_request(
        12,
        "tools/call",
        serde_json::json!({
            "name": "open",
            "arguments": {"file": input_str, "dryRun": true},
        }),
    );
    let dry_open_response = serve_roundtrip(&mut stdin, &mut reader, &dry_open);
    let dry_session = dry_open_response["result"]["structuredContent"]["sessionId"]
        .as_str()
        .expect("dry session id")
        .to_string();
    replacements.push((dry_session.clone(), "[MCP_DRY_RUN_SESSION]".to_string()));
    flow.push(flow_item(
        "tools/call",
        dry_open,
        dry_open_response,
        &replacements,
    ));

    let abort = rpc_request(
        13,
        "tools/call",
        serde_json::json!({"name": "abort", "arguments": {"session": dry_session}}),
    );
    let abort_response = serve_roundtrip(&mut stdin, &mut reader, &abort);
    flow.push(flow_item(
        "tools/call",
        abort,
        abort_response,
        &replacements,
    ));

    drop(stdin);
    let status = child.wait().expect("mcp exit");
    assert!(status.success());
    assert_eq!(Value::Array(flow), baseline["mcp"]["flow"]["flow"]);
}

#[test]
fn mcp_command_resources_cover_advertised_rust_capabilities() {
    let (cap_code, cap_stdout, cap_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(cap_code, 0);
    assert_eq!(cap_stderr, None);
    let capabilities = cap_stdout.expect("capabilities stdout");
    let commands = capabilities["commands"]
        .as_array()
        .expect("capability commands");

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn mcp");
    let mut stdin = child.stdin.take().expect("mcp stdin");
    let stdout = child.stdout.take().expect("mcp stdout");
    let mut reader = BufReader::new(stdout);

    let initialize = rpc_request(
        1,
        "initialize",
        serde_json::json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "rust-contract", "version": "0.0.0"},
        }),
    );
    let initialize_response = serve_roundtrip(&mut stdin, &mut reader, &initialize);
    assert!(
        initialize_response.get("error").is_none(),
        "initialize returned error: {initialize_response:?}"
    );

    let mut id = 2;
    let capabilities_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            id,
            "resources/read",
            serde_json::json!({"uri": "resource://capabilities"}),
        ),
    );
    id += 1;
    assert!(
        capabilities_response.get("error").is_none(),
        "capabilities resource returned error: {capabilities_response:?}"
    );
    let capabilities_text = capabilities_response["result"]["contents"][0]["text"]
        .as_str()
        .expect("capabilities resource text");
    let mcp_capabilities: Value =
        serde_json::from_str(capabilities_text).expect("MCP capabilities JSON");
    assert_eq!(
        mcp_capabilities["commands"], capabilities["commands"],
        "MCP capabilities should expose the same command inventory as CLI capabilities"
    );
    assert_eq!(
        mcp_capabilities["contractVersion"], capabilities["contractVersion"],
        "MCP capabilities should expose CLI contract version"
    );
    assert_eq!(
        mcp_capabilities["exitCodes"], capabilities["exitCodes"],
        "MCP capabilities should expose CLI exit-code contract"
    );
    assert_eq!(
        mcp_capabilities["resourceTemplates"][0]["uriTemplate"],
        Value::String("resource://command/{path}".to_string())
    );

    for command in commands {
        let path = command["path"].as_str().expect("command path");
        let mut request_paths = vec![path.to_string()];
        if let Some(shorthand) = path.strip_prefix("ooxml ") {
            request_paths.push(shorthand.to_string());
        }

        for request_path in request_paths {
            let uri = command_resource_uri(&request_path);
            let response = serve_roundtrip(
                &mut stdin,
                &mut reader,
                &rpc_request(id, "resources/read", serde_json::json!({"uri": uri})),
            );
            id += 1;
            assert!(
                response.get("error").is_none(),
                "command resource for {request_path:?} returned error: {response:?}"
            );
            let summary = summarize_mcp_command_resource(
                &response["result"],
                response["result"]["contents"][0]["uri"]
                    .as_str()
                    .expect("resource uri"),
            );
            assert_eq!(summary["path"], command["path"], "path for {request_path}");
            assert_eq!(
                summary["opCompatible"], command["opCompatible"],
                "opCompatible for {request_path}"
            );
            assert_eq!(
                summary["flags"],
                local_flag_field(command, "name"),
                "flags for {request_path}"
            );
            assert_eq!(
                summary["argNames"],
                local_flag_field(command, "argName"),
                "argNames for {request_path}"
            );
        }
    }

    let unknown = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            id,
            "resources/read",
            serde_json::json!({"uri": "resource://command/xlsx%20not%20real"}),
        ),
    );
    id += 1;
    assert!(
        unknown.get("error").is_some(),
        "unknown command resource should fail: {unknown:?}"
    );
    let bad_escape = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            id,
            "resources/read",
            serde_json::json!({"uri": "resource://command/xlsx%ZZbad"}),
        ),
    );
    assert!(
        bad_escape.get("error").is_some(),
        "invalid command resource URI should fail: {bad_escape:?}"
    );

    drop(stdin);
    let status = child.wait().expect("mcp exit");
    assert!(status.success());
}

#[test]
fn mcp_stdio_parse_errors_notifications_and_error_codes_are_json_rpc_compliant() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn mcp");
    let mut stdin = child.stdin.take().expect("mcp stdin");
    let stdout = child.stdout.take().expect("mcp stdout");
    let mut reader = BufReader::new(stdout);

    writeln!(stdin, "not json").expect("write malformed mcp line");
    stdin.flush().expect("flush malformed mcp line");
    let mut parse_line = String::new();
    reader
        .read_line(&mut parse_line)
        .expect("read parse error response");
    let parse_error: Value = serde_json::from_str(&parse_line).expect("parse error JSON");
    assert_eq!(parse_error["error"]["code"], -32700);
    assert_eq!(parse_error["id"], Value::Null);

    writeln!(
        stdin,
        "{}",
        serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {},
        }))
        .expect("serialize notification")
    )
    .expect("write initialized notification");
    let tools = rpc_request(1, "tools/list", serde_json::json!({}));
    let tools_response = serve_roundtrip(&mut stdin, &mut reader, &tools);
    assert!(
        tools_response.get("error").is_none(),
        "notification should not produce an intervening response: {tools_response:?}"
    );
    assert!(tools_response["result"]["tools"].is_array());

    let unknown_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(2, "not/a-method", serde_json::json!({})),
    );
    assert_eq!(unknown_response["error"]["code"], -32601);
    assert_eq!(
        unknown_response["error"]["data"]["exitCode"],
        Value::from(2)
    );

    drop(stdin);
    let status = child.wait().expect("mcp exit");
    assert!(status.success());
}

#[test]
fn web_smoke_binary_readback_checks_are_supported() {
    let baseline = baseline();
    let web_smoke = &baseline["webSmoke"];
    let checks = web_smoke["binaryReadbackChecks"]
        .as_array()
        .expect("web smoke readback checks")
        .iter()
        .map(|value| value.as_str().expect("check string"))
        .collect::<Vec<_>>();
    assert!(checks.contains(&"validate --strict"));
    assert!(checks.contains(&"pptx slides show"));
    assert!(checks.contains(&"docx text"));
    assert!(checks.contains(&"xlsx sheets list"));

    let pptx = web_smoke["agentDefaultFixture"]
        .as_str()
        .expect("pptx fixture");
    let docx = web_smoke["docxDefaultFixture"]
        .as_str()
        .expect("docx fixture");
    let xlsx = web_smoke["xlsxDefaultFixture"]
        .as_str()
        .expect("xlsx fixture");

    for file in [pptx, docx, xlsx] {
        let (code, stdout, stderr) = run_ooxml(&["--json", "--strict", "validate", file]);
        assert_eq!(code, 0, "validate exit for {file}");
        assert_eq!(stderr, None, "validate stderr for {file}");
        assert_eq!(stdout.expect("validate stdout")["valid"], Value::Bool(true));
    }

    let (pptx_code, pptx_stdout, pptx_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "slides",
        "show",
        pptx,
        "--slide",
        "1",
        "--include-text",
    ]);
    assert_eq!(pptx_code, 0);
    assert_eq!(pptx_stderr, None);
    assert_eq!(
        pptx_stdout.expect("pptx stdout")["slides"][0]["shapes"][0]["textContent"],
        Value::String("Minimal Title Slide".to_string())
    );
    for fixture in [
        pptx,
        "testdata/pptx/notes-slide/presentation.pptx",
        "testdata/pptx/table-slide/presentation.pptx",
        "testdata/pptx/corrupted-dangling-layout/presentation.pptx",
    ] {
        let pptx_list_args = ["--json", "pptx", "slides", "list", fixture];
        let (baseline_list_code, baseline_list_stdout, baseline_list_stderr) =
            run_ooxml_baseline(&pptx_list_args);
        let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&pptx_list_args);
        assert_eq!(
            rust_list_code, baseline_list_code,
            "pptx slides list exit for {fixture}"
        );
        assert_eq!(
            rust_list_stderr, baseline_list_stderr,
            "pptx slides list stderr for {fixture}"
        );
        assert_eq!(
            rust_list_stdout, baseline_list_stdout,
            "pptx slides list stdout for {fixture}"
        );
    }

    let pptx_selectors_args = [
        "--json",
        "pptx",
        "slides",
        "selectors",
        pptx,
        "--slide",
        "1",
    ];
    let (baseline_selectors_code, baseline_selectors_stdout, baseline_selectors_stderr) =
        run_ooxml_baseline(&pptx_selectors_args);
    let (rust_selectors_code, rust_selectors_stdout, rust_selectors_stderr) =
        run_ooxml(&pptx_selectors_args);
    assert_eq!(
        rust_selectors_code, baseline_selectors_code,
        "pptx slides selectors exit"
    );
    assert_eq!(
        rust_selectors_stderr, baseline_selectors_stderr,
        "pptx slides selectors stderr"
    );
    assert_eq!(
        rust_selectors_stdout, baseline_selectors_stdout,
        "pptx slides selectors stdout"
    );

    for args in [
        vec!["--json", "pptx", "extract", "text", pptx],
        vec![
            "--json",
            "pptx",
            "extract",
            "text",
            "testdata/pptx/title-content/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "text",
            "testdata/pptx/title-content/presentation.pptx",
            "--slide",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "text",
            "testdata/pptx/title-content/presentation.pptx",
            "--slide",
            "3",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "text",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ] {
        assert_rust_baseline_match(&args);
    }

    let commented_pptx_path = write_rust_baseline_pptx_comment_fixture();
    let commented_pptx = commented_pptx_path
        .to_str()
        .expect("commented PPTX fixture path");
    for args in [
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            "testdata/pptx/title-content/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            "testdata/pptx/title-content/presentation.pptx",
            "--slide",
            "1",
        ],
        vec!["--json", "pptx", "comments", "list", commented_pptx],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            commented_pptx,
            "--slide",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            commented_pptx,
            "--slide",
            "1",
            "--comment-id",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            commented_pptx,
            "--slide",
            "1",
            "--comment-id",
            "999",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            "testdata/pptx/title-content/presentation.pptx",
            "--slide",
            "999",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            "testdata/pptx/title-content/presentation.pptx",
            "--comment-id",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ] {
        assert_rust_baseline_match(&args);
    }

    for args in [
        vec![
            "--json",
            "pptx",
            "extract",
            "notes",
            "testdata/pptx/notes-slide/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "notes",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--slide",
            "2",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "notes",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--slide",
            "99",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "notes",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "pptx",
            "notes",
            "show",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--slide",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "notes",
            "show",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--slide",
            "2",
        ],
        vec!["--json", "pptx", "notes", "show", pptx, "--slide", "1"],
        vec![
            "--json",
            "pptx",
            "notes",
            "show",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--slide",
            "99",
        ],
        vec![
            "--json",
            "pptx",
            "notes",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--slide",
            "1",
        ],
    ] {
        assert_rust_baseline_match(&args);
    }

    for args in [
        vec![
            "--json",
            "pptx",
            "masters",
            "list",
            "testdata/pptx/minimal-title/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "list",
            "testdata/pptx/multi-layout/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "show",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--master",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "show",
            "testdata/pptx/multi-layout/presentation.pptx",
            "--master",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "show",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--master",
            "999",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--master",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "list",
            "testdata/pptx/minimal-title/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "list",
            "testdata/pptx/title-content/presentation.pptx",
            "--master",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "show",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--layout",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "show",
            "testdata/pptx/title-content/presentation.pptx",
            "--layout",
            "Title and Content",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "show",
            "testdata/pptx/title-content/presentation.pptx",
            "--layout",
            "NOPE",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--layout",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--target",
            "table:1",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--target",
            "@all-tables",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--details",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "99",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--target",
            "title",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--table-id",
            "999",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--slide",
            "1",
        ],
    ] {
        assert_rust_baseline_match(&args);
    }

    for args in [
        [
            "--json",
            "pptx",
            "shapes",
            "show",
            pptx,
            "--slide",
            "1",
            "--include-text",
            "--include-bounds",
        ],
        [
            "--json",
            "pptx",
            "shapes",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--include-text",
            "--include-bounds",
        ],
        [
            "--json",
            "pptx",
            "shapes",
            "show",
            "testdata/pptx/picture-placeholder/presentation.pptx",
            "--slide",
            "2",
            "--include-text",
            "--include-bounds",
        ],
    ] {
        let (baseline_shapes_code, baseline_shapes_stdout, baseline_shapes_stderr) =
            run_ooxml_baseline(&args);
        let (rust_shapes_code, rust_shapes_stdout, rust_shapes_stderr) = run_ooxml(&args);
        assert_eq!(
            rust_shapes_code, baseline_shapes_code,
            "pptx shapes show exit"
        );
        assert_eq!(
            rust_shapes_stderr, baseline_shapes_stderr,
            "pptx shapes show stderr for {args:?}"
        );
        assert_eq!(
            rust_shapes_stdout, baseline_shapes_stdout,
            "pptx shapes show stdout for {args:?}"
        );
    }

    let table_selectors_args = [
        "--json",
        "pptx",
        "slides",
        "selectors",
        "testdata/pptx/table-slide/presentation.pptx",
        "--slide",
        "2",
    ];
    let (
        baseline_table_selectors_code,
        baseline_table_selectors_stdout,
        baseline_table_selectors_stderr,
    ) = run_ooxml_baseline(&table_selectors_args);
    let (rust_table_selectors_code, rust_table_selectors_stdout, rust_table_selectors_stderr) =
        run_ooxml(&table_selectors_args);
    assert_eq!(
        rust_table_selectors_code, baseline_table_selectors_code,
        "pptx table selectors exit"
    );
    assert_eq!(
        rust_table_selectors_stderr, baseline_table_selectors_stderr,
        "pptx table selectors stderr"
    );
    assert_eq!(
        rust_table_selectors_stdout, baseline_table_selectors_stdout,
        "pptx table selectors stdout"
    );

    let (docx_code, docx_stdout, docx_stderr) = run_ooxml(&["--json", "docx", "text", docx]);
    assert_eq!(docx_code, 0);
    assert_eq!(docx_stderr, None);
    assert!(
        docx_stdout.expect("docx stdout")["blocks"]
            .as_array()
            .expect("docx blocks")
            .iter()
            .any(|block| block["text"]
                .as_str()
                .unwrap_or_default()
                .contains("Hello world"))
    );

    let xlsx_args = ["--json", "xlsx", "sheets", "list", xlsx];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&xlsx_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&xlsx_args);
    assert_eq!(rust_code, baseline_code, "xlsx sheets list exit");
    assert_eq!(rust_stderr, baseline_stderr, "xlsx sheets list stderr");
    assert_eq!(rust_stdout, baseline_stdout, "xlsx sheets list stdout");
}
