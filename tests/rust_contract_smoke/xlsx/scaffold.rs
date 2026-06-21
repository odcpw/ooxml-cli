#[test]
fn xlsx_scaffold_creates_valid_readable_mutable_ordered_workbook() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-scaffold-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("xlsx scaffold temp dir");
    let out = temp_dir.join("created.xlsx");
    let out_str = out.to_string_lossy().to_string();

    let (create_code, create_stdout, create_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "scaffold",
        &out_str,
        "--sheet",
        "Budget & Ops",
    ]);
    assert_eq!(create_code, 0, "xlsx scaffold exit");
    assert_eq!(create_stderr, None, "xlsx scaffold stderr");
    let create = create_stdout.expect("xlsx scaffold stdout");
    assert_eq!(create["output"], Value::String(out_str.clone()));
    assert_eq!(create["created"], Value::Bool(true));
    assert_eq!(create["family"], Value::String("xlsx".to_string()));
    assert_eq!(
        create["workbookPart"],
        Value::String("xl/workbook.xml".to_string())
    );
    assert_eq!(
        create["worksheetPart"],
        Value::String("xl/worksheets/sheet1.xml".to_string())
    );
    assert_eq!(
        create["stylesPart"],
        Value::String("xl/styles.xml".to_string())
    );
    assert_eq!(create["sheet"], Value::String("Budget & Ops".to_string()));
    assert_eq!(create["sheetId"], Value::String("1".to_string()));
    assert_eq!(create["validated"], Value::Bool(true));
    assert_eq!(
        create["validateCommand"],
        Value::String(format!(
            "ooxml validate --strict {}",
            command_arg_for_test(&out_str)
        ))
    );
    assert_eq!(
        create["conformanceCommand"],
        Value::String(format!(
            "ooxml --json conformance check {}",
            command_arg_for_test(&out_str)
        ))
    );
    assert_eq!(
        create["readbackCommand"],
        Value::String(format!(
            "ooxml --json xlsx sheets list {}",
            command_arg_for_test(&out_str)
        ))
    );

    for entry in [
        "[Content_Types].xml",
        "_rels/.rels",
        "docProps/core.xml",
        "docProps/app.xml",
        "xl/workbook.xml",
        "xl/_rels/workbook.xml.rels",
        "xl/worksheets/sheet1.xml",
        "xl/styles.xml",
    ] {
        assert!(zip_entry_exists(&out, entry), "missing scaffold entry {entry}");
    }

    let content_types = read_zip_string(&out, "[Content_Types].xml");
    assert!(
        content_types.contains(
            r#"ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml""#
        ),
        "workbook content type missing: {content_types}"
    );
    assert!(
        content_types.contains(
            r#"ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml""#
        ),
        "styles content type missing: {content_types}"
    );
    let root_rels = read_zip_string(&out, "_rels/.rels");
    assert!(
        root_rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument""#
        ),
        "officeDocument relationship missing: {root_rels}"
    );
    let workbook_rels = read_zip_string(&out, "xl/_rels/workbook.xml.rels");
    assert!(
        workbook_rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet""#
        ) && workbook_rels.contains(r#"Target="worksheets/sheet1.xml""#),
        "worksheet relationship missing: {workbook_rels}"
    );
    assert!(
        workbook_rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles""#
        ) && workbook_rels.contains(r#"Target="styles.xml""#),
        "styles relationship missing: {workbook_rels}"
    );

    let workbook_xml = read_zip_string(&out, "xl/workbook.xml");
    assert!(
        workbook_xml.contains(r#"name="Budget &amp; Ops""#),
        "sheet name not escaped in workbook XML: {workbook_xml}"
    );
    assert_xml_tag_order(
        &workbook_xml,
        &[
            "<workbookPr",
            "<bookViews",
            "<sheets",
            "</sheets>",
            "<calcPr",
        ],
    );
    let app_xml = read_zip_string(&out, "docProps/app.xml");
    assert!(
        app_xml.contains("<vt:lpstr>Budget &amp; Ops</vt:lpstr>"),
        "app props did not mirror sheet title: {app_xml}"
    );
    let worksheet_xml = read_zip_string(&out, "xl/worksheets/sheet1.xml");
    assert_xml_tag_order(
        &worksheet_xml,
        &[
            "<dimension",
            "<sheetViews",
            "<sheetFormatPr",
            "<sheetData",
            "<pageMargins",
        ],
    );
    let styles_xml = read_zip_string(&out, "xl/styles.xml");
    assert!(
        styles_xml.contains("<styleSheet")
            && styles_xml.contains("<cellXfs count=\"1\"")
            && styles_xml.contains("<cellStyle name=\"Normal\""),
        "minimal styles part missing expected defaults: {styles_xml}"
    );

    let (sheets_code, sheets_stdout, sheets_stderr) =
        run_ooxml(&["--json", "xlsx", "sheets", "list", &out_str]);
    assert_eq!(sheets_code, 0, "sheets list readback exit");
    assert_eq!(sheets_stderr, None, "sheets list readback stderr");
    let sheets = sheets_stdout.expect("sheets list readback");
    let sheet_items = sheets["sheets"].as_array().expect("sheets array");
    assert_eq!(sheet_items.len(), 1, "scaffold sheet count");
    assert_eq!(
        sheet_items[0]["name"],
        Value::String("Budget & Ops".to_string())
    );
    assert_eq!(sheet_items[0]["sheetId"], Value::String("1".to_string()));

    let mutated = temp_dir.join("mutated.xlsx");
    let mutated_str = mutated.to_string_lossy().to_string();
    let (cell_code, cell_stdout, cell_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "cells",
        "set",
        &out_str,
        "--sheet",
        "Budget & Ops",
        "--cell",
        "A1",
        "--value",
        "hello from scaffold",
        "--out",
        &mutated_str,
    ]);
    assert_eq!(cell_code, 0, "cells set on scaffold exit");
    assert_eq!(cell_stderr, None, "cells set on scaffold stderr");
    assert!(cell_stdout.is_some(), "cells set on scaffold stdout");
    let (export_code, export_stdout, export_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        &mutated_str,
        "--sheet",
        "Budget & Ops",
        "--range",
        "A1:A1",
        "--include-types",
    ]);
    assert_eq!(export_code, 0, "mutated scaffold export exit");
    assert_eq!(export_stderr, None, "mutated scaffold export stderr");
    let exported = export_stdout.expect("mutated scaffold export");
    assert_eq!(
        exported["values"][0][0],
        Value::String("hello from scaffold".to_string())
    );

    for (label, args) in [
        (
            "strict validate",
            vec!["--json", "validate", "--strict", &out_str],
        ),
        (
            "conformance check",
            vec!["--json", "conformance", "check", &out_str],
        ),
    ] {
        let (code, stdout, stderr) = run_ooxml(&args);
        assert_eq!(code, 0, "{label} exit");
        assert_eq!(stderr, None, "{label} stderr");
        assert!(stdout.is_some(), "{label} stdout");
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_scaffold_rejects_existing_output_unless_forced_and_validates_sheet_name() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-scaffold-force-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("xlsx scaffold force temp dir");
    let out = temp_dir.join("created.xlsx");
    let out_str = out.to_string_lossy().to_string();

    let (first_code, _first_stdout, first_stderr) =
        run_ooxml(&["--json", "xlsx", "scaffold", &out_str]);
    assert_eq!(first_code, 0, "initial scaffold exit");
    assert_eq!(first_stderr, None, "initial scaffold stderr");

    let (second_code, second_stdout, second_stderr) =
        run_ooxml(&["--json", "xlsx", "scaffold", &out_str]);
    assert_eq!(second_code, 2, "existing scaffold exit");
    assert_eq!(second_stdout, None, "existing scaffold stdout");
    let error = second_stderr.expect("existing scaffold stderr");
    assert_eq!(
        error["error"]["code"],
        Value::String("invalid_args".to_string())
    );
    assert!(
        error["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("--force"),
        "error should mention --force: {error:?}"
    );

    let (force_code, force_stdout, force_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "scaffold",
        &out_str,
        "--sheet",
        "Forced",
        "--force",
    ]);
    assert_eq!(force_code, 0, "forced scaffold exit");
    assert_eq!(force_stderr, None, "forced scaffold stderr");
    assert_eq!(
        force_stdout.expect("forced scaffold stdout")["sheet"],
        Value::String("Forced".to_string())
    );
    let workbook_xml = read_zip_string(&out, "xl/workbook.xml");
    assert!(
        workbook_xml.contains(r#"name="Forced""#),
        "forced scaffold did not replace workbook: {workbook_xml}"
    );

    let invalid = temp_dir.join("invalid.xlsx").to_string_lossy().to_string();
    let (invalid_code, invalid_stdout, invalid_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "scaffold",
        &invalid,
        "--sheet",
        "Bad/Name",
    ]);
    assert_eq!(invalid_code, 2, "invalid sheet exit");
    assert_eq!(invalid_stdout, None, "invalid sheet stdout");
    let invalid_error = invalid_stderr.expect("invalid sheet stderr");
    assert_eq!(
        invalid_error["error"]["code"],
        Value::String("invalid_args".to_string())
    );
    assert!(
        invalid_error["error"]["message"]
            .as_str()
            .expect("invalid sheet message")
            .contains("invalid Excel worksheet name"),
        "invalid sheet error should explain worksheet-name rules: {invalid_error:?}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

fn assert_xml_tag_order(xml: &str, tags: &[&str]) {
    let mut previous = 0usize;
    for tag in tags {
        let offset = xml[previous..]
            .find(tag)
            .unwrap_or_else(|| panic!("missing {tag} after byte {previous} in:\n{xml}"));
        previous += offset + tag.len();
    }
}
