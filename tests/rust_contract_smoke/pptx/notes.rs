#[test]
fn pptx_notes_set_clear_dry_run_and_errors_match_rust_baseline() {
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
    let baseline_set = temp_dir.join("baseline-set.pptx");
    let rust_set = temp_dir.join("rust-set.pptx");
    let baseline_set_str = baseline_set.to_str().expect("baseline set path");
    let rust_set_str = rust_set.to_str().expect("rust set path");
    let set_text = "First line\nSecond line";

    let baseline_set_args = [
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
        baseline_set_str,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_set_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_code, baseline_code, "notes set exit");
    assert_eq!(rust_stderr, baseline_stderr, "notes set stderr");
    let rust_set_json = rust_stdout.expect("rust notes set stdout");
    assert_eq!(
        scrub_path(rust_set_json.clone(), rust_set_str, "[OUT]"),
        scrub_path(baseline_stdout.expect("baseline notes set stdout"), baseline_set_str, "[OUT]"),
        "notes set stdout"
    );
    assert!(baseline_set.exists(), "Rust baseline notes set output missing");
    assert!(rust_set.exists(), "Rust notes set output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_set_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_set_json, "validateCommand");

    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json", "pptx", "notes", "show", baseline_set_str, "--slide", "1",
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
    assert_eq!(rust_show_code, baseline_show_code, "notes set readback exit");
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "notes set readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust set readback"),
        baseline_show_stdout.expect("baseline set readback"),
        "notes set readback stdout"
    );

    let baseline_clear = temp_dir.join("baseline-clear.pptx");
    let rust_clear = temp_dir.join("rust-clear.pptx");
    let baseline_clear_str = baseline_clear.to_str().expect("baseline clear path");
    let rust_clear_str = rust_clear.to_str().expect("rust clear path");
    let baseline_clear_args = [
        "--json",
        "pptx",
        "notes",
        "clear",
        notes_fixture,
        "--slide",
        "2",
        "--out",
        baseline_clear_str,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_clear_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_code, baseline_code, "notes clear exit");
    assert_eq!(rust_stderr, baseline_stderr, "notes clear stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust notes clear stdout"),
            rust_clear_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline notes clear stdout"),
            baseline_clear_str,
            "[OUT]"
        ),
        "notes clear stdout"
    );

    let (baseline_clear_show_code, baseline_clear_show_stdout, baseline_clear_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "notes",
        "show",
        baseline_clear_str,
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
        rust_clear_show_code, baseline_clear_show_code,
        "notes clear readback exit"
    );
    assert_eq!(
        rust_clear_show_stderr, baseline_clear_show_stderr,
        "notes clear readback stderr"
    );
    assert_eq!(
        rust_clear_show_stdout.expect("rust clear readback"),
        baseline_clear_show_stdout.expect("baseline clear readback"),
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "notes set dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "notes set dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust notes set dry-run"),
        baseline_stdout.expect("baseline notes set dry-run"),
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&out_of_range);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&out_of_range);
    assert_eq!(rust_code, baseline_code, "notes set out-of-range exit");
    assert_eq!(rust_stdout, baseline_stdout, "notes set out-of-range stdout");
    assert_eq!(rust_stderr, baseline_stderr, "notes set out-of-range stderr");
}
