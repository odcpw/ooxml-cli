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
fn xlsx_charts_update_source_preserves_series_style() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-chart-update-style-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let workbook = "testdata/xlsx/chart-workbook/workbook.xlsx";
    let go_style_in_path = temp_dir.join("go-style-in.xlsx");
    let rust_style_in_path = temp_dir.join("rust-style-in.xlsx");
    let go_styled_path = temp_dir.join("go-styled.xlsx");
    let rust_styled_path = temp_dir.join("rust-styled.xlsx");
    let go_update_out_path = temp_dir.join("go-updated.xlsx");
    let rust_update_out_path = temp_dir.join("rust-updated.xlsx");
    fs::copy(workbook, &go_style_in_path).expect("go style input");
    fs::copy(workbook, &rust_style_in_path).expect("rust style input");

    let go_style_in = go_style_in_path.to_string_lossy().to_string();
    let rust_style_in = rust_style_in_path.to_string_lossy().to_string();
    let go_styled = go_styled_path.to_string_lossy().to_string();
    let rust_styled = rust_styled_path.to_string_lossy().to_string();
    let go_update_out = go_update_out_path.to_string_lossy().to_string();
    let rust_update_out = rust_update_out_path.to_string_lossy().to_string();

    let style_go_args = [
        "--json",
        "xlsx",
        "charts",
        "set-series-style",
        &go_style_in,
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
        "--out",
        &go_styled,
    ];
    let style_rust_args = [
        "--json",
        "xlsx",
        "charts",
        "set-series-style",
        &rust_style_in,
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
        "--out",
        &rust_styled,
    ];
    let style_replacements = [
        (go_style_in.as_str(), "[IN]"),
        (rust_style_in.as_str(), "[IN]"),
        (go_styled.as_str(), "[OUT]"),
        (rust_styled.as_str(), "[OUT]"),
    ];
    let rust_style = assert_xlsx_structure_command_matches(
        "xlsx charts set-series-style before update-source",
        &style_go_args,
        &style_rust_args,
        &style_replacements,
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_style, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_style, "chartShowCommand");

    let update_go_args = [
        "--json",
        "xlsx",
        "charts",
        "update-source",
        &go_styled,
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
        &rust_styled,
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
        (go_styled.as_str(), "[IN]"),
        (rust_styled.as_str(), "[IN]"),
        (go_update_out.as_str(), "[OUT]"),
        (rust_update_out.as_str(), "[OUT]"),
    ];
    let rust_update = assert_xlsx_structure_command_matches(
        "xlsx charts update-source preserves series style",
        &update_go_args,
        &update_rust_args,
        &update_replacements,
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_update, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "chartShowCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "rangesExportCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_update, "sourceRangeExportCommand");
    assert_xlsx_chart_style_valid_strict(&rust_update_out);

    let chart_xml = read_zip_string(&rust_update_out_path, "xl/charts/chart1.xml");
    for needle in [
        "Data!$B$2:$B$3",
        r#"<c:ptCount val="2"/>"#,
        r#"<a:srgbClr val="FF8800"/>"#,
        r#"<a:srgbClr val="114477"/>"#,
        r#"<a:ln w="25400">"#,
    ] {
        assert!(
            chart_xml.contains(needle),
            "updated chart XML missing {needle:?}:\n{chart_xml}"
        );
    }

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
fn template_apply_xlsx_chart_target_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-template-xlsx-chart-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("template xlsx chart temp dir");

    let tokens_path = temp_dir.join("chart-tokens.json");
    fs::write(&tokens_path, template_apply_xlsx_chart_tokens_json())
        .expect("write xlsx chart template tokens");
    let tokens_str = tokens_path.to_string_lossy().to_string();
    let target = "testdata/xlsx/chart-workbook/workbook.xlsx";
    let go_out = temp_dir.join("go-chart.xlsx");
    let rust_out = temp_dir.join("rust-chart.xlsx");
    let go_out_str = go_out.to_string_lossy().to_string();
    let rust_out_str = rust_out.to_string_lossy().to_string();
    let go_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--target-charts",
        "--out",
        &go_out_str,
    ];
    let rust_args = [
        "--json",
        "template",
        "apply",
        target,
        "--tokens",
        &tokens_str,
        "--target-charts",
        "--out",
        &rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "template apply XLSX charts exit");
    assert_eq!(rust_stderr, go_stderr, "template apply XLSX charts stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust XLSX chart apply stdout"),
            &[(&tokens_str, "[TOKENS]"), (&rust_out_str, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go XLSX chart apply stdout"),
            &[(&tokens_str, "[TOKENS]"), (&go_out_str, "[OUT]")]
        ),
        "template apply XLSX charts stdout"
    );
    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", &rust_out_str]);
    assert_eq!(
        validate_code, 0,
        "template apply XLSX chart strict validate exit"
    );
    assert_eq!(
        validate_stderr, None,
        "template apply XLSX chart strict validate stderr"
    );
    assert_eq!(
        validate_stdout.expect("template apply XLSX chart strict validate")["valid"],
        Value::Bool(true)
    );
    let (_, conformance_stdout, conformance_stderr) =
        run_ooxml(&["--json", "conformance", "check", &rust_out_str]);
    assert_eq!(
        conformance_stderr, None,
        "template apply XLSX chart conformance stderr"
    );
    assert!(
        conformance_stdout.is_some(),
        "template apply XLSX chart conformance stdout"
    );
    let (_, chart_show, chart_show_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "charts",
        "show",
        &rust_out_str,
        "--chart",
        "chart:1",
    ]);
    assert_eq!(chart_show_stderr, None, "XLSX chart readback stderr");
    let chart_show = chart_show.expect("XLSX chart readback");
    assert_eq!(
        chart_show["charts"][0]["style"]["series"][0]["fillColor"],
        serde_json::json!("FF0000")
    );
    assert_eq!(
        chart_show["charts"][0]["style"]["series"][0]["lineColor"],
        serde_json::json!("00FF00")
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

fn template_apply_xlsx_chart_tokens_json() -> &'static str {
    r#"{
  "schemaVersion": "1.0",
  "type": "xlsx",
  "source": "xlsx-chart-target",
  "pptx": {
    "theme": null,
    "defaultTextStyles": [],
    "tableStyles": [],
    "chartStyles": [
      {
        "partUri": "/template/chart.xml",
        "seriesFillColor": "FF0000",
        "seriesLineColor": "00FF00"
      }
    ]
  },
  "xlsx": {
    "theme": null,
    "namedCellStyles": [],
    "chartStyles": [
      {
        "partUri": "/template/chart.xml",
        "seriesFillColor": "FF0000",
        "seriesLineColor": "00FF00"
      }
    ]
  }
}
"#
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
