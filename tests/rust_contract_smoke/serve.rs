// Serve/session contract tests live here while shared JSON-RPC and scrub helpers remain in the parent integration test crate.
use super::*;

#[test]
fn frozen_serve_flow_matches_legacy_baseline() {
    let baseline = baseline();
    let temp_dir = std::env::temp_dir().join(format!("ooxml-rust-serve-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);
    let mut replacements = vec![
        (input_str.clone(), "[SERVE_INPUT_XLSX]".to_string()),
        (output_str.clone(), "[SERVE_OUT_XLSX]".to_string()),
    ];
    let mut flow = Vec::new();

    let open = rpc_request(
        1,
        "open",
        serde_json::json!({"file": input_str, "out": output_str}),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();
    replacements.push((session.clone(), "[SESSION]".to_string()));
    flow.push(flow_item("open", open, open_response, &replacements));

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx cells set",
            "args": {"sheet": "1", "cell": "A1", "value": "serve-contract"},
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    let working = op_response["result"]["readback"]["file"]
        .as_str()
        .expect("working package")
        .to_string();
    replacements.push((working, "[SESSION_WORKING_PACKAGE]".to_string()));
    flow.push(flow_item("op", op, op_response, &replacements));

    let inspect = rpc_request(
        3,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx ranges export",
            "args": {"sheet": "1", "range": "A1", "include-types": true},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    flow.push(flow_item(
        "inspect",
        inspect,
        inspect_response,
        &replacements,
    ));

    for (id, method) in [(4, "validate"), (5, "plan"), (6, "commit")] {
        let request = rpc_request(id, method, serde_json::json!({"session": session}));
        let response = serve_roundtrip(&mut stdin, &mut reader, &request);
        flow.push(flow_item(method, request, response, &replacements));
    }

    let dry_open = rpc_request(
        7,
        "open",
        serde_json::json!({"file": input_str, "dryRun": true}),
    );
    let dry_open_response = serve_roundtrip(&mut stdin, &mut reader, &dry_open);
    let dry_session = dry_open_response["result"]["sessionId"]
        .as_str()
        .expect("dry session id")
        .to_string();
    replacements.push((dry_session.clone(), "[DRY_RUN_SESSION]".to_string()));
    flow.push(flow_item(
        "open",
        dry_open,
        dry_open_response,
        &replacements,
    ));

    let abort = rpc_request(8, "abort", serde_json::json!({"session": dry_session}));
    let abort_response = serve_roundtrip(&mut stdin, &mut reader, &abort);
    flow.push(flow_item("abort", abort, abort_response, &replacements));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    assert_eq!(Value::Array(flow), baseline["serve"]["flow"]);
}

#[test]
fn serve_commit_does_not_write_dry_run_output() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-dryrun-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("dry-run-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            1,
            "open",
            serde_json::json!({"file": input_str, "out": output_str, "dryRun": true}),
        ),
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let op_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "op",
            serde_json::json!({
                "session": session,
                "command": "xlsx cells set",
                "args": {"sheet": "1", "cell": "A1", "value": "dry-run-commit"},
            }),
        ),
    );
    assert!(
        op_response.get("error").is_none(),
        "dry-run op returned error: {op_response:?}"
    );

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(3, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "dry-run commit returned error: {commit_response:?}"
    );
    assert_eq!(commit_response["result"]["dryRun"], Value::Bool(true));
    assert_eq!(commit_response["result"]["committed"], Value::Bool(false));
    assert_eq!(commit_response["result"]["output"], Value::Null);
    assert_eq!(
        commit_response["result"]["plannedOutput"],
        Value::String(output_str)
    );
    assert!(
        !output.exists(),
        "dry-run commit must not create the requested output"
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_open_supports_in_place_backup_commit() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-in-place-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let backup = temp_dir.join("input.xlsx.bak");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let backup_str = backup.to_str().expect("backup path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            1,
            "open",
            serde_json::json!({
                "file": input_str,
                "inPlace": true,
                "backup": backup_str,
            }),
        ),
    );
    assert!(
        open_response.get("error").is_none(),
        "in-place open failed: {open_response:?}"
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let op_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "op",
            serde_json::json!({
                "session": session,
                "command": "xlsx cells set",
                "args": {"sheet": "1", "cell": "A1", "value": "serve-in-place"},
            }),
        ),
    );
    assert!(
        op_response.get("error").is_none(),
        "in-place op failed: {op_response:?}"
    );

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(3, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "in-place commit failed: {commit_response:?}"
    );
    assert_eq!(
        commit_response["result"]["output"],
        Value::String(input_str.clone())
    );
    assert_eq!(
        commit_response["result"]["validateCommand"],
        Value::String(format!("ooxml validate --strict {input_str}"))
    );
    assert!(backup.exists(), "in-place backup missing");

    let (input_code, input_stdout, input_stderr) = run_ooxml(&[
        "--json", "xlsx", "ranges", "export", &input_str, "--sheet", "1", "--range", "A1",
    ]);
    assert_eq!(input_code, 0, "in-place output readback exit");
    assert_eq!(input_stderr, None, "in-place output readback stderr");
    assert_eq!(
        input_stdout.expect("in-place output readback")["values"][0][0],
        Value::String("serve-in-place".to_string())
    );

    let (backup_code, backup_stdout, backup_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        &backup_str,
        "--sheet",
        "1",
        "--range",
        "A1",
    ]);
    assert_eq!(backup_code, 0, "backup readback exit");
    assert_eq!(backup_stderr, None, "backup readback stderr");
    assert_ne!(
        backup_stdout.expect("backup readback")["values"][0][0],
        Value::String("serve-in-place".to_string())
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

include!("serve/docx.rs");

include!("serve/xlsx.rs");

#[test]
fn serve_op_rejects_unknown_family_commands_in_dispatchers() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-unknown-family-op-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(1, "open", serde_json::json!({"file": input_str}));
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    assert!(
        open_response.get("error").is_none(),
        "open failed: {open_response:?}"
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    for (offset, command) in ["xlsx not-real", "docx not-real", "pptx not-real"]
        .iter()
        .enumerate()
    {
        let response = serve_roundtrip(
            &mut stdin,
            &mut reader,
            &rpc_request(
                2 + offset as i64,
                "op",
                serde_json::json!({
                    "session": session,
                    "command": command,
                    "args": {},
                }),
            ),
        );
        assert!(
            response.get("error").is_some(),
            "unknown serve op command should fail: {response:?}"
        );
        assert_eq!(
            response["error"]["data"]["type"],
            Value::String("invalid_args".to_string())
        );
        assert_eq!(
            response["error"]["message"],
            Value::String(format!("unsupported serve op command: {command}"))
        );
    }

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

include!("serve/pptx.rs");
