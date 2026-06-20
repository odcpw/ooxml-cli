// Serve/session contract tests live here while shared JSON-RPC and scrub helpers remain in the parent integration test crate.
use super::*;

#[test]
fn frozen_serve_flow_matches_go_baseline() {
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

#[test]
fn serve_commit_honors_no_validate_open_option() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-no-validate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("corrupted.docx");
    let blocked_output = temp_dir.join("blocked.docx");
    let skipped_output = temp_dir.join("skipped.docx");
    fs::copy(
        "testdata/docx/corrupted-missing-document/document.docx",
        &input,
    )
    .expect("stage corrupted docx");
    let input_str = input.to_str().expect("input path").to_string();
    let blocked_output_str = blocked_output.to_str().expect("blocked output").to_string();
    let skipped_output_str = skipped_output.to_str().expect("skipped output").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let blocked_open = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            1,
            "open",
            serde_json::json!({"file": input_str, "out": blocked_output_str}),
        ),
    );
    let blocked_session = blocked_open["result"]["sessionId"]
        .as_str()
        .expect("blocked session id")
        .to_string();
    let blocked_commit = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(2, "commit", serde_json::json!({"session": blocked_session})),
    );
    assert_eq!(blocked_commit["error"]["code"], Value::from(5));
    assert_eq!(
        blocked_commit["error"]["data"]["type"],
        Value::String("validation_failed".to_string())
    );
    assert!(
        !blocked_output.exists(),
        "default commit should not write invalid package"
    );

    let skipped_open = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "open",
            serde_json::json!({
                "file": input.to_str().expect("input path"),
                "out": skipped_output_str,
                "noValidate": true,
            }),
        ),
    );
    let skipped_session = skipped_open["result"]["sessionId"]
        .as_str()
        .expect("skipped session id")
        .to_string();
    let skipped_commit = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(4, "commit", serde_json::json!({"session": skipped_session})),
    );
    assert!(
        skipped_commit.get("error").is_none(),
        "noValidate commit failed: {skipped_commit:?}"
    );
    assert!(skipped_output.exists(), "noValidate output missing");

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_validate_reports_corrupted_package_diagnostics() {
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
                "file": "testdata/docx/corrupted-missing-document/document.docx",
            }),
        ),
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let validate_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(2, "validate", serde_json::json!({"session": session})),
    );
    assert!(
        validate_response.get("error").is_none(),
        "serve validate failed: {validate_response:?}"
    );
    let diagnostics = validate_response["result"]["diagnostics"]
        .as_array()
        .expect("diagnostics array");
    let codes = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic["code"].as_str())
        .collect::<BTreeSet<_>>();
    assert!(codes.contains("REL_DANGLING_TARGET"), "{diagnostics:?}");
    assert!(codes.contains("DOCX_MISSING_DOCUMENT"), "{diagnostics:?}");

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_inspect_supports_xlsx_cells_extract() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-cells-{}", std::process::id()));
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
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let inspect = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx cells extract",
            "args": {"sheet": "1", "range": "B1:D2", "includeEmpty": true, "maxRows": 2},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    let working = inspect_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "cells",
        "extract",
        working,
        "--sheet",
        "1",
        "--range",
        "B1:D2",
        "--include-empty",
        "--max-rows",
        "2",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(
        inspect_response["result"],
        expected.expect("extract stdout")
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_inspect_supports_xlsx_sheets_show() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-sheets-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    std::fs::copy("testdata/xlsx/used-range/workbook.xlsx", &input).expect("stage xlsx");
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
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let inspect = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx sheets show",
            "args": {"sheet": "sheetId:1"},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    let working = inspect_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "sheets",
        "show",
        working,
        "--sheet",
        "sheetId:1",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(inspect_response["result"], expected.expect("show stdout"));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_inspect_supports_xlsx_filters_sorts_show() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-filters-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("filters.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &input,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>10</v></c></row></sheetData>
  <autoFilter ref="A1:B2"><filterColumn colId="0"><filters><filter val="North"/></filters></filterColumn></autoFilter>
</worksheet>"#,
    );
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
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let inspect = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx filters-sorts show",
            "args": {"sheet": "1"},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    let working = inspect_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        working,
        "--sheet",
        "1",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(inspect_response["result"], expected.expect("show stdout"));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_inspect_supports_xlsx_names() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-names-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("defined-names.xlsx");
    write_defined_names_xlsx(&input);
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
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let list = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx names list",
            "args": {"scopeSheet": "Data"},
        }),
    );
    let list_response = serve_roundtrip(&mut stdin, &mut reader, &list);
    let working = list_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "names",
        "list",
        working,
        "--scope-sheet",
        "Data",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(
        list_response["result"],
        expected.expect("names list stdout")
    );

    let show = rpc_request(
        3,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx names show",
            "args": {"name": "LocalData", "scopeSheet": "Data"},
        }),
    );
    let show_response = serve_roundtrip(&mut stdin, &mut reader, &show);
    let (show_code, show_expected, show_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "names",
        "show",
        working,
        "--name",
        "LocalData",
        "--scope-sheet",
        "Data",
    ]);
    assert_eq!(show_code, 0);
    assert_eq!(show_stderr, None);
    assert_eq!(
        show_response["result"],
        show_expected.expect("names show stdout")
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_inspect_supports_xlsx_tables_show() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-tables-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("table-workbook.xlsx");
    write_table_xlsx(&input);
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
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let list = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables list",
            "args": {"sheet": "Data"},
        }),
    );
    let list_response = serve_roundtrip(&mut stdin, &mut reader, &list);
    let working = list_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json", "xlsx", "tables", "list", working, "--sheet", "Data",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(list_response["result"], expected.expect("list stdout"));

    let inspect = rpc_request(
        3,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables show",
            "args": {"sheet": "Data", "table": "Sales"},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    let working = inspect_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json", "xlsx", "tables", "show", working, "--sheet", "Data", "--table", "Sales",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(inspect_response["result"], expected.expect("show stdout"));

    let export = rpc_request(
        4,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables export",
            "args": {"sheet": "Data", "table": "Sales", "includeTypes": true, "includeFormulas": true},
        }),
    );
    let export_response = serve_roundtrip(&mut stdin, &mut reader, &export);
    let working = export_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "tables",
        "export",
        working,
        "--sheet",
        "Data",
        "--table",
        "Sales",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(export_response["result"], expected.expect("export stdout"));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_inspect_supports_docx_read_commands() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-docx-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let docx_input = temp_dir.join("headers.docx");
    let image_input = temp_dir.join("image.docx");
    std::fs::copy("testdata/docx/headers/document.docx", &docx_input).expect("stage docx");
    std::fs::copy("testdata/docx/with-image/document.docx", &image_input)
        .expect("stage image docx");
    let docx_input_str = docx_input.to_str().expect("docx path").to_string();
    let image_input_str = image_input.to_str().expect("image docx path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open_docx = rpc_request(1, "open", serde_json::json!({"file": docx_input_str}));
    let open_docx_response = serve_roundtrip(&mut stdin, &mut reader, &open_docx);
    let docx_session = open_docx_response["result"]["sessionId"]
        .as_str()
        .expect("docx session id")
        .to_string();

    assert_serve_inspect_matches_cli(
        &mut stdin,
        &mut reader,
        2,
        &docx_session,
        "docx text",
        serde_json::json!({}),
        (&["docx", "text"], &[]),
    );
    assert_serve_inspect_matches_cli(
        &mut stdin,
        &mut reader,
        3,
        &docx_session,
        "docx headers list",
        serde_json::json!({}),
        (&["docx", "headers", "list"], &[]),
    );
    assert_serve_inspect_matches_cli(
        &mut stdin,
        &mut reader,
        4,
        &docx_session,
        "docx headers show",
        serde_json::json!({"selector": "header:1:default"}),
        (
            &["docx", "headers", "show"],
            &["--selector", "header:1:default"],
        ),
    );
    assert_serve_inspect_matches_cli(
        &mut stdin,
        &mut reader,
        5,
        &docx_session,
        "docx footers list",
        serde_json::json!({}),
        (&["docx", "footers", "list"], &[]),
    );
    assert_serve_inspect_matches_cli(
        &mut stdin,
        &mut reader,
        6,
        &docx_session,
        "docx footers show",
        serde_json::json!({"id": "rId11"}),
        (&["docx", "footers", "show"], &["--id", "rId11"]),
    );

    let open_image_docx = rpc_request(7, "open", serde_json::json!({"file": image_input_str}));
    let open_image_docx_response = serve_roundtrip(&mut stdin, &mut reader, &open_image_docx);
    let image_session = open_image_docx_response["result"]["sessionId"]
        .as_str()
        .expect("image session id")
        .to_string();
    assert_serve_inspect_matches_cli(
        &mut stdin,
        &mut reader,
        8,
        &image_session,
        "docx images list",
        serde_json::json!({}),
        (&["docx", "images", "list"], &[]),
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

fn assert_serve_inspect_matches_cli(
    stdin: &mut impl Write,
    reader: &mut impl BufRead,
    request_id: i64,
    session: &str,
    command: &str,
    args: Value,
    cli_args_around_file: (&[&str], &[&str]),
) {
    let (cli_before_file, cli_after_file) = cli_args_around_file;
    let request = rpc_request(
        request_id,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": command,
            "args": args,
        }),
    );
    let response = serve_roundtrip(stdin, reader, &request);
    assert!(
        response.get("error").is_none(),
        "serve inspect failed for {command}: {response:?}"
    );
    let working = response["result"]["file"]
        .as_str()
        .expect("serve inspect working file");
    let mut cli_args = vec!["--json"];
    cli_args.extend_from_slice(cli_before_file);
    cli_args.push(working);
    cli_args.extend_from_slice(cli_after_file);
    let (code, expected, stderr) = run_ooxml(&cli_args);
    assert_eq!(code, 0, "direct CLI comparison exit for {command}");
    assert_eq!(stderr, None, "direct CLI comparison stderr for {command}");
    assert_eq!(
        response["result"],
        expected.expect("direct CLI comparison stdout"),
        "serve inspect result for {command}"
    );
}

