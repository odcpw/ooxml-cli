#[test]
fn serve_inspect_supports_xlsx_cells_extract() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-cells-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(1, "open", serde_json::json!({"file": input_str}));
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let inspect = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx cells extract",
            "args": {"sheet": "1", "range": "B1:D2", "includeEmpty": true, "maxRows": 2},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    let working = inspect_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "cells",
        "extract",
        working,
        "--sheet",
        "1",
        "--range",
        "B1:D2",
        "--include-empty",
        "--max-rows",
        "2",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(
        inspect_response["result"],
        expected.expect("extract stdout")
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_inspect_supports_xlsx_sheets_show() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-sheets-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    std::fs::copy("testdata/xlsx/used-range/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(1, "open", serde_json::json!({"file": input_str}));
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let inspect = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx sheets show",
            "args": {"sheet": "sheetId:1"},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    let working = inspect_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "sheets",
        "show",
        working,
        "--sheet",
        "sheetId:1",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(inspect_response["result"], expected.expect("show stdout"));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_inspect_supports_xlsx_filters_sorts_show() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-filters-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("filters.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &input,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>10</v></c></row></sheetData>
  <autoFilter ref="A1:B2"><filterColumn colId="0"><filters><filter val="North"/></filters></filterColumn></autoFilter>
</worksheet>"#,
    );
    let input_str = input.to_str().expect("input path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(1, "open", serde_json::json!({"file": input_str}));
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let inspect = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx filters-sorts show",
            "args": {"sheet": "1"},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    let working = inspect_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "filters-sorts",
        "show",
        working,
        "--sheet",
        "1",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(inspect_response["result"], expected.expect("show stdout"));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_inspect_supports_xlsx_names() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-names-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("defined-names.xlsx");
    write_defined_names_xlsx(&input);
    let input_str = input.to_str().expect("input path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(1, "open", serde_json::json!({"file": input_str}));
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let list = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx names list",
            "args": {"scopeSheet": "Data"},
        }),
    );
    let list_response = serve_roundtrip(&mut stdin, &mut reader, &list);
    let working = list_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "names",
        "list",
        working,
        "--scope-sheet",
        "Data",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(
        list_response["result"],
        expected.expect("names list stdout")
    );

    let show = rpc_request(
        3,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx names show",
            "args": {"name": "LocalData", "scopeSheet": "Data"},
        }),
    );
    let show_response = serve_roundtrip(&mut stdin, &mut reader, &show);
    let (show_code, show_expected, show_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "names",
        "show",
        working,
        "--name",
        "LocalData",
        "--scope-sheet",
        "Data",
    ]);
    assert_eq!(show_code, 0);
    assert_eq!(show_stderr, None);
    assert_eq!(
        show_response["result"],
        show_expected.expect("names show stdout")
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_inspect_supports_xlsx_tables_show() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-tables-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("table-workbook.xlsx");
    write_table_xlsx(&input);
    let input_str = input.to_str().expect("input path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(1, "open", serde_json::json!({"file": input_str}));
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let list = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables list",
            "args": {"sheet": "Data"},
        }),
    );
    let list_response = serve_roundtrip(&mut stdin, &mut reader, &list);
    let working = list_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json", "xlsx", "tables", "list", working, "--sheet", "Data",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(list_response["result"], expected.expect("list stdout"));

    let inspect = rpc_request(
        3,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables show",
            "args": {"sheet": "Data", "table": "Sales"},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    let working = inspect_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json", "xlsx", "tables", "show", working, "--sheet", "Data", "--table", "Sales",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(inspect_response["result"], expected.expect("show stdout"));

    let export = rpc_request(
        4,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables export",
            "args": {"sheet": "Data", "table": "Sales", "includeTypes": true, "includeFormulas": true},
        }),
    );
    let export_response = serve_roundtrip(&mut stdin, &mut reader, &export);
    let working = export_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "tables",
        "export",
        working,
        "--sheet",
        "Data",
        "--table",
        "Sales",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(export_response["result"], expected.expect("export stdout"));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_inspect_supports_xlsx_conditional_formats() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-cf-inspect-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("conditional-formats.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &input,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C5"/>
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
  <conditionalFormatting sqref="A1:A5">
    <cfRule type="expression" priority="3" stopIfTrue="1"><formula>A1&gt;0</formula></cfRule>
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
    let input_str = input.to_str().expect("input path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(1, "open", serde_json::json!({"file": input_str}));
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let list = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx cf list",
            "args": {"sheet": "1", "range": "B1:B5"},
        }),
    );
    let list_response = serve_roundtrip(&mut stdin, &mut reader, &list);
    let working = list_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (list_code, list_expected, list_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "list",
        working,
        "--sheet",
        "1",
        "--range",
        "B1:B5",
    ]);
    assert_eq!(list_code, 0);
    assert_eq!(list_stderr, None);
    assert_eq!(list_response["result"], list_expected.expect("list stdout"));

    let show = rpc_request(
        3,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx conditional-formatting show",
            "args": {"sheet": "1", "rule": "block:2/rule:1"},
        }),
    );
    let show_response = serve_roundtrip(&mut stdin, &mut reader, &show);
    let (show_code, show_expected, show_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        working,
        "--sheet",
        "1",
        "--rule",
        "block:2/rule:1",
    ]);
    assert_eq!(show_code, 0);
    assert_eq!(show_stderr, None);
    assert_eq!(show_response["result"], show_expected.expect("show stdout"));
    assert_eq!(
        show_response["result"]["colorScale"]["colors"][1]["rgb"],
        "FF00FF00"
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_xlsx_conditional_formats_add_delete() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-cf-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-cf-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(
        1,
        "open",
        serde_json::json!({"file": input_str, "out": output_str}),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let add_cell_is = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx conditional-formats add",
            "args": {
                "sheet": "1",
                "range": "A1:A5",
                "type": "cell-is",
                "operator": "between",
                "formula": "1",
                "formula2": "10",
                "priority": 4
            },
        }),
    );
    let add_cell_is_response = serve_roundtrip(&mut stdin, &mut reader, &add_cell_is);
    assert!(
        add_cell_is_response.get("error").is_none(),
        "conditional-format cellIs add op failed: {add_cell_is_response:?}"
    );
    let cell_is_readback = &add_cell_is_response["result"]["readback"];
    assert_eq!(cell_is_readback["rule"]["type"], "cellIs");
    assert_eq!(cell_is_readback["rule"]["operator"], "between");
    assert_eq!(cell_is_readback["rule"]["formulas"], serde_json::json!(["1", "10"]));

    let add_expression = rpc_request(
        3,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx cf add",
            "args": {
                "sheet": "1",
                "range": "B1:B5",
                "formula": "B1>0",
                "priority": 5,
                "stopIfTrue": true
            },
        }),
    );
    let add_expression_response = serve_roundtrip(&mut stdin, &mut reader, &add_expression);
    assert!(
        add_expression_response.get("error").is_none(),
        "conditional-format expression add op failed: {add_expression_response:?}"
    );
    assert_eq!(
        add_expression_response["result"]["readback"]["rule"]["type"],
        "expression"
    );
    assert_eq!(
        add_expression_response["result"]["readback"]["rule"]["formula"],
        "B1>0"
    );

    let add_color_scale = rpc_request(
        4,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx conditional-formats add",
            "args": {
                "sheet": "1",
                "range": "C1:C5",
                "type": "color-scale",
                "cfvo": ["min", "percentile:50", "max"],
                "color": ["F8696B", "FFEB84", "63BE7B"],
                "priority": 6
            },
        }),
    );
    let add_color_scale_response = serve_roundtrip(&mut stdin, &mut reader, &add_color_scale);
    assert!(
        add_color_scale_response.get("error").is_none(),
        "conditional-format color-scale add op failed: {add_color_scale_response:?}"
    );
    let color_scale_readback = &add_color_scale_response["result"]["readback"]["rule"];
    assert_eq!(color_scale_readback["type"], "colorScale");
    assert_eq!(
        color_scale_readback["colorScale"]["cfvo"],
        serde_json::json!([
            {"type": "min"},
            {"type": "percentile", "value": "50"},
            {"type": "max"}
        ])
    );
    assert_eq!(
        color_scale_readback["colorScale"]["colors"][2]["rgb"],
        "FF63BE7B"
    );

    let delete_expression = rpc_request(
        5,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx conditional-formats delete",
            "args": {"sheet": "1", "rule": "priority:5"},
        }),
    );
    let delete_response = serve_roundtrip(&mut stdin, &mut reader, &delete_expression);
    assert!(
        delete_response.get("error").is_none(),
        "conditional-format delete op failed: {delete_response:?}"
    );
    assert_eq!(delete_response["result"]["readback"]["action"], "delete");
    assert_eq!(delete_response["result"]["readback"]["rule"]["priority"], 5);

    let plan = rpc_request(6, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    let plan_items = plan_response["result"]["plan"]
        .as_array()
        .expect("planned operations");
    assert_eq!(plan_items.len(), 4);
    assert_eq!(plan_items[0]["argv"][1], "conditional-formats");
    assert_eq!(plan_items[0]["argv"][2], "add");
    assert_eq!(plan_items[1]["argv"][2], "add");
    assert_eq!(plan_items[2]["argv"][2], "add");
    assert_eq!(plan_items[3]["argv"][2], "delete");
    assert!(
        plan_items[0]["argv"]
            .as_array()
            .expect("add plan argv")
            .iter()
            .any(|arg| arg == "--formula2")
    );
    let color_scale_plan = plan_items[2]["argv"].as_array().expect("color-scale plan argv");
    assert!(
        color_scale_plan.iter().any(|arg| arg == "--cfvo")
            && color_scale_plan.iter().any(|arg| arg == "--color"),
        "color-scale plan should include repeated threshold/color flags: {color_scale_plan:?}"
    );

    let commit = rpc_request(7, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "conditional-format commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["file"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "conditional formats serve validate exit");
    assert_eq!(
        validate_stderr, None,
        "conditional formats serve validate stderr"
    );

    let (list_code, list_stdout, list_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "list",
        &output_str,
        "--sheet",
        "1",
    ]);
    assert_eq!(list_code, 0, "conditional formats serve list exit");
    assert_eq!(list_stderr, None, "conditional formats serve list stderr");
    let list = list_stdout.expect("conditional formats serve output list");
    assert_eq!(list["count"], Value::from(2));
    assert_eq!(list["rules"][0]["type"], "cellIs");
    assert_eq!(list["rules"][0]["operator"], "between");
    assert_eq!(list["rules"][0]["formulas"], serde_json::json!(["1", "10"]));
    assert_eq!(list["rules"][1]["type"], "colorScale");
    assert_eq!(
        list["rules"][1]["colorScale"]["colors"][2]["rgb"],
        "FF63BE7B"
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_xlsx_conditional_format_data_bars() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-cf-databar-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-cf-databar-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(
        1,
        "open",
        serde_json::json!({"file": input_str, "out": output_str}),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let add_string_color = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx conditional-formats add",
            "args": {
                "sheet": "1",
                "range": "D1:D5",
                "type": "data-bar",
                "cfvo": ["min", "max"],
                "color": "638EC6",
                "priority": 7
            },
        }),
    );
    let add_string_response = serve_roundtrip(&mut stdin, &mut reader, &add_string_color);
    assert!(
        add_string_response.get("error").is_none(),
        "conditional-format data-bar string color add op failed: {add_string_response:?}"
    );
    let string_rule = &add_string_response["result"]["readback"]["rule"];
    assert_eq!(string_rule["type"], "dataBar");
    assert_eq!(
        string_rule["dataBar"]["cfvo"],
        serde_json::json!([{"type": "min"}, {"type": "max"}])
    );
    assert_eq!(string_rule["dataBar"]["color"]["rgb"], "FF638EC6");

    let add_array_color = rpc_request(
        3,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx conditional-formats add",
            "args": {
                "sheet": "1",
                "range": "E1:E5",
                "type": "data-bar",
                "cfvo": ["min", "max"],
                "color": ["63C384"],
                "priority": 8
            },
        }),
    );
    let add_array_response = serve_roundtrip(&mut stdin, &mut reader, &add_array_color);
    assert!(
        add_array_response.get("error").is_none(),
        "conditional-format data-bar array color add op failed: {add_array_response:?}"
    );
    let array_rule = &add_array_response["result"]["readback"]["rule"];
    assert_eq!(array_rule["type"], "dataBar");
    assert_eq!(array_rule["dataBar"]["color"]["rgb"], "FF63C384");

    let plan = rpc_request(4, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    let plan_items = plan_response["result"]["plan"]
        .as_array()
        .expect("planned operations");
    assert_eq!(plan_items.len(), 2);
    for plan_item in plan_items {
        let argv = plan_item["argv"].as_array().expect("data-bar plan argv");
        assert_eq!(
            argv.iter().filter(|arg| *arg == "--cfvo").count(),
            2,
            "data-bar plan should include two threshold flags: {argv:?}"
        );
        assert_eq!(
            argv.iter().filter(|arg| *arg == "--color").count(),
            1,
            "data-bar plan should include one color flag: {argv:?}"
        );
    }

    let commit = rpc_request(5, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "conditional-format data-bar commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "data-bar serve validate exit");
    assert_eq!(validate_stderr, None, "data-bar serve validate stderr");

    let (list_code, list_stdout, list_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "conditional-formats",
        "list",
        &output_str,
        "--sheet",
        "1",
    ]);
    assert_eq!(list_code, 0, "data-bar serve list exit");
    assert_eq!(list_stderr, None, "data-bar serve list stderr");
    let list = list_stdout.expect("data-bar serve output list");
    assert_eq!(list["count"], Value::from(2));
    assert_eq!(list["rules"][0]["type"], "dataBar");
    assert_eq!(list["rules"][0]["dataBar"]["color"]["rgb"], "FF638EC6");
    assert_eq!(list["rules"][1]["type"], "dataBar");
    assert_eq!(list["rules"][1]["dataBar"]["color"]["rgb"], "FF63C384");

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_xlsx_ranges_set() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-ranges-set-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-ranges-set-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(
        1,
        "open",
        serde_json::json!({"file": input_str, "out": output_str}),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx ranges set",
            "args": {
                "sheet": "Sheet1",
                "range": "A1:C2",
                "values": [
                    ["Agent", {"value": "12.5", "type": "number"}, {"formula": "SUM(B1:B1)"}],
                    ["Flag", true, "tail"]
                ]
            },
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    assert!(
        op_response.get("error").is_none(),
        "ranges set op failed: {op_response:?}"
    );
    let readback = &op_response["result"]["readback"];
    assert_eq!(readback["range"], Value::String("A1:C2".to_string()));
    assert_eq!(readback["updated"], Value::from(6));
    assert_eq!(readback["created"], Value::from(3));
    assert_eq!(readback["formulaCount"], Value::from(1));
    assert_eq!(
        readback["destination"]["values"][0][0],
        Value::String("Agent".to_string())
    );
    assert_eq!(
        readback["destination"]["values"][0][1],
        serde_json::json!(12.5)
    );
    assert_eq!(readback["destination"]["values"][1][1], Value::Bool(true));
    assert_eq!(
        readback["destination"]["formulas"][0][2],
        Value::String("SUM(B1:B1)".to_string())
    );

    let plan = rpc_request(3, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("set".to_string())
    );

    let commit = rpc_request(4, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_xlsx_full_calc_flags(&output);
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["file"],
        Value::String(output_str.clone())
    );
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["output"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "ranges set serve output validate exit");
    assert_eq!(
        validate_stderr, None,
        "ranges set serve output validate stderr"
    );

    let (export_code, export_stdout, export_stderr) = run_go_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        &output_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C2",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(export_code, 0, "Go export readback exit");
    assert_eq!(export_stderr, None, "Go export readback stderr");
    let export = export_stdout.expect("Go export readback");
    assert_eq!(export["values"][0][0], Value::String("Agent".to_string()));
    assert_eq!(export["values"][0][1], serde_json::json!(12.5));
    assert_eq!(export["values"][0][2], Value::Null);
    assert_eq!(
        export["formulas"][0][2],
        Value::String("SUM(B1:B1)".to_string())
    );
    assert_eq!(export["values"][1][0], Value::String("Flag".to_string()));
    assert_eq!(export["values"][1][1], Value::Bool(true));
    assert_eq!(export["values"][1][2], Value::String("tail".to_string()));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}


#[test]
fn serve_op_supports_xlsx_dimension_setters() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-dimensions-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-dimensions-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(
        1,
        "open",
        serde_json::json!({"file": input_str, "out": output_str}),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let col_op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx colwidths set",
            "args": {"sheet": "Sheet1", "range": "B:C", "width": 19.5},
        }),
    );
    let col_response = serve_roundtrip(&mut stdin, &mut reader, &col_op);
    assert!(
        col_response.get("error").is_none(),
        "colwidths set op failed: {col_response:?}"
    );
    let col_readback = &col_response["result"]["readback"];
    assert_eq!(col_readback["range"], Value::String("B:C".to_string()));
    assert_eq!(col_readback["columns"], Value::from(2));
    assert_eq!(col_readback["width"], serde_json::json!(19.5));

    let row_op = rpc_request(
        3,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx rowheights set",
            "args": {"sheet": "Sheet1", "range": "2:3", "height": "23.75"},
        }),
    );
    let row_response = serve_roundtrip(&mut stdin, &mut reader, &row_op);
    assert!(
        row_response.get("error").is_none(),
        "rowheights set op failed: {row_response:?}"
    );
    let row_readback = &row_response["result"]["readback"];
    assert_eq!(row_readback["range"], Value::String("2:3".to_string()));
    assert_eq!(row_readback["rows"], Value::from(2));
    assert_eq!(row_readback["created"], Value::from(2));
    assert_eq!(row_readback["height"], serde_json::json!(23.75));

    let plan = rpc_request(4, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][1],
        Value::String("colwidths".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("set".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][1]["argv"][1],
        Value::String("rowheights".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][1]["argv"][2],
        Value::String("set".to_string())
    );

    let commit = rpc_request(5, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "dimension commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["file"],
        Value::String(output_str.clone())
    );
    assert_eq!(
        commit_response["result"]["applied"][1]["readback"]["file"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "dimension serve output validate exit");
    assert_eq!(
        validate_stderr, None,
        "dimension serve output validate stderr"
    );

    let (col_code, col_stdout, col_stderr) = run_go_ooxml(&[
        "--json",
        "xlsx",
        "colwidths",
        "show",
        &output_str,
        "--sheet",
        "Sheet1",
        "--range",
        "B:C",
    ]);
    assert_eq!(col_code, 0, "Go colwidths readback exit");
    assert_eq!(col_stderr, None, "Go colwidths readback stderr");
    let col_show = col_stdout.expect("Go colwidths readback");
    assert_eq!(col_show["columns"]["B"]["width"], serde_json::json!(19.5));
    assert_eq!(col_show["columns"]["C"]["width"], serde_json::json!(19.5));
    assert_eq!(col_show["columns"]["B"]["custom"], Value::Bool(true));

    let (row_code, row_stdout, row_stderr) = run_go_ooxml(&[
        "--json",
        "xlsx",
        "rowheights",
        "show",
        &output_str,
        "--sheet",
        "Sheet1",
        "--range",
        "2:3",
    ]);
    assert_eq!(row_code, 0, "Go rowheights readback exit");
    assert_eq!(row_stderr, None, "Go rowheights readback stderr");
    let row_show = row_stdout.expect("Go rowheights readback");
    assert_eq!(row_show["rows"]["2"]["height"], serde_json::json!(23.75));
    assert_eq!(row_show["rows"]["3"]["height"], serde_json::json!(23.75));
    assert_eq!(row_show["rows"]["2"]["custom"], Value::Bool(true));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_xlsx_ranges_set_format() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-format-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-format-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(
        1,
        "open",
        serde_json::json!({"file": input_str, "out": output_str}),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx ranges set-format",
            "args": {"sheet": "Sheet1", "range": "A1", "preset": "currency"},
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    assert!(
        op_response.get("error").is_none(),
        "set-format op failed: {op_response:?}"
    );
    let readback = &op_response["result"]["readback"];
    assert_eq!(readback["range"], Value::String("A1".to_string()));
    assert_eq!(readback["preset"], Value::String("currency".to_string()));
    assert_eq!(
        readback["formatCode"],
        Value::String("\"$\"#,##0.00".to_string())
    );
    assert_eq!(readback["updated"], Value::from(1));
    assert_eq!(
        readback["destination"]["numberFormatCodes"][0][0],
        Value::String("\"$\"#,##0.00".to_string())
    );

    let plan = rpc_request(3, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("set-format".to_string())
    );

    let commit = rpc_request(4, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["file"],
        Value::String(output_str.clone())
    );
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["output"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "set-format serve output validate exit");
    assert_eq!(
        validate_stderr, None,
        "set-format serve output validate stderr"
    );

    let (export_code, export_stdout, export_stderr) = run_go_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        &output_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ]);
    assert_eq!(export_code, 0, "Go export readback exit");
    assert_eq!(export_stderr, None, "Go export readback stderr");
    assert_eq!(
        export_stdout.expect("Go export readback")["numberFormatCodes"][0][0],
        Value::String("\"$\"#,##0.00".to_string())
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

include!("xlsx_charts.rs");

#[test]
fn serve_op_supports_xlsx_comments_add_update_remove() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-xlsx-comments-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-comments-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(
        1,
        "open",
        serde_json::json!({"file": input_str, "out": output_str}),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let add = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx comments add",
            "args": {
                "sheet": "Sheet1",
                "cell": "B2",
                "author": "Serve Agent",
                "text": "Serve note"
            },
        }),
    );
    let add_response = serve_roundtrip(&mut stdin, &mut reader, &add);
    assert!(
        add_response.get("error").is_none(),
        "comments add op failed: {add_response:?}"
    );
    let add_readback = &add_response["result"]["readback"];
    assert_eq!(add_readback["commentId"], Value::from(0));
    assert_eq!(add_readback["handle"], "H:xlsx/ws:1/comment:a:B2");
    assert_eq!(add_readback["text"], "Serve note");
    let working = add_readback["file"]
        .as_str()
        .expect("working package")
        .to_string();
    let add_hash = add_readback["contentHash"]
        .as_str()
        .expect("add hash")
        .to_string();

    let inspect = rpc_request(
        3,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx comments list",
            "args": {"sheet": "Sheet1", "commentId": 0},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    assert!(
        inspect_response.get("error").is_none(),
        "comments inspect failed: {inspect_response:?}"
    );
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "comments",
        "list",
        &working,
        "--sheet",
        "Sheet1",
        "--comment-id",
        "0",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(inspect_response["result"], expected.expect("list stdout"));

    let update = rpc_request(
        4,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx comments update",
            "args": {
                "handle": "H:xlsx/ws:1/comment:a:B2",
                "author": "Reviewer",
                "text": "Serve updated",
                "expectHash": add_hash
            },
        }),
    );
    let update_response = serve_roundtrip(&mut stdin, &mut reader, &update);
    assert!(
        update_response.get("error").is_none(),
        "comments update op failed: {update_response:?}"
    );
    let update_readback = &update_response["result"]["readback"];
    assert_eq!(update_readback["previousText"], "Serve note");
    assert_eq!(update_readback["author"], "Reviewer");
    let updated_hash = update_readback["contentHash"]
        .as_str()
        .expect("updated hash")
        .to_string();

    let remove = rpc_request(
        5,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx comments remove",
            "args": {"commentId": 0, "expectHash": updated_hash},
        }),
    );
    let remove_response = serve_roundtrip(&mut stdin, &mut reader, &remove);
    assert!(
        remove_response.get("error").is_none(),
        "comments remove op failed: {remove_response:?}"
    );
    assert_eq!(
        remove_response["result"]["readback"]["previousText"],
        "Serve updated"
    );

    let plan = rpc_request(6, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    let plan_items = plan_response["result"]["plan"]
        .as_array()
        .expect("planned operations");
    let verbs = plan_items
        .iter()
        .map(|item| item["argv"][2].as_str().expect("plan verb").to_string())
        .collect::<Vec<_>>();
    assert_eq!(verbs, ["add", "update", "remove"]);

    let commit = rpc_request(7, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_eq!(
        commit_response["result"]["applied"][2]["readback"]["output"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "comments serve output validate exit");
    assert_eq!(
        validate_stderr, None,
        "comments serve output validate stderr"
    );
    let (list_code, list_stdout, list_stderr) =
        run_ooxml(&["--json", "xlsx", "comments", "list", &output_str]);
    assert_eq!(list_code, 0, "comments serve output list exit");
    assert_eq!(list_stderr, None, "comments serve output list stderr");
    assert_eq!(
        list_stdout.expect("serve output comments list")["comments"],
        Value::Array(Vec::new())
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_xlsx_tables_append_rows() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-table-append-rows-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-table-append-rows-out.xlsx");
    write_table_xlsx(&input);
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(
        1,
        "open",
        serde_json::json!({"file": input_str, "out": output_str}),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables append-rows",
            "args": {
                "table": "Sales",
                "values": [
                    ["North", 30],
                    ["South", 40]
                ]
            },
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    assert!(
        op_response.get("error").is_none(),
        "append-rows op failed: {op_response:?}"
    );
    let readback = &op_response["result"]["readback"];
    assert_eq!(readback["rowsAppended"], Value::from(2));
    assert_eq!(
        readback["previousRange"],
        Value::String("A1:B3".to_string())
    );
    assert_eq!(readback["range"], Value::String("A1:B5".to_string()));
    assert_eq!(
        readback["destination"]["appended"]["values"][0][0],
        Value::String("North".to_string())
    );
    assert_eq!(
        readback["destination"]["appended"]["values"][1][1],
        serde_json::json!(40)
    );

    let plan = rpc_request(3, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    let argv = plan_response["result"]["plan"][0]["argv"]
        .as_array()
        .expect("planned argv");
    assert_eq!(argv[2], Value::String("append-rows".to_string()));
    assert!(
        argv.iter()
            .any(|arg| arg == &Value::String("--values".to_string()))
    );

    let commit = rpc_request(4, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["file"],
        Value::String(output_str.clone())
    );
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["output"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "append-rows serve output validate exit");
    assert_eq!(
        validate_stderr, None,
        "append-rows serve output validate stderr"
    );

    let (export_code, export_stdout, export_stderr) = run_go_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        &output_str,
        "--sheet",
        "Data",
        "--range",
        "A4:B5",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(export_code, 0, "Go export readback exit");
    assert_eq!(export_stderr, None, "Go export readback stderr");
    let export = export_stdout.expect("Go export readback");
    assert_eq!(export["values"][0][0], Value::String("North".to_string()));
    assert_eq!(export["values"][0][1], serde_json::json!(30));
    assert_eq!(export["values"][1][0], Value::String("South".to_string()));
    assert_eq!(export["values"][1][1], serde_json::json!(40));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_xlsx_tables_append_records() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-table-append-records-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-table-append-records-out.xlsx");
    write_table_xlsx(&input);
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(
        1,
        "open",
        serde_json::json!({"file": input_str, "out": output_str}),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables append-records",
            "args": {
                "table": "Sales",
                "expectRange": "A1:B3",
                "records": [
                    {"Region": "North", "Amount": 30},
                    {"Region": "South", "Amount": {"value": "40", "type": "number"}}
                ]
            },
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    assert!(
        op_response.get("error").is_none(),
        "append-records op failed: {op_response:?}"
    );
    let readback = &op_response["result"]["readback"];
    assert_eq!(readback["rowsAppended"], Value::from(2));
    assert_eq!(
        readback["previousRange"],
        Value::String("A1:B3".to_string())
    );
    assert_eq!(readback["range"], Value::String("A1:B5".to_string()));
    assert_eq!(readback["columns"], serde_json::json!(["Region", "Amount"]));
    assert_eq!(
        readback["destination"]["appended"]["values"][0][0],
        Value::String("North".to_string())
    );
    assert_eq!(
        readback["destination"]["appended"]["values"][1][1],
        serde_json::json!(40)
    );

    let plan = rpc_request(3, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][2],
        Value::String("append-records".to_string())
    );

    let commit = rpc_request(4, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["file"],
        Value::String(output_str.clone())
    );
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["output"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(
        validate_code, 0,
        "append-records serve output validate exit"
    );
    assert_eq!(
        validate_stderr, None,
        "append-records serve output validate stderr"
    );

    let (export_code, export_stdout, export_stderr) = run_go_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        &output_str,
        "--sheet",
        "Data",
        "--range",
        "A4:B5",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(export_code, 0, "Go export readback exit");
    assert_eq!(export_stderr, None, "Go export readback stderr");
    let export = export_stdout.expect("Go export readback");
    assert_eq!(export["values"][0][0], Value::String("North".to_string()));
    assert_eq!(export["values"][0][1], serde_json::json!(30));
    assert_eq!(export["values"][1][0], Value::String("South".to_string()));
    assert_eq!(export["values"][1][1], serde_json::json!(40));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn serve_op_supports_xlsx_workbook_metadata_update() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-workbook-metadata-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-metadata-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(
        1,
        "open",
        serde_json::json!({"file": input_str, "out": output_str}),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx workbook metadata update",
            "args": {
                "title": "Serve Title",
                "company": "Acme Corp",
                "fullCalcOnLoad": true
            },
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    assert!(
        op_response.get("error").is_none(),
        "metadata op failed: {op_response:?}"
    );
    let readback = &op_response["result"]["readback"];
    assert_eq!(
        readback["metadata"]["title"],
        Value::String("Serve Title".to_string())
    );
    assert_eq!(
        readback["metadata"]["company"],
        Value::String("Acme Corp".to_string())
    );
    assert_eq!(
        readback["calcSettings"]["fullCalcOnLoad"],
        Value::Bool(true)
    );

    let inspect = rpc_request(
        3,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx workbook metadata inspect",
            "args": {},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    assert_eq!(
        inspect_response["result"]["metadata"]["title"],
        Value::String("Serve Title".to_string())
    );

    let plan = rpc_request(4, "plan", serde_json::json!({"session": session}));
    let plan_response = serve_roundtrip(&mut stdin, &mut reader, &plan);
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][1],
        Value::String("workbook".to_string())
    );
    assert_eq!(
        plan_response["result"]["plan"][0]["argv"][3],
        Value::String("update".to_string())
    );

    let commit = rpc_request(5, "commit", serde_json::json!({"session": session}));
    let commit_response = serve_roundtrip(&mut stdin, &mut reader, &commit);
    assert!(
        commit_response.get("error").is_none(),
        "metadata commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    let (inspect_code, inspect_stdout, inspect_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "workbook",
        "metadata",
        "inspect",
        &output_str,
    ]);
    assert_eq!(inspect_code, 0, "metadata serve inspect output exit");
    assert_eq!(inspect_stderr, None, "metadata serve inspect output stderr");
    assert_eq!(
        inspect_stdout.expect("metadata serve output inspect")["metadata"]["title"],
        Value::String("Serve Title".to_string())
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}
