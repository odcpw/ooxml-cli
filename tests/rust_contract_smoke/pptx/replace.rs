#[test]
fn pptx_replace_text_occurrences_saved_readback_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-occurrences-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("replace text occurrences temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "replace text occurrences dry-run exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "replace text occurrences dry-run stderr"
    );
    let rust_dry_json = rust_stdout.expect("rust replace text occurrences dry-run");
    assert_eq!(
        rust_dry_json,
        baseline_stdout.expect("baseline replace text occurrences dry-run"),
        "replace text occurrences dry-run stdout"
    );
    let plan_hash = rust_dry_json["staleGuard"]["actualPlanHash"]
        .as_str()
        .expect("dry-run plan hash");

    let baseline_out = temp_dir.join("baseline-occurrences.pptx");
    let rust_out = temp_dir.join("rust-occurrences.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline occurrences output path");
    let rust_out_str = rust_out.to_str().expect("rust occurrences output path");
    let baseline_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--expect-count",
        "1",
        "--expect-plan-hash",
        plan_hash,
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--expect-count",
        "1",
        "--expect-plan-hash",
        plan_hash,
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "replace text occurrences saved exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "replace text occurrences saved stderr"
    );
    let rust_json = rust_stdout.expect("rust replace text occurrences saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline replace text occurrences saved"),
            baseline_out_str,
            "[OUT]"
        ),
        "replace text occurrences saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline replace text occurrences output missing"
    );
    assert!(
        rust_out.exists(),
        "Rust replace text occurrences output missing"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json["matches"][0], "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (baseline_text_code, baseline_text_stdout, baseline_text_stderr) =
        run_ooxml_baseline(&["--json", "pptx", "extract", "text", baseline_out_str]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_text_code, baseline_text_code, "text readback exit");
    assert_eq!(rust_text_stderr, baseline_text_stderr, "text readback stderr");
    assert_eq!(
        scrub_path(
            rust_text_stdout.expect("rust text readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_text_stdout.expect("baseline text readback"),
            baseline_out_str,
            "[OUT]"
        ),
        "text readback stdout"
    );

    let count_mismatch = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--expect-count",
        "2",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&count_mismatch);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&count_mismatch);
    assert_eq!(rust_code, baseline_code, "replace text occurrences guard exit");
    assert_eq!(
        rust_stdout, baseline_stdout,
        "replace text occurrences guard stdout"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "replace text occurrences guard stderr"
    );

    let no_match = temp_dir.join("no-match.pptx");
    let no_match_str = no_match.to_str().expect("no match path");
    let no_match_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Missing",
        "--new-text",
        "Tiny",
        "--out",
        no_match_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&no_match_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&no_match_args);
    assert_eq!(rust_code, baseline_code, "replace text occurrences no-match exit");
    assert_eq!(
        rust_stdout, baseline_stdout,
        "replace text occurrences no-match stdout"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "replace text occurrences no-match stderr"
    );
}


