#[test]
fn pptx_tables_set_cell_saved_readback_dry_run_text_file_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-set-cell-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx table set-cell temp dir");

    let fixture = "testdata/pptx/table-slide/presentation.pptx";
    let go_out = temp_dir.join("go-set-cell.pptx");
    let rust_out = temp_dir.join("rust-set-cell.pptx");
    let go_out_str = go_out.to_str().expect("go set-cell path");
    let rust_out_str = rust_out.to_str().expect("rust set-cell path");

    let go_args = [
        "--json",
        "pptx",
        "tables",
        "set-cell",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--row",
        "2",
        "--col",
        "2",
        "--text",
        "Rust Port Cell",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "set-cell",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--row",
        "2",
        "--col",
        "2",
        "--text",
        "Rust Port Cell",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "set-cell saved exit");
    assert_eq!(rust_stderr, go_stderr, "set-cell saved stderr");
    let rust_json = rust_stdout.expect("rust set-cell stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(go_stdout.expect("go set-cell stdout"), go_out_str, "[OUT]"),
        "set-cell saved stdout"
    );
    assert!(go_out.exists(), "Go set-cell output missing");
    assert!(rust_out.exists(), "Rust set-cell output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "tables", "show", go_out_str, "--slide", "2", "--target", "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "set-cell readback exit");
    assert_eq!(rust_show_stderr, go_show_stderr, "set-cell readback stderr");
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust set-cell readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go set-cell readback"),
            go_out_str,
            "[OUT]"
        ),
        "set-cell readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "tables",
        "set-cell",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--row",
        "2",
        "--col",
        "2",
        "--text",
        "Dry Cell",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "set-cell dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "set-cell dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust set-cell dry-run stdout"),
        go_stdout.expect("go set-cell dry-run stdout"),
        "set-cell dry-run stdout"
    );

    let text_file = temp_dir.join("cell-text.txt");
    std::fs::write(&text_file, "Text from file").expect("write set-cell text file");
    let text_file_str = text_file.to_str().expect("text file path");
    let text_file_args = [
        "--json",
        "pptx",
        "tables",
        "set-cell",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--row",
        "1",
        "--col",
        "1",
        "--text-file",
        text_file_str,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&text_file_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&text_file_args);
    assert_eq!(rust_code, go_code, "set-cell text-file dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "set-cell text-file dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust set-cell text-file dry-run stdout"),
        go_stdout.expect("go set-cell text-file dry-run stdout"),
        "set-cell text-file dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "tables",
            "set-cell",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "0",
            "--col",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "set-cell",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "9",
            "--col",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "set-cell",
            fixture,
            "--slide",
            "2",
            "--row",
            "1",
            "--col",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "set-cell",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "1",
            "--col",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "set-cell",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "1",
            "--col",
            "1",
            "--text",
            "x",
            "--text-file",
            text_file_str,
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "set-cell error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "set-cell error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "set-cell error stderr for {args:?}");
    }
}

#[test]
fn pptx_tables_insert_row_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-insert-row-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx table insert-row temp dir");

    let fixture = "testdata/pptx/table-slide/presentation.pptx";
    let merged_fixture = "testdata/pptx/table-merged/presentation.pptx";
    let go_out = temp_dir.join("go-insert-row.pptx");
    let rust_out = temp_dir.join("rust-insert-row.pptx");
    let go_out_str = go_out.to_str().expect("go insert-row path");
    let rust_out_str = rust_out.to_str().expect("rust insert-row path");

    let go_args = [
        "--json",
        "pptx",
        "tables",
        "insert-row",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--at",
        "2",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "insert-row",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--at",
        "2",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "insert-row saved exit");
    assert_eq!(rust_stderr, go_stderr, "insert-row saved stderr");
    let rust_json = rust_stdout.expect("rust insert-row stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go insert-row stdout"),
            go_out_str,
            "[OUT]"
        ),
        "insert-row saved stdout"
    );
    assert!(go_out.exists(), "Go insert-row output missing");
    assert!(rust_out.exists(), "Rust insert-row output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "tables", "show", go_out_str, "--slide", "2", "--target", "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "insert-row readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "insert-row readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust insert-row readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go insert-row readback"),
            go_out_str,
            "[OUT]"
        ),
        "insert-row readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "tables",
        "insert-row",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--at",
        "2",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "insert-row dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "insert-row dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust insert-row dry-run stdout"),
        go_stdout.expect("go insert-row dry-run stdout"),
        "insert-row dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "99",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "0",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-row",
            fixture,
            "--slide",
            "2",
            "--at",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "99",
            "--at",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-row",
            merged_fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "3",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "insert-row error exit for {args:?}");
        assert_eq!(
            rust_stdout, go_stdout,
            "insert-row error stdout for {args:?}"
        );
        assert_eq!(
            rust_stderr, go_stderr,
            "insert-row error stderr for {args:?}"
        );
    }
}

