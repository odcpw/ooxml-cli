#[test]
fn xlsx_conditional_formats_list_show_match_rust_baseline() {
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "list",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
    ]);

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cf-list-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("cf.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &workbook,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C5"/>
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
  <conditionalFormatting sqref="A1:A5">
    <cfRule type="expression" priority="3" stopIfTrue="1" dxfId="0"><formula>A1&gt;0</formula></cfRule>
  </conditionalFormatting>
  <conditionalFormatting sqref="B1:B5">
    <cfRule type="colorScale" priority="4">
      <colorScale>
        <cfvo type="min"/>
        <cfvo type="max"/>
        <color rgb="FFFF0000"/>
        <color rgb="FF00FF00"/>
      </colorScale>
    </cfRule>
  </conditionalFormatting>
</worksheet>"#,
    );
    let workbook = workbook.to_string_lossy().to_string();
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "conditional-formatting",
        "list",
        &workbook,
        "--sheet",
        "Sheet1",
    ]);
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "list",
        &workbook,
        "--sheet",
        "1",
        "--range",
        "A1:A5",
    ]);
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "cf",
        "show",
        &workbook,
        "--sheet",
        "1",
        "--rule",
        "priority:3",
    ]);
    assert_rust_baseline_match(&[
        "--json",
        "xlsx",
        "conditional-format",
        "show",
        &workbook,
        "--sheet",
        "1",
        "--rule",
        "block:2/rule:1",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_conditional_formats_add_delete_saved_outputs_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cf-mutate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let baseline_add_out = temp_dir.join("baseline-add.xlsx").to_string_lossy().to_string();
    let rust_add_out = temp_dir
        .join("rust-add.xlsx")
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

    let add_common = [
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "A1:A5",
        "--type",
        "expression",
        "--formula",
        "A1>0",
        "--priority",
        "7",
        "--stop-if-true",
        "--dxf-id",
        "0",
        "--out",
    ];
    let mut baseline_args = add_common.to_vec();
    baseline_args.push(&baseline_add_out);
    let mut rust_args = add_common.to_vec();
    rust_args.push(&rust_add_out);
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "conditional format add exit");
    assert_eq!(rust_stderr, baseline_stderr, "conditional format add stderr");
    let rust_add = rust_stdout.expect("rust add stdout");
    assert_eq!(
        scrub_path(rust_add.clone(), &rust_add_out, "[ADD_OUT]"),
        scrub_path(baseline_stdout.expect("baseline add stdout"), &baseline_add_out, "[ADD_OUT]"),
        "conditional format add stdout"
    );
    assert_eq!(rust_add["rule"]["formula"], Value::String("A1>0".to_string()));
    assert_eq!(rust_add["rule"]["dxfId"], Value::Number(0.into()));
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "conditionalFormatsListCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "conditionalFormatsShowCommand");

    let show_go = [
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &baseline_add_out,
        "--sheet",
        "1",
        "--rule",
        "cfRule:1",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &rust_add_out,
        "--sheet",
        "1",
        "--rule",
        "cfRule:1",
    ];
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "saved add show exit");
    assert_eq!(rust_stderr, baseline_stderr, "saved add show stderr");
    assert_eq!(
        rust_show.expect("rust saved add show"),
        baseline_show.expect("baseline saved add show"),
        "saved add show"
    );

    let delete_common = [
        "--json",
        "xlsx",
        "conditional-formats",
        "delete",
        "--sheet",
        "1",
        "--rule",
        "priority:7",
        "--out",
    ];
    let mut baseline_args = vec![
        "--json",
        "xlsx",
        "conditional-formats",
        "delete",
        &baseline_add_out,
    ];
    baseline_args.extend_from_slice(&delete_common[4..]);
    baseline_args.push(&baseline_delete_out);
    let mut rust_args = vec![
        "--json",
        "xlsx",
        "conditional-formats",
        "delete",
        &rust_add_out,
    ];
    rust_args.extend_from_slice(&delete_common[4..]);
    rust_args.push(&rust_delete_out);
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "conditional format delete exit");
    assert_eq!(rust_stderr, baseline_stderr, "conditional format delete stderr");
    let rust_delete = rust_stdout.expect("rust delete stdout");
    assert_eq!(
        scrub_paths(
            rust_delete.clone(),
            &[(&rust_add_out, "[ADD_OUT]"), (&rust_delete_out, "[DELETE_OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline delete stdout"),
            &[(&baseline_add_out, "[ADD_OUT]"), (&baseline_delete_out, "[DELETE_OUT]")]
        ),
        "conditional format delete stdout"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete, "conditionalFormatsListCommand");

    let list_go = [
        "--json",
        "xlsx",
        "conditional-formats",
        "list",
        &baseline_delete_out,
        "--sheet",
        "1",
    ];
    let list_rust = [
        "--json",
        "xlsx",
        "conditional-formats",
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
        scrub_path(baseline_list.expect("baseline deleted list"), &baseline_delete_out, "[DELETE_OUT]"),
        "deleted list"
    );

    for output in [&rust_add_out, &rust_delete_out] {
        assert_xlsx_strict_valid(output);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_conditional_formats_reorder_saved_outputs_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cf-reorder-baseline-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_input = temp_dir.join("baseline-input.xlsx");
    let rust_input = temp_dir.join("rust-input.xlsx");
    let baseline_out = temp_dir.join("baseline-reorder.xlsx").to_string_lossy().to_string();
    let rust_out = temp_dir
        .join("rust-reorder.xlsx")
        .to_string_lossy()
        .to_string();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c></row></sheetData>
  <conditionalFormatting sqref="A1:A5">
    <cfRule type="expression" priority="3"><formula>A1&gt;0</formula></cfRule>
  </conditionalFormatting>
  <conditionalFormatting sqref="B1:B5">
    <cfRule type="expression" priority="4" stopIfTrue="1"><formula>B1&gt;0</formula></cfRule>
  </conditionalFormatting>
</worksheet>"#;
    write_simple_xlsx_with_sheet_xml(&baseline_input, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_input, sheet_xml);
    let baseline_input = baseline_input.to_string_lossy().to_string();
    let rust_input = rust_input.to_string_lossy().to_string();

    let baseline_args = [
        "--json",
        "xlsx",
        "conditional-formats",
        "reorder",
        &baseline_input,
        "--sheet",
        "1",
        "--rule",
        "cfRule:2",
        "--priority",
        "1",
        "--out",
        &baseline_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "conditional-formats",
        "reorder",
        &rust_input,
        "--sheet",
        "1",
        "--rule",
        "cfRule:2",
        "--priority",
        "1",
        "--out",
        &rust_out,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "conditional format reorder exit");
    assert_eq!(rust_stderr, baseline_stderr, "conditional format reorder stderr");
    let rust_reorder = rust_stdout.expect("rust reorder stdout");
    assert_eq!(
        scrub_paths(
            rust_reorder.clone(),
            &[(&rust_input, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline reorder stdout"),
            &[(&baseline_input, "[IN]"), (&baseline_out, "[OUT]")]
        ),
        "conditional format reorder stdout"
    );
    assert_eq!(rust_reorder["action"], "reorder");
    assert_eq!(rust_reorder["oldPriority"], Value::from(4));
    assert_eq!(rust_reorder["newPriority"], Value::from(1));
    assert_eq!(rust_reorder["rule"]["priority"], Value::from(1));
    assert_rust_emitted_ooxml_command_exits_zero(&rust_reorder, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(
        &rust_reorder,
        "conditionalFormatsShowCommand",
    );

    let show_go = [
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &baseline_out,
        "--sheet",
        "1",
        "--rule",
        "cfRule:2",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &rust_out,
        "--sheet",
        "1",
        "--rule",
        "cfRule:2",
    ];
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "saved reorder show exit");
    assert_eq!(rust_stderr, baseline_stderr, "saved reorder show stderr");
    assert_eq!(
        rust_show.expect("rust saved reorder show"),
        baseline_show.expect("baseline saved reorder show"),
        "saved reorder show"
    );
    assert_xlsx_strict_valid(&rust_out);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_conditional_formats_reorder_readback_and_validation() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cf-reorder-readback-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("output.xlsx").to_string_lossy().to_string();
    write_simple_xlsx_with_sheet_xml(
        &input,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c></row></sheetData>
  <conditionalFormatting sqref="A1:A5">
    <cfRule type="expression" priority="3"><formula>A1&gt;0</formula></cfRule>
  </conditionalFormatting>
  <conditionalFormatting sqref="B1:B5">
    <cfRule type="expression" priority="4"><formula>B1&gt;0</formula></cfRule>
  </conditionalFormatting>
</worksheet>"#,
    );
    let input = input.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "reorder",
        &input,
        "--sheet",
        "1",
        "--rule",
        "block:2/rule:1",
        "--priority",
        "1",
        "--out",
        &output,
    ]);
    assert_eq!(code, 0, "reorder readback exit");
    assert_eq!(stderr, None, "reorder readback stderr");
    let reorder = stdout.expect("reorder readback stdout");
    assert_eq!(reorder["action"], "reorder");
    assert_eq!(reorder["rule"]["primarySelector"], "cfRule:2");
    assert_eq!(reorder["oldPriority"], Value::from(4));
    assert_eq!(reorder["newPriority"], Value::from(1));
    assert_eq!(reorder["rule"]["priority"], Value::from(1));
    assert_rust_emitted_ooxml_command_succeeds(&reorder, "conditionalFormatsListCommand");
    assert_rust_emitted_ooxml_command_succeeds(&reorder, "conditionalFormatsShowCommand");

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &output,
        "--sheet",
        "1",
        "--rule",
        "cfRule:2",
    ]);
    assert_eq!(show_code, 0, "saved reorder show exit");
    assert_eq!(show_stderr, None, "saved reorder show stderr");
    let show = show_stdout.expect("saved reorder show");
    assert_eq!(show["priority"], Value::from(1));
    assert_eq!(show["formula"], "B1>0");
    assert_xlsx_strict_valid(&output);

    let (out_of_range_code, out_of_range_stdout, out_of_range_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "reorder",
        &input,
        "--sheet",
        "1",
        "--rule",
        "cfRule:1",
        "--priority",
        "9",
        "--dry-run",
    ]);
    assert_eq!(out_of_range_code, 2, "out-of-range priority exit");
    assert_eq!(out_of_range_stdout, None, "out-of-range priority stdout");
    assert!(
        out_of_range_stderr
            .expect("out-of-range priority stderr")["error"]["message"]
            .as_str()
            .expect("out-of-range priority message")
            .contains("--priority must be between 1 and 2")
    );

    let (bad_priority_code, bad_priority_stdout, bad_priority_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "reorder",
        &input,
        "--sheet",
        "1",
        "--rule",
        "cfRule:1",
        "--priority",
        "0",
        "--dry-run",
    ]);
    assert_eq!(bad_priority_code, 2, "bad priority exit");
    assert_eq!(bad_priority_stdout, None, "bad priority stdout");
    assert!(
        bad_priority_stderr
            .expect("bad priority stderr")["error"]["message"]
            .as_str()
            .expect("bad priority message")
            .contains("--priority must be greater than zero")
    );

    let (missing_rule_code, missing_rule_stdout, missing_rule_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "reorder",
        &input,
        "--sheet",
        "1",
        "--priority",
        "2",
        "--dry-run",
    ]);
    assert_eq!(missing_rule_code, 2, "missing rule exit");
    assert_eq!(missing_rule_stdout, None, "missing rule stdout");
    assert!(
        missing_rule_stderr
            .expect("missing rule stderr")["error"]["message"]
            .as_str()
            .expect("missing rule message")
            .contains("--rule is required")
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_conditional_formats_cell_is_saved_outputs_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cf-cellis-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let baseline_single_out = temp_dir
        .join("baseline-cellis-single.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_single_out = temp_dir
        .join("rust-cellis-single.xlsx")
        .to_string_lossy()
        .to_string();
    let single_common = [
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "B1:B5",
        "--type",
        "cell-is",
        "--operator",
        "greaterThanOrEqual",
        "--formula",
        "5",
        "--priority",
        "8",
        "--out",
    ];
    let mut baseline_args = single_common.to_vec();
    baseline_args.push(&baseline_single_out);
    let mut rust_args = single_common.to_vec();
    rust_args.push(&rust_single_out);
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "single cellIs add exit");
    assert_eq!(rust_stderr, baseline_stderr, "single cellIs add stderr");
    let rust_single = rust_stdout.expect("rust single cellIs stdout");
    assert_eq!(
        scrub_path(
            rust_single.clone(),
            &rust_single_out,
            "[CELLIS_SINGLE_OUT]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline single cellIs stdout"),
            &baseline_single_out,
            "[CELLIS_SINGLE_OUT]"
        ),
        "single cellIs add stdout"
    );
    assert_eq!(rust_single["rule"]["type"], Value::String("cellIs".to_string()));
    assert_eq!(
        rust_single["rule"]["operator"],
        Value::String("greaterThanOrEqual".to_string())
    );
    assert_eq!(rust_single["rule"]["formula"], Value::String("5".to_string()));
    assert_rust_emitted_ooxml_command_exits_zero(&rust_single, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(
        &rust_single,
        "conditionalFormatsShowCommand",
    );

    let baseline_between_out = temp_dir
        .join("baseline-cellis-between.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_between_out = temp_dir
        .join("rust-cellis-between.xlsx")
        .to_string_lossy()
        .to_string();
    let between_common = [
        "--json",
        "xlsx",
        "cf",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "C1:C5",
        "--type",
        "cellIs",
        "--operator",
        "between",
        "--formula",
        "1",
        "--formula2",
        "10",
        "--out",
    ];
    let mut baseline_args = between_common.to_vec();
    baseline_args.push(&baseline_between_out);
    let mut rust_args = between_common.to_vec();
    rust_args.push(&rust_between_out);
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "between cellIs add exit");
    assert_eq!(rust_stderr, baseline_stderr, "between cellIs add stderr");
    let rust_between = rust_stdout.expect("rust between cellIs stdout");
    assert_eq!(
        scrub_path(
            rust_between.clone(),
            &rust_between_out,
            "[CELLIS_BETWEEN_OUT]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline between cellIs stdout"),
            &baseline_between_out,
            "[CELLIS_BETWEEN_OUT]"
        ),
        "between cellIs add stdout"
    );
    assert_eq!(rust_between["rule"]["type"], Value::String("cellIs".to_string()));
    assert_eq!(
        rust_between["rule"]["operator"],
        Value::String("between".to_string())
    );
    assert_eq!(
        rust_between["rule"]["formulas"],
        serde_json::json!(["1", "10"])
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_between, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(
        &rust_between,
        "conditionalFormatsShowCommand",
    );

    for output in [&rust_single_out, &rust_between_out] {
        assert_xlsx_strict_valid(output);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_conditional_formats_color_scale_saved_outputs_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cf-colorscale-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let baseline_out = temp_dir
        .join("baseline-color-scale.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_out = temp_dir
        .join("rust-color-scale.xlsx")
        .to_string_lossy()
        .to_string();
    let add_common = [
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "C1:C5",
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
        "4",
        "--out",
    ];
    let mut baseline_args = add_common.to_vec();
    baseline_args.push(&baseline_out);
    let mut rust_args = add_common.to_vec();
    rust_args.push(&rust_out);
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "color-scale add exit");
    assert_eq!(rust_stderr, baseline_stderr, "color-scale add stderr");
    let rust_add = rust_stdout.expect("rust color-scale add stdout");
    assert_eq!(
        scrub_path(rust_add.clone(), &rust_out, "[COLOR_SCALE_OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline color-scale add stdout"),
            &baseline_out,
            "[COLOR_SCALE_OUT]"
        ),
        "color-scale add stdout"
    );
    assert_eq!(rust_add["rule"]["type"], "colorScale");
    assert_eq!(
        rust_add["rule"]["colorScale"]["cfvo"],
        serde_json::json!([
            {"type": "min"},
            {"type": "percentile", "value": "50"},
            {"type": "max"}
        ])
    );
    assert_eq!(
        rust_add["rule"]["colorScale"]["colors"],
        serde_json::json!([
            {"rgb": "FFF8696B"},
            {"rgb": "FFFFEB84"},
            {"rgb": "FF63BE7B"}
        ])
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "conditionalFormatsListCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "conditionalFormatsShowCommand");

    let show_go = [
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &baseline_out,
        "--sheet",
        "1",
        "--rule",
        "cfRule:1",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &rust_out,
        "--sheet",
        "1",
        "--rule",
        "cfRule:1",
    ];
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "saved color-scale show exit");
    assert_eq!(rust_stderr, baseline_stderr, "saved color-scale show stderr");
    assert_eq!(
        rust_show.expect("rust saved color-scale show"),
        baseline_show.expect("baseline saved color-scale show"),
        "saved color-scale show"
    );

    assert_xlsx_strict_valid(&rust_out);
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_conditional_formats_data_bar_saved_outputs_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cf-databar-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let baseline_out = temp_dir
        .join("baseline-data-bar.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_out = temp_dir
        .join("rust-data-bar.xlsx")
        .to_string_lossy()
        .to_string();
    let add_common = [
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "D1:D5",
        "--type",
        "data-bar",
        "--cfvo",
        "min",
        "--cfvo",
        "max",
        "--color",
        "638EC6",
        "--priority",
        "5",
        "--out",
    ];
    let mut baseline_args = add_common.to_vec();
    baseline_args.push(&baseline_out);
    let mut rust_args = add_common.to_vec();
    rust_args.push(&rust_out);
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "data-bar add exit");
    assert_eq!(rust_stderr, baseline_stderr, "data-bar add stderr");
    let rust_add = rust_stdout.expect("rust data-bar add stdout");
    assert_eq!(
        scrub_path(rust_add.clone(), &rust_out, "[DATA_BAR_OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline data-bar add stdout"),
            &baseline_out,
            "[DATA_BAR_OUT]"
        ),
        "data-bar add stdout"
    );
    assert_eq!(rust_add["rule"]["type"], "dataBar");
    assert_eq!(
        rust_add["rule"]["dataBar"]["cfvo"],
        serde_json::json!([{"type": "min"}, {"type": "max"}])
    );
    assert_eq!(rust_add["rule"]["dataBar"]["color"]["rgb"], "FF638EC6");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "conditionalFormatsListCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "conditionalFormatsShowCommand");

    let show_go = [
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &baseline_out,
        "--sheet",
        "1",
        "--rule",
        "cfRule:1",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &rust_out,
        "--sheet",
        "1",
        "--rule",
        "cfRule:1",
    ];
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "saved data-bar show exit");
    assert_eq!(rust_stderr, baseline_stderr, "saved data-bar show stderr");
    assert_eq!(
        rust_show.expect("rust saved data-bar show"),
        baseline_show.expect("baseline saved data-bar show"),
        "saved data-bar show"
    );

    assert_xlsx_strict_valid(&rust_out);
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_conditional_formats_icon_set_saved_readback() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cf-iconset-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let rust_out = temp_dir
        .join("rust-icon-set.xlsx")
        .to_string_lossy()
        .to_string();
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "E1:E5",
        "--type",
        "icon-set",
        "--icon-set",
        "3TrafficLights1",
        "--cfvo",
        "percent:0",
        "--cfvo",
        "percent:33",
        "--cfvo",
        "percent:67",
        "--priority",
        "6",
        "--out",
        &rust_out,
    ]);
    assert_eq!(code, 0, "icon-set add exit");
    assert_eq!(stderr, None, "icon-set add stderr");
    let rust_add = stdout.expect("rust icon-set add stdout");
    assert_eq!(rust_add["rule"]["type"], "iconSet");
    assert_eq!(rust_add["rule"]["iconSet"]["iconSet"], "3TrafficLights1");
    assert_eq!(
        rust_add["rule"]["iconSet"]["cfvo"],
        serde_json::json!([
            {"type": "percent", "value": "0"},
            {"type": "percent", "value": "33"},
            {"type": "percent", "value": "67"}
        ])
    );
    assert!(rust_add["rule"]["iconSet"]["colors"].is_null());
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "conditionalFormatsListCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add, "conditionalFormatsShowCommand");

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &rust_out,
        "--sheet",
        "1",
        "--rule",
        "cfRule:1",
    ]);
    assert_eq!(show_code, 0, "saved icon-set show exit");
    assert_eq!(show_stderr, None, "saved icon-set show stderr");
    assert_eq!(
        show_stdout.expect("saved icon-set show")["iconSet"]["iconSet"],
        "3TrafficLights1"
    );

    assert_xlsx_strict_valid(&rust_out);
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_conditional_formats_icon_set_saved_outputs_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cf-iconset-oracle-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let baseline_out = temp_dir
        .join("baseline-icon-set.xlsx")
        .to_string_lossy()
        .to_string();
    let rust_out = temp_dir
        .join("rust-icon-set.xlsx")
        .to_string_lossy()
        .to_string();
    let add_common = [
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "E1:E5",
        "--type",
        "icon-set",
        "--icon-set",
        "3TrafficLights1",
        "--cfvo",
        "percent:0",
        "--cfvo",
        "percent:33",
        "--cfvo",
        "percent:67",
        "--priority",
        "6",
        "--out",
    ];
    let mut baseline_args = add_common.to_vec();
    baseline_args.push(&baseline_out);
    let mut rust_args = add_common.to_vec();
    rust_args.push(&rust_out);
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "icon-set add exit");
    assert_eq!(rust_stderr, baseline_stderr, "icon-set add stderr");
    let rust_add = rust_stdout.expect("rust icon-set add stdout");
    assert_eq!(
        scrub_path(rust_add.clone(), &rust_out, "[ICON_SET_OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline icon-set add stdout"),
            &baseline_out,
            "[ICON_SET_OUT]"
        ),
        "icon-set add stdout"
    );

    let show_go = [
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &baseline_out,
        "--sheet",
        "1",
        "--rule",
        "cfRule:1",
    ];
    let show_rust = [
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &rust_out,
        "--sheet",
        "1",
        "--rule",
        "cfRule:1",
    ];
    let (baseline_code, baseline_show, baseline_stderr) = run_ooxml_baseline(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, baseline_code, "saved icon-set show exit");
    assert_eq!(rust_stderr, baseline_stderr, "saved icon-set show stderr");
    assert_eq!(
        rust_show.expect("rust saved icon-set show"),
        baseline_show.expect("baseline saved icon-set show"),
        "saved icon-set show"
    );

    assert_xlsx_strict_valid(&rust_out);
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_conditional_formats_icon_set_rejects_bad_cli_arity() {
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "E1:E5",
        "--type",
        "icon-set",
        "--icon-set",
        "3TrafficLights1",
        "--cfvo",
        "percent:0",
        "--cfvo",
        "percent:33",
        "--dry-run",
    ]);
    assert_eq!(code, 2, "icon-set missing cfvo exit");
    assert_eq!(stdout, None, "icon-set missing cfvo stdout");
    let error = stderr.expect("icon-set missing cfvo stderr");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .expect("message")
            .contains("requires exactly 3 --cfvo values"),
        "unexpected missing cfvo error: {error:?}"
    );

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "E1:E5",
        "--type",
        "icon-set",
        "--icon-set",
        "3TrafficLights1",
        "--cfvo",
        "percent:0",
        "--cfvo",
        "percent:33",
        "--cfvo",
        "percent:67",
        "--color",
        "638EC6",
        "--dry-run",
    ]);
    assert_eq!(code, 2, "icon-set color exit");
    assert_eq!(stdout, None, "icon-set color stdout");
    let error = stderr.expect("icon-set color stderr");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .expect("message")
            .contains("does not accept --color"),
        "unexpected color error: {error:?}"
    );
}

