#[test]
fn xlsx_tables_list_show_matches_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-tables-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("table-workbook.xlsx");
    write_table_xlsx(&workbook);
    let workbook = workbook.to_string_lossy().to_string();

    let cases: Vec<Vec<&str>> = vec![
        vec!["--json", "xlsx", "tables", "list", &workbook],
        vec![
            "--json", "xlsx", "tables", "list", &workbook, "--sheet", "Data",
        ],
        vec![
            "--json", "xlsx", "tables", "show", &workbook, "--table", "Sales",
        ],
        vec![
            "--json",
            "xlsx",
            "tables",
            "show",
            &workbook,
            "--sheet",
            "sheetId:1",
            "--table",
            "tableId:1",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }

    for selector in [
        "tableId:1",
        "id:1",
        "table:1",
        "#1",
        "part:/xl/tables/table1.xml",
        "rid:rId1",
        "rId:rId1",
        "table:Sales",
        "displayName:Sales",
        "name:Sales",
        "Sales",
        "1",
    ] {
        assert_go_rust_match(&[
            "--json", "xlsx", "tables", "show", &workbook, "--table", selector,
        ]);
    }

    for missing in ["2", "Missing"] {
        assert_go_rust_match(&[
            "--json", "xlsx", "tables", "show", &workbook, "--table", missing,
        ]);
    }

    let spaced_dir = temp_dir.join("dir with spaces");
    fs::create_dir_all(&spaced_dir).expect("spaced temp dir");
    let spaced_workbook = spaced_dir.join("table workbook.xlsx");
    write_table_xlsx_with_sheet(&spaced_workbook, "Data Sheet");
    let spaced_workbook = spaced_workbook.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "xlsx", "tables", "list", &spaced_workbook]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "tables",
        "show",
        &spaced_workbook,
        "--sheet",
        "Data Sheet",
        "--table",
        "Sales",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_tables_export_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-table-export-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("table-workbook.xlsx");
    write_table_xlsx(&workbook);
    let workbook = workbook.to_string_lossy().to_string();

    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json", "xlsx", "tables", "export", &workbook, "--table", "Sales",
        ],
        vec![
            "--json",
            "xlsx",
            "tables",
            "export",
            &workbook,
            "--table",
            "Sales",
            "--include-types",
        ],
        vec![
            "--json",
            "xlsx",
            "tables",
            "export",
            &workbook,
            "--sheet",
            "Data",
            "--table",
            "tableId:1",
            "--include-types",
            "--include-formulas",
        ],
        vec![
            "--json", "xlsx", "tables", "export", &workbook, "--table", "Missing",
        ],
        vec![
            "--json",
            "xlsx",
            "tables",
            "export",
            &workbook,
            "--table",
            "Sales",
            "--max-cells",
            "1",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }

    let data_out = temp_dir.join("table-export.json");
    let data_out = data_out.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "tables",
        "export",
        &workbook,
        "--table",
        "Sales",
        "--include-types",
        "--include-formulas",
        "--data-out",
        &data_out,
    ]);
    let saved: Value =
        serde_json::from_str(fs::read_to_string(&data_out).expect("data-out file").trim())
            .expect("data-out JSON");
    let (code, expected_full, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "tables",
        "export",
        &workbook,
        "--table",
        "Sales",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let mut expected_full = expected_full.expect("full table export");
    expected_full["dataOut"] = Value::String(data_out);
    assert_eq!(saved, expected_full);

    let spaced_dir = temp_dir.join("dir with spaces");
    fs::create_dir_all(&spaced_dir).expect("spaced temp dir");
    let spaced_workbook = spaced_dir.join("table workbook.xlsx");
    write_table_xlsx_with_sheet(&spaced_workbook, "Data Sheet");
    let spaced_workbook = spaced_workbook.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "tables",
        "export",
        &spaced_workbook,
        "--sheet",
        "Data Sheet",
        "--table",
        "Sales",
        "--include-types",
        "--include-formulas",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_tables_set_column_format_matches_go_oracle_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-table-column-format-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    write_table_xlsx(&go_in_path);
    write_table_xlsx(&rust_in_path);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "xlsx",
        "tables",
        "set-column-format",
        &go_in,
        "--table",
        "Sales",
        "--column",
        "Amount",
        "--preset",
        "currency",
        "--decimals",
        "2",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "tables",
        "set-column-format",
        &rust_in,
        "--table",
        "Sales",
        "--column",
        "Amount",
        "--preset",
        "currency",
        "--decimals",
        "2",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "set-column-format exit");
    assert_eq!(rust_stderr, go_stderr, "set-column-format stderr");
    let rust_result = rust_stdout.expect("rust set-column-format stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go set-column-format stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "set-column-format stdout"
    );

    assert_eq!(rust_result["table"], "Sales");
    assert_eq!(rust_result["column"], "Amount");
    assert_eq!(rust_result["columnIndex"], 1);
    assert_eq!(rust_result["range"], "B2:B3");
    assert_eq!(rust_result["tableRange"], "A1:B3");
    for field in [
        "validateCommand",
        "cellsExtractCommand",
        "rangesExportCommand",
        "tableShowCommand",
        "tableExportCommand",
    ] {
        assert_rust_emitted_ooxml_command_succeeds(&rust_result, field);
    }

    let show_go = [
        "--json", "xlsx", "tables", "show", &go_out, "--table", "Sales",
    ];
    let show_rust = [
        "--json", "xlsx", "tables", "show", &rust_out, "--table", "Sales",
    ];
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "saved table show exit");
    assert_eq!(rust_stderr, go_stderr, "saved table show stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust saved table show"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(go_show.expect("go saved table show"), &go_out, "[OUT]"),
        "saved table show"
    );

    let range_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Data",
        "--range",
        "B2:B3",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ];
    let range_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Data",
        "--range",
        "B2:B3",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ];
    let (go_code, go_range, go_stderr) = run_go_ooxml(&range_go);
    let (rust_code, rust_range, rust_stderr) = run_ooxml(&range_rust);
    assert_eq!(rust_code, go_code, "saved formatted range export exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "saved formatted range export stderr"
    );
    assert_eq!(
        scrub_path(
            rust_range.expect("rust saved formatted range export"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(
            go_range.expect("go saved formatted range export"),
            &go_out,
            "[OUT]"
        ),
        "saved formatted range export"
    );
    let styles_xml = read_zip_string(&rust_out_path, "xl/styles.xml");
    assert!(
        styles_xml.contains(r#"applyNumberFormat="1""#),
        "Rust styles.xml missing applied number format:\n{styles_xml}"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "tables",
        "set-column-format",
        &go_in,
        "--table",
        "Sales",
        "--column",
        "Amount",
        "--preset",
        "integer",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "tables",
        "set-column-format",
        &rust_in,
        "--table",
        "Sales",
        "--column",
        "Amount",
        "--preset",
        "integer",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "set-column-format dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "set-column-format dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust set-column-format dry-run stdout"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(
            go_stdout.expect("go set-column-format dry-run stdout"),
            &go_in,
            "[IN]"
        ),
        "set-column-format dry-run stdout"
    );
    assert!(
        !zip_entry_exists(&rust_in_path, "xl/styles.xml"),
        "dry-run wrote styles.xml into Rust input workbook"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_tables_set_column_format_errors_and_totals_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-table-column-format-errors-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    write_table_xlsx(&go_in_path);
    write_table_xlsx(&rust_in_path);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();

    for (label, extra_args) in [
        (
            "guard mismatch",
            vec![
                "--table",
                "Sales",
                "--column",
                "Amount",
                "--expect-column",
                "Region",
                "--preset",
                "currency",
                "--dry-run",
            ],
        ),
        (
            "unknown column",
            vec![
                "--table",
                "Sales",
                "--column",
                "Missing",
                "--preset",
                "currency",
                "--dry-run",
            ],
        ),
        (
            "missing column",
            vec!["--table", "Sales", "--preset", "currency", "--dry-run"],
        ),
    ] {
        let mut go_args = vec!["--json", "xlsx", "tables", "set-column-format", &go_in];
        go_args.extend(extra_args.iter().copied());
        let mut rust_args = vec!["--json", "xlsx", "tables", "set-column-format", &rust_in];
        rust_args.extend(extra_args.iter().copied());
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, go_code, "{label} exit");
        assert_eq!(rust_stdout, go_stdout, "{label} stdout");
        assert_eq!(rust_stderr, go_stderr, "{label} stderr");
    }

    let totals_go_path = temp_dir.join("totals-go.xlsx");
    let totals_rust_path = temp_dir.join("totals-rust.xlsx");
    let totals_go_out_path = temp_dir.join("totals-go-out.xlsx");
    let totals_rust_out_path = temp_dir.join("totals-rust-out.xlsx");
    write_table_xlsx_with_totals(&totals_go_path);
    write_table_xlsx_with_totals(&totals_rust_path);
    let totals_go = totals_go_path.to_string_lossy().to_string();
    let totals_rust = totals_rust_path.to_string_lossy().to_string();
    let totals_go_out = totals_go_out_path.to_string_lossy().to_string();
    let totals_rust_out = totals_rust_out_path.to_string_lossy().to_string();
    let totals_go_args = [
        "--json",
        "xlsx",
        "tables",
        "set-column-format",
        &totals_go,
        "--table",
        "Sales",
        "--column",
        "Amount",
        "--preset",
        "number",
        "--decimals",
        "0",
        "--out",
        &totals_go_out,
    ];
    let totals_rust_args = [
        "--json",
        "xlsx",
        "tables",
        "set-column-format",
        &totals_rust,
        "--table",
        "Sales",
        "--column",
        "Amount",
        "--preset",
        "number",
        "--decimals",
        "0",
        "--out",
        &totals_rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&totals_go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&totals_rust_args);
    assert_eq!(rust_code, go_code, "totals set-column-format exit");
    assert_eq!(rust_stderr, go_stderr, "totals set-column-format stderr");
    let rust_result = rust_stdout.expect("rust totals stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&totals_rust, "[IN]"), (&totals_rust_out, "[OUT]"),]
        ),
        scrub_paths(
            go_stdout.expect("go totals stdout"),
            &[(&totals_go, "[IN]"), (&totals_go_out, "[OUT]")]
        ),
        "totals set-column-format stdout"
    );
    assert_eq!(rust_result["range"], "B2:B3");
    assert_eq!(rust_result["rows"], 2);
    let sheet_xml = read_zip_string(&totals_rust_out_path, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains(r#"<c r="B4"><v>30</v></c>"#),
        "totals row should not receive a style index:\n{sheet_xml}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_tables_append_rows_matches_go_oracle_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-table-append-rows-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    write_table_xlsx(&go_in_path);
    write_table_xlsx(&rust_in_path);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();
    let values = r#"[["North",30],["South",40]]"#;

    let go_args = [
        "--json",
        "xlsx",
        "tables",
        "append-rows",
        &go_in,
        "--table",
        "Sales",
        "--values",
        values,
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "tables",
        "append-rows",
        &rust_in,
        "--table",
        "Sales",
        "--values",
        values,
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "append-rows exit");
    assert_eq!(rust_stderr, go_stderr, "append-rows stderr");
    let rust_result = rust_stdout.expect("rust append-rows stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go append-rows stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "append-rows stdout"
    );

    for field in [
        "validateCommand",
        "cellsExtractCommand",
        "rangesExportCommand",
        "tableShowCommand",
        "tableExportCommand",
    ] {
        assert_rust_emitted_ooxml_command_succeeds(&rust_result, field);
    }

    let show_go = [
        "--json", "xlsx", "tables", "show", &go_out, "--table", "Sales",
    ];
    let show_rust = [
        "--json", "xlsx", "tables", "show", &rust_out, "--table", "Sales",
    ];
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "saved table show exit");
    assert_eq!(rust_stderr, go_stderr, "saved table show stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust saved table show"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(go_show.expect("go saved table show"), &go_out, "[OUT]"),
        "saved table show"
    );

    let range_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Data",
        "--range",
        "A4:B5",
        "--include-types",
        "--include-formulas",
    ];
    let range_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Data",
        "--range",
        "A4:B5",
        "--include-types",
        "--include-formulas",
    ];
    let (go_code, go_range, go_stderr) = run_go_ooxml(&range_go);
    let (rust_code, rust_range, rust_stderr) = run_ooxml(&range_rust);
    assert_eq!(rust_code, go_code, "saved appended range export exit");
    assert_eq!(rust_stderr, go_stderr, "saved appended range export stderr");
    assert_eq!(
        scrub_path(
            rust_range.expect("rust saved appended range export"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(
            go_range.expect("go saved appended range export"),
            &go_out,
            "[OUT]"
        ),
        "saved appended range export"
    );

    let table_xml = read_zip_string(&rust_out_path, "xl/tables/table1.xml");
    assert!(
        table_xml.contains(r#"ref="A1:B5""#),
        "Rust table ref was not expanded:\n{table_xml}"
    );
    let sheet_xml = read_zip_string(&rust_out_path, "xl/worksheets/sheet1.xml");
    for wanted in [r#"r="A4""#, "North", r#"r="B5""#, "<v>40</v>"] {
        assert!(
            sheet_xml.contains(wanted),
            "Rust worksheet missing {wanted:?} after append:\n{sheet_xml}"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

fn write_table_xlsx_with_totals(dest: &Path) {
    let base = dest.with_extension("base.xlsx");
    write_table_xlsx(&base);
    rewrite_zip_fixture(base.to_str().expect("base path"), dest, |name, data| {
        let data = match name {
            "xl/worksheets/sheet1.xml" => {
                let data = replace_ascii(
                    data,
                    r#"<dimension ref="A1:B3"/>"#,
                    r#"<dimension ref="A1:B4"/>"#,
                );
                replace_ascii(
                    data,
                    "  </sheetData>",
                    r#"    <row r="4"><c r="A4" t="inlineStr"><is><t>Total</t></is></c><c r="B4"><v>30</v></c></row>
  </sheetData>"#,
                )
            }
            "xl/tables/table1.xml" => {
                let data = replace_ascii(data, r#"ref="A1:B3""#, r#"ref="A1:B4""#);
                replace_ascii(
                    data,
                    r#"totalsRowShown="0""#,
                    r#"totalsRowShown="1" totalsRowCount="1""#,
                )
            }
            _ => data,
        };
        Some((name.to_string(), data))
    });
    let _ = fs::remove_file(base);
}

#[test]
fn xlsx_tables_append_rows_dry_run_and_errors_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-table-append-rows-dry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    write_table_xlsx(&go_in_path);
    write_table_xlsx(&rust_in_path);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let values = r#"[[{"formula":"SUM(B2:B3)"},"dry"]]"#;

    let dry_go = [
        "--json",
        "xlsx",
        "tables",
        "append-rows",
        &go_in,
        "--table",
        "Sales",
        "--values",
        values,
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "tables",
        "append-rows",
        &rust_in,
        "--table",
        "Sales",
        "--values",
        values,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "append-rows dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "append-rows dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust append-rows dry-run stdout"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(
            go_stdout.expect("go append-rows dry-run stdout"),
            &go_in,
            "[IN]"
        ),
        "append-rows dry-run stdout"
    );
    assert!(
        read_zip_string(&rust_in_path, "xl/tables/table1.xml").contains(r#"ref="A1:B3""#),
        "dry-run changed Rust input table"
    );

    let bad_go = [
        "--json",
        "xlsx",
        "tables",
        "append-rows",
        &go_in,
        "--table",
        "Sales",
        "--values",
        r#"[["Only one column"]]"#,
        "--dry-run",
    ];
    let bad_rust = [
        "--json",
        "xlsx",
        "tables",
        "append-rows",
        &rust_in,
        "--table",
        "Sales",
        "--values",
        r#"[["Only one column"]]"#,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&bad_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&bad_rust);
    assert_eq!(rust_code, go_code, "append-rows bad args exit");
    assert_eq!(rust_stdout, go_stdout, "append-rows bad args stdout");
    assert_eq!(rust_stderr, go_stderr, "append-rows bad args stderr");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_tables_append_records_matches_go_oracle_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-table-append-records-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    write_table_xlsx(&go_in_path);
    write_table_xlsx(&rust_in_path);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();
    let records = r#"[{"Amount":30,"Region":"North"},{"Region":"South","Amount":{"value":"40","type":"number"}}]"#;

    let go_args = [
        "--json",
        "xlsx",
        "tables",
        "append-records",
        &go_in,
        "--table",
        "Sales",
        "--expect-range",
        "A1:B3",
        "--records",
        records,
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "tables",
        "append-records",
        &rust_in,
        "--table",
        "Sales",
        "--expect-range",
        "A1:B3",
        "--records",
        records,
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "append-records exit");
    assert_eq!(rust_stderr, go_stderr, "append-records stderr");
    let rust_result = rust_stdout.expect("rust append-records stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go append-records stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "append-records stdout"
    );

    for field in [
        "validateCommand",
        "cellsExtractCommand",
        "rangesExportCommand",
        "tableShowCommand",
        "tableExportCommand",
    ] {
        assert_rust_emitted_ooxml_command_succeeds(&rust_result, field);
    }

    let show_go = [
        "--json", "xlsx", "tables", "show", &go_out, "--table", "Sales",
    ];
    let show_rust = [
        "--json", "xlsx", "tables", "show", &rust_out, "--table", "Sales",
    ];
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "saved table show exit");
    assert_eq!(rust_stderr, go_stderr, "saved table show stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust saved table show"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(go_show.expect("go saved table show"), &go_out, "[OUT]"),
        "saved table show"
    );

    let range_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Data",
        "--range",
        "A4:B5",
        "--include-types",
        "--include-formulas",
    ];
    let range_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Data",
        "--range",
        "A4:B5",
        "--include-types",
        "--include-formulas",
    ];
    let (go_code, go_range, go_stderr) = run_go_ooxml(&range_go);
    let (rust_code, rust_range, rust_stderr) = run_ooxml(&range_rust);
    assert_eq!(rust_code, go_code, "saved appended records export exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "saved appended records export stderr"
    );
    assert_eq!(
        scrub_path(
            rust_range.expect("rust saved appended records export"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(
            go_range.expect("go saved appended records export"),
            &go_out,
            "[OUT]"
        ),
        "saved appended records export"
    );

    let table_xml = read_zip_string(&rust_out_path, "xl/tables/table1.xml");
    assert!(
        table_xml.contains(r#"ref="A1:B5""#),
        "Rust table ref was not expanded:\n{table_xml}"
    );
    let sheet_xml = read_zip_string(&rust_out_path, "xl/worksheets/sheet1.xml");
    for wanted in [r#"r="A4""#, "North", r#"r="B5""#, "<v>40</v>"] {
        assert!(
            sheet_xml.contains(wanted),
            "Rust worksheet missing {wanted:?} after append-records:\n{sheet_xml}"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_tables_append_records_dry_run_and_errors_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-table-append-records-dry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    write_table_xlsx(&go_in_path);
    write_table_xlsx(&rust_in_path);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();

    let dry_records = r#"[{"Region":"Dry","Ignored":"extra"}]"#;
    let dry_go = [
        "--json",
        "xlsx",
        "tables",
        "append-records",
        &go_in,
        "--table",
        "Sales",
        "--expect-range",
        "A1:B3",
        "--records",
        dry_records,
        "--missing",
        "skip",
        "--ignore-extra-fields",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "tables",
        "append-records",
        &rust_in,
        "--table",
        "Sales",
        "--expect-range",
        "A1:B3",
        "--records",
        dry_records,
        "--missing",
        "skip",
        "--ignore-extra-fields",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "append-records dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "append-records dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust append-records dry-run stdout"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(
            go_stdout.expect("go append-records dry-run stdout"),
            &go_in,
            "[IN]"
        ),
        "append-records dry-run stdout"
    );
    assert!(
        read_zip_string(&rust_in_path, "xl/tables/table1.xml").contains(r#"ref="A1:B3""#),
        "dry-run changed Rust input table"
    );

    for (label, extra_args) in [
        (
            "missing field",
            vec![
                "--table",
                "Sales",
                "--expect-range",
                "A1:B3",
                "--records",
                r#"[{"Region":"North"}]"#,
                "--dry-run",
            ],
        ),
        (
            "unknown field",
            vec![
                "--table",
                "Sales",
                "--expect-range",
                "A1:B3",
                "--records",
                r#"[{"Region":"North","Amount":1,"Extra":true}]"#,
                "--dry-run",
            ],
        ),
        (
            "range mismatch",
            vec![
                "--table",
                "Sales",
                "--expect-range",
                "A1:B2",
                "--records",
                r#"[{"Region":"North","Amount":1}]"#,
                "--dry-run",
            ],
        ),
        (
            "missing expect range",
            vec![
                "--table",
                "Sales",
                "--records",
                r#"[{"Region":"North","Amount":1}]"#,
                "--dry-run",
            ],
        ),
    ] {
        let mut go_args = vec!["--json", "xlsx", "tables", "append-records", &go_in];
        go_args.extend(extra_args.iter().copied());
        let mut rust_args = vec!["--json", "xlsx", "tables", "append-records", &rust_in];
        rust_args.extend(extra_args.iter().copied());
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, go_code, "{label} exit");
        assert_eq!(rust_stdout, go_stdout, "{label} stdout");
        assert_eq!(rust_stderr, go_stderr, "{label} stderr");
    }

    let blank_go = temp_dir.join("blank-go.xlsx");
    let blank_rust = temp_dir.join("blank-rust.xlsx");
    write_table_xlsx_with_sheet_and_columns(&blank_go, "Data", ["", "Amount"]);
    write_table_xlsx_with_sheet_and_columns(&blank_rust, "Data", ["", "Amount"]);
    let blank_go = blank_go.to_string_lossy().to_string();
    let blank_rust = blank_rust.to_string_lossy().to_string();
    let blank_args = [
        "--json",
        "xlsx",
        "tables",
        "append-records",
        "--table",
        "Sales",
        "--expect-range",
        "A1:B3",
        "--records",
        r#"[{"Amount":1}]"#,
        "--dry-run",
    ];
    let mut go_args = blank_args[..4].to_vec();
    go_args.push(&blank_go);
    go_args.extend_from_slice(&blank_args[4..]);
    let mut rust_args = blank_args[..4].to_vec();
    rust_args.push(&blank_rust);
    rust_args.extend_from_slice(&blank_args[4..]);
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "blank table column exit");
    assert_eq!(rust_stdout, go_stdout, "blank table column stdout");
    assert_eq!(rust_stderr, go_stderr, "blank table column stderr");

    let duplicate_go = temp_dir.join("duplicate-go.xlsx");
    let duplicate_rust = temp_dir.join("duplicate-rust.xlsx");
    write_table_xlsx_with_sheet_and_columns(&duplicate_go, "Data", ["Region", "Region"]);
    write_table_xlsx_with_sheet_and_columns(&duplicate_rust, "Data", ["Region", "Region"]);
    let duplicate_go = duplicate_go.to_string_lossy().to_string();
    let duplicate_rust = duplicate_rust.to_string_lossy().to_string();
    let duplicate_records = r#"[{"Region":"North"}]"#;
    let duplicate_go_args = [
        "--json",
        "xlsx",
        "tables",
        "append-records",
        &duplicate_go,
        "--table",
        "Sales",
        "--expect-range",
        "A1:B3",
        "--records",
        duplicate_records,
        "--dry-run",
    ];
    let duplicate_rust_args = [
        "--json",
        "xlsx",
        "tables",
        "append-records",
        &duplicate_rust,
        "--table",
        "Sales",
        "--expect-range",
        "A1:B3",
        "--records",
        duplicate_records,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&duplicate_go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&duplicate_rust_args);
    assert_eq!(rust_code, go_code, "duplicate table column exit");
    assert_eq!(rust_stdout, go_stdout, "duplicate table column stdout");
    assert_eq!(rust_stderr, go_stderr, "duplicate table column stderr");

    let _ = fs::remove_dir_all(&temp_dir);
}
