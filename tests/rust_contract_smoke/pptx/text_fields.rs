#[test]
fn pptx_text_set_content_and_paragraph_file_contracts_are_actionable() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-text-content-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("text content temp dir");
    let paragraphs_file = temp_dir.join("paragraphs.json");
    std::fs::write(
        &paragraphs_file,
        r#"[{"text":"Lead","bold":true},{"text":"Detail","level":1,"bullet":true}]"#,
    )
    .expect("write paragraphs JSON");
    let output = temp_dir.join("content.pptx");
    let output_str = output.to_str().expect("text content output path");
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "text",
        "set",
        "testdata/pptx/multi-layout/presentation.pptx",
        "--slide",
        "2",
        "--target",
        "body",
        "--paragraphs-file",
        paragraphs_file.to_str().expect("paragraphs path"),
        "--out",
        output_str,
    ]);
    assert_eq!(code, 0, "text content exit");
    assert_eq!(stderr, None, "text content stderr");
    let result = stdout.expect("text content stdout");
    assert_eq!(result["mode"], "paragraph-content");
    assert_eq!(result["paragraphCount"], 2);
    assert_eq!(result["destination"]["paragraphs"][0]["text"], "Lead");
    assert_eq!(result["destination"]["paragraphs"][1]["level"], 1);
    assert_rust_emitted_ooxml_command_succeeds(&result, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&result, "validateCommand");
    assert_strict_validate_succeeds(output_str, "text content output");

    let (code, _, stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "text",
        "set",
        "testdata/pptx/multi-layout/presentation.pptx",
        "--slide",
        "2",
        "--target",
        "body",
        "--text",
        "plain",
        "--paragraphs-file",
        paragraphs_file.to_str().expect("paragraphs path"),
        "--dry-run",
    ]);
    assert_eq!(code, 2, "mutually exclusive content sources exit");
    assert!(
        stderr
            .as_ref()
            .is_some_and(|error| error["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("mutually exclusive"))),
        "mutual exclusion error must be actionable: {stderr:?}"
    );
}

#[test]
fn pptx_text_set_saved_readback_dry_run_hyperlink_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-text-set-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx text set temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let baseline_out = temp_dir.join("baseline-text-set.pptx");
    let rust_out = temp_dir.join("rust-text-set.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline text set path");
    let rust_out_str = rust_out.to_str().expect("rust text set path");

    let baseline_args = [
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--paragraph",
        "0",
        "--run-index",
        "0",
        "--bold",
        "--italic",
        "--font-size",
        "28",
        "--color",
        "ff0000",
        "--font-family",
        "Arial",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--paragraph",
        "0",
        "--run-index",
        "0",
        "--bold",
        "--italic",
        "--font-size",
        "28",
        "--color",
        "ff0000",
        "--font-family",
        "Arial",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "text set saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "text set saved stderr");
    let rust_json = rust_stdout.expect("rust text set stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline text set stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "text set saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline text set output missing"
    );
    assert!(rust_out.exists(), "Rust text set output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        baseline_out_str,
        "--slide",
        "2",
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
        "2",
        "--target",
        "title",
        "--include-text",
    ]);
    assert_eq!(rust_read_code, baseline_read_code, "text set readback exit");
    assert_eq!(
        rust_read_stderr, baseline_read_stderr,
        "text set readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust text set readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_read_stdout.expect("baseline text set readback"),
            baseline_out_str,
            "[OUT]"
        ),
        "text set readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--paragraph",
        "0",
        "--run-index",
        "0",
        "--underline",
        "single",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "text set dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "text set dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust text set dry-run"),
        baseline_stdout.expect("baseline text set dry-run"),
        "text set dry-run stdout"
    );

    let baseline_hyper = temp_dir.join("baseline-hyperlink.pptx");
    let rust_hyper = temp_dir.join("rust-hyperlink.pptx");
    let baseline_hyper_str = baseline_hyper.to_str().expect("baseline hyperlink path");
    let rust_hyper_str = rust_hyper.to_str().expect("rust hyperlink path");
    let baseline_hyper_args = [
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--paragraph",
        "0",
        "--run-index",
        "0",
        "--hyperlink",
        "https://example.com",
        "--out",
        baseline_hyper_str,
    ];
    let rust_hyper_args = [
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--paragraph",
        "0",
        "--run-index",
        "0",
        "--hyperlink",
        "https://example.com",
        "--out",
        rust_hyper_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_hyper_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_hyper_args);
    assert_eq!(rust_code, baseline_code, "text set hyperlink exit");
    assert_eq!(rust_stderr, baseline_stderr, "text set hyperlink stderr");
    let rust_hyper_json = rust_stdout.expect("rust hyperlink stdout");
    assert_eq!(
        scrub_path(rust_hyper_json.clone(), rust_hyper_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline hyperlink stdout"),
            baseline_hyper_str,
            "[OUT]"
        ),
        "text set hyperlink stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_hyper_json, "validateCommand");

    for (label, args) in [
        (
            "text set paragraph out of range",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "title",
                "--paragraph",
                "99",
                "--bold",
                "--dry-run",
            ],
        ),
        (
            "text set run index out of range",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "title",
                "--paragraph",
                "0",
                "--run-index",
                "99",
                "--bold",
                "--dry-run",
            ],
        ),
        (
            "text set invalid color",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "title",
                "--paragraph",
                "0",
                "--color",
                "ZZZZZZ",
                "--dry-run",
            ],
        ),
        (
            "text set mutually exclusive flags",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "title",
                "--paragraph",
                "0",
                "--bold",
                "--remove-bold",
                "--dry-run",
            ],
        ),
        (
            "text set no styling flags",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "title",
                "--paragraph",
                "0",
                "--dry-run",
            ],
        ),
        (
            "text set unknown target",
            vec![
                "--json",
                "pptx",
                "text",
                "set",
                fixture,
                "--slide",
                "2",
                "--target",
                "nonexistent",
                "--paragraph",
                "0",
                "--bold",
                "--dry-run",
            ],
        ),
    ] {
        assert_baseline_rust_json_match(&args, label);
    }
}

