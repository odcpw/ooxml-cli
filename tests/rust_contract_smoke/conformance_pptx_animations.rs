use super::*;
use serde_json::Value;
use std::path::Path;

#[test]
fn conformance_check_matches_rust_baseline_for_pptx_animation_targets() {
    // Provenance: deterministic synthetic PPTX packages generated in this test,
    // compared against the Rust baseline CLI oracle's repair-invariants check.
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-pptx-animations-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance pptx animations temp dir");

    let broken = temp_dir.join("animation-targets-broken.pptx");
    write_animation_target_pptx(
        &broken,
        &slide_xml(
            r#"
      <p:set><p:cBhvr><p:tgtEl><p:spTgt spid="2"/></p:tgtEl></p:cBhvr></p:set>
      <p:set><p:cBhvr><p:tgtEl><p:spTgt spid="99"/></p:tgtEl></p:cBhvr></p:set>
      <p:set><p:cBhvr><p:tgtEl><p:spTgt spid="bad"/></p:tgtEl></p:cBhvr></p:set>
      <p:set><p:cBhvr><p:tgtEl><p:spTgt spid="-1"/></p:tgtEl></p:cBhvr></p:set>
      <p:set><p:cBhvr><p:tgtEl><p:spTgt/></p:tgtEl></p:cBhvr></p:set>"#,
        ),
    );
    let repair = assert_baseline_rust_repair_invariants_match(&broken);
    assert_repair_message_contains(&repair, "missing slide shape id 99");
    assert_repair_message_contains(&repair, r#"invalid spid "bad""#);
    assert_repair_message_contains(&repair, r#"invalid spid "-1""#);
    assert_repair_message_contains(&repair, "missing required spid");

    let clean = temp_dir.join("animation-targets-clean.pptx");
    write_animation_target_pptx(
        &clean,
        &slide_xml(
            r#"
      <p:set><p:cBhvr><p:tgtEl><p:spTgt spid="2"/></p:tgtEl></p:cBhvr></p:set>"#,
        ),
    );
    let repair = assert_baseline_rust_repair_invariants_match(&clean);
    let body = serde_json::to_string(&repair).expect("repair JSON");
    assert!(
        !body.contains("PPTX_ANIMATION_TARGET_REFERENCE"),
        "clean animation target fixture should not emit PPTX_ANIMATION_TARGET_REFERENCE: {body}"
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

fn assert_repair_message_contains(repair: &Value, needle: &str) {
    let diagnostics = repair["diagnostics"].as_array().expect("diagnostics array");
    let body = serde_json::to_string(repair).expect("repair JSON");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic["code"].as_str() == Some("PPTX_ANIMATION_TARGET_REFERENCE")
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains(needle))
        }),
        "repair-invariants output should include {needle}: {body}"
    );
}

fn write_animation_target_pptx(dest: &Path, slide_xml: &str) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create pptx animation fixture");
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
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
</Types>"#,
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
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst>
    <p:sldId id="256" r:id="rId1"/>
  </p:sldIdLst>
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
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#,
    );
    write_zip_string(&mut writer, options, "ppt/slides/slide1.xml", slide_xml);
    writer.finish().expect("finish pptx animation fixture");
}

fn slide_xml(animation_targets: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name="Group"/></p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/></p:nvSpPr><p:spPr/></p:sp>
    </p:spTree>
  </p:cSld>
  <p:timing>
    <p:tnLst><p:par><p:cTn><p:childTnLst>{animation_targets}
    </p:childTnLst></p:cTn></p:par></p:tnLst>
  </p:timing>
</p:sld>"#
    )
}
