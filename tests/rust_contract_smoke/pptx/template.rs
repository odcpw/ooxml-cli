#[test]
fn pptx_template_inspect_and_validate_layout_match_go_oracle() {
    assert_go_rust_json_match(
        &[
            "--json",
            "pptx",
            "template",
            "inspect",
            "testdata/pptx/template-branded/manifest.json",
        ],
        "pptx template inspect branded manifest",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "pptx",
            "template",
            "inspect",
            "testdata/pptx/template-branded/missing.json",
        ],
        "pptx template inspect missing manifest",
    );

    for fixture in [
        "testdata/pptx/minimal-title/presentation.pptx",
        "testdata/pptx/title-content/presentation.pptx",
        "testdata/pptx/table-slide/presentation.pptx",
        "testdata/pptx/layout-qa-dense-slide/presentation.pptx",
        "testdata/pptx/layout-qa-shape-collision/presentation.pptx",
        "testdata/pptx/layout-qa-text-overflow/presentation.pptx",
    ] {
        let args = ["--json", "pptx", "validate-layout", fixture];
        assert_go_rust_json_match(&args, fixture);
    }

    assert_go_rust_json_match(
        &[
            "--json",
            "pptx",
            "validate-layout",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        "pptx validate-layout rejects xlsx",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "pptx",
            "validate-layout",
            "testdata/pptx/missing-layout-qa.pptx",
        ],
        "pptx validate-layout missing file",
    );
}

