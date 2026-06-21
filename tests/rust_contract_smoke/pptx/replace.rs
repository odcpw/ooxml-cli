#[test]
fn pptx_replace_text_occurrences_saved_readback_dry_run_and_errors_match_go_oracle() {
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "replace text occurrences dry-run exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "replace text occurrences dry-run stderr"
    );
    let rust_dry_json = rust_stdout.expect("rust replace text occurrences dry-run");
    assert_eq!(
        rust_dry_json,
        go_stdout.expect("go replace text occurrences dry-run"),
        "replace text occurrences dry-run stdout"
    );
    let plan_hash = rust_dry_json["staleGuard"]["actualPlanHash"]
        .as_str()
        .expect("dry-run plan hash");

    let go_out = temp_dir.join("go-occurrences.pptx");
    let rust_out = temp_dir.join("rust-occurrences.pptx");
    let go_out_str = go_out.to_str().expect("go occurrences output path");
    let rust_out_str = rust_out.to_str().expect("rust occurrences output path");
    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "replace text occurrences saved exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "replace text occurrences saved stderr"
    );
    let rust_json = rust_stdout.expect("rust replace text occurrences saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go replace text occurrences saved"),
            go_out_str,
            "[OUT]"
        ),
        "replace text occurrences saved stdout"
    );
    assert!(
        go_out.exists(),
        "Go replace text occurrences output missing"
    );
    assert!(
        rust_out.exists(),
        "Rust replace text occurrences output missing"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json["matches"][0], "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_text_code, go_text_stdout, go_text_stderr) =
        run_go_ooxml(&["--json", "pptx", "extract", "text", go_out_str]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_text_code, go_text_code, "text readback exit");
    assert_eq!(rust_text_stderr, go_text_stderr, "text readback stderr");
    assert_eq!(
        scrub_path(
            rust_text_stdout.expect("rust text readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_text_stdout.expect("go text readback"),
            go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&count_mismatch);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&count_mismatch);
    assert_eq!(rust_code, go_code, "replace text occurrences guard exit");
    assert_eq!(
        rust_stdout, go_stdout,
        "replace text occurrences guard stdout"
    );
    assert_eq!(
        rust_stderr, go_stderr,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&no_match_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&no_match_args);
    assert_eq!(rust_code, go_code, "replace text occurrences no-match exit");
    assert_eq!(
        rust_stdout, go_stdout,
        "replace text occurrences no-match stdout"
    );
    assert_eq!(
        rust_stderr, go_stderr,
        "replace text occurrences no-match stderr"
    );
}


#[test]
fn pptx_replace_images_saved_readback_dry_run_and_errors_match_go_oracle() {
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "replace images dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "replace images dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust replace images dry-run"),
        go_stdout.expect("go replace images dry-run"),
        "replace images dry-run stdout"
    );

    let go_out = temp_dir.join("go-image.pptx");
    let rust_out = temp_dir.join("rust-image.pptx");
    let go_out_str = go_out.to_str().expect("go image output path");
    let rust_out_str = rust_out.to_str().expect("rust image output path");
    let go_args = [
        "--json", "pptx", "replace", "images", fixture, "--slide", "2", "--target", "shape:4",
        "--image", image, "--out", go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "replace images saved exit");
    assert_eq!(rust_stderr, go_stderr, "replace images saved stderr");
    let rust_json = rust_stdout.expect("rust replace images saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go replace images saved"),
            go_out_str,
            "[OUT]"
        ),
        "replace images saved stdout"
    );
    assert!(go_out.exists(), "Go replace images output missing");
    assert!(rust_out.exists(), "Rust replace images output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let go_dir = temp_dir.join("go-image-extract");
    let rust_dir = temp_dir.join("rust-image-extract");
    let go_dir_str = go_dir.to_str().expect("go image extract dir");
    let rust_dir_str = rust_dir.to_str().expect("rust image extract dir");
    let (go_extract_code, go_extract_stdout, go_extract_stderr) = run_go_ooxml(&[
        "--json", "pptx", "extract", "images", go_out_str, "--out", go_dir_str,
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
    assert_eq!(rust_extract_code, go_extract_code, "image readback exit");
    assert_eq!(
        rust_extract_stderr, go_extract_stderr,
        "image readback stderr"
    );
    assert_eq!(
        scrub_paths(
            rust_extract_stdout.expect("rust image readback"),
            &[(rust_out_str, "[PPTX]"), (rust_dir_str, "[OUT]")]
        ),
        scrub_paths(
            go_extract_stdout.expect("go image readback"),
            &[(go_out_str, "[PPTX]"), (go_dir_str, "[OUT]")]
        ),
        "image readback stdout"
    );
    assert_export_dirs_match(&go_dir, &rust_dir);

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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&missing_target);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_target);
    assert_eq!(rust_code, go_code, "replace images missing target exit");
    assert_eq!(
        rust_stdout, go_stdout,
        "replace images missing target stdout"
    );
    assert_eq!(
        rust_stderr, go_stderr,
        "replace images missing target stderr"
    );
}


