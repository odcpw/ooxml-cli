#[test]
fn pptx_charts_list_show_json_and_errors_match_rust_baseline() {
    let fixture = "testdata/pptx/chart-simple/presentation.pptx";
    let list_args = ["--json", "pptx", "charts", "list", fixture];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&list_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&list_args);
    assert_eq!(rust_code, baseline_code, "charts list exit");
    assert_eq!(rust_stderr, baseline_stderr, "charts list stderr");
    let rust_list = rust_stdout.expect("rust charts list stdout");
    assert_eq!(
        rust_list,
        baseline_stdout.expect("baseline charts list stdout"),
        "charts list stdout"
    );
    assert_eq!(rust_list["charts"].as_array().map(Vec::len), Some(2));
    assert_eq!(rust_list["charts"][0]["primarySelector"], "chart:1");
    assert_eq!(
        rust_list["charts"][0]["style"]["title"]["text"],
        "Revenue by Region"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_list, "validateCommand");

    let second_show = rust_list["charts"][1]["showCommand"]
        .as_str()
        .expect("second chart showCommand");
    let emitted_args = emitted_ooxml_args(second_show);
    let emitted_borrowed = emitted_args.iter().map(String::as_str).collect::<Vec<_>>();
    let (show_code, show_stdout, show_stderr) = run_ooxml(&emitted_borrowed);
    assert_eq!(show_code, 0, "generated second chart show exit");
    assert_eq!(show_stderr, None, "generated second chart show stderr");
    let show_json = show_stdout.expect("generated second chart show stdout");
    assert_eq!(show_json["charts"].as_array().map(Vec::len), Some(1));
    assert_eq!(show_json["charts"][0]["partUri"], "/ppt/charts/chart2.xml");

    for args in [
        vec!["--json", "pptx", "charts", "list", fixture, "--slide", "2"],
        vec![
            "--json",
            "pptx",
            "charts",
            "show",
            fixture,
            "--slide",
            "1",
            "--chart",
            "Revenue Chart",
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "show",
            fixture,
            "--chart",
            "part:/ppt/charts/chart2.xml",
        ],
    ] {
        let borrowed = args.to_vec();
        assert_rust_baseline_match(&borrowed);
    }

    for args in [
        vec!["--json", "pptx", "charts", "show", fixture],
        vec![
            "--json", "pptx", "charts", "show", fixture, "--chart", "missing",
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "show",
            "testdata/pptx/minimal-title/presentation.pptx",
        ],
    ] {
        let borrowed = args.to_vec();
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&borrowed);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&borrowed);
        assert_eq!(rust_code, baseline_code, "charts error exit for {borrowed:?}");
        assert_eq!(
            rust_stdout, baseline_stdout,
            "charts error stdout for {borrowed:?}"
        );
        assert_eq!(
            rust_stderr, baseline_stderr,
            "charts error stderr for {borrowed:?}"
        );
    }
}

