// XLSX command-family parity tests live in a child module to keep this integration
// test crate navigable while preserving the shared oracle/fixture helpers above.
use super::*;

#[test]
fn xlsx_ranges_export_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "xlsx",
            "ranges",
            "export",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "1",
            "--range",
            "A1:B2",
        ],
        vec![
            "--json",
            "xlsx",
            "ranges",
            "export",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "1",
            "--range",
            "A1:B2",
            "--include-types",
        ],
        vec![
            "--json",
            "xlsx",
            "ranges",
            "export",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
            "--sheet",
            "Types",
            "--range",
            "A1:H2",
            "--include-types",
            "--include-formulas",
            "--include-formats",
        ],
        vec![
            "--json",
            "xlsx",
            "ranges",
            "export",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "1",
            "--range",
            "A1:B2",
            "--max-cells",
            "1",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

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

#[test]
fn xlsx_charts_list_and_show_match_go_oracle() {
    let workbook = "testdata/xlsx/chart-workbook/workbook.xlsx";
    assert_go_rust_match(&["--json", "xlsx", "charts", "list", workbook]);
    assert_go_rust_match(&[
        "--json", "xlsx", "charts", "list", workbook, "--sheet", "Data",
    ]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "charts",
        "show",
        workbook,
        "--chart",
        "Revenue Chart 1",
    ]);
    assert_go_rust_match(&[
        "--json", "xlsx", "charts", "show", workbook, "--sheet", "Data", "--chart", "chart:1",
    ]);

    for selector in [
        "chart:1",
        "#1",
        "chart:Revenue Chart 1",
        "name:Revenue Chart 1",
        "~Revenue Chart 1",
        "part:/xl/charts/chart1.xml",
        "rid:rIdChart1",
        "drawingRid:rIdDrawing1",
    ] {
        assert_go_rust_match(&[
            "--json", "xlsx", "charts", "show", workbook, "--chart", selector,
        ]);
    }
}

#[test]
fn xlsx_charts_empty_and_errors_match_go_oracle() {
    let chart_workbook = "testdata/xlsx/chart-workbook/workbook.xlsx";
    let no_chart_workbook = "testdata/xlsx/minimal-workbook/workbook.xlsx";
    assert_go_rust_match(&["--json", "xlsx", "charts", "list", no_chart_workbook]);
    assert_go_rust_match(&["--json", "xlsx", "charts", "show", no_chart_workbook]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "charts",
        "show",
        chart_workbook,
        "--chart",
        "Missing",
    ]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "charts",
        "convert-type",
        chart_workbook,
        "--sheet",
        "Data",
        "--chart",
        "chart:1",
        "--to",
        "doughnut",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "charts",
        "copy-style",
        chart_workbook,
        "--sheet",
        "Data",
        "--chart",
        "chart:1",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "charts",
        "set-axis",
        chart_workbook,
        "--sheet",
        "Data",
        "--chart",
        "chart:1",
        "--axis",
        "value",
        "--dry-run",
    ]);
}

#[test]
fn xlsx_charts_style_mutations_match_go_oracle() {
    let workbook = "testdata/xlsx/chart-workbook/workbook.xlsx";
    for args in [
        vec![
            "--json",
            "xlsx",
            "charts",
            "set-title",
            workbook,
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
            "--title",
            "Styled Revenue",
            "--font-family",
            "Aptos",
            "--font-size",
            "14",
            "--font-bold=true",
            "--font-color",
            "2255AA",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "charts",
            "set-legend",
            workbook,
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
            "--position",
            "bottom",
            "--overlay=false",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "charts",
            "set-chart-area-fill",
            workbook,
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
            "--fill-color",
            "FFEEDD",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "charts",
            "set-plot-area-fill",
            workbook,
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
            "--fill-color",
            "CCEEFF",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "charts",
            "set-series-style",
            workbook,
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
            "--series",
            "1",
            "--fill-color",
            "FF8800",
            "--line-color",
            "114477",
            "--line-width-pt",
            "2",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "charts",
            "set-series-style",
            workbook,
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
            "--series",
            "1",
            "--marker-symbol",
            "circle",
            "--dry-run",
        ],
    ] {
        assert_go_rust_match(&args);
    }
}

#[test]
fn xlsx_charts_remaining_mutations_match_go_oracle() {
    let workbook = "testdata/xlsx/chart-workbook/workbook.xlsx";
    for args in [
        vec![
            "--json",
            "xlsx",
            "charts",
            "convert-type",
            workbook,
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
            "--to",
            "line",
            "--expect-type",
            "column",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "charts",
            "copy-style",
            workbook,
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
            "--from",
            workbook,
            "--from-chart",
            "chart:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "charts",
            "set-axis",
            workbook,
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
            "--axis",
            "value",
            "--title",
            "Sales Axis",
            "--min",
            "0",
            "--max",
            "100",
            "--major-unit",
            "25",
            "--number-format",
            "#,##0",
            "--major-gridlines=true",
            "--minor-gridlines=true",
            "--tick-label-font-size",
            "9",
            "--tick-label-font-color",
            "333333",
            "--tick-label-font-bold=true",
            "--title-font-color",
            "2255AA",
            "--title-font-size",
            "12",
            "--dry-run",
        ],
    ] {
        assert_go_rust_match(&args);
    }
}

#[test]
fn xlsx_charts_create_and_update_source_dry_runs_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-chart-create-dry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let source_path = temp_dir.join("chart-source.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &source_path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="str"><v>Region</v></c><c r="B1" t="str"><v>Sales</v></c></row>
    <row r="2"><c r="A2" t="str"><v>North</v></c><c r="B2"><v>42</v></c></row>
    <row r="3"><c r="A3" t="str"><v>South</v></c><c r="B3"><v>58</v></c></row>
    <row r="4"><c r="A4" t="str"><v>East</v></c><c r="B4"><v>30</v></c></row>
  </sheetData>
</worksheet>"#,
    );
    let source = source_path.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "charts",
        "create",
        &source,
        "--type",
        "bar",
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B4",
        "--title",
        "Sales",
        "--anchor",
        "D1",
        "--dry-run",
    ]);

    let workbook = "testdata/xlsx/chart-workbook/workbook.xlsx";
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "charts",
        "update-source",
        workbook,
        "--chart",
        "chart:1",
        "--series",
        "1",
        "--role",
        "values",
        "--source-sheet",
        "Data",
        "--source-range",
        "$B$2:$B$3",
        "--expect-source-range",
        "$B$2:$B$4",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "charts",
        "update-source",
        workbook,
        "--chart",
        "chart:1",
        "--series",
        "1",
        "--role",
        "values",
        "--source-sheet",
        "Data",
        "--source-range",
        "$B$2:$B$3",
        "--expect-source-range",
        "$B$2:$B$99",
        "--dry-run",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_charts_create_and_update_source_saved_outputs_validate_and_read_back() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-chart-create-update-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="str"><v>Region</v></c><c r="B1" t="str"><v>Sales</v></c></row>
    <row r="2"><c r="A2" t="str"><v>North</v></c><c r="B2"><v>42</v></c></row>
    <row r="3"><c r="A3" t="str"><v>South</v></c><c r="B3"><v>58</v></c></row>
    <row r="4"><c r="A4" t="str"><v>East</v></c><c r="B4"><v>30</v></c></row>
  </sheetData>