#[test]
fn pptx_replace_images_for_slides_saved_dry_run_and_invalid_cases_match_go_oracle() {
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "replace images for-slides dry-run exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "replace images for-slides dry-run stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust replace images for-slides dry-run"),
        go_stdout.expect("go replace images for-slides dry-run"),
        "replace images for-slides dry-run stdout"
    );

    let go_out = temp_dir.join("go-for-slides.pptx");
    let rust_out = temp_dir.join("rust-for-slides.pptx");
    let go_out_str = go_out.to_str().expect("go for-slides output path");
    let rust_out_str = rust_out.to_str().expect("rust for-slides output path");
    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "replace images for-slides saved exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "replace images for-slides saved stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust replace images for-slides saved"),
        go_stdout.expect("go replace images for-slides saved"),
        "replace images for-slides saved stdout"
    );
    assert!(go_out.exists(), "Go for-slides output missing");
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

    let go_dir = temp_dir.join("go-for-slides-extract");
    let rust_dir = temp_dir.join("rust-for-slides-extract");
    let go_dir_str = go_dir.to_str().expect("go for-slides extract dir");
    let rust_dir_str = rust_dir.to_str().expect("rust for-slides extract dir");
    let (go_extract_code, go_extract_stdout, go_extract_stderr) = run_go_ooxml(&[
        "--json", "pptx", "extract", "images", go_out_str, "--out", go_dir_str,
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
        rust_extract_code, go_extract_code,
        "for-slides image readback exit"
    );
    assert_eq!(
        rust_extract_stderr, go_extract_stderr,
        "for-slides image readback stderr"
    );
    assert_eq!(
        scrub_paths(
            rust_extract_stdout.expect("rust for-slides image readback"),
            &[(rust_out_str, "[PPTX]"), (rust_dir_str, "[OUT]")]
        ),
        scrub_paths(
            go_extract_stdout.expect("go for-slides image readback"),
            &[(go_out_str, "[PPTX]"), (go_dir_str, "[OUT]")]
        ),
        "for-slides image readback stdout"
    );
    assert_export_dirs_match(&go_dir, &rust_dir);

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
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "{name} exit");
        assert_eq!(rust_stdout, go_stdout, "{name} stdout");
        assert_eq!(rust_stderr, go_stderr, "{name} stderr");
    }
}


#[test]
fn pptx_replace_text_from_xlsx_matches_go_oracle_saved_dry_run_and_errors() {
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
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "missing file exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "missing file stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "missing file stderr for {args:?}");
    }

    let go_out = temp_dir.join("go-text-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-text-from-xlsx.pptx");
    let go_out_str = go_out.to_str().expect("go text-from-xlsx output path");
    let rust_out_str = rust_out.to_str().expect("rust text-from-xlsx output path");
    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "text-from-xlsx saved exit");
    assert_eq!(rust_stderr, go_stderr, "text-from-xlsx saved stderr");
    let rust_json = rust_stdout.expect("rust text-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go text-from-xlsx stdout"),
            go_out_str,
            "[OUT]"
        ),
        "text-from-xlsx saved stdout"
    );
    assert!(go_out.exists(), "Go text-from-xlsx output missing");
    assert!(rust_out.exists(), "Rust text-from-xlsx output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_read_code, go_read_stdout, go_read_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        go_out_str,
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
    assert_eq!(rust_read_code, go_read_code, "text-from readback exit");
    assert_eq!(
        rust_read_stderr, go_read_stderr,
        "text-from readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust text-from readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_read_stdout.expect("go text-from readback"),
            go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "text-from-xlsx dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "text-from-xlsx dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust text-from-xlsx dry-run stdout"),
        go_stdout.expect("go text-from-xlsx dry-run stdout"),
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&missing_target);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_target);
    assert_eq!(rust_code, go_code, "text-from missing target exit");
    assert_eq!(rust_stdout, go_stdout, "text-from missing target stdout");
    assert_eq!(rust_stderr, go_stderr, "text-from missing target stderr");
}


#[test]
fn pptx_replace_text_map_from_xlsx_matches_go_oracle_saved_dry_run_and_errors() {
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

    let go_out = temp_dir.join("go-text-map-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-text-map-from-xlsx.pptx");
    let go_out_str = go_out.to_str().expect("go text-map output path");
    let rust_out_str = rust_out.to_str().expect("rust text-map output path");
    let go_args = [
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
        go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "text-map-from-xlsx saved exit");
    assert_eq!(rust_stderr, go_stderr, "text-map-from-xlsx saved stderr");
    let rust_json = rust_stdout.expect("rust text-map-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go text-map-from-xlsx stdout"),
            go_out_str,
            "[OUT]"
        ),
        "text-map-from-xlsx saved stdout"
    );
    assert!(go_out.exists(), "Go text-map-from-xlsx output missing");
    assert!(rust_out.exists(), "Rust text-map-from-xlsx output missing");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_extract_code, go_extract_stdout, go_extract_stderr) =
        run_go_ooxml(&["--json", "pptx", "extract", "text", go_out_str]);
    let (rust_extract_code, rust_extract_stdout, rust_extract_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_extract_code, go_extract_code, "text-map extract exit");
    assert_eq!(
        rust_extract_stderr, go_extract_stderr,
        "text-map extract stderr"
    );
    assert_eq!(
        scrub_path(
            rust_extract_stdout.expect("rust text-map extract"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_extract_stdout.expect("go text-map extract"),
            go_out_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "text-map-from-xlsx dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "text-map-from-xlsx dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust text-map dry-run stdout"),
        go_stdout.expect("go text-map dry-run stdout"),
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
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "text-map error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "text-map error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "text-map error stderr for {args:?}");
    }
}

