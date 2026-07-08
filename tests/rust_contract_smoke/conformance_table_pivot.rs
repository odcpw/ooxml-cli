use super::*;
use serde_json::Value;
use std::path::Path;

#[test]
fn conformance_check_matches_rust_baseline_for_table_structural_invariants() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-table-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance table temp dir");

    let source = temp_dir.join("table-clean.xlsx");
    write_table_xlsx(&source);
    let source_arg = source.to_string_lossy().to_string();

    let broken = temp_dir.join("table-structural-broken.xlsx");
    rewrite_zip_fixture(&source_arg, &broken, |name, data| {
        if name == "xl/tables/table1.xml" {
            Some((
                name.to_string(),
                br#"<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="abc" name="Sales" displayName="Sales" ref="C3:A1" headerRowCount="1" totalsRowShown="0">
  <tableStyleInfo name="TableStyleMedium2" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/>
  <autoFilter ref="A1:B3"/>
  <tableColumns count="3">
    <tableColumn id="1" name="Region"/>
    <tableColumn id="1"/>
  </tableColumns>
</table>"#
                    .to_vec(),
            ))
        } else {
            Some((name.to_string(), data))
        }
    });

    let repair = assert_baseline_rust_repair_invariants_match(&broken);
    assert_repair_codes(
        &repair,
        &["XLSX_TABLE_CHILD_ORDER", "XLSX_TABLE_DEFINITION"],
    );
}

#[test]
fn conformance_check_matches_rust_baseline_for_pivot_structural_invariants() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-pivot-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance pivot temp dir");

    let broken = temp_dir.join("pivot-structural-broken.xlsx");
    write_pivot_structural_xlsx(&broken);

    let repair = assert_baseline_rust_repair_invariants_match(&broken);
    assert_repair_codes(
        &repair,
        &[
            "XLSX_PIVOT_TABLE_CHILD_ORDER",
            "XLSX_PIVOT_TABLE_DEFINITION",
            "XLSX_PIVOT_CACHE_CHILD_ORDER",
            "XLSX_PIVOT_CACHE_DEFINITION",
            "XLSX_PIVOT_RECORDS_DEFINITION",
        ],
    );
}

fn assert_baseline_rust_repair_invariants_match(file: &Path) -> Value {
    let file = file.to_string_lossy().to_string();
    let args = ["--json", "conformance", "check", file.as_str()];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_code, baseline_code, "exit code for {file}");
    assert_eq!(rust_stderr, baseline_stderr, "stderr for {file}");
    let rust_report = rust_stdout.expect("rust conformance stdout");
    let baseline_report = baseline_stdout.expect("baseline conformance stdout");
    let rust_repair = check_by_name(&rust_report, "repair-invariants").clone();
    let baseline_repair = check_by_name(&baseline_report, "repair-invariants").clone();
    assert_eq!(
        rust_repair, baseline_repair,
        "repair-invariants check for {file}"
    );
    rust_repair
}

fn check_by_name<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing check {name}: {}", report["checks"]))
}

fn assert_repair_codes(repair: &Value, codes: &[&str]) {
    let body = serde_json::to_string(repair).expect("repair JSON");
    for code in codes {
        assert!(
            body.contains(code),
            "repair-invariants output should include {code}: {body}"
        );
    }
}

fn write_pivot_structural_xlsx(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create pivot structural xlsx");
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
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/pivotTables/pivotTable1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/>
  <Override PartName="/xl/pivotCache/pivotCacheDefinition1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"/>
  <Override PartName="/xl/pivotCache/pivotCacheRecords1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml"/>
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
  <sheets>
    <sheet name="Data" sheetId="1" r:id="rIdSheet1"/>
  </sheets>
  <pivotCaches>
    <pivotCache cacheId="1" r:id="rIdCache1"/>
  </pivotCaches>
</workbook>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdSheet1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rIdCache1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition" Target="pivotCache/pivotCacheDefinition1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData/>
  <pivotTableDefinition r:id="rIdPivot1"/>
</worksheet>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/_rels/sheet1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdPivot1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotTables/pivotTable1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" cacheId="0" dataCaption="Values">
  <location ref="A"/>
  <pivotFields count="2">
    <pivotField axis="axisRow"/>
    <pivotField axis="axisCol"/>
    <pivotField dataField="1"/>
  </pivotFields>
  <dataFields count="2">
    <dataField fld="abc"/>
  </dataFields>
  <rowFields count="1"><field x="4"/></rowFields>
  <colFields count="1"><field x="-1"/></colFields>
  <pageFields count="1"><pageField/></pageFields>
</pivotTableDefinition>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotCache/pivotCacheDefinition1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" recordCount="-1">
  <cacheFields count="2">
    <cacheField/>
    <cacheField name="Amount"/>
  </cacheFields>
  <cacheSource type="worksheet"><worksheetSource ref="A"/></cacheSource>
</pivotCacheDefinition>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotCache/_rels/pivotCacheDefinition1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdRecords1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords" Target="pivotCacheRecords1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotCache/pivotCacheRecords1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotCacheRecords xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3"><r/><r/></pivotCacheRecords>"#,
    );
    writer.finish().expect("finish pivot structural xlsx");
}
