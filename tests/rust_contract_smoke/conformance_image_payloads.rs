use super::*;

#[test]
fn conformance_check_matches_go_for_docx_image_payload_and_relationship_failures() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-image-payloads-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance image payload temp dir");

    let clean = temp_dir.join("docx-image-payload-clean.docx");
    write_docx_image_payload_package(&clean, false);
    assert_go_rust_conformance_check_match(&clean);

    let broken = temp_dir.join("docx-image-payload-broken.docx");
    write_docx_image_payload_package(&broken, true);
    assert_go_rust_repair_invariants_match(&broken);

    let report = rust_conformance_report(&broken);
    let repair = check_by_name(&report, "repair-invariants");
    let repair_json = serde_json::to_string(repair).expect("repair check JSON");
    for code in ["OOXML_IMAGE_RELATIONSHIP_REFERENCE", "OOXML_IMAGE_PAYLOAD"] {
        assert!(
            repair_json.contains(code),
            "repair-invariants should include {code}: {repair_json}"
        );
    }
    assert!(
        repair_json.contains("payload signature does not match"),
        "repair-invariants should include image payload mismatch: {repair_json}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

fn assert_go_rust_conformance_check_match(file: &Path) {
    let file = file.to_string_lossy().to_string();
    let args = ["--json", "conformance", "check", file.as_str()];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_code, go_code, "exit code for {file}");
    assert_eq!(rust_stderr, go_stderr, "stderr for {file}");
    assert_eq!(
        rust_stdout.map(scrub_file_fields),
        go_stdout.map(scrub_file_fields),
        "stdout for {file}"
    );
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

fn rust_conformance_report(file: &Path) -> Value {
    let file = file.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&["--json", "conformance", "check", file.as_str()]);
    assert_ne!(code, 0, "broken fixture should fail conformance check");
    assert_eq!(stderr, None, "broken fixture stderr");
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

fn write_docx_image_payload_package(dest: &Path, broken: bool) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create docx image payload fixture");
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
  <Default Extension="png" ContentType="image/png"/>
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

    let blips = if broken {
        r#"<w:p><w:r><w:drawing><wp:inline><wp:extent cx="914400" cy="914400"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rIdBadPayload"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
    <w:p><w:r><w:drawing><wp:inline><wp:extent cx="914400" cy="914400"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rIdMissingImage"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
    <w:p><w:r><w:drawing><wp:inline><wp:extent cx="914400" cy="914400"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rIdExternalEmbed"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
    <w:p><w:r><w:drawing><wp:inline><wp:extent cx="914400" cy="914400"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:link="rIdWrongContent"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    } else {
        r#"<w:p><w:r><w:drawing><wp:inline><wp:extent cx="914400" cy="914400"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rIdImage1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    };
    write_zip_string(
        &mut writer,
        options,
        "word/document.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
  xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
  xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    {blips}
    <w:sectPr/>
  </w:body>
</w:document>"#
        ),
    );

    let rels = if broken {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdBadPayload" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
  <Relationship Id="rIdExternalEmbed" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="https://example.invalid/image.png" TargetMode="External"/>
  <Relationship Id="rIdWrongContent" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="document.xml"/>
</Relationships>"#
    } else {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImage1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
</Relationships>"#
    };
    write_zip_string(&mut writer, options, "word/_rels/document.xml.rels", rels);
    let image = if broken {
        b"not actually a png".as_slice()
    } else {
        include_bytes!("../../testdata/test_image.png").as_slice()
    };
    write_zip_bytes(&mut writer, options, "word/media/image1.png", image);

    writer.finish().expect("finish docx image payload fixture");
}

fn write_zip_bytes(
    writer: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    name: &str,
    data: &[u8],
) {
    writer.start_file(name, options).expect("write zip entry");
    writer.write_all(data).expect("write zip data");
}
