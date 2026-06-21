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

#[test]
fn xlsx_scaffold_rust_only_builds_formula_table_workbook() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-scaffold-workflow-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("xlsx scaffold workflow temp dir");

    let scaffold_path = temp_dir.join("01-scaffold.xlsx");
    let data_path = temp_dir.join("02-data.xlsx");
    let table_path = temp_dir.join("03-table.xlsx");
    let formatted_path = temp_dir.join("04-formatted.xlsx");
    let final_path = temp_dir.join("05-final.xlsx");
    let scaffold = scaffold_path.to_string_lossy().to_string();
    let data = data_path.to_string_lossy().to_string();
    let table = table_path.to_string_lossy().to_string();
    let formatted = formatted_path.to_string_lossy().to_string();
    let final_file = final_path.to_string_lossy().to_string();

    let scaffold_result = run_ooxml_json_ok("workflow scaffold", &[
        "--json",
        "xlsx",
        "scaffold",
        &scaffold,
        "--sheet",
        "Sales Ops",
    ]);
    assert_eq!(
        scaffold_result["sheet"],
        Value::String("Sales Ops".to_string())
    );

    let workbook_values = r#"[
        ["Region","Account","Units","Unit Price","Revenue"],
        ["North","Enterprise",12,19.95,{"formula":"C2*D2"}],
        ["South","Midmarket",8,24.50,{"formula":"C3*D3"}],
        ["West","Startup",15,9.99,{"formula":"C4*D4"}],
        ["East","Renewal",10,29.00,{"formula":"C5*D5"}]
    ]"#;
    let set_result = run_ooxml_json_ok("workflow ranges set", &[
        "--json",
        "xlsx",
        "ranges",
        "set",
        &scaffold,
        "--sheet",
        "Sales Ops",
        "--range",
        "A1:E5",
        "--values",
        workbook_values,
        "--out",
        &data,
    ]);
    assert_eq!(set_result["range"], "A1:E5");
    assert_eq!(set_result["formulaCount"], Value::from(4));
    assert_rust_emitted_ooxml_command_succeeds(&set_result, "rangesExportCommand");

    let table_result = run_ooxml_json_ok("workflow table create", &[
        "--json",
        "xlsx",
        "tables",
        "create",
        &data,
        "--sheet",
        "Sales Ops",
        "--range",
        "A1:E5",
        "--table",
        "SalesOps",
        "--style",
        "TableStyleMedium4",
        "--out",
        &table,
    ]);
    assert_eq!(table_result["table"], "SalesOps");
    assert_eq!(table_result["columns"], serde_json::json!([
        "Region",
        "Account",
        "Units",
        "Unit Price",
        "Revenue"
    ]));
    assert_rust_emitted_ooxml_command_succeeds(&table_result, "tableExportCommand");

    let format_result = run_ooxml_json_ok("workflow table column format", &[
        "--json",
        "xlsx",
        "tables",
        "set-column-format",
        &table,
        "--table",
        "SalesOps",
        "--column",
        "Revenue",
        "--preset",
        "currency",
        "--decimals",
        "2",
        "--out",
        &formatted,
    ]);
    assert_eq!(format_result["range"], "E2:E5");
    assert_eq!(format_result["column"], "Revenue");
    assert_rust_emitted_ooxml_command_succeeds(&format_result, "rangesExportCommand");

    let cf_result = run_ooxml_json_ok("workflow conditional format", &[
        "--json",
        "xlsx",
        "conditional-formats",
        "add",
        &formatted,
        "--sheet",
        "Sales Ops",
        "--range",
        "E2:E5",
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
        "1",
        "--out",
        &final_file,
    ]);
    assert_eq!(cf_result["range"], "E2:E5");
    assert_eq!(cf_result["rule"]["type"], "colorScale");
    assert_rust_emitted_ooxml_command_succeeds(&cf_result, "conditionalFormatsShowCommand");

    assert_xlsx_strict_valid(&final_file);
    assert_conformance_check_passed("workflow conformance", &final_file);

    let range = run_ooxml_json_ok("workflow range readback", &[
        "--json",
        "xlsx",
        "ranges",
        "export",
        &final_file,
        "--sheet",
        "Sales Ops",
        "--range",
        "A1:E5",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ]);
    assert_eq!(range["values"][1][0], "North");
    assert_eq!(range["values"][2][2], Value::from(8));
    assert_eq!(range["formulaCount"], Value::from(4));
    assert_eq!(range["formulas"][1][4], "C2*D2");
    assert_eq!(range["formulas"][4][4], "C5*D5");
    let revenue_format = range["numberFormatCodes"][1][4]
        .as_str()
        .expect("Revenue number format readback");
    assert!(
        revenue_format.contains("#,##0.00"),
        "unexpected Revenue number format: {revenue_format}"
    );

    let table_export = run_ooxml_json_ok("workflow table export", &[
        "--json",
        "xlsx",
        "tables",
        "export",
        &final_file,
        "--table",
        "SalesOps",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(table_export["range"], "A1:E5");
    assert_eq!(table_export["formulaCount"], Value::from(4));
    assert_eq!(table_export["values"][4][1], "Renewal");
    assert_eq!(table_export["formulas"][3][4], "C4*D4");

    let cf_show = run_ooxml_json_ok("workflow conditional format show", &[
        "--json",
        "xlsx",
        "conditional-formats",
        "show",
        &final_file,
        "--sheet",
        "Sales Ops",
        "--rule",
        "cfRule:1",
    ]);
    assert_eq!(cf_show["sqref"], "E2:E5");
    assert_eq!(cf_show["colorScale"]["colors"][2]["rgb"], "FF63BE7B");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_pivot_and_chart_create_reject_uncached_formula_sources() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-formula-cache-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("xlsx formula cache temp dir");

    let scaffold_path = temp_dir.join("01-scaffold.xlsx");
    let data_path = temp_dir.join("02-formulas.xlsx");
    let scaffold = scaffold_path.to_string_lossy().to_string();
    let data = data_path.to_string_lossy().to_string();

    run_ooxml_json_ok("formula cache scaffold", &[
        "--json",
        "xlsx",
        "scaffold",
        &scaffold,
        "--sheet",
        "Data",
    ]);
    let values = r#"[
        ["Region","Units","Revenue"],
        ["North",2,{"formula":"B2*10"}],
        ["South",3,{"formula":"B3*10"}]
    ]"#;
    let set_result = run_ooxml_json_ok("formula cache ranges set", &[
        "--json",
        "xlsx",
        "ranges",
        "set",
        &scaffold,
        "--sheet",
        "Data",
        "--range",
        "A1:C3",
        "--values",
        values,
        "--out",
        &data,
    ]);
    assert_eq!(set_result["formulaCount"], Value::from(2));

    let (pivot_code, pivot_stdout, pivot_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "pivots",
        "create",
        &data,
        "--sheet",
        "Data",
        "--range",
        "A1:C3",
        "--rows",
        "Region",
        "--values",
        "Revenue:sum",
        "--dry-run",
    ]);
    assert_eq!(pivot_code, 2, "pivot create should reject uncached formulas");
    assert_eq!(pivot_stdout, None, "pivot create stdout");
    assert_formula_cache_error("pivot", pivot_stderr, "pivot source cell C2");

    let (chart_code, chart_stdout, chart_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "charts",
        "create",
        &data,
        "--type",
        "bar",
        "--sheet",
        "Data",
        "--range",
        "A1:C3",
        "--title",
        "Revenue",
        "--dry-run",
    ]);
    assert_eq!(chart_code, 2, "chart create should reject uncached formulas");
    assert_eq!(chart_stdout, None, "chart create stdout");
    assert_formula_cache_error("chart", chart_stderr, "chart source cell C2");

    let cached_dirty_path = temp_dir.join("03-cached-dirty.xlsx");
    rewrite_zip_fixture(&data, &cached_dirty_path, |name, bytes| {
        if name != "xl/worksheets/sheet1.xml" {
            return Some((name.to_string(), bytes));
        }
        let xml = String::from_utf8(bytes).expect("worksheet utf8");
        let updated = xml
            .replace(
                r#"<c r="C2"><f>B2*10</f></c>"#,
                r#"<c r="C2"><f>B2*10</f><v>20</v></c>"#,
            )
            .replace(
                r#"<c r="C3"><f>B3*10</f></c>"#,
                r#"<c r="C3"><f>B3*10</f><v>30</v></c>"#,
            );
        assert_ne!(updated, xml, "cached formula fixture should be rewritten");
        Some((name.to_string(), updated.into_bytes()))
    });
    assert_xlsx_full_calc_flags(&cached_dirty_path);
    let cached_dirty = cached_dirty_path.to_string_lossy().to_string();

    let (dirty_pivot_code, dirty_pivot_stdout, dirty_pivot_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "pivots",
        "create",
        &cached_dirty,
        "--sheet",
        "Data",
        "--range",
        "A1:C3",
        "--rows",
        "Region",
        "--values",
        "Revenue:sum",
        "--dry-run",
    ]);
    assert_eq!(
        dirty_pivot_code, 2,
        "pivot create should reject dirty cached formulas"
    );
    assert_eq!(dirty_pivot_stdout, None, "dirty pivot stdout");
    assert_formula_recalc_error("dirty pivot", dirty_pivot_stderr, "pivot source cell C2");

    let (dirty_chart_code, dirty_chart_stdout, dirty_chart_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "charts",
        "create",
        &cached_dirty,
        "--type",
        "bar",
        "--sheet",
        "Data",
        "--range",
        "A1:C3",
        "--title",
        "Revenue",
        "--dry-run",
    ]);
    assert_eq!(
        dirty_chart_code, 2,
        "chart create should reject dirty cached formulas"
    );
    assert_eq!(dirty_chart_stdout, None, "dirty chart stdout");
    assert_formula_recalc_error("dirty chart", dirty_chart_stderr, "chart source cell C2");

    let _ = fs::remove_dir_all(&temp_dir);
}

