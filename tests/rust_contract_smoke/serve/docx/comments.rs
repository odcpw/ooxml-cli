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