#[test]
fn pptx_replace_images_saved_readback_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-images-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("replace images temp dir");

    let fixture = "testdata/pptx/slide-assembly-notes-media/presentation.pptx";
    let image = "testdata/test_image.png";
    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--slide",
        "2",
        "--target",
        "shape:4",
        "--image",
        image,
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "replace images dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "replace images dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust replace images dry-run"),
        baseline_stdout.expect("baseline replace images dry-run"),
        "replace images dry-run stdout"
    );

    let baseline_out = temp_dir.join("baseline-image.pptx");
    let rust_out = temp_dir.join("rust-image.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline image output path");
    let rust_out_str = rust_out.to_str().expect("rust image output path");
    let baseline_args = [
        "--json", "pptx", "replace", "images", fixture, "--slide", "2", "--target", "shape:4",
        "--image", image, "--out", baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--slide",
        "2",
        "--target",
        "shape:4",
        "--image",
        image,
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "replace images saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "replace images saved stderr");
    let rust_json = rust_stdout.expect("rust replace images saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline replace images saved"),
            baseline_out_str,
            "[OUT]"
        ),
        "replace images saved stdout"
    );
    assert!(baseline_out.exists(), "Rust baseline replace images output missing");
    assert!(rust_out.exists(), "Rust replace images output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let baseline_dir = temp_dir.join("baseline-image-extract");
    let rust_dir = temp_dir.join("rust-image-extract");
    let baseline_dir_str = baseline_dir.to_str().expect("baseline image extract dir");
    let rust_dir_str = rust_dir.to_str().expect("rust image extract dir");
    let (baseline_extract_code, baseline_extract_stdout, baseline_extract_stderr) = run_ooxml_baseline(&[
        "--json", "pptx", "extract", "images", baseline_out_str, "--out", baseline_dir_str,
    ]);
    let (rust_extract_code, rust_extract_stdout, rust_extract_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "extract",
        "images",
        rust_out_str,
        "--out",
        rust_dir_str,
    ]);
    assert_eq!(rust_extract_code, baseline_extract_code, "image readback exit");
    assert_eq!(
        rust_extract_stderr, baseline_extract_stderr,
        "image readback stderr"
    );
    assert_eq!(
        scrub_paths(
            rust_extract_stdout.expect("rust image readback"),
            &[(rust_out_str, "[PPTX]"), (rust_dir_str, "[OUT]")]
        ),
        scrub_paths(
            baseline_extract_stdout.expect("baseline image readback"),
            &[(baseline_out_str, "[PPTX]"), (baseline_dir_str, "[OUT]")]
        ),
        "image readback stdout"
    );
    assert_export_dirs_match(&baseline_dir, &rust_dir);

    let missing_target = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--slide",
        "2",
        "--target",
        "shape:999",
        "--image",
        image,
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&missing_target);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_target);
    assert_eq!(rust_code, baseline_code, "replace images missing target exit");
    assert_eq!(
        rust_stdout, baseline_stdout,
        "replace images missing target stdout"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "replace images missing target stderr"
    );
}


