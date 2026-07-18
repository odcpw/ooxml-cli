#[test]
fn xlsx_pivots_list_show_match_rust_baseline_and_generated_commands() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-pivots-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_path = temp_dir.join("baseline-pivots.xlsx");
    let rust_path = temp_dir.join("rust-pivots.xlsx");
    write_pivot_xlsx(&baseline_path, false);
    write_pivot_xlsx(&rust_path, false);
    let baseline_file = baseline_path.to_string_lossy().to_string();
    let rust_file = rust_path.to_string_lossy().to_string();
    let replacements = [(&baseline_file[..], "[XLSX]"), (&rust_file[..], "[XLSX]")];

    let list = assert_rust_baseline_match_scrubbed(
        "pivots list",
        &["--json", "xlsx", "pivots", "list", &baseline_file],
        &["--json", "xlsx", "pivots", "list", &rust_file],
        &replacements,
    )
    .expect("rust pivots list");
    let pivot = &list["pivots"][0];
    assert_eq!(pivot["name"], "SalesPivot");
    assert_eq!(pivot["primarySelector"], "pivot:1");
    assert_eq!(pivot["cache"]["source"]["range"], "A1:D3");
    assert_eq!(pivot["rowFields"][0]["name"], "Region");
    assert_eq!(pivot["columnFields"][0]["name"], "Quarter");
    assert_eq!(pivot["dataFields"][0]["caption"], "Sum of Amount");
    assert_rust_emitted_ooxml_command_succeeds(&list, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(pivot, "showCommand");
    assert_rust_emitted_ooxml_command_succeeds(pivot, "sourceExportCommand");

    for selector in [
        "pivot:1",
        "#1",
        "SalesPivot",
        "name:SalesPivot",
        "~SalesPivot",
        "cacheId:1",
        "rId:rIdPivot1",
        "rid:rIdPivot1",
        "part:/xl/pivotTables/pivotTable1.xml",
    ] {
        assert_rust_baseline_match_scrubbed(
            &format!("pivots show selector {selector}"),
            &[
                "--json", "xlsx", "pivots", "show", &baseline_file, "--sheet", "Data", "--pivot",
                selector,
            ],
            &[
                "--json", "xlsx", "pivots", "show", &rust_file, "--sheet", "Data", "--pivot",
                selector,
            ],
            &replacements,
        );
    }

    let two_go = temp_dir.join("two-baseline.xlsx");
    let two_rust = temp_dir.join("two-rust.xlsx");
    write_pivot_xlsx(&two_go, true);
    write_pivot_xlsx(&two_rust, true);
    let two_go = two_go.to_string_lossy().to_string();
    let two_rust = two_rust.to_string_lossy().to_string();
    assert_rust_baseline_match_scrubbed(
        "pivots show requires selector",
        &["--json", "xlsx", "pivots", "show", &two_go],
        &["--json", "xlsx", "pivots", "show", &two_rust],
        &[(&two_go, "[XLSX]"), (&two_rust, "[XLSX]")],
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_pivots_create_matches_rust_baseline_saved_readback_dry_run_and_errors() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-pivots-create-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let baseline_out_path = temp_dir.join("baseline-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    write_table_xlsx(&baseline_in_path);
    write_table_xlsx(&rust_in_path);
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let baseline_out = baseline_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();
    let replacements = [
        (&baseline_in[..], "[IN]"),
        (&rust_in[..], "[IN]"),
        (&baseline_out[..], "[OUT]"),
        (&rust_out[..], "[OUT]"),
    ];

    let create = assert_rust_baseline_match_scrubbed(
        "pivots create",
        &[
            "--json",
            "xlsx",
            "pivots",
            "create",
            &baseline_in,
            "--table",
            "Sales",
            "--name",
            "SalesPivot",
            "--rows",
            "Region",
            "--values",
            "Amount:sum",
            "--anchor",
            "D1",
            "--out",
            &baseline_out,
        ],
        &[
            "--json",
            "xlsx",
            "pivots",
            "create",
            &rust_in,
            "--table",
            "Sales",
            "--name",
            "SalesPivot",
            "--rows",
            "Region",
            "--values",
            "Amount:sum",
            "--anchor",
            "D1",
            "--out",
            &rust_out,
        ],
        &replacements,
    )
    .expect("rust pivots create");
    assert_eq!(create["name"], "SalesPivot");
    assert_eq!(create["sourceRange"], "A1:B3");
    assert_eq!(create["location"], "D1:E5");
    assert_rust_emitted_ooxml_command_succeeds(&create, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&create, "pivotsListCommand");
    assert_xlsx_strict_valid(&rust_out);
    for part in [
        "xl/pivotTables/pivotTable1.xml",
        "xl/pivotTables/_rels/pivotTable1.xml.rels",
        "xl/pivotCache/pivotCacheDefinition1.xml",
        "xl/pivotCache/_rels/pivotCacheDefinition1.xml.rels",
        "xl/pivotCache/pivotCacheRecords1.xml",
    ] {
        assert!(zip_entry_exists(&rust_out_path, part), "missing {part}");
    }

    assert_rust_baseline_match_scrubbed(
        "pivots create saved list",
        &["--json", "xlsx", "pivots", "list", &baseline_out],
        &["--json", "xlsx", "pivots", "list", &rust_out],
        &[(&baseline_out, "[OUT]"), (&rust_out, "[OUT]")],
    );
    assert_rust_baseline_match_scrubbed(
        "pivots create saved show",
        &[
            "--json",
            "xlsx",
            "pivots",
            "show",
            &baseline_out,
            "--sheet",
            "Data",
            "--pivot",
            "SalesPivot",
        ],
        &[
            "--json",
            "xlsx",
            "pivots",
            "show",
            &rust_out,
            "--sheet",
            "Data",
            "--pivot",
            "SalesPivot",
        ],
        &[(&baseline_out, "[OUT]"), (&rust_out, "[OUT]")],
    );

    assert_rust_baseline_match_scrubbed(
        "pivots create dry-run",
        &[
            "--json",
            "xlsx",
            "pivots",
            "create",
            &baseline_in,
            "--table",
            "Sales",
            "--rows",
            "Region",
            "--values",
            "Amount",
            "--dry-run",
        ],
        &[
            "--json",
            "xlsx",
            "pivots",
            "create",
            &rust_in,
            "--table",
            "Sales",
            "--rows",
            "Region",
            "--values",
            "Amount",
            "--dry-run",
        ],
        &[(&baseline_in, "[IN]"), (&rust_in, "[IN]")],
    );
    assert!(
        !zip_entry_exists(&rust_in_path, "xl/pivotTables/pivotTable1.xml"),
        "dry-run wrote pivot table into input workbook"
    );

    for (label, extra_args) in [
        (
            "missing rows/cols",
            vec!["--table", "Sales", "--values", "Amount", "--dry-run"],
        ),
        (
            "unknown row field",
            vec![
                "--table",
                "Sales",
                "--rows",
                "Missing",
                "--values",
                "Amount",
                "--dry-run",
            ],
        ),
        (
            "source range mismatch",
            vec![
                "--table",
                "Sales",
                "--rows",
                "Region",
                "--values",
                "Amount",
                "--expect-source-range",
                "A1:B9",
                "--dry-run",
            ],
        ),
    ] {
        let mut baseline_args = vec!["--json", "xlsx", "pivots", "create", &baseline_in];
        baseline_args.extend(extra_args.iter().copied());
        let mut rust_args = vec!["--json", "xlsx", "pivots", "create", &rust_in];
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
fn xlsx_pivots_create_adds_prefixed_worksheet_pivot_parts() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-pivots-prefixed-worksheet-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let base_path = temp_dir.join("base.xlsx");
    let input_path = temp_dir.join("prefixed.xlsx");
    let output_path = temp_dir.join("pivoted.xlsx");
    write_table_xlsx(&base_path);
    let base = base_path.to_string_lossy().to_string();
    rewrite_zip_fixture(&base, &input_path, |name, data| {
        if name == "xl/workbook.xml" {
            let mut xml = String::from_utf8(data).expect("workbook XML utf8");
            xml = xml.replace(
                r#" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#,
                "",
            );
            xml = xml.replacen("<workbook xmlns=", "<x:workbook xmlns:x=", 1);
            xml = xml.replace("</workbook>", "</x:workbook>");
            for tag in ["sheets", "sheet"] {
                xml = xml.replace(&format!("<{tag}"), &format!("<x:{tag}"));
                xml = xml.replace(&format!("</{tag}>"), &format!("</x:{tag}>"));
            }
            xml = xml.replacen(
                "<x:sheet name=",
                r#"<x:sheet xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" name="#,
                1,
            );
            return Some((name.to_string(), xml.into_bytes()));
        }
        if name != "xl/worksheets/sheet1.xml" {
            return Some((name.to_string(), data));
        }
        let mut xml = String::from_utf8(data).expect("worksheet XML utf8");
        xml = xml.replacen("<worksheet xmlns=", "<x:worksheet xmlns:x=", 1);
        xml = xml.replace("</worksheet>", "</x:worksheet>");
        for tag in [
            "dimension",
            "sheetData",
            "row",
            "c",
            "is",
            "t",
            "v",
            "tableParts",
            "tablePart",
        ] {
            xml = xml.replace(&format!("<{tag}"), &format!("<x:{tag}"));
            xml = xml.replace(&format!("</{tag}>"), &format!("</x:{tag}>"));
        }
        Some((name.to_string(), xml.into_bytes()))
    });
    let input = input_path.to_string_lossy().to_string();
    let output = output_path.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "pivots",
        "create",
        &input,
        "--table",
        "Sales",
        "--name",
        "SalesPivot",
        "--rows",
        "Region",
        "--values",
        "Amount:sum",
        "--anchor",
        "D1",
        "--out",
        &output,
    ]);
    assert_eq!(code, 0, "pivots create exit: {stderr:?} {stdout:?}");
    let sheet_xml = read_zip_string(&output_path, "xl/worksheets/sheet1.xml");
    let workbook_xml = read_zip_string(&output_path, "xl/workbook.xml");
    assert!(sheet_xml.contains("xmlns:r="), "relationships namespace missing");
    assert!(
        sheet_xml.contains(
            r#"<x:pivotTableParts count="1"><x:pivotTablePart r:id="rId2"/></x:pivotTableParts>"#
        ),
        "prefixed pivotTableParts missing: {sheet_xml}"
    );
    assert!(
        workbook_xml.contains(
            r#"<x:pivotCaches><x:pivotCache cacheId="1" r:id="rId2"/></x:pivotCaches>"#
        ),
        "prefixed pivotCaches missing: {workbook_xml}"
    );
    let workbook_start = workbook_xml
        .split_once('>')
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once('>').map(|(start, _)| start))
        .unwrap_or_default();
    assert!(
        workbook_start.contains("xmlns:r="),
        "workbook root relationships namespace missing: {workbook_xml}"
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_pivots_create_appends_to_existing_worksheet_pivot_parts() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-pivots-two-same-sheet-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input_path = temp_dir.join("input.xlsx");
    let first_path = temp_dir.join("first.xlsx");
    let second_path = temp_dir.join("second.xlsx");
    write_table_xlsx(&input_path);
    let input = input_path.to_string_lossy().to_string();
    let first = first_path.to_string_lossy().to_string();
    let second = second_path.to_string_lossy().to_string();

    for (source, output, name, anchor) in [
        (&input, &first, "SalesPivotOne", "D1"),
        (&first, &second, "SalesPivotTwo", "G1"),
    ] {
        let (code, stdout, stderr) = run_ooxml(&[
            "--json",
            "xlsx",
            "pivots",
            "create",
            source,
            "--table",
            "Sales",
            "--name",
            name,
            "--rows",
            "Region",
            "--values",
            "Amount:sum",
            "--anchor",
            anchor,
            "--out",
            output,
        ]);
        assert_eq!(code, 0, "pivots create {name} exit: {stderr:?} {stdout:?}");
    }

    let sheet_xml = read_zip_string(&second_path, "xl/worksheets/sheet1.xml");
    assert_eq!(
        sheet_xml.matches("<pivotTableParts").count(),
        1,
        "worksheet must contain one pivotTableParts container: {sheet_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<pivotTableParts count="2">"#),
        "pivotTableParts count was not updated: {sheet_xml}"
    );
    assert_eq!(
        sheet_xml.matches("<pivotTablePart ").count(),
        2,
        "both pivotTablePart children must be retained: {sheet_xml}"
    );
    let (code, list, stderr) = run_ooxml(&["--json", "xlsx", "pivots", "list", &second]);
    assert_eq!(code, 0, "pivots list exit: {stderr:?} {list:?}");
    let pivots = list
        .expect("pivots list output")["pivots"]
        .as_array()
        .expect("pivots array")
        .to_vec();
    assert_eq!(pivots.len(), 2, "both pivots must be discoverable");
    assert_xlsx_strict_valid(&second);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_pivots_create_keeps_defined_names_calc_pr_before_pivot_caches() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-pivots-workbook-order-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let base_path = temp_dir.join("table.xlsx");
    let input_path = temp_dir.join("with-defined-names-calc-pr.xlsx");
    let output_path = temp_dir.join("pivoted.xlsx");
    write_table_xlsx(&base_path);
    let base = base_path.to_string_lossy().to_string();
    rewrite_zip_fixture(&base, &input_path, |name, data| {
        let data = if name == "xl/workbook.xml" {
            let xml = String::from_utf8(data).expect("workbook XML utf8");
            xml.replace(
                "  </sheets>\n</workbook>",
                r#"  </sheets>
  <definedNames>
    <definedName name="SalesData">Data!$A$1:$B$3</definedName>
  </definedNames>
  <calcPr calcId="191029"/>
</workbook>"#,
            )
            .into_bytes()
        } else {
            data
        };
        Some((name.to_string(), data))
    });

    let input = input_path.to_string_lossy().to_string();
    let output = output_path.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "pivots",
        "create",
        &input,
        "--table",
        "Sales",
        "--name",
        "SalesPivot",
        "--rows",
        "Region",
        "--values",
        "Amount:sum",
        "--anchor",
        "D1",
        "--out",
        &output,
    ]);
    assert_eq!(code, 0, "pivots create exit");
    assert_eq!(stderr, None, "pivots create stderr");
    assert!(stdout.is_some(), "pivots create stdout");

    let workbook_xml = read_zip_string(&output_path, "xl/workbook.xml");
    assert_xml_tag_order(
        &workbook_xml,
        &[
            "<sheets",
            "</sheets>",
            "<definedNames",
            "</definedNames>",
            "<calcPr",
            "<pivotCaches",
        ],
    );
    assert_xlsx_strict_valid(&output);
    assert_conformance_check_passed("pivots workbook child order conformance", &output);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_pivots_validate_and_conformance_reject_pivot_caches_before_names_or_calc_pr() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-pivots-bad-workbook-order-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    for (label, workbook_xml) in [
        (
            "before-defined-names",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Data" sheetId="1" r:id="rId1"/>
  </sheets>
  <pivotCaches>
    <pivotCache cacheId="1" r:id="rIdCache1"/>
  </pivotCaches>
  <definedNames>
    <definedName name="SalesData">Data!$A$1:$D$3</definedName>
  </definedNames>
  <calcPr calcId="191029"/>
</workbook>"#,
        ),
        (
            "before-calc-pr",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Data" sheetId="1" r:id="rId1"/>
  </sheets>
  <definedNames>
    <definedName name="SalesData">Data!$A$1:$D$3</definedName>
  </definedNames>
  <pivotCaches>
    <pivotCache cacheId="1" r:id="rIdCache1"/>
  </pivotCaches>
  <calcPr calcId="191029"/>
</workbook>"#,
        ),
    ] {
        let base_path = temp_dir.join(format!("{label}-base.xlsx"));
        let bad_path = temp_dir.join(format!("{label}.xlsx"));
        write_pivot_xlsx(&base_path, false);
        let base = base_path.to_string_lossy().to_string();
        rewrite_zip_fixture(&base, &bad_path, |name, data| {
            let data = if name == "xl/workbook.xml" {
                workbook_xml.as_bytes().to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        });

        let bad = bad_path.to_string_lossy().to_string();
        assert_pivot_workbook_child_order_rejected(label, &bad);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

fn assert_pivot_workbook_child_order_rejected(label: &str, file: &str) {
    for (command, args) in [
        (
            "strict validate",
            vec!["--json", "validate", "--strict", file],
        ),
        (
            "conformance check",
            vec!["--json", "conformance", "check", file],
        ),
    ] {
        let (code, report, stderr) = run_ooxml(&args);
        assert_ne!(code, 0, "{label} {command} should reject bad order");
        assert_eq!(stderr, None, "{label} {command} stderr");
        let report = report.unwrap_or_else(|| panic!("{label} {command} should emit JSON"));
        assert!(
            json_contains_diagnostic_code(&report, "XLSX_WORKBOOK_CHILD_ORDER"),
            "{label} {command} did not report workbook child order:\n{report:#}"
        );
    }
}

fn write_pivot_xlsx(dest: &Path, two_pivots: bool) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create pivot xlsx");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut pivot_sheet_refs = r#"<pivotTableDefinition r:id="rIdPivot1"/>"#.to_string();
    let mut pivot_sheet_rels = r#"<Relationship Id="rIdPivot1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable1.xml"/>"#.to_string();
    let mut pivot_overrides = r#"<Override PartName="/xl/pivotTables/pivotTable1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/>"#.to_string();
    if two_pivots {
        pivot_sheet_refs.push_str(
            r#"
  <pivotTableDefinition r:id="rIdPivot2"/>"#,
        );
        pivot_sheet_rels.push_str(
            r#"
  <Relationship Id="rIdPivot2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable2.xml"/>"#,
        );
        pivot_overrides.push_str(
            r#"
  <Override PartName="/xl/pivotTables/pivotTable2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/>"#,
        );
    }

    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  {pivot_overrides}
  <Override PartName="/xl/pivotCache/pivotCacheDefinition1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"/>
  <Override PartName="/xl/pivotCache/pivotCacheRecords1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml"/>
</Types>"#
        ),
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
  <sheets>
    <sheet name="Data" sheetId="1" r:id="rId1"/>
  </sheets>
  <pivotCaches>
    <pivotCache cacheId="1" r:id="rIdCache1"/>
  </pivotCaches>
</workbook>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rIdCache1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition" Target="pivotCache/pivotCacheDefinition1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:E6"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Quarter</t></is></c><c r="C1" t="inlineStr"><is><t>Amount</t></is></c><c r="D1" t="inlineStr"><is><t>Segment</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>East</t></is></c><c r="B2" t="inlineStr"><is><t>Q1</t></is></c><c r="C2"><v>10</v></c><c r="D2" t="inlineStr"><is><t>Enterprise</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>West</t></is></c><c r="B3" t="inlineStr"><is><t>Q2</t></is></c><c r="C3"><v>20</v></c><c r="D3" t="inlineStr"><is><t>SMB</t></is></c></row>
  </sheetData>
  {pivot_sheet_refs}
</worksheet>"#
        ),
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/_rels/sheet1.xml.rels",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  {pivot_sheet_rels}
</Relationships>"#
        ),
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotTables/pivotTable1.xml",
        &test_pivot_table_xml("SalesPivot", "D3:E6"),
    );
    if two_pivots {
        write_zip_string(
            &mut writer,
            options,
            "xl/pivotTables/pivotTable2.xml",
            &test_pivot_table_xml("SalesPivot2", "G3:H6"),
        );
    }
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotCache/pivotCacheDefinition1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" recordCount="2" createdVersion="6" refreshedVersion="6" refreshOnLoad="1" saveData="1">
  <cacheSource type="worksheet">
    <worksheetSource ref="A1:D3" sheet="Data"/>
  </cacheSource>
  <cacheFields count="4">
    <cacheField name="Region"><sharedItems count="2"/></cacheField>
    <cacheField name="Quarter"><sharedItems count="2"/></cacheField>
    <cacheField name="Amount" numFmtId="0"><sharedItems containsNumber="1" count="2"/></cacheField>
    <cacheField name="Segment"><sharedItems count="2"/></cacheField>
  </cacheFields>
