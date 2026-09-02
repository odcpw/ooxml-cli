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
                "--json",
                "xlsx",
                "pivots",
                "show",
                &baseline_file,
                "--sheet",
                "Data",
                "--pivot",
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

    let (code, list, stderr) = run_ooxml(&["--json", "xlsx", "pivots", "list", &two_rust]);
    assert_eq!(code, 0, "relationship-only multi-sheet list: {stderr:?}");
    let pivots = list.expect("relationship-only multi-sheet list output")["pivots"]
        .as_array()
        .expect("pivots array")
        .to_vec();
    assert_eq!(pivots.len(), 3, "expected all relationship-linked pivots");
    assert_eq!(
        pivots
            .iter()
            .filter(|pivot| pivot["sheet"] == "Data")
            .count(),
        2,
        "expected two pivots on Data"
    );
    assert_eq!(
        pivots
            .iter()
            .filter(|pivot| pivot["sheet"] == "Archive")
            .count(),
        1,
        "expected one pivot on Archive"
    );
    assert_xlsx_strict_valid(&two_rust);
    for sheet_part in ["xl/worksheets/sheet1.xml", "xl/worksheets/sheet2.xml"] {
        let sheet_xml = read_zip_string(Path::new(&two_rust), sheet_part);
        assert!(
            !sheet_xml.contains("pivotTablePart"),
            "pivot discovery must not depend on worksheet children: {sheet_xml}"
        );
    }

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
    assert_rust_emitted_ooxml_command_succeeds(&create, "pivotsShowCommand");
    assert!(
        create["pivotsShowCommand"]
            .as_str()
            .expect("pivots show command")
            .contains("--pivot part:/xl/pivotTables/pivotTable1.xml"),
        "created pivot proof should use its durable part selector: {create}"
    );
    assert_rust_emitted_ooxml_command_succeeds(&create, "sourceExportCommand");
    assert_rust_emitted_ooxml_command_succeeds(&create, "conformanceCommand");
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
fn xlsx_pivots_create_rejects_failed_internal_strict_validation() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-pivots-validation-gate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let valid_path = temp_dir.join("valid.xlsx");
    let broken_path = temp_dir.join("broken.xlsx");
    let output_path = temp_dir.join("output.xlsx");
    write_table_xlsx(&valid_path);
    rewrite_zip_fixture(
        valid_path.to_str().expect("valid path"),
        &broken_path,
        |name, data| {
            if name != "xl/_rels/workbook.xml.rels" {
                return Some((name.to_string(), data));
            }
            let xml = String::from_utf8(data).expect("workbook rels utf8");
            let xml = xml.replace(
                "</Relationships>",
                r#"<Relationship Id="rId99" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml" Target="missing.xml"/></Relationships>"#,
            );
            Some((name.to_string(), xml.into_bytes()))
        },
    );
    let broken = broken_path.to_string_lossy().to_string();
    let output = output_path.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "pivots",
        "create",
        &broken,
        "--table",
        "Sales",
        "--rows",
        "Region",
        "--values",
        "Amount:sum",
        "--out",
        &output,
    ]);
    assert_eq!(code, 5, "strict validation gate exit: {stderr:?}");
    assert_eq!(stdout, None, "failed mutation must not return success JSON");
    assert_eq!(
        stderr.expect("validation error")["error"]["code"],
        "validation_failed"
    );
    assert!(
        !output_path.exists(),
        "failed pivot validation must not leave an output workbook"
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_pivots_create_keeps_prefixed_worksheet_relationship_only() {
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
    let input_sheet_xml = read_zip_string(&input_path, "xl/worksheets/sheet1.xml");
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
    let sheet_rels = read_zip_string(&output_path, "xl/worksheets/_rels/sheet1.xml.rels");
    let workbook_xml = read_zip_string(&output_path, "xl/workbook.xml");
    assert!(
        !sheet_xml.contains("pivotTableParts"),
        "worksheet must not contain pivotTableParts: {sheet_xml}"
    );
    assert_eq!(
        sheet_xml, input_sheet_xml,
        "pivot creation must leave worksheet XML, including tableParts, untouched"
    );
    assert_eq!(
        sheet_rels.matches("relationships/pivotTable\"").count(),
        1,
        "worksheet must have exactly one pivotTable relationship: {sheet_rels}"
    );
    assert!(
        workbook_xml
            .contains(r#"<x:pivotCaches><x:pivotCache cacheId="1" r:id="rId2"/></x:pivotCaches>"#),
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
fn xlsx_pivots_create_adds_one_relationship_per_pivot_without_worksheet_children() {
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

    let mut second_create = None;
    for (source, output, anchor) in [(&input, &first, "D1"), (&first, &second, "G1")] {
        let (code, stdout, stderr) = run_ooxml(&[
            "--json",
            "xlsx",
            "pivots",
            "create",
            source,
            "--table",
            "Sales",
            "--name",
            "SalesPivot",
            "--rows",
            "Region",
            "--values",
            "Amount:sum",
            "--anchor",
            anchor,
            "--out",
            output,
        ]);
        assert_eq!(
            code, 0,
            "pivots create at {anchor} exit: {stderr:?} {stdout:?}"
        );
        if output == &second {
            second_create = stdout;
        }
    }

    let sheet_xml = read_zip_string(&second_path, "xl/worksheets/sheet1.xml");
    assert!(
        !sheet_xml.contains("pivotTableParts"),
        "worksheet must not contain pivotTableParts: {sheet_xml}"
    );
    let sheet_rels = read_zip_string(&second_path, "xl/worksheets/_rels/sheet1.xml.rels");
    assert_eq!(
        sheet_rels.matches("relationships/pivotTable\"").count(),
        2,
        "each pivot must have exactly one worksheet relationship: {sheet_rels}"
    );
    assert!(sheet_rels.contains("../pivotTables/pivotTable1.xml"));
    assert!(sheet_rels.contains("../pivotTables/pivotTable2.xml"));
    let (code, list, stderr) = run_ooxml(&["--json", "xlsx", "pivots", "list", &second]);
    assert_eq!(code, 0, "pivots list exit: {stderr:?} {list:?}");
    let pivots = list.expect("pivots list output")["pivots"]
        .as_array()
        .expect("pivots array")
        .to_vec();
    assert_eq!(pivots.len(), 2, "both pivots must be discoverable");
    assert_eq!(
        pivots
            .iter()
            .filter(|pivot| pivot["name"] == "SalesPivot")
            .count(),
        2,
        "regression requires duplicate pivot names"
    );
    let second_create = second_create.expect("second pivot create result");
    assert!(
        second_create["pivotsShowCommand"]
            .as_str()
            .expect("second pivot show command")
            .contains("--pivot part:/xl/pivotTables/pivotTable2.xml"),
        "repeat create should point at the newly allocated pivot part: {second_create}"
    );
    assert_rust_emitted_ooxml_command_succeeds(&second_create, "pivotsShowCommand");
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

#[test]
fn xlsx_pivots_three_ci_scenarios_are_relationship_only_and_schema_clean() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-pivots-ci-scenarios-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let minimal =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/xlsx/minimal-workbook/workbook.xlsx");
    let pivot_values = temp_dir.join("xlsx-pivot-data.csv");
    fs::write(
        &pivot_values,
        "Region,Product,Sales\nNorth,A,42\nSouth,A,58\nNorth,B,30\nSouth,B,33\n",
    )
    .expect("write pivot CSV");
    let authoring_values = temp_dir.join("xlsx-authoring-values.json");
    fs::write(
        &authoring_values,
        r#"[
  ["Region","Account","Units","Unit Price","Revenue"],
  ["North","Enterprise",12,19.95,{"formula":"C2*D2"}],
  ["South","Midmarket",8,24.50,{"formula":"C3*D3"}],
  ["West","Startup",15,9.99,{"formula":"C4*D4"}],
  ["East","Renewal",10,29.00,{"formula":"C5*D5"}]
]"#,
    )
    .expect("write authoring JSON");

    let pivot_data = temp_dir.join("xlsx-pivot-data.xlsx");
    run_pivot_logged_ok(
        "stage:xlsx-pivot-data",
        &[
            "--json",
            "xlsx",
            "ranges",
            "set",
            path_str(&minimal),
            "--sheet",
            "1",
            "--anchor",
            "A1",
            "--data-format",
            "csv",
            "--values-file",
            path_str(&pivot_values),
            "--out",
            path_str(&pivot_data),
        ],
    );
    assert_pivot_strict_logged("stage:xlsx-pivot-data", &pivot_data);

    let csv_pivot = temp_dir.join("xlsx-pivot-create.xlsx");
    run_pivot_logged_ok(
        "xlsx-pivot-create",
        &[
            "--json",
            "xlsx",
            "pivots",
            "create",
            path_str(&pivot_data),
            "--sheet",
            "1",
            "--range",
            "A1:C5",
            "--rows",
            "Region",
            "--values",
            "Sales:sum",
            "--anchor",
            "F1",
            "--out",
            path_str(&csv_pivot),
        ],
    );
    assert_pivot_artifact_proof("xlsx-pivot-create", &csv_pivot, "1", "A1:C5");

    let named_data = temp_dir.join("xlsx-pivot-named-data.xlsx");
    run_pivot_logged_ok(
        "stage:xlsx-pivot-named-data",
        &[
            "--json",
            "xlsx",
            "names",
            "add",
            path_str(&pivot_data),
            "--name",
            "PivotSource",
            "--sheet",
            "1",
            "--range",
            "A1:C5",
            "--comment",
            "Pivot smoke source",
            "--out",
            path_str(&named_data),
        ],
    );
    assert_pivot_strict_logged("stage:xlsx-pivot-named-data", &named_data);

    let named_pivot = temp_dir.join("xlsx-pivot-create-after-names.xlsx");
    run_pivot_logged_ok(
        "xlsx-pivot-create-after-names",
        &[
            "--json",
            "xlsx",
            "pivots",
            "create",
            path_str(&named_data),
            "--sheet",
            "1",
            "--range",
            "A1:C5",
            "--rows",
            "Region",
            "--values",
            "Sales:sum",
            "--anchor",
            "F1",
            "--out",
            path_str(&named_pivot),
        ],
    );
    assert_pivot_artifact_proof("xlsx-pivot-create-after-names", &named_pivot, "1", "A1:C5");

    let authoring_seed = temp_dir.join("xlsx-authoring-seed.xlsx");
    run_pivot_logged_ok(
        "stage:xlsx-authoring-seed",
        &[
            "--json",
            "xlsx",
            "scaffold",
            path_str(&authoring_seed),
            "--sheet",
            "Sales Ops",
            "--force",
        ],
    );
    assert_pivot_strict_logged("stage:xlsx-authoring-seed", &authoring_seed);
    let authoring_data = temp_dir.join("xlsx-authoring-data.xlsx");
    run_pivot_logged_ok(
        "stage:xlsx-authoring-data",
        &[
            "--json",
            "xlsx",
            "ranges",
            "set",
            path_str(&authoring_seed),
            "--sheet",
            "Sales Ops",
            "--range",
            "A1:E5",
            "--values-file",
            path_str(&authoring_values),
            "--out",
            path_str(&authoring_data),
        ],
    );
    assert_pivot_strict_logged("stage:xlsx-authoring-data", &authoring_data);
    let authoring_table = temp_dir.join("xlsx-authoring-table.xlsx");
    run_pivot_logged_ok(
        "stage:xlsx-authoring-table",
        &[
            "--json",
            "xlsx",
            "tables",
            "create",
            path_str(&authoring_data),
            "--sheet",
            "Sales Ops",
            "--range",
            "A1:E5",
            "--table",
            "SalesOps",
            "--style",
            "TableStyleMedium4",
            "--out",
            path_str(&authoring_table),
        ],
    );
    assert_pivot_strict_logged("stage:xlsx-authoring-table", &authoring_table);
    let authoring_cf = temp_dir.join("xlsx-authoring-conditional-format.xlsx");
    run_pivot_logged_ok(
        "stage:xlsx-authoring-conditional-format",
        &[
            "--json",
            "xlsx",
            "conditional-formats",
            "add",
            path_str(&authoring_table),
            "--sheet",
            "Sales Ops",
            "--range",
            "E2:E5",
            "--type",
            "color-scale",
            "--cfvo",
            "min",
            "--cfvo",
            "percentile:50",
            "--cfvo",
            "max",
            "--color",
            "F8696B",
            "--color",
            "FFEB84",
            "--color",
            "63BE7B",
            "--priority",
            "1",
            "--out",
            path_str(&authoring_cf),
        ],
    );
    assert_pivot_strict_logged("stage:xlsx-authoring-conditional-format", &authoring_cf);
    let authoring_named = temp_dir.join("xlsx-authoring-named-range.xlsx");
    run_pivot_logged_ok(
        "stage:xlsx-authoring-named-range",
        &[
            "--json",
            "xlsx",
            "names",
            "add",
            path_str(&authoring_cf),
            "--name",
            "SalesOpsSource",
            "--sheet",
            "Sales Ops",
            "--range",
            "A1:E5",
            "--comment",
            "Scaffold-derived Office smoke source",
            "--out",
            path_str(&authoring_named),
        ],
    );
    assert_pivot_strict_logged("stage:xlsx-authoring-named-range", &authoring_named);
    let table_parts_before = worksheet_table_parts(&authoring_named);

    let realistic_pivot = temp_dir.join("xlsx-realistic-scaffold-pivot-chain.xlsx");
    run_pivot_logged_ok(
        "xlsx-realistic-scaffold-pivot-chain",
        &[
            "--json",
            "xlsx",
            "pivots",
            "create",
            path_str(&authoring_named),
            "--sheet",
            "Sales Ops",
            "--range",
            "A1:D5",
            "--rows",
            "Region",
            "--values",
            "Units:sum",
            "--anchor",
            "G1",
            "--out",
            path_str(&realistic_pivot),
        ],
    );
    assert_pivot_artifact_proof(
        "xlsx-realistic-scaffold-pivot-chain",
        &realistic_pivot,
        "Sales Ops",
        "A1:D5",
    );
    assert_eq!(
        worksheet_table_parts(&realistic_pivot),
        table_parts_before,
        "pivot creation must leave legitimate worksheet tableParts untouched"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_pivots_legacy_fixture_is_diagnosed_and_repaired_by_both_paths() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-pivots-legacy-repair-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/xlsx/invalid/pivot-table-parts.xlsx");

    println!(
        "command: {} --json validate --strict {}",
        env!("CARGO_BIN_EXE_ooxml"),
        fixture.display()
    );
    let (code, report, stderr) = run_ooxml(&["--json", "validate", "--strict", path_str(&fixture)]);
    assert_eq!(code, 5, "legacy fixture strict validation exit");
    assert_eq!(stderr, None, "legacy fixture validation stderr");
    let report = report.expect("legacy fixture validation report");
    println!("validation summary: {}", report["summary"]);
    assert!(json_contains_diagnostic_code(&report, "XML_UNKNOWN_CHILD"));
    let diagnostic = &report["diagnostics"][0];
    assert_eq!(diagnostic["part"], "/xl/worksheets/sheet1.xml");
    assert_eq!(diagnostic["element"], "pivotTableParts");

    let before = run_pivot_logged_ok(
        "legacy fixture relationship readback",
        &["--json", "xlsx", "pivots", "list", path_str(&fixture)],
    );
    assert_eq!(before["pivots"][0]["name"], "PivotTable1");

    let mutated = temp_dir.join("repaired-on-next-mutation.xlsx");
    let mutation = run_pivot_logged_ok(
        "legacy repair on next XLSX mutation",
        &[
            "--json",
            "xlsx",
            "cells",
            "set",
            path_str(&fixture),
            "--sheet",
            "1",
            "--cell",
            "B2",
            "--value",
            "Repaired",
            "--out",
            path_str(&mutated),
        ],
    );
    assert_eq!(mutation["repairedLegacyPivotTableParts"], true);
    assert_pivot_strict_logged("legacy repair on next XLSX mutation", &mutated);
    assert_relationship_only_pivot_survives("legacy mutation repair", &mutated);
    assert_openxml_sdk_clean_or_log_skip("legacy mutation repair", &mutated);

    let normalized = temp_dir.join("repaired-by-normalize.xlsx");
    let repair = run_pivot_logged_ok(
        "repair normalize legacy pivot child",
        &[
            "--json",
            "repair",
            "normalize",
            path_str(&fixture),
            "--out",
            path_str(&normalized),
        ],
    );
    assert_eq!(repair["changed"], true);
    assert_eq!(repair["repairedLegacyPivotTableParts"], true);
    assert_eq!(
        repair["repairs"][0]["code"],
        "WORKSHEET_LEGACY_PIVOT_TABLE_PARTS_REMOVED"
    );
    assert_pivot_strict_logged("repair normalize legacy pivot child", &normalized);
    assert_relationship_only_pivot_survives("legacy normalize repair", &normalized);
    assert_openxml_sdk_clean_or_log_skip("legacy normalize repair", &normalized);

    let _ = fs::remove_dir_all(&temp_dir);
}

fn run_pivot_logged_ok(label: &str, args: &[&str]) -> Value {
    println!(
        "command [{label}]: {} {}",
        env!("CARGO_BIN_EXE_ooxml"),
        args.join(" ")
    );
    let (code, stdout, stderr) = run_ooxml(args);
    assert_eq!(code, 0, "{label} exit: stderr={stderr:?} stdout={stdout:?}");
    assert_eq!(stderr, None, "{label} stderr");
    let output = stdout.unwrap_or_else(|| panic!("{label} JSON stdout"));
    if let Some(path) = output.get("output").and_then(Value::as_str) {
        println!("output [{label}]: {path}");
    }
    output
}

fn assert_pivot_strict_logged(label: &str, path: &Path) {
    let report = run_pivot_logged_ok(
        &format!("validate:{label}"),
        &["--json", "validate", "--strict", path_str(path)],
    );
    println!("validation summary [{label}]: {}", report["summary"]);
    assert_eq!(report["valid"], true, "{label} strict validation");
}

fn assert_pivot_artifact_proof(label: &str, path: &Path, sheet: &str, range: &str) {
    println!("artifact [{label}]: {}", path.display());
    assert_pivot_strict_logged(label, path);
    let conformance = run_pivot_logged_ok(
        &format!("conformance:{label}"),
        &["--json", "conformance", "check", path_str(path)],
    );
    assert_eq!(conformance["status"], "passed", "{label} conformance");
    let pivot = run_pivot_logged_ok(
        &format!("pivots-show:{label}"),
        &[
            "--json",
            "xlsx",
            "pivots",
            "show",
            path_str(path),
            "--sheet",
            sheet,
            "--pivot",
            "part:/xl/pivotTables/pivotTable1.xml",
        ],
    );
    assert_eq!(
        pivot["pivots"][0]["partUri"],
        "/xl/pivotTables/pivotTable1.xml"
    );
    let source = run_pivot_logged_ok(
        &format!("source-export:{label}"),
        &[
            "--json",
            "xlsx",
            "ranges",
            "export",
            path_str(path),
            "--sheet",
            sheet,
            "--range",
            range,
            "--include-types",
            "--include-formulas",
        ],
    );
    assert_eq!(source["range"], range);
    let sheet_xml = read_zip_string(path, "xl/worksheets/sheet1.xml");
    assert!(
        !sheet_xml.contains("pivotTableParts"),
        "{label} wrote invalid worksheet pivotTableParts: {sheet_xml}"
    );
    let sheet_rels = read_zip_string(path, "xl/worksheets/_rels/sheet1.xml.rels");
    assert_eq!(
        sheet_rels.matches("relationships/pivotTable\"").count(),
        1,
        "{label} must write exactly one pivotTable relationship: {sheet_rels}"
    );
    assert_openxml_sdk_clean_or_log_skip(label, path);
}

fn assert_relationship_only_pivot_survives(label: &str, path: &Path) {
    let sheet_xml = read_zip_string(path, "xl/worksheets/sheet1.xml");
    assert!(
        !sheet_xml.contains("pivotTableParts"),
        "{label} left legacy pivotTableParts: {sheet_xml}"
    );
    let show = run_pivot_logged_ok(
        &format!("pivots-show:{label}"),
        &[
            "--json",
            "xlsx",
            "pivots",
            "show",
            path_str(path),
            "--sheet",
            "1",
            "--pivot",
            "part:/xl/pivotTables/pivotTable1.xml",
        ],
    );
    assert_eq!(show["pivots"][0]["name"], "PivotTable1");
}

fn worksheet_table_parts(path: &Path) -> Option<String> {
    let xml = read_zip_string(path, "xl/worksheets/sheet1.xml");
    let start = xml.find("<tableParts")?;
    let end = xml[start..].find("</tableParts>")? + start + "</tableParts>".len();
    Some(xml[start..end].to_string())
}

fn assert_openxml_sdk_clean_or_log_skip(label: &str, path: &Path) {
    let validator = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    if !validator.is_file() {
        println!(
            "SKIP Open XML SDK [{label}]: validator DLL is unavailable at {}",
            validator.display()
        );
        return;
    }
    let dotnet = std::env::var_os("OOXML_DOTNET")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join("dotnet/dotnet"))
                .filter(|candidate| candidate.is_file())
        })
        .unwrap_or_else(|| PathBuf::from("dotnet"));
    let sdk_probe = match Command::new(&dotnet).arg("--list-sdks").output() {
        Ok(output) => output,
        Err(err) => {
            println!(
                "SKIP Open XML SDK [{label}]: failed to run {} --list-sdks: {err}",
                dotnet.display()
            );
            return;
        }
    };
    let sdk_list = String::from_utf8_lossy(&sdk_probe.stdout);
    if !sdk_probe.status.success()
        || !sdk_list
            .lines()
            .any(|line| line.trim_start().starts_with("8."))
    {
        println!(
            "SKIP Open XML SDK [{label}]: {} has no discoverable .NET 8 SDK; stdout={sdk_list:?}, stderr={:?}",
            dotnet.display(),
            String::from_utf8_lossy(&sdk_probe.stderr)
        );
        return;
    }

    println!(
        "command [openxml-sdk:{label}]: {} {} {}",
        dotnet.display(),
        validator.display(),
        path.display()
    );
    let output = Command::new(&dotnet)
        .arg(&validator)
        .arg(path)
        .output()
        .expect("run Open XML SDK validator");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("Open XML SDK stdout [{label}]: {stdout}");
    if !stderr.is_empty() {
        println!("Open XML SDK stderr [{label}]: {stderr}");
    }
    assert!(
        output.status.success(),
        "Open XML SDK rejected {label} at {}: stdout={stdout:?} stderr={stderr:?}",
        path.display()
    );
    assert!(
        stdout.contains("0 errors"),
        "Open XML SDK clean summary missing for {label}: {stdout:?}"
    );
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("UTF-8 test path")
}

fn assert_pivot_workbook_child_order_rejected(label: &str, file: &str) {
    for (command, expected_code, args) in [
        (
            "strict validate",
            "XML_CHILD_ORDER",
            vec!["--json", "validate", "--strict", file],
        ),
        (
            "conformance check",
            "XLSX_WORKBOOK_CHILD_ORDER",
            vec!["--json", "conformance", "check", file],
        ),
    ] {
        let (code, report, stderr) = run_ooxml(&args);
        assert_ne!(code, 0, "{label} {command} should reject bad order");
        assert_eq!(stderr, None, "{label} {command} stderr");
        let report = report.unwrap_or_else(|| panic!("{label} {command} should emit JSON"));
        assert!(
            json_contains_diagnostic_code(&report, expected_code),
            "{label} {command} did not report {expected_code}:\n{report:#}"
        );
    }
}

// Model the relationship-only package shape emitted by Excel: worksheet
// relationship targets are relative and no pivot reference element appears in
// worksheet XML. The multi-sheet variant has two pivots on Data and one on
// Archive so discovery cannot accidentally depend on a single sheet.
fn write_pivot_xlsx(dest: &Path, multiple_pivots: bool) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create pivot xlsx");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut pivot_sheet_rels = r#"<Relationship Id="rIdPivot1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable1.xml"/>"#.to_string();
    let mut pivot_overrides = r#"<Override PartName="/xl/pivotTables/pivotTable1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/>"#.to_string();
    let (extra_sheet_override, extra_sheet, extra_sheet_rel) = if multiple_pivots {
        pivot_sheet_rels.push_str(
            r#"
  <Relationship Id="rIdPivot2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable2.xml"/>"#,
        );
        pivot_overrides.push_str(
            r#"
  <Override PartName="/xl/pivotTables/pivotTable2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/>"#,
        );
        pivot_overrides.push_str(
            r#"
  <Override PartName="/xl/pivotTables/pivotTable3.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/>"#,
        );
        (
            r#"<Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#,
            r#"<sheet name="Archive" sheetId="2" r:id="rId2"/>"#,
            r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>"#,
        )
    } else {
        ("", "", "")
    };

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
  {extra_sheet_override}
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
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Data" sheetId="1" r:id="rId1"/>
    {extra_sheet}
  </sheets>
  <pivotCaches>
    <pivotCache cacheId="1" r:id="rIdCache1"/>
  </pivotCaches>
</workbook>"#
        ),
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  {extra_sheet_rel}
  <Relationship Id="rIdCache1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition" Target="pivotCache/pivotCacheDefinition1.xml"/>