#[test]
fn pptx_fields_inspect_set_readback_dry_run_and_errors_match_rust_baseline() {
    let header_footer_fixture = "testdata/pptx/header-footer/presentation.pptx";
    let title_content_fixture = "testdata/pptx/title-content/presentation.pptx";

    assert_baseline_rust_json_match(
        &["--json", "pptx", "fields", "inspect", header_footer_fixture],
        "fields inspect header-footer",
    );
    assert_baseline_rust_json_match(
        &["--json", "pptx", "fields", "inspect", title_content_fixture],
        "fields inspect no-header-footer",
    );

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-fields-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx fields temp dir");

    let rust_out = temp_dir.join("rust-fields-set.pptx");
    let rust_out_str = rust_out.to_str().expect("rust fields set path");
    let rust_args = [
        "--json",
        "pptx",
        "fields",
        "set",
        header_footer_fixture,
        "--footer",
        "Confidential",
        "--show-slide-number=false",
        "--date-format",
        "date-only",
        "--out",
        rust_out_str,
    ];
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, 0, "fields set saved exit");
    assert_eq!(rust_stderr, None, "fields set saved stderr");
    let rust_json = rust_stdout.expect("rust fields set stdout");
    let scrubbed = scrub_path(rust_json.clone(), rust_out_str, "[OUT]");
    assert_eq!(scrubbed["output"], Value::String("[OUT]".to_string()));
    assert_eq!(
        scrubbed["footerText"],
        Value::String("Confidential".to_string())
    );
    assert_eq!(scrubbed["footerPlaceholdersUpdated"], Value::from(2));
    assert_eq!(scrubbed["footerPlaceholdersCreated"], Value::from(1));
    assert_eq!(scrubbed.get("slidesWithoutFooterPlaceholder"), None);
    assert!(rust_out.exists(), "Rust fields set output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (rust_read_code, rust_read_stdout, rust_read_stderr) =
        run_ooxml(&["--json", "pptx", "fields", "inspect", rust_out_str]);
    assert_eq!(rust_read_code, 0, "fields readback exit");
    assert_eq!(rust_read_stderr, None, "fields readback stderr");
    let readback = rust_read_stdout.expect("rust fields readback");
    let slides = readback["slides"]
        .as_array()
        .expect("fields readback slides");
    assert_eq!(slides.len(), 2, "header-footer fixture slide count");
    for slide in slides {
        assert_eq!(
            slide["footerPlaceholder"]["text"],
            Value::String("Confidential".to_string()),
            "fields readback footer text: {slide:?}"
        );
    }

    let (dry_code, dry_stdout, dry_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "fields",
        "set",
        header_footer_fixture,
        "--footer",
        "Confidential",
        "--show-slide-number=false",
        "--date-format",
        "date-only",
        "--dry-run",
    ]);
    assert_eq!(dry_code, 0, "fields set dry-run exit");
    assert_eq!(dry_stderr, None, "fields set dry-run stderr");
    let dry_result = dry_stdout.expect("fields set dry-run stdout");
    assert_eq!(dry_result["footerPlaceholdersUpdated"], Value::from(2));
    assert_eq!(dry_result["footerPlaceholdersCreated"], Value::from(1));
    assert_eq!(dry_result.get("slidesWithoutFooterPlaceholder"), None);

    for (label, args) in [
        (
            "fields set creates master hf dry-run",
            vec![
                "--json",
                "pptx",
                "fields",
                "set",
                title_content_fixture,
                "--show-footer=false",
                "--dry-run",
            ],
        ),
        (
            "fields set no flags",
            vec![
                "--json",
                "pptx",
                "fields",
                "set",
                header_footer_fixture,
                "--dry-run",
            ],
        ),
        (
            "fields set invalid date format",
            vec![
                "--json",
                "pptx",
                "fields",
                "set",
                header_footer_fixture,
                "--date-format",
                "bogus",
                "--dry-run",
            ],
        ),
        (
            "fields inspect unsupported xlsx",
            vec![
                "--json",
                "pptx",
                "fields",
                "inspect",
                "testdata/xlsx/minimal-workbook/workbook.xlsx",
            ],
        ),
        (
            "fields set unsupported xlsx",
            vec![
                "--json",
                "pptx",
                "fields",
                "set",
                "testdata/xlsx/minimal-workbook/workbook.xlsx",
                "--footer",
                "Confidential",
                "--dry-run",
            ],
        ),
    ] {
        assert_baseline_rust_json_match(&args, label);
    }
}

