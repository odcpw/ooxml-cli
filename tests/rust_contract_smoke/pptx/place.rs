#[test]
fn pptx_add_textbox_saved_readback_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-add-textbox-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("add textbox temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let dry_run_args = [
        "--json",
        "pptx",
        "add-textbox",
        fixture,
        "--slide",
        "1",
        "--text",
        "Agent text box",
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "2000000",
        "--cy",
        "500000",
        "--name",
        "Agent Box",
        "--font-size",
        "20",
        "--font",
        "Arial",
        "--bold",
        "--color",
        "FF0000",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "add-textbox dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "add-textbox dry-run stderr");
    assert_eq!(
        scrub_created_at(rust_stdout.expect("rust add-textbox dry-run")),
        scrub_created_at(baseline_stdout.expect("baseline add-textbox dry-run")),
        "add-textbox dry-run stdout"
    );

    let baseline_out = temp_dir.join("baseline-add-textbox.pptx");
    let rust_out = temp_dir.join("rust-add-textbox.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline add-textbox output");
    let rust_out_str = rust_out.to_str().expect("rust add-textbox output");
    let baseline_args = [
        "--json",
        "pptx",
        "add-textbox",
        fixture,
        "--slide",
        "1",
        "--text",
        "Agent text box",
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "2000000",
        "--cy",
        "500000",
        "--name",
        "Agent Box",
        "--font-size",
        "20",
        "--font",
        "Arial",
        "--bold",
        "--color",
        "FF0000",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "add-textbox",
        fixture,
        "--slide",
        "1",
        "--text",
        "Agent text box",
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "2000000",
        "--cy",
        "500000",
        "--name",
        "Agent Box",
        "--font-size",
        "20",
        "--font",
        "Arial",
        "--bold",
        "--color",
        "FF0000",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "add-textbox saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "add-textbox saved stderr");
    let rust_json = rust_stdout.expect("rust add-textbox saved");
    assert_eq!(
        scrub_created_at(scrub_path(rust_json.clone(), rust_out_str, "[OUT]")),
        scrub_created_at(scrub_path(
            baseline_stdout.expect("baseline add-textbox saved"),
            baseline_out_str,
            "[OUT]"
        )),
        "add-textbox saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline add-textbox output missing"
    );
    assert!(rust_out.exists(), "Rust add-textbox output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    for args in [
        vec![
            "--json",
            "pptx",
            "add-textbox",
            fixture,
            "--slide",
            "1",
            "--cx",
            "2000000",
            "--cy",
            "500000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "add-textbox",
            fixture,
            "--slide",
            "1",
            "--text",
            "Bad dimensions",
            "--cx",
            "0",
            "--cy",
            "500000",
            "--dry-run",
        ],
    ] {
        assert_baseline_rust_json_match(&args, "add-textbox representative error");
    }
}

#[test]
fn pptx_place_image_saved_readback_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-place-image-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("place image temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let image = "testdata/test_image.png";
    let dry_run_args = [
        "--json",
        "pptx",
        "place",
        "image",
        fixture,
        "--slide",
        "1",
        "--image",
        image,
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "1000000",
        "--cy",
        "700000",
        "--name",
        "Agent Image",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "place image dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "place image dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust place image dry-run"),
        baseline_stdout.expect("baseline place image dry-run"),
        "place image dry-run stdout"
    );

    let baseline_out = temp_dir.join("baseline-place-image.pptx");
    let rust_out = temp_dir.join("rust-place-image.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline place image output");
    let rust_out_str = rust_out.to_str().expect("rust place image output");
    let baseline_args = [
        "--json",
        "pptx",
        "place",
        "image",
        fixture,
        "--slide",
        "1",
        "--image",
        image,
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "1000000",
        "--cy",
        "700000",
        "--name",
        "Agent Image",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "place",
        "image",
        fixture,
        "--slide",
        "1",
        "--image",
        image,
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "1000000",
        "--cy",
        "700000",
        "--name",
        "Agent Image",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "place image saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "place image saved stderr");
    let rust_json = rust_stdout.expect("rust place image saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline place image saved"),
            baseline_out_str,
            "[OUT]"
        ),
        "place image saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline place image output missing"
    );
    assert!(rust_out.exists(), "Rust place image output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    for args in [
        vec![
            "--json",
            "pptx",
            "place",
            "image",
            fixture,
            "--slide",
            "1",
            "--image",
            "testdata/missing.png",
            "--x",
            "0",
            "--y",
            "0",
            "--cx",
            "1",
            "--cy",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "place",
            "image",
            fixture,
            "--slide",
            "1",
            "--image",
            image,
            "--x",
            "0",
            "--y",
            "0",
            "--cx",
            "1",
            "--cy",
            "1",
            "--fit-mode",
            "stretch",
            "--dry-run",
        ],
    ] {
        assert_baseline_rust_json_match(&args, "place image representative error");
    }
}