fn run_ooxml_json_ok(label: &str, args: &[&str]) -> Value {
    let (code, stdout, stderr) = run_ooxml(args);
    assert_eq!(code, 0, "{label} exit");
    assert_eq!(stderr, None, "{label} stderr");
    stdout.unwrap_or_else(|| panic!("{label} stdout"))
}

fn assert_formula_cache_error(label: &str, stderr: Option<Value>, expected_prefix: &str) {
    assert_formula_source_error(
        label,
        stderr,
        expected_prefix,
        "cached calculated value",
    );
}

fn assert_formula_recalc_error(label: &str, stderr: Option<Value>, expected_prefix: &str) {
    assert_formula_source_error(label, stderr, expected_prefix, "marked for recalculation");
}

fn assert_formula_source_error(
    label: &str,
    stderr: Option<Value>,
    expected_prefix: &str,
    expected_detail: &str,
) {
    let error = stderr.unwrap_or_else(|| panic!("{label} stderr"));
    assert_eq!(
        error["error"]["code"],
        Value::String("invalid_args".to_string())
    );
    let message = error["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("{label} error message"));
    assert!(
        message.contains(expected_prefix) && message.contains(expected_detail),
        "{label} error should explain formula source rejection: {message}"
    );
}

fn assert_conformance_check_passed(label: &str, file: &str) {
    let conformance = run_ooxml_json_ok(label, &["--json", "conformance", "check", file]);
    assert_eq!(conformance["schemaVersion"], "ooxml-cli.conformance.v1");
    assert_eq!(conformance["status"], "passed");
    assert_eq!(conformance["summary"]["failed"], Value::from(0));
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
