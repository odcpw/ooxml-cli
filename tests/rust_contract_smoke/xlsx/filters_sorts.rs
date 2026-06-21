#[test]
fn xlsx_filters_sorts_show_matches_go_oracle() {
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
    ]);

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filters-show-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("filters.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &workbook,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c><c r="C1" t="inlineStr"><is><t>Status</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>15</v></c><c r="C2" t="inlineStr"><is><t>Open</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>South</t></is></c><c r="B3"><v>25</v></c><c r="C3" t="inlineStr"><is><t>Closed</t></is></c></row>
  </sheetData>
  <autoFilter ref="A1:C3">
    <filterColumn colId="0"><filters><filter val="North"/><filter val="South"/></filters></filterColumn>
    <filterColumn colId="1"><customFilters and="1"><customFilter operator="greaterThanOrEqual" val="10"/><customFilter operator="lessThanOrEqual" val="20"/></customFilters></filterColumn>
  </autoFilter>
  <sortState ref="A1:C3"><sortCondition descending="1" ref="B1:B3"/></sortState>
</worksheet>"#,
    );
    let workbook = workbook.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &workbook,
        "--sheet",
        "Sheet1",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_filters_sorts_set_autofilter_matches_go_oracle_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filters-set-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in_path).expect("go input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-autofilter",
        &go_in,
        "--sheet",
        "1",
        "--range",
        "A1:C1",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-autofilter",
        &rust_in,
        "--sheet",
        "1",
        "--range",
        "A1:C1",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "set-autofilter exit");
    assert_eq!(rust_stderr, go_stderr, "set-autofilter stderr");
    let rust_result = rust_stdout.expect("rust set-autofilter stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go set-autofilter stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "set-autofilter stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_result, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_result, "showCommand");

    let show_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &go_out,
        "--sheet",
        "1",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &rust_out,
        "--sheet",
        "1",
    ];
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "saved show exit");
    assert_eq!(rust_stderr, go_stderr, "saved show stderr");
    assert_eq!(
        scrub_path(rust_show.expect("rust saved show"), &rust_out, "[OUT]"),
        scrub_path(go_show.expect("go saved show"), &go_out, "[OUT]"),
        "saved show stdout"
    );
    assert!(
        read_zip_string(&rust_out_path, "xl/worksheets/sheet1.xml").contains(r#"ref="A1:C1""#),
        "Rust worksheet should contain autoFilter ref"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-autofilter",
        &go_in,
        "--sheet",
        "1",
        "--range",
        "A1:C1",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-autofilter",
        &rust_in,
        "--sheet",
        "1",
        "--range",
        "A1:C1",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "dry-run stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust dry-run"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go dry-run"), &go_in, "[IN]"),
        "dry-run stdout"
    );
    assert!(
        !read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml").contains("<autoFilter"),
        "dry-run should not mutate source workbook"
    );

    let bad_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-autofilter",
        &go_in,
        "--sheet",
        "1",
        "--range",
        "not-a-range",
        "--dry-run",
    ];
    let bad_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-autofilter",
        &rust_in,
        "--sheet",
        "1",
        "--range",
        "not-a-range",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&bad_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&bad_rust);
    assert_eq!(rust_code, go_code, "invalid range exit");
    assert_eq!(rust_stdout, go_stdout, "invalid range stdout");
    assert_eq!(rust_stderr, go_stderr, "invalid range stderr");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_filters_sorts_set_autofilter_on_table_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filters-table-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-table-in.xlsx");
    let rust_in_path = temp_dir.join("rust-table-in.xlsx");
    let go_out_path = temp_dir.join("go-table-out.xlsx");
    let rust_out_path = temp_dir.join("rust-table-out.xlsx");
    write_table_xlsx(&go_in_path);
    write_table_xlsx(&rust_in_path);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-autofilter",
        &go_in,
        "--table",
        "Sales",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-autofilter",
        &rust_in,
        "--table",
        "Sales",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "table set-autofilter exit");
    assert_eq!(rust_stderr, go_stderr, "table set-autofilter stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust table set-autofilter"),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go table set-autofilter"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "table set-autofilter stdout"
    );

    let show_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &go_out,
        "--table",
        "Sales",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &rust_out,
        "--table",
        "Sales",
    ];
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "table show exit");
    assert_eq!(rust_stderr, go_stderr, "table show stderr");
    assert_eq!(
        scrub_path(rust_show.expect("rust table show"), &rust_out, "[OUT]"),
        scrub_path(go_show.expect("go table show"), &go_out, "[OUT]"),
        "table show stdout"
    );
    assert!(
        read_zip_string(&rust_out_path, "xl/tables/table1.xml").contains(r#"ref="A1:B3""#),
        "Rust table autoFilter should retain table range"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_filters_sorts_clear_autofilter_matches_go_oracle_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filters-clear-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c><c r="C1" t="inlineStr"><is><t>Status</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>15</v></c><c r="C2" t="inlineStr"><is><t>Open</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>South</t></is></c><c r="B3"><v>25</v></c><c r="C3" t="inlineStr"><is><t>Closed</t></is></c></row>
  </sheetData>
  <autoFilter ref="A1:C3"><filterColumn colId="0"><filters><filter val="North"/></filters></filterColumn></autoFilter>
</worksheet>"#;
    write_simple_xlsx_with_sheet_xml(&go_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_in_path, sheet_xml);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-autofilter",
        &go_in,
        "--sheet",
        "1",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-autofilter",
        &rust_in,
        "--sheet",
        "1",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "clear-autofilter exit");
    assert_eq!(rust_stderr, go_stderr, "clear-autofilter stderr");
    let rust_result = rust_stdout.expect("rust clear-autofilter stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go clear-autofilter stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "clear-autofilter stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_result, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_result, "showCommand");

    let show_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &go_out,
        "--sheet",
        "1",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &rust_out,
        "--sheet",
        "1",
    ];
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "clear saved show exit");
    assert_eq!(rust_stderr, go_stderr, "clear saved show stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust clear saved show"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(go_show.expect("go clear saved show"), &go_out, "[OUT]"),
        "clear saved show stdout"
    );
    assert!(
        !read_zip_string(&rust_out_path, "xl/worksheets/sheet1.xml").contains("<autoFilter"),
        "Rust worksheet should not contain autoFilter after clear"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-autofilter",
        &go_in,
        "--sheet",
        "1",
        "--expect-range",
        "A1:C3",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-autofilter",
        &rust_in,
        "--sheet",
        "1",
        "--expect-range",
        "A1:C3",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "clear dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "clear dry-run stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust clear dry-run"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go clear dry-run"), &go_in, "[IN]"),
        "clear dry-run stdout"
    );
    assert!(
        read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml").contains("<autoFilter"),
        "clear dry-run should not mutate source workbook"
    );

    let no_filter_go = temp_dir.join("no-filter-go.xlsx");
    let no_filter_rust = temp_dir.join("no-filter-rust.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &no_filter_go,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>"#,
    );
    write_simple_xlsx_with_sheet_xml(
        &no_filter_rust,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>"#,
    );
    let no_filter_go = no_filter_go.to_string_lossy().to_string();
    let no_filter_rust = no_filter_rust.to_string_lossy().to_string();
    let bad_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-autofilter",
        &no_filter_go,
        "--sheet",
        "1",
        "--dry-run",
    ];
    let bad_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-autofilter",
        &no_filter_rust,
        "--sheet",
        "1",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&bad_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&bad_rust);
    assert_eq!(rust_code, go_code, "clear missing autoFilter exit");
    assert_eq!(rust_stdout, go_stdout, "clear missing autoFilter stdout");
    assert_eq!(
        scrub_path(
            rust_stderr.expect("rust clear missing autoFilter stderr"),
            &no_filter_rust,
            "[IN]"
        ),
        scrub_path(
            go_stderr.expect("go clear missing autoFilter stderr"),
            &no_filter_go,
            "[IN]"
        ),
        "clear missing autoFilter stderr"
    );

    let go_table_in = temp_dir.join("go-table-in.xlsx");
    let rust_table_in = temp_dir.join("rust-table-in.xlsx");
    let go_table_out = temp_dir.join("go-table-out.xlsx");
    let rust_table_out = temp_dir.join("rust-table-out.xlsx");
    write_table_xlsx(&go_table_in);
    write_table_xlsx(&rust_table_in);
    let go_table_in = go_table_in.to_string_lossy().to_string();
    let rust_table_in = rust_table_in.to_string_lossy().to_string();
    let go_table_out = go_table_out.to_string_lossy().to_string();
    let rust_table_out = rust_table_out.to_string_lossy().to_string();
    let table_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-autofilter",
        &go_table_in,
        "--table",
        "Sales",
        "--out",
        &go_table_out,
    ];
    let table_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-autofilter",
        &rust_table_in,
        "--table",
        "Sales",
        "--out",
        &rust_table_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&table_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&table_rust);
    assert_eq!(rust_code, go_code, "table clear-autofilter exit");
    assert_eq!(rust_stderr, go_stderr, "table clear-autofilter stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust table clear"),
            &[(&rust_table_in, "[IN]"), (&rust_table_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go table clear"),
            &[(&go_table_in, "[IN]"), (&go_table_out, "[OUT]")]
        ),
        "table clear-autofilter stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_filters_sorts_add_column_filter_matches_go_oracle_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filter-column-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c><c r="C1" t="inlineStr"><is><t>Status</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>15</v></c><c r="C2" t="inlineStr"><is><t>Open</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>South</t></is></c><c r="B3"><v>25</v></c><c r="C3" t="inlineStr"><is><t>Closed</t></is></c></row>
  </sheetData>
  <autoFilter ref="A1:C3"/>
</worksheet>"#;
    write_simple_xlsx_with_sheet_xml(&go_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_in_path, sheet_xml);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "add-column-filter",
        &go_in,
        "--sheet",
        "1",
        "--column",
        "0",
        "--values",
        "North,South,North",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "add-column-filter",
        &rust_in,
        "--sheet",
        "1",
        "--column",
        "0",
        "--values",
        "North,South,North",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "add-column-filter exit");
    assert_eq!(rust_stderr, go_stderr, "add-column-filter stderr");
    let rust_result = rust_stdout.expect("rust add-column-filter stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go add-column-filter stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "add-column-filter stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_result, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_result, "showCommand");

    let show_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &go_out,
        "--sheet",
        "1",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &rust_out,
        "--sheet",
        "1",
    ];
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "add saved show exit");
    assert_eq!(rust_stderr, go_stderr, "add saved show stderr");
    assert_eq!(
        scrub_path(rust_show.expect("rust add saved show"), &rust_out, "[OUT]"),
        scrub_path(go_show.expect("go add saved show"), &go_out, "[OUT]"),
        "add saved show stdout"
    );
    let sheet_xml = read_zip_string(&rust_out_path, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains(r#"<filter val="North"/>"#)
            && sheet_xml.contains(r#"<filter val="South"/>"#),
        "Rust worksheet should contain deduped filter values:\n{sheet_xml}"
    );

    let custom_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "add-column-filter",
        &go_out,
        "--sheet",
        "1",
        "--column",
        "1",
        "--custom-op",
        "between",
        "--custom-val1",
        "10",
        "--custom-val2",
        "20",
        "--dry-run",
    ];
    let custom_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "add-column-filter",
        &rust_out,
        "--sheet",
        "1",
        "--column",
        "1",
        "--custom-op",
        "between",
        "--custom-val1",
        "10",
        "--custom-val2",
        "20",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&custom_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&custom_rust);
    assert_eq!(rust_code, go_code, "custom dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "custom dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust custom dry-run"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(go_stdout.expect("go custom dry-run"), &go_out, "[OUT]"),
        "custom dry-run stdout"
    );
    assert!(
        !read_zip_string(&rust_out_path, "xl/worksheets/sheet1.xml").contains("customFilters"),
        "custom dry-run should not mutate saved workbook"
    );

    let no_filter_go = temp_dir.join("no-filter-go.xlsx");
    let no_filter_rust = temp_dir.join("no-filter-rust.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &no_filter_go,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>"#,
    );
    write_simple_xlsx_with_sheet_xml(
        &no_filter_rust,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>"#,
    );
    let no_filter_go = no_filter_go.to_string_lossy().to_string();
    let no_filter_rust = no_filter_rust.to_string_lossy().to_string();

    for (label, extra_args) in [
        (
            "missing autoFilter",
            vec![
                "--sheet",
                "1",
                "--column",
                "0",
                "--values",
                "North",
                "--dry-run",
            ],
        ),
        (
            "column out of bounds",
            vec![
                "--sheet",
                "1",
                "--column",
                "9",
                "--values",
                "North",
                "--dry-run",
            ],
        ),
        ("missing criteria", vec!["--sheet", "1", "--dry-run"]),
        (
            "expect filter mismatch",
            vec![
                "--sheet",
                "1",
                "--column",
                "0",
                "--values",
                "East",
                "--expect-filter",
                "none",
                "--dry-run",
            ],
        ),
    ] {
        let (go_file, rust_file) = if label == "missing autoFilter" {
            (&no_filter_go, &no_filter_rust)
        } else {
            (&go_out, &rust_out)
        };
        let mut go_args = vec![
            "--json",
            "xlsx",
            "filters-sorts",
            "add-column-filter",
            go_file,
        ];
        go_args.extend(extra_args.iter().copied());
        let mut rust_args = vec![
            "--json",
            "xlsx",
            "filters-sorts",
            "add-column-filter",
            rust_file,
        ];
        rust_args.extend(extra_args.iter().copied());
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, go_code, "{label} exit");
        assert_eq!(rust_stdout, go_stdout, "{label} stdout");
        assert_eq!(rust_stderr, go_stderr, "{label} stderr");
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_filters_sorts_clear_column_filter_matches_go_oracle_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filter-column-clear-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c><c r="C1" t="inlineStr"><is><t>Status</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>15</v></c><c r="C2" t="inlineStr"><is><t>Open</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>South</t></is></c><c r="B3"><v>25</v></c><c r="C3" t="inlineStr"><is><t>Closed</t></is></c></row>
  </sheetData>
  <autoFilter ref="A1:C3">
    <filterColumn colId="0"><filters><filter val="North"/></filters></filterColumn>
    <filterColumn colId="2"><filters><filter val="Open"/></filters></filterColumn>
  </autoFilter>
