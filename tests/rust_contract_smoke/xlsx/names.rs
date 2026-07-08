#[test]
fn xlsx_names_list_show_matches_rust_baseline() {
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
        assert_rust_baseline_match(&args);
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
fn xlsx_names_mutations_match_rust_baseline_and_saved_readback() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-names-mutate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in = temp_dir.join("baseline-input.xlsx");
    let rust_in = temp_dir.join("rust-input.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &baseline_in).expect("stage baseline input");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &rust_in).expect("stage rust input");
    let baseline_add = temp_dir.join("baseline-add.xlsx");
    let rust_add = temp_dir.join("rust-add.xlsx");
    let baseline_update = temp_dir.join("baseline-update.xlsx");
    let rust_update = temp_dir.join("rust-update.xlsx");
    let baseline_rename = temp_dir.join("baseline-rename.xlsx");
    let rust_rename = temp_dir.join("rust-rename.xlsx");
    let baseline_delete = temp_dir.join("baseline-delete.xlsx");
    let rust_delete = temp_dir.join("rust-delete.xlsx");
    let baseline_in = baseline_in.to_string_lossy().to_string();
    let rust_in = rust_in.to_string_lossy().to_string();
    let baseline_add = baseline_add.to_string_lossy().to_string();
    let rust_add = rust_add.to_string_lossy().to_string();
    let baseline_update = baseline_update.to_string_lossy().to_string();
    let rust_update = rust_update.to_string_lossy().to_string();
    let baseline_rename = baseline_rename.to_string_lossy().to_string();
    let rust_rename = rust_rename.to_string_lossy().to_string();
    let baseline_delete = baseline_delete.to_string_lossy().to_string();
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
                &baseline_in,
                "--name",
                "SalesData",
                "--sheet",
                "Sheet1",
                "--range",
                "A1:B2",
                "--out",
                &baseline_add,
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
            vec![(&baseline_in, "[IN]"), (&baseline_add, "[ADD]")],
            vec![(&rust_in, "[IN]"), (&rust_add, "[ADD]")],
        ),
        (
            "update",
            vec![
                "--json",
                "xlsx",
                "names",
                "update",
                &baseline_add,
                "--name",
                "SalesData",
                "--ref",
                updated_ref,
                "--expect-ref",
                initial_ref,
                "--out",
                &baseline_update,
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
            vec![(&baseline_add, "[ADD]"), (&baseline_update, "[UPDATE]")],
            vec![(&rust_add, "[ADD]"), (&rust_update, "[UPDATE]")],
        ),
        (
            "rename",
            vec![
                "--json",
                "xlsx",
                "names",
                "rename",
                &baseline_update,
                "--name",
                "SalesData",
                "--new-name",
                "RevenueData",
                "--expect-ref",
                updated_ref,
                "--out",
                &baseline_rename,
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
            vec![(&baseline_update, "[UPDATE]"), (&baseline_rename, "[RENAME]")],
            vec![(&rust_update, "[UPDATE]"), (&rust_rename, "[RENAME]")],
        ),
        (
            "delete",
            vec![
                "--json",
                "xlsx",
                "names",
                "delete",
                &baseline_rename,
                "--name",
                "RevenueData",
                "--expect-ref",
                updated_ref,
                "--out",
                &baseline_delete,
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
            vec![(&baseline_rename, "[RENAME]"), (&baseline_delete, "[DELETE]")],
            vec![(&rust_rename, "[RENAME]"), (&rust_delete, "[DELETE]")],
        ),
    ];

    for (label, baseline_args, rust_args, baseline_paths, rust_paths) in steps {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, baseline_code, "{label} exit");
        assert_eq!(rust_stderr, baseline_stderr, "{label} stderr");
        let baseline_path_refs = baseline_paths
            .iter()
            .map(|(from, to)| (from.as_str(), *to))
            .collect::<Vec<_>>();
        let rust_path_refs = rust_paths
            .iter()
            .map(|(from, to)| (from.as_str(), *to))
            .collect::<Vec<_>>();
        let rust_raw = rust_stdout.expect("rust names mutation stdout");
        let baseline_json = scrub_paths(baseline_stdout.expect("baseline names mutation stdout"), &baseline_path_refs);
        let rust_json = scrub_paths(rust_raw.clone(), &rust_path_refs);
        assert_eq!(rust_json, baseline_json, "{label} stdout");
        assert_rust_emitted_ooxml_command_exits_zero(&rust_raw, "validateCommand");
        assert_rust_emitted_ooxml_command_succeeds(&rust_raw, "namesListCommand");
        if label != "delete" {
            assert_rust_emitted_ooxml_command_succeeds(&rust_raw, "nameShowCommand");
        }
    }

    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&["--json", "xlsx", "names", "list", &baseline_delete]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "xlsx", "names", "list", &rust_delete]);
    assert_eq!(rust_code, baseline_code, "post-delete list exit");
    assert_eq!(rust_stderr, baseline_stderr, "post-delete list stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust post-delete list"),
            &rust_delete,
            "[DELETE]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline post-delete list"),
            &baseline_delete,
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
fn xlsx_names_add_places_defined_names_after_sheets_and_validators_catch_bad_order() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-names-order-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let input_path = temp_dir.join("input.xlsx");
    let output_path = temp_dir.join("named.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input_path).expect("stage input");
    let input = input_path.to_string_lossy().to_string();
    let output = output_path.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "names",
        "add",
        &input,
        "--name",
        "SalesData",
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--out",
        &output,
    ]);
    assert_eq!(code, 0, "names add exit");
    assert_eq!(stderr, None, "names add stderr");
    assert!(
        stdout.is_some(),
        "names add should emit a success report"
    );

    let workbook_xml = read_zip_string(&output_path, "xl/workbook.xml");
    let sheets_end = workbook_xml
        .find("</sheets>")
        .unwrap_or_else(|| panic!("workbook missing </sheets>:\n{workbook_xml}"));
    let defined_names = workbook_xml
        .find("<definedNames")
        .unwrap_or_else(|| panic!("workbook missing <definedNames>:\n{workbook_xml}"));
    assert!(
        sheets_end < defined_names,
        "definedNames must be after sheets:\n{workbook_xml}"
    );
    if let Some(calc_pr) = workbook_xml.find("<calcPr") {
        assert!(
            defined_names < calc_pr,
            "definedNames must be before calcPr:\n{workbook_xml}"
        );
    }

    let function_groups_input_path = temp_dir.join("function-groups-input.xlsx");
    let function_groups_output_path = temp_dir.join("function-groups-named.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &function_groups_input_path,
        |name, data| {
            let data = if name == "xl/workbook.xml" {
                let xml = String::from_utf8(data).expect("minimal workbook XML utf8");
                xml.replace(
                    "</sheets>",
                    "</sheets><functionGroups count=\"1\"><functionGroup name=\"UserDefined\"/></functionGroups>",
                )
                .into_bytes()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let function_groups_input = function_groups_input_path.to_string_lossy().to_string();
    let function_groups_output = function_groups_output_path.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "names",
        "add",
        &function_groups_input,
        "--name",
        "AfterFunctionGroups",
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--out",
        &function_groups_output,
    ]);
    assert_eq!(code, 0, "functionGroups names add exit");
    assert_eq!(stderr, None, "functionGroups names add stderr");
    assert!(
        stdout.is_some(),
        "functionGroups names add should emit JSON"
    );
    let function_groups_workbook = read_zip_string(&function_groups_output_path, "xl/workbook.xml");
    let function_groups = function_groups_workbook
        .find("<functionGroups")
        .unwrap_or_else(|| panic!("workbook missing functionGroups:\n{function_groups_workbook}"));
    let defined_names = function_groups_workbook.find("<definedNames").unwrap_or_else(|| {
        panic!("workbook missing definedNames after functionGroups:\n{function_groups_workbook}")
    });
    assert!(
        function_groups < defined_names,
        "definedNames must be after functionGroups:\n{function_groups_workbook}"
    );
    assert_xlsx_strict_valid(&function_groups_output);

    let bad_order_path = temp_dir.join("bad-order.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &bad_order_path,
        |name, data| {
            let data = if name == "xl/workbook.xml" {
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <workbookPr/>
  <definedNames><definedName name="SalesData">Sheet1!$A$1:$B$2</definedName></definedNames>
  <bookViews><workbookView activeTab="0"/></bookViews>
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
  <calcPr calcId="191029"/>
</workbook>"#
                .to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let bad_order = bad_order_path.to_string_lossy().to_string();
    for args in [
        vec!["--json", "--strict", "validate", &bad_order],
        vec!["--json", "conformance", "check", &bad_order],
    ] {
        let (code, report, stderr) = run_ooxml(&args);
        assert_ne!(code, 0, "{args:?} should reject bad workbook order");
        assert_eq!(stderr, None, "{args:?} stderr");
        let report = report.unwrap_or_else(|| panic!("{args:?} should emit JSON"));
        assert!(
            json_contains_diagnostic_code(&report, "XLSX_WORKBOOK_CHILD_ORDER"),
            "{args:?} did not report workbook child order:\n{report:#}"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

fn json_contains_diagnostic_code(value: &Value, code: &str) -> bool {
    match value {
        Value::Object(map) => {
            map.get("code").and_then(Value::as_str) == Some(code)
                || map
                    .values()
                    .any(|child| json_contains_diagnostic_code(child, code))
        }
        Value::Array(items) => items
            .iter()
            .any(|child| json_contains_diagnostic_code(child, code)),
        _ => false,
    }
}

#[test]
fn xlsx_names_dry_run_and_errors_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-names-errors-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-input.xlsx");
    let rust_in_path = temp_dir.join("rust-input.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &baseline_in_path).expect("baseline input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();

    let dry_go = [
        "--json",
        "xlsx",
        "names",
        "add",
        &baseline_in,
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
        !read_zip_string(&rust_in_path, "xl/workbook.xml").contains("definedNames"),
        "dry-run should not write source workbook"
    );

    let bad_cases: Vec<Vec<&str>> = vec![
        vec!["--json", "xlsx", "names", "show", &baseline_in],
        vec![
            "--json",
            "xlsx",
            "names",
            "add",
            &baseline_in,
            "--ref",
            "Sheet1!$A$1",
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "names",
            "add",
            &baseline_in,
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
            &baseline_in,
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
            &baseline_in,
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
            &baseline_in,
            "--name",
            "Input",
            "--range",
            "A1",
            "--dry-run",
        ],
    ];
    for baseline_args in bad_cases {
        let mut rust_args = baseline_args.clone();
        for arg in &mut rust_args {
            if *arg == baseline_in {
                *arg = &rust_in;
            }
        }
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, baseline_code, "bad args exit for {baseline_args:?}");
        assert_eq!(rust_stdout, baseline_stdout, "bad args stdout for {baseline_args:?}");
        assert_eq!(
            scrub_path(rust_stderr.expect("rust bad args stderr"), &rust_in, "[IN]"),
            scrub_path(baseline_stderr.expect("baseline bad args stderr"), &baseline_in, "[IN]"),
            "bad args stderr for {baseline_args:?}"
        );
    }

    let baseline_add = temp_dir.join("baseline-add.xlsx").to_string_lossy().to_string();
    let rust_add = temp_dir.join("rust-add.xlsx").to_string_lossy().to_string();
    assert_eq!(
        run_ooxml_baseline(&[
            "--json",
            "xlsx",
            "names",
            "add",
            &baseline_in,
            "--name",
            "Input",
            "--ref",
            "Sheet1!$A$1",
            "--out",
            &baseline_add,
        ])
        .0,
        0,
        "baseline stale setup"
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
    let baseline_stale = [
        "--json",
        "xlsx",
        "names",
        "update",
        &baseline_add,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_stale);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_stale);
    assert_eq!(rust_code, baseline_code, "stale guard exit");
    assert_eq!(rust_stdout, baseline_stdout, "stale guard stdout");
    assert_eq!(
        scrub_path(rust_stderr.expect("rust stale stderr"), &rust_add, "[ADD]"),
        scrub_path(baseline_stderr.expect("baseline stale stderr"), &baseline_add, "[ADD]"),
        "stale guard stderr"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

