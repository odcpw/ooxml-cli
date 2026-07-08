// XLSX command-family parity tests live in a child module to keep this integration
// test crate navigable while preserving the shared oracle/fixture helpers above.
use super::*;

include!("xlsx/scaffold.rs");
include!("xlsx/ranges_cells.rs");

#[test]
fn xlsx_colwidths_show_matches_rust_baseline() {
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "colwidths",
        "show",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "A:C",
    ]);

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-colwidths-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("widths.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &workbook,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetFormatPr defaultColWidth="11"/>
  <cols>
    <col min="2" max="3" width="18.5" customWidth="1"/>
    <col min="4" max="4" width="0" hidden="1"/>
  </cols>
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>"#,
    );
    let workbook = workbook.to_string_lossy().to_string();
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "colwidths",
        "show",
        &workbook,
        "--sheet",
        "Sheet1",
        "--range",
        "D:A",
    ]);
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "colwidths",
        "show",
        &workbook,
        "--sheet",
        "1",
        "--range",
        "A1",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_rowheights_show_matches_rust_baseline() {
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "rowheights",
        "show",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "1:3",
    ]);

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-rowheights-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("heights.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &workbook,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetFormatPr defaultRowHeight="18"/>
  <sheetData>
    <row r="1"><c r="A1"><v>1</v></c></row>
    <row r="2" ht="22.5" customHeight="1"><c r="A2"><v>2</v></c></row>
    <row r="4" ht="0" hidden="1"/>
    <row r="5" customHeight="1"/>
  </sheetData>
</worksheet>"#,
    );
    let workbook = workbook.to_string_lossy().to_string();
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "rowheights",
        "show",
        &workbook,
        "--sheet",
        "Sheet1",
        "--range",
        "5:2",
    ]);
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "rowheights",
        "show",
        &workbook,
        "--sheet",
        "1",
        "--range",
        "2:bad",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

include!("xlsx/charts.rs");
include!("xlsx/conditional_formatting.rs");

fn assert_xlsx_structure_command_matches(
    label: &str,
    baseline_args: &[&str],
    rust_args: &[&str],
    replacements: &[(&str, &str)],
) -> Value {
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, baseline_code, "{label} exit");
    assert_eq!(rust_stderr, baseline_stderr, "{label} stderr");
    let rust_result = rust_stdout.expect("rust xlsx structure stdout");
    assert_eq!(
        scrub_paths(rust_result.clone(), replacements),
        scrub_paths(
            baseline_stdout.unwrap_or_else(|| panic!("baseline xlsx structure stdout for {label}")),
            replacements
        ),
        "{label} stdout"
    );
    rust_result
}

fn assert_xlsx_structure_saved_readback(
    label: &str,
    baseline_out: &str,
    rust_out: &str,
    readback_range: &str,
) {
    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", rust_out]);
    assert_eq!(validate_code, 0, "{label} strict validate exit");
    assert_eq!(validate_stderr, None, "{label} strict validate stderr");
    assert!(
        validate_stdout.is_some(),
        "{label} strict validate should emit JSON"
    );

    for (readback_label, baseline_args, rust_args) in [
        (
            "sheet show",
            vec![
                "--json",
                "xlsx",
                "sheets",
                "show",
                baseline_out,
                "--sheet",
                "Sheet1",
            ],
            vec![
                "--json", "xlsx", "sheets", "show", rust_out, "--sheet", "Sheet1",
            ],
        ),
        (
            "range export",
            vec![
                "--json",
                "xlsx",
                "ranges",
                "export",
                baseline_out,
                "--sheet",
                "Sheet1",
                "--range",
                readback_range,
                "--include-types",
            ],
            vec![
                "--json",
                "xlsx",
                "ranges",
                "export",
                rust_out,
                "--sheet",
                "Sheet1",
                "--range",
                readback_range,
                "--include-types",
            ],
        ),
    ] {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml_baseline(&rust_args);
        assert_eq!(rust_code, baseline_code, "{label} {readback_label} exit");
        assert_eq!(
            rust_stderr, baseline_stderr,
            "{label} {readback_label} stderr"
        );
        assert_eq!(
            scrub_path(
                rust_stdout.unwrap_or_else(|| {
                    panic!("rust xlsx structure saved {readback_label} stdout")
                }),
                rust_out,
                "[OUT]"
            ),
            scrub_path(
                baseline_stdout.unwrap_or_else(|| {
                    panic!("baseline xlsx structure saved {readback_label} stdout")
                }),
                baseline_out,
                "[OUT]"
            ),
            "{label} {readback_label}"
        );
    }
}

#[test]
fn xlsx_structure_mutations_match_rust_baseline_saved_readback_and_dry_run() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-structure-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let base_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:D4"/>
  <sheetData>
    <row r="1"><c r="A1" t="str"><v>r1a</v></c><c r="C1"><v>13</v></c></row>
    <row r="2"><c r="B2"><v>22</v></c><c r="D2"><v>24</v></c></row>
    <row r="4"><c r="A4"><v>41</v></c><c r="D4"><v>44</v></c></row>
  </sheetData>