#[test]
fn pptx_place_table_saved_dry_run_readback_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-place-table-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("place table temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let csv = temp_dir.join("data.csv");
    fs::write(&csv, "Region,Amount\nNorth,42").expect("write csv table data");
    let csv_str = csv.to_str().expect("csv data path");
    let json_data = temp_dir.join("data.json");
    fs::write(&json_data, r#"[["Region","Amount"],["South",55]]"#).expect("write json table data");
    let json_data_str = json_data.to_str().expect("json data path");

    let baseline_out = temp_dir.join("baseline-place-table.pptx");
    let rust_out = temp_dir.join("rust-place-table.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline place table output");
    let rust_out_str = rust_out.to_str().expect("rust place table output");
    let baseline_args = [
        "--json",
        "pptx",
        "place",
        "table",
        fixture,
        "--slide",
        "1",
        "--data",
        csv_str,
        "--format",
        "csv",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "3000000",
        "--header",
        "--name",
        "Revenue Table",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "place",
        "table",
        fixture,
        "--slide",
        "1",
        "--data",
        csv_str,
        "--format",
        "csv",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "3000000",
        "--header",
        "--name",
        "Revenue Table",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "place table saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "place table saved stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust place table stdout"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline place table stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "place table saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline place table output missing"
    );
    assert!(rust_out.exists(), "Rust place table output missing");

    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "tables",
        "show",
        baseline_out_str,
        "--slide",
        "1",
        "--target",
        "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_out_str,
        "--slide",
        "1",
        "--target",
        "table:1",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "place table readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "place table readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust place table readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_show_stdout.expect("baseline place table readback"),
            baseline_out_str,
            "[OUT]"
        ),
        "place table readback stdout"
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_out_str]);
    assert_eq!(validate_code, 0, "place table validate exit");
    assert_eq!(validate_stderr, None, "place table validate stderr");

    let dry_run_args = [
        "--json",
        "pptx",
        "place",
        "table",
        fixture,
        "--slide",
        "1",
        "--data",
        json_data_str,
        "--format",
        "json",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "2000000",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "place table dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "place table dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust place table dry-run"),
        baseline_stdout.expect("baseline place table dry-run"),
        "place table dry-run stdout"
    );

    for args in [
        vec![
            "--json",
            "pptx",
            "place",
            "table",
            fixture,
            "--slide",
            "1",
            "--data",
            csv_str,
            "--format",
            "tsv",
            "--cx",
            "1000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "place",
            "table",
            fixture,
            "--slide",
            "1",
            "--data",
            csv_str,
            "--cx",
            "0",
            "--dry-run",
        ],
    ] {
        assert_baseline_rust_json_match(&args, "place table representative error");
    }
}

#[test]
fn pptx_place_table_from_xlsx_saved_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-place-table-xlsx-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("place table xlsx temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let workbook = temp_dir.join("source-range.xlsx");
    write_simple_xlsx_with_sheet_xml(&workbook, pptx_update_source_sheet_xml_4x4());
    let workbook_str = workbook.to_str().expect("source workbook path");
    let table_workbook = temp_dir.join("source-table.xlsx");
    write_pptx_update_table_xlsx(&table_workbook);
    let table_workbook_str = table_workbook.to_str().expect("source table workbook path");

    let baseline_out = temp_dir.join("baseline-place-table-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-place-table-from-xlsx.pptx");
    let baseline_out_str = baseline_out
        .to_str()
        .expect("baseline place table xlsx output");
    let rust_out_str = rust_out.to_str().expect("rust place table xlsx output");
    let baseline_args = [
        "--json",
        "pptx",
        "place",
        "table-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--formula-mode",
        "formula",
        "--expect-source-range",
        "A1:B2",
        "--slide",
        "1",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "3000000",
        "--header",
        "--name",
        "Revenue Table",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "place",
        "table-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--formula-mode",
        "formula",
        "--expect-source-range",
        "A1:B2",
        "--slide",
        "1",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "3000000",
        "--header",
        "--name",
        "Revenue Table",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "place table-from-xlsx saved exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "place table-from-xlsx saved stderr"
    );
    let rust_json = rust_stdout.expect("rust place table-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline place table-from-xlsx stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "place table-from-xlsx saved stdout"
    );
    assert_eq!(rust_json["destination"]["cells"][0][0], "=SUM(B1:C1)");
    assert!(
        baseline_out.exists(),
        "Rust baseline place table-from-xlsx output missing"
    );
    assert!(
        rust_out.exists(),
        "Rust place table-from-xlsx output missing"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let dry_run_args = [
        "--json",
        "pptx",
        "place",
        "table-from-xlsx",
        fixture,
        "--workbook",
        table_workbook_str,
        "--table",
        "Sales",
        "--expect-source-range",
        "A1:C3",
        "--slide",
        "1",
        "--x",
        "0",
        "--y",
        "0",
        "--cx",
        "3000000",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(
        rust_code, baseline_code,
        "place table-from-xlsx dry-run exit"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "place table-from-xlsx dry-run stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust place table-from-xlsx dry-run"),
        baseline_stdout.expect("baseline place table-from-xlsx dry-run"),
        "place table-from-xlsx dry-run stdout"
    );

    let bad_out = temp_dir.join("bad.pptx");
    let bad_out_str = bad_out.to_str().expect("bad output path");
    for args in [
        vec![
            "--json",
            "pptx",
            "place",
            "table-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--range",
            "A1",
            "--slide",
            "1",
            "--cx",
            "1000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "place",
            "table-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:B2",
            "--max-cells",
            "1",
            "--slide",
            "1",
            "--cx",
            "1000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "place",
            "table-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:B2",
            "--expect-source-range",
            "A1:C2",
            "--slide",
            "1",
            "--cx",
            "1000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "place",
            "table-from-xlsx",
            workbook_str,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1",
            "--slide",
            "1",
            "--cx",
            "1000",
            "--out",
            bad_out_str,
        ],
    ] {
        assert_baseline_rust_json_match(&args, "place table-from-xlsx representative error");
    }
}