</pivotCacheDefinition>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotCache/_rels/pivotCacheDefinition1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdRecords1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords" Target="pivotCacheRecords1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotCache/pivotCacheRecords1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotCacheRecords xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2">
  <r><s v="East"/><s v="Q1"/><n v="10"/><s v="Enterprise"/></r>
  <r><s v="West"/><s v="Q2"/><n v="20"/><s v="SMB"/></r>
</pivotCacheRecords>"#,
    );
    writer.finish().expect("finish pivot xlsx");
}

fn test_pivot_table_xml(name: &str, location: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="{name}" cacheId="1" dataCaption="Values" updatedVersion="6" minRefreshableVersion="3">
  <location ref="{location}" firstHeaderRow="1" firstDataRow="2" firstDataCol="1"/>
  <pivotFields count="4">
    <pivotField axis="axisRow" showAll="0"/>
    <pivotField axis="axisCol" showAll="0"/>
    <pivotField dataField="1"/>
    <pivotField axis="axisPage" showAll="0"/>
  </pivotFields>
  <rowFields count="1"><field x="0"/></rowFields>
  <colFields count="1"><field x="1"/></colFields>
  <pageFields count="1"><pageField fld="3" hier="-1"/></pageFields>
  <dataFields count="1"><dataField name="Sum of Amount" fld="2" subtotal="sum"/></dataFields>
</pivotTableDefinition>"#
    )
}
