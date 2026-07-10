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

#[test]
fn serve_dispatches_every_op_compatible_capability_to_validation() {
    let (cap_code, cap_stdout, cap_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(cap_code, 0);
    assert_eq!(cap_stderr, None);
    let caps = cap_stdout.expect("capabilities");
    let commands = caps["commands"]
        .as_array()
        .expect("commands array")
        .iter()
        .filter(|command| command["opCompatible"].as_bool().unwrap_or(false))
        .map(|command| {
            command["path"]
                .as_str()
                .expect("command path")
                .strip_prefix("ooxml ")
                .expect("ooxml prefix")
                .to_string()
        })
        .collect::<Vec<_>>();
    let command_set = commands.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(commands.len(), 70, "advertised opCompatible command count");
    assert_eq!(
        command_set.len(),
        commands.len(),
        "advertised opCompatible commands must be unique"
    );
    let frozen_caps: Value = serde_json::from_str(include_str!(
        "../../testdata/golden/command-manifest-contract/capabilities.json"
    ))
    .expect("frozen capabilities JSON");
    let frozen_command_set = frozen_caps["commands"]
        .as_array()
        .expect("frozen capability commands")
        .iter()
        .filter(|command| command["opCompatible"].as_bool().unwrap_or(false))
        .map(|command| {
            command["path"]
                .as_str()
                .expect("frozen command path")
                .strip_prefix("ooxml ")
                .expect("frozen ooxml prefix")
                .to_string()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(frozen_command_set.len(), 70);
    assert_eq!(
        command_set, frozen_command_set,
        "advertised opCompatible set drifted from the committed contract"
    );

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-op-compatible-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);
    let mut id = 1_i64;

    for command in commands {
        let input = temp_dir.join(format!(
            "input-{id}.{}",
            fixture_extension_for_command(&command)
        ));
        fs::copy(fixture_for_command(&command), &input).expect("stage fixture");
        let input_str = input.to_str().expect("input path").to_string();
        let open_response = serve_roundtrip(
            &mut stdin,
            &mut reader,
            &rpc_request(id, "open", serde_json::json!({"file": input_str})),
        );
        id += 1;
        assert!(
            open_response.get("error").is_none(),
            "open failed for {command}: {open_response:?}"
        );
        let session = open_response["result"]["sessionId"]
            .as_str()
            .expect("session id")
            .to_string();
        let response = serve_roundtrip(
            &mut stdin,
            &mut reader,
            &rpc_request(
                id,
                "op",
                serde_json::json!({
                    "session": session,
                    "command": command,
                    "args": {},
                }),
            ),
        );
        id += 1;
        if let Some(error) = response.get("error") {
            let message = error["message"].as_str().unwrap_or_default();
            assert!(
                !message.starts_with("unsupported serve op command:"),
                "opCompatible command did not reach argument validation: {command}: {response:?}"
            );
        }
    }

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_stdio_parse_errors_are_json_rpc_errors_and_loop_continues() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    writeln!(stdin, "not json").expect("write malformed serve line");
    stdin.flush().expect("flush malformed serve line");
    let mut parse_line = String::new();
    reader
        .read_line(&mut parse_line)
        .expect("read parse error response");
    let parse_error: Value = serde_json::from_str(&parse_line).expect("parse error JSON");
    assert_eq!(parse_error["error"]["code"], -32700);
    assert_eq!(parse_error["id"], Value::Null);

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-parse-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let open_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(1, "open", serde_json::json!({"file": input_str})),
    );
    assert!(
        open_response.get("error").is_none(),
        "serve did not continue after parse error: {open_response:?}"
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

fn fixture_for_command(command: &str) -> &'static str {
    if command.starts_with("pptx ") {
        "testdata/pptx/title-content/presentation.pptx"
    } else if command.starts_with("docx ") {
        "testdata/docx/minimal/document.docx"
    } else {
        "testdata/xlsx/minimal-workbook/workbook.xlsx"
    }
}

fn fixture_extension_for_command(command: &str) -> &'static str {
    if command.starts_with("pptx ") {
        "pptx"
    } else if command.starts_with("docx ") {
        "docx"
    } else {
        "xlsx"
    }
}

include!("serve/pptx.rs");