</worksheet>"#;

    for (label, family, action, position_flag, position_value, count, range) in [
        ("rows insert", "rows", "insert", "--at", "2", "2", "A1:D6"),
        ("rows delete", "rows", "delete", "--row", "2", "1", "A1:D3"),
        ("cols insert", "cols", "insert", "--at", "B", "2", "A1:F4"),
        ("cols delete", "cols", "delete", "--col", "B", "1", "A1:C4"),
    ] {
        let baseline_in_path = temp_dir.join(format!("baseline-{family}-{action}-in.xlsx"));
        let rust_in_path = temp_dir.join(format!("rust-{family}-{action}-in.xlsx"));
        let baseline_out_path = temp_dir.join(format!("baseline-{family}-{action}-out.xlsx"));
        let rust_out_path = temp_dir.join(format!("rust-{family}-{action}-out.xlsx"));
        write_simple_xlsx_with_sheet_xml(&baseline_in_path, base_xml);
        write_simple_xlsx_with_sheet_xml(&rust_in_path, base_xml);
        let baseline_in = baseline_in_path.to_string_lossy().to_string();
        let rust_in = rust_in_path.to_string_lossy().to_string();
        let baseline_out = baseline_out_path.to_string_lossy().to_string();
        let rust_out = rust_out_path.to_string_lossy().to_string();
        let replacements = [
            (rust_in.as_str(), "[IN]"),
            (rust_out.as_str(), "[OUT]"),
            (baseline_in.as_str(), "[IN]"),
            (baseline_out.as_str(), "[OUT]"),
        ];

        let baseline_args = [
            "--json",
            "xlsx",
            family,
            action,
            &baseline_in,
            "--sheet",
            "Sheet1",
            position_flag,
            position_value,
            "--count",
            count,
            "--out",
            &baseline_out,
        ];
        let rust_args = [
            "--json",
            "xlsx",
            family,
            action,
            &rust_in,
            "--sheet",
            "Sheet1",
            position_flag,
            position_value,
            "--count",
            count,
            "--out",
            &rust_out,
        ];
        let rust_result =
            assert_xlsx_structure_command_matches(label, &baseline_args, &rust_args, &replacements);
        assert_rust_emitted_ooxml_command_exits_zero(&rust_result, "validateCommand");
        assert_rust_emitted_ooxml_command_succeeds(&rust_result, "sheetShowCommand");
        assert_rust_emitted_ooxml_command_succeeds(&rust_result, "sheetsListCommand");
        assert_xlsx_structure_saved_readback(label, &baseline_out, &rust_out, range);
    }

    let baseline_dry_in_path = temp_dir.join("baseline-rows-dry-in.xlsx");
    let rust_dry_in_path = temp_dir.join("rust-rows-dry-in.xlsx");
    write_simple_xlsx_with_sheet_xml(&baseline_dry_in_path, base_xml);
    write_simple_xlsx_with_sheet_xml(&rust_dry_in_path, base_xml);
    let before_rows = read_zip_string(&rust_dry_in_path, "xl/worksheets/sheet1.xml");
    let baseline_dry_in = baseline_dry_in_path.to_string_lossy().to_string();
    let rust_dry_in = rust_dry_in_path.to_string_lossy().to_string();
    let baseline_dry = [
        "--json",
        "xlsx",
        "rows",
        "insert",
        &baseline_dry_in,
        "--sheet",
        "Sheet1",
        "--at",
        "3",
        "--count",
        "2",
        "--dry-run",
    ];
    let rust_dry = [
        "--json",
        "xlsx",
        "rows",
        "insert",
        &rust_dry_in,
        "--sheet",
        "Sheet1",
        "--at",
        "3",
        "--count",
        "2",
        "--dry-run",
    ];
    assert_xlsx_structure_command_matches(
        "rows insert dry-run",
        &baseline_dry,
        &rust_dry,
        &[
            (rust_dry_in.as_str(), "[IN]"),
            (baseline_dry_in.as_str(), "[IN]"),
        ],
    );
    assert_eq!(
        read_zip_string(&rust_dry_in_path, "xl/worksheets/sheet1.xml"),
        before_rows,
        "rows insert dry-run should not mutate source workbook"
    );

    let baseline_col_dry_in_path = temp_dir.join("baseline-cols-dry-in.xlsx");
    let rust_col_dry_in_path = temp_dir.join("rust-cols-dry-in.xlsx");
    write_simple_xlsx_with_sheet_xml(&baseline_col_dry_in_path, base_xml);
    write_simple_xlsx_with_sheet_xml(&rust_col_dry_in_path, base_xml);
    let before_cols = read_zip_string(&rust_col_dry_in_path, "xl/worksheets/sheet1.xml");
    let baseline_col_dry_in = baseline_col_dry_in_path.to_string_lossy().to_string();
    let rust_col_dry_in = rust_col_dry_in_path.to_string_lossy().to_string();
    let baseline_col_dry = [
        "--json",
        "xlsx",
        "cols",
        "delete",
        &baseline_col_dry_in,
        "--sheet",
        "Sheet1",
        "--col",
        "C",
        "--count",
        "1",
        "--dry-run",
    ];
    let rust_col_dry = [
        "--json",
        "xlsx",
        "cols",
        "delete",
        &rust_col_dry_in,
        "--sheet",
        "Sheet1",
        "--col",
        "C",
        "--count",
        "1",
        "--dry-run",
    ];
    assert_xlsx_structure_command_matches(
        "cols delete dry-run",
        &baseline_col_dry,
        &rust_col_dry,
        &[
            (rust_col_dry_in.as_str(), "[IN]"),
            (baseline_col_dry_in.as_str(), "[IN]"),
        ],
    );
    assert_eq!(
        read_zip_string(&rust_col_dry_in_path, "xl/worksheets/sheet1.xml"),
        before_cols,
        "cols delete dry-run should not mutate source workbook"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_structure_mutation_errors_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-structure-errors-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let clean_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData>
    <row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c></row>
    <row r="2"><c r="A2"><v>3</v></c><c r="B2"><v>4</v></c></row>
  </sheetData>
</worksheet>"#;
    let baseline_clean_path = temp_dir.join("baseline-clean.xlsx");
    let rust_clean_path = temp_dir.join("rust-clean.xlsx");
    write_simple_xlsx_with_sheet_xml(&baseline_clean_path, clean_xml);
    write_simple_xlsx_with_sheet_xml(&rust_clean_path, clean_xml);
    let baseline_clean = baseline_clean_path.to_string_lossy().to_string();
    let rust_clean = rust_clean_path.to_string_lossy().to_string();

    for (label, baseline_bad, rust_bad) in [
        (
            "missing sheet",
            vec![
                "--json",
                "xlsx",
                "rows",
                "insert",
                &baseline_clean,
                "--at",
                "1",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "rows",
                "insert",
                &rust_clean,
                "--at",
                "1",
                "--dry-run",
            ],
        ),
        (
            "row zero",
            vec![
                "--json",
                "xlsx",
                "rows",
                "insert",
                &baseline_clean,
                "--sheet",
                "Sheet1",
                "--at",
                "0",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "rows",
                "insert",
                &rust_clean,
                "--sheet",
                "Sheet1",
                "--at",
                "0",
                "--dry-run",
            ],
        ),
        (
            "count zero",
            vec![
                "--json",
                "xlsx",
                "rows",
                "delete",
                &baseline_clean,
                "--sheet",
                "Sheet1",
                "--row",
                "1",
                "--count",
                "0",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "rows",
                "delete",
                &rust_clean,
                "--sheet",
                "Sheet1",
                "--row",
                "1",
                "--count",
                "0",
                "--dry-run",
            ],
        ),
        (
            "missing workbook sheet",
            vec![
                "--json",
                "xlsx",
                "cols",
                "insert",
                &baseline_clean,
                "--sheet",
                "Missing",
                "--at",
                "A",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "cols",
                "insert",
                &rust_clean,
                "--sheet",
                "Missing",
                "--at",
                "A",
                "--dry-run",
            ],
        ),
        (
            "bad column reference",
            vec![
                "--json",
                "xlsx",
                "cols",
                "insert",
                &baseline_clean,
                "--sheet",
                "Sheet1",
                "--at",
                "A1",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "cols",
                "insert",
                &rust_clean,
                "--sheet",
                "Sheet1",
                "--at",
                "A1",
                "--dry-run",
            ],
        ),
        (
            "column out of bounds",
            vec![
                "--json",
                "xlsx",
                "cols",
                "delete",
                &baseline_clean,
                "--sheet",
                "Sheet1",
                "--col",
                "XFE",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "cols",
                "delete",
                &rust_clean,
                "--sheet",
                "Sheet1",
                "--col",
                "XFE",
                "--dry-run",
            ],
        ),
        (
            "column span out of bounds",
            vec![
                "--json",
                "xlsx",
                "cols",
                "insert",
                &baseline_clean,
                "--sheet",
                "Sheet1",
                "--at",
                "XFD",
                "--count",
                "2",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "cols",
                "insert",
                &rust_clean,
                "--sheet",
                "Sheet1",
                "--at",
                "XFD",
                "--count",
                "2",
                "--dry-run",
            ],
        ),
    ] {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, baseline_code, "{label} exit");
        assert_eq!(rust_stdout, baseline_stdout, "{label} stdout");
        assert_eq!(
            scrub_path(
                rust_stderr.unwrap_or_else(|| panic!("rust structure error stderr for {label}")),
                &rust_clean,
                "[IN]"
            ),
            scrub_path(
                baseline_stderr
                    .unwrap_or_else(|| panic!("baseline structure error stderr for {label}")),
                &baseline_clean,
                "[IN]"
            ),
            "{label} stderr"
        );
    }

    for (label, sheet_xml, family, action, position_flag, position_value) in [
        (
            "formula guard",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><f>B1</f><v>2</v></c><c r="B1"><v>2</v></c></row></sheetData>
</worksheet>"#,
            "rows",
            "insert",
            "--at",
            "1",
        ),
        (
            "merged cell guard",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
  <mergeCells count="1"><mergeCell ref="A1:B1"/></mergeCells>
</worksheet>"#,
            "rows",
            "delete",
            "--row",
            "1",
        ),
        (
            "table guard",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
  <tableParts count="1"><tablePart r:id="rIdTable1"/></tableParts>
</worksheet>"#,
            "rows",
            "insert",
            "--at",
            "1",
        ),
        (
            "column metadata guard",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cols><col min="1" max="1" width="20" customWidth="1"/></cols>
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>"#,
            "cols",
            "insert",
            "--at",
            "A",
        ),
        (
            "invalid row references guard",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="2"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>"#,
            "rows",
            "insert",
            "--at",
            "1",
        ),
    ] {
        let baseline_path = temp_dir.join(format!("baseline-{label}.xlsx").replace(' ', "-"));
        let rust_path = temp_dir.join(format!("rust-{label}.xlsx").replace(' ', "-"));
        write_simple_xlsx_with_sheet_xml(&baseline_path, sheet_xml);
        write_simple_xlsx_with_sheet_xml(&rust_path, sheet_xml);
        let baseline_file = baseline_path.to_string_lossy().to_string();
        let rust_file = rust_path.to_string_lossy().to_string();
        let baseline_bad = [
            "--json",
            "xlsx",
            family,
            action,
            &baseline_file,
            "--sheet",
            "Sheet1",
            position_flag,
            position_value,
            "--dry-run",
        ];
        let rust_bad = [
            "--json",
            "xlsx",
            family,
            action,
            &rust_file,
            "--sheet",
            "Sheet1",
            position_flag,
            position_value,
            "--dry-run",
        ];
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, baseline_code, "{label} exit");
        assert_eq!(rust_stdout, baseline_stdout, "{label} stdout");
        assert_eq!(
            scrub_path(
                rust_stderr.unwrap_or_else(|| panic!("rust structure guard stderr for {label}")),
                &rust_file,
                "[IN]"
            ),
            scrub_path(
                baseline_stderr
                    .unwrap_or_else(|| panic!("baseline structure guard stderr for {label}")),
                &baseline_file,
                "[IN]"
            ),
            "{label} stderr"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

include!("xlsx/data_validations.rs");

#[test]
fn xlsx_dimension_setters_match_rust_baseline_saved_readback_dry_run_and_errors() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-dim-set-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let baseline_cols_in_path = temp_dir.join("baseline-cols-in.xlsx");
    let rust_cols_in_path = temp_dir.join("rust-cols-in.xlsx");
    let baseline_cols_out_path = temp_dir.join("baseline-cols-out.xlsx");
    let rust_cols_out_path = temp_dir.join("rust-cols-out.xlsx");
    let cols_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetFormatPr defaultColWidth="11"/>
  <cols>
    <col min="2" max="4" width="30" customWidth="1" hidden="1" style="3"/>
    <col min="7" max="7" width="9" customWidth="1"/>
  </cols>
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>"#;
    write_simple_xlsx_with_sheet_xml(&baseline_cols_in_path, cols_xml);
    write_simple_xlsx_with_sheet_xml(&rust_cols_in_path, cols_xml);
    let baseline_cols_in = baseline_cols_in_path.to_string_lossy().to_string();
    let rust_cols_in = rust_cols_in_path.to_string_lossy().to_string();
    let baseline_cols_out = baseline_cols_out_path.to_string_lossy().to_string();
    let rust_cols_out = rust_cols_out_path.to_string_lossy().to_string();

    let baseline_args = [
        "--json",
        "xlsx",
        "colwidths",
        "set",
        &baseline_cols_in,
        "--sheet",
        "Sheet1",
        "--range",
        "C:E",
        "--width",
        "12.5",
        "--expect-width",
        "30",
        "--out",
        &baseline_cols_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "colwidths",
        "set",
        &rust_cols_in,
        "--sheet",
        "Sheet1",
        "--range",
        "C:E",
        "--width",
        "12.5",
        "--expect-width",
        "30",
        "--out",
        &rust_cols_out,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "colwidths set exit");
    assert_eq!(rust_stderr, baseline_stderr, "colwidths set stderr");
    let rust_result = rust_stdout.expect("rust colwidths set stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_cols_in, "[IN]"), (&rust_cols_out, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline colwidths set stdout"),
            &[(&baseline_cols_in, "[IN]"), (&baseline_cols_out, "[OUT]")]
        ),
        "colwidths set stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_result, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_result, "colwidthsShowCommand");

    let col_show_go = [
        "--json",
        "xlsx",
        "colwidths",
        "show",
        &baseline_cols_out,
        "--sheet",
        "Sheet1",
        "--range",
        "B:E",
    ];
    let col_show_rust = [
        "--json",
        "xlsx",
        "colwidths",
        "show",
        &rust_cols_out,
        "--sheet",
        "Sheet1",
        "--range",
        "B:E",
    ];
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&col_show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml_baseline(&col_show_rust);
    assert_eq!(rust_code, baseline_code, "colwidths saved readback exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "colwidths saved readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show.expect("rust colwidths saved readback"),
            &rust_cols_out,
            "[OUT]"
        ),
        scrub_path(
            baseline_show.expect("baseline colwidths saved readback"),
            &baseline_cols_out,
            "[OUT]"
        ),
        "colwidths saved readback"
    );

    let before_cols = read_zip_string(&rust_cols_in_path, "xl/worksheets/sheet1.xml");
    let dry_go = [
        "--json",
        "xlsx",
        "colwidths",
        "set",
        &baseline_cols_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A:A",
        "--width",
        "20.25",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "colwidths",
        "set",
        &rust_cols_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A:A",
        "--width",
        "20.25",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, baseline_code, "colwidths dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "colwidths dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust colwidths dry-run stdout"),
            &rust_cols_in,
            "[IN]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline colwidths dry-run stdout"),
            &baseline_cols_in,
            "[IN]"
        ),
        "colwidths dry-run stdout"
    );
    assert_eq!(
        read_zip_string(&rust_cols_in_path, "xl/worksheets/sheet1.xml"),
        before_cols,
        "colwidths dry-run should not mutate source workbook"
    );

    for (label, baseline_bad, rust_bad) in [
        (
            "missing width",
            vec![
                "--json",
                "xlsx",
                "colwidths",
                "set",
                &baseline_cols_in,
                "--sheet",
                "Sheet1",
                "--range",
                "A:A",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "colwidths",
                "set",
                &rust_cols_in,
                "--sheet",
                "Sheet1",
                "--range",
                "A:A",
                "--dry-run",
            ],
        ),
        (
            "width out of range",
            vec![
                "--json",
                "xlsx",
                "colwidths",
                "set",
                &baseline_cols_in,
                "--sheet",
                "Sheet1",
                "--range",
                "A:A",
                "--width",
                "999",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "colwidths",
                "set",
                &rust_cols_in,
                "--sheet",
                "Sheet1",
                "--range",
                "A:A",
                "--width",
                "999",
                "--dry-run",
            ],
        ),
        (
            "expect width mismatch",
            vec![
                "--json",
                "xlsx",
                "colwidths",
                "set",
                &baseline_cols_in,
                "--sheet",
                "Sheet1",
                "--range",
                "A:A",
                "--width",
                "13",
                "--expect-width",
                "99",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "colwidths",
                "set",
                &rust_cols_in,
                "--sheet",
                "Sheet1",
                "--range",
                "A:A",
                "--width",
                "13",
                "--expect-width",
                "99",
                "--dry-run",
            ],
        ),
    ] {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, baseline_code, "colwidths {label} exit");
        assert_eq!(rust_stdout, baseline_stdout, "colwidths {label} stdout");
        assert_eq!(
            scrub_path(
                rust_stderr.expect("rust colwidths bad stderr"),
                &rust_cols_in,
                "[IN]"
            ),
            scrub_path(
                baseline_stderr.expect("baseline colwidths bad stderr"),
                &baseline_cols_in,
                "[IN]"
            ),
            "colwidths {label} stderr"
        );
    }

    let baseline_rows_in_path = temp_dir.join("baseline-rows-in.xlsx");
    let rust_rows_in_path = temp_dir.join("rust-rows-in.xlsx");
    let baseline_rows_out_path = temp_dir.join("baseline-rows-out.xlsx");
    let rust_rows_out_path = temp_dir.join("rust-rows-out.xlsx");
    let rows_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetFormatPr defaultRowHeight="17"/>
  <sheetData>
    <row r="1"><c r="A1"><v>1</v></c></row>
    <row r="3" ht="18" customHeight="1" hidden="1" spans="1:2"><c r="A3"><v>3</v></c></row>
    <row r="5"><c r="A5"><v>5</v></c></row>
  </sheetData>
