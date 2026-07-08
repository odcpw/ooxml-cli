use super::*;

#[test]
fn conformance_coverage_keeps_relationship_read_error_in_rust_baseline_parity() {
    let args = ["--json", "conformance", "coverage"];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_code, baseline_code, "exit code for {args:?}");
    assert_eq!(rust_stderr, baseline_stderr, "stderr for {args:?}");
    assert_eq!(rust_stdout, baseline_stdout, "stdout for {args:?}");

    let report = rust_stdout.expect("coverage report");
    let body = serde_json::to_string(&report).expect("coverage JSON");
    assert!(
        body.contains("OOXML_RELS_READ_ERROR"),
        "relationship read-error diagnostic should remain in static coverage: {body}"
    );
}

#[test]
fn validation_matches_relationship_targets_percent_decoded_and_case_insensitive() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-opc-rel-target-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let package = temp_dir.join("relationship-target.docx");
    write_minimal_docx(
        &package,
        None,
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/caf%C3%A9.PNG"/>
</Relationships>"#,
        ),
        &[("word/media/café.png", b"png".to_vec())],
    );

    let package = package.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&["--json", "validate", "--strict", &package]);
    assert_eq!(
        code, 0,
        "strict validate should accept OPC part equivalence"
    );
    assert_eq!(stderr, None, "strict validate stderr");
    let report = stdout.expect("strict validate stdout");
    assert_eq!(report["valid"], true);
    assert_no_diagnostic_code(&report, "REL_DANGLING_TARGET");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn conformance_accepts_explicit_internal_target_mode() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-opc-internal-target-mode-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let package = temp_dir.join("internal-target-mode.docx");
    write_minimal_docx(
        &package,
        None,
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png" TargetMode="Internal"/>
</Relationships>"#,
        ),
        &[("word/media/image1.png", minimal_png_bytes())],
    );

    let package = package.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&["--json", "conformance", "check", &package]);
    assert_eq!(code, 0, "conformance check exit");
    assert_eq!(stderr, None, "conformance check stderr");
    let report = stdout.expect("conformance check stdout");
    assert_eq!(report["status"], "passed");
    assert_no_repair_diagnostic_code(&report, "OOXML_RELATIONSHIP_TARGET_MODE");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn conformance_default_extension_matching_is_case_insensitive() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-opc-content-types-case-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let package = temp_dir.join("default-extension-case.docx");
    write_minimal_docx(
        &package,
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
        ),
        None,
        &[("word/attachments/photo.JPG", b"jpg".to_vec())],
    );

    let package = package.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&["--json", "conformance", "check", &package]);
    assert_eq!(code, 0, "conformance check exit");
    assert_eq!(stderr, None, "conformance check stderr");
    let report = stdout.expect("conformance check stdout");
    assert_eq!(report["status"], "passed");
    assert_no_repair_diagnostic_code(&report, "OOXML_CONTENT_TYPES_PART_UNMAPPED");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn validate_reports_malformed_relationship_part_as_diagnostic() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-opc-malformed-rels-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let package = temp_dir.join("malformed-rels.docx");
    write_minimal_docx(
        &package,
        None,
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdBroken" Target="media/image1.png""#,
        ),
        &[],
    );

    let package = package.to_string_lossy().to_string();
    let (code, stdout, stderr) = run_ooxml(&["--json", "validate", "--strict", &package]);
    assert_eq!(code, 5, "strict validate exit");
    assert_eq!(stderr, None, "strict validate stderr");
    let report = stdout.expect("strict validate stdout");
    assert_eq!(report["valid"], false);
    assert_diagnostic_code(&report, "REL_MALFORMED");

    let _ = fs::remove_dir_all(&temp_dir);
}

fn write_minimal_docx(
    path: &Path,
    content_types_xml: Option<&str>,
    document_rels_xml: Option<&str>,
    extra_entries: &[(&str, Vec<u8>)],
) {
    let output = File::create(path).expect("create package");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        content_types_xml.unwrap_or(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
        ),
    );
    write_zip_string(
        &mut writer,
        options,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdDocument" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "word/document.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p><w:sectPr/></w:body>
</w:document>"#,
    );
    if let Some(document_rels_xml) = document_rels_xml {
        write_zip_string(
            &mut writer,
            options,
            "word/_rels/document.xml.rels",
            document_rels_xml,
        );
    }
    for (name, data) in extra_entries {
        writer.start_file(*name, options).expect("start extra part");
        writer.write_all(data).expect("write extra part");
    }
    writer.finish().expect("finish package");
}

fn assert_diagnostic_code(report: &Value, code: &str) {
    let diagnostics = report["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"].as_str() == Some(code)),
        "missing {code}: {diagnostics:?}"
    );
}

fn assert_no_diagnostic_code(report: &Value, code: &str) {
    let diagnostics = report["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic["code"].as_str() != Some(code)),
        "unexpected {code}: {diagnostics:?}"
    );
}

fn assert_no_repair_diagnostic_code(report: &Value, code: &str) {
    let repair = report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["name"].as_str() == Some("repair-invariants"))
        .expect("repair-invariants check");
    let diagnostics = repair["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic["code"].as_str() != Some(code)),
        "unexpected {code}: {diagnostics:?}"
    );
}

fn minimal_png_bytes() -> Vec<u8> {
    vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 0, 1, 0, 0, 5, 0, 1,
        13, 10, 45, 180, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]
}
