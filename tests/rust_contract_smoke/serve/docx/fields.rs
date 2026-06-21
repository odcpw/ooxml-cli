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
