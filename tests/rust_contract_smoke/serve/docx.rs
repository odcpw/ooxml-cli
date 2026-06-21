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

include!("docx/fields.rs");

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

include!("docx/comments.rs");