</worksheet>"#;
    let go_create_in_path = temp_dir.join("go-create-in.xlsx");
    let rust_create_in_path = temp_dir.join("rust-create-in.xlsx");
    let go_create_out_path = temp_dir.join("go-create-out.xlsx");
    let rust_create_out_path = temp_dir.join("rust-create-out.xlsx");
    write_simple_xlsx_with_sheet_xml(&go_create_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_create_in_path, sheet_xml);

    let go_create_in = go_create_in_path.to_string_lossy().to_string();
    let rust_create_in = rust_create_in_path.to_string_lossy().to_string();
    let go_create_out = go_create_out_path.to_string_lossy().to_string();
    let rust_create_out = rust_create_out_path.to_string_lossy().to_string();
    let create_go_args = [
        "--json",
        "xlsx",
        "charts",
        "create",
        &go_create_in,
        "--type",
        "bar",
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B4",
        "--title",
        "Sales",
        "--anchor",
        "D1",
        "--out",
        &go_create_out,
    ];
    let create_rust_args = [
        "--json",
        "xlsx",
        "charts",
        "create",
        &rust_create_in,
        "--type",
        "bar",
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B4",
        "--title",
        "Sales",
        "--anchor",
        "D1",
        "--out",
        &rust_create_out,
    ];
    let create_replacements = [
        (go_create_in.as_str(), "[IN]"),
        (rust_create_in.as_str(), "[IN]"),
        (go_create_out.as_str(), "[OUT]"),
        (rust_create_out.as_str(), "[OUT]"),
    ];
    let rust_create = assert_xlsx_structure_command_matches(
        "xlsx charts create",
        &create_go_args,
        &create_rust_args,
        &create_replacements,
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_create, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_create, "chartsListCommand");
    let created_chart_xml = read_zip_string(&rust_create_out_path, "xl/charts/chart1.xml");
    assert!(created_chart_xml.contains("barChart"));
    assert!(created_chart_xml.contains("'Sheet1'!$B$2:$B$4"));

    let workbook = "testdata/xlsx/chart-workbook/workbook.xlsx";
    let go_update_in_path = temp_dir.join("go-update-in.xlsx");
    let rust_update_in_path = temp_dir.join("rust-update-in.xlsx");
    let go_update_out_path = temp_dir.join("go-update-out.xlsx");
    let rust_update_out_path = temp_dir.join("rust-update-out.xlsx");
    fs::copy(workbook, &go_update_in_path).expect("go update input");
    fs::copy(workbook, &rust_update_in_path).expect("rust update input");
    let go_update_in = go_update_in_path.to_string_lossy().to_string();
    let rust_update_in = rust_update_in_path.to_string_lossy().to_string();
    let go_update_out = go_update_out_path.to_string_lossy().to_string();
    let rust_update_out = rust_update_out_path.to_string_lossy().to_string();
    let update_go_args = [
        "--json",
        "xlsx",
        "charts",
        "update-source",
        &go_update_in,
        "--chart",
        "chart:1",
        "--series",
        "1",
        "--role",
        "values",
        "--source-sheet",
        "Data",
        "--source-range",
        "$B$2:$B$3",
        "--expect-source-range",
        "$B$2:$B$4",
        "--out",
        &go_update_out,
    ];
    let update_rust_args = [
        "--json",
        "xlsx",
        "charts",
        "update-source",
        &rust_update_in,
        "--chart",
        "chart:1",
        "--series",
        "1",
        "--role",
        "values",
        "--source-sheet",
        "Data",
        "--source-range",
        "$B$2:$B$3",
        "--expect-source-range",
        "$B$2:$B$4",
        "--out",
        &rust_update_out,
    ];
    let update_replacements = [
        (go_update_in.as_str(), "[IN]"),
        (rust_update_in.as_str(), "[IN]"),
        (go_update_out.as_str(), "[OUT]"),
        (rust_update_out.as_str(), "[OUT]"),
    ];
    let rust_update = assert_xlsx_structure_command_matches(
        "xlsx charts update-source",
        &update_go_args,
        &update_rust_args,
        &update_replacements,
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_update, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "chartShowCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "rangesExportCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "sourceRangeExportCommand");
    let updated_chart_xml = read_zip_string(&rust_update_out_path, "xl/charts/chart1.xml");
    assert!(updated_chart_xml.contains("Data!$B$2:$B$3"));
    assert!(updated_chart_xml.contains(r#"<c:ptCount val="2"/>"#));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_charts_style_saved_outputs_validate_and_read_back() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-chart-style-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let cases: Vec<(&str, Vec<&str>, Vec<&str>)> = vec![
        (
            "set-title",
            vec![
                "set-title",
                "--sheet",
                "Data",
                "--chart",
                "chart:1",
                "--title",
                "Styled Revenue",
                "--font-family",
                "Aptos",
                "--font-size",
                "14",
                "--font-bold=true",
                "--font-color",
                "2255AA",
            ],
            vec!["Styled Revenue", "2255AA", "Aptos"],
        ),
        (
            "set-legend",
            vec![
                "set-legend",
                "--sheet",
                "Data",
                "--chart",
                "chart:1",
                "--position",
                "bottom",
                "--overlay=false",
            ],
            vec![r#"legendPos val="b""#, r#"overlay val="0""#],
        ),
        (
            "set-chart-area-fill",
            vec![
                "set-chart-area-fill",
                "--sheet",
                "Data",
                "--chart",
                "chart:1",
                "--fill-color",
                "FFEEDD",
            ],
            vec!["FFEEDD"],
        ),
        (
            "set-plot-area-fill",
            vec![
                "set-plot-area-fill",
                "--sheet",
                "Data",
                "--chart",
                "chart:1",
                "--fill-color",
                "CCEEFF",
            ],
            vec!["CCEEFF"],
        ),
        (
            "set-series-style",
            vec![
                "set-series-style",
                "--sheet",
                "Data",
                "--chart",
                "chart:1",
                "--series",
                "1",
                "--fill-color",
                "FF8800",
                "--line-color",
                "114477",
                "--line-width-pt",
                "2",
            ],
            vec!["FF8800", "114477", r#"w="25400""#],
        ),
    ];

    for (label, flags, xml_needles) in cases {
        let go_in_path = temp_dir.join(format!("{label}-go-input.xlsx"));
        let rust_in_path = temp_dir.join(format!("{label}-rust-input.xlsx"));
        let go_out_path = temp_dir.join(format!("{label}-go-out.xlsx"));
        let rust_out_path = temp_dir.join(format!("{label}-rust-out.xlsx"));
        fs::copy("testdata/xlsx/chart-workbook/workbook.xlsx", &go_in_path).expect("go input");
        fs::copy("testdata/xlsx/chart-workbook/workbook.xlsx", &rust_in_path).expect("rust input");

        let go_in = go_in_path.to_string_lossy().to_string();
        let rust_in = rust_in_path.to_string_lossy().to_string();
        let go_out = go_out_path.to_string_lossy().to_string();
        let rust_out = rust_out_path.to_string_lossy().to_string();

        let mut go_args = vec![
            "--json".to_string(),
            "xlsx".to_string(),
            "charts".to_string(),
            flags[0].to_string(),
            go_in.clone(),
        ];
        go_args.extend(flags.iter().skip(1).map(|value| value.to_string()));
        go_args.extend(["--out".to_string(), go_out.clone()]);

        let mut rust_args = vec![
            "--json".to_string(),
            "xlsx".to_string(),
            "charts".to_string(),
            flags[0].to_string(),
            rust_in.clone(),
        ];
        rust_args.extend(flags.iter().skip(1).map(|value| value.to_string()));
        rust_args.extend(["--out".to_string(), rust_out.clone()]);

        let go_refs = go_args.iter().map(String::as_str).collect::<Vec<_>>();
        let rust_refs = rust_args.iter().map(String::as_str).collect::<Vec<_>>();
        let replacements = [
            (go_in.as_str(), "[IN]"),
            (rust_in.as_str(), "[IN]"),
            (go_out.as_str(), "[OUT]"),
            (rust_out.as_str(), "[OUT]"),
        ];
        assert_xlsx_structure_command_matches(label, &go_refs, &rust_refs, &replacements);
        assert_xlsx_chart_style_valid_strict(&rust_out);

        let show_go_args = [
            "--json",
            "xlsx",
            "charts",
            "show",
            go_out.as_str(),
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
        ];
        let show_rust_args = [
            "--json",
            "xlsx",
            "charts",
            "show",
            rust_out.as_str(),
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
        ];
        assert_xlsx_structure_command_matches(
            &format!("{label} readback"),
            &show_go_args,
            &show_rust_args,
            &[(go_out.as_str(), "[OUT]"), (rust_out.as_str(), "[OUT]")],
        );

        let chart_xml = read_zip_string(&rust_out_path, "xl/charts/chart1.xml");
        for needle in xml_needles {
            assert!(
                chart_xml.contains(needle),
                "{label} chart XML contains {needle}"
            );
        }
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_charts_remaining_saved_outputs_validate_and_read_back() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-chart-rest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let workbook = "testdata/xlsx/chart-workbook/workbook.xlsx";
    let cases: Vec<(&str, Vec<&str>, Vec<&str>)> = vec![
        (
            "convert-type",
            vec![
                "convert-type",
                "--sheet",
                "Data",
                "--chart",
                "chart:1",
                "--to",
                "line",
                "--expect-type",
                "column",
            ],
            vec!["lineChart"],
        ),
        (
            "copy-style",
            vec![
                "copy-style",
                "--sheet",
                "Data",
                "--chart",
                "chart:1",
                "--from",
                workbook,
                "--from-chart",
                "chart:1",
            ],
            vec!["barChart", "ser"],
        ),
        (
            "set-axis",
            vec![
                "set-axis",
                "--sheet",
                "Data",
                "--chart",
                "chart:1",
                "--axis",
                "value",
                "--title",
                "Sales Axis",
                "--min",
                "0",
                "--max",
                "100",
                "--major-unit",
                "25",
                "--number-format",
                "#,##0",
                "--major-gridlines=true",
                "--minor-gridlines=true",
                "--tick-label-font-size",
                "9",
                "--tick-label-font-color",
                "333333",
                "--tick-label-font-bold=true",
                "--title-font-color",
                "2255AA",
                "--title-font-size",
                "12",
            ],
            vec![
                "Sales Axis",
                "numFmt formatCode=\"#,##0\"",
                "majorGridlines",
                "minorGridlines",
                "333333",
                "2255AA",
            ],
        ),
    ];

    for (label, flags, xml_needles) in cases {
        let go_in_path = temp_dir.join(format!("{label}-go-input.xlsx"));
        let rust_in_path = temp_dir.join(format!("{label}-rust-input.xlsx"));
        let go_out_path = temp_dir.join(format!("{label}-go-out.xlsx"));
        let rust_out_path = temp_dir.join(format!("{label}-rust-out.xlsx"));
        fs::copy(workbook, &go_in_path).expect("go input");
        fs::copy(workbook, &rust_in_path).expect("rust input");

        let go_in = go_in_path.to_string_lossy().to_string();
        let rust_in = rust_in_path.to_string_lossy().to_string();
        let go_out = go_out_path.to_string_lossy().to_string();
        let rust_out = rust_out_path.to_string_lossy().to_string();

        let mut go_args = vec![
            "--json".to_string(),
            "xlsx".to_string(),
            "charts".to_string(),
            flags[0].to_string(),
            go_in.clone(),
        ];
        go_args.extend(flags.iter().skip(1).map(|value| value.to_string()));
        go_args.extend(["--out".to_string(), go_out.clone()]);

        let mut rust_args = vec![
            "--json".to_string(),
            "xlsx".to_string(),
            "charts".to_string(),
            flags[0].to_string(),
            rust_in.clone(),
        ];
        rust_args.extend(flags.iter().skip(1).map(|value| value.to_string()));
        rust_args.extend(["--out".to_string(), rust_out.clone()]);

        let go_refs = go_args.iter().map(String::as_str).collect::<Vec<_>>();
        let rust_refs = rust_args.iter().map(String::as_str).collect::<Vec<_>>();
        let replacements = [
            (go_in.as_str(), "[IN]"),
            (rust_in.as_str(), "[IN]"),
            (go_out.as_str(), "[OUT]"),
            (rust_out.as_str(), "[OUT]"),
        ];
        assert_xlsx_structure_command_matches(label, &go_refs, &rust_refs, &replacements);
        assert_xlsx_chart_style_valid_strict(&rust_out);

        let show_go_args = [
            "--json",
            "xlsx",
            "charts",
            "show",
            go_out.as_str(),
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
        ];
        let show_rust_args = [
            "--json",
            "xlsx",
            "charts",
            "show",
            rust_out.as_str(),
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
        ];
        assert_xlsx_structure_command_matches(
            &format!("{label} readback"),
            &show_go_args,
            &show_rust_args,
            &[(go_out.as_str(), "[OUT]"), (rust_out.as_str(), "[OUT]")],
        );

        let chart_xml = read_zip_string(&rust_out_path, "xl/charts/chart1.xml");
        for needle in xml_needles {
            assert!(
                chart_xml.contains(needle),
                "{label} chart XML contains {needle}"
            );
        }
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

fn assert_xlsx_chart_style_valid_strict(path: &str) {
    let (code, stdout, stderr) = run_ooxml(&["--json", "--strict", "validate", path]);
    assert_eq!(code, 0, "strict validate exit for {path}");
    assert_eq!(stderr, None, "strict validate stderr for {path}");
    assert_eq!(
        stdout.expect("strict validate stdout")["valid"],
        Value::Bool(true),
        "strict validate result for {path}"
    );
}

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

#[test]
fn xlsx_hyperlinks_list_show_match_go_oracle() {
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "hyperlinks",
        "list",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
    ]);

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-hyperlinks-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("hyperlinks.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &workbook,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
  <hyperlinks>
    <hyperlink ref="B2:A1" location="Sheet1!C3" display="Jump" tooltip="Tip"/>
    <hyperlink ref="$C$3" r:id="rIdMissing"/>
  </hyperlinks>
</worksheet>"#,
    );
    let workbook = workbook.to_string_lossy().to_string();

    for args in [
        vec![
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &workbook,
            "--sheet",
            "Sheet1",
        ],
        vec![
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &workbook,
            "--sheet",
            "sheetId:1",
            "--include-broken",
        ],
        vec![
            "--json",
            "xlsx",
            "hyperlinks",
            "show",
            &workbook,
            "--sheet",
            "1",
            "--cell",
            "B2:A1",
        ],
        vec![
            "--json", "xlsx", "links", "show", &workbook, "--sheet", "1", "--cell", "$C$3",
        ],
    ] {
        assert_go_rust_match(&args);
    }

    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "hyperlinks",
        "show",
        &workbook,
        "--sheet",
        "1",
        "--cell",
        "Z9",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_hyperlinks_mutations_match_go_oracle_and_validate_saved_outputs() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-hyperlinks-mut-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_added_path = temp_dir.join("go-added.xlsx");
    let rust_added_path = temp_dir.join("rust-added.xlsx");
    let go_updated_path = temp_dir.join("go-updated.xlsx");
    let rust_updated_path = temp_dir.join("rust-updated.xlsx");
    let go_deleted_path = temp_dir.join("go-deleted.xlsx");
    let rust_deleted_path = temp_dir.join("rust-deleted.xlsx");
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
    let go_deleted = go_deleted_path.to_string_lossy().to_string();
    let rust_deleted = rust_deleted_path.to_string_lossy().to_string();

    let add_go = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &go_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.com/report",
        "--display",
        "Details",
        "--tooltip",
        "Open report",
        "--out",
        &go_added,
    ];
    let add_rust = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.com/report",
        "--display",
        "Details",
        "--tooltip",
        "Open report",
        "--out",
        &rust_added,
    ];
    let rust_add = assert_go_rust_match_scrubbed(
        "hyperlinks add",
        &add_go,
        &add_rust,
        &[
            (&go_in, "[IN]"),
            (&rust_in, "[IN]"),
            (&go_added, "[OUT]"),
            (&rust_added, "[OUT]"),
        ],
    )
    .expect("rust add stdout");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "hyperlinksListCommand");
    assert_xlsx_strict_valid(&rust_added);
    let added_sheet = read_zip_string(&rust_added_path, "xl/worksheets/sheet1.xml");
    assert!(
        added_sheet.contains("<hyperlinks") && added_sheet.contains(r#"ref="A1""#),
        "saved worksheet missing hyperlink element:\n{added_sheet}"
    );
    let added_rels = read_zip_string(&rust_added_path, "xl/worksheets/_rels/sheet1.xml.rels");
    assert!(
        added_rels.contains("/hyperlink")
            && added_rels.contains(r#"TargetMode="External""#)
            && added_rels.contains("https://example.com/report"),
        "saved worksheet rels missing external hyperlink:\n{added_rels}"
    );
    assert_go_rust_match_scrubbed(
        "hyperlinks added readback list",
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &go_added,
            "--sheet",
            "1",
        ],
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &rust_added,
            "--sheet",
            "1",
        ],
        &[(&go_added, "[OUT]"), (&rust_added, "[OUT]")],
    );
    assert_go_rust_match_scrubbed(
        "hyperlinks added readback show",
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "show",
            &go_added,
            "--sheet",
            "Sheet1",
            "--cell",
            "A1",
        ],
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "show",
            &rust_added,
            "--sheet",
            "Sheet1",
            "--cell",
            "A1",
        ],
        &[(&go_added, "[OUT]"), (&rust_added, "[OUT]")],
    );

    let update_go = [
        "--json",
        "xlsx",
        "hyperlinks",
        "update",
        &go_added,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.net/new",
        "--display",
        "Updated",
        "--expect-url",
        "https://example.com/report",
        "--out",
        &go_updated,
    ];
    let update_rust = [
        "--json",
        "xlsx",
        "hyperlinks",
        "update",
        &rust_added,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.net/new",
        "--display",
        "Updated",
        "--expect-url",
        "https://example.com/report",
        "--out",
        &rust_updated,
    ];
    let rust_update = assert_go_rust_match_scrubbed(
        "hyperlinks update",
        &update_go,
        &update_rust,
        &[
            (&go_added, "[IN]"),
            (&rust_added, "[IN]"),
            (&go_updated, "[OUT]"),
            (&rust_updated, "[OUT]"),
        ],
    )
    .expect("rust update stdout");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_update, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "hyperlinksListCommand");
    assert_xlsx_strict_valid(&rust_updated);
    let updated_rels = read_zip_string(&rust_updated_path, "xl/worksheets/_rels/sheet1.xml.rels");
    assert!(
        updated_rels.contains("https://example.net/new")
            && !updated_rels.contains("https://example.com/report"),
        "saved worksheet rels did not update target:\n{updated_rels}"
    );
    assert_go_rust_match_scrubbed(
        "hyperlinks updated readback list",
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &go_updated,
            "--sheet",
            "1",
        ],
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &rust_updated,
            "--sheet",
            "1",
        ],
        &[(&go_updated, "[OUT]"), (&rust_updated, "[OUT]")],
    );

    let delete_go = [
        "--json",
        "xlsx",
        "hyperlinks",
        "delete",
        &go_updated,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--expect-url",
        "https://example.net/new",
        "--out",
        &go_deleted,
    ];
    let delete_rust = [
        "--json",
        "xlsx",
        "hyperlinks",
        "delete",
        &rust_updated,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--expect-url",
        "https://example.net/new",
        "--out",
        &rust_deleted,
    ];
    let rust_delete = assert_go_rust_match_scrubbed(
        "hyperlinks delete",
        &delete_go,
        &delete_rust,
        &[
            (&go_updated, "[IN]"),
            (&rust_updated, "[IN]"),
            (&go_deleted, "[OUT]"),
            (&rust_deleted, "[OUT]"),
        ],
    )
    .expect("rust delete stdout");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete, "hyperlinksListCommand");
    assert_xlsx_strict_valid(&rust_deleted);
    assert_go_rust_match_scrubbed(
        "hyperlinks deleted readback list",
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &go_deleted,
            "--sheet",
            "1",
        ],
        &[
            "--json",
            "xlsx",
            "hyperlinks",
            "list",
            &rust_deleted,
            "--sheet",
            "1",
        ],
        &[(&go_deleted, "[OUT]"), (&rust_deleted, "[OUT]")],
    );
    let deleted_sheet = read_zip_string(&rust_deleted_path, "xl/worksheets/sheet1.xml");
    assert!(
        !deleted_sheet.contains("<hyperlink"),
        "delete left hyperlink XML:\n{deleted_sheet}"
    );
    let deleted_rels = read_zip_string(&rust_deleted_path, "xl/worksheets/_rels/sheet1.xml.rels");
    assert!(
        !deleted_rels.contains("/hyperlink") && !deleted_rels.contains("https://example.net/new"),
        "delete left hyperlink rel:\n{deleted_rels}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_hyperlinks_dry_run_and_errors_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-hyperlinks-dry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_added_path = temp_dir.join("go-added.xlsx");
    let rust_added_path = temp_dir.join("rust-added.xlsx");
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

    let before_sheet = read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml");
    let dry_go = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &go_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "B2",
        "--location",
        "Sheet1!A1",
        "--display",
        "Jump",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "B2",
        "--location",
        "Sheet1!A1",
        "--display",
        "Jump",
        "--dry-run",
    ];
    let rust_dry = assert_go_rust_match_scrubbed(
        "hyperlinks add dry-run",
        &dry_go,
        &dry_rust,
        &[(&go_in, "[IN]"), (&rust_in, "[IN]")],
    )
    .expect("rust dry-run stdout");
    assert_eq!(rust_dry["dryRun"], Value::Bool(true));
    assert!(
        rust_dry.get("output").is_none(),
        "dry-run should not emit output"
    );
    assert!(
        rust_dry.get("hyperlinksListCommand").is_none(),
        "dry-run should not emit readback command"
    );
    assert_eq!(
        read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml"),
        before_sheet,
        "dry-run mutated source worksheet"
    );
    assert!(
        !zip_entry_exists(&rust_in_path, "xl/worksheets/_rels/sheet1.xml.rels"),
        "dry-run created worksheet rels"
    );

    for (label, extra) in [
        (
            "missing target",
            vec![
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--display",
                "No target",
                "--dry-run",
            ],
        ),
        (
            "two targets",
            vec![
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--url",
                "https://example.com",
                "--location",
                "Sheet1!A1",
                "--dry-run",
            ],
        ),
    ] {
        let mut go_args = vec!["--json", "xlsx", "hyperlinks", "add", &go_in];
        go_args.extend(extra.iter().copied());
        let mut rust_args = vec!["--json", "xlsx", "hyperlinks", "add", &rust_in];
        rust_args.extend(extra.iter().copied());
        assert_go_rust_match_scrubbed(
            &format!("hyperlinks add error {label}"),
            &go_args,
            &rust_args,
            &[(&go_in, "[IN]"), (&rust_in, "[IN]")],
        );
    }

    let add_go = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &go_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.com/original",
        "--out",
        &go_added,
    ];
    let add_rust = [
        "--json",
        "xlsx",
        "hyperlinks",
        "add",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--url",
        "https://example.com/original",
        "--out",
        &rust_added,
    ];
    assert_go_rust_match_scrubbed(
        "hyperlinks add setup",
        &add_go,
        &add_rust,
        &[
            (&go_in, "[IN]"),
            (&rust_in, "[IN]"),
            (&go_added, "[OUT]"),
            (&rust_added, "[OUT]"),
        ],
    );

    for (label, command, extra) in [
        (
            "guard mismatch",
            "update",
            vec![
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--url",
                "https://example.com/next",
                "--expect-url",
                "https://wrong.example",
                "--dry-run",
            ],
        ),
        (
            "update two targets",
            "update",
            vec![
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--url",
                "https://example.com/next",
                "--location",
                "Sheet1!B2",
                "--dry-run",
            ],
        ),
        (
            "delete guard mismatch",
            "delete",
            vec![
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--expect-location",
                "Sheet1!B2",
                "--dry-run",
            ],
        ),
    ] {
        let mut go_args = vec!["--json", "xlsx", "hyperlinks", command, &go_added];
        go_args.extend(extra.iter().copied());
        let mut rust_args = vec!["--json", "xlsx", "hyperlinks", command, &rust_added];
        rust_args.extend(extra.iter().copied());
        assert_go_rust_match_scrubbed(
            &format!("hyperlinks {label}"),
            &go_args,
            &rust_args,
            &[(&go_added, "[IN]"), (&rust_added, "[IN]")],
        );
    }

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