#[test]
fn pptx_fields_set_synthesizes_missing_footer_placeholders() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-footer-synthesis-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx footer synthesis temp dir");
    let out = temp_dir.join("footer-visible.pptx");
    let out_str = out.to_str().expect("footer output path");

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "fields",
        "set",
        "testdata/pptx/title-content/presentation.pptx",
        "--footer",
        "Confidential",
        "--show-footer=true",
        "--out",
        out_str,
    ]);
    assert_eq!(code, 0, "fields set footer synthesis exit");
    assert_eq!(stderr, None, "fields set footer synthesis stderr");
    let result = stdout.expect("fields set footer synthesis stdout");
    assert_eq!(result["footerPlaceholdersCreated"], Value::from(2));
    assert_eq!(result.get("slidesWithoutFooterPlaceholder"), None);
    assert_rust_emitted_ooxml_command_exits_zero(&result, "validateCommand");

    let slide_xml = read_zip_string(&out, "ppt/slides/slide1.xml");
    assert!(
        slide_xml.contains(r#"type="ftr""#),
        "synthesized slide XML should contain a footer placeholder: {slide_xml}"
    );
    assert!(
        slide_xml.contains("Confidential"),
        "synthesized slide XML should contain footer text: {slide_xml}"
    );

    let (inspect_code, inspect_stdout, inspect_stderr) =
        run_ooxml(&["--json", "pptx", "fields", "inspect", out_str]);
    assert_eq!(inspect_code, 0, "footer synthesis inspect exit");
    assert_eq!(inspect_stderr, None, "footer synthesis inspect stderr");
    let inspect = inspect_stdout.expect("footer synthesis inspect stdout");
    let slides = inspect["slides"].as_array().expect("inspect slides");
    assert_eq!(slides.len(), 2, "title-content fixture slide count");
    for slide in slides {
        assert_eq!(
            slide["footerPlaceholder"]["text"],
            Value::String("Confidential".to_string()),
            "slide footer placeholder should be inspectable: {slide:?}"
        );
    }
}