</Relationships>"#
        ),
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:E6"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Quarter</t></is></c><c r="C1" t="inlineStr"><is><t>Amount</t></is></c><c r="D1" t="inlineStr"><is><t>Segment</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>East</t></is></c><c r="B2" t="inlineStr"><is><t>Q1</t></is></c><c r="C2"><v>10</v></c><c r="D2" t="inlineStr"><is><t>Enterprise</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>West</t></is></c><c r="B3" t="inlineStr"><is><t>Q2</t></is></c><c r="C3"><v>20</v></c><c r="D3" t="inlineStr"><is><t>SMB</t></is></c></row>
  </sheetData>
</worksheet>"#,
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
    if multiple_pivots {
        write_zip_string(
            &mut writer,
            options,
            "xl/pivotTables/pivotTable2.xml",
            &test_pivot_table_xml("SalesPivot2", "G3:H6"),
        );
        write_zip_string(
            &mut writer,
            options,
            "xl/worksheets/sheet2.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:D3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Quarter</t></is></c><c r="C1" t="inlineStr"><is><t>Amount</t></is></c><c r="D1" t="inlineStr"><is><t>Segment</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>East</t></is></c><c r="B2" t="inlineStr"><is><t>Q1</t></is></c><c r="C2"><v>10</v></c><c r="D2" t="inlineStr"><is><t>Enterprise</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>West</t></is></c><c r="B3" t="inlineStr"><is><t>Q2</t></is></c><c r="C3"><v>20</v></c><c r="D3" t="inlineStr"><is><t>SMB</t></is></c></row>
  </sheetData>
</worksheet>"#,
        );
        write_zip_string(
            &mut writer,
            options,
            "xl/worksheets/_rels/sheet2.xml.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdPivot3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable3.xml"/>
</Relationships>"#,
        );
        write_zip_string(
            &mut writer,
            options,
            "xl/pivotTables/pivotTable3.xml",
            &test_pivot_table_xml("ArchivePivot", "D3:E6"),
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
