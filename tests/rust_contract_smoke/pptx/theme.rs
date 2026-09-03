#[test]
fn pptx_theme_update_deck_readback_dry_run_and_errors_match_rust_baseline() {
    let fixture = "testdata/pptx/multi-layout/presentation.pptx";
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-theme-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx theme temp dir");

    assert_baseline_rust_json_match(
        &[
            "--json",
            "pptx",
            "theme",
            "update",
            fixture,
            "--color",
            "accent1=FF0000",
            "--major-font",
            "Georgia",
            "--minor-font",
            "Verdana",
            "--dry-run",
        ],
        "theme update dry-run",
    );

    let baseline_out = temp_dir.join("baseline-theme-update.pptx");
    let rust_out = temp_dir.join("rust-theme-update.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline theme update path");
    let rust_out_str = rust_out.to_str().expect("rust theme update path");
    let baseline_args = [
        "--json",
        "pptx",
        "theme",
        "update",
        fixture,
        "--color",
        "accent1=FF0000",
        "--major-font",
        "Georgia",
        "--minor-font",
        "Verdana",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "theme",
        "update",
        fixture,
        "--color",
        "accent1=FF0000",
        "--major-font",
        "Georgia",
        "--minor-font",
        "Verdana",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "theme update saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "theme update saved stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust theme update stdout"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline theme update stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "theme update saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline theme update output missing"
    );
    assert!(rust_out.exists(), "Rust theme update output missing");

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "masters",
        "show",
        baseline_out_str,
        "--master",
        "1",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "masters",
        "show",
        rust_out_str,
        "--master",
        "1",
    ]);
    assert_eq!(rust_read_code, baseline_read_code, "theme readback exit");
    assert_eq!(
        rust_read_stderr, baseline_read_stderr,
        "theme readback stderr"
    );
    assert_eq!(
        rust_read_stdout.expect("rust theme readback"),
        baseline_read_stdout.expect("baseline theme readback"),
        "theme readback stdout"
    );

    for (label, args) in [
        (
            "theme update no updates",
            vec!["--json", "pptx", "theme", "update", fixture, "--dry-run"],
        ),
        (
            "theme update invalid color format",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--color",
                "accent1",
                "--dry-run",
            ],
        ),
        (
            "theme update invalid color name",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--color",
                "bad=FF0000",
                "--dry-run",
            ],
        ),
        (
            "theme update invalid hex",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--color",
                "accent1=ZZZZZZ",
                "--dry-run",
            ],
        ),
        (
            "theme update slide color oracle error",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--mode",
                "slide",
                "--slide",
                "1",
                "--color",
                "accent1=FF0000",
                "--dry-run",
            ],
        ),
        (
            "theme update slide font unsupported",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--mode",
                "slide",
                "--slide",
                "1",
                "--major-font",
                "Georgia",
                "--dry-run",
            ],
        ),
        (
            "theme update unsupported xlsx",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                "testdata/xlsx/minimal-workbook/workbook.xlsx",
                "--color",
                "accent1=FF0000",
                "--dry-run",
            ],
        ),
    ] {
        assert_baseline_rust_json_match(&args, label);
    }
}
