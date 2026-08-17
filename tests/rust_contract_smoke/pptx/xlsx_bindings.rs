#[test]
fn pptx_xlsx_bindings_apply_saved_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-xlsx-bindings-apply-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&temp_dir).expect("xlsx bindings apply temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let workbook = temp_dir.join("bindings.xlsx");
    write_xlsx_bindings_workbook(&workbook, "subtitle");
    let workbook_str = workbook.to_string_lossy().to_string();
    let baseline_out = temp_dir.join("baseline-bindings.pptx");
    let rust_out = temp_dir.join("rust-bindings.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline bindings output");
    let rust_out_str = rust_out.to_str().expect("rust bindings output");

    let baseline_args = [
        "--json",
        "pptx",
        "xlsx-bindings",
        "apply",
        fixture,
        "--workbook",
        &workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:P3",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "xlsx-bindings",
        "apply",
        fixture,
        "--workbook",
        &workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:P3",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "xlsx-bindings apply saved exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "xlsx-bindings apply saved stderr"
    );
    let baseline_json = baseline_stdout.expect("baseline xlsx-bindings apply saved");
    let rust_json = rust_stdout.expect("rust xlsx-bindings apply saved");
    assert_eq!(
        scrub_paths(rust_json.clone(), &[(rust_out_str, "[OUT]")]),
        scrub_paths(baseline_json, &[(baseline_out_str, "[OUT]")]),
        "xlsx-bindings apply saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline xlsx-bindings output missing"
    );
    assert!(rust_out.exists(), "Rust xlsx-bindings output missing");
    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_out_str]);
    assert_eq!(validate_code, 0, "bindings output strict validate exit");
    assert_eq!(
        validate_stderr, None,
        "bindings output strict validate stderr"
    );
    assert_eq!(
        validate_stdout.expect("bindings strict validate")["valid"],
        Value::Bool(true)
    );

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) =
        run_ooxml_baseline(&["--json", "pptx", "extract", "text", baseline_out_str]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_read_code, baseline_read_code, "bindings readback exit");
    assert_eq!(
        rust_read_stderr, baseline_read_stderr,
        "bindings readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust bindings readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_read_stdout.expect("baseline bindings readback"),
            baseline_out_str,
            "[OUT]"
        ),
        "bindings readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "xlsx-bindings",
        "apply",
        fixture,
        "--workbook",
        &workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:P3",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "xlsx-bindings apply dry-run exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "xlsx-bindings apply dry-run stderr"
    );
    assert_eq!(
        rust_stdout.expect("rust xlsx-bindings dry-run"),
        baseline_stdout.expect("baseline xlsx-bindings dry-run"),
        "xlsx-bindings apply dry-run stdout"
    );

    assert_baseline_rust_json_match(
        &[
            "--json",
            "pptx",
            "xlsx-bindings",
            "apply",
            fixture,
            "--workbook",
            &workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:P3",
            "--dry-run",
            "--out",
            rust_out_str,
        ],
        "xlsx-bindings apply dry-run out error",
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

fn write_xlsx_bindings_workbook(path: &Path, second_target: &str) {
    let header = [
        ("A", "id"),
        ("B", "op"),
        ("C", "slide"),
        ("D", "target"),
        ("E", "sourceSheet"),
        ("F", "sourceRange"),
        ("G", "mode"),
        ("H", "rowSep"),
        ("I", "colSep"),
        ("J", "formulaMode"),
        ("K", "x"),
        ("L", "y"),
        ("M", "cx"),
        ("N", "cy"),
        ("O", "name"),
        ("P", "header"),
    ];
    let row1 = header
        .iter()
        .map(|(col, value)| inline_str_cell(&format!("{col}1"), value))
        .collect::<String>();
    let row2 = [
        ("A2", "title"),
        ("B2", "replace-text"),
        ("C2", "1"),
        ("D2", "title"),
        ("E2", "Sheet1"),
        ("F2", "AA1"),
        ("G2", "preserve-format"),
        ("H2", "\\n"),
        ("I2", " | "),
        ("J2", "value"),
    ]
    .iter()
    .map(|(cell, value)| inline_str_cell(cell, value))
    .collect::<String>();
    let row3 = [
        ("A3", "move"),
        ("B3", "set-bounds"),
        ("C3", "1"),
        ("D3", second_target),
        ("K3", "100"),
        ("L3", "200"),
        ("M3", "3000000"),
        ("N3", "1000000"),
    ]
    .iter()
    .map(|(cell, value)| inline_str_cell(cell, value))
    .collect::<String>();
    let sheet_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:AA3"/>
  <sheetData>
    <row r="1">{row1}<c r="AA1" t="inlineStr"><is><t>Bound Title</t></is></c></row>
    <row r="2">{row2}</row>
    <row r="3">{row3}</row>
  </sheetData>
</worksheet>"#
    );
    write_simple_xlsx_with_sheet_xml(path, &sheet_xml);
}

fn inline_str_cell(cell: &str, value: &str) -> String {
    format!(
        r#"<c r="{cell}" t="inlineStr"><is><t>{}</t></is></c>"#,
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    )
}

