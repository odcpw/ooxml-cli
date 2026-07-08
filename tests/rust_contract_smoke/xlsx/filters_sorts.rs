#[test]
fn xlsx_filters_sorts_show_matches_rust_baseline() {
    assert_rust_baseline_match(&[
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
    assert_rust_baseline_match(&[
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
fn xlsx_filters_sorts_set_autofilter_matches_rust_baseline_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filters-set-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let baseline_out_path = temp_dir.join("baseline-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &baseline_in_path).expect("baseline input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let baseline_out = baseline_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let baseline_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-autofilter",
        &baseline_in,
        "--sheet",
        "1",
        "--range",
        "A1:C1",
        "--out",
        &baseline_out,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "set-autofilter exit");
    assert_eq!(rust_stderr, baseline_stderr, "set-autofilter stderr");
    let rust_result = rust_stdout.expect("rust set-autofilter stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline set-autofilter stdout"),
            &[(&baseline_in, "[IN]"), (&baseline_out, "[OUT]")]
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
        &baseline_out,
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
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "saved show exit");
    assert_eq!(rust_stderr, baseline_stderr, "saved show stderr");
    assert_eq!(
        scrub_path(rust_show.expect("rust saved show"), &rust_out, "[OUT]"),
        scrub_path(baseline_show.expect("baseline saved show"), &baseline_out, "[OUT]"),
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
        &baseline_in,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, baseline_code, "dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "dry-run stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust dry-run"), &rust_in, "[IN]"),
        scrub_path(baseline_stdout.expect("baseline dry-run"), &baseline_in, "[IN]"),
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
        &baseline_in,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&bad_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&bad_rust);
    assert_eq!(rust_code, baseline_code, "invalid range exit");
    assert_eq!(rust_stdout, baseline_stdout, "invalid range stdout");
    assert_eq!(rust_stderr, baseline_stderr, "invalid range stderr");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_filters_sorts_set_autofilter_on_table_matches_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filters-table-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-table-in.xlsx");
    let rust_in_path = temp_dir.join("rust-table-in.xlsx");
    let baseline_out_path = temp_dir.join("baseline-table-out.xlsx");
    let rust_out_path = temp_dir.join("rust-table-out.xlsx");
    write_table_xlsx(&baseline_in_path);
    write_table_xlsx(&rust_in_path);
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let baseline_out = baseline_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let baseline_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-autofilter",
        &baseline_in,
        "--table",
        "Sales",
        "--out",
        &baseline_out,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "table set-autofilter exit");
    assert_eq!(rust_stderr, baseline_stderr, "table set-autofilter stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust table set-autofilter"),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline table set-autofilter"),
            &[(&baseline_in, "[IN]"), (&baseline_out, "[OUT]")]
        ),
        "table set-autofilter stdout"
    );

    let show_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        &baseline_out,
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
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "table show exit");
    assert_eq!(rust_stderr, baseline_stderr, "table show stderr");
    assert_eq!(
        scrub_path(rust_show.expect("rust table show"), &rust_out, "[OUT]"),
        scrub_path(baseline_show.expect("baseline table show"), &baseline_out, "[OUT]"),
        "table show stdout"
    );
    assert!(
        read_zip_string(&rust_out_path, "xl/tables/table1.xml").contains(r#"ref="A1:B3""#),
        "Rust table autoFilter should retain table range"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_filters_sorts_clear_autofilter_matches_rust_baseline_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filters-clear-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let baseline_out_path = temp_dir.join("baseline-out.xlsx");
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
    write_simple_xlsx_with_sheet_xml(&baseline_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_in_path, sheet_xml);
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let baseline_out = baseline_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let baseline_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-autofilter",
        &baseline_in,
        "--sheet",
        "1",
        "--out",
        &baseline_out,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "clear-autofilter exit");
    assert_eq!(rust_stderr, baseline_stderr, "clear-autofilter stderr");
    let rust_result = rust_stdout.expect("rust clear-autofilter stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline clear-autofilter stdout"),
            &[(&baseline_in, "[IN]"), (&baseline_out, "[OUT]")]
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
        &baseline_out,
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
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "clear saved show exit");
    assert_eq!(rust_stderr, baseline_stderr, "clear saved show stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust clear saved show"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(baseline_show.expect("baseline clear saved show"), &baseline_out, "[OUT]"),
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
        &baseline_in,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, baseline_code, "clear dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "clear dry-run stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust clear dry-run"), &rust_in, "[IN]"),
        scrub_path(baseline_stdout.expect("baseline clear dry-run"), &baseline_in, "[IN]"),
        "clear dry-run stdout"
    );
    assert!(
        read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml").contains("<autoFilter"),
        "clear dry-run should not mutate source workbook"
    );

    let no_filter_go = temp_dir.join("no-filter-baseline.xlsx");
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&bad_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&bad_rust);
    assert_eq!(rust_code, baseline_code, "clear missing autoFilter exit");
    assert_eq!(rust_stdout, baseline_stdout, "clear missing autoFilter stdout");
    assert_eq!(
        scrub_path(
            rust_stderr.expect("rust clear missing autoFilter stderr"),
            &no_filter_rust,
            "[IN]"
        ),
        scrub_path(
            baseline_stderr.expect("baseline clear missing autoFilter stderr"),
            &no_filter_go,
            "[IN]"
        ),
        "clear missing autoFilter stderr"
    );

    let baseline_table_in = temp_dir.join("baseline-table-in.xlsx");
    let rust_table_in = temp_dir.join("rust-table-in.xlsx");
    let baseline_table_out = temp_dir.join("baseline-table-out.xlsx");
    let rust_table_out = temp_dir.join("rust-table-out.xlsx");
    write_table_xlsx(&baseline_table_in);
    write_table_xlsx(&rust_table_in);
    let baseline_table_in = baseline_table_in.to_string_lossy().to_string();
    let rust_table_in = rust_table_in.to_string_lossy().to_string();
    let baseline_table_out = baseline_table_out.to_string_lossy().to_string();
    let rust_table_out = rust_table_out.to_string_lossy().to_string();
    let table_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-autofilter",
        &baseline_table_in,
        "--table",
        "Sales",
        "--out",
        &baseline_table_out,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&table_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&table_rust);
    assert_eq!(rust_code, baseline_code, "table clear-autofilter exit");
    assert_eq!(rust_stderr, baseline_stderr, "table clear-autofilter stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust table clear"),
            &[(&rust_table_in, "[IN]"), (&rust_table_out, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline table clear"),
            &[(&baseline_table_in, "[IN]"), (&baseline_table_out, "[OUT]")]
        ),
        "table clear-autofilter stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_filters_sorts_add_column_filter_matches_rust_baseline_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filter-column-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let baseline_out_path = temp_dir.join("baseline-out.xlsx");
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
    write_simple_xlsx_with_sheet_xml(&baseline_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_in_path, sheet_xml);
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let baseline_out = baseline_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let baseline_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "add-column-filter",
        &baseline_in,
        "--sheet",
        "1",
        "--column",
        "0",
        "--values",
        "North,South,North",
        "--out",
        &baseline_out,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "add-column-filter exit");
    assert_eq!(rust_stderr, baseline_stderr, "add-column-filter stderr");
    let rust_result = rust_stdout.expect("rust add-column-filter stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline add-column-filter stdout"),
            &[(&baseline_in, "[IN]"), (&baseline_out, "[OUT]")]
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
        &baseline_out,
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
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "add saved show exit");
    assert_eq!(rust_stderr, baseline_stderr, "add saved show stderr");
    assert_eq!(
        scrub_path(rust_show.expect("rust add saved show"), &rust_out, "[OUT]"),
        scrub_path(baseline_show.expect("baseline add saved show"), &baseline_out, "[OUT]"),
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
        &baseline_out,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&custom_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&custom_rust);
    assert_eq!(rust_code, baseline_code, "custom dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "custom dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust custom dry-run"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(baseline_stdout.expect("baseline custom dry-run"), &baseline_out, "[OUT]"),
        "custom dry-run stdout"
    );
    assert!(
        !read_zip_string(&rust_out_path, "xl/worksheets/sheet1.xml").contains("customFilters"),
        "custom dry-run should not mutate saved workbook"
    );

    let no_filter_go = temp_dir.join("no-filter-baseline.xlsx");
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
        let (baseline_file, rust_file) = if label == "missing autoFilter" {
            (&no_filter_go, &no_filter_rust)
        } else {
            (&baseline_out, &rust_out)
        };
        let mut baseline_args = vec![
            "--json",
            "xlsx",
            "filters-sorts",
            "add-column-filter",
            baseline_file,
        ];
        baseline_args.extend(extra_args.iter().copied());
        let mut rust_args = vec![
            "--json",
            "xlsx",
            "filters-sorts",
            "add-column-filter",
            rust_file,
        ];
        rust_args.extend(extra_args.iter().copied());
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, baseline_code, "{label} exit");
        assert_eq!(rust_stdout, baseline_stdout, "{label} stdout");
        assert_eq!(rust_stderr, baseline_stderr, "{label} stderr");
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_filters_sorts_clear_column_filter_matches_rust_baseline_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filter-column-clear-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let baseline_out_path = temp_dir.join("baseline-out.xlsx");
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
    write_simple_xlsx_with_sheet_xml(&baseline_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_in_path, sheet_xml);
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let baseline_out = baseline_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let baseline_args = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-column-filter",
        &baseline_in,
        "--sheet",
        "1",
        "--column",
        "0",
        "--out",
        &baseline_out,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "clear-column-filter exit");
    assert_eq!(rust_stderr, baseline_stderr, "clear-column-filter stderr");
    let rust_result = rust_stdout.expect("rust clear-column-filter stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline clear-column-filter stdout"),
            &[(&baseline_in, "[IN]"), (&baseline_out, "[OUT]")]
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
        &baseline_out,
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
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "clear-column saved show exit");
    assert_eq!(rust_stderr, baseline_stderr, "clear-column saved show stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust clear-column saved show"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(
            baseline_show.expect("baseline clear-column saved show"),
            &baseline_out,
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
        &baseline_out,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, baseline_code, "clear-column dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "clear-column dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust clear-column dry-run"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline clear-column dry-run"),
            &baseline_out,
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
        &baseline_out,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&missing_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_rust);
    assert_eq!(rust_code, baseline_code, "clear-column missing exit");
    assert_eq!(rust_stdout, baseline_stdout, "clear-column missing stdout");
    assert_eq!(rust_stderr, baseline_stderr, "clear-column missing stderr");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_filters_sorts_set_and_clear_sort_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-filter-sort-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let baseline_sort1_path = temp_dir.join("baseline-sort1.xlsx");
    let rust_sort1_path = temp_dir.join("rust-sort1.xlsx");
    let baseline_sort2_path = temp_dir.join("baseline-sort2.xlsx");
    let rust_sort2_path = temp_dir.join("rust-sort2.xlsx");
    let baseline_cleared_path = temp_dir.join("baseline-cleared.xlsx");
    let rust_cleared_path = temp_dir.join("rust-cleared.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &baseline_in_path).expect("baseline input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let baseline_sort1 = baseline_sort1_path.to_string_lossy().to_string();
    let rust_sort1 = rust_sort1_path.to_string_lossy().to_string();
    let baseline_sort2 = baseline_sort2_path.to_string_lossy().to_string();
    let rust_sort2 = rust_sort2_path.to_string_lossy().to_string();
    let baseline_cleared = baseline_cleared_path.to_string_lossy().to_string();
    let rust_cleared = rust_cleared_path.to_string_lossy().to_string();

    let set_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-sort",
        &baseline_in,
        "--sheet",
        "1",
        "--ref",
        "A1:C3",
        "--column",
        "A",
        "--out",
        &baseline_sort1,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&set_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&set_rust);
    assert_eq!(rust_code, baseline_code, "set-sort exit");
    assert_eq!(rust_stderr, baseline_stderr, "set-sort stderr");
    let rust_set = rust_stdout.expect("rust set-sort stdout");
    assert_eq!(
        scrub_paths(
            rust_set.clone(),
            &[(&rust_in, "[IN]"), (&rust_sort1, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline set-sort stdout"),
            &[(&baseline_in, "[IN]"), (&baseline_sort1, "[OUT]")]
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
        &baseline_sort1,
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
        &baseline_sort2,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&set2_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&set2_rust);
    assert_eq!(rust_code, baseline_code, "second set-sort exit");
    assert_eq!(rust_stderr, baseline_stderr, "second set-sort stderr");
    let rust_set2 = rust_stdout.expect("rust second set-sort stdout");
    assert_eq!(
        scrub_paths(
            rust_set2.clone(),
            &[(&rust_sort1, "[IN]"), (&rust_sort2, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline second set-sort stdout"),
            &[(&baseline_sort1, "[IN]"), (&baseline_sort2, "[OUT]")]
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
        &baseline_sort2,
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
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "sort saved show exit");
    assert_eq!(rust_stderr, baseline_stderr, "sort saved show stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust sort saved show"),
            &rust_sort2,
            "[OUT]"
        ),
        scrub_path(baseline_show.expect("baseline sort saved show"), &baseline_sort2, "[OUT]"),
        "sort saved show stdout"
    );

    let bad_expect_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "set-sort",
        &baseline_sort2,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&bad_expect_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&bad_expect_rust);
    assert_eq!(rust_code, baseline_code, "set-sort bad expect exit");
    assert_eq!(rust_stdout, baseline_stdout, "set-sort bad expect stdout");
    assert_eq!(rust_stderr, baseline_stderr, "set-sort bad expect stderr");

    let clear_go = [
        "--json",
        "xlsx",
        "filters-sorts",
        "clear-sort",
        &baseline_sort2,
        "--sheet",
        "1",
        "--out",
        &baseline_cleared,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&clear_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&clear_rust);
    assert_eq!(rust_code, baseline_code, "clear-sort exit");
    assert_eq!(rust_stderr, baseline_stderr, "clear-sort stderr");
    let rust_clear = rust_stdout.expect("rust clear-sort stdout");
    assert_eq!(
        scrub_paths(
            rust_clear.clone(),
            &[(&rust_sort2, "[IN]"), (&rust_cleared, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline clear-sort stdout"),
            &[(&baseline_sort2, "[IN]"), (&baseline_cleared, "[OUT]")]
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
        &baseline_cleared,
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
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_cleared_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_cleared_rust);
    assert_eq!(rust_code, baseline_code, "clear-sort saved show exit");
    assert_eq!(rust_stderr, baseline_stderr, "clear-sort saved show stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust clear-sort saved show"),
            &rust_cleared,
            "[OUT]"
        ),
        scrub_path(
            baseline_show.expect("baseline clear-sort saved show"),
            &baseline_cleared,
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
        &baseline_in,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&no_sort_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&no_sort_rust);
    assert_eq!(rust_code, baseline_code, "clear-sort missing exit");
    assert_eq!(rust_stdout, baseline_stdout, "clear-sort missing stdout");
    assert_eq!(rust_stderr, baseline_stderr, "clear-sort missing stderr");

    let _ = fs::remove_dir_all(&temp_dir);
}
