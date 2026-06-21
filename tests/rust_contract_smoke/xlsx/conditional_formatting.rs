#[test]
fn xlsx_conditional_formats_list_show_match_go_oracle() {
    assert_go_rust_match(&[
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
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "conditional-formatting",
        "list",
        &workbook,
        "--sheet",
        "Sheet1",
    ]);
    assert_go_rust_match(&[
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
    assert_go_rust_match(&[
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
    assert_go_rust_match(&[
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
fn xlsx_conditional_formats_add_delete_saved_outputs_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-cf-mutate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let go_add_out = temp_dir.join("go-add.xlsx").to_string_lossy().to_string();
    let rust_add_out = temp_dir
        .join("rust-add.xlsx")
        .to_string_lossy()
        .to_string();
    let go_delete_out = temp_dir
        .join("go-delete.xlsx")
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
    let mut go_args = add_common.to_vec();
    go_args.push(&go_add_out);
    let mut rust_args = add_common.to_vec();
    rust_args.push(&rust_add_out);
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "conditional format add exit");
    assert_eq!(rust_stderr, go_stderr, "conditional format add stderr");
    let rust_add = rust_stdout.expect("rust add stdout");
    assert_eq!(
        scrub_path(rust_add.clone(), &rust_add_out, "[ADD_OUT]"),
        scrub_path(go_stdout.expect("go add stdout"), &go_add_out, "[ADD_OUT]"),
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
        &go_add_out,
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
    let (go_code, go_show, go_stderr) = run_go_ooxml(&show_go);
    let (rust_code, rust_show, rust_stderr) = run_ooxml(&show_rust);
    assert_eq!(rust_code, go_code, "saved add show exit");
    assert_eq!(rust_stderr, go_stderr, "saved add show stderr");
    assert_eq!(
        rust_show.expect("rust saved add show"),
        go_show.expect("go saved add show"),
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
    let mut go_args = vec![
        "--json",
        "xlsx",
        "conditional-formats",
        "delete",
        &go_add_out,
    ];
    go_args.extend_from_slice(&delete_common[4..]);
    go_args.push(&go_delete_out);
    let mut rust_args = vec![
        "--json",
        "xlsx",
        "conditional-formats",
        "delete",
        &rust_add_out,
    ];
    rust_args.extend_from_slice(&delete_common[4..]);
    rust_args.push(&rust_delete_out);
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "conditional format delete exit");
    assert_eq!(rust_stderr, go_stderr, "conditional format delete stderr");
    let rust_delete = rust_stdout.expect("rust delete stdout");
    assert_eq!(
        scrub_paths(
            rust_delete.clone(),
            &[(&rust_add_out, "[ADD_OUT]"), (&rust_delete_out, "[DELETE_OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go delete stdout"),
            &[(&go_add_out, "[ADD_OUT]"), (&go_delete_out, "[DELETE_OUT]")]
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
        &go_delete_out,
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
    let (go_code, go_list, go_stderr) = run_go_ooxml(&list_go);
    let (rust_code, rust_list, rust_stderr) = run_ooxml(&list_rust);
    assert_eq!(rust_code, go_code, "deleted list exit");
    assert_eq!(rust_stderr, go_stderr, "deleted list stderr");
    assert_eq!(
        scrub_path(
            rust_list.expect("rust deleted list"),
            &rust_delete_out,
            "[DELETE_OUT]"
        ),
        scrub_path(go_list.expect("go deleted list"), &go_delete_out, "[DELETE_OUT]"),
        "deleted list"
    );

    for output in [&rust_add_out, &rust_delete_out] {
        assert_xlsx_strict_valid(output);
    }

    let _ = fs::remove_dir_all(&temp_dir);
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