#[test]
fn pptx_charts_create_inline_saved_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-chart-create-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx chart create temp dir");

    let fixture = "testdata/pptx/multi-layout/presentation.pptx";
    let values = r#"[["","North","South"],["Q1",10,20],["Q2",15,25]]"#;
    let dry_run_args = [
        "--json",
        "pptx",
        "charts",
        "create",
        fixture,
        "--slide",
        "1",
        "--type",
        "bar",
        "--title",
        "Quarterly Revenue",
        "--values-json",
        values,
        "--dry-run",
    ];
    assert_rust_baseline_match(&dry_run_args);

    let baseline_out = temp_dir.join("baseline-create-inline.pptx");
    let rust_out = temp_dir.join("rust-create-inline.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline chart create output path");
    let rust_out_str = rust_out.to_str().expect("rust chart create output path");
    let baseline_args = [
        "--json",
        "pptx",
        "charts",
        "create",
        fixture,
        "--slide",
        "1",
        "--type",
        "bar",
        "--title",
        "Quarterly Revenue",
        "--values-json",
        values,
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "charts",
        "create",
        fixture,
        "--slide",
        "1",
        "--type",
        "bar",
        "--title",
        "Quarterly Revenue",
        "--values-json",
        values,
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "chart create exit");
    assert_eq!(rust_stderr, baseline_stderr, "chart create stderr");
    let rust_json = rust_stdout.expect("rust chart create stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline chart create stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "chart create stdout"
    );
    assert!(baseline_out.exists(), "Rust baseline chart create output missing");
    assert!(rust_out.exists(), "Rust chart create output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "chartShowCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let extlst_input = temp_dir.join("chart-create-extlst-input.pptx");
    rewrite_zip_fixture(fixture, &extlst_input, |name, data| {
        if name == "ppt/slides/slide1.xml" {
            Some((
                name.to_string(),
                replace_ascii(
                    data,
                    "</p:spTree>",
                    r#"<p:extLst><p:ext uri="{BB962C8B-B14F-4D97-AF65-F5344CB8AC3E}"><p14:creationId xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" val="{11111111-1111-1111-1111-111111111111}"/></p:ext></p:extLst></p:spTree>"#,
                ),
            ))
        } else {
            Some((name.to_string(), data))
        }
    });
    let extlst_output = temp_dir.join("rust-create-before-extlst.pptx");
    let extlst_input_str = extlst_input.to_str().expect("extLst input path");
    let extlst_output_str = extlst_output.to_str().expect("extLst output path");
    let (extlst_code, extlst_stdout, extlst_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "charts",
        "create",
        extlst_input_str,
        "--slide",
        "1",
        "--type",
        "bar",
        "--title",
        "ExtLst Chart",
        "--values-json",
        values,
        "--out",
        extlst_output_str,
    ]);
    assert_eq!(extlst_code, 0, "chart create before extLst exit");
    assert_eq!(extlst_stderr, None, "chart create before extLst stderr");
    let extlst_json = extlst_stdout.expect("chart create before extLst stdout");
    assert_rust_emitted_ooxml_command_exits_zero(&extlst_json, "validateCommand");
    let extlst_slide_xml = read_zip_string(&extlst_output, "ppt/slides/slide1.xml");
    let chart_pos = extlst_slide_xml
        .find("<p:graphicFrame")
        .expect("chart graphicFrame in slide XML");
    let extlst_pos = extlst_slide_xml
        .find("<p:extLst")
        .expect("spTree extLst in slide XML");
    assert!(
        chart_pos < extlst_pos,
        "chart graphicFrame must be inserted before p:spTree p:extLst for Open XML SDK schema order"
    );

    for args in [
        vec![
            "--json",
            "pptx",
            "charts",
            "create",
            fixture,
            "--slide",
            "1",
            "--type",
            "bar",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "create",
            fixture,
            "--slide",
            "1",
            "--type",
            "doughnut",
            "--values-json",
            values,
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "create",
            fixture,
            "--slide",
            "99",
            "--type",
            "bar",
            "--values-json",
            values,
            "--dry-run",
        ],
    ] {
        assert_rust_baseline_match(&args);
    }
}

#[test]
fn pptx_charts_update_data_saved_dry_run_and_guards_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-chart-update-data-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx chart update-data temp dir");

    let fixture = "testdata/pptx/chart-simple/presentation.pptx";
    let values = r#"["12","24","36"]"#;
    let categories = r#"["East","West","Central"]"#;
    let dry_run_args = [
        "--json",
        "pptx",
        "charts",
        "update-data",
        fixture,
        "--slide",
        "1",
        "--chart",
        "chart:1",
        "--series",
        "1",
        "--values-json",
        values,
        "--categories-json",
        categories,
        "--dry-run",
    ];
    assert_rust_baseline_match(&dry_run_args);

    let baseline_out = temp_dir.join("baseline-update-data.pptx");
    let rust_out = temp_dir.join("rust-update-data.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline chart update output path");
    let rust_out_str = rust_out.to_str().expect("rust chart update output path");
    let baseline_args = [
        "--json",
        "pptx",
        "charts",
        "update-data",
        fixture,
        "--slide",
        "1",
        "--chart",
        "chart:1",
        "--series",
        "1",
        "--values-json",
        values,
        "--categories-json",
        categories,
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "charts",
        "update-data",
        fixture,
        "--slide",
        "1",
        "--chart",
        "chart:1",
        "--series",
        "1",
        "--values-json",
        values,
        "--categories-json",
        categories,
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "chart update-data exit");
    assert_eq!(rust_stderr, baseline_stderr, "chart update-data stderr");
    let rust_json = rust_stdout.expect("rust chart update-data stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline chart update-data stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "chart update-data stdout"
    );
    assert!(baseline_out.exists(), "Rust baseline chart update-data output missing");
    assert!(rust_out.exists(), "Rust chart update-data output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "chartShowCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    for args in [
        vec![
            "--json",
            "pptx",
            "charts",
            "update-data",
            fixture,
            "--chart",
            "chart:1",
            "--series",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "update-data",
            fixture,
            "--chart",
            "chart:1",
            "--series",
            "1",
            "--values",
            "1,2,3",
            "--values-json",
            values,
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "update-data",
            fixture,
            "--chart",
            "chart:1",
            "--series",
            "1",
            "--values-json",
            values,
            "--expect-values-hash",
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        ],
    ] {
        assert_rust_baseline_match(&args);
    }
}