#[test]
fn template_tokens_profiles_and_xlsx_binding_plan_match_go_oracle() {
    assert_go_rust_json_match(
        &[
            "--json",
            "template",
            "tokens",
            "testdata/pptx/theme-custom-colors/presentation.pptx",
        ],
        "template tokens pptx theme",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "template",
            "tokens",
            "testdata/pptx/chart-simple/presentation.pptx",
        ],
        "template tokens pptx charts",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "template",
            "tokens",
            "testdata/xlsx/chart-workbook/workbook.xlsx",
        ],
        "template tokens xlsx charts",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "template",
            "profile",
            "save",
            "testdata/pptx/theme-custom-colors/presentation.pptx",
            "--name",
            "Theme",
            "--description",
            "Demo",
        ],
        "template profile save stdout",
    );

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-template-bindings-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("template binding temp dir");
    let profile_path = temp_dir.join("profile.json");
    let profile_str = profile_path.to_string_lossy().to_string();
    let (save_code, save_stdout, save_stderr) = run_go_ooxml(&[
        "--json",
        "template",
        "profile",
        "save",
        "testdata/pptx/theme-custom-colors/presentation.pptx",
        "--name",
        "Theme",
        "--description",
        "Demo",
    ]);
    assert_eq!(save_code, 0, "go profile save for inspect fixture");
    assert_eq!(save_stderr, None, "go profile save stderr");
    fs::write(
        &profile_path,
        serde_json::to_vec_pretty(&save_stdout.expect("go profile save stdout")).unwrap(),
    )
    .expect("write profile fixture");
    assert_go_rust_json_match(
        &["--json", "template", "profile", "inspect", &profile_str],
        "template profile inspect",
    );

    let workbook = temp_dir.join("bindings.xlsx");
    write_xlsx_bindings_workbook(&workbook, "subtitle");
    let workbook_str = workbook.to_string_lossy().to_string();
    let plan_args = [
        "--json",
        "pptx",
        "xlsx-bindings",
        "plan",
        "testdata/pptx/title-content/presentation.pptx",
        "--workbook",
        &workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:P3",
    ];
    assert_go_rust_json_match(&plan_args, "pptx xlsx-bindings plan");

    let duplicate_workbook = temp_dir.join("duplicate-bindings.xlsx");
    write_xlsx_bindings_workbook(&duplicate_workbook, "title");
    let duplicate_workbook_str = duplicate_workbook.to_string_lossy().to_string();
    let duplicate_args = [
        "--json",
        "pptx",
        "xlsx-bindings",
        "plan",
        "testdata/pptx/title-content/presentation.pptx",
        "--workbook",
        &duplicate_workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:P3",
    ];
    assert_go_rust_json_match(&duplicate_args, "pptx xlsx-bindings duplicate target");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn template_parent_groups_are_help_only_like_go_oracle() {
    for args in [
        vec!["--json", "template"],
        vec!["--json", "template", "profile"],
        vec!["--json", "pptx", "template"],
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml_raw(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml_raw(&args);
        assert_eq!(rust_code, go_code, "exit code for {args:?}");
        assert_eq!(go_code, 0, "Go parent help exit for {args:?}");
        assert!(go_stderr.is_empty(), "Go parent help stderr for {args:?}");
        assert!(
            rust_stderr.is_empty(),
            "Rust parent help stderr for {args:?}: {rust_stderr}"
        );
        assert!(go_stdout.contains("Usage:"), "Go help text for {args:?}");
        assert!(
            rust_stdout.contains("Usage:"),
            "Rust help text for {args:?}: {rust_stdout}"
        );
    }
}

#[test]
fn template_apply_theme_tokens_dry_run_saved_ranges_and_errors_match_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-template-apply-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("template apply temp dir");
    let tokens_path = temp_dir.join("tokens.json");
    fs::write(&tokens_path, template_apply_tokens_json()).expect("write template tokens");
    let tokens_str = tokens_path.to_string_lossy().to_string();
    let target = "testdata/pptx/minimal-title/presentation.pptx";

    let dry_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_args);
    assert_eq!(rust_code, go_code, "template apply dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "template apply dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust dry-run stdout"),
            &tokens_str,
            "[TOKENS]"
        ),
        scrub_path(
            go_stdout.expect("go dry-run stdout"),
            &tokens_str,
            "[TOKENS]"
        ),
        "template apply dry-run stdout"
    );

    let go_out = temp_dir.join("go-applied.pptx");
    let rust_out = temp_dir.join("rust-applied.pptx");
    let go_out_str = go_out.to_string_lossy().to_string();
    let rust_out_str = rust_out.to_string_lossy().to_string();
    let go_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--out",
        &go_out_str,
    ];
    let rust_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--out",
        &rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "template apply saved exit");
    assert_eq!(rust_stderr, go_stderr, "template apply saved stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust saved stdout"),
            &[(&tokens_str, "[TOKENS]"), (&rust_out_str, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go saved stdout"),
            &[(&tokens_str, "[TOKENS]"), (&go_out_str, "[OUT]")]
        ),
        "template apply saved stdout"
    );
    assert!(rust_out.exists(), "Rust template apply output exists");
    assert_strict_validate_succeeds(&rust_out_str, "template apply output");
    let (_, applied_tokens, applied_tokens_stderr) =
        run_ooxml(&["--json", "template", "tokens", &rust_out_str]);
    assert_eq!(
        applied_tokens_stderr, None,
        "template tokens stderr for applied output"
    );
    let applied_tokens = applied_tokens.expect("applied output tokens");
    assert_eq!(
        applied_tokens["pptx"]["theme"]["colorScheme"]["accent1"],
        serde_json::json!("123456")
    );
    assert_eq!(
        applied_tokens["pptx"]["theme"]["fontScheme"]["majorFont"],
        serde_json::json!("Aptos Display")
    );

    let go_ranges_out = temp_dir.join("go-ranges.pptx");
    let rust_ranges_out = temp_dir.join("rust-ranges.pptx");
    let go_ranges_out_str = go_ranges_out.to_string_lossy().to_string();
    let rust_ranges_out_str = rust_ranges_out.to_string_lossy().to_string();
    let go_ranges_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--target-ranges",
        "--out",
        &go_ranges_out_str,
    ];
    let rust_ranges_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--target-ranges",
        "--out",
        &rust_ranges_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_ranges_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_ranges_args);
    assert_eq!(rust_code, go_code, "template apply ranges exit");
    assert_eq!(rust_stderr, go_stderr, "template apply ranges stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust ranges stdout"),
            &tokens_str,
            "[TOKENS]"
        ),
        scrub_path(
            go_stdout.expect("go ranges stdout"),
            &tokens_str,
            "[TOKENS]"
        ),
        "template apply ranges stdout"
    );
    assert!(
        !rust_ranges_out.exists(),
        "ranges-only apply should not produce an output file"
    );

    assert_go_rust_json_match(
        &["--json", "template", "apply", target],
        "template apply missing output mode",
    );
    assert_go_rust_json_match(
        &[
            "--json",
            "pptx",
            "template",
            "compile",
            "testdata/pptx/template-branded/manifest.json",
            "testdata/pptx/template-branded/spec-simple.yaml",
        ],
        "pptx template compile missing required flags",
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn template_apply_pptx_chart_and_text_style_targets_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-template-targets-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("template targets temp dir");

    let tokens_path = temp_dir.join("targets.json");
    fs::write(&tokens_path, template_apply_chart_text_tokens_json())
        .expect("write template target tokens");
    let tokens_str = tokens_path.to_string_lossy().to_string();

    let chart_target = "testdata/pptx/chart-simple/presentation.pptx";
    let go_chart_out = temp_dir.join("go-chart.pptx");
    let rust_chart_out = temp_dir.join("rust-chart.pptx");
    let go_chart_out_str = go_chart_out.to_string_lossy().to_string();
    let rust_chart_out_str = rust_chart_out.to_string_lossy().to_string();
    let go_chart_args = [
        "--json",
        "template",
        "apply",
        chart_target,
        "--tokens",
        &tokens_str,
        "--target-charts",
        "--out",
        &go_chart_out_str,
    ];
    let rust_chart_args = [
        "--json",
        "template",
        "apply",
        chart_target,
        "--tokens",
        &tokens_str,
        "--target-charts",
        "--out",
        &rust_chart_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_chart_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_chart_args);
    assert_eq!(rust_code, go_code, "template apply PPTX charts exit");
    assert_eq!(rust_stderr, go_stderr, "template apply PPTX charts stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust PPTX chart apply stdout"),
            &[(&tokens_str, "[TOKENS]"), (&rust_chart_out_str, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go PPTX chart apply stdout"),
            &[(&tokens_str, "[TOKENS]"), (&go_chart_out_str, "[OUT]")]
        ),
        "template apply PPTX charts stdout"
    );
    assert_strict_validate_succeeds(&rust_chart_out_str, "template apply PPTX chart output");
    assert_conformance_check_runs(&rust_chart_out_str, "template apply PPTX chart output");
    let (_, chart_show, chart_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "charts",
        "show",
        &rust_chart_out_str,
        "--chart",
        "chart:1",
    ]);
    assert_eq!(chart_show_stderr, None, "PPTX chart readback stderr");
    let chart_show = chart_show.expect("PPTX chart readback");
    assert_eq!(
        chart_show["charts"][0]["style"]["series"][0]["fillColor"],
        serde_json::json!("FF0000")
    );
    assert_eq!(
        chart_show["charts"][0]["style"]["series"][0]["lineColor"],
        serde_json::json!("00FF00")
    );

    let text_target = "testdata/pptx/minimal-title/presentation.pptx";
    let go_text_out = temp_dir.join("go-text-styles.pptx");
    let rust_text_out = temp_dir.join("rust-text-styles.pptx");
    let go_text_out_str = go_text_out.to_string_lossy().to_string();
    let rust_text_out_str = rust_text_out.to_string_lossy().to_string();
    let go_text_args = [
        "--json",
        "template",
        "apply",
        text_target,
        "--tokens",
        &tokens_str,
        "--target-text-styles",
        "--out",
        &go_text_out_str,
    ];
    let rust_text_args = [
        "--json",
        "template",
        "apply",
        text_target,
        "--tokens",
        &tokens_str,
        "--target-text-styles",
        "--out",
        &rust_text_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_text_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_text_args);
    assert_eq!(rust_code, go_code, "template apply text styles exit");
    assert_eq!(rust_stderr, go_stderr, "template apply text styles stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust text styles apply stdout"),
            &[(&tokens_str, "[TOKENS]"), (&rust_text_out_str, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go text styles apply stdout"),
            &[(&tokens_str, "[TOKENS]"), (&go_text_out_str, "[OUT]")]
        ),
        "template apply text styles stdout"
    );
    assert_strict_validate_succeeds(&rust_text_out_str, "template apply text style output");
    let (_, text_tokens, text_tokens_stderr) =
        run_ooxml(&["--json", "template", "tokens", &rust_text_out_str]);
    assert_eq!(text_tokens_stderr, None, "text style token readback stderr");
    let text_tokens = text_tokens.expect("text style token readback");
    let styles = text_tokens["pptx"]["defaultTextStyles"]
        .as_array()
        .expect("default text styles");
    assert!(
        styles.iter().any(|style| {
            style["role"] == serde_json::json!("title")
                && style["fontRef"] == serde_json::json!("major")
                && style["colorRef"] == serde_json::json!("accent1")
        }),
        "updated title style should read back"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn pptx_template_compile_simple_matches_go_oracle_and_validates() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-template-compile-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("template compile temp dir");
    let go_out = temp_dir.join("go-compiled.pptx");
    let rust_out = temp_dir.join("rust-compiled.pptx");
    let go_out_str = go_out.to_string_lossy().to_string();
    let rust_out_str = rust_out.to_string_lossy().to_string();
    let go_args = [
        "--json",
        "pptx",
        "template",
        "compile",
        "testdata/pptx/template-branded/manifest.json",
        "testdata/pptx/template-branded/spec-simple.yaml",
        "--archetype",
        "testdata/pptx/template-branded/presentation.pptx",
        "--out",
        &go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "template",
        "compile",
        "testdata/pptx/template-branded/manifest.json",
        "testdata/pptx/template-branded/spec-simple.yaml",
        "--archetype",
        "testdata/pptx/template-branded/presentation.pptx",
        "--out",
        &rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "pptx template compile exit");
    assert_eq!(rust_stderr, go_stderr, "pptx template compile stderr");
    assert_eq!(
        scrub_template_compile_result(rust_stdout.expect("rust compile stdout"), &rust_out_str),
        scrub_template_compile_result(go_stdout.expect("go compile stdout"), &go_out_str),
        "pptx template compile stdout"
    );
    assert!(rust_out.exists(), "Rust compiled PPTX exists");
    assert_strict_validate_succeeds(&rust_out_str, "compiled PPTX");

    let (go_text_code, go_text, go_text_stderr) =
        run_go_ooxml(&["--json", "pptx", "extract", "text", &go_out_str]);
    let (rust_text_code, rust_text, rust_text_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", &rust_out_str]);
    assert_eq!(rust_text_code, go_text_code, "compiled text readback exit");
    assert_eq!(
        rust_text_stderr, go_text_stderr,
        "compiled text readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_text.expect("rust compiled text"),
            &rust_out_str,
            "[OUT]"
        ),
        scrub_path(go_text.expect("go compiled text"), &go_out_str, "[OUT]"),
        "compiled PPTX text readback"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

fn template_apply_tokens_json() -> &'static str {
    r#"{
  "schemaVersion": "1.0",
  "type": "pptx",
  "source": "probe-tokens",
  "pptx": {
    "theme": {
      "colorScheme": {
        "dark1": "101010",
        "light1": "FAFAFA",
        "dark2": "202020",
        "light2": "EEEEEE",
        "accent1": "123456",
        "accent2": "234567",
        "accent3": "345678",
        "accent4": "456789",
        "accent5": "56789A",
        "accent6": "6789AB",
        "hypLink": "789ABC",
        "folLink": "89ABCD"
      },
      "fontScheme": {
        "majorFont": "Aptos Display",
        "minorFont": "Aptos"
      }
    }
  }
}
"#
}

