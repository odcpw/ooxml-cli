#[test]
fn docx_tables_show_matches_go_oracle() {
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
        assert_go_rust_match(&args);
    }

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-tables-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables temp dir");
    let nested_table_docx = temp_dir.join("nested-table.docx");
    write_nested_table_docx(&nested_table_docx);
    let nested_table_docx = nested_table_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "tables", "show", &nested_table_docx]);
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_tables_set_clear_cell_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-tables-cell-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables cell temp dir");

    let document = "testdata/docx/table/document.docx";
    let (hash_code, hash_stdout, hash_stderr) =
        run_go_ooxml(&["--json", "docx", "tables", "show", document, "--table", "1"]);
    assert_eq!(hash_code, 0, "oracle table hash lookup exit");
    assert_eq!(hash_stderr, None, "oracle table hash lookup stderr");
    let table_hash = hash_stdout.expect("oracle table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("table hash")
        .to_string();

    let go_set_out = temp_dir
        .join("tables-set-cell-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_set_out = temp_dir
        .join("tables-set-cell-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_set_args = [
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
        &go_set_out,
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
    let (go_set_code, go_set_stdout, go_set_stderr) = run_go_ooxml(&go_set_args);
    let (rust_set_code, rust_set_stdout, rust_set_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_set_code, go_set_code, "set-cell exit");
    assert_eq!(rust_set_stderr, go_set_stderr, "set-cell stderr");
    let go_set_json = scrub_path(
        go_set_stdout.expect("Go set-cell stdout"),
        &go_set_out,
        "[SET_OUT]",
    );
    let rust_set_json = scrub_path(
        rust_set_stdout.expect("Rust set-cell stdout"),
        &rust_set_out,
        "[SET_OUT]",
    );
    assert_eq!(rust_set_json, go_set_json, "set-cell stdout");
    assert_eq!(rust_set_json["text"], Value::String("Approved".to_string()));
    assert_eq!(
        rust_set_json["previousText"],
        Value::String("B1".to_string())
    );

    let (set_validate_code, _set_validate_stdout, set_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_set_out]);
    assert_eq!(set_validate_code, 0, "set-cell validate exit");
    assert_eq!(set_validate_stderr, None, "set-cell validate stderr");

    let (go_set_read_code, go_set_read_stdout, go_set_read_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &go_set_out,
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
    assert_eq!(rust_set_read_code, go_set_read_code, "set readback exit");
    assert_eq!(
        rust_set_read_stderr, go_set_read_stderr,
        "set readback stderr"
    );
    let go_set_table = scrub_path(
        go_set_read_stdout.expect("Go set readback JSON")["tables"][0].clone(),
        &go_set_out,
        "[SET_OUT]",
    );
    let rust_set_table = scrub_path(
        rust_set_read_stdout.expect("Rust set readback JSON")["tables"][0].clone(),
        &rust_set_out,
        "[SET_OUT]",
    );
    assert_eq!(rust_set_table, go_set_table, "set readback table");
    assert_eq!(
        rust_set_table["cells"][0][1],
        Value::String("Approved".to_string())
    );

    let set_hash = rust_set_json["contentHash"]
        .as_str()
        .expect("set-cell content hash")
        .to_string();
    let go_clear_out = temp_dir
        .join("tables-clear-cell-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_clear_out = temp_dir
        .join("tables-clear-cell-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_clear_args = [
        "--json",
        "docx",
        "tables",
        "clear-cell",
        &go_set_out,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &set_hash,
        "--out",
        &go_clear_out,
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
    let (go_clear_code, go_clear_stdout, go_clear_stderr) = run_go_ooxml(&go_clear_args);
    let (rust_clear_code, rust_clear_stdout, rust_clear_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_clear_code, go_clear_code, "clear-cell exit");
    assert_eq!(rust_clear_stderr, go_clear_stderr, "clear-cell stderr");
    let go_clear_json = scrub_paths(
        go_clear_stdout.expect("Go clear-cell stdout"),
        &[(&go_set_out, "[SET_OUT]"), (&go_clear_out, "[CLEAR_OUT]")],
    );
    let rust_clear_json = scrub_paths(
        rust_clear_stdout.expect("Rust clear-cell stdout"),
        &[
            (&rust_set_out, "[SET_OUT]"),
            (&rust_clear_out, "[CLEAR_OUT]"),
        ],
    );
    assert_eq!(rust_clear_json, go_clear_json, "clear-cell stdout");
    assert_eq!(
        rust_clear_json["previousText"],
        Value::String("Approved".to_string())
    );

    let (clear_validate_code, _clear_validate_stdout, clear_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_clear_out]);
    assert_eq!(clear_validate_code, 0, "clear-cell validate exit");
    assert_eq!(clear_validate_stderr, None, "clear-cell validate stderr");

    let (go_clear_read_code, go_clear_read_stdout, go_clear_read_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &go_clear_out,
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
        rust_clear_read_code, go_clear_read_code,
        "clear readback exit"
    );
    assert_eq!(
        rust_clear_read_stderr, go_clear_read_stderr,
        "clear readback stderr"
    );
    let go_clear_table = scrub_path(
        go_clear_read_stdout.expect("Go clear readback JSON")["tables"][0].clone(),
        &go_clear_out,
        "[CLEAR_OUT]",
    );
    let rust_clear_table = scrub_path(
        rust_clear_read_stdout.expect("Rust clear readback JSON")["tables"][0].clone(),
        &rust_clear_out,
        "[CLEAR_OUT]",
    );
    assert_eq!(rust_clear_table, go_clear_table, "clear readback table");
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
    let (go_dry_code, go_dry_stdout, go_dry_stderr) = run_go_ooxml(&dry_args);
    let (rust_dry_code, rust_dry_stdout, rust_dry_stderr) = run_ooxml(&dry_args);
    assert_eq!(rust_dry_code, go_dry_code, "set-cell dry-run exit");
    assert_eq!(rust_dry_stderr, go_dry_stderr, "set-cell dry-run stderr");
    let dry_json = rust_dry_stdout.expect("Rust set-cell dry-run stdout");
    assert_eq!(
        dry_json,
        go_dry_stdout.expect("Go set-cell dry-run stdout"),
        "set-cell dry-run stdout"
    );
    assert_eq!(dry_json["dryRun"], Value::Bool(true));
    assert!(dry_json.get("output").is_none(), "dry-run omits output");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_tables_insert_row_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-tables-insert-row-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables insert-row temp dir");

    let document = "testdata/docx/table/document.docx";
    let (hash_code, hash_stdout, hash_stderr) =
        run_go_ooxml(&["--json", "docx", "tables", "show", document, "--table", "1"]);
    assert_eq!(hash_code, 0, "oracle table hash lookup exit");
    assert_eq!(hash_stderr, None, "oracle table hash lookup stderr");
    let table_hash = hash_stdout.expect("oracle table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("table hash")
        .to_string();

    let go_insert_out = temp_dir
        .join("tables-insert-row-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_insert_out = temp_dir
        .join("tables-insert-row-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_insert_args = [
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
        &go_insert_out,
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
    let (go_insert_code, go_insert_stdout, go_insert_stderr) = run_go_ooxml(&go_insert_args);
    let (rust_insert_code, rust_insert_stdout, rust_insert_stderr) = run_ooxml(&rust_insert_args);
    assert_eq!(rust_insert_code, go_insert_code, "insert-row exit");
    assert_eq!(rust_insert_stderr, go_insert_stderr, "insert-row stderr");
    let go_insert_json = scrub_path(
        go_insert_stdout.expect("Go insert-row stdout"),
        &go_insert_out,
        "[INSERT_OUT]",
    );
    let rust_insert_json = scrub_path(
        rust_insert_stdout.expect("Rust insert-row stdout"),
        &rust_insert_out,
        "[INSERT_OUT]",
    );
    assert_eq!(rust_insert_json, go_insert_json, "insert-row stdout");
    assert_eq!(rust_insert_json["row"], Value::from(2));
    assert_eq!(rust_insert_json["rows"], Value::from(3));
    assert_eq!(rust_insert_json["cols"], Value::from(2));

    let (insert_validate_code, _insert_validate_stdout, insert_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_insert_out]);
    assert_eq!(insert_validate_code, 0, "insert-row validate exit");
    assert_eq!(insert_validate_stderr, None, "insert-row validate stderr");

    let (go_read_code, go_read_stdout, go_read_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &go_insert_out,
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
    assert_eq!(rust_read_code, go_read_code, "insert readback exit");
    assert_eq!(rust_read_stderr, go_read_stderr, "insert readback stderr");
    let go_read_table = scrub_path(
        go_read_stdout.expect("Go insert readback JSON")["tables"][0].clone(),
        &go_insert_out,
        "[INSERT_OUT]",
    );
    let rust_read_table = scrub_path(
        rust_read_stdout.expect("Rust insert readback JSON")["tables"][0].clone(),
        &rust_insert_out,
        "[INSERT_OUT]",
    );
    assert_eq!(rust_read_table, go_read_table, "insert readback table");
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
    let (go_dry_code, go_dry_stdout, go_dry_stderr) = run_go_ooxml(&dry_args);
    let (rust_dry_code, rust_dry_stdout, rust_dry_stderr) = run_ooxml(&dry_args);
    assert_eq!(rust_dry_code, go_dry_code, "insert-row dry-run exit");
    assert_eq!(rust_dry_stderr, go_dry_stderr, "insert-row dry-run stderr");
    let dry_json = rust_dry_stdout.expect("Rust insert-row dry-run stdout");
    assert_eq!(
        dry_json,
        go_dry_stdout.expect("Go insert-row dry-run stdout"),
        "insert-row dry-run stdout"
    );
    assert_eq!(dry_json["dryRun"], Value::Bool(true));
    assert!(dry_json.get("output").is_none(), "dry-run omits output");

    assert_go_rust_match(&[
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
    assert_go_rust_match(&[
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
    assert_go_rust_match(&[
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
    assert_go_rust_match(&[
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
    let (merged_hash_code, merged_hash_stdout, merged_hash_stderr) = run_go_ooxml(&[
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
    assert_go_rust_match(&[
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
fn docx_tables_delete_row_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-tables-delete-row-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables delete-row temp dir");

    let document = "testdata/docx/table/document.docx";
    let (hash_code, hash_stdout, hash_stderr) =
        run_go_ooxml(&["--json", "docx", "tables", "show", document, "--table", "1"]);
    assert_eq!(hash_code, 0, "oracle table hash lookup exit");
    assert_eq!(hash_stderr, None, "oracle table hash lookup stderr");
    let table_hash = hash_stdout.expect("oracle table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("table hash")
        .to_string();

    let go_delete_out = temp_dir
        .join("tables-delete-row-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_delete_out = temp_dir
        .join("tables-delete-row-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_delete_args = [
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
        &go_delete_out,
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
    let (go_delete_code, go_delete_stdout, go_delete_stderr) = run_go_ooxml(&go_delete_args);
    let (rust_delete_code, rust_delete_stdout, rust_delete_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_delete_code, go_delete_code, "delete-row exit");
    assert_eq!(rust_delete_stderr, go_delete_stderr, "delete-row stderr");
    let go_delete_json = scrub_path(
        go_delete_stdout.expect("Go delete-row stdout"),
        &go_delete_out,
        "[DELETE_OUT]",
    );
    let rust_delete_json = scrub_path(
        rust_delete_stdout.expect("Rust delete-row stdout"),
        &rust_delete_out,
        "[DELETE_OUT]",
    );
    assert_eq!(rust_delete_json, go_delete_json, "delete-row stdout");
    assert_eq!(rust_delete_json["row"], Value::from(1));
    assert_eq!(rust_delete_json["rows"], Value::from(1));
    assert_eq!(rust_delete_json["cols"], Value::from(2));

    let (delete_validate_code, _delete_validate_stdout, delete_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_delete_out]);
    assert_eq!(delete_validate_code, 0, "delete-row validate exit");
    assert_eq!(delete_validate_stderr, None, "delete-row validate stderr");

    let (go_read_code, go_read_stdout, go_read_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &go_delete_out,
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
    assert_eq!(rust_read_code, go_read_code, "delete readback exit");
    assert_eq!(rust_read_stderr, go_read_stderr, "delete readback stderr");
    let go_read_table = scrub_path(
        go_read_stdout.expect("Go delete readback JSON")["tables"][0].clone(),
        &go_delete_out,
        "[DELETE_OUT]",
    );
    let rust_read_table = scrub_path(
        rust_read_stdout.expect("Rust delete readback JSON")["tables"][0].clone(),
        &rust_delete_out,
        "[DELETE_OUT]",
    );
    assert_eq!(rust_read_table, go_read_table, "delete readback table");
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
    let (go_dry_code, go_dry_stdout, go_dry_stderr) = run_go_ooxml(&dry_args);
    let (rust_dry_code, rust_dry_stdout, rust_dry_stderr) = run_ooxml(&dry_args);
    assert_eq!(rust_dry_code, go_dry_code, "delete-row dry-run exit");
    assert_eq!(rust_dry_stderr, go_dry_stderr, "delete-row dry-run stderr");
    let dry_json = rust_dry_stdout.expect("Rust delete-row dry-run stdout");
    assert_eq!(
        dry_json,
        go_dry_stdout.expect("Go delete-row dry-run stdout"),
        "delete-row dry-run stdout"
    );
    assert_eq!(dry_json["dryRun"], Value::Bool(true));
    assert!(dry_json.get("output").is_none(), "dry-run omits output");

    let delete_hash = rust_delete_json["contentHash"]
        .as_str()
        .expect("delete-row content hash")
        .to_string();
    let (go_last_code, go_last_stdout, go_last_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "delete-row",
        &go_delete_out,
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
    assert_eq!(rust_last_code, go_last_code, "last-row delete exit");
    assert_eq!(rust_last_stdout, go_last_stdout, "last-row delete stdout");
    assert_eq!(rust_last_stderr, go_last_stderr, "last-row delete stderr");

    let bad_out = temp_dir.join("bad.docx").to_string_lossy().to_string();
    assert_go_rust_match(&[
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
    assert_go_rust_match(&[
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
    let (merged_hash_code, merged_hash_stdout, merged_hash_stderr) = run_go_ooxml(&[
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
    assert_go_rust_match(&[
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
