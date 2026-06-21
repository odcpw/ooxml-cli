// XLSX command-family parity tests live in a child module to keep this integration
// test crate navigable while preserving the shared oracle/fixture helpers above.
use super::*;

include!("xlsx/ranges_cells.rs");

#[test]
fn xlsx_colwidths_show_matches_go_oracle() {
    assert_go_rust_match(&[
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
    assert_go_rust_match(&[
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
    assert_go_rust_match(&[
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
fn xlsx_rowheights_show_matches_go_oracle() {
    assert_go_rust_match(&[
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
    assert_go_rust_match(&[
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
    assert_go_rust_match(&[
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

fn assert_xlsx_structure_command_matches(
    label: &str,
    go_args: &[&str],
    rust_args: &[&str],
    replacements: &[(&str, &str)],
) -> Value {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, go_code, "{label} exit");
    assert_eq!(rust_stderr, go_stderr, "{label} stderr");
    let rust_result = rust_stdout.expect("rust xlsx structure stdout");
    assert_eq!(
        scrub_paths(rust_result.clone(), replacements),
        scrub_paths(
            go_stdout.unwrap_or_else(|| panic!("go xlsx structure stdout for {label}")),
            replacements
        ),
        "{label} stdout"
    );
    rust_result
}

fn assert_xlsx_structure_saved_readback(
    label: &str,
    go_out: &str,
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

    for (readback_label, go_args, rust_args) in [
        (
            "sheet show",
            vec![
                "--json", "xlsx", "sheets", "show", go_out, "--sheet", "Sheet1",
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
                go_out,
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
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_go_ooxml(&rust_args);
        assert_eq!(rust_code, go_code, "{label} {readback_label} exit");
        assert_eq!(rust_stderr, go_stderr, "{label} {readback_label} stderr");
        assert_eq!(
            scrub_path(
                rust_stdout.unwrap_or_else(|| {
                    panic!("rust xlsx structure saved {readback_label} stdout")
                }),
                rust_out,
                "[OUT]"
            ),
            scrub_path(
                go_stdout.unwrap_or_else(|| {
                    panic!("go xlsx structure saved {readback_label} stdout")
                }),
                go_out,
                "[OUT]"
            ),
            "{label} {readback_label}"
        );
    }
}

#[test]
fn xlsx_structure_mutations_match_go_oracle_saved_readback_and_dry_run() {
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
        let go_in_path = temp_dir.join(format!("go-{family}-{action}-in.xlsx"));
        let rust_in_path = temp_dir.join(format!("rust-{family}-{action}-in.xlsx"));
        let go_out_path = temp_dir.join(format!("go-{family}-{action}-out.xlsx"));
        let rust_out_path = temp_dir.join(format!("rust-{family}-{action}-out.xlsx"));
        write_simple_xlsx_with_sheet_xml(&go_in_path, base_xml);
        write_simple_xlsx_with_sheet_xml(&rust_in_path, base_xml);
        let go_in = go_in_path.to_string_lossy().to_string();
        let rust_in = rust_in_path.to_string_lossy().to_string();
        let go_out = go_out_path.to_string_lossy().to_string();
        let rust_out = rust_out_path.to_string_lossy().to_string();
        let replacements = [
            (rust_in.as_str(), "[IN]"),
            (rust_out.as_str(), "[OUT]"),
            (go_in.as_str(), "[IN]"),
            (go_out.as_str(), "[OUT]"),
        ];

        let go_args = [
            "--json",
            "xlsx",
            family,
            action,
            &go_in,
            "--sheet",
            "Sheet1",
            position_flag,
            position_value,
            "--count",
            count,
            "--out",
            &go_out,
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
            assert_xlsx_structure_command_matches(label, &go_args, &rust_args, &replacements);
        assert_rust_emitted_ooxml_command_exits_zero(&rust_result, "validateCommand");
        assert_rust_emitted_ooxml_command_succeeds(&rust_result, "sheetShowCommand");
        assert_rust_emitted_ooxml_command_succeeds(&rust_result, "sheetsListCommand");
        assert_xlsx_structure_saved_readback(label, &go_out, &rust_out, range);
    }

    let go_dry_in_path = temp_dir.join("go-rows-dry-in.xlsx");
    let rust_dry_in_path = temp_dir.join("rust-rows-dry-in.xlsx");
    write_simple_xlsx_with_sheet_xml(&go_dry_in_path, base_xml);
    write_simple_xlsx_with_sheet_xml(&rust_dry_in_path, base_xml);
    let before_rows = read_zip_string(&rust_dry_in_path, "xl/worksheets/sheet1.xml");
    let go_dry_in = go_dry_in_path.to_string_lossy().to_string();
    let rust_dry_in = rust_dry_in_path.to_string_lossy().to_string();
    let go_dry = [
        "--json",
        "xlsx",
        "rows",
        "insert",
        &go_dry_in,
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
        &go_dry,
        &rust_dry,
        &[(rust_dry_in.as_str(), "[IN]"), (go_dry_in.as_str(), "[IN]")],
    );
    assert_eq!(
        read_zip_string(&rust_dry_in_path, "xl/worksheets/sheet1.xml"),
        before_rows,
        "rows insert dry-run should not mutate source workbook"
    );

    let go_col_dry_in_path = temp_dir.join("go-cols-dry-in.xlsx");
    let rust_col_dry_in_path = temp_dir.join("rust-cols-dry-in.xlsx");
    write_simple_xlsx_with_sheet_xml(&go_col_dry_in_path, base_xml);
    write_simple_xlsx_with_sheet_xml(&rust_col_dry_in_path, base_xml);
    let before_cols = read_zip_string(&rust_col_dry_in_path, "xl/worksheets/sheet1.xml");
    let go_col_dry_in = go_col_dry_in_path.to_string_lossy().to_string();
    let rust_col_dry_in = rust_col_dry_in_path.to_string_lossy().to_string();
    let go_col_dry = [
        "--json",
        "xlsx",
        "cols",
        "delete",
        &go_col_dry_in,
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
        &go_col_dry,
        &rust_col_dry,
        &[
            (rust_col_dry_in.as_str(), "[IN]"),
            (go_col_dry_in.as_str(), "[IN]"),
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
fn xlsx_structure_mutation_errors_match_go_oracle() {
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
    let go_clean_path = temp_dir.join("go-clean.xlsx");
    let rust_clean_path = temp_dir.join("rust-clean.xlsx");
    write_simple_xlsx_with_sheet_xml(&go_clean_path, clean_xml);
    write_simple_xlsx_with_sheet_xml(&rust_clean_path, clean_xml);
    let go_clean = go_clean_path.to_string_lossy().to_string();
    let rust_clean = rust_clean_path.to_string_lossy().to_string();

    for (label, go_bad, rust_bad) in [
        (
            "missing sheet",
            vec![
                "--json",
                "xlsx",
                "rows",
                "insert",
                &go_clean,
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
                &go_clean,
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
                &go_clean,
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
                &go_clean,
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
                &go_clean,
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
                &go_clean,
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
                &go_clean,
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
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, go_code, "{label} exit");
        assert_eq!(rust_stdout, go_stdout, "{label} stdout");
        assert_eq!(
            scrub_path(
                rust_stderr.unwrap_or_else(|| panic!("rust structure error stderr for {label}")),
                &rust_clean,
                "[IN]"
            ),
            scrub_path(
                go_stderr.unwrap_or_else(|| panic!("go structure error stderr for {label}")),
                &go_clean,
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
        let go_path = temp_dir.join(format!("go-{label}.xlsx").replace(' ', "-"));
        let rust_path = temp_dir.join(format!("rust-{label}.xlsx").replace(' ', "-"));
        write_simple_xlsx_with_sheet_xml(&go_path, sheet_xml);
        write_simple_xlsx_with_sheet_xml(&rust_path, sheet_xml);
        let go_file = go_path.to_string_lossy().to_string();
        let rust_file = rust_path.to_string_lossy().to_string();
        let go_bad = [
            "--json",
            "xlsx",
            family,
            action,
            &go_file,
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
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, go_code, "{label} exit");
        assert_eq!(rust_stdout, go_stdout, "{label} stdout");
        assert_eq!(
            scrub_path(
                rust_stderr.unwrap_or_else(|| panic!("rust structure guard stderr for {label}")),
                &rust_file,
                "[IN]"
            ),
            scrub_path(
                go_stderr.unwrap_or_else(|| panic!("go structure guard stderr for {label}")),
                &go_file,
                "[IN]"
            ),
            "{label} stderr"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_data_validations_list_show_match_go_oracle() {
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "data-validations",
        "list",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
    ]);

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-dv-list-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("dv.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &workbook,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
  <dataValidations count="2">
    <dataValidation type="list" sqref="A1:A3" allowBlank="1" showInputMessage="1" promptTitle="Pick" prompt="Choose a color"><formula1>"Red,Green"</formula1></dataValidation>
    <dataValidation type="whole" operator="between" sqref="$B$1:$B$2 C1" showErrorMessage="true" errorStyle="warning" errorTitle="Bad" error="1-10 only"><formula1>1</formula1><formula2>10</formula2></dataValidation>
  </dataValidations>
</worksheet>"#,
    );
    let workbook = workbook.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "data-validations",
        "list",
        &workbook,
        "--sheet",
        "Sheet1",
    ]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "data-validations",
        "show",
        &workbook,
        "--sheet",
        "1",
        "--range",
        "A1:A3",
    ]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "data-validations",
        "show",
        &workbook,
        "--sheet",
        "1",
        "--range",
        "$B$1:$B$2 C1",
    ]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "data-validations",
        "show",
        &workbook,
        "--sheet",
        "1",
        "--range",
        "Z9",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_data_validations_create_update_delete_saved_outputs_match_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-dv-mutate-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let go_create_out = temp_dir
        .join("go-create.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_create_out = temp_dir
        .join("rust-create.xlsx")
        .to_string_lossy()
        .to_string();
    let go_update_out = temp_dir
        .join("go-update.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_update_out = temp_dir
        .join("rust-update.xlsx")
        .to_string_lossy()
        .to_string();
    let go_delete_out = temp_dir
        .join("go-delete.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_delete_out = temp_dir
        .join("rust-delete.xlsx")
        .to_string_lossy()
        .to_string();

    let create_common = [
        "--json",
        "xlsx",
        "data-validations",
        "create",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "A1:A10",
        "--type",
        "list",
        "--list-values",
        "Red,Green,Blue",
        "--show-input-message",
        "--input-title",
        "Pick",
        "--input-message",
        "Choose a color",
        "--out",
    ];
    let mut go_args = create_common.to_vec();
    go_args.push(&go_create_out);
    let mut rust_args = create_common.to_vec();
    rust_args.push(&rust_create_out);
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "data validation create exit");
    assert_eq!(rust_stderr, go_stderr, "data validation create stderr");
    let rust_create = rust_stdout.expect("rust create stdout");
    assert_eq!(
        scrub_path(rust_create.clone(), &rust_create_out, "[CREATE_OUT]"),
        scrub_path(
            go_stdout.expect("go create stdout"),
            &go_create_out,
            "[CREATE_OUT]"
        ),
        "data validation create stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_create, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_create, "dataValidationsListCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_create, "dataValidationsShowCommand");

    let show_go = [
        "--json",
        "xlsx",
        "data-validations",
        "show",
        &go_create_out,
        "--sheet",
        "1",
        "--range",
        "A1:A10",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "data-validations",
        "show",
        &rust_create_out,
        "--sheet",
        "1",
        "--range",
        "A1:A10",
    ];
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "saved create show exit");
    assert_eq!(rust_stderr, go_stderr, "saved create show stderr");
    assert_eq!(
        rust_show.expect("rust saved create show"),
        go_show.expect("go saved create show"),
        "saved create show"
    );

    let update_common = [
        "--json",
        "xlsx",
        "data-validations",
        "update",
        "--sheet",
        "1",
        "--range",
        "A1:A10",
        "--list-values",
        "Red,Green,Blue,Amber",
        "--allow-blank",
        "--expect-type",
        "list",
        "--out",
    ];
    let mut go_args = vec![
        "--json",
        "xlsx",
        "data-validations",
        "update",
        &go_create_out,
    ];
    go_args.extend_from_slice(&update_common[4..]);
    go_args.push(&go_update_out);
    let mut rust_args = vec![
        "--json",
        "xlsx",
        "data-validations",
        "update",
        &rust_create_out,
    ];
    rust_args.extend_from_slice(&update_common[4..]);
    rust_args.push(&rust_update_out);
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "data validation update exit");
    assert_eq!(rust_stderr, go_stderr, "data validation update stderr");
    let rust_update = rust_stdout.expect("rust update stdout");
    assert_eq!(
        scrub_paths(
            rust_update.clone(),
            &[
                (&rust_create_out, "[CREATE_OUT]"),
                (&rust_update_out, "[UPDATE_OUT]")
            ]
        ),
        scrub_paths(
            go_stdout.expect("go update stdout"),
            &[
                (&go_create_out, "[CREATE_OUT]"),
                (&go_update_out, "[UPDATE_OUT]")
            ]
        ),
        "data validation update stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_update, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "dataValidationsListCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "dataValidationsShowCommand");

    let delete_common = [
        "--json",
        "xlsx",
        "data-validations",
        "delete",
        "--sheet",
        "1",
        "--range",
        "A1:A10",
        "--expect-type",
        "list",
        "--expect-formula1",
        "\"Red,Green,Blue,Amber\"",
        "--out",
    ];
    let mut go_args = vec![
        "--json",
        "xlsx",
        "data-validations",
        "delete",
        &go_update_out,
    ];
    go_args.extend_from_slice(&delete_common[4..]);
    go_args.push(&go_delete_out);
    let mut rust_args = vec![
        "--json",
        "xlsx",
        "data-validations",
        "delete",
        &rust_update_out,
    ];
    rust_args.extend_from_slice(&delete_common[4..]);
    rust_args.push(&rust_delete_out);
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "data validation delete exit");
    assert_eq!(rust_stderr, go_stderr, "data validation delete stderr");
    let rust_delete = rust_stdout.expect("rust delete stdout");
    assert_eq!(
        scrub_paths(
            rust_delete.clone(),
            &[
                (&rust_update_out, "[UPDATE_OUT]"),
                (&rust_delete_out, "[DELETE_OUT]")
            ]
        ),
        scrub_paths(
            go_stdout.expect("go delete stdout"),
            &[
                (&go_update_out, "[UPDATE_OUT]"),
                (&go_delete_out, "[DELETE_OUT]")
            ]
        ),
        "data validation delete stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete, "dataValidationsListCommand");

    let list_go = [
        "--json",
        "xlsx",
        "data-validations",
        "list",
        &go_delete_out,
        "--sheet",
        "1",
    ];
    let list_rust = [
        "--json",
        "xlsx",
        "data-validations",
        "list",
        &rust_delete_out,
        "--sheet",
        "1",
    ];
    let (go_code, go_list, go_stderr) = run_go_ooxml(&list_go);
    let (rust_code, rust_list, rust_stderr) = run_ooxml(&list_rust);
    assert_eq!(rust_code, go_code, "deleted list exit");
    assert_eq!(rust_stderr, go_stderr, "deleted list stderr");
    assert_eq!(
        scrub_path(
            rust_list.expect("rust deleted list"),
            &rust_delete_out,
            "[DELETE_OUT]"
        ),
        scrub_path(
            go_list.expect("go deleted list"),
            &go_delete_out,
            "[DELETE_OUT]"
        ),
        "deleted list"
    );

    for output in [&rust_create_out, &rust_update_out, &rust_delete_out] {
        let (code, _, stderr) = run_ooxml(&["--json", "--strict", "validate", output]);
        assert_eq!(code, 0, "strict validation for {output}");
        assert_eq!(stderr, None, "strict validation stderr for {output}");
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_data_validations_dry_run_and_errors_match_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-dv-dry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in_path).expect("copy go input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("copy rust input");
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();

    let before = read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml");
    let dry_go = [
        "--json",
        "xlsx",
        "data-validations",
        "create",
        &go_in,
        "--sheet",
        "1",
        "--range",
        "A1:A5 C1:C5",
        "--type",
        "whole",
        "--operator",
        "greaterThan",
        "--formula1",
        "0",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "data-validations",
        "create",
        &rust_in,
        "--sheet",
        "1",
        "--range",
        "A1:A5 C1:C5",
        "--type",
        "whole",
        "--operator",
        "greaterThan",
        "--formula1",
        "0",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "data validation dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "data validation dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust data validation dry-run"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(
            go_stdout.expect("go data validation dry-run"),
            &go_in,
            "[IN]"
        ),
        "data validation dry-run stdout"
    );
    assert_eq!(
        read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml"),
        before,
        "data validation dry-run should not mutate source workbook"
    );

    for (label, go_bad, rust_bad) in [
        (
            "missing list source",
            vec![
                "--json",
                "xlsx",
                "data-validations",
                "create",
                &go_in,
                "--sheet",
                "1",
                "--range",
                "A1:A10",
                "--type",
                "list",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "data-validations",
                "create",
                &rust_in,
                "--sheet",
                "1",
                "--range",
                "A1:A10",
                "--type",
                "list",
                "--dry-run",
            ],
        ),
        (
            "invalid operator",
            vec![
                "--json",
                "xlsx",
                "data-validations",
                "create",
                &go_in,
                "--sheet",
                "1",
                "--range",
                "A1:A10",
                "--type",
                "list",
                "--list-values",
                "a,b",
                "--operator",
                "between",
                "--dry-run",
            ],
            vec![
                "--json",
                "xlsx",
                "data-validations",
                "create",
                &rust_in,
                "--sheet",
                "1",
                "--range",
                "A1:A10",
                "--type",
                "list",
                "--list-values",
                "a,b",
                "--operator",
                "between",
                "--dry-run",
            ],
        ),
        (
            "invalid range",
            vec![
                "--json",
                "xlsx",
                "data-validations",
                "show",
                &go_in,
                "--sheet",
                "1",
                "--range",
                "A1:",
            ],
            vec![
                "--json",
                "xlsx",
                "data-validations",
                "show",
                &rust_in,
                "--sheet",
                "1",
                "--range",
                "A1:",
            ],
        ),
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, go_code, "{label} exit");
        assert_eq!(rust_stdout, go_stdout, "{label} stdout");
        assert_eq!(
            scrub_path(
                rust_stderr.expect("rust data validation bad stderr"),
                &rust_in,
                "[IN]"
            ),
            scrub_path(
                go_stderr.expect("go data validation bad stderr"),
                &go_in,
                "[IN]"
            ),
            "{label} stderr"
        );
    }

    let go_created = temp_dir
        .join("go-created.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_created = temp_dir
        .join("rust-created.xlsx")
        .to_string_lossy()
        .to_string();
    for (input, output, runner) in [
        (
            &go_in,
            &go_created,
            run_go_ooxml as fn(&[&str]) -> (i32, Option<Value>, Option<Value>),
        ),
        (
            &rust_in,
            &rust_created,
            run_ooxml as fn(&[&str]) -> (i32, Option<Value>, Option<Value>),
        ),
    ] {
        let args = [
            "--json",
            "xlsx",
            "data-validations",
            "create",
            input,
            "--sheet",
            "1",
            "--range",
            "A1:A10",
            "--type",
            "list",
            "--list-values",
            "a,b",
            "--out",
            output,
        ];
        let (code, _, stderr) = runner(&args);
        assert_eq!(code, 0, "setup create {input}");
        assert_eq!(stderr, None, "setup create stderr {input}");
    }
    let guard_go = [
        "--json",
        "xlsx",
        "data-validations",
        "update",
        &go_created,
        "--sheet",
        "1",
        "--range",
        "A1:A10",
        "--list-values",
        "x,y",
        "--expect-type",
        "whole",
        "--dry-run",
    ];
    let guard_rust = [
        "--json",
        "xlsx",
        "data-validations",
        "update",
        &rust_created,
        "--sheet",
        "1",
        "--range",
        "A1:A10",
        "--list-values",
        "x,y",
        "--expect-type",
        "whole",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&guard_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&guard_rust);
    assert_eq!(rust_code, go_code, "guard mismatch exit");
    assert_eq!(rust_stdout, go_stdout, "guard mismatch stdout");
    assert_eq!(
        scrub_path(
            rust_stderr.expect("rust guard mismatch stderr"),
            &rust_created,
            "[CREATED]"
        ),
        scrub_path(
            go_stderr.expect("go guard mismatch stderr"),
            &go_created,
            "[CREATED]"
        ),
        "guard mismatch stderr"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_dimension_setters_match_go_oracle_saved_readback_dry_run_and_errors() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-dim-set-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let go_cols_in_path = temp_dir.join("go-cols-in.xlsx");
    let rust_cols_in_path = temp_dir.join("rust-cols-in.xlsx");
    let go_cols_out_path = temp_dir.join("go-cols-out.xlsx");
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
    write_simple_xlsx_with_sheet_xml(&go_cols_in_path, cols_xml);
    write_simple_xlsx_with_sheet_xml(&rust_cols_in_path, cols_xml);
    let go_cols_in = go_cols_in_path.to_string_lossy().to_string();
    let rust_cols_in = rust_cols_in_path.to_string_lossy().to_string();
    let go_cols_out = go_cols_out_path.to_string_lossy().to_string();
    let rust_cols_out = rust_cols_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "xlsx",
        "colwidths",
        "set",
        &go_cols_in,
        "--sheet",
        "Sheet1",
        "--range",
        "C:E",
        "--width",
        "12.5",
        "--expect-width",
        "30",
        "--out",
        &go_cols_out,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "colwidths set exit");
    assert_eq!(rust_stderr, go_stderr, "colwidths set stderr");
    let rust_result = rust_stdout.expect("rust colwidths set stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_cols_in, "[IN]"), (&rust_cols_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go colwidths set stdout"),
            &[(&go_cols_in, "[IN]"), (&go_cols_out, "[OUT]")]
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
        &go_cols_out,
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
    let (go_code, go_show, go_stderr) = run_go_ooxml(&col_show_go);
    let (rust_code, rust_show, rust_stderr) = run_go_ooxml(&col_show_rust);
    assert_eq!(rust_code, go_code, "colwidths saved readback exit");
    assert_eq!(rust_stderr, go_stderr, "colwidths saved readback stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust colwidths saved readback"),
            &rust_cols_out,
            "[OUT]"
        ),
        scrub_path(
            go_show.expect("go colwidths saved readback"),
            &go_cols_out,
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
        &go_cols_in,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "colwidths dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "colwidths dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust colwidths dry-run stdout"),
            &rust_cols_in,
            "[IN]"
        ),
        scrub_path(
            go_stdout.expect("go colwidths dry-run stdout"),
            &go_cols_in,
            "[IN]"
        ),
        "colwidths dry-run stdout"
    );
    assert_eq!(
        read_zip_string(&rust_cols_in_path, "xl/worksheets/sheet1.xml"),
        before_cols,
        "colwidths dry-run should not mutate source workbook"
    );

    for (label, go_bad, rust_bad) in [
        (
            "missing width",
            vec![
                "--json",
                "xlsx",
                "colwidths",
                "set",
                &go_cols_in,
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
                &go_cols_in,
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
                &go_cols_in,
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
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, go_code, "colwidths {label} exit");
        assert_eq!(rust_stdout, go_stdout, "colwidths {label} stdout");
        assert_eq!(
            scrub_path(
                rust_stderr.expect("rust colwidths bad stderr"),
                &rust_cols_in,
                "[IN]"
            ),
            scrub_path(
                go_stderr.expect("go colwidths bad stderr"),
                &go_cols_in,
                "[IN]"
            ),
            "colwidths {label} stderr"
        );
    }

    let go_rows_in_path = temp_dir.join("go-rows-in.xlsx");
    let rust_rows_in_path = temp_dir.join("rust-rows-in.xlsx");
    let go_rows_out_path = temp_dir.join("go-rows-out.xlsx");
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
    write_simple_xlsx_with_sheet_xml(&go_rows_in_path, rows_xml);
    write_simple_xlsx_with_sheet_xml(&rust_rows_in_path, rows_xml);
    let go_rows_in = go_rows_in_path.to_string_lossy().to_string();
    let rust_rows_in = rust_rows_in_path.to_string_lossy().to_string();
    let go_rows_out = go_rows_out_path.to_string_lossy().to_string();
    let rust_rows_out = rust_rows_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "xlsx",
        "rowheights",
        "set",
        &go_rows_in,
        "--sheet",
        "Sheet1",
        "--range",
        "2:4",
        "--height",
        "24.5",
        "--expect-height",
        "17",
        "--out",
        &go_rows_out,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "rowheights set exit");
    assert_eq!(rust_stderr, go_stderr, "rowheights set stderr");
    let rust_result = rust_stdout.expect("rust rowheights set stdout");
    assert_eq!(
        scrub_paths(
            rust_result.clone(),
            &[(&rust_rows_in, "[IN]"), (&rust_rows_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go rowheights set stdout"),
            &[(&go_rows_in, "[IN]"), (&go_rows_out, "[OUT]")]
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
        &go_rows_out,
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
    let (go_code, go_show, go_stderr) = run_go_ooxml(&row_show_go);
    let (rust_code, rust_show, rust_stderr) = run_go_ooxml(&row_show_rust);
    assert_eq!(rust_code, go_code, "rowheights saved readback exit");
    assert_eq!(rust_stderr, go_stderr, "rowheights saved readback stderr");
    assert_eq!(
        scrub_path(
            rust_show.expect("rust rowheights saved readback"),
            &rust_rows_out,
            "[OUT]"
        ),
        scrub_path(
            go_show.expect("go rowheights saved readback"),
            &go_rows_out,
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
        &go_rows_in,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "rowheights dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "rowheights dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust rowheights dry-run stdout"),
            &rust_rows_in,
            "[IN]"
        ),
        scrub_path(
            go_stdout.expect("go rowheights dry-run stdout"),
            &go_rows_in,
            "[IN]"
        ),
        "rowheights dry-run stdout"
    );
    assert_eq!(
        read_zip_string(&rust_rows_in_path, "xl/worksheets/sheet1.xml"),
        before_rows,
        "rowheights dry-run should not mutate source workbook"
    );

    for (label, go_bad, rust_bad) in [
        (
            "missing height",
            vec![
                "--json",
                "xlsx",
                "rowheights",
                "set",
                &go_rows_in,
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
                &go_rows_in,
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
                &go_rows_in,
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
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, go_code, "rowheights {label} exit");
        assert_eq!(rust_stdout, go_stdout, "rowheights {label} stdout");
        assert_eq!(
            scrub_path(
                rust_stderr.expect("rust rowheights bad stderr"),
                &rust_rows_in,
                "[IN]"
            ),
            scrub_path(
                go_stderr.expect("go rowheights bad stderr"),
                &go_rows_in,
                "[IN]"
            ),
            "rowheights {label} stderr"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

include!("xlsx/filters_sorts.rs");

#[test]
fn xlsx_comments_add_update_remove_matches_go_oracle_and_saved_output() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-comments-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_added_path = temp_dir.join("go-added.xlsx");
    let rust_added_path = temp_dir.join("rust-added.xlsx");
    let go_updated_path = temp_dir.join("go-updated.xlsx");
    let rust_updated_path = temp_dir.join("rust-updated.xlsx");
    let go_removed_path = temp_dir.join("go-removed.xlsx");
    let rust_removed_path = temp_dir.join("rust-removed.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in_path).expect("go input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_added = go_added_path.to_string_lossy().to_string();
    let rust_added = rust_added_path.to_string_lossy().to_string();
    let go_updated = go_updated_path.to_string_lossy().to_string();
    let rust_updated = rust_updated_path.to_string_lossy().to_string();
    let go_removed = go_removed_path.to_string_lossy().to_string();
    let rust_removed = rust_removed_path.to_string_lossy().to_string();

    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "comments",
        "list",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
    ]);

    let add_go = [
        "--json", "xlsx", "comments", "add", &go_in, "--sheet", "Sheet1", "--cell", "C3",
        "--author", "Ann", "--text", "before", "--out", &go_added,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&add_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&add_rust);
    assert_eq!(rust_code, go_code, "comments add exit");
    assert_eq!(rust_stderr, go_stderr, "comments add stderr");
    let rust_add = rust_stdout.expect("rust comments add stdout");
    assert_eq!(
        scrub_paths(
            rust_add.clone(),
            &[(&rust_in, "[IN]"), (&rust_added, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go comments add stdout"),
            &[(&go_in, "[IN]"), (&go_added, "[OUT]")]
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
        "--json", "xlsx", "comments", "list", &go_added, "--sheet", "Sheet1",
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
    let (go_code, go_list, go_stderr) = run_go_ooxml(&list_go);
    let (rust_code, rust_list, rust_stderr) = run_ooxml(&list_rust);
    assert_eq!(rust_code, go_code, "saved comments list exit");
    assert_eq!(rust_stderr, go_stderr, "saved comments list stderr");
    assert_eq!(
        scrub_path(
            rust_list.expect("rust saved comments list"),
            &rust_added,
            "[OUT]"
        ),
        scrub_path(go_list.expect("go saved comments list"), &go_added, "[OUT]"),
        "saved comments list"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &go_in,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "comments add dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "comments add dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust comments add dry-run"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(go_stdout.expect("go comments add dry-run"), &go_in, "[IN]"),
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
        &go_added,
        "--handle",
        "H:xlsx/ws:1/comment:a:C3",
        "--author",
        "Ben",
        "--text",
        "after",
        "--expect-hash",
        expect_hash,
        "--out",
        &go_updated,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&update_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&update_rust);
    assert_eq!(rust_code, go_code, "comments update exit");
    assert_eq!(rust_stderr, go_stderr, "comments update stderr");
    let rust_update = rust_stdout.expect("rust comments update stdout");
    assert_eq!(
        scrub_paths(
            rust_update.clone(),
            &[(&rust_added, "[IN]"), (&rust_updated, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go comments update stdout"),
            &[(&go_added, "[IN]"), (&go_updated, "[OUT]")]
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
        &go_added,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&stale_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&stale_rust);
    assert_eq!(rust_code, go_code, "comments update stale-hash exit");
    assert_eq!(rust_stdout, go_stdout, "comments update stale-hash stdout");
    assert_eq!(rust_stderr, go_stderr, "comments update stale-hash stderr");

    let duplicate_go = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &go_added,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&duplicate_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&duplicate_rust);
    assert_eq!(rust_code, go_code, "comments duplicate add exit");
    assert_eq!(rust_stdout, go_stdout, "comments duplicate add stdout");
    assert_eq!(rust_stderr, go_stderr, "comments duplicate add stderr");

    let text_file_path = temp_dir.join("comment.txt");
    fs::write(&text_file_path, "from file").expect("comment text file");
    let text_file = text_file_path.to_string_lossy().to_string();
    let text_conflict_go = [
        "--json",
        "xlsx",
        "comments",
        "add",
        &go_in,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&text_conflict_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&text_conflict_rust);
    assert_eq!(rust_code, go_code, "comments text conflict exit");
    assert_eq!(rust_stdout, go_stdout, "comments text conflict stdout");
    assert_eq!(rust_stderr, go_stderr, "comments text conflict stderr");

    let missing_go = [
        "--json",
        "xlsx",
        "comments",
        "list",
        &go_added,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&missing_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_rust);
    assert_eq!(rust_code, go_code, "comments missing list exit");
    assert_eq!(rust_stdout, go_stdout, "comments missing list stdout");
    assert_eq!(rust_stderr, go_stderr, "comments missing list stderr");

    let updated_hash = rust_update["contentHash"].as_str().expect("updated hash");
    let remove_go = [
        "--json",
        "xlsx",
        "comments",
        "remove",
        &go_updated,
        "--sheet",
        "Sheet1",
        "--comment-id",
        "0",
        "--expect-hash",
        updated_hash,
        "--out",
        &go_removed,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&remove_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&remove_rust);
    assert_eq!(rust_code, go_code, "comments remove exit");
    assert_eq!(rust_stderr, go_stderr, "comments remove stderr");
    let rust_remove = rust_stdout.expect("rust comments remove stdout");
    assert_eq!(
        scrub_paths(
            rust_remove.clone(),
            &[(&rust_updated, "[IN]"), (&rust_removed, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go comments remove stdout"),
            &[(&go_updated, "[IN]"), (&go_removed, "[OUT]")]
        ),
        "comments remove stdout"
    );
    assert_eq!(rust_remove["previousAuthor"], "Ben");
    assert_eq!(rust_remove["previousText"], "after");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_remove, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_remove, "listCommand");

    let (go_code, go_list, go_stderr) =
        run_go_ooxml(&["--json", "xlsx", "comments", "list", &go_removed]);
    let (rust_code, rust_list, rust_stderr) =
        run_ooxml(&["--json", "xlsx", "comments", "list", &rust_removed]);
    assert_eq!(rust_code, go_code, "removed comments list exit");
    assert_eq!(rust_stderr, go_stderr, "removed comments list stderr");
    assert_eq!(
        scrub_path(
            rust_list.expect("rust removed comments list"),
            &rust_removed,
            "[OUT]"
        ),
        scrub_path(
            go_list.expect("go removed comments list"),
            &go_removed,
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

fn assert_go_rust_match_scrubbed(
    label: &str,
    go_args: &[&str],
    rust_args: &[&str],
    replacements: &[(&str, &str)],
) -> Option<Value> {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, go_code, "{label} exit");
    assert_eq!(
        rust_stderr
            .clone()
            .map(|value| scrub_paths(value, replacements)),
        go_stderr.map(|value| scrub_paths(value, replacements)),
        "{label} stderr"
    );
    assert_eq!(
        rust_stdout
            .clone()
            .map(|value| scrub_paths(value, replacements)),
        go_stdout.map(|value| scrub_paths(value, replacements)),
        "{label} stdout"
    );
    rust_stdout
}

include!("xlsx/pivots.rs");

include!("xlsx/hyperlinks.rs");

include!("xlsx/workbook_metadata.rs");

#[test]
fn xlsx_sheets_show_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "xlsx",
            "sheets",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "xlsx",
            "sheets",
            "show",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
            "--sheet",
            "Types",
        ],
        vec![
            "--json",
            "xlsx",
            "sheets",
            "show",
            "testdata/xlsx/used-range/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn xlsx_sheets_add_matches_go_oracle_shape_and_saved_output() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-sheets-add-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-add.xlsx");
    let rust_out_path = temp_dir.join("rust-add.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &go_in_path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>"#,
    );
    fs::copy(&go_in_path, &rust_in_path).expect("copy rust add input");
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json", "xlsx", "sheets", "add", &go_in, "--name", "Added", "--out", &go_out,
    ];
    let rust_args = [
        "--json", "xlsx", "sheets", "add", &rust_in, "--name", "Added", "--out", &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "sheets add exit");
    assert_eq!(rust_stderr, go_stderr, "sheets add stderr");
    let rust_result = rust_stdout.expect("rust sheets add stdout");
    assert_eq!(
        normalize_xlsx_dynamic_sheet_id(
            scrub_paths(
                rust_result.clone(),
                &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
            ),
            "Added",
        ),
        normalize_xlsx_dynamic_sheet_id(
            scrub_paths(
                go_stdout.expect("go sheets add stdout"),
                &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
            ),
            "Added",
        ),
        "sheets add stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_result, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_result, "sheetsListCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_result, "sheetShowCommand");
    assert!(zip_entry_exists(&rust_out_path, "xl/worksheets/sheet2.xml"));
    assert!(
        read_zip_string(&rust_out_path, "[Content_Types].xml")
            .contains(r#"PartName="/xl/worksheets/sheet2.xml""#)
    );
    assert!(
        read_zip_string(&rust_out_path, "xl/_rels/workbook.xml.rels")
            .contains(r#"Target="worksheets/sheet2.xml""#)
    );

    let before_workbook = read_zip_string(&rust_in_path, "xl/workbook.xml");
    let dry_go = [
        "--json",
        "xlsx",
        "sheets",
        "add",
        &go_in,
        "--name",
        "Dry",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "sheets",
        "add",
        &rust_in,
        "--name",
        "Dry",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "sheets add dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "sheets add dry-run stderr");
    assert_eq!(
        normalize_xlsx_dynamic_sheet_id(
            scrub_path(
                rust_stdout.expect("rust sheets add dry-run stdout"),
                &rust_in,
                "[IN]",
            ),
            "Dry",
        ),
        normalize_xlsx_dynamic_sheet_id(
            scrub_path(
                go_stdout.expect("go sheets add dry-run stdout"),
                &go_in,
                "[IN]"
            ),
            "Dry",
        ),
        "sheets add dry-run stdout"
    );
    assert_eq!(
        read_zip_string(&rust_in_path, "xl/workbook.xml"),
        before_workbook,
        "sheets add dry-run should not mutate source workbook"
    );

    for (label, extra) in [
        ("duplicate name", vec!["--name", "Sheet1", "--dry-run"]),
        ("invalid name", vec!["--name", "Bad/Name", "--dry-run"]),
    ] {
        let mut go_bad = vec!["--json", "xlsx", "sheets", "add", &go_in];
        go_bad.extend(extra.iter().copied());
        let mut rust_bad = vec!["--json", "xlsx", "sheets", "add", &rust_in];
        rust_bad.extend(extra.iter().copied());
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, go_code, "sheets add {label} exit");
        assert_eq!(rust_stdout, go_stdout, "sheets add {label} stdout");
        assert_eq!(rust_stderr, go_stderr, "sheets add {label} stderr");
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_sheets_rename_move_delete_match_go_oracle_and_saved_outputs() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-sheets-life-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    write_sheet_lifecycle_xlsx(&go_in_path);
    fs::copy(&go_in_path, &rust_in_path).expect("copy rust lifecycle input");
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();

    let go_rename_path = temp_dir.join("go-rename.xlsx");
    let rust_rename_path = temp_dir.join("rust-rename.xlsx");
    let go_rename = go_rename_path.to_string_lossy().to_string();
    let rust_rename = rust_rename_path.to_string_lossy().to_string();
    let go_args = [
        "--json", "xlsx", "sheets", "rename", &go_in, "--sheet", "Data", "--name", "Facts",
        "--out", &go_rename,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "sheets",
        "rename",
        &rust_in,
        "--sheet",
        "Data",
        "--name",
        "Facts",
        "--out",
        &rust_rename,
    ];
    let rust_rename_result = assert_xlsx_sheet_mutation_matches_go(
        "sheets rename",
        &go_args,
        &rust_args,
        &[(&go_in, "[IN]"), (&go_rename, "[OUT]")],
        &[(&rust_in, "[IN]"), (&rust_rename, "[OUT]")],
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_rename_result, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_rename_result, "sheetsListCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_rename_result, "sheetShowCommand");
    let renamed_workbook = read_zip_string(&rust_rename_path, "xl/workbook.xml");
    assert!(renamed_workbook.contains(r#"name="Facts""#));
    assert!(!renamed_workbook.contains(r#"name="Data""#));

    let dry_go = [
        "--json",
        "xlsx",
        "sheets",
        "rename",
        &go_in,
        "--sheet",
        "Data",
        "--name",
        "DryFacts",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "sheets",
        "rename",
        &rust_in,
        "--sheet",
        "Data",
        "--name",
        "DryFacts",
        "--dry-run",
    ];
    assert_xlsx_sheet_mutation_matches_go(
        "sheets rename dry-run",
        &dry_go,
        &dry_rust,
        &[(&go_in, "[IN]")],
        &[(&rust_in, "[IN]")],
    );
    assert!(
        read_zip_string(&rust_in_path, "xl/workbook.xml").contains(r#"name="Data""#),
        "rename dry-run changed source workbook"
    );

    let go_move_path = temp_dir.join("go-move.xlsx");
    let rust_move_path = temp_dir.join("rust-move.xlsx");
    let go_move = go_move_path.to_string_lossy().to_string();
    let rust_move = rust_move_path.to_string_lossy().to_string();
    let go_args = [
        "--json", "xlsx", "sheets", "move", &go_rename, "--sheet", "Facts", "--before", "Summary",
        "--out", &go_move,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "sheets",
        "move",
        &rust_rename,
        "--sheet",
        "Facts",
        "--before",
        "Summary",
        "--out",
        &rust_move,
    ];
    let rust_move_result = assert_xlsx_sheet_mutation_matches_go(
        "sheets move",
        &go_args,
        &rust_args,
        &[(&go_rename, "[IN]"), (&go_move, "[OUT]")],
        &[(&rust_rename, "[IN]"), (&rust_move, "[OUT]")],
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_move_result, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_move_result, "sheetsListCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_move_result, "sheetShowCommand");
    let moved_workbook = read_zip_string(&rust_move_path, "xl/workbook.xml");
    let facts_pos = moved_workbook
        .find(r#"name="Facts""#)
        .expect("Facts sheet after move");
    let summary_pos = moved_workbook
        .find(r#"name="Summary""#)
        .expect("Summary sheet after move");
    assert!(
        facts_pos < summary_pos,
        "Facts should move before Summary:\n{moved_workbook}"
    );
    assert!(moved_workbook.contains(r#"firstSheet="1""#));

    let bad_move_go = [
        "--json",
        "xlsx",
        "sheets",
        "move",
        &go_move,
        "--sheet",
        "Facts",
        "--to",
        "1",
        "--before",
        "Tail",
        "--dry-run",
    ];
    let bad_move_rust = [
        "--json",
        "xlsx",
        "sheets",
        "move",
        &rust_move,
        "--sheet",
        "Facts",
        "--to",
        "1",
        "--before",
        "Tail",
        "--dry-run",
    ];
    assert_xlsx_sheet_error_matches_go("sheets move target guard", &bad_move_go, &bad_move_rust);

    let go_delete_path = temp_dir.join("go-delete.xlsx");
    let rust_delete_path = temp_dir.join("rust-delete.xlsx");
    let go_delete = go_delete_path.to_string_lossy().to_string();
    let rust_delete = rust_delete_path.to_string_lossy().to_string();
    let go_args = [
        "--json", "xlsx", "sheets", "delete", &go_move, "--sheet", "Summary", "--out", &go_delete,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "sheets",
        "delete",
        &rust_move,
        "--sheet",
        "Summary",
        "--out",
        &rust_delete,
    ];
    let rust_delete_result = assert_xlsx_sheet_mutation_matches_go(
        "sheets delete",
        &go_args,
        &rust_args,
        &[(&go_move, "[IN]"), (&go_delete, "[OUT]")],
        &[(&rust_move, "[IN]"), (&rust_delete, "[OUT]")],
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete_result, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete_result, "sheetsListCommand");
    assert!(!zip_entry_exists(
        &rust_delete_path,
        "xl/worksheets/sheet1.xml"
    ));
    assert!(!read_zip_string(&rust_delete_path, "xl/_rels/workbook.xml.rels").contains("rId1"));
    assert!(
        !read_zip_string(&rust_delete_path, "[Content_Types].xml")
            .contains("/xl/worksheets/sheet1.xml")
    );

    let last_go = [
        "--json",
        "xlsx",
        "sheets",
        "delete",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "Sheet1",
        "--dry-run",
    ];
    assert_xlsx_sheet_error_matches_go("sheets delete last sheet", &last_go, &last_go);

    let _ = fs::remove_dir_all(&temp_dir);
}

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

fn assert_xlsx_sheet_mutation_matches_go(
    label: &str,
    go_args: &[&str],
    rust_args: &[&str],
    go_paths: &[(&str, &str)],
    rust_paths: &[(&str, &str)],
) -> Value {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, go_code, "{label} exit");
    assert_eq!(rust_stderr, go_stderr, "{label} stderr");
    let rust_value = rust_stdout.expect("rust sheet mutation stdout");
    assert_eq!(
        scrub_paths(rust_value.clone(), rust_paths),
        scrub_paths(go_stdout.expect("go sheet mutation stdout"), go_paths),
        "{label} stdout"
    );
    rust_value
}

fn assert_xlsx_sheet_error_matches_go(label: &str, go_args: &[&str], rust_args: &[&str]) {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, go_code, "{label} exit");
    assert_eq!(rust_stdout, go_stdout, "{label} stdout");
    assert_eq!(rust_stderr, go_stderr, "{label} stderr");
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