#[test]
fn pptx_tables_delete_row_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-delete-row-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx table temp dir");

    let fixture = "testdata/pptx/table-slide/presentation.pptx";
    let go_out = temp_dir.join("go-delete-row.pptx");
    let rust_out = temp_dir.join("rust-delete-row.pptx");
    let go_out_str = go_out.to_str().expect("go delete-row path");
    let rust_out_str = rust_out.to_str().expect("rust delete-row path");

    let go_args = [
        "--json",
        "pptx",
        "tables",
        "delete-row",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--row",
        "2",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "delete-row",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--row",
        "2",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "delete-row saved exit");
    assert_eq!(rust_stderr, go_stderr, "delete-row saved stderr");
    let rust_json = rust_stdout.expect("rust delete-row stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go delete-row stdout"),
            go_out_str,
            "[OUT]"
        ),
        "delete-row saved stdout"
    );
    assert!(go_out.exists(), "Go delete-row output missing");
    assert!(rust_out.exists(), "Rust delete-row output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "tables", "show", go_out_str, "--slide", "2", "--target", "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "delete-row readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "delete-row readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust delete-row readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go delete-row readback"),
            go_out_str,
            "[OUT]"
        ),
        "delete-row readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "tables",
        "delete-row",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--row",
        "2",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "delete-row dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "delete-row dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust delete-row dry-run stdout"),
        go_stdout.expect("go delete-row dry-run stdout"),
        "delete-row dry-run stdout"
    );

    for args in [
        [
            "--json",
            "pptx",
            "tables",
            "delete-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "99",
            "--dry-run",
        ],
        [
            "--json",
            "pptx",
            "tables",
            "delete-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "0",
            "--dry-run",
        ],
        [
            "--json",
            "pptx",
            "tables",
            "delete-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "99",
            "--row",
            "1",
            "--dry-run",
        ],
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "delete-row error exit for {args:?}");
        assert_eq!(
            rust_stdout, go_stdout,
            "delete-row error stdout for {args:?}"
        );
        assert_eq!(
            rust_stderr, go_stderr,
            "delete-row error stderr for {args:?}"
        );
    }
}

#[test]
fn pptx_tables_column_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-table-cols-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx table column temp dir");

    let fixture = "testdata/pptx/table-slide/presentation.pptx";
    let merged_fixture = "testdata/pptx/table-merged/presentation.pptx";
    let go_insert_out = temp_dir.join("go-insert-col.pptx");
    let rust_insert_out = temp_dir.join("rust-insert-col.pptx");
    let go_insert_out_str = go_insert_out.to_str().expect("go insert-col path");
    let rust_insert_out_str = rust_insert_out.to_str().expect("rust insert-col path");

    let go_args = [
        "--json",
        "pptx",
        "tables",
        "insert-col",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--at",
        "1",
        "--width-emu",
        "1234567",
        "--out",
        go_insert_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "insert-col",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--at",
        "1",
        "--width-emu",
        "1234567",
        "--out",
        rust_insert_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "insert-col saved exit");
    assert_eq!(rust_stderr, go_stderr, "insert-col saved stderr");
    let rust_json = rust_stdout.expect("rust insert-col stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_insert_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go insert-col stdout"),
            go_insert_out_str,
            "[OUT]"
        ),
        "insert-col saved stdout"
    );
    assert!(go_insert_out.exists(), "Go insert-col output missing");
    assert!(rust_insert_out.exists(), "Rust insert-col output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        go_insert_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_insert_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "insert-col readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "insert-col readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust insert-col readback"),
            rust_insert_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go insert-col readback"),
            go_insert_out_str,
            "[OUT]"
        ),
        "insert-col readback stdout"
    );

    let go_delete_out = temp_dir.join("go-delete-col.pptx");
    let rust_delete_out = temp_dir.join("rust-delete-col.pptx");
    let go_delete_out_str = go_delete_out.to_str().expect("go delete-col path");
    let rust_delete_out_str = rust_delete_out.to_str().expect("rust delete-col path");
    let go_args = [
        "--json",
        "pptx",
        "tables",
        "delete-col",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--col",
        "2",
        "--out",
        go_delete_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "delete-col",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--col",
        "2",
        "--out",
        rust_delete_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "delete-col saved exit");
    assert_eq!(rust_stderr, go_stderr, "delete-col saved stderr");
    let rust_json = rust_stdout.expect("rust delete-col stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_delete_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go delete-col stdout"),
            go_delete_out_str,
            "[OUT]"
        ),
        "delete-col saved stdout"
    );
    assert!(go_delete_out.exists(), "Go delete-col output missing");
    assert!(rust_delete_out.exists(), "Rust delete-col output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        go_delete_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_delete_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "delete-col readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "delete-col readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust delete-col readback"),
            rust_delete_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go delete-col readback"),
            go_delete_out_str,
            "[OUT]"
        ),
        "delete-col readback stdout"
    );

    for dry_run_args in [
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--col",
            "2",
            "--dry-run",
        ],
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
        assert_eq!(
            rust_code, go_code,
            "column dry-run exit for {dry_run_args:?}"
        );
        assert_eq!(
            rust_stderr, go_stderr,
            "column dry-run stderr for {dry_run_args:?}"
        );
        assert_eq!(
            rust_stdout.expect("rust column dry-run stdout"),
            go_stdout.expect("go column dry-run stdout"),
            "column dry-run stdout for {dry_run_args:?}"
        );
    }

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "99",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "0",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--at",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "99",
            "--at",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "1",
            "--width-emu",
            "-1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--col",
            "99",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--col",
            "0",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            fixture,
            "--slide",
            "2",
            "--col",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "99",
            "--col",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            merged_fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--col",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            merged_fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "2",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "column error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "column error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "column error stderr for {args:?}");
    }
}

