#[test]
fn docx_tables_create_appends_table_to_scaffold_and_validates() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-tables-create-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables create temp dir");

    let scaffold_path = temp_dir.join("scaffold.docx");
    let output_path = temp_dir.join("table.docx");
    let scaffold = scaffold_path.to_string_lossy().to_string();
    let output = output_path.to_string_lossy().to_string();

    let (scaffold_code, scaffold_stdout, scaffold_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "scaffold",
        &scaffold,
        "--text",
        "Quarterly report",
    ]);
    assert_eq!(scaffold_code, 0, "scaffold exit");
    assert_eq!(scaffold_stderr, None, "scaffold stderr");
    assert!(scaffold_stdout.is_some(), "scaffold stdout");

    let values = r#"[["Region","Units","Notes"],["West",12,"Ready"],["North",null,"Needs review"]]"#;
    let (create_code, create_stdout, create_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "create",
        &scaffold,
        "--values",
        values,
        "--out",
        &output,
    ]);
    assert_eq!(create_code, 0, "tables create exit");
    assert_eq!(create_stderr, None, "tables create stderr");
    let created = create_stdout.expect("tables create stdout");
    assert_eq!(created["output"], Value::String(output.clone()));
    assert_eq!(created["table"], Value::from(1));
    assert_eq!(created["rows"], Value::from(3));
    assert_eq!(created["cols"], Value::from(3));
    assert_eq!(
        created["validateCommand"],
        Value::String(format!(
            "ooxml validate --strict {}",
            command_arg_for_test(&output)
        ))
    );
    assert_rust_emitted_ooxml_command_succeeds(&created, "tablesShowCommand");

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json", "docx", "tables", "show", &output, "--table", "1",
    ]);
    assert_eq!(show_code, 0, "tables show exit");
    assert_eq!(show_stderr, None, "tables show stderr");
    let table = &show_stdout.expect("tables show stdout")["tables"][0];
    assert_eq!(table["rows"], Value::from(3));
    assert_eq!(table["cols"], Value::from(3));
    assert_eq!(table["cells"][0][0], Value::String("Region".to_string()));
    assert_eq!(table["cells"][1][1], Value::String("12".to_string()));
    assert_eq!(table["cells"][2][1], Value::String(String::new()));
    assert_eq!(
        table["cells"][2][2],
        Value::String("Needs review".to_string())
    );

    let document_xml = read_zip_string(&output_path, "word/document.xml");
    assert_xml_tag_order(
        &document_xml,
        &["<w:p>", "<w:tbl>", "<w:tblPr", "<w:tblGrid", "<w:tr", "<w:sectPr"],
    );
    assert_eq!(document_xml.matches("<w:tr>").count(), 3);
    assert_eq!(document_xml.matches("<w:tc>").count(), 9);

    assert_strict_valid(&output);
    let (conformance_code, conformance_stdout, conformance_stderr) =
        run_ooxml(&["--json", "conformance", "check", &output]);
    assert_eq!(conformance_code, 0, "conformance exit");
    assert_eq!(conformance_stderr, None, "conformance stderr");
    assert_eq!(
        conformance_stdout.expect("conformance stdout")["status"],
        Value::String("passed".to_string())
    );

    let (bad_code, bad_stdout, bad_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "create",
        &scaffold,
        "--values",
        r#"[["A"],["B","C"]]"#,
        "--dry-run",
    ]);
    assert_eq!(bad_code, 2, "ragged matrix exit");
    assert_eq!(bad_stdout, None, "ragged matrix stdout");
    assert!(
        bad_stderr
            .expect("ragged matrix stderr")["error"]["message"]
            .as_str()
            .expect("ragged matrix error")
            .contains("rectangular"),
        "ragged matrix should explain rectangular requirement"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_tables_show_matches_rust_baseline() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--table",
            "1",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/merged-table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--details",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--table",
            "2",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--table",
            "-1",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/corrupted-missing-document/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_rust_baseline_match(&args);
    }

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-tables-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables temp dir");
    let nested_table_docx = temp_dir.join("nested-table.docx");
    write_nested_table_docx(&nested_table_docx);
    let nested_table_docx = nested_table_docx.to_string_lossy().to_string();
    assert_rust_baseline_match(&["--json", "docx", "tables", "show", &nested_table_docx]);
    let _ = fs::remove_dir_all(&temp_dir);
}

