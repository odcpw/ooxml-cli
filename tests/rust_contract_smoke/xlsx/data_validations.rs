#[test]
fn xlsx_data_validations_list_show_match_rust_baseline() {
    assert_rust_baseline_match(&[
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
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "data-validations",
        "list",
        &workbook,
        "--sheet",
        "Sheet1",
    ]);
    assert_rust_baseline_match(&[
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
    assert_rust_baseline_match(&[
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
    assert_rust_baseline_match(&[
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
fn xlsx_data_validations_create_update_delete_saved_outputs_match_rust_baseline() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-dv-mutate-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let baseline_create_out = temp_dir
        .join("baseline-create.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_create_out = temp_dir
        .join("rust-create.xlsx")
        .to_string_lossy()
        .to_string();
    let baseline_update_out = temp_dir
        .join("baseline-update.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_update_out = temp_dir
        .join("rust-update.xlsx")
        .to_string_lossy()
        .to_string();
    let baseline_delete_out = temp_dir
        .join("baseline-delete.xlsx")
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
    let mut baseline_args = create_common.to_vec();
    baseline_args.push(&baseline_create_out);
    let mut rust_args = create_common.to_vec();
    rust_args.push(&rust_create_out);
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "data validation create exit");
    assert_eq!(rust_stderr, baseline_stderr, "data validation create stderr");
    let rust_create = rust_stdout.expect("rust create stdout");
    assert_eq!(
        scrub_path(rust_create.clone(), &rust_create_out, "[CREATE_OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline create stdout"),
            &baseline_create_out,
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
        &baseline_create_out,
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
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "saved create show exit");
    assert_eq!(rust_stderr, baseline_stderr, "saved create show stderr");
    assert_eq!(
        rust_show.expect("rust saved create show"),
        baseline_show.expect("baseline saved create show"),
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
    let mut baseline_args = vec![
        "--json",
        "xlsx",
        "data-validations",
        "update",
        &baseline_create_out,
    ];
    baseline_args.extend_from_slice(&update_common[4..]);
    baseline_args.push(&baseline_update_out);
    let mut rust_args = vec![
        "--json",
        "xlsx",
        "data-validations",
        "update",
        &rust_create_out,
    ];
    rust_args.extend_from_slice(&update_common[4..]);
    rust_args.push(&rust_update_out);
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "data validation update exit");
    assert_eq!(rust_stderr, baseline_stderr, "data validation update stderr");
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
            baseline_stdout.expect("baseline update stdout"),
            &[
                (&baseline_create_out, "[CREATE_OUT]"),
                (&baseline_update_out, "[UPDATE_OUT]")
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
    let mut baseline_args = vec![
        "--json",
        "xlsx",
        "data-validations",
        "delete",
        &baseline_update_out,
    ];
    baseline_args.extend_from_slice(&delete_common[4..]);
    baseline_args.push(&baseline_delete_out);
    let mut rust_args = vec![
        "--json",
        "xlsx",
        "data-validations",
        "delete",
        &rust_update_out,
    ];
    rust_args.extend_from_slice(&delete_common[4..]);
    rust_args.push(&rust_delete_out);
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "data validation delete exit");
    assert_eq!(rust_stderr, baseline_stderr, "data validation delete stderr");
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
            baseline_stdout.expect("baseline delete stdout"),
            &[
                (&baseline_update_out, "[UPDATE_OUT]"),
                (&baseline_delete_out, "[DELETE_OUT]")
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
        &baseline_delete_out,
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
    let (baseline_code, baseline_list, baseline_stderr) = run_ooxml_baseline(&list_go);
    let (rust_code, rust_list, rust_stderr) = run_ooxml(&list_rust);
    assert_eq!(rust_code, baseline_code, "deleted list exit");
    assert_eq!(rust_stderr, baseline_stderr, "deleted list stderr");
    assert_eq!(
        scrub_path(
            rust_list.expect("rust deleted list"),
            &rust_delete_out,
            "[DELETE_OUT]"
        ),
        scrub_path(
            baseline_list.expect("baseline deleted list"),
            &baseline_delete_out,
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
fn xlsx_data_validations_dry_run_and_errors_match_rust_baseline() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-dv-dry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &baseline_in_path).expect("copy baseline input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("copy rust input");
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();

    let before = read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml");
    let dry_go = [
        "--json",
        "xlsx",
        "data-validations",
        "create",
        &baseline_in,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, baseline_code, "data validation dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "data validation dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust data validation dry-run"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline data validation dry-run"),
            &baseline_in,
            "[IN]"
        ),
        "data validation dry-run stdout"
    );
    assert_eq!(
        read_zip_string(&rust_in_path, "xl/worksheets/sheet1.xml"),
        before,
        "data validation dry-run should not mutate source workbook"
    );

    for (label, baseline_bad, rust_bad) in [
        (
            "missing list source",
            vec![
                "--json",
                "xlsx",
                "data-validations",
                "create",
                &baseline_in,
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
                &baseline_in,
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
                &baseline_in,
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
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, baseline_code, "{label} exit");
        assert_eq!(rust_stdout, baseline_stdout, "{label} stdout");
        assert_eq!(
            scrub_path(
                rust_stderr.expect("rust data validation bad stderr"),
                &rust_in,
                "[IN]"
            ),
            scrub_path(
                baseline_stderr.expect("baseline data validation bad stderr"),
                &baseline_in,
                "[IN]"
            ),
            "{label} stderr"
        );
    }

    let baseline_created = temp_dir
        .join("baseline-created.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_created = temp_dir
        .join("rust-created.xlsx")
        .to_string_lossy()
        .to_string();
    for (input, output, runner) in [
        (
            &baseline_in,
            &baseline_created,
            run_ooxml_baseline as fn(&[&str]) -> (i32, Option<Value>, Option<Value>),
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
        &baseline_created,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&guard_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&guard_rust);
    assert_eq!(rust_code, baseline_code, "guard mismatch exit");
    assert_eq!(rust_stdout, baseline_stdout, "guard mismatch stdout");
    assert_eq!(
        scrub_path(
            rust_stderr.expect("rust guard mismatch stderr"),
            &rust_created,
            "[CREATED]"
        ),
        scrub_path(
            baseline_stderr.expect("baseline guard mismatch stderr"),
            &baseline_created,
            "[CREATED]"
        ),
        "guard mismatch stderr"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