#[test]
fn pptx_tables_update_from_xlsx_matches_go_oracle_saved_dry_run_and_errors() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-update-xlsx-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx xlsx update temp dir");

    let fixture = "testdata/pptx/table-slide/presentation.pptx";
    let merged_fixture = "testdata/pptx/table-merged/presentation.pptx";
    let workbook = temp_dir.join("source-range.xlsx");
    write_simple_xlsx_with_sheet_xml(&workbook, pptx_update_source_sheet_xml_4x4());
    let workbook_str = workbook.to_str().expect("source workbook path");
    let table_workbook = temp_dir.join("source-table.xlsx");
    write_pptx_update_table_xlsx(&table_workbook);
    let table_workbook_str = table_workbook.to_str().expect("source table workbook path");

    let go_out = temp_dir.join("go-update-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-update-from-xlsx.pptx");
    let go_out_str = go_out.to_str().expect("go update output path");
    let rust_out_str = rust_out.to_str().expect("rust update output path");
    let go_args = [
        "--json",
        "pptx",
        "tables",
        "update-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C3",
        "--formula-mode",
        "formula",
        "--expect-source-range",
        "A1:C3",
        "--slide",
        "2",
        "--target",
        "table:1",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "update-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C3",
        "--formula-mode",
        "formula",
        "--expect-source-range",
        "A1:C3",
        "--slide",
        "2",
        "--target",
        "table:1",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "update-from-xlsx saved exit");
    assert_eq!(rust_stderr, go_stderr, "update-from-xlsx saved stderr");
    let rust_json = rust_stdout.expect("rust update-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go update-from-xlsx stdout"),
            go_out_str,
            "[OUT]"
        ),
        "update-from-xlsx saved stdout"
    );
    assert!(go_out.exists(), "Go update-from-xlsx output missing");
    assert!(rust_out.exists(), "Rust update-from-xlsx output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "tables", "show", go_out_str, "--slide", "2", "--target", "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "update readback exit");
    assert_eq!(rust_show_stderr, go_show_stderr, "update readback stderr");
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust update readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go update readback"),
            go_out_str,
            "[OUT]"
        ),
        "update readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "tables",
        "update-from-xlsx",
        fixture,
        "--workbook",
        table_workbook_str,
        "--table",
        "Sales",
        "--slide",
        "2",
        "--target",
        "table:1",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "update-from-xlsx dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "update-from-xlsx dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust update-from-xlsx dry-run stdout"),
        go_stdout.expect("go update-from-xlsx dry-run stdout"),
        "update-from-xlsx dry-run stdout"
    );

    let title_content = "testdata/pptx/title-content/presentation.pptx";
    let bad_out = temp_dir.join("bad-update.pptx");
    let bad_out_str = bad_out.to_str().expect("bad update output path");
    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            table_workbook_str,
            "--range",
            "A1:C3",
            "--table",
            "Sales",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--range",
            "A1:C3",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--max-cells",
            "1",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--expect-source-range",
            "A1:B3",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:B2",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--formula-mode",
            "bad",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            merged_fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:D4",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            title_content,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1",
            "--slide",
            "2",
            "--target",
            "body",
            "--out",
            bad_out_str,
        ],
    ];
    for args in error_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "update error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "update error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "update error stderr for {args:?}");
    }
}

fn write_pptx_update_table_xlsx(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create pptx update table xlsx");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/tables/table1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>
</Types>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/workbook.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Data" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c><c r="C1" t="inlineStr"><is><t>Note</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>East</t></is></c><c r="B2"><v>10</v></c><c r="C2" t="inlineStr"><is><t>ok</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>West</t></is></c><c r="B3"><f>SUM(B2:B2)*2</f><v>20</v></c><c r="C3" t="inlineStr"><is><t>done</t></is></c></row>
  </sheetData>
  <tableParts count="1"><tablePart r:id="rId1"/></tableParts>
</worksheet>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/_rels/sheet1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/tables/table1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="Sales" displayName="Sales" ref="A1:C3" headerRowCount="1" totalsRowShown="0">
  <autoFilter ref="A1:C3"/>
  <tableColumns count="3">
    <tableColumn id="1" name="Region"/>
    <tableColumn id="2" name="Amount"/>
    <tableColumn id="3" name="Note"/>
  </tableColumns>
  <tableStyleInfo name="TableStyleMedium2" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/>
</table>"#,
    );
    writer.finish().expect("finish pptx update table xlsx");
}
