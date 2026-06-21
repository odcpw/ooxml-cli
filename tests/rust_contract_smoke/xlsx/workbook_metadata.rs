#[test]
fn xlsx_workbook_metadata_inspect_matches_go_oracle() {
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "workbook",
        "metadata",
        "inspect",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
    ]);
}

#[test]
fn xlsx_workbook_metadata_update_matches_go_oracle_and_readback() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-workbook-metadata-update-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in = temp_dir.join("go in.xlsx");
    let rust_in = temp_dir.join("rust in.xlsx");
    let go_out = temp_dir.join("go out.xlsx");
    let rust_out = temp_dir.join("rust out.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in).expect("stage go input");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &rust_in).expect("stage rust input");
    let go_in = go_in.to_string_lossy().to_string();
    let rust_in = rust_in.to_string_lossy().to_string();
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "xlsx",
        "workbook",
        "metadata",
        "update",
        &go_in,
        "--keywords",
        "budget,forecast",
        "--description",
        "Board pack",
        "--subject",
        "FY26",
        "--category",
        "Finance",
        "--company",
        "Acme Corp",
        "--manager",
        "Carol White",
        "--calc-mode",
        "manual",
        "--full-calc-on-load",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "workbook",
        "metadata",
        "update",
        &rust_in,
        "--keywords",
        "budget,forecast",
        "--description",
        "Board pack",
        "--subject",
        "FY26",
        "--category",
        "Finance",
        "--company",
        "Acme Corp",
        "--manager",
        "Carol White",
        "--calc-mode",
        "manual",
        "--full-calc-on-load",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "metadata update exit");
    assert_eq!(rust_stderr, go_stderr, "metadata update stderr");
    let go_raw = go_stdout.expect("go metadata update stdout");
    let rust_raw = rust_stdout.expect("rust metadata update stdout");
    let go_json = scrub_paths(go_raw, &[(&go_in, "[IN]"), (&go_out, "[OUT]")]);
    let rust_json = scrub_paths(
        rust_raw.clone(),
        &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")],
    );
    assert_eq!(rust_json, go_json, "metadata update stdout");
    assert_eq!(
        rust_json["updatedFields"],
        serde_json::json!([
            "subject",
            "description",
            "keywords",
            "category",
            "company",
            "manager",
            "calcMode",
            "fullCalcOnLoad"
        ]),
        "updatedFields must use Go mutator order"
    );

    assert_rust_emitted_ooxml_command_exits_zero(&rust_raw, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_raw, "inspectCommand");

    let (go_read_code, go_read_stdout, go_read_stderr) =
        run_go_ooxml(&["--json", "xlsx", "workbook", "metadata", "inspect", &go_out]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json", "xlsx", "workbook", "metadata", "inspect", &rust_out,
    ]);
    assert_eq!(rust_read_code, go_read_code, "metadata readback exit");
    assert_eq!(rust_read_stderr, go_read_stderr, "metadata readback stderr");
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust metadata readback"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(
            go_read_stdout.expect("go metadata readback"),
            &go_out,
            "[OUT]"
        ),
        "metadata readback stdout"
    );
    let content_types = read_zip_string(Path::new(&rust_out), "[Content_Types].xml");
    assert!(
        content_types.contains(
            r#"ContentType="application/vnd.openxmlformats-package.core-properties+xml""#
        ),
        "metadata update must emit the SDK-expected OPC core properties content type: {content_types}"
    );
    assert!(
        !content_types
            .contains("application/vnd.openxmlformats-officedocument.core-properties+xml"),
        "metadata update must not emit the invalid officedocument core properties content type: {content_types}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_workbook_metadata_update_dry_run_matches_go_oracle() {
    let workbook = "testdata/xlsx/minimal-workbook/workbook.xlsx";
    let args = [
        "--json",
        "xlsx",
        "workbook",
        "metadata",
        "update",
        workbook,
        "--title",
        "Dry",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_code, go_code, "metadata dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "metadata dry-run stderr");
    let rust_json = rust_stdout.expect("rust dry-run stdout");
    assert_eq!(rust_json, go_stdout.expect("go dry-run stdout"));
    assert_eq!(rust_json["dryRun"], Value::Bool(true));
    assert!(rust_json.get("output").is_none(), "dry-run output omitted");
    assert!(
        rust_json.get("validateCommand").is_none(),
        "dry-run validateCommand omitted"
    );
    assert!(
        rust_json.get("inspectCommand").is_none(),
        "dry-run inspectCommand omitted"
    );
}

#[test]
fn xlsx_workbook_metadata_clear_fields_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-workbook-metadata-clear-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_seed = temp_dir.join("go-seed.xlsx").to_string_lossy().to_string();
    let rust_seed = temp_dir
        .join("rust-seed.xlsx")
        .to_string_lossy()
        .to_string();
    let go_clear = temp_dir.join("go-clear.xlsx").to_string_lossy().to_string();
    let rust_clear = temp_dir
        .join("rust-clear.xlsx")
        .to_string_lossy()
        .to_string();

    let go_seed_args = [
        "--json",
        "xlsx",
        "workbook",
        "metadata",
        "update",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--title",
        "Temporary",
        "--full-calc-on-load",
        "--out",
        &go_seed,
    ];
    let rust_seed_args = [
        "--json",
        "xlsx",
        "workbook",
        "metadata",
        "update",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--title",
        "Temporary",
        "--full-calc-on-load",
        "--out",
        &rust_seed,
    ];
    assert_eq!(run_go_ooxml(&go_seed_args).0, 0, "go seed");
    assert_eq!(run_ooxml(&rust_seed_args).0, 0, "rust seed");

    let go_clear_args = [
        "--json",
        "xlsx",
        "workbook",
        "metadata",
        "update",
        &go_seed,
        "--title",
        "",
        "--full-calc-on-load=false",
        "--out",
        &go_clear,
    ];
    let rust_clear_args = [
        "--json",
        "xlsx",
        "workbook",
        "metadata",
        "update",
        &rust_seed,
        "--title",
        "",
        "--full-calc-on-load=false",
        "--out",
        &rust_clear,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_clear_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_code, go_code, "metadata clear exit");
    assert_eq!(rust_stderr, go_stderr, "metadata clear stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust clear stdout"),
            &[(&rust_seed, "[IN]"), (&rust_clear, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go clear stdout"),
            &[(&go_seed, "[IN]"), (&go_clear, "[OUT]")]
        ),
        "metadata clear stdout"
    );
    let core_xml = read_zip_string(Path::new(&rust_clear), "docProps/core.xml");
    assert!(
        !core_xml.contains("<dc:title>"),
        "empty title should remove dc:title"
    );
    let workbook_xml = read_zip_string(Path::new(&rust_clear), "xl/workbook.xml");
    assert!(
        !workbook_xml.contains("fullCalcOnLoad") && !workbook_xml.contains("forceFullCalc"),
        "explicit false should remove recalc attrs"
    );
    let (validate_code, _, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_clear]);
    assert_eq!(validate_code, 0, "metadata clear validate");
    assert_eq!(validate_stderr, None, "metadata clear validate stderr");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_workbook_metadata_error_envelopes_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-workbook-metadata-errors-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let out = temp_dir.join("out.xlsx").to_string_lossy().to_string();
    let workbook = "testdata/xlsx/minimal-workbook/workbook.xlsx";
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json", "xlsx", "workbook", "metadata", "update", workbook, "--out", &out,
        ],
        vec![
            "--json",
            "xlsx",
            "workbook",
            "metadata",
            "update",
            workbook,
            "--title",
            "Needs output mode",
        ],
        vec![
            "--json",
            "xlsx",
            "workbook",
            "metadata",
            "update",
            workbook,
            "--calc-mode",
            "bogus",
            "--out",
            &out,
        ],
        vec![
            "--json",
            "xlsx",
            "workbook",
            "metadata",
            "update",
            workbook,
            "--title",
            "New",
            "--expect-title",
            "Wrong",
            "--out",
            &out,
        ],
    ];
    for args in cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "metadata error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "metadata error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "metadata error stderr for {args:?}");
    }

    let _ = fs::remove_dir_all(&temp_dir);
}