</worksheet>"#;
    write_simple_xlsx_with_sheet_xml(&baseline_rows_in_path, rows_xml);
    write_simple_xlsx_with_sheet_xml(&rust_rows_in_path, rows_xml);
    let baseline_rows_in = baseline_rows_in_path.to_string_lossy().to_string();
    let rust_rows_in = rust_rows_in_path.to_string_lossy().to_string();
    let baseline_rows_out = baseline_rows_out_path.to_string_lossy().to_string();
    let rust_rows_out = rust_rows_out_path.to_string_lossy().to_string();

    let baseline_args = [
        "--json",
        "xlsx",
        "rowheights",
        "set",
        &baseline_rows_in,
        "--sheet",
        "Sheet1",
        "--range",
        "2:4",
        "--height",
        "24.5",
        "--expect-height",
        "17",
        "--out",
        &baseline_rows_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "rowheights",
        "set",
        &rust_rows_in,
        "--sheet",
        "Sheet1",
        "--range",
        "2:4",
        "--height",
        "24.5",
        "--expect-height",
        "17",
        "--out",
        &rust_rows_out,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "rowheights set exit");
    assert_eq!(rust_stderr, baseline_stderr, "rowheights set stderr");
    let rust_result = rust_stdout.expect("rust rowheights set stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_rows_in, "[IN]"), (&rust_rows_out, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline rowheights set stdout"),
            &[(&baseline_rows_in, "[IN]"), (&baseline_rows_out, "[OUT]")]
        ),
        "rowheights set stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_result, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_result, "rowheightsShowCommand");

    let row_show_go = [
        "--json",
        "xlsx",
        "rowheights",
        "show",
        &baseline_rows_out,
        "--sheet",
        "Sheet1",
        "--range",
        "2:4",
    ];
    let row_show_rust = [
        "--json",
        "xlsx",
        "rowheights",
        "show",
        &rust_rows_out,
        "--sheet",
        "Sheet1",
        "--range",
        "2:4",
    ];
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&row_show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml_baseline(&row_show_rust);
    assert_eq!(rust_code, baseline_code, "rowheights saved readback exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "rowheights saved readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show.expect("rust rowheights saved readback"),
            &rust_rows_out,
            "[OUT]"
        ),
        scrub_path(
            baseline_show.expect("baseline rowheights saved readback"),
            &baseline_rows_out,
            "[OUT]"
        ),
        "rowheights saved readback"
    );

    let before_rows = read_zip_string(&rust_rows_in_path, "xl/worksheets/sheet1.xml");
    let dry_go = [
        "--json",
        "xlsx",
        "rowheights",
        "set",
        &baseline_rows_in,
        "--sheet",
        "Sheet1",
        "--range",
        "1:1",
        "--height",
        "19.25",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "rowheights",
        "set",
        &rust_rows_in,
        "--sheet",
        "Sheet1",
        "--range",
        "1:1",
        "--height",
        "19.25",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, baseline_code, "rowheights dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "rowheights dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust rowheights dry-run stdout"),
            &rust_rows_in,
            "[IN]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline rowheights dry-run stdout"),
            &baseline_rows_in,
            "[IN]"
        ),
        "rowheights dry-run stdout"
    );
    assert_eq!(
        read_zip_string(&rust_rows_in_path, "xl/worksheets/sheet1.xml"),
        before_rows,
        "rowheights dry-run should not mutate source workbook"
    );

    for (label, baseline_bad, rust_bad) in [
        (
            "missing height",
            vec![
                "--json",
                "xlsx",
                "rowheights",
                "set",
                &baseline_rows_in,
                "--sheet",
                "Sheet1",
                "--range",
                "1:1",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "rowheights",
                "set",
                &rust_rows_in,
                "--sheet",
                "Sheet1",
                "--range",
                "1:1",
                "--dry-run",
            ],
        ),
        (
            "height out of range",
            vec![
                "--json",
                "xlsx",
                "rowheights",
                "set",
                &baseline_rows_in,
                "--sheet",
                "Sheet1",
                "--range",
                "1:1",
                "--height",
                "500",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "rowheights",
                "set",
                &rust_rows_in,
                "--sheet",
                "Sheet1",
                "--range",
                "1:1",
                "--height",
                "500",
                "--dry-run",
            ],
        ),
        (
            "expect height mismatch",
            vec![
                "--json",
                "xlsx",
                "rowheights",
                "set",
                &baseline_rows_in,
                "--sheet",
                "Sheet1",
                "--range",
                "1:1",
                "--height",
                "18",
                "--expect-height",
                "99",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "rowheights",
                "set",
                &rust_rows_in,
                "--sheet",
                "Sheet1",
                "--range",
                "1:1",
                "--height",
                "18",
                "--expect-height",
                "99",
                "--dry-run",
            ],
        ),
    ] {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, baseline_code, "rowheights {label} exit");
        assert_eq!(rust_stdout, baseline_stdout, "rowheights {label} stdout");
        assert_eq!(
            scrub_path(
                rust_stderr.expect("rust rowheights bad stderr"),
                &rust_rows_in,
                "[IN]"
            ),
            scrub_path(
                baseline_stderr.expect("baseline rowheights bad stderr"),
                &baseline_rows_in,
                "[IN]"
            ),
            "rowheights {label} stderr"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

include!("xlsx/filters_sorts.rs");

#[test]
fn xlsx_comments_add_update_remove_matches_rust_baseline_and_saved_output() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-comments-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let baseline_added_path = temp_dir.join("baseline-added.xlsx");
    let rust_added_path = temp_dir.join("rust-added.xlsx");
    let baseline_updated_path = temp_dir.join("baseline-updated.xlsx");
    let rust_updated_path = temp_dir.join("rust-updated.xlsx");
    let baseline_removed_path = temp_dir.join("baseline-removed.xlsx");
    let rust_removed_path = temp_dir.join("rust-removed.xlsx");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &baseline_in_path,
    )
    .expect("baseline input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let baseline_added = baseline_added_path.to_string_lossy().to_string();
    let rust_added = rust_added_path.to_string_lossy().to_string();
    let baseline_updated = baseline_updated_path.to_string_lossy().to_string();
    let rust_updated = rust_updated_path.to_string_lossy().to_string();
    let baseline_removed = baseline_removed_path.to_string_lossy().to_string();
    let rust_removed = rust_removed_path.to_string_lossy().to_string();

    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "comments",
        "list",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
    ]);

    let add_go = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &baseline_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "C3",
        "--author",
        "Ann",
        "--text",
        "before",
        "--out",
        &baseline_added,
    ];
    let add_rust = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "C3",
        "--author",
        "Ann",
        "--text",
        "before",
        "--out",
        &rust_added,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&add_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&add_rust);
    assert_eq!(rust_code, baseline_code, "comments add exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments add stderr");
    let rust_add = rust_stdout.expect("rust comments add stdout");
    assert_eq!(
        scrub_paths(
            rust_add.clone(),
            &[(&rust_in, "[IN]"), (&rust_added, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline comments add stdout"),
            &[(&baseline_in, "[IN]"), (&baseline_added, "[OUT]")]
        ),
        "comments add stdout"
    );
    assert_eq!(rust_add["handle"], "H:xlsx/ws:1/comment:a:C3");
    assert_eq!(rust_add["createdPart"], Value::Bool(true));
    assert_eq!(rust_add["createdRef"], Value::Bool(true));
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "listCommand");

    assert!(zip_entry_exists(&rust_added_path, "xl/comments1.xml"));
    assert!(zip_entry_exists(
        &rust_added_path,
        "xl/drawings/vmlDrawing1.vml"
    ));
    let content_types = read_zip_string(&rust_added_path, "[Content_Types].xml");
    assert!(
        content_types
            .contains("application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml"),
        "missing comments content type:\n{content_types}"
    );
    assert!(
        content_types.contains("application/vnd.openxmlformats-officedocument.vmlDrawing"),
        "missing VML content type:\n{content_types}"
    );
    let sheet_rels = read_zip_string(&rust_added_path, "xl/worksheets/_rels/sheet1.xml.rels");
    assert!(
        sheet_rels.contains("/comments") && sheet_rels.contains("/vmlDrawing"),
        "worksheet rels missing comment/VML links:\n{sheet_rels}"
    );
    let sheet_xml = read_zip_string(&rust_added_path, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains("<legacyDrawing"),
        "worksheet missing legacyDrawing:\n{sheet_xml}"
    );

    let list_go = [
        "--json",
        "xlsx",
        "comments",
        "list",
        &baseline_added,
        "--sheet",
        "Sheet1",
    ];
    let list_rust = [
        "--json",
        "xlsx",
        "comments",
        "list",
        &rust_added,
        "--sheet",
        "Sheet1",
    ];
    let (baseline_code, baseline_list, baseline_stderr) = run_ooxml_baseline(&list_go);
    let (rust_code, rust_list, rust_stderr) = run_ooxml(&list_rust);
    assert_eq!(rust_code, baseline_code, "saved comments list exit");
    assert_eq!(rust_stderr, baseline_stderr, "saved comments list stderr");
    assert_eq!(
        scrub_path(
            rust_list.expect("rust saved comments list"),
            &rust_added,
            "[OUT]"
        ),
        scrub_path(
            baseline_list.expect("baseline saved comments list"),
            &baseline_added,
            "[OUT]"
        ),
        "saved comments list"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &baseline_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "D4",
        "--author",
        "Dry",
        "--text",
        "preview",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "D4",
        "--author",
        "Dry",
        "--text",
        "preview",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, baseline_code, "comments add dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments add dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust comments add dry-run"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline comments add dry-run"),
            &baseline_in,
            "[IN]"
        ),
        "comments add dry-run stdout"
    );
    assert!(
        !zip_entry_exists(&rust_in_path, "xl/comments1.xml"),
        "dry-run wrote comments part into Rust input"
    );

    let expect_hash = rust_add["contentHash"].as_str().expect("add hash");
    let update_go = [
        "--json",
        "xlsx",
        "comments",
        "update",
        &baseline_added,
        "--handle",
        "H:xlsx/ws:1/comment:a:C3",
        "--author",
        "Ben",
        "--text",
        "after",
        "--expect-hash",
        expect_hash,
        "--out",
        &baseline_updated,
    ];
    let update_rust = [
        "--json",
        "xlsx",
        "comments",
        "update",
        &rust_added,
        "--handle",
        "H:xlsx/ws:1/comment:a:C3",
        "--author",
        "Ben",
        "--text",
        "after",
        "--expect-hash",
        expect_hash,
        "--out",
        &rust_updated,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&update_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&update_rust);
    assert_eq!(rust_code, baseline_code, "comments update exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments update stderr");
    let rust_update = rust_stdout.expect("rust comments update stdout");
    assert_eq!(
        scrub_paths(
            rust_update.clone(),
            &[(&rust_added, "[IN]"), (&rust_updated, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline comments update stdout"),
            &[(&baseline_added, "[IN]"), (&baseline_updated, "[OUT]")]
        ),
        "comments update stdout"
    );
    assert_eq!(rust_update["previousText"], "before");
    assert_eq!(rust_update["author"], "Ben");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_update, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "listCommand");

    let stale_go = [
        "--json",
        "xlsx",
        "comments",
        "update",
        &baseline_added,
        "--comment-id",
        "0",
        "--text",
        "bad",
        "--expect-hash",
        "sha256:wrong",
        "--dry-run",
    ];
    let stale_rust = [
        "--json",
        "xlsx",
        "comments",
        "update",
        &rust_added,
        "--comment-id",
        "0",
        "--text",
        "bad",
        "--expect-hash",
        "sha256:wrong",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&stale_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&stale_rust);
    assert_eq!(rust_code, baseline_code, "comments update stale-hash exit");
    assert_eq!(
        rust_stdout, baseline_stdout,
        "comments update stale-hash stdout"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "comments update stale-hash stderr"
    );

    let duplicate_go = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &baseline_added,
        "--sheet",
        "Sheet1",
        "--cell",
        "C3",
        "--author",
        "Ann",
        "--text",
        "duplicate",
        "--dry-run",
    ];
    let duplicate_rust = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &rust_added,
        "--sheet",
        "Sheet1",
        "--cell",
        "C3",
        "--author",
        "Ann",
        "--text",
        "duplicate",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&duplicate_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&duplicate_rust);
    assert_eq!(rust_code, baseline_code, "comments duplicate add exit");
    assert_eq!(
        rust_stdout, baseline_stdout,
        "comments duplicate add stdout"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "comments duplicate add stderr"
    );

    let text_file_path = temp_dir.join("comment.txt");
    fs::write(&text_file_path, "from file").expect("comment text file");
    let text_file = text_file_path.to_string_lossy().to_string();
    let text_conflict_go = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &baseline_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "E5",
        "--author",
        "Ann",
        "--text",
        "inline",
        "--text-file",
        &text_file,
        "--dry-run",
    ];
    let text_conflict_rust = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "E5",
        "--author",
        "Ann",
        "--text",
        "inline",
        "--text-file",
        &text_file,
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&text_conflict_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&text_conflict_rust);
    assert_eq!(rust_code, baseline_code, "comments text conflict exit");
    assert_eq!(
        rust_stdout, baseline_stdout,
        "comments text conflict stdout"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "comments text conflict stderr"
    );

    let missing_go = [
        "--json",
        "xlsx",
        "comments",
        "list",
        &baseline_added,
        "--sheet",
        "Sheet1",
        "--comment-id",
        "9",
    ];
    let missing_rust = [
        "--json",
        "xlsx",
        "comments",
        "list",
        &rust_added,
        "--sheet",
        "Sheet1",
        "--comment-id",
        "9",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&missing_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_rust);
    assert_eq!(rust_code, baseline_code, "comments missing list exit");
    assert_eq!(rust_stdout, baseline_stdout, "comments missing list stdout");
    assert_eq!(rust_stderr, baseline_stderr, "comments missing list stderr");

    let updated_hash = rust_update["contentHash"].as_str().expect("updated hash");
    let remove_go = [
        "--json",
        "xlsx",
        "comments",
        "remove",
        &baseline_updated,
        "--sheet",
        "Sheet1",
        "--comment-id",
        "0",
        "--expect-hash",
        updated_hash,
        "--out",
        &baseline_removed,
    ];
    let remove_rust = [
        "--json",
        "xlsx",
        "comments",
        "remove",
        &rust_updated,
        "--sheet",
        "Sheet1",
        "--comment-id",
        "0",
        "--expect-hash",
        updated_hash,
        "--out",
        &rust_removed,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&remove_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&remove_rust);
    assert_eq!(rust_code, baseline_code, "comments remove exit");
    assert_eq!(rust_stderr, baseline_stderr, "comments remove stderr");
    let rust_remove = rust_stdout.expect("rust comments remove stdout");
    assert_eq!(
        scrub_paths(
            rust_remove.clone(),
            &[(&rust_updated, "[IN]"), (&rust_removed, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline comments remove stdout"),
            &[(&baseline_updated, "[IN]"), (&baseline_removed, "[OUT]")]
        ),
        "comments remove stdout"
    );
    assert_eq!(rust_remove["previousAuthor"], "Ben");
    assert_eq!(rust_remove["previousText"], "after");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_remove, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_remove, "listCommand");

    let (baseline_code, baseline_list, baseline_stderr) =
        run_ooxml_baseline(&["--json", "xlsx", "comments", "list", &baseline_removed]);
    let (rust_code, rust_list, rust_stderr) =
        run_ooxml(&["--json", "xlsx", "comments", "list", &rust_removed]);
    assert_eq!(rust_code, baseline_code, "removed comments list exit");
    assert_eq!(rust_stderr, baseline_stderr, "removed comments list stderr");
    assert_eq!(
        scrub_path(
            rust_list.expect("rust removed comments list"),
            &rust_removed,
            "[OUT]"
        ),
        scrub_path(
            baseline_list.expect("baseline removed comments list"),
            &baseline_removed,
            "[OUT]"
        ),
        "removed comments list"
    );
    assert!(!zip_entry_exists(&rust_removed_path, "xl/comments1.xml"));
    assert!(!zip_entry_exists(
        &rust_removed_path,
        "xl/drawings/vmlDrawing1.vml"
    ));
    let removed_sheet = read_zip_string(&rust_removed_path, "xl/worksheets/sheet1.xml");
    assert!(
        !removed_sheet.contains("<legacyDrawing"),
        "remove-last left legacyDrawing:\n{removed_sheet}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

fn assert_xlsx_strict_valid(path: &str) {
    let (code, stdout, stderr) = run_ooxml(&["--json", "--strict", "validate", path]);
    assert_eq!(code, 0, "strict validate exit for {path}");
    assert_eq!(stderr, None, "strict validate stderr for {path}");
    assert_eq!(
        stdout.expect("strict validate stdout")["valid"],
        Value::Bool(true),
        "strict validate result for {path}"
    );
}

fn assert_rust_baseline_match_scrubbed(
    label: &str,
    baseline_args: &[&str],
    rust_args: &[&str],
    replacements: &[(&str, &str)],
) -> Option<Value> {
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, baseline_code, "{label} exit");
    assert_eq!(
        rust_stderr
            .clone()
            .map(|value| scrub_paths(value, replacements)),
        baseline_stderr.map(|value| scrub_paths(value, replacements)),
        "{label} stderr"
    );
    assert_eq!(
        rust_stdout
            .clone()
            .map(|value| scrub_paths(value, replacements)),
        baseline_stdout.map(|value| scrub_paths(value, replacements)),
        "{label} stdout"
    );
    rust_stdout
}

include!("xlsx/pivots.rs");

include!("xlsx/hyperlinks.rs");

include!("xlsx/workbook_metadata.rs");

include!("xlsx/sheets.rs");

fn write_sheet_lifecycle_xlsx(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create sheet lifecycle xlsx");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet3.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/workbook.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <bookViews><workbookView activeTab="2" firstSheet="0"/></bookViews>
  <sheets>
    <sheet name="Summary" sheetId="1" r:id="rId1"/>
    <sheet name="Data" sheetId="2" r:id="rId2"/>
    <sheet name="Tail" sheetId="3" r:id="rId3"/>
  </sheets>
</workbook>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet3.xml"/>
</Relationships>"#,
    );
    for sheet_number in 1..=3 {
        write_zip_string(
            &mut writer,
            options,
            &format!("xl/worksheets/sheet{sheet_number}.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
</worksheet>"#,
        );
    }
    writer.finish().expect("finish sheet lifecycle xlsx");
}

fn assert_xlsx_sheet_mutation_matches_rust_baseline(
    label: &str,
    baseline_args: &[&str],
    rust_args: &[&str],
    baseline_paths: &[(&str, &str)],
    rust_paths: &[(&str, &str)],
) -> Value {
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, baseline_code, "{label} exit");
    assert_eq!(rust_stderr, baseline_stderr, "{label} stderr");
    let rust_value = rust_stdout.expect("rust sheet mutation stdout");
    assert_eq!(
        scrub_paths(rust_value.clone(), rust_paths),
        scrub_paths(
            baseline_stdout.expect("baseline sheet mutation stdout"),
            baseline_paths
        ),
        "{label} stdout"
    );
    rust_value
}

