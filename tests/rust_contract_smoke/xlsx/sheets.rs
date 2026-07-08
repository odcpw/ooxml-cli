#[test]
fn xlsx_sheets_show_matches_rust_baseline() {
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
        assert_rust_baseline_match(&args);
    }
}

#[test]
fn xlsx_sheets_add_matches_rust_baseline_shape_and_saved_output() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-sheets-add-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let baseline_out_path = temp_dir.join("baseline-add.xlsx");
    let rust_out_path = temp_dir.join("rust-add.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &baseline_in_path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>"#,
    );
    fs::copy(&baseline_in_path, &rust_in_path).expect("copy rust add input");
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let baseline_out = baseline_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let baseline_args = [
        "--json", "xlsx", "sheets", "add", &baseline_in, "--name", "Added", "--out", &baseline_out,
    ];
    let rust_args = [
        "--json", "xlsx", "sheets", "add", &rust_in, "--name", "Added", "--out", &rust_out,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "sheets add exit");
    assert_eq!(rust_stderr, baseline_stderr, "sheets add stderr");
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
                baseline_stdout.expect("baseline sheets add stdout"),
                &[(&baseline_in, "[IN]"), (&baseline_out, "[OUT]")]
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
        &baseline_in,
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
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, baseline_code, "sheets add dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "sheets add dry-run stderr");
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
                baseline_stdout.expect("baseline sheets add dry-run stdout"),
                &baseline_in,
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
        let mut baseline_bad = vec!["--json", "xlsx", "sheets", "add", &baseline_in];
        baseline_bad.extend(extra.iter().copied());
        let mut rust_bad = vec!["--json", "xlsx", "sheets", "add", &rust_in];
        rust_bad.extend(extra.iter().copied());
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_bad);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bad);
        assert_eq!(rust_code, baseline_code, "sheets add {label} exit");
        assert_eq!(rust_stdout, baseline_stdout, "sheets add {label} stdout");
        assert_eq!(rust_stderr, baseline_stderr, "sheets add {label} stderr");
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_sheets_rename_move_delete_match_rust_baseline_and_saved_outputs() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-sheets-life-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_in_path = temp_dir.join("baseline-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    write_sheet_lifecycle_xlsx(&baseline_in_path);
    fs::copy(&baseline_in_path, &rust_in_path).expect("copy rust lifecycle input");
    let baseline_in = baseline_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();

    let baseline_rename_path = temp_dir.join("baseline-rename.xlsx");
    let rust_rename_path = temp_dir.join("rust-rename.xlsx");
    let baseline_rename = baseline_rename_path.to_string_lossy().to_string();
    let rust_rename = rust_rename_path.to_string_lossy().to_string();
    let baseline_args = [
        "--json", "xlsx", "sheets", "rename", &baseline_in, "--sheet", "Data", "--name", "Facts",
        "--out", &baseline_rename,
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
    let rust_rename_result = assert_xlsx_sheet_mutation_matches_rust_baseline(
        "sheets rename",
        &baseline_args,
        &rust_args,
        &[(&baseline_in, "[IN]"), (&baseline_rename, "[OUT]")],
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
        &baseline_in,
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
    assert_xlsx_sheet_mutation_matches_rust_baseline(
        "sheets rename dry-run",
        &dry_go,
        &dry_rust,
        &[(&baseline_in, "[IN]")],
        &[(&rust_in, "[IN]")],
    );
    assert!(
        read_zip_string(&rust_in_path, "xl/workbook.xml").contains(r#"name="Data""#),
        "rename dry-run changed source workbook"
    );

    let baseline_move_path = temp_dir.join("baseline-move.xlsx");
    let rust_move_path = temp_dir.join("rust-move.xlsx");
    let baseline_move = baseline_move_path.to_string_lossy().to_string();
    let rust_move = rust_move_path.to_string_lossy().to_string();
    let baseline_args = [
        "--json", "xlsx", "sheets", "move", &baseline_rename, "--sheet", "Facts", "--before", "Summary",
        "--out", &baseline_move,
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
    let rust_move_result = assert_xlsx_sheet_mutation_matches_rust_baseline(
        "sheets move",
        &baseline_args,
        &rust_args,
        &[(&baseline_rename, "[IN]"), (&baseline_move, "[OUT]")],
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
        &baseline_move,
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
    assert_xlsx_sheet_error_matches_rust_baseline("sheets move target guard", &bad_move_go, &bad_move_rust);

    let baseline_delete_path = temp_dir.join("baseline-delete.xlsx");
    let rust_delete_path = temp_dir.join("rust-delete.xlsx");
    let baseline_delete = baseline_delete_path.to_string_lossy().to_string();
    let rust_delete = rust_delete_path.to_string_lossy().to_string();
    let baseline_args = [
        "--json", "xlsx", "sheets", "delete", &baseline_move, "--sheet", "Summary", "--out", &baseline_delete,
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
    let rust_delete_result = assert_xlsx_sheet_mutation_matches_rust_baseline(
        "sheets delete",
        &baseline_args,
        &rust_args,
        &[(&baseline_move, "[IN]"), (&baseline_delete, "[OUT]")],
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
    assert_xlsx_sheet_error_matches_rust_baseline("sheets delete last sheet", &last_go, &last_go);

    let _ = fs::remove_dir_all(&temp_dir);
}
