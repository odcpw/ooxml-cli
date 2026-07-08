use super::*;

#[test]
fn conformance_check_matches_rust_baseline_for_spreadsheet_semantic_references() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-spreadsheet-semantics-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance spreadsheet semantics temp dir");

    let defined_names = temp_dir.join("defined-name-semantics.xlsx");
    write_defined_name_semantics_xlsx(&defined_names);
    assert_baseline_rust_repair_invariants_match(&defined_names);

    let calc_chain = temp_dir.join("calc-chain-semantics.xlsx");
    write_calc_chain_semantics_xlsx(&calc_chain);
    assert_baseline_rust_repair_invariants_match(&calc_chain);

    let missing_styles = temp_dir.join("cell-style-missing-styles.xlsx");
    write_style_semantics_xlsx(&missing_styles, StyleFixtureKind::MissingStyles);
    assert_baseline_rust_repair_invariants_match(&missing_styles);

    let style_out_of_range = temp_dir.join("cell-style-out-of-range.xlsx");
    write_style_semantics_xlsx(&style_out_of_range, StyleFixtureKind::OutOfRange);
    assert_baseline_rust_repair_invariants_match(&style_out_of_range);

    let hyperlink_references = temp_dir.join("worksheet-hyperlink-references.xlsx");
    write_hyperlink_reference_xlsx(&hyperlink_references);
    assert_baseline_rust_repair_invariants_match(&hyperlink_references);
    assert_rust_baseline_reports_hyperlink_reference_diagnostics(&hyperlink_references);
}

fn assert_baseline_rust_repair_invariants_match(file: &Path) {
    let file = file.to_string_lossy().to_string();
    let args = ["--json", "conformance", "check", file.as_str()];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_code, baseline_code, "exit code for {file}");
    assert_eq!(rust_stderr, baseline_stderr, "stderr for {file}");
    let rust_report = rust_stdout.expect("rust conformance stdout");
    let baseline_report = baseline_stdout.expect("baseline conformance stdout");
    assert_eq!(
        check_by_name(&rust_report, "repair-invariants"),
        check_by_name(&baseline_report, "repair-invariants"),
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

fn write_hyperlink_reference_xlsx(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create worksheet hyperlink reference xlsx");
    let mut writer = ZipWriter::new(output);
    let zip_options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    write_zip_string(
        &mut writer,
        zip_options,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/drawings/drawing1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/>
</Types>"#,
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
    write_zip_string(&mut writer, zip_options, "xl/workbook.xml", workbook_xml());
    write_zip_string(
        &mut writer,
        zip_options,
        "xl/_rels/workbook.xml.rels",
        &workbook_rels(false, false),
    );
    write_zip_string(
        &mut writer,
        zip_options,
        "xl/worksheets/sheet1.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData/>
  <hyperlinks>
    <hyperlink ref="A1" r:id="rIdMissingHyperlink"/>
    <hyperlink ref="A2" r:id="rIdWrongHyperlink"/>
    <hyperlink ref="A3" r:id="rIdInternalHyperlink"/>
    <hyperlink ref="A4" location="Sheet2!A1"/>
  </hyperlinks>
</worksheet>"#,
    );
    write_zip_string(
        &mut writer,
        zip_options,
        "xl/worksheets/_rels/sheet1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdWrongHyperlink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/>
  <Relationship Id="rIdInternalHyperlink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="Sheet2!A1"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        zip_options,
        "xl/drawings/drawing1.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"/>"#,
    );
    writer
        .finish()
        .expect("finish worksheet hyperlink reference xlsx");
}

fn assert_rust_baseline_reports_hyperlink_reference_diagnostics(file: &Path) {
    let file = file.to_string_lossy().to_string();
    let args = ["--json", "conformance", "check", file.as_str()];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
    assert_ne!(
        baseline_code, 0,
        "Rust baseline should reject hyperlink fixture"
    );
    assert_eq!(
        baseline_stderr, None,
        "Rust baseline hyperlink fixture stderr"
    );
    let baseline_report = baseline_stdout.expect("Rust baseline conformance stdout");
    let diagnostics = check_by_name(&baseline_report, "repair-invariants")["diagnostics"]
        .as_array()
        .expect("repair-invariants diagnostics");
    let hyperlink_diagnostics: Vec<&Value> = diagnostics
        .iter()
        .filter(|diag| diag["code"].as_str() == Some("XLSX_WORKSHEET_HYPERLINK_REFERENCE"))
        .collect();
    assert_eq!(
        hyperlink_diagnostics.len(),
        3,
        "Rust baseline hyperlink fixture should exercise missing relationship, wrong type, and internal TargetMode: {hyperlink_diagnostics:#?}"
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