fn template_apply_chart_text_tokens_json() -> &'static str {
    r#"{
  "schemaVersion": "1.0",
  "type": "pptx",
  "source": "template-targets",
  "pptx": {
    "theme": null,
    "defaultTextStyles": [
      {
        "masterRef": "/template/master.xml",
        "role": "title",
        "fontRef": "major",
        "sizePt": 31,
        "colorRef": "accent1"
      },
      {
        "masterRef": "/template/master.xml",
        "role": "body",
        "fontRef": "minor",
        "sizePt": 19,
        "color": "FF00AA"
      }
    ],
    "tableStyles": [],
    "chartStyles": [
      {
        "partUri": "/template/chart.xml",
        "seriesFillColor": "FF0000",
        "seriesLineColor": "00FF00"
      }
    ]
  },
  "xlsx": {
    "theme": null,
    "namedCellStyles": [],
    "chartStyles": [
      {
        "partUri": "/template/chart.xml",
        "seriesFillColor": "FF0000",
        "seriesLineColor": "00FF00"
      }
    ]
  }
}
"#
}

fn scrub_template_compile_result(value: Value, output_path: &str) -> Value {
    let mut value = scrub_path(value, output_path, "[OUT]");
    if let Value::Object(map) = &mut value {
        for key in ["startedAt", "completedAt", "duration"] {
            if map.contains_key(key) {
                map.insert(key.to_string(), Value::String(format!("[{key}]")));
            }
        }
    }
    value
}

#[test]
fn pptx_template_capture_produces_manifest_for_readable_deck() {
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "template",
        "capture",
        "testdata/pptx/title-content/presentation.pptx",
        "--name",
        "Captured",
        "--slides",
        "1",
        "--version",
        "1.2.3",
    ]);
    assert_eq!(code, 0, "capture exit");
    assert_eq!(stderr, None, "capture stderr");
    let manifest = stdout.expect("capture manifest");
    assert_eq!(
        manifest["manifestVersion"],
        Value::String("1.0.0".to_string())
    );
    assert_eq!(manifest["name"], Value::String("Captured".to_string()));
    assert_eq!(manifest["version"]["major"], Value::from(1));
    assert_eq!(manifest["version"]["minor"], Value::from(2));
    assert_eq!(manifest["version"]["patch"], Value::from(3));
    let archetypes = manifest["archetypes"].as_array().expect("archetypes");
    assert_eq!(archetypes.len(), 1);
    assert_eq!(archetypes[0]["sourceSlideNumber"], Value::from(1));
    assert!(
        archetypes[0]["slots"]
            .as_array()
            .expect("slots")
            .iter()
            .any(|slot| slot["id"] == Value::String("title".to_string()))
    );
}