#[test]
fn xlsx_ranges_set_matches_go_oracle_and_saved_output() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-ranges-set-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in = temp_dir.join("go-in.xlsx");
    let rust_in = temp_dir.join("rust-in.xlsx");
    let go_out = temp_dir.join("go-out.xlsx");
    let rust_out = temp_dir.join("rust-out.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in).expect("stage go input");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &rust_in).expect("stage rust input");
    let go_in = go_in.to_string_lossy().to_string();
    let rust_in = rust_in.to_string_lossy().to_string();
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();
    let values = r#"[["Name",{"value":"42.5","type":"number"},{"formula":"SUM(B1:B1)"}],[null,true,"tail"]]"#;

    let go_args = [
        "--json", "xlsx", "ranges", "set", &go_in, "--sheet", "Sheet1", "--range", "A1:C2",
        "--values", values, "--out", &go_out,
    ];
    let rust_args = [
        "--json", "xlsx", "ranges", "set", &rust_in, "--sheet", "Sheet1", "--range", "A1:C2",
        "--values", values, "--out", &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "ranges set exit");
    assert_eq!(rust_stderr, go_stderr, "ranges set stderr");
    let go_json = scrub_paths(
        go_stdout.expect("go ranges set stdout"),
        &[(&go_in, "[IN]"), (&go_out, "[OUT]")],
    );
    let rust_json = scrub_paths(
        rust_stdout.expect("rust ranges set stdout"),
        &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")],
    );
    assert_eq!(rust_json, go_json, "ranges set stdout");

    let export_args_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C2",
        "--include-types",
        "--include-formulas",
    ];
    let export_args_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C2",
        "--include-types",
        "--include-formulas",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_args_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_args_rust);
    assert_eq!(rust_code, go_code, "saved output export exit");
    assert_eq!(rust_stderr, go_stderr, "saved output export stderr");
    assert_eq!(
        scrub_path(rust_export.expect("rust saved export"), &rust_out, "[OUT]"),
        scrub_path(go_export.expect("go saved export"), &go_out, "[OUT]"),
        "saved output readback"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B1",
        "--values",
        r#"[["Dry",1]]"#,
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B1",
        "--values",
        r#"[["Dry",1]]"#,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "ranges set dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "ranges set dry-run stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust dry-run stdout"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go dry-run stdout"), &go_in, "[IN]"),
        "ranges set dry-run stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_formula_recalc_metadata_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-set-formula-recalc-{}",
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
  <dimension ref="A1:B1"/>
  <sheetData><row r="1"><c r="B1"><v>7</v></c></row></sheetData>
</worksheet>"#;
    write_simple_xlsx_with_sheet_xml(&go_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_in_path, sheet_xml);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();
    let values = r#"[[{"formula":"SUM(B1:B1)"}]]"#;

    let go_args = [
        "--json", "xlsx", "ranges", "set", &go_in, "--sheet", "Sheet1", "--range", "C1:C1",
        "--values", values, "--out", &go_out,
    ];
    let rust_args = [
        "--json", "xlsx", "ranges", "set", &rust_in, "--sheet", "Sheet1", "--range", "C1:C1",
        "--values", values, "--out", &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "formula recalc exit");
    assert_eq!(rust_stderr, go_stderr, "formula recalc stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust formula recalc stdout"),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go formula recalc stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "formula recalc stdout"
    );
    assert_xlsx_full_calc_flags(&go_out_path);
    assert_xlsx_full_calc_flags(&rust_out_path);
    assert!(
        !read_zip_string(&rust_out_path, "xl/worksheets/sheet1.xml")
            .contains(r#"<c r="C1"><f>SUM(B1:B1)</f><v>"#),
        "new Rust formula should not have a cached value"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_freeze_show_set_clear_matches_go_oracle_and_saved_output() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-freeze-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_frozen_path = temp_dir.join("go-frozen.xlsx");
    let rust_frozen_path = temp_dir.join("rust-frozen.xlsx");
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
    let go_frozen = go_frozen_path.to_string_lossy().to_string();
    let rust_frozen = rust_frozen_path.to_string_lossy().to_string();
    let go_cleared = go_cleared_path.to_string_lossy().to_string();
    let rust_cleared = rust_cleared_path.to_string_lossy().to_string();

    let show_go = ["--json", "xlsx", "freeze", "show", &go_in, "--sheet", "1"];
    let show_rust = ["--json", "xlsx", "freeze", "show", &rust_in, "--sheet", "1"];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "initial show exit");
    assert_eq!(rust_stderr, go_stderr, "initial show stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust initial show"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go initial show"), &go_in, "[IN]"),
        "initial show stdout"
    );

    let set_go = [
        "--json", "xlsx", "freeze", "set", &go_in, "--sheet", "1", "--rows", "1", "--cols", "1",
        "--out", &go_frozen,
    ];
    let set_rust = [
        "--json",
        "xlsx",
        "freeze",
        "set",
        &rust_in,
        "--sheet",
        "1",
        "--rows",
        "1",
        "--cols",
        "1",
        "--out",
        &rust_frozen,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&set_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&set_rust);
    assert_eq!(rust_code, go_code, "set exit");
    assert_eq!(rust_stderr, go_stderr, "set stderr");
    let rust_set = rust_stdout.expect("rust set stdout");
    assert_eq!(
        scrub_paths(
            rust_set.clone(),
            &[(&rust_in, "[IN]"), (&rust_frozen, "[FROZEN]")]
        ),
        scrub_paths(
            go_stdout.expect("go set stdout"),
            &[(&go_in, "[IN]"), (&go_frozen, "[FROZEN]")]
        ),
        "set stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_set, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_set, "showCommand");

    let show_go_frozen = [
        "--json", "xlsx", "freeze", "show", &go_frozen, "--sheet", "1",
    ];
    let show_rust_frozen = [
        "--json",
        "xlsx",
        "freeze",
        "show",
        &rust_frozen,
        "--sheet",
        "1",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&show_go_frozen);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&show_rust_frozen);
    assert_eq!(rust_code, go_code, "frozen show exit");
    assert_eq!(rust_stderr, go_stderr, "frozen show stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust frozen show"),
            &rust_frozen,
            "[FROZEN]"
        ),
        scrub_path(go_stdout.expect("go frozen show"), &go_frozen, "[FROZEN]"),
        "frozen show stdout"
    );

    let clear_go = [
        "--json",
        "xlsx",
        "freeze",
        "clear",
        &go_frozen,
        "--sheet",
        "1",
        "--out",
        &go_cleared,
    ];
    let clear_rust = [
        "--json",
        "xlsx",
        "freeze",
        "clear",
        &rust_frozen,
        "--sheet",
        "1",
        "--out",
        &rust_cleared,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&clear_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&clear_rust);
    assert_eq!(rust_code, go_code, "clear exit");
    assert_eq!(rust_stderr, go_stderr, "clear stderr");
    let rust_clear = rust_stdout.expect("rust clear stdout");
    assert_eq!(
        scrub_paths(
            rust_clear.clone(),
            &[(&rust_frozen, "[FROZEN]"), (&rust_cleared, "[CLEARED]")]
        ),
        scrub_paths(
            go_stdout.expect("go clear stdout"),
            &[(&go_frozen, "[FROZEN]"), (&go_cleared, "[CLEARED]")]
        ),
        "clear stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_clear, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_clear, "showCommand");

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "xlsx",
        "freeze",
        "show",
        &go_cleared,
        "--sheet",
        "1",
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "freeze",
        "show",
        &rust_cleared,
        "--sheet",
        "1",
    ]);
    assert_eq!(rust_code, go_code, "cleared show exit");
    assert_eq!(rust_stderr, go_stderr, "cleared show stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust cleared show"),
            &rust_cleared,
            "[CLEARED]"
        ),
        scrub_path(
            go_stdout.expect("go cleared show"),
            &go_cleared,
            "[CLEARED]"
        ),
        "cleared show stdout"
    );
    assert!(
        !read_zip_string(&rust_cleared_path, "xl/worksheets/sheet1.xml").contains("<pane"),
        "clear should remove frozen pane"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_freeze_dry_run_and_errors_match_go_oracle() {
    type BadFreezeCase<'a> = (
        Vec<&'a str>,
        Vec<&'a str>,
        Vec<(&'a str, &'a str)>,
        Vec<(&'a str, &'a str)>,
    );

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-freeze-errors-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in_path).expect("go input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();

    let dry_go = [
        "--json",
        "xlsx",
        "freeze",
        "set",
        &go_in,
        "--sheet",
        "1",
        "--rows",
        "2",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "freeze",
        "set",
        &rust_in,
        "--sheet",
        "1",
        "--rows",
        "2",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "dry-run stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust dry-run stdout"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go dry-run stdout"), &go_in, "[IN]"),
        "dry-run stdout"
    );
    assert!(
        !read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml").contains("<pane"),
        "dry-run should not mutate source workbook"
    );

    let go_out = temp_dir.join("go-out.xlsx").to_string_lossy().to_string();
    let rust_out = temp_dir.join("rust-out.xlsx").to_string_lossy().to_string();
    let bad_cases: Vec<BadFreezeCase<'_>> = vec![
        (
            vec![
                "--json", "xlsx", "freeze", "set", &go_in, "--sheet", "1", "--rows", "0", "--cols",
                "0", "--out", &go_out,
            ],
            vec![
                "--json", "xlsx", "freeze", "set", &rust_in, "--sheet", "1", "--rows", "0",
                "--cols", "0", "--out", &rust_out,
            ],
            vec![(&go_in, "[IN]"), (&go_out, "[OUT]")],
            vec![(&rust_in, "[IN]"), (&rust_out, "[OUT]")],
        ),
        (
            vec![
                "--json", "xlsx", "freeze", "set", &go_in, "--sheet", "1", "--rows", "1048576",
                "--out", &go_out,
            ],
            vec![
                "--json", "xlsx", "freeze", "set", &rust_in, "--sheet", "1", "--rows", "1048576",
                "--out", &rust_out,
            ],
            vec![(&go_in, "[IN]"), (&go_out, "[OUT]")],
            vec![(&rust_in, "[IN]"), (&rust_out, "[OUT]")],
        ),
        (
            vec![
                "--json",
                "xlsx",
                "freeze",
                "set",
                &go_in,
                "--sheet",
                "1",
                "--rows",
                "1",
                "--expect-state",
                "frozen",
                "--out",
                &go_out,
            ],
            vec![
                "--json",
                "xlsx",
                "freeze",
                "set",
                &rust_in,
                "--sheet",
                "1",
                "--rows",
                "1",
                "--expect-state",
                "frozen",
                "--out",
                &rust_out,
            ],
            vec![(&go_in, "[IN]"), (&go_out, "[OUT]")],
            vec![(&rust_in, "[IN]"), (&rust_out, "[OUT]")],
        ),
        (
            vec![
                "--json", "xlsx", "freeze", "clear", &go_in, "--sheet", "1", "--out", &go_out,
            ],
            vec![
                "--json", "xlsx", "freeze", "clear", &rust_in, "--sheet", "1", "--out", &rust_out,
            ],
            vec![(&go_in, "[IN]"), (&go_out, "[OUT]")],
            vec![(&rust_in, "[IN]"), (&rust_out, "[OUT]")],
        ),
    ];
    for (go_args, rust_args, go_paths, rust_paths) in bad_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, go_code, "bad case exit for {go_args:?}");
        assert_eq!(rust_stdout, go_stdout, "bad case stdout for {go_args:?}");
        assert_eq!(
            scrub_paths(rust_stderr.expect("rust bad case stderr"), &rust_paths),
            scrub_paths(go_stderr.expect("go bad case stderr"), &go_paths),
            "bad case stderr for {go_args:?}"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

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

#[test]
fn xlsx_ranges_set_formula_overwrite_invalidates_calc_chain_like_go() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-set-calc-chain-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    write_calc_chain_xlsx(&go_in_path);
    write_calc_chain_xlsx(&rust_in_path);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();
    let values = r#"[[{"value":"9","type":"number"}]]"#;

    let go_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--values",
        values,
        "--overwrite-formulas",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--values",
        values,
        "--overwrite-formulas",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "calc-chain invalidation exit");
    assert_eq!(rust_stderr, go_stderr, "calc-chain invalidation stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust calc-chain invalidation stdout"),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go calc-chain invalidation stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "calc-chain invalidation stdout"
    );
    assert_xlsx_full_calc_flags(&go_out_path);
    assert_xlsx_full_calc_flags(&rust_out_path);
    assert_xlsx_calc_chain_removed(&go_out_path);
    assert_xlsx_calc_chain_removed(&rust_out_path);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_formula_clear_invalidates_calc_chain_like_go() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-set-clear-calc-chain-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    write_calc_chain_xlsx(&go_in_path);
    write_calc_chain_xlsx(&rust_in_path);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--values",
        "[[null]]",
        "--null-policy",
        "clear",
        "--overwrite-formulas",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--values",
        "[[null]]",
        "--null-policy",
        "clear",
        "--overwrite-formulas",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "clear calc-chain invalidation exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "clear calc-chain invalidation stderr"
    );
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust clear calc-chain invalidation stdout"),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go clear calc-chain invalidation stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "clear calc-chain invalidation stdout"
    );
    assert_xlsx_full_calc_flags(&go_out_path);
    assert_xlsx_full_calc_flags(&rust_out_path);
    assert_xlsx_calc_chain_removed(&go_out_path);
    assert_xlsx_calc_chain_removed(&rust_out_path);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_cells_set_matches_go_oracle_and_emitted_commands_run() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-cells-set-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in_path).expect("stage go input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("stage rust input");
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json", "xlsx", "cells", "set", &go_in, "--sheet", "Sheet1", "--cell", "B2", "--value",
        "42.50", "--type", "number", "--out", &go_out,
    ];
    let rust_args = [
        "--json", "xlsx", "cells", "set", &rust_in, "--sheet", "Sheet1", "--cell", "B2", "--value",
        "42.50", "--type", "number", "--out", &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "cells set exit");
    assert_eq!(rust_stderr, go_stderr, "cells set stderr");
    let go_json = scrub_paths(
        go_stdout.expect("go cells set stdout"),
        &[(&go_in, "[IN]"), (&go_out, "[OUT]")],
    );
    let rust_raw = rust_stdout.expect("rust cells set stdout");
    let rust_json = scrub_paths(
        rust_raw.clone(),
        &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")],
    );
    assert_eq!(rust_json, go_json, "cells set stdout");
    for field in [
        "validateCommand",
        "cellsExtractCommand",
        "rangesExportCommand",
    ] {
        assert_rust_emitted_ooxml_command_succeeds(&rust_raw, field);
    }

    let export_args_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Sheet1",
        "--range",
        "B2",
        "--include-types",
        "--include-formulas",
    ];
    let export_args_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Sheet1",
        "--range",
        "B2",
        "--include-types",
        "--include-formulas",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_args_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_args_rust);
    assert_eq!(rust_code, go_code, "saved cells set export exit");
    assert_eq!(rust_stderr, go_stderr, "saved cells set export stderr");
    assert_eq!(
        scrub_path(rust_export.expect("rust saved export"), &rust_out, "[OUT]"),
        scrub_path(go_export.expect("go saved export"), &go_out, "[OUT]"),
        "saved cells set readback"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "cells",
        "set",
        &go_in,
        "--sheet",
        "MissingSheetIsIgnoredForHandle",
        "--cell",
        "H:xlsx/ws:1/cell:a:A1",
        "--value",
        "handled",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "cells",
        "set",
        &rust_in,
        "--sheet",
        "MissingSheetIsIgnoredForHandle",
        "--cell",
        "H:xlsx/ws:1/cell:a:A1",
        "--value",
        "handled",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "cells set handle dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "cells set handle dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust handle dry-run stdout"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(go_stdout.expect("go handle dry-run stdout"), &go_in, "[IN]"),
        "cells set handle dry-run stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_cells_clear_matches_go_oracle_saved_dry_run_and_errors() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cells-clear-{}",
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
  <dimension ref="A1:C2"/>
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>alpha</t></is></c>
      <c r="B1"><v>7</v></c>
      <c r="C1"><f>B1*2</f><v>14</v></c>
    </row>
    <row r="2"><c r="A2"><v>99</v></c></row>
  </sheetData>
</worksheet>"#;
    write_simple_xlsx_with_sheet_xml(&go_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_in_path, sheet_xml);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json", "xlsx", "cells", "clear", &go_in, "--sheet", "Sheet1", "--range", "A1:C1",
        "--out", &go_out,
    ];
    let rust_args = [
        "--json", "xlsx", "cells", "clear", &rust_in, "--sheet", "Sheet1", "--range", "A1:C1",
        "--out", &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "cells clear exit");
    assert_eq!(rust_stderr, go_stderr, "cells clear stderr");
    let rust_raw = rust_stdout.expect("rust cells clear stdout");
    assert_eq!(
        scrub_paths(
            rust_raw.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go cells clear stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "cells clear stdout"
    );
    for field in [
        "validateCommand",
        "cellsExtractCommand",
        "rangesExportCommand",
    ] {
        assert_rust_emitted_ooxml_command_succeeds(&rust_raw, field);
    }
    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "cells clear strict validate exit");
    assert_eq!(validate_stderr, None, "cells clear strict validate stderr");
    assert!(
        validate_stdout.is_some(),
        "cells clear strict validate stdout"
    );

    let export_args_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C2",
        "--include-types",
        "--include-formulas",
    ];
    let export_args_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C2",
        "--include-types",
        "--include-formulas",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_args_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_args_rust);
    assert_eq!(rust_code, go_code, "saved cells clear export exit");
    assert_eq!(rust_stderr, go_stderr, "saved cells clear export stderr");
    assert_eq!(
        scrub_path(
            rust_export.expect("rust clear saved export"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(go_export.expect("go clear saved export"), &go_out, "[OUT]"),
        "saved cells clear readback"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "cells",
        "clear",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C1",
        "--readback-max-cells",
        "1",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "cells",
        "clear",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C1",
        "--readback-max-cells",
        "1",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "cells clear dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "cells clear dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust clear dry-run stdout"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(go_stdout.expect("go clear dry-run stdout"), &go_in, "[IN]"),
        "cells clear dry-run stdout"
    );

    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "cells",
        "clear",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "Sheet1",
        "--range",
        "A1",
        "--ref",
        "A1",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "cells",
        "clear",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "Sheet1",
        "--dry-run",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_cells_set_batch_matches_go_oracle_saved_stdin_and_errors() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cells-set-batch-{}",
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
  <dimension ref="A1:B1"/>
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>seed</t></is></c>
      <c r="B1"><v>42</v></c>
    </row>
  </sheetData>
</worksheet>"#;
    write_simple_xlsx_with_sheet_xml(&go_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_in_path, sheet_xml);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();
    let cells = r#"[{"ref":"B1","value":"64","type":"number"},{"ref":"A2","value":"batch","type":"string"},{"ref":"C2","formula":"SUM(B1:B1)"}]"#;

    let go_args = [
        "--json",
        "xlsx",
        "cells",
        "set-batch",
        &go_in,
        "--sheet",
        "Sheet1",
        "--cells",
        cells,
        "--details",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "cells",
        "set-batch",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cells",
        cells,
        "--details",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "cells set-batch exit");
    assert_eq!(rust_stderr, go_stderr, "cells set-batch stderr");
    let rust_raw = rust_stdout.expect("rust cells set-batch stdout");
    assert_eq!(
        scrub_paths(
            rust_raw.clone(),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go cells set-batch stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "cells set-batch stdout"
    );
    for field in [
        "validateCommand",
        "cellsExtractCommand",
        "rangesExportCommand",
    ] {
        assert_rust_emitted_ooxml_command_succeeds(&rust_raw, field);
    }
    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "cells set-batch strict validate exit");
    assert_eq!(
        validate_stderr, None,
        "cells set-batch strict validate stderr"
    );
    assert!(
        validate_stdout.is_some(),
        "cells set-batch strict validate stdout"
    );

    let export_args_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C2",
        "--include-types",
        "--include-formulas",
    ];
    let export_args_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C2",
        "--include-types",
        "--include-formulas",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_args_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_args_rust);
    assert_eq!(rust_code, go_code, "saved cells set-batch export exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "saved cells set-batch export stderr"
    );
    assert_eq!(
        scrub_path(
            rust_export.expect("rust set-batch saved export"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(
            go_export.expect("go set-batch saved export"),
            &go_out,
            "[OUT]"
        ),
        "saved cells set-batch readback"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "cells",
        "set-batch",
        &go_in,
        "--sheet",
        "Sheet1",
        "--cells-file",
        "-",
        "--details",
        "--readback-max-cells",
        "2",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "cells",
        "set-batch",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--cells-file",
        "-",
        "--details",
        "--readback-max-cells",
        "2",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml_with_input(&dry_go, cells);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml_with_input(&dry_rust, cells);
    assert_eq!(rust_code, go_code, "cells set-batch stdin dry-run exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "cells set-batch stdin dry-run stderr"
    );
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust set-batch stdin dry-run stdout"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(
            go_stdout.expect("go set-batch stdin dry-run stdout"),
            &go_in,
            "[IN]"
        ),
        "cells set-batch stdin dry-run stdout"
    );

    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "cells",
        "set-batch",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "Sheet1",
        "--cells",
        "[]",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "cells",
        "set-batch",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "Sheet1",
        "--cells",
        r#"[{"ref":"A1","value":"x"}]"#,
        "--cells-file",
        "ignored.json",
        "--dry-run",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_format_matches_go_oracle_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-set-format-{}",
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
  <dimension ref="A1:B1"/>
  <sheetData>
    <row r="1">
      <c r="A1"><v>1234.5</v></c>
      <c r="B1"><f>A1*2</f><v>2469</v></c>
    </row>
  </sheetData>
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
        "ranges",
        "set-format",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--preset",
        "currency",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--preset",
        "currency",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "ranges set-format exit");
    assert_eq!(rust_stderr, go_stderr, "ranges set-format stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust ranges set-format stdout"),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go ranges set-format stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "ranges set-format stdout"
    );

    let export_args_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ];
    let export_args_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_args_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_args_rust);
    assert_eq!(rust_code, go_code, "saved output export exit");
    assert_eq!(rust_stderr, go_stderr, "saved output export stderr");
    assert_eq!(
        scrub_path(rust_export.expect("rust saved export"), &rust_out, "[OUT]"),
        scrub_path(go_export.expect("go saved export"), &go_out, "[OUT]"),
        "saved output format readback"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "C3",
        "--preset",
        "percent",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "C3",
        "--preset",
        "percent",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "ranges set-format dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "ranges set-format dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust set-format dry-run stdout"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(
            go_stdout.expect("go set-format dry-run stdout"),
            &go_in,
            "[IN]"
        ),
        "ranges set-format dry-run stdout"
    );
    assert!(
        !zip_entry_exists(&rust_in_path, "xl/styles.xml"),
        "dry-run wrote styles.xml into Rust input workbook"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_style_matches_go_oracle_and_preserves_number_formats() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-set-style-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_format_path = temp_dir.join("go-format.xlsx");
    let rust_format_path = temp_dir.join("rust-format.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B1"/>
  <sheetData>
    <row r="1">
      <c r="A1"><v>1234.5</v></c>
      <c r="B1"><f>A1*2</f><v>2469</v></c>
    </row>
  </sheetData>
</worksheet>"#;
    write_simple_xlsx_with_sheet_xml(&go_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_in_path, sheet_xml);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_format = go_format_path.to_string_lossy().to_string();
    let rust_format = rust_format_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_format_args = [
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B1",
        "--preset",
        "currency",
        "--out",
        &go_format,
    ];
    let rust_format_args = [
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B1",
        "--preset",
        "currency",
        "--out",
        &rust_format,
    ];
    let (go_code, _, go_stderr) = run_go_ooxml(&go_format_args);
    let (rust_code, _, rust_stderr) = run_ooxml(&rust_format_args);
    assert_eq!(rust_code, go_code, "style setup set-format exit");
    assert_eq!(rust_stderr, go_stderr, "style setup set-format stderr");

    let go_args = [
        "--json",
        "xlsx",
        "ranges",
        "set-style",
        &go_format,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--font-name",
        "Aptos",
        "--font-size",
        "11",
        "--font-bold",
        "--font-color",
        "#FF0000",
        "--fill-color",
        "#FFF2CC",
        "--border-style",
        "thin",
        "--border-color",
        "#4472C4",
        "--alignment-horizontal",
        "center",
        "--alignment-wrap-text",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "ranges",
        "set-style",
        &rust_format,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--font-name",
        "Aptos",
        "--font-size",
        "11",
        "--font-bold",
        "--font-color",
        "#FF0000",
        "--fill-color",
        "#FFF2CC",
        "--border-style",
        "thin",
        "--border-color",
        "#4472C4",
        "--alignment-horizontal",
        "center",
        "--alignment-wrap-text",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "ranges set-style exit");
    assert_eq!(rust_stderr, go_stderr, "ranges set-style stderr");
    let rust_raw = rust_stdout.expect("rust ranges set-style stdout");
    assert_eq!(
        scrub_paths(
            rust_raw.clone(),
            &[(&rust_format, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go ranges set-style stdout"),
            &[(&go_format, "[IN]"), (&go_out, "[OUT]")]
        ),
        "ranges set-style stdout"
    );
    for field in ["validateCommand", "rangesExportCommand"] {
        assert_rust_emitted_ooxml_command_succeeds(&rust_raw, field);
    }
    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "ranges set-style strict validate exit");
    assert_eq!(
        validate_stderr, None,
        "ranges set-style strict validate stderr"
    );
    assert!(
        validate_stdout.is_some(),
        "ranges set-style strict validate stdout"
    );

    let export_args_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ];
    let export_args_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_args_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_args_rust);
    assert_eq!(rust_code, go_code, "saved ranges set-style export exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "saved ranges set-style export stderr"
    );
    assert_eq!(
        scrub_path(
            rust_export.expect("rust set-style saved export"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(
            go_export.expect("go set-style saved export"),
            &go_out,
            "[OUT]"
        ),
        "saved ranges set-style readback"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "ranges",
        "set-style",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "C3",
        "--font-bold",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "ranges",
        "set-style",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "C3",
        "--font-bold",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "ranges set-style dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "ranges set-style dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust set-style dry-run stdout"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(
            go_stdout.expect("go set-style dry-run stdout"),
            &go_in,
            "[IN]"
        ),
        "ranges set-style dry-run stdout"
    );
    assert!(
        !zip_entry_exists(&rust_in_path, "xl/styles.xml"),
        "dry-run wrote styles.xml into Rust input workbook"
    );

    for args in [
        vec![
            "--json",
            "xlsx",
            "ranges",
            "set-style",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--range",
            "A1",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "ranges",
            "set-style",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--range",
            "A1",
            "--fill-color",
            "#12",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "ranges",
            "set-style",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--range",
            "A1",
            "--border-style",
            "zigzag",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "ranges",
            "set-style",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--range",
            "A1",
            "--alignment-horizontal",
            "middle",
            "--dry-run",
        ],
    ] {
        assert_go_rust_match(&args);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_format_range_edges_match_go_oracle() {
    let file = "testdata/xlsx/minimal-workbook/workbook.xlsx";
    for range in ["A1B2", "A0", "A1:B2:C3", ":B2"] {
        assert_go_rust_match(&[
            "--json",
            "xlsx",
            "ranges",
            "set-format",
            file,
            "--sheet",
            "Sheet1",
            "--range",
            range,
            "--preset",
            "number",
            "--dry-run",
        ]);
    }
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        file,
        "--sheet",
        "Sheet1",
        "--range",
        "B2:A1",
        "--preset",
        "number",
        "--dry-run",
    ]);
}

#[test]
fn xlsx_ranges_set_delimited_and_stdin_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-delimited-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in = temp_dir.join("go-csv-in.xlsx");
    let rust_in = temp_dir.join("rust-csv-in.xlsx");
    let go_out = temp_dir.join("go-csv-out.xlsx");
    let rust_out = temp_dir.join("rust-csv-out.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in).expect("stage go input");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &rust_in).expect("stage rust input");
    let go_in = go_in.to_string_lossy().to_string();
    let rust_in = rust_in.to_string_lossy().to_string();
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();
    let csv = "Name,Value\nAlpha,\"two, too\"\nBeta,\"multi\nline\"\n";
    let go_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B3",
        "--data-format",
        "csv",
        "--values-file",
        "-",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B3",
        "--data-format",
        "csv",
        "--values-file",
        "-",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml_with_input(&go_args, csv);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml_with_input(&rust_args, csv);
    assert_eq!(rust_code, go_code, "CSV stdin ranges set exit");
    assert_eq!(rust_stderr, go_stderr, "CSV stdin ranges set stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust CSV stdin stdout"),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go CSV stdin stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "CSV stdin ranges set stdout"
    );

    let export_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B3",
        "--include-types",
        "--include-formulas",
    ];
    let export_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B3",
        "--include-types",
        "--include-formulas",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_rust);
    assert_eq!(rust_code, go_code, "CSV stdin saved export exit");
    assert_eq!(rust_stderr, go_stderr, "CSV stdin saved export stderr");
    assert_eq!(
        scrub_path(
            rust_export.expect("rust CSV saved export"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(go_export.expect("go CSV saved export"), &go_out, "[OUT]"),
        "CSV stdin saved readback"
    );

    let tsv = "Name\tValue\nAlpha\ttwo\n";
    let go_tsv_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--data-format",
        "tsv",
        "--values",
        tsv,
        "--dry-run",
    ];
    let rust_tsv_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--data-format",
        "tsv",
        "--values",
        tsv,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_tsv_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_tsv_args);
    assert_eq!(rust_code, go_code, "TSV ranges set exit");
    assert_eq!(rust_stderr, go_stderr, "TSV ranges set stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust TSV stdout"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go TSV stdout"), &go_in, "[IN]"),
        "TSV ranges set stdout"
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_preserves_untouched_cell_xml() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-preserve-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("output.xlsx");
    write_preservation_xlsx(&input);
    let input_s = input.to_string_lossy().to_string();
    let output_s = output.to_string_lossy().to_string();

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "set",
        &input_s,
        "--sheet",
        "Sheet1",
        "--range",
        "D1:D1",
        "--values",
        r#"[["new"]]"#,
        "--out",
        &output_s,
    ]);
    assert_eq!(code, 0, "preservation edit stderr={stderr:?}");
    assert!(stdout.is_some(), "preservation edit stdout");
    let sheet_xml = read_zip_string(&output, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains(r#"<c r="A1" t="s"><v>0</v></c>"#),
        "shared-string cell changed:\n{sheet_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B1" s="1"><v>45123</v></c>"#),
        "styled/date cell changed:\n{sheet_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="C1"><f>B1*2</f><v>90246</v></c>"#),
        "formula cache cell changed:\n{sheet_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="D1" t="inlineStr"><is><t>new</t></is></c>"#),
        "new cell missing:\n{sheet_xml}"
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_in_place_backup_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-in-place-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in = temp_dir.join("go.xlsx");
    let rust_in = temp_dir.join("rust.xlsx");
    let go_backup = temp_dir.join("go.xlsx.bak");
    let rust_backup = temp_dir.join("rust.xlsx.bak");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in).expect("stage go input");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &rust_in).expect("stage rust input");
    let go_in = go_in.to_string_lossy().to_string();
    let rust_in = rust_in.to_string_lossy().to_string();
    let go_backup = go_backup.to_string_lossy().to_string();
    let rust_backup = rust_backup.to_string_lossy().to_string();
    let go_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--values",
        r#"[["In place"]]"#,
        "--in-place",
        "--backup",
        &go_backup,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--values",
        r#"[["In place"]]"#,
        "--in-place",
        "--backup",
        &rust_backup,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "in-place exit");
    assert_eq!(rust_stderr, go_stderr, "in-place stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust in-place stdout"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go in-place stdout"), &go_in, "[IN]"),
        "in-place stdout"
    );
    assert!(Path::new(&go_backup).exists(), "go backup missing");
    assert!(Path::new(&rust_backup).exists(), "rust backup missing");

    let export_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--include-types",
    ];
    let export_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--include-types",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_rust);
    assert_eq!(rust_code, go_code, "in-place readback exit");
    assert_eq!(rust_stderr, go_stderr, "in-place readback stderr");
    assert_eq!(
        scrub_path(rust_export.expect("rust in-place export"), &rust_in, "[IN]"),
        scrub_path(go_export.expect("go in-place export"), &go_in, "[IN]"),
        "in-place saved readback"
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_rejects_formula_and_merged_cells_like_go() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-guards-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let formula = temp_dir.join("formula.xlsx");
    let merged = temp_dir.join("merged.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &formula,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1"/>
  <sheetData><row r="1"><c r="A1"><f>SUM(B1:B1)</f><v>1</v></c></row></sheetData>
</worksheet>"#,
    );
    write_simple_xlsx_with_sheet_xml(
        &merged,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B1"/>
  <sheetData><row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c></row></sheetData>
  <mergeCells count="1"><mergeCell ref="A1:B1"/></mergeCells>
</worksheet>"#,
    );
    let formula_s = formula.to_string_lossy().to_string();
    let merged_s = merged.to_string_lossy().to_string();
    for args in [
        vec![
            "--json",
            "xlsx",
            "ranges",
            "set",
            &formula_s,
            "--sheet",
            "Sheet1",
            "--anchor",
            "A1",
            "--values",
            r#"[["replace"]]"#,
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "ranges",
            "set",
            &merged_s,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:B1",
            "--values",
            r#"[["x","y"]]"#,
            "--dry-run",
        ],
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "guard exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "guard stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "guard stderr for {args:?}");
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_cells_extract_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "xlsx",
            "cells",
            "extract",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "xlsx",
            "cells",
            "extract",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--range",
            "B1:D2",
            "--include-empty",
            "--max-rows",
            "2",
        ],
        vec![
            "--json",
            "xlsx",
            "cells",
            "extract",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
            "--sheet",
            "Types",
            "--range",
            "E2:H2",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

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

#[test]
fn xlsx_names_list_show_matches_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-names-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("defined-names.xlsx");
    write_defined_names_xlsx(&workbook);
    let workbook = workbook.to_string_lossy().to_string();

    let cases: Vec<Vec<&str>> = vec![
        vec!["--json", "xlsx", "names", "list", &workbook],
        vec![
            "--json",
            "xlsx",
            "names",
            "list",
            &workbook,
            "--scope-sheet",
            "Data",
        ],
        vec![
            "--json",
            "xlsx",
            "names",
            "show",
            &workbook,
            "--name",
            "GlobalName",
        ],
        vec![
            "--json",
            "xlsx",
            "names",
            "show",
            &workbook,
            "--name",
            "H:xlsx/wb/name:n:GlobalName",
        ],
        vec![
            "--json",
            "xlsx",
            "names",
            "show",
            &workbook,
            "--name",
            "LocalData",
            "--scope-sheet",
            "Data",
        ],
        vec![
            "--json",
            "xlsx",
            "names",
            "show",
            &workbook,
            "--name",
            "sheet:2/name:LocalData",
        ],
        vec!["--json", "xlsx", "defined-names", "list", &workbook],
        vec!["--json", "xlsx", "names", "show", &workbook],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }

    let (code, stdout, stderr) = run_ooxml(&["--json", "xlsx", "names", "list", &workbook]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let result = stdout.expect("names list stdout");
    assert_eq!(result["file"], Value::String(workbook.clone()));
    assert_eq!(
        result["validateCommand"],
        Value::String(format!(
            "ooxml validate --strict {}",
            command_arg_for_test(&workbook)
        ))
    );
    let names = result["names"].as_array().expect("names array");
    assert_eq!(names.len(), 4);

    let global = &names[0];
    assert_eq!(global["name"], Value::String("GlobalName".to_string()));
    assert_eq!(global["scope"], Value::String("workbook".to_string()));
    assert_eq!(global["ref"], Value::String("Summary!$A$1".to_string()));
    assert_eq!(
        global["primarySelector"],
        Value::String("name:GlobalName".to_string())
    );
    assert_eq!(
        global["handle"],
        Value::String("H:xlsx/wb/name:n:GlobalName".to_string())
    );
    assert!(
        global["selectors"]
            .as_array()
            .expect("global selectors")
            .contains(&Value::String("scope:workbook/name:GlobalName".to_string()))
    );
    assert_rust_emitted_ooxml_command_succeeds(global, "showCommand");

    let local = names
        .iter()
        .find(|name| name["name"] == Value::String("LocalData".to_string()))
        .expect("local data defined name");
    assert_eq!(local["scope"], Value::String("sheet".to_string()));
    assert_eq!(local["localSheetId"], Value::from(1));
    assert_eq!(local["sheetNumber"], Value::from(2));
    assert_eq!(local["sheetName"], Value::String("Data".to_string()));
    assert_eq!(local["ref"], Value::String("Data!$A$1".to_string()));
    assert!(
        local.get("handle").is_none(),
        "sheet-local names are not handles"
    );
    assert!(
        local["selectors"]
            .as_array()
            .expect("local selectors")
            .contains(&Value::String("sheet:2/name:LocalData".to_string()))
    );
    assert_eq!(
        local["showCommand"],
        Value::String(format!(
            "ooxml --json xlsx names show {} --name LocalData --scope-sheet sheet:2",
            command_arg_for_test(&workbook)
        ))
    );
    assert_rust_emitted_ooxml_command_succeeds(local, "showCommand");

    let (filtered_code, filtered_stdout, filtered_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "names",
        "list",
        &workbook,
        "--scope-sheet",
        "Data",
    ]);
    assert_eq!(filtered_code, 0);
    assert_eq!(filtered_stderr, None);
    let filtered = filtered_stdout.expect("filtered names stdout");
    let filtered_names = filtered["names"].as_array().expect("filtered names array");
    assert_eq!(filtered_names.len(), 1);
    assert_eq!(
        filtered_names[0]["name"],
        Value::String("LocalData".to_string())
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_names_mutations_match_go_oracle_and_saved_readback() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-names-mutate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in = temp_dir.join("go-input.xlsx");
    let rust_in = temp_dir.join("rust-input.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in).expect("stage go input");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &rust_in).expect("stage rust input");
    let go_add = temp_dir.join("go-add.xlsx");
    let rust_add = temp_dir.join("rust-add.xlsx");
    let go_update = temp_dir.join("go-update.xlsx");
    let rust_update = temp_dir.join("rust-update.xlsx");
    let go_rename = temp_dir.join("go-rename.xlsx");
    let rust_rename = temp_dir.join("rust-rename.xlsx");
    let go_delete = temp_dir.join("go-delete.xlsx");
    let rust_delete = temp_dir.join("rust-delete.xlsx");
    let go_in = go_in.to_string_lossy().to_string();
    let rust_in = rust_in.to_string_lossy().to_string();
    let go_add = go_add.to_string_lossy().to_string();
    let rust_add = rust_add.to_string_lossy().to_string();
    let go_update = go_update.to_string_lossy().to_string();
    let rust_update = rust_update.to_string_lossy().to_string();
    let go_rename = go_rename.to_string_lossy().to_string();
    let rust_rename = rust_rename.to_string_lossy().to_string();
    let go_delete = go_delete.to_string_lossy().to_string();
    let rust_delete = rust_delete.to_string_lossy().to_string();
    let initial_ref = "'Sheet1'!$A$1:$B$2";
    let updated_ref = "SUM('Sheet1'!$B$1:$B$2)";

    let steps = [
        (
            "add",
            vec![
                "--json",
                "xlsx",
                "names",
                "add",
                &go_in,
                "--name",
                "SalesData",
                "--sheet",
                "Sheet1",
                "--range",
                "A1:B2",
                "--out",
                &go_add,
            ],
            vec![
                "--json",
                "xlsx",
                "names",
                "add",
                &rust_in,
                "--name",
                "SalesData",
                "--sheet",
                "Sheet1",
                "--range",
                "A1:B2",
                "--out",
                &rust_add,
            ],
            vec![(&go_in, "[IN]"), (&go_add, "[ADD]")],
            vec![(&rust_in, "[IN]"), (&rust_add, "[ADD]")],
        ),
        (
            "update",
            vec![
                "--json",
                "xlsx",
                "names",
                "update",
                &go_add,
                "--name",
                "SalesData",
                "--ref",
                updated_ref,
                "--expect-ref",
                initial_ref,
                "--out",
                &go_update,
            ],
            vec![
                "--json",
                "xlsx",
                "names",
                "update",
                &rust_add,
                "--name",
                "SalesData",
                "--ref",
                updated_ref,
                "--expect-ref",
                initial_ref,
                "--out",
                &rust_update,
            ],
            vec![(&go_add, "[ADD]"), (&go_update, "[UPDATE]")],
            vec![(&rust_add, "[ADD]"), (&rust_update, "[UPDATE]")],
        ),
        (
            "rename",
            vec![
                "--json",
                "xlsx",
                "names",
                "rename",
                &go_update,
                "--name",
                "SalesData",
                "--new-name",
                "RevenueData",
                "--expect-ref",
                updated_ref,
                "--out",
                &go_rename,
            ],
            vec![
                "--json",
                "xlsx",
                "names",
                "rename",
                &rust_update,
                "--name",
                "SalesData",
                "--new-name",
                "RevenueData",
                "--expect-ref",
                updated_ref,
                "--out",
                &rust_rename,
            ],
            vec![(&go_update, "[UPDATE]"), (&go_rename, "[RENAME]")],
            vec![(&rust_update, "[UPDATE]"), (&rust_rename, "[RENAME]")],
        ),
        (
            "delete",
            vec![
                "--json",
                "xlsx",
                "names",
                "delete",
                &go_rename,
                "--name",
                "RevenueData",
                "--expect-ref",
                updated_ref,
                "--out",
                &go_delete,
            ],
            vec![
                "--json",
                "xlsx",
                "names",
                "delete",
                &rust_rename,
                "--name",
                "RevenueData",
                "--expect-ref",
                updated_ref,
                "--out",
                &rust_delete,
            ],
            vec![(&go_rename, "[RENAME]"), (&go_delete, "[DELETE]")],
            vec![(&rust_rename, "[RENAME]"), (&rust_delete, "[DELETE]")],
        ),
    ];

    for (label, go_args, rust_args, go_paths, rust_paths) in steps {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, go_code, "{label} exit");
        assert_eq!(rust_stderr, go_stderr, "{label} stderr");
        let go_path_refs = go_paths
            .iter()
            .map(|(from, to)| (from.as_str(), *to))
            .collect::<Vec<_>>();
        let rust_path_refs = rust_paths
            .iter()
            .map(|(from, to)| (from.as_str(), *to))
            .collect::<Vec<_>>();
        let rust_raw = rust_stdout.expect("rust names mutation stdout");
        let go_json = scrub_paths(go_stdout.expect("go names mutation stdout"), &go_path_refs);
        let rust_json = scrub_paths(rust_raw.clone(), &rust_path_refs);
        assert_eq!(rust_json, go_json, "{label} stdout");
        assert_rust_emitted_ooxml_command_exits_zero(&rust_raw, "validateCommand");
        assert_rust_emitted_ooxml_command_succeeds(&rust_raw, "namesListCommand");
        if label != "delete" {
            assert_rust_emitted_ooxml_command_succeeds(&rust_raw, "nameShowCommand");
        }
    }

    let (go_code, go_stdout, go_stderr) =
        run_go_ooxml(&["--json", "xlsx", "names", "list", &go_delete]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "xlsx", "names", "list", &rust_delete]);
    assert_eq!(rust_code, go_code, "post-delete list exit");
    assert_eq!(rust_stderr, go_stderr, "post-delete list stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust post-delete list"),
            &rust_delete,
            "[DELETE]"
        ),
        scrub_path(
            go_stdout.expect("go post-delete list"),
            &go_delete,
            "[DELETE]"
        ),
        "post-delete list stdout"
    );
    assert!(
        !read_zip_string(Path::new(&rust_delete), "xl/workbook.xml").contains("definedNames"),
        "empty definedNames element should be removed"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_names_dry_run_and_errors_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-names-errors-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-input.xlsx");
    let rust_in_path = temp_dir.join("rust-input.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in_path).expect("go input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();

    let dry_go = [
        "--json",
        "xlsx",
        "names",
        "add",
        &go_in,
        "--name",
        "LocalInput",
        "--ref",
        "Sheet1!$A$1",
        "--scope-sheet",
        "Sheet1",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "names",
        "add",
        &rust_in,
        "--name",
        "LocalInput",
        "--ref",
        "Sheet1!$A$1",
        "--scope-sheet",
        "Sheet1",
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
        !read_zip_string(&rust_in_path, "xl/workbook.xml").contains("definedNames"),
        "dry-run should not write source workbook"
    );

    let bad_cases: Vec<Vec<&str>> = vec![
        vec!["--json", "xlsx", "names", "show", &go_in],
        vec![
            "--json",
            "xlsx",
            "names",
            "add",
            &go_in,
            "--ref",
            "Sheet1!$A$1",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "names",
            "add",
            &go_in,
            "--name",
            "A1",
            "--ref",
            "Sheet1!$A$1",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "names",
            "add",
            &go_in,
            "--name",
            "Bad Name",
            "--ref",
            "Sheet1!$A$1",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "names",
            "add",
            &go_in,
            "--name",
            "Input",
            "--ref",
            "Sheet1!$A$1",
            "--range",
            "A1",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "names",
            "add",
            &go_in,
            "--name",
            "Input",
            "--range",
            "A1",
            "--dry-run",
        ],
    ];
    for go_args in bad_cases {
        let mut rust_args = go_args.clone();
        for arg in &mut rust_args {
            if *arg == go_in {
                *arg = &rust_in;
            }
        }
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, go_code, "bad args exit for {go_args:?}");
        assert_eq!(rust_stdout, go_stdout, "bad args stdout for {go_args:?}");
        assert_eq!(
            scrub_path(rust_stderr.expect("rust bad args stderr"), &rust_in, "[IN]"),
            scrub_path(go_stderr.expect("go bad args stderr"), &go_in, "[IN]"),
            "bad args stderr for {go_args:?}"
        );
    }

    let go_add = temp_dir.join("go-add.xlsx").to_string_lossy().to_string();
    let rust_add = temp_dir.join("rust-add.xlsx").to_string_lossy().to_string();
    assert_eq!(
        run_go_ooxml(&[
            "--json",
            "xlsx",
            "names",
            "add",
            &go_in,
            "--name",
            "Input",
            "--ref",
            "Sheet1!$A$1",
            "--out",
            &go_add,
        ])
        .0,
        0,
        "go stale setup"
    );
    assert_eq!(
        run_ooxml(&[
            "--json",
            "xlsx",
            "names",
            "add",
            &rust_in,
            "--name",
            "Input",
            "--ref",
            "Sheet1!$A$1",
            "--out",
            &rust_add,
        ])
        .0,
        0,
        "rust stale setup"
    );
    let go_stale = [
        "--json",
        "xlsx",
        "names",
        "update",
        &go_add,
        "--name",
        "Input",
        "--ref",
        "Sheet1!$A$2",
        "--expect-ref",
        "Sheet1!$A$99",
        "--dry-run",
    ];
    let rust_stale = [
        "--json",
        "xlsx",
        "names",
        "update",
        &rust_add,
        "--name",
        "Input",
        "--ref",
        "Sheet1!$A$2",
        "--expect-ref",
        "Sheet1!$A$99",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_stale);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_stale);
    assert_eq!(rust_code, go_code, "stale guard exit");
    assert_eq!(rust_stdout, go_stdout, "stale guard stdout");
    assert_eq!(
        scrub_path(rust_stderr.expect("rust stale stderr"), &rust_add, "[ADD]"),
        scrub_path(go_stderr.expect("go stale stderr"), &go_add, "[ADD]"),
        "stale guard stderr"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

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
