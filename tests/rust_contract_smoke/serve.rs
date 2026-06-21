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
fn serve_op_supports_pptx_table_mutations() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-pptx-tables-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = "testdata/pptx/table-slide/presentation.pptx";
    let output = temp_dir.join("serve-pptx-tables-out.pptx");
    let output_str = output.to_str().expect("output path").to_string();
    let workbook = temp_dir.join("serve-pptx-table-source.xlsx");
    write_simple_xlsx_with_sheet_xml(&workbook, serve_pptx_update_source_sheet_xml_4x4());
    let workbook_str = workbook.to_str().expect("workbook path").to_string();

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
    assert!(
        open_response.get("error").is_none(),
        "pptx tables open failed: {open_response:?}"
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
                "command": "pptx tables show",
                "args": {"slide": 2, "target": "table:1"},
            }),
        ),
    );
    assert!(
        table_response.get("error").is_none(),
        "pptx tables show inspect failed: {table_response:?}"
    );
    assert_eq!(
        table_response["result"]["tables"][0]["rows"],
        Value::from(3)
    );
    assert_eq!(
        table_response["result"]["tables"][0]["cells"][1][1],
        Value::String("R1C1".to_string())
    );

    let set_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "op",
            serde_json::json!({
                "session": session,
                "command": "pptx tables set-cell",
                "args": {
                    "slide": 2,
                    "target": "table:1",
                    "row": 2,
                    "col": 2,
                    "text": "Serve PPTX Cell"
                },
            }),
        ),
    );
    assert!(
        set_response.get("error").is_none(),
        "pptx tables set-cell op failed: {set_response:?}"
    );
    assert_eq!(
        set_response["result"]["readback"]["previousText"],
        Value::String("R1C1".to_string())
    );
    assert_eq!(
        set_response["result"]["readback"]["text"],
        Value::String("Serve PPTX Cell".to_string())
    );

    let changed_table_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            4,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx tables show",
                "args": {"slide": 2, "target": "table:1"},
            }),
        ),
    );
    assert!(
        changed_table_response.get("error").is_none(),
        "pptx tables changed inspect failed: {changed_table_response:?}"
    );
    assert_eq!(
        changed_table_response["result"]["tables"][0]["cells"][1][1],
        Value::String("Serve PPTX Cell".to_string())
    );

    let insert_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            5,
            "op",
            serde_json::json!({
                "session": session,
                "command": "pptx tables insert-row",
                "args": {
                    "slide": 2,
                    "target": "table:1",
                    "at": 3
                },
            }),
        ),
    );
    assert!(
        insert_response.get("error").is_none(),
        "pptx tables insert-row op failed: {insert_response:?}"
    );
    assert_eq!(insert_response["result"]["readback"]["at"], Value::from(3));
    assert_eq!(
        insert_response["result"]["readback"]["rows"],
        Value::from(4)
    );
    assert_eq!(
        insert_response["result"]["readback"]["cellCount"],
        Value::from(3)
    );

    let delete_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            6,
            "op",
            serde_json::json!({
                "session": session,
                "command": "pptx tables delete-row",
                "args": {
                    "slide": 2,
                    "target": "table:1",
                    "row": 3
                },
            }),
        ),
    );
    assert!(
        delete_response.get("error").is_none(),
        "pptx tables delete-row op failed: {delete_response:?}"
    );
    assert_eq!(delete_response["result"]["readback"]["row"], Value::from(3));
    assert_eq!(
        delete_response["result"]["readback"]["rows"],
        Value::from(3)
    );

    let insert_col_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            7,
            "op",
            serde_json::json!({
                "session": session,
                "command": "pptx tables insert-col",
                "args": {
                    "slide": 2,
                    "target": "table:1",
                    "at": 2,
                    "widthEmu": 1234567
                },
            }),
        ),
    );
    assert!(
        insert_col_response.get("error").is_none(),
        "pptx tables insert-col op failed: {insert_col_response:?}"
    );
    assert_eq!(
        insert_col_response["result"]["readback"]["at"],
        Value::from(2)
    );
    assert_eq!(
        insert_col_response["result"]["readback"]["cols"],
        Value::from(4)
    );
    assert_eq!(
        insert_col_response["result"]["readback"]["widthEmu"],
        Value::from(1234567)
    );

    let delete_col_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            8,
            "op",
            serde_json::json!({
                "session": session,
                "command": "pptx tables delete-col",
                "args": {
                    "slide": 2,
                    "target": "table:1",
                    "col": 2
                },
            }),
        ),
    );
    assert!(
        delete_col_response.get("error").is_none(),
        "pptx tables delete-col op failed: {delete_col_response:?}"
    );
    assert_eq!(
        delete_col_response["result"]["readback"]["col"],
        Value::from(2)
    );
    assert_eq!(
        delete_col_response["result"]["readback"]["cols"],
        Value::from(3)
    );

    let update_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            9,
            "op",
            serde_json::json!({
                "session": session,
                "command": "pptx tables update-from-xlsx",
                "args": {
                    "slide": 2,
                    "target": "table:1",
                    "workbook": workbook_str,
                    "sheet": "Sheet1",
                    "range": "A1:C3",
                    "formulaMode": "formula",
                    "expectSourceRange": "A1:C3"
                },
            }),
        ),
    );
    assert!(
        update_response.get("error").is_none(),
        "pptx tables update-from-xlsx op failed: {update_response:?}"
    );
    assert_eq!(
        update_response["result"]["readback"]["update"]["updatedCells"],
        Value::from(9)
    );
    assert_eq!(
        update_response["result"]["readback"]["destination"]["cells"][0][0],
        Value::String("=SUM(B1:C1)".to_string())
    );

    let plan_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(10, "plan", serde_json::json!({"session": session})),
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][0],
        Value::String("pptx".to_string())
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
        Value::String("insert-row".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][2]["argv"][2],
        Value::String("delete-row".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][3]["argv"][2],
        Value::String("insert-col".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][4]["argv"][2],
        Value::String("delete-col".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][5]["argv"][2],
        Value::String("update-from-xlsx".to_string())
    );

    let validate_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(11, "validate", serde_json::json!({"session": session})),
    );
    assert!(
        validate_response.get("error").is_none(),
        "pptx tables validate failed: {validate_response:?}"
    );

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(12, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "pptx tables commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "pptx tables serve validate exit");
    assert_eq!(validate_stderr, None, "pptx tables serve validate stderr");

    let (tables_code, tables_stdout, tables_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        &output_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(tables_code, 0, "pptx tables output readback exit");
    assert_eq!(tables_stderr, None, "pptx tables output readback stderr");
    let tables = tables_stdout.expect("pptx tables output readback");
    assert_eq!(tables["tables"][0]["rows"], Value::from(3));
    assert_eq!(tables["tables"][0]["cols"], Value::from(3));
    assert_eq!(
        tables["tables"][0]["cells"][0][0],
        Value::String("=SUM(B1:C1)".to_string())
    );
    assert_eq!(
        tables["tables"][0]["cells"][1][1],
        Value::String("42".to_string())
    );
    assert_eq!(
        tables["tables"][0]["cells"][2][2],
        Value::String("done".to_string())
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

fn serve_pptx_update_source_sheet_xml_4x4() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:D4"/>
  <sheetData>
    <row r="1"><c r="A1"><f>SUM(B1:C1)</f><v>7</v></c><c r="B1" t="inlineStr"><is><t>Header B</t></is></c><c r="C1" t="inlineStr"><is><t>Header C</t></is></c><c r="D1" t="inlineStr"><is><t>D</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>42</v></c><c r="C2" t="inlineStr"><is><t>ok</t></is></c><c r="D2" t="inlineStr"><is><t>H</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>South</t></is></c><c r="B3"><v>55</v></c><c r="C3" t="inlineStr"><is><t>done</t></is></c><c r="D3" t="inlineStr"><is><t>L</t></is></c></row>
    <row r="4"><c r="A4" t="inlineStr"><is><t>M</t></is></c><c r="B4" t="inlineStr"><is><t>N</t></is></c><c r="C4" t="inlineStr"><is><t>O</t></is></c><c r="D4" t="inlineStr"><is><t>P</t></is></c></row>
  </sheetData>
</worksheet>"#
}

#[test]
fn serve_op_supports_pptx_notes_mutations() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-pptx-notes-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = "testdata/pptx/title-content/presentation.pptx";
    let output = temp_dir.join("serve-pptx-notes-out.pptx");
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
            serde_json::json!({"file": input, "out": output_str}),
        ),
    );
    assert!(
        open_response.get("error").is_none(),
        "pptx notes open failed: {open_response:?}"
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let set_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "op",
            serde_json::json!({
                "session": session,
                "command": "pptx notes set",
                "args": {
                    "slide": 1,
                    "text": "Serve speaker note\nSecond line"
                },
            }),
        ),
    );
    assert!(
        set_response.get("error").is_none(),
        "pptx notes set op failed: {set_response:?}"
    );
    assert_eq!(
        set_response["result"]["readback"]["text"],
        Value::String("Serve speaker note\nSecond line".to_string())
    );
    assert_eq!(
        set_response["result"]["readback"]["createdPart"],
        Value::Bool(true)
    );

    let changed_notes_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx notes show",
                "args": {"slide": 1},
            }),
        ),
    );
    assert!(
        changed_notes_response.get("error").is_none(),
        "pptx notes changed inspect failed: {changed_notes_response:?}"
    );
    assert_eq!(
        changed_notes_response["result"]["notes"]["plainText"],
        Value::String("Serve speaker note\nSecond line".to_string())
    );

    let clear_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            4,
            "op",
            serde_json::json!({
                "session": session,
                "command": "pptx notes clear",
                "args": {"slide": 1},
            }),
        ),
    );
    assert!(
        clear_response.get("error").is_none(),
        "pptx notes clear op failed: {clear_response:?}"
    );
    assert_eq!(
        clear_response["result"]["readback"]["text"],
        Value::String(String::new())
    );
    assert_eq!(
        clear_response["result"]["readback"]["createdPart"],
        Value::Bool(false)
    );

    let cleared_notes_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            5,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx notes show",
                "args": {"slide": 1},
            }),
        ),
    );
    assert!(
        cleared_notes_response.get("error").is_none(),
        "pptx notes cleared inspect failed: {cleared_notes_response:?}"
    );
    assert_eq!(
        cleared_notes_response["result"]["notes"]["plainText"],
        Value::String(String::new())
    );

    let plan_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(6, "plan", serde_json::json!({"session": session})),
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][0],
        Value::String("pptx".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][1],
        Value::String("notes".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("set".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][1]["argv"][2],
        Value::String("clear".to_string())
    );

    let validate_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(7, "validate", serde_json::json!({"session": session})),
    );
    assert!(
        validate_response.get("error").is_none(),
        "pptx notes validate failed: {validate_response:?}"
    );

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(8, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "pptx notes commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "pptx notes serve validate exit");
    assert_eq!(validate_stderr, None, "pptx notes serve validate stderr");

    let (notes_code, notes_stdout, notes_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "notes",
        "show",
        &output_str,
        "--slide",
        "1",
    ]);
    assert_eq!(notes_code, 0, "pptx notes output readback exit");
    assert_eq!(notes_stderr, None, "pptx notes output readback stderr");
    let notes = notes_stdout.expect("pptx notes output readback");
    assert_eq!(notes["notes"]["plainText"], Value::String(String::new()));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_pptx_shapes_delete() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-pptx-shapes-delete-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = "testdata/pptx/title-content/presentation.pptx";
    let output = temp_dir.join("serve-pptx-shapes-delete-out.pptx");
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
            serde_json::json!({"file": input, "out": output_str}),
        ),
    );
    assert!(
        open_response.get("error").is_none(),
        "pptx shapes delete open failed: {open_response:?}"
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let before_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx shapes show",
                "args": {"slide": 2, "include-text": true, "include-bounds": true},
            }),
        ),
    );
    assert!(
        before_response.get("error").is_none(),
        "pptx shapes before inspect failed: {before_response:?}"
    );
    let before_shapes = before_response["result"]["shapes"]
        .as_array()
        .expect("before shapes");
    assert!(
        before_shapes
            .iter()
            .any(|shape| shape["primarySelector"] == Value::String("title".to_string())),
        "source slide should expose title selector: {before_shapes:?}"
    );

    let delete_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "op",
            serde_json::json!({
                "session": session,
                "command": "pptx shapes delete",
                "args": {"slide": 2, "target": "title"},
            }),
        ),
    );
    assert!(
        delete_response.get("error").is_none(),
        "pptx shapes delete op failed: {delete_response:?}"
    );
    assert_eq!(
        delete_response["result"]["readback"]["slide"],
        Value::from(2)
    );
    assert_eq!(
        delete_response["result"]["readback"]["target"],
        Value::String("title".to_string())
    );
    assert_eq!(
        delete_response["result"]["readback"]["deleted"]["target"],
        Value::String("title".to_string())
    );

    let changed_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            4,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx shapes show",
                "args": {"slide": 2, "include-text": true, "include-bounds": true},
            }),
        ),
    );
    assert!(
        changed_response.get("error").is_none(),
        "pptx shapes changed inspect failed: {changed_response:?}"
    );
    let changed_shapes = changed_response["result"]["shapes"]
        .as_array()
        .expect("changed shapes");
    assert!(
        !changed_shapes
            .iter()
            .any(|shape| shape["primarySelector"] == Value::String("title".to_string())),
        "deleted title selector should be absent: {changed_shapes:?}"
    );
    assert!(
        changed_shapes
            .iter()
            .any(|shape| shape["primarySelector"] == Value::String("body".to_string())),
        "body selector should remain after title deletion: {changed_shapes:?}"
    );

    let plan_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(5, "plan", serde_json::json!({"session": session})),
    );
    let argv = plan_response["result"]["plan"][0]["argv"]
        .as_array()
        .expect("delete plan argv");
    assert_eq!(argv[0], Value::String("pptx".to_string()));
    assert_eq!(argv[1], Value::String("shapes".to_string()));
    assert_eq!(argv[2], Value::String("delete".to_string()));
    assert_eq!(argv[4], Value::String("--slide".to_string()));
    assert_eq!(argv[5], Value::String("2".to_string()));
    assert_eq!(argv[6], Value::String("--target".to_string()));
    assert_eq!(argv[7], Value::String("title".to_string()));

    let validate_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(6, "validate", serde_json::json!({"session": session})),
    );
    assert!(
        validate_response.get("error").is_none(),
        "pptx shapes delete validate failed: {validate_response:?}"
    );

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(7, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "pptx shapes delete commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "pptx shapes delete validate exit");
    assert_eq!(validate_stderr, None, "pptx shapes delete validate stderr");

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        &output_str,
        "--slide",
        "2",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(show_code, 0, "pptx shapes delete output show exit");
    assert_eq!(show_stderr, None, "pptx shapes delete output show stderr");
    let output_shapes = show_stdout.expect("pptx shapes delete output show")["shapes"]
        .as_array()
        .expect("output shapes")
        .clone();
    assert!(
        !output_shapes
            .iter()
            .any(|shape| shape["primarySelector"] == Value::String("title".to_string())),
        "committed output should not contain deleted title selector: {output_shapes:?}"
    );
    assert!(
        output_shapes
            .iter()
            .any(|shape| shape["primarySelector"] == Value::String("body".to_string())),
        "committed output should retain body selector: {output_shapes:?}"
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