fn assert_xml_tag_order(xml: &str, tags: &[&str]) {
    let mut previous = 0usize;
    for tag in tags {
        let offset = xml[previous..]
            .find(tag)
            .unwrap_or_else(|| panic!("missing {tag} after byte {previous} in:\n{xml}"));
        previous += offset + tag.len();
    }
}

fn assert_docx_table_command_templates(value: &Value, table: usize) {
    assert_eq!(
        value["validateCommandTemplate"],
        Value::String("ooxml validate --strict '<out.docx>'".to_string())
    );
    assert_eq!(
        value["tablesListCommandTemplate"],
        Value::String("ooxml --json docx tables show '<out.docx>'".to_string())
    );
    assert_eq!(
        value["tablesShowCommandTemplate"],
        Value::String(format!(
            "ooxml --json docx tables show '<out.docx>' --table {table}"
        ))
    );
}

#[test]
fn docx_tables_set_clear_cell_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-tables-cell-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables cell temp dir");

    let document = "testdata/docx/table/document.docx";
    let (hash_code, hash_stdout, hash_stderr) =
        run_ooxml_baseline(&["--json", "docx", "tables", "show", document, "--table", "1"]);
    assert_eq!(hash_code, 0, "oracle table hash lookup exit");
    assert_eq!(hash_stderr, None, "oracle table hash lookup stderr");
    let table_hash = hash_stdout.expect("oracle table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("table hash")
        .to_string();

    let baseline_set_out = temp_dir
        .join("tables-set-cell-baseline.docx")
        .to_string_lossy()
        .to_string();
    let rust_set_out = temp_dir
        .join("tables-set-cell-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_set_args = [
        "--json",
        "docx",
        "tables",
        "set-cell",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &table_hash,
        "--text",
        "Approved",
        "--out",
        &baseline_set_out,
    ];
    let rust_set_args = [
        "--json",
        "docx",
        "tables",
        "set-cell",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &table_hash,
        "--text",
        "Approved",
        "--out",
        &rust_set_out,
    ];
    let (baseline_set_code, baseline_set_stdout, baseline_set_stderr) = run_ooxml_baseline(&baseline_set_args);
    let (rust_set_code, rust_set_stdout, rust_set_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_set_code, baseline_set_code, "set-cell exit");
    assert_eq!(rust_set_stderr, baseline_set_stderr, "set-cell stderr");
    let baseline_set_json = scrub_path(
        baseline_set_stdout.expect("Rust baseline set-cell stdout"),
        &baseline_set_out,
        "[SET_OUT]",
    );
    let rust_set_json = scrub_path(
        rust_set_stdout.expect("Rust set-cell stdout"),
        &rust_set_out,
        "[SET_OUT]",
    );
    assert_eq!(rust_set_json, baseline_set_json, "set-cell stdout");
    assert_eq!(rust_set_json["text"], Value::String("Approved".to_string()));
    assert_eq!(
        rust_set_json["previousText"],
        Value::String("B1".to_string())
    );

    let (set_validate_code, _set_validate_stdout, set_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_set_out]);
    assert_eq!(set_validate_code, 0, "set-cell validate exit");
    assert_eq!(set_validate_stderr, None, "set-cell validate stderr");

    let (baseline_set_read_code, baseline_set_read_stdout, baseline_set_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "docx",
        "tables",
        "show",
        &baseline_set_out,
        "--table",
        "1",
    ]);
    let (rust_set_read_code, rust_set_read_stdout, rust_set_read_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &rust_set_out,
        "--table",
        "1",
    ]);
    assert_eq!(rust_set_read_code, baseline_set_read_code, "set readback exit");
    assert_eq!(
        rust_set_read_stderr, baseline_set_read_stderr,
        "set readback stderr"
    );
    let baseline_set_table = scrub_path(
        baseline_set_read_stdout.expect("Rust baseline set readback JSON")["tables"][0].clone(),
        &baseline_set_out,
        "[SET_OUT]",
    );
    let rust_set_table = scrub_path(
        rust_set_read_stdout.expect("Rust set readback JSON")["tables"][0].clone(),
        &rust_set_out,
        "[SET_OUT]",
    );
    assert_eq!(rust_set_table, baseline_set_table, "set readback table");
    assert_eq!(
        rust_set_table["cells"][0][1],
        Value::String("Approved".to_string())
    );

    let set_hash = rust_set_json["contentHash"]
        .as_str()
        .expect("set-cell content hash")
        .to_string();
    let baseline_clear_out = temp_dir
        .join("tables-clear-cell-baseline.docx")
        .to_string_lossy()
        .to_string();
    let rust_clear_out = temp_dir
        .join("tables-clear-cell-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_clear_args = [
        "--json",
        "docx",
        "tables",
        "clear-cell",
        &baseline_set_out,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &set_hash,
        "--out",
        &baseline_clear_out,
    ];
    let rust_clear_args = [
        "--json",
        "docx",
        "tables",
        "clear-cell",
        &rust_set_out,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &set_hash,
        "--out",
        &rust_clear_out,
    ];
    let (baseline_clear_code, baseline_clear_stdout, baseline_clear_stderr) = run_ooxml_baseline(&baseline_clear_args);
    let (rust_clear_code, rust_clear_stdout, rust_clear_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_clear_code, baseline_clear_code, "clear-cell exit");
    assert_eq!(rust_clear_stderr, baseline_clear_stderr, "clear-cell stderr");
    let baseline_clear_json = scrub_paths(
        baseline_clear_stdout.expect("Rust baseline clear-cell stdout"),
        &[(&baseline_set_out, "[SET_OUT]"), (&baseline_clear_out, "[CLEAR_OUT]")],
    );
    let rust_clear_json = scrub_paths(
        rust_clear_stdout.expect("Rust clear-cell stdout"),
        &[
            (&rust_set_out, "[SET_OUT]"),
            (&rust_clear_out, "[CLEAR_OUT]"),
        ],
    );
    assert_eq!(rust_clear_json, baseline_clear_json, "clear-cell stdout");
    assert_eq!(
        rust_clear_json["previousText"],
        Value::String("Approved".to_string())
    );

    let (clear_validate_code, _clear_validate_stdout, clear_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_clear_out]);
    assert_eq!(clear_validate_code, 0, "clear-cell validate exit");
    assert_eq!(clear_validate_stderr, None, "clear-cell validate stderr");

    let (baseline_clear_read_code, baseline_clear_read_stdout, baseline_clear_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "docx",
        "tables",
        "show",
        &baseline_clear_out,
        "--table",
        "1",
    ]);
    let (rust_clear_read_code, rust_clear_read_stdout, rust_clear_read_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &rust_clear_out,
        "--table",
        "1",
    ]);
    assert_eq!(
        rust_clear_read_code, baseline_clear_read_code,
        "clear readback exit"
    );
    assert_eq!(
        rust_clear_read_stderr, baseline_clear_read_stderr,
        "clear readback stderr"
    );
    let baseline_clear_table = scrub_path(
        baseline_clear_read_stdout.expect("Rust baseline clear readback JSON")["tables"][0].clone(),
        &baseline_clear_out,
        "[CLEAR_OUT]",
    );
    let rust_clear_table = scrub_path(
        rust_clear_read_stdout.expect("Rust clear readback JSON")["tables"][0].clone(),
        &rust_clear_out,
        "[CLEAR_OUT]",
    );
    assert_eq!(rust_clear_table, baseline_clear_table, "clear readback table");
    assert_eq!(
        rust_clear_table["cells"][0][1],
        Value::String(String::new())
    );

    let dry_args = [
        "--json",
        "docx",
        "tables",
        "set-cell",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "1",
        "--expect-hash",
        &table_hash,
        "--text",
        "",
        "--dry-run",
    ];
    let (rust_dry_code, rust_dry_stdout, rust_dry_stderr) = run_ooxml(&dry_args);
    assert_eq!(rust_dry_code, 0, "set-cell dry-run exit");
    assert_eq!(rust_dry_stderr, None, "set-cell dry-run stderr");
    let dry_json = rust_dry_stdout.expect("Rust set-cell dry-run stdout");
    assert_docx_table_command_templates(&dry_json, 1);
    assert_eq!(dry_json["dryRun"], Value::Bool(true));
    assert!(dry_json.get("output").is_none(), "dry-run omits output");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_tables_insert_row_matches_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-tables-insert-row-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables insert-row temp dir");

    let document = "testdata/docx/table/document.docx";
    let (hash_code, hash_stdout, hash_stderr) =
        run_ooxml_baseline(&["--json", "docx", "tables", "show", document, "--table", "1"]);
    assert_eq!(hash_code, 0, "oracle table hash lookup exit");
    assert_eq!(hash_stderr, None, "oracle table hash lookup stderr");
    let table_hash = hash_stdout.expect("oracle table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("table hash")
        .to_string();

    let baseline_insert_out = temp_dir
        .join("tables-insert-row-baseline.docx")
        .to_string_lossy()
        .to_string();
    let rust_insert_out = temp_dir
        .join("tables-insert-row-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_insert_args = [
        "--json",
        "docx",
        "tables",
        "insert-row",
        document,
        "--table",
        "1",
        "--at",
        "2",
        "--expect-hash",
        &table_hash,
        "--out",
        &baseline_insert_out,
    ];
    let rust_insert_args = [
        "--json",
        "docx",
        "tables",
        "insert-row",
        document,
        "--table",
        "1",
        "--at",
        "2",
        "--expect-hash",
        &table_hash,
        "--out",
        &rust_insert_out,
    ];
    let (baseline_insert_code, baseline_insert_stdout, baseline_insert_stderr) = run_ooxml_baseline(&baseline_insert_args);
    let (rust_insert_code, rust_insert_stdout, rust_insert_stderr) = run_ooxml(&rust_insert_args);
    assert_eq!(rust_insert_code, baseline_insert_code, "insert-row exit");
    assert_eq!(rust_insert_stderr, baseline_insert_stderr, "insert-row stderr");
    let baseline_insert_json = scrub_path(
        baseline_insert_stdout.expect("Rust baseline insert-row stdout"),
        &baseline_insert_out,
        "[INSERT_OUT]",
    );
    let rust_insert_json = scrub_path(
        rust_insert_stdout.expect("Rust insert-row stdout"),
        &rust_insert_out,
        "[INSERT_OUT]",
    );
    assert_eq!(rust_insert_json, baseline_insert_json, "insert-row stdout");
    assert_eq!(rust_insert_json["row"], Value::from(2));
    assert_eq!(rust_insert_json["rows"], Value::from(3));
    assert_eq!(rust_insert_json["cols"], Value::from(2));

    let (insert_validate_code, _insert_validate_stdout, insert_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_insert_out]);
    assert_eq!(insert_validate_code, 0, "insert-row validate exit");
    assert_eq!(insert_validate_stderr, None, "insert-row validate stderr");

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "docx",
        "tables",
        "show",
        &baseline_insert_out,
        "--table",
        "1",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &rust_insert_out,
        "--table",
        "1",
    ]);
    assert_eq!(rust_read_code, baseline_read_code, "insert readback exit");
    assert_eq!(rust_read_stderr, baseline_read_stderr, "insert readback stderr");
    let baseline_read_table = scrub_path(
        baseline_read_stdout.expect("Rust baseline insert readback JSON")["tables"][0].clone(),
        &baseline_insert_out,
        "[INSERT_OUT]",
    );
    let rust_read_table = scrub_path(
        rust_read_stdout.expect("Rust insert readback JSON")["tables"][0].clone(),
        &rust_insert_out,
        "[INSERT_OUT]",
    );
    assert_eq!(rust_read_table, baseline_read_table, "insert readback table");
    assert_eq!(rust_read_table["cells"][1][0], Value::String(String::new()));
    assert_eq!(rust_read_table["cells"][1][1], Value::String(String::new()));
    assert_eq!(
        rust_read_table["cells"][2][0],
        Value::String("A2".to_string())
    );

    let dry_args = [
        "--json",
        "docx",
        "tables",
        "insert-row",
        document,
        "--table",
        "1",
        "--at",
        "3",
        "--expect-hash",
        &table_hash,
        "--dry-run",
    ];
    let (rust_dry_code, rust_dry_stdout, rust_dry_stderr) = run_ooxml(&dry_args);
    assert_eq!(rust_dry_code, 0, "insert-row dry-run exit");
    assert_eq!(rust_dry_stderr, None, "insert-row dry-run stderr");
    let dry_json = rust_dry_stdout.expect("Rust insert-row dry-run stdout");
    assert_docx_table_command_templates(&dry_json, 1);
    assert_eq!(dry_json["dryRun"], Value::Bool(true));
    assert!(dry_json.get("output").is_none(), "dry-run omits output");

    assert_rust_baseline_match(&[
        "--json",
        "docx",
        "tables",
        "insert-row",
        document,
        "--table",
        "1",
        "--at",
        "0",
        "--expect-hash",
        &table_hash,
        "--dry-run",
    ]);

    let bad_out = temp_dir.join("bad.docx").to_string_lossy().to_string();
    assert_rust_baseline_match(&[
        "--json",
        "docx",
        "tables",
        "insert-row",
        document,
        "--table",
        "1",
        "--at",
        "9",
        "--expect-hash",
        &table_hash,
        "--out",
        &bad_out,
    ]);
    assert_rust_baseline_match(&[
        "--json",
        "docx",
        "tables",
        "insert-row",
        document,
        "--table",
        "9",
        "--at",
        "1",
        "--expect-hash",
        &table_hash,
        "--out",
        &bad_out,
    ]);
    assert_rust_baseline_match(&[
        "--json",
        "docx",
        "tables",
        "insert-row",
        document,
        "--table",
        "1",
        "--at",
        "1",
        "--expect-hash",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "--out",
        &bad_out,
    ]);

    let merged_document = "testdata/docx/merged-table/document.docx";
    let (merged_hash_code, merged_hash_stdout, merged_hash_stderr) = run_ooxml_baseline(&[
        "--json",
        "docx",
        "tables",
        "show",
        merged_document,
        "--table",
        "1",
    ]);
    assert_eq!(merged_hash_code, 0, "merged table hash lookup exit");
    assert_eq!(merged_hash_stderr, None, "merged table hash lookup stderr");
    let merged_hash = merged_hash_stdout.expect("merged table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("merged table hash")
        .to_string();
    assert_rust_baseline_match(&[
        "--json",
        "docx",
        "tables",
        "insert-row",
        merged_document,
        "--table",
        "1",
        "--at",
        "1",
        "--expect-hash",
        &merged_hash,
        "--out",
        &bad_out,
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_tables_delete_row_matches_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-tables-delete-row-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables delete-row temp dir");

    let document = "testdata/docx/table/document.docx";
    let (hash_code, hash_stdout, hash_stderr) =
        run_ooxml_baseline(&["--json", "docx", "tables", "show", document, "--table", "1"]);
    assert_eq!(hash_code, 0, "oracle table hash lookup exit");
    assert_eq!(hash_stderr, None, "oracle table hash lookup stderr");
    let table_hash = hash_stdout.expect("oracle table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("table hash")
        .to_string();

    let baseline_delete_out = temp_dir
        .join("tables-delete-row-baseline.docx")
        .to_string_lossy()
        .to_string();
    let rust_delete_out = temp_dir
        .join("tables-delete-row-rust.docx")
        .to_string_lossy()
        .to_string();
    let baseline_delete_args = [
        "--json",
        "docx",
        "tables",
        "delete-row",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        &table_hash,
        "--out",
        &baseline_delete_out,
    ];
    let rust_delete_args = [
        "--json",
        "docx",
        "tables",
        "delete-row",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        &table_hash,
        "--out",
        &rust_delete_out,
    ];
    let (baseline_delete_code, baseline_delete_stdout, baseline_delete_stderr) = run_ooxml_baseline(&baseline_delete_args);
    let (rust_delete_code, rust_delete_stdout, rust_delete_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_delete_code, baseline_delete_code, "delete-row exit");
    assert_eq!(rust_delete_stderr, baseline_delete_stderr, "delete-row stderr");
    let baseline_delete_json = scrub_path(
        baseline_delete_stdout.expect("Rust baseline delete-row stdout"),
        &baseline_delete_out,
        "[DELETE_OUT]",
    );
    let rust_delete_json = scrub_path(
        rust_delete_stdout.expect("Rust delete-row stdout"),
        &rust_delete_out,
        "[DELETE_OUT]",
    );
    assert_eq!(rust_delete_json, baseline_delete_json, "delete-row stdout");
    assert_eq!(rust_delete_json["row"], Value::from(1));
    assert_eq!(rust_delete_json["rows"], Value::from(1));
    assert_eq!(rust_delete_json["cols"], Value::from(2));

    let (delete_validate_code, _delete_validate_stdout, delete_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_delete_out]);
    assert_eq!(delete_validate_code, 0, "delete-row validate exit");
    assert_eq!(delete_validate_stderr, None, "delete-row validate stderr");

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "docx",
        "tables",
        "show",
        &baseline_delete_out,
        "--table",
        "1",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &rust_delete_out,
        "--table",
        "1",
    ]);
    assert_eq!(rust_read_code, baseline_read_code, "delete readback exit");
    assert_eq!(rust_read_stderr, baseline_read_stderr, "delete readback stderr");
    let baseline_read_table = scrub_path(
        baseline_read_stdout.expect("Rust baseline delete readback JSON")["tables"][0].clone(),
        &baseline_delete_out,
        "[DELETE_OUT]",
    );
    let rust_read_table = scrub_path(
        rust_read_stdout.expect("Rust delete readback JSON")["tables"][0].clone(),
        &rust_delete_out,
        "[DELETE_OUT]",
    );
    assert_eq!(rust_read_table, baseline_read_table, "delete readback table");
    assert_eq!(
        rust_read_table["cells"][0][0],
        Value::String("A2".to_string())
    );

    let dry_args = [
        "--json",
        "docx",
        "tables",
        "delete-row",
        document,
        "--table",
        "1",
        "--row",
        "2",
        "--expect-hash",
        &table_hash,
        "--dry-run",
    ];
    let (rust_dry_code, rust_dry_stdout, rust_dry_stderr) = run_ooxml(&dry_args);
    assert_eq!(rust_dry_code, 0, "delete-row dry-run exit");
    assert_eq!(rust_dry_stderr, None, "delete-row dry-run stderr");
    let dry_json = rust_dry_stdout.expect("Rust delete-row dry-run stdout");
    assert_docx_table_command_templates(&dry_json, 1);
    assert_eq!(dry_json["dryRun"], Value::Bool(true));
    assert!(dry_json.get("output").is_none(), "dry-run omits output");

    let delete_hash = rust_delete_json["contentHash"]
        .as_str()
        .expect("delete-row content hash")
        .to_string();
    let (baseline_last_code, baseline_last_stdout, baseline_last_stderr) = run_ooxml_baseline(&[
        "--json",
        "docx",
        "tables",
        "delete-row",
        &baseline_delete_out,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        &delete_hash,
        "--dry-run",
    ]);
    let (rust_last_code, rust_last_stdout, rust_last_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "delete-row",
        &rust_delete_out,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        &delete_hash,
        "--dry-run",
    ]);
    assert_eq!(rust_last_code, baseline_last_code, "last-row delete exit");
    assert_eq!(rust_last_stdout, baseline_last_stdout, "last-row delete stdout");
    assert_eq!(rust_last_stderr, baseline_last_stderr, "last-row delete stderr");

    let bad_out = temp_dir.join("bad.docx").to_string_lossy().to_string();
    assert_rust_baseline_match(&[
        "--json",
        "docx",
        "tables",
        "delete-row",
        document,
        "--table",
        "1",
        "--row",
        "9",
        "--expect-hash",
        &table_hash,
        "--out",
        &bad_out,
    ]);
    assert_rust_baseline_match(&[
        "--json",
        "docx",
        "tables",
        "delete-row",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "--out",
        &bad_out,
    ]);

    let merged_document = "testdata/docx/merged-table/document.docx";
    let (merged_hash_code, merged_hash_stdout, merged_hash_stderr) = run_ooxml_baseline(&[
        "--json",
        "docx",
        "tables",
        "show",
        merged_document,
        "--table",
        "1",
    ]);
    assert_eq!(merged_hash_code, 0, "merged table hash lookup exit");
    assert_eq!(merged_hash_stderr, None, "merged table hash lookup stderr");
    let merged_hash = merged_hash_stdout.expect("merged table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("merged table hash")
        .to_string();
    assert_rust_baseline_match(&[
        "--json",
        "docx",
        "tables",
        "delete-row",
        merged_document,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        &merged_hash,
        "--out",
        &bad_out,
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}