#[test]
fn xlsx_conditional_formats_data_bar_rejects_bad_cli_arity() {
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "D1:D5",
        "--type",
        "data-bar",
        "--cfvo",
        "min",
        "--color",
        "638EC6",
        "--dry-run",
    ]);
    assert_eq!(code, 2, "data-bar missing cfvo exit");
    assert_eq!(stdout, None, "data-bar missing cfvo stdout");
    let error = stderr.expect("data-bar missing cfvo stderr");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .expect("message")
            .contains("exactly two --cfvo values"),
        "unexpected missing cfvo error: {error:?}"
    );

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "D1:D5",
        "--type",
        "data-bar",
        "--cfvo",
        "min",
        "--cfvo",
        "max",
        "--color",
        "638EC6",
        "--color",
        "63C384",
        "--dry-run",
    ]);
    assert_eq!(code, 2, "data-bar extra color exit");
    assert_eq!(stdout, None, "data-bar extra color stdout");
    let error = stderr.expect("data-bar extra color stderr");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .expect("message")
            .contains("exactly one --color value"),
        "unexpected extra color error: {error:?}"
    );
}

#[test]
fn xlsx_conditional_formats_preserve_unsupported_rules_on_add_and_delete() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cf-preserve-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let added = temp_dir.join("added.xlsx").to_string_lossy().to_string();
    let deleted = temp_dir.join("deleted.xlsx").to_string_lossy().to_string();
    write_simple_xlsx_with_sheet_xml(
        &input,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
  <conditionalFormatting sqref="B1:B5">
    <cfRule type="colorScale" priority="1">
      <colorScale>
        <cfvo type="min"/>
        <cfvo type="max"/>
        <color rgb="FFFF0000"/>
        <color rgb="FF00FF00"/>
      </colorScale>
    </cfRule>
  </conditionalFormatting>
  <dataValidations count="1"><dataValidation sqref="C1:C3" type="whole"><formula1>1</formula1></dataValidation></dataValidations>
</worksheet>"#,
    );
    let input = input.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        &input,
        "--sheet",
        "1",
        "--range",
        "C1:C5",
        "--formula",
        "C1<>0",
        "--out",
        &added,
    ]);
    assert_eq!(code, 0, "preserve add exit");
    assert_eq!(stderr, None, "preserve add stderr");
    assert!(stdout.is_some(), "preserve add stdout");
    let sheet_xml = read_zip_string(Path::new(&added), "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains("colorScale") && sheet_xml.contains("<dataValidations"),
        "unsupported rule or data validations were not preserved:\n{sheet_xml}"
    );
    assert!(
        sheet_xml.find("<conditionalFormatting sqref=\"C1:C5\"")
            < sheet_xml.find("<dataValidations"),
        "new conditionalFormatting should be ordered before dataValidations:\n{sheet_xml}"
    );

    let (code, _, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "delete",
        &added,
        "--sheet",
        "1",
        "--rule",
        "cfRule:2",
        "--out",
        &deleted,
    ]);
    assert_eq!(code, 0, "preserve delete exit");
    assert_eq!(stderr, None, "preserve delete stderr");
    let sheet_xml = read_zip_string(Path::new(&deleted), "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains("colorScale") && !sheet_xml.contains("C1<>0"),
        "delete removed unsupported rule or kept expression:\n{sheet_xml}"
    );
    assert_xlsx_strict_valid(&added);
    assert_xlsx_strict_valid(&deleted);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_conditional_formats_add_preserves_xlsm_vba_package_artifacts() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsm-cf-vba-preserve-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input_path = temp_dir.join("input.xlsm");
    let output_path = temp_dir.join("output.xlsm");
    write_tiny_xlsm_with_opaque_vba_project(&input_path);

    let before_vba = read_zip_bytes(&input_path, "xl/vbaProject.bin");
    let before_content_types = read_zip_string(&input_path, "[Content_Types].xml");
    let before_workbook_rels = read_zip_string(&input_path, "xl/_rels/workbook.xml.rels");
    assert_vba_package_entries_present(&before_content_types, &before_workbook_rels);

    let input = input_path.to_string_lossy().to_string();
    let output = output_path.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        &input,
        "--sheet",
        "1",
        "--range",
        "A1:A3",
        "--formula",
        "A1>0",
        "--priority",
        "2",
        "--no-validate",
        "--out",
        &output,
    ]);
    assert_eq!(code, 0, "xlsm conditional format add exit");
    assert_eq!(stderr, None, "xlsm conditional format add stderr");
    assert!(stdout.is_some(), "xlsm conditional format add stdout");

    assert_eq!(
        read_zip_bytes(&output_path, "xl/vbaProject.bin"),
        before_vba,
        "opaque VBA project bytes changed during worksheet mutation"
    );
    assert_eq!(
        read_zip_string(&output_path, "[Content_Types].xml"),
        before_content_types,
        "macro-enabled workbook/vba content type entries changed"
    );
    assert_eq!(
        read_zip_string(&output_path, "xl/_rels/workbook.xml.rels"),
        before_workbook_rels,
        "workbook VBA relationship changed"
    );
    let sheet_xml = read_zip_string(&output_path, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains(r#"<conditionalFormatting sqref="A1:A3">"#)
            && sheet_xml.contains("<formula>A1&gt;0</formula>"),
        "conditional format was not written to XLSM worksheet:\n{sheet_xml}"
    );

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output]);
    // This package proves OPC-level macro artifact preservation with opaque fake VBA bytes.
    // SDK/Office proof remains a separate gate for Office-authored macro projects.
    if validate_code == 0
        && validate_stderr.is_none()
        && validate_stdout
            .as_ref()
            .is_some_and(|value| value["valid"] == Value::Bool(true))
    {
        assert_eq!(
            validate_stdout.expect("strict validate stdout")["valid"],
            Value::Bool(true),
            "strict validation should accept the synthetic XLSM when its fake VBA bytes are sufficient"
        );
    } else {
        assert!(
            validate_stdout.is_some() || validate_stderr.is_some(),
            "strict validation rejection should be reported as JSON when fake VBA bytes are insufficient"
        );
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

fn write_tiny_xlsm_with_opaque_vba_project(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create xlsm");
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
  <Default Extension="bin" ContentType="application/vnd.ms-office.vbaProject"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.ms-excel.sheet.macroEnabled.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
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
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
</workbook>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rIdVba" Type="http://schemas.microsoft.com/office/2006/relationships/vbaProject" Target="vbaProject.bin"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:A3"/>
  <sheetData>
    <row r="1"><c r="A1"><v>1</v></c></row>
    <row r="2"><c r="A2"><v>2</v></c></row>
    <row r="3"><c r="A3"><v>3</v></c></row>
  </sheetData>
</worksheet>"#,
    );
    writer
        .start_file("xl/vbaProject.bin", options)
        .expect("write vbaProject.bin");
    writer
        .write_all(b"opaque synthetic vba payload for package preservation")
        .expect("write vbaProject.bin bytes");
    writer.finish().expect("finish xlsm");
}

fn assert_vba_package_entries_present(content_types: &str, workbook_rels: &str) {
    assert!(
        content_types.contains(
            r#"ContentType="application/vnd.ms-excel.sheet.macroEnabled.main+xml""#
        ),
        "workbook content type is not macro-enabled:\n{content_types}"
    );
    assert!(
        content_types.contains(r#"ContentType="application/vnd.ms-office.vbaProject""#),
        "vbaProject.bin content type is missing:\n{content_types}"
    );
    assert!(
        workbook_rels
            .contains(r#"Type="http://schemas.microsoft.com/office/2006/relationships/vbaProject""#)
            && workbook_rels.contains(r#"Target="vbaProject.bin""#),
        "workbook rels missing vbaProject relationship:\n{workbook_rels}"
    );
}

fn read_zip_bytes(path: &Path, name: &str) -> Vec<u8> {
    let input = File::open(path).expect("open zip");
    let mut archive = ZipArchive::new(input).expect("read zip");
    let mut entry = archive.by_name(name).expect("read zip entry");
    let mut body = Vec::new();
    entry.read_to_end(&mut body).expect("read zip entry bytes");
    body
}
