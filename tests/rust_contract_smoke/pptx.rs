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
