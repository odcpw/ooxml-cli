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
