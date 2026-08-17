fn scrub_translation_exported_at(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            for (key, item) in map.iter_mut() {
                if key == "exportedAt" && item.as_str().is_some() {
                    *item = Value::String("[EXPORTED_AT]".to_string());
                } else {
                    *item = scrub_translation_exported_at(item.take());
                }
            }
            Value::Object(map)
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(scrub_translation_exported_at)
                .collect(),
        ),
        other => other,
    }
}
#[test]
fn pptx_translate_export_matches_rust_baseline() {
    for args in [
        vec![
            "--json",
            "pptx",
            "translate",
            "export",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--source-lang",
            "en-US",
            "--target-lang",
            "fr-FR",
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "export",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--include-notes",
            "--source-lang",
            "en-US",
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "export",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--slide",
            "99",
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "export",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--source-lang",
            "xx_BAD",
            "--target-lang",
            "??",
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "export",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ] {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(
            rust_code, baseline_code,
            "translate export exit for {args:?}"
        );
        assert_eq!(
            rust_stderr, baseline_stderr,
            "translate export stderr for {args:?}"
        );
        assert_eq!(
            rust_stdout.map(scrub_translation_exported_at),
            baseline_stdout.map(scrub_translation_exported_at),
            "translate export stdout for {args:?}"
        );
    }
}

#[test]
fn pptx_translate_apply_saved_stale_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-translate-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("translate temp dir");

    let baseline_input = temp_dir.join("baseline-input.pptx");
    let rust_input = temp_dir.join("rust-input.pptx");
    let xlsx_input = temp_dir.join("input.xlsx");
    std::fs::copy(
        "testdata/pptx/minimal-title/presentation.pptx",
        &baseline_input,
    )
    .expect("copy Rust baseline translate fixture");
    std::fs::copy("testdata/pptx/minimal-title/presentation.pptx", &rust_input)
        .expect("copy Rust translate fixture");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &xlsx_input)
        .expect("copy xlsx translate fixture");

    let manifest_path = temp_dir.join("manifest.json");
    let stale_manifest_path = temp_dir.join("stale.json");
    let invalid_manifest_path = temp_dir.join("invalid-id.json");
    std::fs::write(
        &manifest_path,
        r#"{"metadata":{"version":"1.0.0","exportedAt":"2026-06-20T00:00:00Z","sourceLanguage":"en-US","targetLanguage":"fr-FR","deckName":"presentation.pptx","slideCount":1,"entryCount":1},"entries":[{"id":"slide:0_title_p0_r0","type":"title","sourceText":"Minimal Title Slide","targetText":"Titre minimal","slideId":0,"slideNumber":1,"placeholderKey":"title","shapeId":2,"shapeName":"Title 1","paragraphIndex":0,"runIndex":0,"segmentType":"text"}]}"#,
    )
    .expect("write translate manifest");
    std::fs::write(
        &stale_manifest_path,
        r#"{"metadata":{"version":"1.0.0","exportedAt":"2026-06-20T00:00:00Z","slideCount":1,"entryCount":1},"entries":[{"id":"slide:0_title_p0_r0","type":"title","sourceText":"Old source","targetText":"Titre stale","slideId":0,"slideNumber":1,"placeholderKey":"title","shapeId":2,"shapeName":"Title 1","paragraphIndex":0,"runIndex":0,"segmentType":"text"}]}"#,
    )
    .expect("write stale translate manifest");
    std::fs::write(
        &invalid_manifest_path,
        r#"{"metadata":{"version":"1.0.0","exportedAt":"2026-06-20T00:00:00Z"},"entries":[{"id":"bad","type":"title","sourceText":"Minimal Title Slide","targetText":"Titre","slideId":0,"slideNumber":1,"paragraphIndex":0,"runIndex":0}]}"#,
    )
    .expect("write invalid translate manifest");

    let baseline_out = temp_dir.join("baseline-out.pptx");
    let rust_out = temp_dir.join("rust-out.pptx");
    let baseline_input_str = baseline_input.to_str().expect("baseline input");
    let rust_input_str = rust_input.to_str().expect("rust input");
    let xlsx_input_str = xlsx_input.to_str().expect("xlsx input");
    let manifest_str = manifest_path.to_str().expect("manifest path");
    let stale_manifest_str = stale_manifest_path.to_str().expect("stale manifest path");
    let invalid_manifest_str = invalid_manifest_path
        .to_str()
        .expect("invalid manifest path");
    let baseline_out_str = baseline_out.to_str().expect("baseline output");
    let rust_out_str = rust_out.to_str().expect("rust output");

    let baseline_args = [
        "--json",
        "pptx",
        "translate",
        "apply",
        baseline_input_str,
        manifest_str,
        "--output",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "translate",
        "apply",
        rust_input_str,
        manifest_str,
        "--output",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "translate apply saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "translate apply saved stderr");
    assert_eq!(rust_stdout, baseline_stdout, "translate apply saved stdout");
    assert!(
        baseline_out.exists(),
        "Rust baseline translate output missing"
    );
    assert!(rust_out.exists(), "Rust translate output missing");

    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&["--json", "pptx", "extract", "text", baseline_out_str]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_code, baseline_code, "translate apply readback exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "translate apply readback stderr"
    );
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust translate readback"),
            &[(rust_out_str, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline translate readback"),
            &[(baseline_out_str, "[OUT]")]
        ),
        "translate apply readback stdout"
    );

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_out_str]);
    assert_eq!(validate_code, 0, "translate output strict validate exit");
    assert_eq!(
        validate_stderr, None,
        "translate output strict validate stderr"
    );
    assert_eq!(
        validate_stdout.expect("translate output strict validate")["valid"],
        Value::Bool(true)
    );

    for stale_mode in [None, Some("warn"), Some("error")] {
        let stale_label = stale_mode.unwrap_or("skip");
        let rust_stale_out = temp_dir.join(format!("rust-stale-{stale_label}.pptx"));
        let baseline_stale_out = temp_dir.join(format!("baseline-stale-{stale_label}.pptx"));
        let rust_stale_out_str = rust_stale_out.to_str().expect("rust stale output");
        let baseline_stale_out_str = baseline_stale_out.to_str().expect("baseline stale output");
        let mut baseline_args = vec![
            "--json",
            "pptx",
            "translate",
            "apply",
            baseline_input_str,
            stale_manifest_str,
        ];
        let mut rust_args = vec![
            "--json",
            "pptx",
            "translate",
            "apply",
            rust_input_str,
            stale_manifest_str,
        ];
        if let Some(mode) = stale_mode {
            baseline_args.extend(["--stale", mode]);
            rust_args.extend(["--stale", mode]);
        }
        baseline_args.extend(["--output", baseline_stale_out_str]);
        rust_args.extend(["--output", rust_stale_out_str]);

        let (baseline_code, baseline_stdout, baseline_stderr) =
            run_ooxml_baseline_raw(&baseline_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml_raw(&rust_args);
        assert_eq!(
            rust_code, baseline_code,
            "translate stale {stale_mode:?} exit"
        );
        assert_eq!(
            rust_stderr, baseline_stderr,
            "translate stale {stale_mode:?} stderr"
        );
        if baseline_stdout.trim().is_empty() || rust_stdout.trim().is_empty() {
            assert_eq!(
                rust_stdout, baseline_stdout,
                "translate stale {stale_mode:?} stdout"
            );
        } else {
            assert_eq!(
                parse_raw_json(&rust_stdout),
                parse_raw_json(&baseline_stdout),
                "translate stale {stale_mode:?} stdout"
            );
        }
    }

    for args in [
        vec![
            "--json",
            "pptx",
            "translate",
            "apply",
            rust_input_str,
            manifest_str,
            "--stale",
            "explode",
            "--output",
            rust_out_str,
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "apply",
            rust_input_str,
            invalid_manifest_str,
            "--output",
            rust_out_str,
        ],
        vec![
            "--json",
            "pptx",
            "translate",
            "apply",
            xlsx_input_str,
            manifest_str,
            "--output",
            rust_out_str,
        ],
    ] {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(
            rust_code, baseline_code,
            "translate apply error exit for {args:?}"
        );
        assert_eq!(
            rust_stderr, baseline_stderr,
            "translate apply error stderr for {args:?}"
        );
        assert_eq!(
            rust_stdout, baseline_stdout,
            "translate apply error stdout for {args:?}"
        );
    }
}
