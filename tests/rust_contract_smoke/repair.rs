use super::*;

#[test]
fn repair_normalize_fixes_xlsx_workbook_child_order() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-repair-normalize-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("repair temp dir");

    let broken_path = temp_dir.join("bad-order.xlsx");
    let repaired_path = temp_dir.join("repaired.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &broken_path,
        |name, data| {
            let data = if name == "xl/workbook.xml" {
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <workbookPr/>
  <definedNames><definedName name="SalesData">Sheet1!$A$1:$B$2</definedName></definedNames>
  <bookViews><workbookView activeTab="0"/></bookViews>
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
  <calcPr calcId="191029"/>
</workbook>"#
                    .to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let broken = broken_path.to_string_lossy().to_string();
    let repaired = repaired_path.to_string_lossy().to_string();

    assert_workbook_child_order_rejected("broken input", &broken);

    let dry_run = run_ooxml_json_ok(
        "repair dry-run",
        &["--json", "repair", "normalize", &broken, "--dry-run"],
    );
    assert_eq!(dry_run["changed"], Value::Bool(true));
    assert_eq!(
        dry_run["repairs"][0]["code"],
        Value::String("XLSX_WORKBOOK_CHILD_ORDER_NORMALIZED".to_string())
    );

    let repaired_report = run_ooxml_json_ok(
        "repair normalize",
        &["--json", "repair", "normalize", &broken, "--out", &repaired],
    );
    assert_eq!(repaired_report["output"], Value::String(repaired.clone()));
    assert_eq!(repaired_report["changed"], Value::Bool(true));
    assert_eq!(
        repaired_report["validateCommand"],
        Value::String(format!(
            "ooxml validate --strict {}",
            command_arg_for_test(&repaired)
        ))
    );

    let workbook_xml = read_zip_string(&repaired_path, "xl/workbook.xml");
    assert_xml_tag_order(
        &workbook_xml,
        &[
            "<workbookPr",
            "<bookViews",
            "<sheets",
            "</sheets>",
            "<definedNames",
            "</definedNames>",
            "<calcPr",
        ],
    );

    assert_command_succeeds(
        "repaired strict validate",
        &["--json", "validate", "--strict", &repaired],
    );
    assert_command_succeeds(
        "repaired conformance",
        &["--json", "conformance", "check", &repaired],
    );

    let no_change = run_ooxml_json_ok(
        "repair normalized dry-run",
        &["--json", "repair", "normalize", &repaired, "--dry-run"],
    );
    assert_eq!(no_change["changed"], Value::Bool(false));
    assert_eq!(no_change["repairs"], Value::Array(Vec::new()));

    let _ = fs::remove_dir_all(&temp_dir);
}

fn run_ooxml_json_ok(label: &str, args: &[&str]) -> Value {
    let (code, stdout, stderr) = run_ooxml(args);
    assert_eq!(code, 0, "{label} exit");
    assert_eq!(stderr, None, "{label} stderr");
    stdout.unwrap_or_else(|| panic!("{label} stdout"))
}

fn assert_command_succeeds(label: &str, args: &[&str]) {
    let (code, stdout, stderr) = run_ooxml(args);
    assert_eq!(code, 0, "{label} exit");
    assert_eq!(stderr, None, "{label} stderr");
    assert!(stdout.is_some(), "{label} stdout");
}

fn assert_workbook_child_order_rejected(label: &str, file: &str) {
    for args in [
        vec!["--json", "validate", "--strict", file],
        vec!["--json", "conformance", "check", file],
    ] {
        let (code, stdout, stderr) = run_ooxml(&args);
        assert_ne!(code, 0, "{label} {args:?} should reject bad order");
        assert_eq!(stderr, None, "{label} {args:?} stderr");
        let report = stdout.unwrap_or_else(|| panic!("{label} {args:?} stdout"));
        assert!(
            json_contains_diagnostic_code(&report, "XLSX_WORKBOOK_CHILD_ORDER"),
            "{label} {args:?} did not report workbook child order:\n{report:#}"
        );
    }
}

fn json_contains_diagnostic_code(value: &Value, code: &str) -> bool {
    match value {
        Value::Object(map) => {
            map.get("code").and_then(Value::as_str) == Some(code)
                || map
                    .values()
                    .any(|child| json_contains_diagnostic_code(child, code))
        }
        Value::Array(items) => items
            .iter()
            .any(|child| json_contains_diagnostic_code(child, code)),
        _ => false,
    }
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