</worksheet>"#;
    write_simple_xlsx_with_sheet_xml(&go_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_in_path, sheet_xml);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-column-filter",
        &go_in,
        "--sheet",
        "1",
        "--column",
        "0",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-column-filter",
        &rust_in,
        "--sheet",
        "1",
        "--column",
        "0",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "clear-column-filter exit");
    assert_eq!(rust_stderr, go_stderr, "clear-column-filter stderr");
    let rust_result = rust_stdout.expect("rust clear-column-filter stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go clear-column-filter stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "clear-column-filter stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_result, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_result, "showCommand");

    let show_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &go_out,
        "--sheet",
        "1",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &rust_out,
        "--sheet",
        "1",
    ];
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "clear-column saved show exit");
    assert_eq!(rust_stderr, go_stderr, "clear-column saved show stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust clear-column saved show"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(
            go_show.expect("go clear-column saved show"),
            &go_out,
            "[OUT]"
        ),
        "clear-column saved show stdout"
    );
    let rust_sheet_xml = read_zip_string(&rust_out_path, "xl/worksheets/sheet1.xml");
    assert!(!rust_sheet_xml.contains(r#"colId="0""#));
    assert!(rust_sheet_xml.contains(r#"colId="2""#));

    let dry_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-column-filter",
        &go_out,
        "--sheet",
        "1",
        "--column",
        "2",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-column-filter",
        &rust_out,
        "--sheet",
        "1",
        "--column",
        "2",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "clear-column dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "clear-column dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust clear-column dry-run"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go clear-column dry-run"),
            &go_out,
            "[OUT]"
        ),
        "clear-column dry-run stdout"
    );
    assert!(
        read_zip_string(&rust_out_path, "xl/worksheets/sheet1.xml").contains(r#"colId="2""#),
        "dry-run should not remove column 2 filter"
    );

    let missing_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-column-filter",
        &go_out,
        "--sheet",
        "1",
        "--column",
        "1",
        "--dry-run",
    ];
    let missing_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-column-filter",
        &rust_out,
        "--sheet",
        "1",
        "--column",
        "1",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&missing_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_rust);
    assert_eq!(rust_code, go_code, "clear-column missing exit");
    assert_eq!(rust_stdout, go_stdout, "clear-column missing stdout");
    assert_eq!(rust_stderr, go_stderr, "clear-column missing stderr");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_filters_sorts_set_and_clear_sort_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filter-sort-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_sort1_path = temp_dir.join("go-sort1.xlsx");
    let rust_sort1_path = temp_dir.join("rust-sort1.xlsx");
    let go_sort2_path = temp_dir.join("go-sort2.xlsx");
    let rust_sort2_path = temp_dir.join("rust-sort2.xlsx");
    let go_cleared_path = temp_dir.join("go-cleared.xlsx");
    let rust_cleared_path = temp_dir.join("rust-cleared.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in_path).expect("go input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_sort1 = go_sort1_path.to_string_lossy().to_string();
    let rust_sort1 = rust_sort1_path.to_string_lossy().to_string();
    let go_sort2 = go_sort2_path.to_string_lossy().to_string();
    let rust_sort2 = rust_sort2_path.to_string_lossy().to_string();
    let go_cleared = go_cleared_path.to_string_lossy().to_string();
    let rust_cleared = rust_cleared_path.to_string_lossy().to_string();

    let set_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-sort",
        &go_in,
        "--sheet",
        "1",
        "--ref",
        "A1:C3",
        "--column",
        "A",
        "--out",
        &go_sort1,
    ];
    let set_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-sort",
        &rust_in,
        "--sheet",
        "1",
        "--ref",
        "A1:C3",
        "--column",
        "A",
        "--out",
        &rust_sort1,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&set_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&set_rust);
    assert_eq!(rust_code, go_code, "set-sort exit");
    assert_eq!(rust_stderr, go_stderr, "set-sort stderr");
    let rust_set = rust_stdout.expect("rust set-sort stdout");
    assert_eq!(
        scrub_paths(
            rust_set.clone(),
            &[(&rust_in, "[IN]"), (&rust_sort1, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go set-sort stdout"),
            &[(&go_in, "[IN]"), (&go_sort1, "[OUT]")]
        ),
        "set-sort stdout"
    );
    assert_eq!(rust_set["sortState"]["conditions"][0]["ref"], "A1:A3");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_set, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_set, "showCommand");

    let set2_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-sort",
        &go_sort1,
        "--sheet",
        "1",
        "--ref",
        "A1:C3",
        "--column",
        "B",
        "--descending",
        "--expect-sort",
        "A1:C3",
        "--out",
        &go_sort2,
    ];
    let set2_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-sort",
        &rust_sort1,
        "--sheet",
        "1",
        "--ref",
        "A1:C3",
        "--column",
        "B",
        "--descending",
        "--expect-sort",
        "A1:C3",
        "--out",
        &rust_sort2,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&set2_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&set2_rust);
    assert_eq!(rust_code, go_code, "second set-sort exit");
    assert_eq!(rust_stderr, go_stderr, "second set-sort stderr");
    let rust_set2 = rust_stdout.expect("rust second set-sort stdout");
    assert_eq!(
        scrub_paths(
            rust_set2.clone(),
            &[(&rust_sort1, "[IN]"), (&rust_sort2, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go second set-sort stdout"),
            &[(&go_sort1, "[IN]"), (&go_sort2, "[OUT]")]
        ),
        "second set-sort stdout"
    );
    assert_eq!(rust_set2["sortState"]["conditions"][1]["ref"], "B1:B3");
    assert_eq!(
        rust_set2["sortState"]["conditions"][1]["descending"],
        Value::Bool(true)
    );

    let show_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &go_sort2,
        "--sheet",
        "1",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &rust_sort2,
        "--sheet",
        "1",
    ];
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "sort saved show exit");
    assert_eq!(rust_stderr, go_stderr, "sort saved show stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust sort saved show"),
            &rust_sort2,
            "[OUT]"
        ),
        scrub_path(go_show.expect("go sort saved show"), &go_sort2, "[OUT]"),
        "sort saved show stdout"
    );

    let bad_expect_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-sort",
        &go_sort2,
        "--sheet",
        "1",
        "--ref",
        "A1:C3",
        "--column",
        "C",
        "--expect-sort",
        "A1:B2",
        "--dry-run",
    ];
    let bad_expect_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-sort",
        &rust_sort2,
        "--sheet",
        "1",
        "--ref",
        "A1:C3",
        "--column",
        "C",
        "--expect-sort",
        "A1:B2",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&bad_expect_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&bad_expect_rust);
    assert_eq!(rust_code, go_code, "set-sort bad expect exit");
    assert_eq!(rust_stdout, go_stdout, "set-sort bad expect stdout");
    assert_eq!(rust_stderr, go_stderr, "set-sort bad expect stderr");

    let clear_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-sort",
        &go_sort2,
        "--sheet",
        "1",
        "--out",
        &go_cleared,
    ];
    let clear_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-sort",
        &rust_sort2,
        "--sheet",
        "1",
        "--out",
        &rust_cleared,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&clear_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&clear_rust);
    assert_eq!(rust_code, go_code, "clear-sort exit");
    assert_eq!(rust_stderr, go_stderr, "clear-sort stderr");
    let rust_clear = rust_stdout.expect("rust clear-sort stdout");
    assert_eq!(
        scrub_paths(
            rust_clear.clone(),
            &[(&rust_sort2, "[IN]"), (&rust_cleared, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go clear-sort stdout"),
            &[(&go_sort2, "[IN]"), (&go_cleared, "[OUT]")]
        ),
        "clear-sort stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_clear, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_clear, "showCommand");

    let show_cleared_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &go_cleared,
        "--sheet",
        "1",
    ];
    let show_cleared_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &rust_cleared,
        "--sheet",
        "1",
    ];
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_cleared_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_cleared_rust);
    assert_eq!(rust_code, go_code, "clear-sort saved show exit");
    assert_eq!(rust_stderr, go_stderr, "clear-sort saved show stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust clear-sort saved show"),
            &rust_cleared,
            "[OUT]"
        ),
        scrub_path(
            go_show.expect("go clear-sort saved show"),
            &go_cleared,
            "[OUT]"
        ),
        "clear-sort saved show stdout"
    );
    assert!(
        !read_zip_string(&rust_cleared_path, "xl/worksheets/sheet1.xml").contains("sortState"),
        "clear-sort should remove sortState"
    );

    let no_sort_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-sort",
        &go_in,
        "--sheet",
        "1",
        "--dry-run",
    ];
    let no_sort_rust = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-sort",
        &rust_in,
        "--sheet",
        "1",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&no_sort_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&no_sort_rust);
    assert_eq!(rust_code, go_code, "clear-sort missing exit");
    assert_eq!(rust_stdout, go_stdout, "clear-sort missing stdout");
    assert_eq!(rust_stderr, go_stderr, "clear-sort missing stderr");

    let _ = fs::remove_dir_all(&temp_dir);
}
