use super::*;
use std::io::{Cursor, Seek};

#[test]
fn conformance_check_matches_rust_baseline_for_chart_embedded_workbook_open_invariants() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-embedded-workbook-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("embedded workbook temp dir");

    let corrupt = temp_dir.join("chart-embedded-workbook-corrupt.pptx");
    write_chart_embedded_workbook_pptx(
        &corrupt,
        EmbeddedWorkbookFixture::Internal(b"not a zip package".to_vec()),
    );
    assert_baseline_rust_repair_invariants_match(&corrupt);
    assert_rust_repair_invariants_include_code(
        &corrupt,
        "OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN",
    );

    let wrong_family = temp_dir.join("chart-embedded-workbook-docx.pptx");
    write_chart_embedded_workbook_pptx(
        &wrong_family,
        EmbeddedWorkbookFixture::Internal(minimal_docx_bytes()),
    );
    assert_baseline_rust_repair_invariants_match(&wrong_family);
    assert_rust_repair_invariants_include_code(
        &wrong_family,
        "OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN",
    );

    let clean = temp_dir.join("chart-embedded-workbook-clean.pptx");
    write_chart_embedded_workbook_pptx(
        &clean,
        EmbeddedWorkbookFixture::Internal(minimal_xlsx_bytes()),
    );
    assert_baseline_rust_repair_invariants_match(&clean);
    assert_rust_repair_invariants_exclude_code(
        &clean,
        "OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN",
    );

    let external = temp_dir.join("chart-external-workbook.pptx");
    write_chart_embedded_workbook_pptx(&external, EmbeddedWorkbookFixture::External);
    assert_baseline_rust_repair_invariants_match(&external);
    assert_rust_repair_invariants_exclude_code(
        &external,
        "OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN",
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

enum EmbeddedWorkbookFixture {
    Internal(Vec<u8>),
    External,
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

fn assert_rust_repair_invariants_include_code(file: &Path, code: &str) {
    let report = rust_conformance_report(file);
    let repair_json =
        serde_json::to_string(check_by_name(&report, "repair-invariants")).expect("repair JSON");
    assert!(
        repair_json.contains(code),
        "repair-invariants should include {code}: {repair_json}"
    );
}

fn assert_rust_repair_invariants_exclude_code(file: &Path, code: &str) {
    let report = rust_conformance_report(file);
    let repair_json =
        serde_json::to_string(check_by_name(&report, "repair-invariants")).expect("repair JSON");
    assert!(
        !repair_json.contains(code),
        "repair-invariants should not include {code}: {repair_json}"
    );
}

fn rust_conformance_report(file: &Path) -> Value {
    let file = file.to_string_lossy().to_string();
    let (_, stdout, stderr) = run_ooxml(&["--json", "conformance", "check", file.as_str()]);
    assert_eq!(stderr, None, "conformance check stderr for {file}");
    stdout.expect("rust conformance stdout")
}

fn check_by_name<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing check {name}: {}", report["checks"]))
}

fn write_chart_embedded_workbook_pptx(dest: &Path, fixture: EmbeddedWorkbookFixture) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create embedded workbook pptx");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let internal_workbook = matches!(fixture, EmbeddedWorkbookFixture::Internal(_));

    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/ppt/charts/chart1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/>
  {embedding_override}
</Types>"#,
            embedding_override = if internal_workbook {
                r#"<Override PartName="/ppt/embeddings/Microsoft_Excel_Sheet1.xlsx" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"/>"#
            } else {
                ""
            }
        ),
    );
    write_zip_string(
        &mut writer,
        options,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "ppt/presentation.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst><p:sldId id="256" r:id="rIdSlide1"/></p:sldIdLst>
  <p:sldSz cx="9144000" cy="6858000" type="screen4x3"/>
  <p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "ppt/_rels/presentation.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdSlide1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "ppt/slides/slide1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:graphicFrame>
        <p:nvGraphicFramePr><p:cNvPr id="2" name="Chart 1"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>
        <p:xfrm><a:off x="0" y="0"/><a:ext cx="9144000" cy="6858000"/></p:xfrm>
        <a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart r:id="rIdChart1"/></a:graphicData></a:graphic>
      </p:graphicFrame>
    </p:spTree>
  </p:cSld>
</p:sld>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "ppt/slides/_rels/slide1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdChart1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/chart1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "ppt/charts/chart1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <c:chart><c:plotArea/></c:chart>
  <c:externalData r:id="rIdWorkbook"/>
</c:chartSpace>"#,
    );

    match fixture {
        EmbeddedWorkbookFixture::Internal(bytes) => {
            write_zip_string(
                &mut writer,
                options,
                "ppt/charts/_rels/chart1.xml.rels",
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdWorkbook" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/package" Target="../embeddings/Microsoft_Excel_Sheet1.xlsx"/>
</Relationships>"#,
            );
            write_zip_bytes(
                &mut writer,
                options,
                "ppt/embeddings/Microsoft_Excel_Sheet1.xlsx",
                &bytes,
            );
        }
        EmbeddedWorkbookFixture::External => {
            write_zip_string(
                &mut writer,
                options,
                "ppt/charts/_rels/chart1.xml.rels",
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdWorkbook" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/package" Target="https://example.invalid/workbook.xlsx" TargetMode="External"/>
</Relationships>"#,
            );
        }
    }

    writer.finish().expect("finish embedded workbook pptx");
}

fn minimal_xlsx_bytes() -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
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
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
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
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"#,
    );
    writer.finish().expect("finish xlsx bytes").into_inner()
}

fn minimal_docx_bytes() -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "word/document.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p/></w:body></w:document>"#,
    );
    writer.finish().expect("finish docx bytes").into_inner()
}

fn write_zip_string<W: Write + Seek>(
    writer: &mut ZipWriter<W>,
    options: SimpleFileOptions,
    name: &str,
    body: &str,
) {
    writer.start_file(name, options).expect("write zip entry");
    writer.write_all(body.as_bytes()).expect("write zip data");
}

fn write_zip_bytes<W: Write + Seek>(
    writer: &mut ZipWriter<W>,
    options: SimpleFileOptions,
    name: &str,
    data: &[u8],
) {
    writer.start_file(name, options).expect("write zip entry");
    writer.write_all(data).expect("write zip data");
}