#[test]
fn serve_op_supports_xlsx_ranges_set() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-ranges-set-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-ranges-set-out.xlsx");
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

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx ranges set",
            "args": {
                "sheet": "Sheet1",
                "range": "A1:C2",
                "values": [
                    ["Agent", {"value": "12.5", "type": "number"}, {"formula": "SUM(B1:B1)"}],
                    ["Flag", true, "tail"]
                ]
            },
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    assert!(
        op_response.get("error").is_none(),
        "ranges set op failed: {op_response:?}"
    );
    let readback = &op_response["result"]["readback"];
    assert_eq!(readback["range"], Value::String("A1:C2".to_string()));
    assert_eq!(readback["updated"], Value::from(6));
    assert_eq!(readback["created"], Value::from(3));
    assert_eq!(readback["formulaCount"], Value::from(1));
    assert_eq!(
        readback["destination"]["values"][0][0],
        Value::String("Agent".to_string())
    );
    assert_eq!(
        readback["destination"]["values"][0][1],
        serde_json::json!(12.5)
    );
    assert_eq!(readback["destination"]["values"][1][1], Value::Bool(true));
    assert_eq!(
        readback["destination"]["formulas"][0][2],
        Value::String("SUM(B1:B1)".to_string())
    );

    let plan = rpc_request(3, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("set".to_string())
    );

    let commit = rpc_request(4, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_xlsx_full_calc_flags(&output);
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["file"],
        Value::String(output_str.clone())
    );
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["output"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "ranges set serve output validate exit");
    assert_eq!(
        validate_stderr, None,
        "ranges set serve output validate stderr"
    );

    let (export_code, export_stdout, export_stderr) = run_go_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        &output_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C2",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(export_code, 0, "Go export readback exit");
    assert_eq!(export_stderr, None, "Go export readback stderr");
    let export = export_stdout.expect("Go export readback");
    assert_eq!(export["values"][0][0], Value::String("Agent".to_string()));
    assert_eq!(export["values"][0][1], serde_json::json!(12.5));
    assert_eq!(export["values"][0][2], Value::Null);
    assert_eq!(
        export["formulas"][0][2],
        Value::String("SUM(B1:B1)".to_string())
    );
    assert_eq!(export["values"][1][0], Value::String("Flag".to_string()));
    assert_eq!(export["values"][1][1], Value::Bool(true));
    assert_eq!(export["values"][1][2], Value::String("tail".to_string()));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_xlsx_ranges_set_format() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-format-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-format-out.xlsx");
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

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx ranges set-format",
            "args": {"sheet": "Sheet1", "range": "A1", "preset": "currency"},
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    assert!(
        op_response.get("error").is_none(),
        "set-format op failed: {op_response:?}"
    );
    let readback = &op_response["result"]["readback"];
    assert_eq!(readback["range"], Value::String("A1".to_string()));
    assert_eq!(readback["preset"], Value::String("currency".to_string()));
    assert_eq!(
        readback["formatCode"],
        Value::String("\"$\"#,##0.00".to_string())
    );
    assert_eq!(readback["updated"], Value::from(1));
    assert_eq!(
        readback["destination"]["numberFormatCodes"][0][0],
        Value::String("\"$\"#,##0.00".to_string())
    );

    let plan = rpc_request(3, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("set-format".to_string())
    );

    let commit = rpc_request(4, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["file"],
        Value::String(output_str.clone())
    );
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["output"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "set-format serve output validate exit");
    assert_eq!(
        validate_stderr, None,
        "set-format serve output validate stderr"
    );

    let (export_code, export_stdout, export_stderr) = run_go_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        &output_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ]);
    assert_eq!(export_code, 0, "Go export readback exit");
    assert_eq!(export_stderr, None, "Go export readback stderr");
    assert_eq!(
        export_stdout.expect("Go export readback")["numberFormatCodes"][0][0],
        Value::String("\"$\"#,##0.00".to_string())
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_xlsx_tables_append_rows() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-table-append-rows-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-table-append-rows-out.xlsx");
    write_table_xlsx(&input);
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

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables append-rows",
            "args": {
                "table": "Sales",
                "values": [
                    ["North", 30],
                    ["South", 40]
                ]
            },
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    assert!(
        op_response.get("error").is_none(),
        "append-rows op failed: {op_response:?}"
    );
    let readback = &op_response["result"]["readback"];
    assert_eq!(readback["rowsAppended"], Value::from(2));
    assert_eq!(
        readback["previousRange"],
        Value::String("A1:B3".to_string())
    );
    assert_eq!(readback["range"], Value::String("A1:B5".to_string()));
    assert_eq!(
        readback["destination"]["appended"]["values"][0][0],
        Value::String("North".to_string())
    );
    assert_eq!(
        readback["destination"]["appended"]["values"][1][1],
        serde_json::json!(40)
    );

    let plan = rpc_request(3, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    let argv = plan_response["result"]["plan"][0]["argv"]
        .as_array()
        .expect("planned argv");
    assert_eq!(argv[2], Value::String("append-rows".to_string()));
    assert!(
        argv.iter()
            .any(|arg| arg == &Value::String("--values".to_string()))
    );

    let commit = rpc_request(4, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["file"],
        Value::String(output_str.clone())
    );
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["output"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "append-rows serve output validate exit");
    assert_eq!(
        validate_stderr, None,
        "append-rows serve output validate stderr"
    );

    let (export_code, export_stdout, export_stderr) = run_go_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        &output_str,
        "--sheet",
        "Data",
        "--range",
        "A4:B5",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(export_code, 0, "Go export readback exit");
    assert_eq!(export_stderr, None, "Go export readback stderr");
    let export = export_stdout.expect("Go export readback");
    assert_eq!(export["values"][0][0], Value::String("North".to_string()));
    assert_eq!(export["values"][0][1], serde_json::json!(30));
    assert_eq!(export["values"][1][0], Value::String("South".to_string()));
    assert_eq!(export["values"][1][1], serde_json::json!(40));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_xlsx_tables_append_records() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-table-append-records-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-table-append-records-out.xlsx");
    write_table_xlsx(&input);
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

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables append-records",
            "args": {
                "table": "Sales",
                "expectRange": "A1:B3",
                "records": [
                    {"Region": "North", "Amount": 30},
                    {"Region": "South", "Amount": {"value": "40", "type": "number"}}
                ]
            },
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    assert!(
        op_response.get("error").is_none(),
        "append-records op failed: {op_response:?}"
    );
    let readback = &op_response["result"]["readback"];
    assert_eq!(readback["rowsAppended"], Value::from(2));
    assert_eq!(
        readback["previousRange"],
        Value::String("A1:B3".to_string())
    );
    assert_eq!(readback["range"], Value::String("A1:B5".to_string()));
    assert_eq!(readback["columns"], serde_json::json!(["Region", "Amount"]));
    assert_eq!(
        readback["destination"]["appended"]["values"][0][0],
        Value::String("North".to_string())
    );
    assert_eq!(
        readback["destination"]["appended"]["values"][1][1],
        serde_json::json!(40)
    );

    let plan = rpc_request(3, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("append-records".to_string())
    );

    let commit = rpc_request(4, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["file"],
        Value::String(output_str.clone())
    );
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["output"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(
        validate_code, 0,
        "append-records serve output validate exit"
    );
    assert_eq!(
        validate_stderr, None,
        "append-records serve output validate stderr"
    );

    let (export_code, export_stdout, export_stderr) = run_go_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        &output_str,
        "--sheet",
        "Data",
        "--range",
        "A4:B5",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(export_code, 0, "Go export readback exit");
    assert_eq!(export_stderr, None, "Go export readback stderr");
    let export = export_stdout.expect("Go export readback");
    assert_eq!(export["values"][0][0], Value::String("North".to_string()));
    assert_eq!(export["values"][0][1], serde_json::json!(30));
    assert_eq!(export["values"][1][0], Value::String("South".to_string()));
    assert_eq!(export["values"][1][1], serde_json::json!(40));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_xlsx_workbook_metadata_update() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-workbook-metadata-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-metadata-out.xlsx");
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

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx workbook metadata update",
            "args": {
                "title": "Serve Title",
                "company": "Acme Corp",
                "fullCalcOnLoad": true
            },
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    assert!(
        op_response.get("error").is_none(),
        "metadata op failed: {op_response:?}"
    );
    let readback = &op_response["result"]["readback"];
    assert_eq!(
        readback["metadata"]["title"],
        Value::String("Serve Title".to_string())
    );
    assert_eq!(
        readback["metadata"]["company"],
        Value::String("Acme Corp".to_string())
    );
    assert_eq!(
        readback["calcSettings"]["fullCalcOnLoad"],
        Value::Bool(true)
    );

    let inspect = rpc_request(
        3,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx workbook metadata inspect",
            "args": {},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    assert_eq!(
        inspect_response["result"]["metadata"]["title"],
        Value::String("Serve Title".to_string())
    );

    let plan = rpc_request(4, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][1],
        Value::String("workbook".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][3],
        Value::String("update".to_string())
    );

    let commit = rpc_request(5, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "metadata commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    let (inspect_code, inspect_stdout, inspect_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "workbook",
        "metadata",
        "inspect",
        &output_str,
    ]);
    assert_eq!(inspect_code, 0, "metadata serve inspect output exit");
    assert_eq!(inspect_stderr, None, "metadata serve inspect output stderr");
    assert_eq!(
        inspect_stdout.expect("metadata serve output inspect")["metadata"]["title"],
        Value::String("Serve Title".to_string())
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_rejects_unknown_xlsx_and_docx_commands_in_family_dispatchers() {
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

    for (offset, command) in ["xlsx not-real", "docx not-real"].iter().enumerate() {
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
fn serve_op_supports_docx_headers_set_text() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-docx-header-set-text-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.docx");
    let output = temp_dir.join("serve-docx-out.docx");
    std::fs::copy("testdata/docx/headers/document.docx", &input).expect("stage docx");
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

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "docx headers set-text",
            "args": {
                "selector": "header:1:default/p:1",
                "text": "Serve Header"
            },
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    assert!(
        op_response.get("error").is_none(),
        "docx header op failed: {op_response:?}"
    );
    assert_eq!(
        op_response["result"]["readback"]["text"],
        Value::String("Serve Header".to_string())
    );
    assert_eq!(
        op_response["result"]["readback"]["previousText"],
        Value::String("Page Header".to_string())
    );

    let plan = rpc_request(3, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][1],
        Value::String("headers".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("set-text".to_string())
    );

    let commit = rpc_request(4, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "docx header commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "headers",
        "show",
        &output_str,
        "--selector",
        "header:1:default",
    ]);
    assert_eq!(show_code, 0, "docx serve show output exit");
    assert_eq!(show_stderr, None, "docx serve show output stderr");
    assert_eq!(
        show_stdout.expect("docx serve output show")["paragraphs"][0]["text"],
        Value::String("Serve Header".to_string())
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_docx_fields_editing() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-docx-fields-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.docx");
    let output = temp_dir.join("serve-docx-fields-out.docx");
    fs::copy("testdata/docx/with-fields/document.docx", &input).expect("stage docx");
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
            serde_json::json!({"file": input_str, "out": output_str}),
        ),
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let inspect_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "docx fields list",
                "args": {"type": "PAGE"},
            }),
        ),
    );
    assert!(
        inspect_response.get("error").is_none(),
        "docx fields inspect failed: {inspect_response:?}"
    );
    assert_eq!(
        inspect_response["result"]["fields"][0]["location"],
        Value::String("body:1".to_string())
    );

    let set_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx fields set-result",
                "args": {"selector": "body:1:0", "result": "77"},
            }),
        ),
    );
    assert!(
        set_response.get("error").is_none(),
        "docx field set-result op failed: {set_response:?}"
    );
    assert_eq!(
        set_response["result"]["readback"]["cachedResult"],
        Value::String("77".to_string())
    );

    let insert_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            4,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx fields insert",
                "args": {"location": "body:1", "fieldCode": "NUMPAGES", "result": "2"},
            }),
        ),
    );
    assert!(
        insert_response.get("error").is_none(),
        "docx field insert op failed: {insert_response:?}"
    );
    assert_eq!(
        insert_response["result"]["readback"]["instruction"],
        Value::String("NUMPAGES".to_string())
    );

    let plan_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(5, "plan", serde_json::json!({"session": session})),
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][1],
        Value::String("fields".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("set-result".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][1]["argv"][2],
        Value::String("insert".to_string())
    );

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(6, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "docx fields commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");

    let (list_code, list_stdout, list_stderr) =
        run_ooxml(&["--json", "docx", "fields", "list", &output_str]);
    assert_eq!(list_code, 0, "docx fields output list exit");
    assert_eq!(list_stderr, None, "docx fields output list stderr");
    let list = list_stdout.expect("docx fields output list");
    assert!(
        list["fields"]
            .as_array()
            .expect("fields")
            .iter()
            .any(|field| {
                field["instruction"] == Value::String("PAGE".to_string())
                    && field["cachedResult"] == Value::String("77".to_string())
            })
    );
    assert!(
        list["fields"]
            .as_array()
            .expect("fields")
            .iter()
            .any(|field| {
                field["instruction"] == Value::String("NUMPAGES".to_string())
                    && field["cachedResult"] == Value::String("2".to_string())
            })
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_docx_blocks_editing() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-docx-blocks-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.docx");
    let output = temp_dir.join("serve-docx-blocks-out.docx");
    fs::copy("testdata/docx/mixed-blocks/document.docx", &input).expect("stage docx");
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
            serde_json::json!({"file": input_str, "out": output_str}),
        ),
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let first_block_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "docx blocks",
                "args": {"block": 1},
            }),
        ),
    );
    assert!(
        first_block_response.get("error").is_none(),
        "docx blocks inspect failed: {first_block_response:?}"
    );
    let first_hash = first_block_response["result"]["blocks"][0]["contentHash"]
        .as_str()
        .expect("first block hash")
        .to_string();

    let insert_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx blocks insert-after",
                "args": {
                    "block": 1,
                    "expectHash": first_hash,
                    "text": "Serve inserted block",
                    "style": "Heading1"
                },
            }),
        ),
    );
    assert!(
        insert_response.get("error").is_none(),
        "docx blocks insert-after op failed: {insert_response:?}"
    );
    assert_eq!(
        insert_response["result"]["readback"]["text"],
        Value::String("Serve inserted block".to_string())
    );

    let inserted_block_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            4,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "docx blocks",
                "args": {"block": 2, "includeRuns": true},
            }),
        ),
    );
    assert!(
        inserted_block_response.get("error").is_none(),
        "docx blocks inserted inspect failed: {inserted_block_response:?}"
    );
    let inserted_hash = inserted_block_response["result"]["blocks"][0]["contentHash"]
        .as_str()
        .expect("inserted block hash")
        .to_string();

    let replace_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            5,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx blocks replace",
                "args": {
                    "block": 2,
                    "expectHash": inserted_hash,
                    "text": "Serve replaced block",
                    "style": "Heading1"
                },
            }),
        ),
    );
    assert!(
        replace_response.get("error").is_none(),
        "docx blocks replace op failed: {replace_response:?}"
    );
    assert_eq!(
        replace_response["result"]["readback"]["destination"]["text"],
        Value::String("Serve replaced block".to_string())
    );

    let delete_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            6,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx blocks delete",
                "args": {"block": 1, "expectHash": first_hash},
            }),
        ),
    );
    assert!(
        delete_response.get("error").is_none(),
        "docx blocks delete op failed: {delete_response:?}"
    );
    assert_eq!(
        delete_response["result"]["readback"]["previousKind"],
        Value::String("table".to_string())
    );

    let plan_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(7, "plan", serde_json::json!({"session": session})),
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][1],
        Value::String("blocks".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("insert-after".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][1]["argv"][2],
        Value::String("replace".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][2]["argv"][2],
        Value::String("delete".to_string())
    );

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(8, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "docx blocks commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "docx blocks serve validate exit");
    assert_eq!(validate_stderr, None, "docx blocks serve validate stderr");

    let (blocks_code, blocks_stdout, blocks_stderr) =
        run_ooxml(&["--json", "docx", "blocks", &output_str]);
    assert_eq!(blocks_code, 0, "docx blocks output readback exit");
    assert_eq!(blocks_stderr, None, "docx blocks output readback stderr");
    let blocks = blocks_stdout.expect("docx blocks output readback");
    let output_blocks = blocks["blocks"].as_array().expect("blocks");
    assert_eq!(output_blocks.len(), 4);
    assert_eq!(
        output_blocks[0]["text"],
        Value::String("Serve replaced block".to_string())
    );
    assert_eq!(
        output_blocks[0]["paragraph"]["style"],
        Value::String("Heading1".to_string())
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_docx_paragraph_editing() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-docx-paragraphs-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.docx");
    let output = temp_dir.join("serve-docx-paragraphs-out.docx");
    fs::copy("testdata/docx/styled-headings/document.docx", &input).expect("stage docx");
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
            serde_json::json!({"file": input_str, "out": output_str}),
        ),
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let append_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx paragraphs append",
                "args": {"text": "Serve appended paragraph", "style": "Heading1"},
            }),
        ),
    );
    assert!(
        append_response.get("error").is_none(),
        "docx paragraphs append op failed: {append_response:?}"
    );
    assert_eq!(
        append_response["result"]["readback"]["text"],
        Value::String("Serve appended paragraph".to_string())
    );

    let insert_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx paragraphs insert",
                "args": {"insertAfter": 0, "text": "Serve prepended paragraph"},
            }),
        ),
    );
    assert!(
        insert_response.get("error").is_none(),
        "docx paragraphs insert op failed: {insert_response:?}"
    );
    assert_eq!(
        insert_response["result"]["readback"]["index"],
        Value::from(1)
    );
    assert_eq!(
        insert_response["result"]["readback"]["text"],
        Value::String("Serve prepended paragraph".to_string())
    );

    let set_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            4,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx paragraphs set",
                "args": {"index": 2, "text": "Serve updated heading"},
            }),
        ),
    );
    assert!(
        set_response.get("error").is_none(),
        "docx paragraphs set op failed: {set_response:?}"
    );
    assert_eq!(
        set_response["result"]["readback"]["previousText"],
        Value::String("Heading Text".to_string())
    );
    assert_eq!(
        set_response["result"]["readback"]["text"],
        Value::String("Serve updated heading".to_string())
    );
    let handle = set_response["result"]["readback"]["handle"]
        .as_str()
        .expect("paragraph handle")
        .to_string();

    let clear_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            5,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx paragraphs clear",
                "args": {"handle": handle},
            }),
        ),
    );
    assert!(
        clear_response.get("error").is_none(),
        "docx paragraphs clear op failed: {clear_response:?}"
    );
    assert_eq!(
        clear_response["result"]["readback"]["previousText"],
        Value::String("Serve updated heading".to_string())
    );

    let inspect_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            6,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "docx blocks",
                "args": {},
            }),
        ),
    );
    assert!(
        inspect_response.get("error").is_none(),
        "docx paragraphs blocks inspect failed: {inspect_response:?}"
    );
    let session_blocks = inspect_response["result"]["blocks"]
        .as_array()
        .expect("session docx blocks");
    assert_eq!(
        session_blocks[0]["text"],
        Value::String("Serve prepended paragraph".to_string())
    );
    assert_eq!(session_blocks[1]["text"], Value::String(String::new()));
    assert_eq!(
        session_blocks[3]["text"],
        Value::String("Serve appended paragraph".to_string())
    );

    let plan_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(7, "plan", serde_json::json!({"session": session})),
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][1],
        Value::String("paragraphs".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("append".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][1]["argv"][2],
        Value::String("insert".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][2]["argv"][2],
        Value::String("set".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][3]["argv"][2],
        Value::String("clear".to_string())
    );

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(8, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "docx paragraphs commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "docx paragraphs serve validate exit");
    assert_eq!(
        validate_stderr, None,
        "docx paragraphs serve validate stderr"
    );

    let (text_code, text_stdout, text_stderr) = run_ooxml(&["--json", "docx", "text", &output_str]);
    assert_eq!(text_code, 0, "docx paragraphs output readback exit");
    assert_eq!(text_stderr, None, "docx paragraphs output readback stderr");
    let output_text = text_stdout.expect("docx paragraphs output text");
    let output_blocks = output_text["blocks"].as_array().expect("output blocks");
    assert_eq!(
        output_blocks[0]["text"],
        Value::String("Serve prepended paragraph".to_string())
    );
    assert_eq!(output_blocks[1]["text"], Value::String(String::new()));
    assert_eq!(
        output_blocks[3]["text"],
        Value::String("Serve appended paragraph".to_string())
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_docx_styles_apply() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-docx-styles-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.docx");
    let output = temp_dir.join("serve-docx-styles-out.docx");
    fs::copy("testdata/docx/apply-styles/document.docx", &input).expect("stage docx");
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
            serde_json::json!({"file": input_str, "out": output_str}),
        ),
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let styles_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "docx styles list",
                "args": {"type": "paragraph"},
            }),
        ),
    );
    assert!(
        styles_response.get("error").is_none(),
        "docx styles list inspect failed: {styles_response:?}"
    );
    let heading2_handle = styles_response["result"]["styles"]
        .as_array()
        .expect("styles")
        .iter()
        .find(|style| style["styleId"] == Value::String("Heading2".to_string()))
        .and_then(|style| style["handle"].as_str())
        .expect("Heading2 handle")
        .to_string();

    let show_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "docx styles show",
                "args": {"style": "Heading2"},
            }),
        ),
    );
    assert!(
        show_response.get("error").is_none(),
        "docx styles show inspect failed: {show_response:?}"
    );
    assert_eq!(show_response["result"]["found"], Value::Bool(true));
    assert_eq!(
        show_response["result"]["style"]["handle"],
        Value::String(heading2_handle.clone())
    );

    let first_block_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            4,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "docx blocks",
                "args": {"block": 1},
            }),
        ),
    );
    assert!(
        first_block_response.get("error").is_none(),
        "docx blocks inspect failed: {first_block_response:?}"
    );
    let first_hash = first_block_response["result"]["blocks"][0]["contentHash"]
        .as_str()
        .expect("first block hash")
        .to_string();

    let paragraph_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            5,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx styles apply",
                "args": {
                    "index": 1,
                    "target": "paragraph",
                    "style": heading2_handle,
                    "expectHash": first_hash
                },
            }),
        ),
    );
    assert!(
        paragraph_response.get("error").is_none(),
        "docx paragraph style apply op failed: {paragraph_response:?}"
    );
    assert_eq!(
        paragraph_response["result"]["readback"]["style"],
        Value::String("Heading2".to_string())
    );
    assert_eq!(
        paragraph_response["result"]["readback"]["target"],
        Value::String("paragraph".to_string())
    );

    let run_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            6,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx styles apply",
                "args": {"index": 2, "target": "run", "style": "Emphasis"},
            }),
        ),
    );
    assert!(
        run_response.get("error").is_none(),
        "docx run style apply op failed: {run_response:?}"
    );
    assert_eq!(
        run_response["result"]["readback"]["target"],
        Value::String("run".to_string())
    );
    assert_eq!(
        run_response["result"]["readback"]["style"],
        Value::String("Emphasis".to_string())
    );

    let table_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            7,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx styles apply",
                "args": {"index": 1, "target": "table", "style": "TableGrid"},
            }),
        ),
    );
    assert!(
        table_response.get("error").is_none(),
        "docx table style apply op failed: {table_response:?}"
    );
    assert_eq!(
        table_response["result"]["readback"]["target"],
        Value::String("table".to_string())
    );
    assert_eq!(
        table_response["result"]["readback"]["style"],
        Value::String("TableGrid".to_string())
    );
    assert_eq!(
        table_response["result"]["readback"]["blockKind"],
        Value::String("table".to_string())
    );

    let plan_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(8, "plan", serde_json::json!({"session": session})),
    );
    for index in 0..3 {
        assert_eq!(
            plan_response["result"]["plan"][index]["argv"][1],
            Value::String("styles".to_string())
        );
        assert_eq!(
            plan_response["result"]["plan"][index]["argv"][2],
            Value::String("apply".to_string())
        );
        assert!(
            !plan_response["result"]["plan"][index]["argv"]
                .as_array()
                .expect("plan argv")
                .contains(&Value::String("--no-validate".to_string())),
            "style apply plan should not silently skip style validation"
        );
    }

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(9, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "docx styles commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "docx styles serve validate exit");
    assert_eq!(validate_stderr, None, "docx styles serve validate stderr");

    let (blocks_code, blocks_stdout, blocks_stderr) =
        run_ooxml(&["--json", "docx", "blocks", &output_str, "--block", "1"]);
    assert_eq!(blocks_code, 0, "docx styles output blocks exit");
    assert_eq!(blocks_stderr, None, "docx styles output blocks stderr");
    let blocks = blocks_stdout.expect("docx styles output blocks");
    assert_eq!(
        blocks["blocks"][0]["paragraph"]["style"],
        Value::String("Heading2".to_string())
    );

    let document_xml = read_zip_string(Path::new(&output_str), "word/document.xml");
    assert!(
        document_xml.contains("w:rStyle w:val=\"Emphasis\""),
        "run style was not written to document.xml"
    );
    assert!(
        document_xml.contains("w:tblStyle w:val=\"TableGrid\""),
        "table style was not written to document.xml"
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_docx_tables_editing() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-docx-tables-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.docx");
    let output = temp_dir.join("serve-docx-tables-out.docx");
    fs::copy("testdata/docx/table/document.docx", &input).expect("stage docx");
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
            serde_json::json!({"file": input_str, "out": output_str}),
        ),
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let table_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "docx tables show",
                "args": {"table": 1},
            }),
        ),
    );
    assert!(
        table_response.get("error").is_none(),
        "docx tables show inspect failed: {table_response:?}"
    );
    assert_eq!(
        table_response["result"]["tables"][0]["cells"][0][1],
        Value::String("B1".to_string())
    );
    let table_hash = table_response["result"]["tables"][0]["contentHash"]
        .as_str()
        .expect("table hash")
        .to_string();

    let set_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx tables set-cell",
                "args": {
                    "table": 1,
                    "row": 1,
                    "col": 2,
                    "expectHash": table_hash,
                    "text": "Serve table value"
                },
            }),
        ),
    );
    assert!(
        set_response.get("error").is_none(),
        "docx tables set-cell op failed: {set_response:?}"
    );
    assert_eq!(
        set_response["result"]["readback"]["previousText"],
        Value::String("B1".to_string())
    );
    assert_eq!(
        set_response["result"]["readback"]["text"],
        Value::String("Serve table value".to_string())
    );
    let set_hash = set_response["result"]["readback"]["contentHash"]
        .as_str()
        .expect("set-cell content hash")
        .to_string();

    let changed_table_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            4,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "docx tables show",
                "args": {"table": 1},
            }),
        ),
    );
    assert!(
        changed_table_response.get("error").is_none(),
        "docx tables changed inspect failed: {changed_table_response:?}"
    );
    assert_eq!(
        changed_table_response["result"]["tables"][0]["cells"][0][1],
        Value::String("Serve table value".to_string())
    );

    let clear_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            5,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx tables clear-cell",
                "args": {
                    "table": 1,
                    "row": 1,
                    "col": 2,
                    "expectHash": set_hash
                },
            }),
        ),
    );
    assert!(
        clear_response.get("error").is_none(),
        "docx tables clear-cell op failed: {clear_response:?}"
    );
    assert_eq!(
        clear_response["result"]["readback"]["previousText"],
        Value::String("Serve table value".to_string())
    );
    let clear_hash = clear_response["result"]["readback"]["contentHash"]
        .as_str()
        .expect("clear-cell content hash")
        .to_string();

    let insert_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            6,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx tables insert-row",
                "args": {
                    "table": 1,
                    "at": 2,
                    "expectHash": clear_hash
                },
            }),
        ),
    );
    assert!(
        insert_response.get("error").is_none(),
        "docx tables insert-row op failed: {insert_response:?}"
    );
    assert_eq!(insert_response["result"]["readback"]["row"], Value::from(2));
    assert_eq!(
        insert_response["result"]["readback"]["rows"],
        Value::from(3)
    );
    let insert_hash = insert_response["result"]["readback"]["contentHash"]
        .as_str()
        .expect("insert-row content hash")
        .to_string();

    let delete_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            7,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx tables delete-row",
                "args": {
                    "table": 1,
                    "row": 2,
                    "expectHash": insert_hash
                },
            }),
        ),
    );
    assert!(
        delete_response.get("error").is_none(),
        "docx tables delete-row op failed: {delete_response:?}"
    );
    assert_eq!(delete_response["result"]["readback"]["row"], Value::from(2));
    assert_eq!(
        delete_response["result"]["readback"]["rows"],
        Value::from(2)
    );

    let plan_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(8, "plan", serde_json::json!({"session": session})),
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][1],
        Value::String("tables".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("set-cell".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][1]["argv"][2],
        Value::String("clear-cell".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][2]["argv"][2],
        Value::String("insert-row".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][3]["argv"][2],
        Value::String("delete-row".to_string())
    );

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(9, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "docx tables commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "docx tables serve validate exit");
    assert_eq!(validate_stderr, None, "docx tables serve validate stderr");

    let (tables_code, tables_stdout, tables_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &output_str,
        "--table",
        "1",
    ]);
    assert_eq!(tables_code, 0, "docx tables output readback exit");
    assert_eq!(tables_stderr, None, "docx tables output readback stderr");
    let tables = tables_stdout.expect("docx tables output readback");
    assert_eq!(tables["tables"][0]["rows"], Value::from(2));
    assert_eq!(
        tables["tables"][0]["cells"][0][1],
        Value::String(String::new())
    );
    assert_eq!(
        tables["tables"][0]["cells"][1][0],
        Value::String("A2".to_string())
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_docx_comments_editing() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-docx-comments-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.docx");
    let output = temp_dir.join("serve-docx-comments-out.docx");
    fs::copy("testdata/docx/with-comments/document.docx", &input).expect("stage docx");
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
            serde_json::json!({"file": input_str, "out": output_str}),
        ),
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let comments_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "docx comments list",
                "args": {},
            }),
        ),
    );
    assert!(
        comments_response.get("error").is_none(),
        "docx comments list inspect failed: {comments_response:?}"
    );
    let comment_id = comments_response["result"]["comments"][0]["id"]
        .as_i64()
        .expect("comment id");
    let initial_hash = comments_response["result"]["comments"][0]["contentHash"]
        .as_str()
        .expect("comment content hash")
        .to_string();

    let edit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx comments edit",
                "args": {
                    "commentId": comment_id,
                    "expectHash": initial_hash,
                    "author": "Serve Agent",
                    "date": "2026-06-19T10:00:00Z",
                    "text": "Serve edited comment"
                },
            }),
        ),
    );
    assert!(
        edit_response.get("error").is_none(),
        "docx comments edit op failed: {edit_response:?}"
    );
    assert_eq!(
        edit_response["result"]["readback"]["commentId"],
        Value::from(comment_id)
    );
    assert_eq!(
        edit_response["result"]["readback"]["text"],
        Value::String("Serve edited comment".to_string())
    );
    let edited_hash = edit_response["result"]["readback"]["contentHash"]
        .as_str()
        .expect("edited content hash")
        .to_string();

    let edited_comments_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            4,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "docx comments list",
                "args": {"commentId": comment_id},
            }),
        ),
    );
    assert!(
        edited_comments_response.get("error").is_none(),
        "docx comments edited inspect failed: {edited_comments_response:?}"
    );
    assert_eq!(
        edited_comments_response["result"]["comments"][0]["text"],
        Value::String("Serve edited comment".to_string())
    );

    let remove_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            5,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx comments remove",
                "args": {"commentId": comment_id, "expectHash": edited_hash},
            }),
        ),
    );
    assert!(
        remove_response.get("error").is_none(),
        "docx comments remove op failed: {remove_response:?}"
    );
    assert_eq!(
        remove_response["result"]["readback"]["previousText"],
        Value::String("Serve edited comment".to_string())
    );

    let add_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            6,
            "op",
            serde_json::json!({
                "session": session,
                "command": "docx comments add",
                "args": {
                    "anchorBlock": 1,
                    "author": "Serve Agent",
                    "initials": "SA",
                    "date": "2026-06-19T10:05:00Z",
                    "text": "Serve added comment"
                },
            }),
        ),
    );
    assert!(
        add_response.get("error").is_none(),
        "docx comments add op failed: {add_response:?}"
    );
    assert_eq!(
        add_response["result"]["readback"]["text"],
        Value::String("Serve added comment".to_string())
    );
    assert_eq!(add_response["result"]["readback"]["anchoredToBlock"], 1);

    let plan_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(7, "plan", serde_json::json!({"session": session})),
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][1],
        Value::String("comments".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("edit".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][1]["argv"][2],
        Value::String("remove".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][2]["argv"][2],
        Value::String("add".to_string())
    );

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(8, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "docx comments commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "docx comments serve validate exit");
    assert_eq!(validate_stderr, None, "docx comments serve validate stderr");

    let (comments_code, comments_stdout, comments_stderr) =
        run_ooxml(&["--json", "docx", "comments", "list", &output_str]);
    assert_eq!(comments_code, 0, "docx comments output list exit");
    assert_eq!(comments_stderr, None, "docx comments output list stderr");
    let comments = comments_stdout.expect("docx comments output list");
    let output_comments = comments["comments"].as_array().expect("comments");
    assert!(
        output_comments.iter().any(|comment| {
            comment["text"] == Value::String("Serve added comment".to_string())
                && comment["author"] == Value::String("Serve Agent".to_string())
                && comment["anchoredToBlock"].as_i64() == Some(1)
        }),
        "committed output should contain added serve comment: {output_comments:?}"
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_pptx_generic_web_agent_edit_path_works() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-pptx-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = "testdata/pptx/minimal-title/presentation.pptx";
    let output = temp_dir.join("serve-pptx-out.pptx");
    let output_str = output.to_str().expect("output path").to_string();
    let marker = format!("Rust serve web {}", std::process::id());

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
            serde_json::json!({"file": input, "out": output_str}),
        ),
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let list_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx slides list",
                "args": {},
            }),
        ),
    );
    assert_eq!(
        list_response["result"]["slides"][0]["number"],
        Value::from(1)
    );

    let inspect_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx slides show",
                "args": {"slide": 1, "include-text": true},
            }),
        ),
    );
    assert_eq!(
        inspect_response["result"]["slides"][0]["shapes"][0]["textContent"],
        Value::String("Minimal Title Slide".to_string())
    );

    let extract_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            30,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx extract text",
                "args": {"slide": 1},
            }),
        ),
    );
    assert_eq!(
        extract_response["result"]["slides"][0]["shapes"][0]["text"]["plainText"],
        Value::String("Minimal Title Slide".to_string())
    );

    let notes_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            32,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx notes show",
                "args": {"slide": 1},
            }),
        ),
    );
    assert_eq!(
        notes_response["result"]["notes"]["plainText"],
        Value::String(String::new())
    );

    let comments_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            38,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx comments list",
                "args": {},
            }),
        ),
    );
    assert_eq!(
        comments_response["result"]["slides"][0]["comments"],
        Value::Array(Vec::new())
    );

    let masters_list_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            36,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx masters list",
                "args": {},
            }),
        ),
    );
    assert_eq!(
        masters_list_response["result"]["masters"][0]["primarySelector"],
        Value::String("1".to_string())
    );

    let masters_show_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            37,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx masters show",
                "args": {"master": 1},
            }),
        ),
    );
    assert_eq!(masters_show_response["result"]["shapes"], Value::from(12));

    let layouts_list_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            34,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx layouts list",
                "args": {},
            }),
        ),
    );
    assert_eq!(
        layouts_list_response["result"]["layouts"][0]["primarySelector"],
        Value::String("1".to_string())
    );

    let layouts_show_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            35,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx layouts show",
                "args": {"layout": "1"},
            }),
        ),
    );
    assert_eq!(
        layouts_show_response["result"]["placeholders"][0]["key"],
        Value::String("ctrTitle".to_string())
    );

    let tables_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            33,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx tables show",
                "args": {"slide": 1},
            }),
        ),
    );
    assert_eq!(
        tables_response["result"]["tables"],
        Value::Array(Vec::new())
    );

    let shapes_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            31,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx shapes show",
                "args": {"slide": 1, "include-text": true, "include-bounds": true},
            }),
        ),
    );
    assert_eq!(
        shapes_response["result"]["shapes"][0]["primarySelector"],
        Value::String("title".to_string())
    );

    let op_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            4,
            "op",
            serde_json::json!({
                "session": session,
                "command": "pptx replace text",
                "args": {"slide": 1, "target": "title", "text": marker},
            }),
        ),
    );
    assert_eq!(op_response["result"]["readback"]["newText"], marker);
    let pptx_working = op_response["result"]["readback"]["file"]
        .as_str()
        .expect("pptx working package");
    assert_eq!(
        op_response["result"]["readback"]["readbackCommand"],
        Value::String(format!(
            "ooxml --json pptx shapes get {} --slide 1 --target title --include-text --include-bounds",
            command_arg_for_test(pptx_working)
        ))
    );

    for (id, method) in [(5, "validate"), (6, "commit")] {
        let response = serve_roundtrip(
            &mut stdin,
            &mut reader,
            &rpc_request(id, method, serde_json::json!({"session": session})),
        );
        assert!(
            response.get("error").is_none(),
            "{method} returned error: {response:?}"
        );
    }

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "slides",
        "show",
        &output_str,
        "--slide",
        "1",
        "--include-text",
    ]);
    assert_eq!(show_code, 0);
    assert_eq!(show_stderr, None);
    assert_eq!(
        show_stdout.expect("show stdout")["slides"][0]["shapes"][0]["textContent"],
        Value::String(marker)
    );
}
