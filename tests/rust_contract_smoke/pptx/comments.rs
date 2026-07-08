#[test]
fn pptx_comments_add_edit_remove_saved_readback_dry_run_and_errors_match_rust_baseline() {
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
    let baseline_added = temp_dir.join("baseline-added.pptx");
    let rust_added = temp_dir.join("rust-added.pptx");
    let baseline_added_str = baseline_added.to_str().expect("baseline added path");
    let rust_added_str = rust_added.to_str().expect("rust added path");

    let baseline_add_args = [
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
        baseline_added_str,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_add_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_add_args);
    assert_eq!(rust_code, baseline_code, "comments add exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments add stderr");
    let baseline_add_json = baseline_stdout.expect("baseline comments add stdout");
    let rust_add_json = rust_stdout.expect("rust comments add stdout");
    assert_eq!(
        scrub_path(rust_add_json.clone(), rust_added_str, "[ADDED]"),
        scrub_path(baseline_add_json.clone(), baseline_added_str, "[ADDED]"),
        "comments add stdout"
    );
    assert!(baseline_added.exists(), "Rust baseline comments add output missing");
    assert!(rust_added.exists(), "Rust comments add output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add_json, "validateCommand");

    let (baseline_list_code, baseline_list_stdout, baseline_list_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "comments",
        "list",
        baseline_added_str,
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
    assert_eq!(rust_list_code, baseline_list_code, "comments add readback exit");
    assert_eq!(
        rust_list_stderr, baseline_list_stderr,
        "comments add readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_list_stdout.expect("rust add readback"),
            rust_added_str,
            "[ADDED]"
        ),
        scrub_path(
            baseline_list_stdout.expect("baseline add readback"),
            baseline_added_str,
            "[ADDED]"
        ),
        "comments add readback stdout"
    );

    let handle = rust_add_json["handle"].as_str().expect("comment handle");
    let baseline_edited = temp_dir.join("baseline-edited.pptx");
    let rust_edited = temp_dir.join("rust-edited.pptx");
    let baseline_edited_str = baseline_edited.to_str().expect("baseline edited path");
    let rust_edited_str = rust_edited.to_str().expect("rust edited path");
    let baseline_edit_args = [
        "--json",
        "pptx",
        "comments",
        "edit",
        baseline_added_str,
        "--handle",
        handle,
        "--text",
        "Updated note",
        "--out",
        baseline_edited_str,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_edit_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_edit_args);
    assert_eq!(rust_code, baseline_code, "comments edit exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments edit stderr");
    let rust_edit_json = rust_stdout.expect("rust comments edit stdout");
    assert_eq!(
        scrub_paths(
            rust_edit_json.clone(),
            &[(rust_added_str, "[ADDED]"), (rust_edited_str, "[EDITED]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline comments edit stdout"),
            &[(baseline_added_str, "[ADDED]"), (baseline_edited_str, "[EDITED]")]
        ),
        "comments edit stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_edit_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_edit_json, "validateCommand");

    let baseline_removed = temp_dir.join("baseline-removed.pptx");
    let rust_removed = temp_dir.join("rust-removed.pptx");
    let baseline_removed_str = baseline_removed.to_str().expect("baseline removed path");
    let rust_removed_str = rust_removed.to_str().expect("rust removed path");
    let baseline_remove_args = [
        "--json",
        "pptx",
        "comments",
        "remove",
        baseline_edited_str,
        "--handle",
        handle,
        "--out",
        baseline_removed_str,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_remove_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_remove_args);
    assert_eq!(rust_code, baseline_code, "comments remove exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments remove stderr");
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
            baseline_stdout.expect("baseline comments remove stdout"),
            &[(baseline_edited_str, "[EDITED]"), (baseline_removed_str, "[REMOVED]")]
        ),
        "comments remove stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_remove_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_remove_json, "validateCommand");

    let (baseline_empty_code, baseline_empty_stdout, baseline_empty_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "comments",
        "list",
        baseline_removed_str,
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
        rust_empty_code, baseline_empty_code,
        "comments remove readback exit"
    );
    assert_eq!(
        rust_empty_stderr, baseline_empty_stderr,
        "comments remove readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_empty_stdout.expect("rust remove readback"),
            rust_removed_str,
            "[REMOVED]"
        ),
        scrub_path(
            baseline_empty_stdout.expect("baseline remove readback"),
            baseline_removed_str,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "comments add dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments add dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust comments add dry-run"),
        baseline_stdout.expect("baseline comments add dry-run"),
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
        let baseline_args = args
            .iter()
            .map(|arg| {
                if *arg == rust_added_str {
                    baseline_added_str
                } else {
                    *arg
                }
            })
            .collect::<Vec<_>>();
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, baseline_code, "comments error exit for {args:?}");
        assert_eq!(rust_stdout, baseline_stdout, "comments error stdout for {args:?}");
        assert_eq!(rust_stderr, baseline_stderr, "comments error stderr for {args:?}");
    }
}