#[test]
fn pptx_chart_style_mutations_match_rust_baseline_and_validate() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-chart-style-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("chart style temp dir");

    assert_pptx_chart_saved_mutation_matches_rust_baseline(
        &temp_dir,
        "set-title",
        &[
            "--slide",
            "1",
            "--chart",
            "chart:1",
            "--title",
            "Rust Title",
            "--expect-title",
            "Revenue by Region",
            "--font-family",
            "Arial",
            "--font-size",
            "14",
            "--font-color",
            "#112233",
            "--font-bold",
        ],
    );
    assert_pptx_chart_saved_mutation_matches_rust_baseline(
        &temp_dir,
        "set-legend",
        &[
            "--slide",
            "1",
            "--chart",
            "chart:1",
            "--position",
            "bottom",
            "--overlay=false",
        ],
    );
    assert_pptx_chart_saved_mutation_matches_rust_baseline(
        &temp_dir,
        "set-plot-area-fill",
        &[
            "--slide",
            "1",
            "--chart",
            "chart:1",
            "--fill-color",
            "#F3F6FA",
        ],
    );
    assert_pptx_chart_saved_mutation_matches_rust_baseline(
        &temp_dir,
        "set-chart-area-fill",
        &["--slide", "1", "--chart", "chart:1", "--fill-color", "none"],
    );
    assert_pptx_chart_saved_mutation_matches_rust_baseline(
        &temp_dir,
        "set-series-style",
        &[
            "--slide",
            "1",
            "--chart",
            "chart:1",
            "--series",
            "1",
            "--fill-color",
            "#AA5500",
            "--line-color",
            "#111111",
            "--line-width-pt",
            "2.25",
            "--expect-series-count",
            "1",
        ],
    );
    assert_pptx_chart_saved_mutation_matches_rust_baseline(
        &temp_dir,
        "set-axis",
        &[
            "--slide",
            "1",
            "--chart",
            "chart:1",
            "--axis",
            "value",
            "--title",
            "Revenue",
            "--number-format",
            "#,##0",
            "--major-gridlines=false",
            "--expect-axis-count",
            "2",
        ],
    );
    assert_pptx_chart_saved_mutation_matches_rust_baseline(
        &temp_dir,
        "convert-type",
        &[
            "--slide",
            "1",
            "--chart",
            "chart:1",
            "--to",
            "line",
            "--expect-type",
            "column",
        ],
    );
    assert_pptx_chart_copy_style_matches_rust_baseline(&temp_dir);

    let fixture = "testdata/pptx/chart-simple/presentation.pptx";
    let dry_run_args = [
        "--json",
        "pptx",
        "charts",
        "set-title",
        fixture,
        "--slide",
        "1",
        "--chart",
        "chart:1",
        "--title",
        "Dry Run Title",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "chart title dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "chart title dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust chart title dry-run stdout"),
        baseline_stdout.expect("baseline chart title dry-run stdout"),
        "chart title dry-run stdout"
    );

    for args in [
        vec![
            "--json",
            "pptx",
            "charts",
            "convert-type",
            fixture,
            "--chart",
            "chart:1",
            "--out",
            "unused.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "convert-type",
            fixture,
            "--chart",
            "chart:1",
            "--to",
            "line",
            "--expect-type",
            "pie",
            "--out",
            "unused.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "set-axis",
            fixture,
            "--chart",
            "chart:1",
            "--axis",
            "value",
            "--out",
            "unused.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "set-axis",
            fixture,
            "--chart",
            "chart:1",
            "--axis",
            "category",
            "--major-unit",
            "10",
            "--out",
            "unused.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "copy-style",
            fixture,
            "--chart",
            "chart:1",
            "--to-chart",
            "chart:1",
            "--from",
            fixture,
            "--out",
            "unused.pptx",
        ],
    ] {
        let borrowed = args.to_vec();
        assert_rust_baseline_match(&borrowed);
    }
}
