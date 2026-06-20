use super::*;

#[test]
fn conformance_check_matches_go_for_spreadsheet_semantic_references() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-spreadsheet-semantics-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance spreadsheet semantics temp dir");

    let defined_names = temp_dir.join("defined-name-semantics.xlsx");
    write_defined_name_semantics_xlsx(&defined_names);
    assert_go_rust_repair_invariants_match(&defined_names);

    let calc_chain = temp_dir.join("calc-chain-semantics.xlsx");
    write_calc_chain_semantics_xlsx(&calc_chain);
    assert_go_rust_repair_invariants_match(&calc_chain);

    let missing_styles = temp_dir.join("cell-style-missing-styles.xlsx");
    write_style_semantics_xlsx(&missing_styles, StyleFixtureKind::MissingStyles);
    assert_go_rust_repair_invariants_match(&missing_styles);

    let style_out_of_range = temp_dir.join("cell-style-out-of-range.xlsx");
    write_style_semantics_xlsx(&style_out_of_range, StyleFixtureKind::OutOfRange);
    assert_go_rust_repair_invariants_match(&style_out_of_range);
}

fn assert_go_rust_repair_invariants_match(file: &Path) {
    let file = file.to_string_lossy().to_string();
    let args = ["--json", "conformance", "check", file.as_str()];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_code, go_code, "exit code for {file}");
    assert_eq!(rust_stderr, go_stderr, "stderr for {file}");
    let rust_report = rust_stdout.expect("rust conformance stdout");
    let go_report = go_stdout.expect("go conformance stdout");
    assert_eq!(
        check_by_name(&rust_report, "repair-invariants"),
        check_by_name(&go_report, "repair-invariants"),
        "repair-invariants check for {file}"
    );
}

fn check_by_name<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing check {name}: {}", report["checks"]))
}

fn write_defined_name_semantics_xlsx(dest: &Path) {
    write_semantic_xlsx(
        dest,
        SemanticWorkbookOptions {
            workbook_xml: r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rId1"/>
  </sheets>
  <definedNames>
    <definedName name="MissingRef">Missing!$A$1</definedName>
    <definedName name="BadScope" localSheetId="5">Sheet1!$A$1</definedName>
    <definedName name="Dup">Sheet1!$A$1</definedName>
    <definedName name="dup">Sheet1!$B$1</definedName>
    <definedName localSheetId="-1">Sheet1!$A$1</definedName>
    <definedName name="Stale">#REF!$A$1</definedName>
  </definedNames>
</workbook>"#,
            workbook_rels_xml: workbook_rels(false, false),
            sheet_xml: worksheet_xml(false, "", ""),
            styles_xml: None,
            calc_chain_xml: None,
        },
    );
}

fn write_calc_chain_semantics_xlsx(dest: &Path) {
    write_semantic_xlsx(
        dest,
        SemanticWorkbookOptions {
            workbook_xml: workbook_xml(),
            workbook_rels_xml: workbook_rels(false, true),
            sheet_xml: worksheet_xml(true, "", ""),
            styles_xml: None,
            calc_chain_xml: Some(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<calcChain xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <c r="B1" i="1"/>
  <c r="A1" i="99"/>
  <c i="1"/>
  <c r="NotACell" i="1"/>
</calcChain>"#,
            ),
        },
    );
}

#[derive(Clone, Copy)]
enum StyleFixtureKind {
    MissingStyles,
    OutOfRange,
}

fn write_style_semantics_xlsx(dest: &Path, kind: StyleFixtureKind) {
    let (style_index, include_styles) = match kind {
        StyleFixtureKind::MissingStyles => ("1", false),
        StyleFixtureKind::OutOfRange => ("2", true),
    };
    write_semantic_xlsx(
        dest,
        SemanticWorkbookOptions {
            workbook_xml: workbook_xml(),
            workbook_rels_xml: workbook_rels(include_styles, false),
            sheet_xml: worksheet_xml(false, style_index, ""),
            styles_xml: include_styles.then_some(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cellXfs count="1">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
  </cellXfs>
</styleSheet>"#,
            ),
            calc_chain_xml: None,
        },
    );
}

struct SemanticWorkbookOptions<'a> {
    workbook_xml: &'a str,
    workbook_rels_xml: String,
    sheet_xml: String,
    styles_xml: Option<&'a str>,
    calc_chain_xml: Option<&'a str>,
}

fn write_semantic_xlsx(dest: &Path, options: SemanticWorkbookOptions<'_>) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create spreadsheet semantic xlsx");
    let mut writer = ZipWriter::new(output);
    let zip_options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    write_zip_string(
        &mut writer,
        zip_options,
        "[Content_Types].xml",
        &content_types_xml(
            options.styles_xml.is_some(),
            options.calc_chain_xml.is_some(),
        ),
    );
    write_zip_string(
        &mut writer,
        zip_options,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        zip_options,
        "xl/workbook.xml",
        options.workbook_xml,
    );
    write_zip_string(
        &mut writer,
        zip_options,
        "xl/_rels/workbook.xml.rels",
        &options.workbook_rels_xml,
    );
    write_zip_string(
        &mut writer,
        zip_options,
        "xl/worksheets/sheet1.xml",
        &options.sheet_xml,
    );
    if let Some(styles_xml) = options.styles_xml {
        write_zip_string(&mut writer, zip_options, "xl/styles.xml", styles_xml);
    }
    if let Some(calc_chain_xml) = options.calc_chain_xml {
        write_zip_string(&mut writer, zip_options, "xl/calcChain.xml", calc_chain_xml);
    }
    writer.finish().expect("finish spreadsheet semantic xlsx");
}

fn content_types_xml(include_styles: bool, include_calc_chain: bool) -> String {
    let mut overrides = String::new();
    if include_styles {
        overrides.push_str(
            r#"
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
        );
    }
    if include_calc_chain {
        overrides.push_str(
            r#"
  <Override PartName="/xl/calcChain.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml"/>"#,
        );
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>{overrides}
</Types>"#
    )
}

fn workbook_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#
}

fn workbook_rels(include_styles: bool, include_calc_chain: bool) -> String {
    let mut rels = String::new();
    if include_styles {
        rels.push_str(
            r#"
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
        );
    }
    if include_calc_chain {
        rels.push_str(
            r#"
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain" Target="calcChain.xml"/>"#,
        );
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>{rels}
</Relationships>"#
    )
}

fn worksheet_xml(include_formula: bool, style_index: &str, extra_cell_attrs: &str) -> String {
    let style_attr = if style_index.is_empty() {
        String::new()
    } else {
        format!(r#" s="{style_index}""#)
    };
    let formula = if include_formula {
        "<f>1+1</f><v>2</v>"
    } else {
        "<v>2</v>"
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1"{style_attr}{extra_cell_attrs}>{formula}</c>
      <c r="B1"><v>42</v></c>
    </row>
  </sheetData>
</worksheet>"#
    )
}
