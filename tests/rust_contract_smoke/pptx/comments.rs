#[test]
fn pptx_comments_add_edit_remove_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-comments-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx comments temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let go_added = temp_dir.join("go-added.pptx");
    let rust_added = temp_dir.join("rust-added.pptx");
    let go_added_str = go_added.to_str().expect("go added path");
    let rust_added_str = rust_added.to_str().expect("rust added path");

    let go_add_args = [
        "--json",
        "pptx",
        "comments",
        "add",
        fixture,
        "--slide",
        "1",
        "--author",
        "Alice",
        "--initials",
        "AB",
        "--text",
        "Fix the title",
        "--date",
        "2026-06-06T10:30:00Z",
        "--out",
        go_added_str,
    ];
    let rust_add_args = [
        "--json",
        "pptx",
        "comments",
        "add",
        fixture,
        "--slide",
        "1",
        "--author",
        "Alice",
        "--initials",
        "AB",
        "--text",
        "Fix the title",
        "--date",
        "2026-06-06T10:30:00Z",
        "--out",
        rust_added_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_add_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_add_args);
    assert_eq!(rust_code, go_code, "comments add exit");
    assert_eq!(rust_stderr, go_stderr, "comments add stderr");
    let go_add_json = go_stdout.expect("go comments add stdout");
    let rust_add_json = rust_stdout.expect("rust comments add stdout");
    assert_eq!(
        scrub_path(rust_add_json.clone(), rust_added_str, "[ADDED]"),
        scrub_path(go_add_json.clone(), go_added_str, "[ADDED]"),
        "comments add stdout"
    );
    assert!(go_added.exists(), "Go comments add output missing");
    assert!(rust_added.exists(), "Rust comments add output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add_json, "validateCommand");

    let (go_list_code, go_list_stdout, go_list_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "comments",
        "list",
        go_added_str,
        "--slide",
        "1",
    ]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "comments",
        "list",
        rust_added_str,
        "--slide",
        "1",
    ]);
    assert_eq!(rust_list_code, go_list_code, "comments add readback exit");
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "comments add readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_list_stdout.expect("rust add readback"),
            rust_added_str,
            "[ADDED]"
        ),
        scrub_path(
            go_list_stdout.expect("go add readback"),
            go_added_str,
            "[ADDED]"
        ),
        "comments add readback stdout"
    );

    let handle = rust_add_json["handle"].as_str().expect("comment handle");
    let go_edited = temp_dir.join("go-edited.pptx");
    let rust_edited = temp_dir.join("rust-edited.pptx");
    let go_edited_str = go_edited.to_str().expect("go edited path");
    let rust_edited_str = rust_edited.to_str().expect("rust edited path");
    let go_edit_args = [
        "--json",
        "pptx",
        "comments",
        "edit",
        go_added_str,
        "--handle",
        handle,
        "--text",
        "Updated note",
        "--out",
        go_edited_str,
    ];
    let rust_edit_args = [
        "--json",
        "pptx",
        "comments",
        "edit",
        rust_added_str,
        "--handle",
        handle,
        "--text",
        "Updated note",
        "--out",
        rust_edited_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_edit_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_edit_args);
    assert_eq!(rust_code, go_code, "comments edit exit");
    assert_eq!(rust_stderr, go_stderr, "comments edit stderr");
    let rust_edit_json = rust_stdout.expect("rust comments edit stdout");
    assert_eq!(
        scrub_paths(
            rust_edit_json.clone(),
            &[(rust_added_str, "[ADDED]"), (rust_edited_str, "[EDITED]")]
        ),
        scrub_paths(
            go_stdout.expect("go comments edit stdout"),
            &[(go_added_str, "[ADDED]"), (go_edited_str, "[EDITED]")]
        ),
        "comments edit stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_edit_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_edit_json, "validateCommand");

    let go_removed = temp_dir.join("go-removed.pptx");
    let rust_removed = temp_dir.join("rust-removed.pptx");
    let go_removed_str = go_removed.to_str().expect("go removed path");
    let rust_removed_str = rust_removed.to_str().expect("rust removed path");
    let go_remove_args = [
        "--json",
        "pptx",
        "comments",
        "remove",
        go_edited_str,
        "--handle",
        handle,
        "--out",
        go_removed_str,
    ];
    let rust_remove_args = [
        "--json",
        "pptx",
        "comments",
        "remove",
        rust_edited_str,
        "--handle",
        handle,
        "--out",
        rust_removed_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_remove_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_remove_args);
    assert_eq!(rust_code, go_code, "comments remove exit");
    assert_eq!(rust_stderr, go_stderr, "comments remove stderr");
    let rust_remove_json = rust_stdout.expect("rust comments remove stdout");
    assert_eq!(
        scrub_paths(
            rust_remove_json.clone(),
            &[
                (rust_edited_str, "[EDITED]"),
                (rust_removed_str, "[REMOVED]")
            ]
        ),
        scrub_paths(
            go_stdout.expect("go comments remove stdout"),
            &[(go_edited_str, "[EDITED]"), (go_removed_str, "[REMOVED]")]
        ),
        "comments remove stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_remove_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_remove_json, "validateCommand");

    let (go_empty_code, go_empty_stdout, go_empty_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "comments",
        "list",
        go_removed_str,
        "--slide",
        "1",
    ]);
    let (rust_empty_code, rust_empty_stdout, rust_empty_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "comments",
        "list",
        rust_removed_str,
        "--slide",
        "1",
    ]);
    assert_eq!(
        rust_empty_code, go_empty_code,
        "comments remove readback exit"
    );
    assert_eq!(
        rust_empty_stderr, go_empty_stderr,
        "comments remove readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_empty_stdout.expect("rust remove readback"),
            rust_removed_str,
            "[REMOVED]"
        ),
        scrub_path(
            go_empty_stdout.expect("go remove readback"),
            go_removed_str,
            "[REMOVED]"
        ),
        "comments remove readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "comments",
        "add",
        fixture,
        "--slide",
        "1",
        "--author",
        "Alice",
        "--text",
        "Dry note",
        "--date",
        "2026-06-06T10:30:00Z",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "comments add dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "comments add dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust comments add dry-run"),
        go_stdout.expect("go comments add dry-run"),
        "comments add dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "comments",
            "add",
            fixture,
            "--slide",
            "99",
            "--author",
            "Alice",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "add",
            fixture,
            "--slide",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "edit",
            rust_added_str,
            "--slide",
            "1",
            "--comment-id",
            "1",
            "--text",
            "changed",
            "--expect-hash",
            "sha256:bogus",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "remove",
            fixture,
            "--slide",
            "1",
            "--comment-id",
            "1",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let go_args = args
            .iter()
            .map(|arg| {
                if *arg == rust_added_str {
                    go_added_str
                } else {
                    *arg
                }
            })
            .collect::<Vec<_>>();
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "comments error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "comments error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "comments error stderr for {args:?}");
    }
}
