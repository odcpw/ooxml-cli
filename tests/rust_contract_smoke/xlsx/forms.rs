#[test]
fn xlsx_forms_entry_creates_macro_workbook_with_non_activex_button() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-forms-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let out = temp_dir.join("entry-form.xlsm");
    let out_str = out.to_string_lossy().to_string();

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "forms",
        "entry",
        "--out",
        &out_str,
        "--field",
        "Name",
        "--field",
        "Email",
        "--field",
        "Notes",
        "--button",
        "Submit Entry",
    ]);
    assert_eq!(code, 0, "forms entry exit");
    assert_eq!(stderr, None, "forms entry stderr");
    let created = stdout.expect("forms entry stdout");
    assert_eq!(created["output"], Value::String(out_str.clone()));
    assert_eq!(created["macroEnabled"], Value::Bool(true));
    assert_eq!(created["activeX"], Value::Bool(false));
    assert_eq!(
        created["formKind"],
        Value::String("worksheet-form-control".to_string())
    );
    assert_eq!(
        created["button"]["macro"],
        Value::String("SubmitEntry".to_string())
    );
    assert_eq!(
        created["textInputKind"],
        Value::String("worksheet-cells".to_string())
    );
    assert_eq!(created["inputRange"], Value::String("B5:B7".to_string()));
    assert_eq!(created["buttons"].as_array().expect("buttons").len(), 3);
    assert_eq!(created["controls"].as_array().expect("controls").len(), 2);
    assert!(created["macros"].to_string().contains("ClearEntryForm"));
    assert!(created["macros"].to_string().contains("FillSampleEntry"));
    assert_rust_emitted_ooxml_command_exits_zero(&created, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&created, "vbaListCommand");

    let (cap_code, cap_stdout, cap_stderr) = run_ooxml(&["--json", "capabilities", "--for", "form"]);
    assert_eq!(cap_code, 0, "form capabilities exit");
    assert_eq!(cap_stderr, None, "form capabilities stderr");
    let capabilities = cap_stdout.expect("form capabilities stdout");
    assert!(
        capabilities["commands"]
            .as_array()
            .expect("commands array")
            .iter()
            .any(|command| command["path"] == Value::String("ooxml xlsx forms entry".to_string())),
        "form capabilities should advertise xlsx forms entry: {capabilities}"
    );

    for entry in [
        "[Content_Types].xml",
        "_rels/.rels",
        "docProps/core.xml",
        "docProps/app.xml",
        "xl/workbook.xml",
        "xl/_rels/workbook.xml.rels",
        "xl/worksheets/sheet1.xml",
        "xl/worksheets/sheet2.xml",
        "xl/worksheets/_rels/sheet1.xml.rels",
        "xl/drawings/vmlDrawing1.vml",
        "xl/styles.xml",
        "xl/vbaProject.bin",
    ] {
        assert!(zip_entry_exists(&out, entry), "missing entry {entry}");
    }

    let content_types = read_zip_string(&out, "[Content_Types].xml");
    assert!(
        content_types.contains("application/vnd.ms-excel.sheet.macroEnabled.main+xml"),
        "missing macro-enabled workbook content type:\n{content_types}"
    );
    assert!(
        content_types.contains("application/vnd.openxmlformats-officedocument.vmlDrawing"),
        "missing VML content type:\n{content_types}"
    );
    assert!(
        content_types.contains("application/vnd.ms-office.vbaProject"),
        "missing VBA content type:\n{content_types}"
    );

    let workbook_rels = read_zip_string(&out, "xl/_rels/workbook.xml.rels");
    assert!(
        workbook_rels.contains("relationships/vbaProject")
            && workbook_rels.contains(r#"Target="vbaProject.bin""#),
        "missing vbaProject relationship:\n{workbook_rels}"
    );
    let sheet_rels = read_zip_string(&out, "xl/worksheets/_rels/sheet1.xml.rels");
    assert!(
        sheet_rels.contains("relationships/vmlDrawing")
            && sheet_rels.contains(r#"Target="../drawings/vmlDrawing1.vml""#),
        "missing worksheet VML relationship:\n{sheet_rels}"
    );

    let form_sheet = read_zip_string(&out, "xl/worksheets/sheet1.xml");
    assert!(form_sheet.contains(r#"<legacyDrawing r:id="rId1"/>"#));
    assert!(form_sheet.contains(r#"showGridLines="0""#));
    assert!(form_sheet.contains(r#"<c r="A5" s="2" t="inlineStr"><is><t>Name</t></is></c>"#));
    assert!(form_sheet.contains(r#"<c r="B5" s="3"/>"#));
    assert!(form_sheet.contains(r#"<c r="A7" s="2" t="inlineStr"><is><t>Notes</t></is></c>"#));

    let data_sheet = read_zip_string(&out, "xl/worksheets/sheet2.xml");
    assert!(data_sheet.contains(r#"<c r="A1" s="4" t="inlineStr"><is><t>Timestamp</t></is></c>"#));
    assert!(data_sheet.contains(r#"<c r="D1" s="4" t="inlineStr"><is><t>Notes</t></is></c>"#));

    let vml = read_zip_string(&out, "xl/drawings/vmlDrawing1.vml");
    assert!(vml.contains(r#"ObjectType="Button""#));
    assert!(vml.contains(r#"ObjectType="GBox""#));
    assert!(vml.contains(r#"ObjectType="Label""#));
    assert!(vml.contains("<x:FmlaMacro>SubmitEntry</x:FmlaMacro>"));
    assert!(vml.contains("<x:FmlaMacro>ClearEntryForm</x:FmlaMacro>"));
    assert!(vml.contains("<x:FmlaMacro>FillSampleEntry</x:FmlaMacro>"));
    assert!(vml.contains("Submit Entry"));
    assert!(vml.contains("Entry details"));
    assert!(vml.contains("Worksheet inputs"));
    assert!(vml.contains("Fill Sample"));
    assert!(
        !vml.to_ascii_lowercase().contains("activex") && !vml.contains("OLEObject"),
        "VML should not contain ActiveX/OLE control markers:\n{vml}"
    );

    let (vba_code, vba_stdout, vba_stderr) = run_ooxml(&["--json", "vba", "list", &out_str]);
    assert_eq!(vba_code, 0, "vba list exit");
    assert_eq!(vba_stderr, None, "vba list stderr");
    let vba = vba_stdout.expect("vba list stdout");
    assert!(
        vba.to_string().contains("EntryFormMacros"),
        "VBA list should expose generated module: {vba}"
    );

    let extract_dir = temp_dir.join("macros");
    let extract_dir_str = extract_dir.to_string_lossy().to_string();
    let (extract_code, extract_stdout, extract_stderr) =
        run_ooxml(&["--json", "vba", "extract", &out_str, "--out-dir", &extract_dir_str]);
    assert_eq!(extract_code, 0, "vba extract exit");
    assert_eq!(extract_stderr, None, "vba extract stderr");
    assert!(extract_stdout.is_some(), "vba extract stdout");
    let source = fs::read_to_string(extract_dir.join("EntryFormMacros.bas"))
        .expect("extracted EntryFormMacros.bas");
    assert!(source.contains("Public Sub SubmitEntry()"));
    assert!(source.contains("Public Sub ClearEntryForm()"));
    assert!(source.contains("Public Sub FillSampleEntry()"));
    assert!(source.contains("dataSheet.Cells(nextRow, 4).Value = formSheet.Range(\"B7\").Value"));

    let _ = fs::remove_dir_all(&temp_dir);
}