#[test]
fn pptx_replace_images_for_slides_saved_dry_run_and_invalid_cases_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-images-for-slides-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("replace images for-slides temp dir");

    let fixture = "testdata/pptx/picture-placeholder/presentation.pptx";
    let image = "testdata/test_image.png";
    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--for-slides",
        "1-2",
        "--target",
        "shape:2",
        "--image",
        image,
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "replace images for-slides dry-run exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "replace images for-slides dry-run stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust replace images for-slides dry-run"),
        baseline_stdout.expect("baseline replace images for-slides dry-run"),
        "replace images for-slides dry-run stdout"
    );

    let baseline_out = temp_dir.join("baseline-for-slides.pptx");
    let rust_out = temp_dir.join("rust-for-slides.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline for-slides output path");
    let rust_out_str = rust_out.to_str().expect("rust for-slides output path");
    let baseline_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--for-slides",
        "1-2",
        "--target",
        "shape:2",
        "--image",
        image,
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--for-slides",
        "1-2",
        "--target",
        "shape:2",
        "--image",
        image,
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "replace images for-slides saved exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "replace images for-slides saved stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust replace images for-slides saved"),
        baseline_stdout.expect("baseline replace images for-slides saved"),
        "replace images for-slides saved stdout"
    );
    assert!(baseline_out.exists(), "Rust baseline for-slides output missing");
    assert!(rust_out.exists(), "Rust for-slides output missing");

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", rust_out_str, "--strict"]);
    assert_eq!(validate_code, 0, "strict validate exit");
    assert!(validate_stdout.is_some(), "strict validate stdout");
    assert_eq!(validate_stderr, None, "strict validate stderr");
    let (conformance_code, conformance_stdout, conformance_stderr) =
        run_ooxml(&["--json", "conformance", "check", rust_out_str]);
    assert_eq!(conformance_code, 0, "conformance check exit");
    assert!(conformance_stdout.is_some(), "conformance check stdout");
    assert_eq!(conformance_stderr, None, "conformance check stderr");

    let baseline_dir = temp_dir.join("baseline-for-slides-extract");
    let rust_dir = temp_dir.join("rust-for-slides-extract");
    let baseline_dir_str = baseline_dir.to_str().expect("baseline for-slides extract dir");
    let rust_dir_str = rust_dir.to_str().expect("rust for-slides extract dir");
    let (baseline_extract_code, baseline_extract_stdout, baseline_extract_stderr) = run_ooxml_baseline(&[
        "--json", "pptx", "extract", "images", baseline_out_str, "--out", baseline_dir_str,
    ]);
    let (rust_extract_code, rust_extract_stdout, rust_extract_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "extract",
        "images",
        rust_out_str,
        "--out",
        rust_dir_str,
    ]);
    assert_eq!(
        rust_extract_code, baseline_extract_code,
        "for-slides image readback exit"
    );
    assert_eq!(
        rust_extract_stderr, baseline_extract_stderr,
        "for-slides image readback stderr"
    );
    assert_eq!(
        scrub_paths(
            rust_extract_stdout.expect("rust for-slides image readback"),
            &[(rust_out_str, "[PPTX]"), (rust_dir_str, "[OUT]")]
        ),
        scrub_paths(
            baseline_extract_stdout.expect("baseline for-slides image readback"),
            &[(baseline_out_str, "[PPTX]"), (baseline_dir_str, "[OUT]")]
        ),
        "for-slides image readback stdout"
    );
    assert_export_dirs_match(&baseline_dir, &rust_dir);

    for (name, args) in [
        (
            "combined slide and for-slides",
            vec![
                "--json",
                "pptx",
                "replace",
                "images",
                fixture,
                "--slide",
                "2",
                "--for-slides",
                "1-2",
                "--target",
                "shape:2",
                "--image",
                image,
                "--dry-run",
            ],
        ),
        (
            "invalid for-slides range",
            vec![
                "--json",
                "pptx",
                "replace",
                "images",
                fixture,
                "--for-slides",
                "2-1",
                "--target",
                "shape:2",
                "--image",
                image,
                "--dry-run",
            ],
        ),
        (
            "handle target with for-slides",
            vec![
                "--json",
                "pptx",
                "replace",
                "images",
                fixture,
                "--for-slides",
                "2",
                "--target",
                "H:pptx/s:257/shape:n:2",
                "--image",
                image,
                "--dry-run",
            ],
        ),
        (
            "unsupported selector is per-slide batch error",
            vec![
                "--json",
                "pptx",
                "replace",
                "images",
                fixture,
                "--for-slides",
                "2",
                "--target",
                "body",
                "--image",
                image,
                "--dry-run",
            ],
        ),
    ] {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, baseline_code, "{name} exit");
        assert_eq!(rust_stdout, baseline_stdout, "{name} stdout");
        assert_eq!(rust_stderr, baseline_stderr, "{name} stderr");
    }
}


#[test]
fn pptx_replace_text_from_xlsx_matches_rust_baseline_saved_dry_run_and_errors() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-text-xlsx-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx replace text xlsx temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let workbook = temp_dir.join("source-text.xlsx");
    write_simple_xlsx_with_sheet_xml(&workbook, pptx_replace_text_source_sheet_xml());
    let workbook_str = workbook.to_str().expect("source workbook path");

    for args in [
        vec!["--json", "pptx", "replace", "text-from-xlsx"],
        vec!["--json", "pptx", "replace", "text-map-from-xlsx"],
    ] {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, baseline_code, "missing file exit for {args:?}");
        assert_eq!(rust_stdout, baseline_stdout, "missing file stdout for {args:?}");
        assert_eq!(rust_stderr, baseline_stderr, "missing file stderr for {args:?}");
    }

    let baseline_out = temp_dir.join("baseline-text-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-text-from-xlsx.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline text-from-xlsx output path");
    let rust_out_str = rust_out.to_str().expect("rust text-from-xlsx output path");
    let baseline_args = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--slide",
        "1",
        "--target",
        "title",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--slide",
        "1",
        "--target",
        "title",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "text-from-xlsx saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "text-from-xlsx saved stderr");
    let rust_json = rust_stdout.expect("rust text-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline text-from-xlsx stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "text-from-xlsx saved stdout"
    );
    assert!(baseline_out.exists(), "Rust baseline text-from-xlsx output missing");
    assert!(rust_out.exists(), "Rust text-from-xlsx output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        baseline_out_str,
        "--slide",
        "1",
        "--target",
        "title",
        "--include-text",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        rust_out_str,
        "--slide",
        "1",
        "--target",
        "title",
        "--include-text",
    ]);
    assert_eq!(rust_read_code, baseline_read_code, "text-from readback exit");
    assert_eq!(
        rust_read_stderr, baseline_read_stderr,
        "text-from readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust text-from readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_read_stdout.expect("baseline text-from readback"),
            baseline_out_str,
            "[OUT]"
        ),
        "text-from readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
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
        "body",
        "--row-sep",
        "\\n",
        "--col-sep",
        " | ",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "text-from-xlsx dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "text-from-xlsx dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust text-from-xlsx dry-run stdout"),
        baseline_stdout.expect("baseline text-from-xlsx dry-run stdout"),
        "text-from-xlsx dry-run stdout"
    );

    let missing_target = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--slide",
        "1",
        "--target",
        "missing",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&missing_target);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_target);
    assert_eq!(rust_code, baseline_code, "text-from missing target exit");
    assert_eq!(rust_stdout, baseline_stdout, "text-from missing target stdout");
    assert_eq!(rust_stderr, baseline_stderr, "text-from missing target stderr");
}


