// PPTX frozen mutation/render/verify contract tests live here while shared
// baseline and process helpers remain in the parent integration test crate.
use super::*;

#[test]
fn frozen_pptx_mutation_and_validate_match_go_baseline() {
    let baseline = baseline();
    let temp_dir = std::env::temp_dir().join(format!("ooxml-rust-contract-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let edited = temp_dir.join("edited.pptx");
    let render_dir = temp_dir.join("rendered");
    let edited_str = edited.to_str().expect("temp path");
    let render_dir_str = render_dir.to_str().expect("render path");

    let edit_args = [
        "--json",
        "pptx",
        "replace",
        "text",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--slide",
        "1",
        "--target",
        "title",
        "--text",
        "Rust Port Contract",
        "--out",
        edited_str,
    ];
    let (edit_code, edit_stdout, edit_stderr) = run_ooxml(&edit_args);
    assert_eq!(edit_code, 0);
    assert_eq!(edit_stderr, None);
    let edit_expected = baseline["mutation"]["edit"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            edit_stdout.expect("edit stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        edit_expected
    );
    assert!(edited.exists());

    let validate_args = ["--json", "--strict", "validate", edited_str];
    let (validate_code, validate_stdout, validate_stderr) = run_ooxml(&validate_args);
    assert_eq!(validate_code, 0);
    assert_eq!(validate_stderr, None);
    let validate_expected = baseline["mutation"]["validate"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            validate_stdout.expect("validate stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        validate_expected
    );

    let render_args = [
        "pptx",
        "render",
        edited_str,
        "--out",
        render_dir_str,
        "--slides",
        "1",
        "--format",
        "json",
    ];
    let (render_code, render_stdout, render_stderr) =
        run_ooxml_with_env(&render_args, &[("OOXML_RUST_MOCK_RENDER", "1")]);
    assert_eq!(render_code, 0);
    assert_eq!(render_stderr, None);
    let render_expected = baseline["mutation"]["render"]["stdoutJson"].clone();
    assert_eq!(
        scrub_paths(
            render_stdout.expect("render stdout"),
            &[
                (edited_str, "[EDITED_PPTX]"),
                (render_dir_str, "[RENDER_DIR]")
            ]
        ),
        render_expected
    );

    let verify_args = [
        "--format",
        "json",
        "verify",
        edited_str,
        "--baseline",
        "testdata/pptx/minimal-title/presentation.pptx",
    ];
    let (verify_code, verify_stdout, verify_stderr) = run_ooxml(&verify_args);
    assert_eq!(verify_code, 0);
    assert_eq!(verify_stderr, None);
    let verify_expected = baseline["mutation"]["verify"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            verify_stdout.expect("verify stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        verify_expected
    );
}

#[test]
fn pptx_notes_set_clear_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-notes-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("notes temp dir");

    let set_fixture = "testdata/pptx/title-content/presentation.pptx";
    let notes_fixture = "testdata/pptx/notes-slide/presentation.pptx";
    let go_set = temp_dir.join("go-set.pptx");
    let rust_set = temp_dir.join("rust-set.pptx");
    let go_set_str = go_set.to_str().expect("go set path");
    let rust_set_str = rust_set.to_str().expect("rust set path");
    let set_text = "First line\nSecond line";

    let go_set_args = [
        "--json",
        "pptx",
        "notes",
        "set",
        set_fixture,
        "--slide",
        "1",
        "--text",
        set_text,
        "--out",
        go_set_str,
    ];
    let rust_set_args = [
        "--json",
        "pptx",
        "notes",
        "set",
        set_fixture,
        "--slide",
        "1",
        "--text",
        set_text,
        "--out",
        rust_set_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_set_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_code, go_code, "notes set exit");
    assert_eq!(rust_stderr, go_stderr, "notes set stderr");
    let rust_set_json = rust_stdout.expect("rust notes set stdout");
    assert_eq!(
        scrub_path(rust_set_json.clone(), rust_set_str, "[OUT]"),
        scrub_path(go_stdout.expect("go notes set stdout"), go_set_str, "[OUT]"),
        "notes set stdout"
    );
    assert!(go_set.exists(), "Go notes set output missing");
    assert!(rust_set.exists(), "Rust notes set output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_set_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_set_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "notes", "show", go_set_str, "--slide", "1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "notes",
        "show",
        rust_set_str,
        "--slide",
        "1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "notes set readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "notes set readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust set readback"),
        go_show_stdout.expect("go set readback"),
        "notes set readback stdout"
    );

    let go_clear = temp_dir.join("go-clear.pptx");
    let rust_clear = temp_dir.join("rust-clear.pptx");
    let go_clear_str = go_clear.to_str().expect("go clear path");
    let rust_clear_str = rust_clear.to_str().expect("rust clear path");
    let go_clear_args = [
        "--json",
        "pptx",
        "notes",
        "clear",
        notes_fixture,
        "--slide",
        "2",
        "--out",
        go_clear_str,
    ];
    let rust_clear_args = [
        "--json",
        "pptx",
        "notes",
        "clear",
        notes_fixture,
        "--slide",
        "2",
        "--out",
        rust_clear_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_clear_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_code, go_code, "notes clear exit");
    assert_eq!(rust_stderr, go_stderr, "notes clear stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust notes clear stdout"),
            rust_clear_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go notes clear stdout"),
            go_clear_str,
            "[OUT]"
        ),
        "notes clear stdout"
    );

    let (go_clear_show_code, go_clear_show_stdout, go_clear_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "notes",
        "show",
        go_clear_str,
        "--slide",
        "2",
    ]);
    let (rust_clear_show_code, rust_clear_show_stdout, rust_clear_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "notes",
        "show",
        rust_clear_str,
        "--slide",
        "2",
    ]);
    assert_eq!(
        rust_clear_show_code, go_clear_show_code,
        "notes clear readback exit"
    );
    assert_eq!(
        rust_clear_show_stderr, go_clear_show_stderr,
        "notes clear readback stderr"
    );
    assert_eq!(
        rust_clear_show_stdout.expect("rust clear readback"),
        go_clear_show_stdout.expect("go clear readback"),
        "notes clear readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "notes",
        "set",
        set_fixture,
        "--slide",
        "1",
        "--text",
        "draft notes",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "notes set dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "notes set dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust notes set dry-run"),
        go_stdout.expect("go notes set dry-run"),
        "notes set dry-run stdout"
    );

    let out_of_range = [
        "--json",
        "pptx",
        "notes",
        "set",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--slide",
        "99",
        "--text",
        "x",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&out_of_range);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&out_of_range);
    assert_eq!(rust_code, go_code, "notes set out-of-range exit");
    assert_eq!(rust_stdout, go_stdout, "notes set out-of-range stdout");
    assert_eq!(rust_stderr, go_stderr, "notes set out-of-range stderr");
}

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