fn assert_xlsx_sheet_error_matches_rust_baseline(
    label: &str,
    baseline_args: &[&str],
    rust_args: &[&str],
) {
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, baseline_code, "{label} exit");
    assert_eq!(rust_stdout, baseline_stdout, "{label} stdout");
    assert_eq!(rust_stderr, baseline_stderr, "{label} stderr");
}

fn normalize_xlsx_dynamic_sheet_id(value: Value, sheet_name: &str) -> Value {
    let Some(sheet_id) = find_sheet_id_for_name(&value, sheet_name) else {
        return value;
    };
    replace_json_string(value, &sheet_id, "[DYNAMIC_SHEET_ID]")
}

fn find_sheet_id_for_name(value: &Value, sheet_name: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if map.get("name").and_then(Value::as_str) == Some(sheet_name)
                && let Some(sheet_id) = map.get("sheetId").and_then(Value::as_str)
            {
                return Some(sheet_id.to_string());
            }
            map.values()
                .find_map(|child| find_sheet_id_for_name(child, sheet_name))
        }
        Value::Array(items) => items
            .iter()
            .find_map(|child| find_sheet_id_for_name(child, sheet_name)),
        _ => None,
    }
}

fn replace_json_string(value: Value, from: &str, to: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace(from, to)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| replace_json_string(item, from, to))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, replace_json_string(value, from, to)))
                .collect(),
        ),
        other => other,
    }
}

include!("xlsx/names.rs");

include!("xlsx/tables.rs");