#[test]
fn pptx_replace_text_map_from_xlsx_matches_rust_baseline_saved_dry_run_and_errors() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-text-map-xlsx-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx replace text map xlsx temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let workbook = temp_dir.join("source-map.xlsx");
    write_simple_xlsx_with_sheet_xml(&workbook, pptx_replace_text_map_source_sheet_xml());
    let workbook_str = workbook.to_str().expect("source map workbook path");
    let table_workbook = temp_dir.join("source-map-table.xlsx");
    write_pptx_text_map_table_xlsx(&table_workbook);
    let table_workbook_str = table_workbook.to_str().expect("source table workbook path");

    let baseline_out = temp_dir.join("baseline-text-map-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-text-map-from-xlsx.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline text-map output path");
    let rust_out_str = rust_out.to_str().expect("rust text-map output path");
    let baseline_args = [
        "--json",
        "pptx",
        "replace",
        "text-map-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C3",
        "--expect-source-range",
        "A1:C3",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "text-map-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C3",
        "--expect-source-range",
        "A1:C3",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "text-map-from-xlsx saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "text-map-from-xlsx saved stderr");
    let rust_json = rust_stdout.expect("rust text-map-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline text-map-from-xlsx stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "text-map-from-xlsx saved stdout"
    );
    assert!(baseline_out.exists(), "Rust baseline text-map-from-xlsx output missing");
    assert!(rust_out.exists(), "Rust text-map-from-xlsx output missing");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (baseline_extract_code, baseline_extract_stdout, baseline_extract_stderr) =
        run_ooxml_baseline(&["--json", "pptx", "extract", "text", baseline_out_str]);
    let (rust_extract_code, rust_extract_stdout, rust_extract_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_extract_code, baseline_extract_code, "text-map extract exit");
    assert_eq!(
        rust_extract_stderr, baseline_extract_stderr,
        "text-map extract stderr"
    );
    assert_eq!(
        scrub_path(
            rust_extract_stdout.expect("rust text-map extract"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_extract_stdout.expect("baseline text-map extract"),
            baseline_out_str,
            "[OUT]"
        ),
        "text-map extract stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "text-map-from-xlsx",
        fixture,
        "--workbook",
        table_workbook_str,
        "--table",
        "TextMap",
        "--slide-col",
        "1",
        "--target-col",
        "2",
        "--text-col",
        "3",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "text-map-from-xlsx dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "text-map-from-xlsx dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust text-map dry-run stdout"),
        baseline_stdout.expect("baseline text-map dry-run stdout"),
        "text-map-from-xlsx dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "replace",
            "text-map-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--range",
            "A1:C3",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "replace",
            "text-map-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--slide-col",
            "1",
            "--target-col",
            "1",
            "--text-col",
            "3",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "replace",
            "text-map-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--target-col",
            "3",
            "--text-col",
            "2",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, baseline_code, "text-map error exit for {args:?}");
        assert_eq!(rust_stdout, baseline_stdout, "text-map error stdout for {args:?}");
        assert_eq!(rust_stderr, baseline_stderr, "text-map error stderr for {args:?}");
    }
}

