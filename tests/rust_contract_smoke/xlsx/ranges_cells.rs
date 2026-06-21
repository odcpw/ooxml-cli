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
fn xlsx_ranges_set_new_formula_removes_existing_calc_chain() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-set-new-formula-calc-chain-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input_path = temp_dir.join("in.xlsx");
    let output_path = temp_dir.join("out.xlsx");
    write_calc_chain_xlsx(&input_path);
    let input = input_path.to_string_lossy().to_string();
    let output = output_path.to_string_lossy().to_string();

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "set",
        &input,
        "--sheet",
        "Sheet1",
        "--range",
        "C1:C1",
        "--values",
        r#"[[{"formula":"SUM(A1:B1)"}]]"#,
        "--out",
        &output,
    ]);
    assert_eq!(code, 0, "new formula calc-chain exit");
    assert_eq!(stderr, None, "new formula calc-chain stderr");
    assert_eq!(
        stdout.expect("new formula calc-chain stdout")["formulaCount"],
        Value::from(1)
    );
    assert_xlsx_full_calc_flags(&output_path);
    assert_xlsx_calc_chain_removed(&output_path);
    assert!(
        read_zip_string(&output_path, "xl/worksheets/sheet1.xml")
            .contains(r#"<c r="C1"><f>SUM(A1:B1)</f></c>"#),
        "new formula should be written without a cached value"
    );

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
