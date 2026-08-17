// PPTX frozen mutation/render/verify contract tests live here while shared
// baseline and process helpers remain in the parent integration test crate.
use super::*;

include!("pptx/scaffold.rs");

const PPTX_NOTES_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
const PPTX_SLIDE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

fn rels_part_for_uri(uri: &str) -> String {
    let part = uri.trim_start_matches('/');
    let (dir, name) = part
        .rsplit_once('/')
        .unwrap_or_else(|| panic!("relationship source should be a package part: {uri}"));
    format!("{dir}/_rels/{name}.rels")
}

fn relationship_target_between_parts(source_uri: &str, target_uri: &str) -> String {
    let source = source_uri.trim_start_matches('/');
    let target = target_uri.trim_start_matches('/');
    let source_dirs: Vec<&str> = source
        .rsplit_once('/')
        .map(|(dir, _)| dir.split('/').filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    let target_parts: Vec<&str> = target.split('/').filter(|part| !part.is_empty()).collect();
    let common = source_dirs
        .iter()
        .zip(target_parts.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let mut parts = Vec::new();
    for _ in common..source_dirs.len() {
        parts.push("..".to_string());
    }
    for part in target_parts.iter().skip(common) {
        parts.push((*part).to_string());
    }
    if parts.is_empty() {
        target.rsplit('/').next().unwrap_or(target).to_string()
    } else {
        parts.join("/")
    }
}

fn scrub_created_at(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            for (key, item) in map.iter_mut() {
                if key == "createdAt" && item.as_str().is_some() {
                    *item = Value::String("[CREATED_AT]".to_string());
                } else {
                    *item = scrub_created_at(item.take());
                }
            }
            Value::Object(map)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(scrub_created_at).collect()),
        other => other,
    }
}

include!("pptx/translate.rs");

include!("pptx/render.rs");

fn run_ooxml_raw(args: &[&str]) -> (i32, String, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run Rust ooxml raw");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).expect("Rust stdout utf8"),
        String::from_utf8(output.stderr).expect("Rust stderr utf8"),
    )
}

fn run_ooxml_baseline_raw(args: &[&str]) -> (i32, String, String) {
    let output = std::process::Command::new(rust_repeat_or_comparison_binary())
        .args(args)
        .output()
        .expect("run Rust baseline ooxml raw");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).expect("Rust baseline stdout utf8"),
        String::from_utf8(output.stderr).expect("Rust baseline stderr utf8"),
    )
}

fn parse_raw_json(text: &str) -> Value {
    serde_json::from_str(text.trim()).unwrap_or_else(|err| {
        panic!("invalid raw JSON {err}: {text}");
    })
}

include!("pptx/place.rs");

include!("pptx/charts.rs");

include!("pptx/animations.rs");

include!("pptx/template.rs");

fn assert_strict_validate_succeeds(path: &str, label: &str) {
    let (code, stdout, stderr) = run_ooxml(&["validate", "--strict", path]);
    assert_eq!(code, 0, "{label} strict validate exit");
    assert_eq!(stderr, None, "{label} strict validate stderr");
    assert!(stdout.is_some(), "{label} strict validate stdout");
}

fn assert_conformance_check_runs(path: &str, label: &str) {
    let (_, stdout, stderr) = run_ooxml(&["--json", "conformance", "check", path]);
    assert_eq!(stderr, None, "{label} conformance check stderr");
    assert!(stdout.is_some(), "{label} conformance check stdout");
}

include!("pptx/xlsx_bindings.rs");

include!("pptx/legacy_baseline.rs");

include!("pptx/media.rs");

include!("pptx/replace.rs");

include!("pptx/notes.rs");

include!("pptx/shapes.rs");

include!("pptx/layouts.rs");

include!("pptx/slides.rs");

fn assert_baseline_rust_json_match(args: &[&str], label: &str) {
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(args);
    assert_eq!(rust_code, baseline_code, "{label} exit");
    assert_eq!(rust_stdout, baseline_stdout, "{label} stdout");
    assert_eq!(rust_stderr, baseline_stderr, "{label} stderr");
}

fn assert_baseline_rust_json_match_with_path_scrub(
    baseline_args: &[&str],
    rust_args: &[&str],
    baseline_path: &str,
    rust_path: &str,
    label: &str,
) {
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, baseline_code, "{label} exit");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust stdout"), rust_path, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline stdout"),
            baseline_path,
            "[OUT]"
        ),
        "{label} stdout"
    );
    assert_eq!(rust_stderr, baseline_stderr, "{label} stderr");
}

include!("pptx/text_fields.rs");

#[test]
fn pptx_theme_update_deck_readback_dry_run_and_errors_match_rust_baseline() {
    let fixture = "testdata/pptx/multi-layout/presentation.pptx";
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-theme-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx theme temp dir");

    assert_baseline_rust_json_match(
        &[
            "--json",
            "pptx",
            "theme",
            "update",
            fixture,
            "--color",
            "accent1=FF0000",
            "--major-font",
            "Georgia",
            "--minor-font",
            "Verdana",
            "--dry-run",
        ],
        "theme update dry-run",
    );

    let baseline_out = temp_dir.join("baseline-theme-update.pptx");
    let rust_out = temp_dir.join("rust-theme-update.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline theme update path");
    let rust_out_str = rust_out.to_str().expect("rust theme update path");
    let baseline_args = [
        "--json",
        "pptx",
        "theme",
        "update",
        fixture,
        "--color",
        "accent1=FF0000",
        "--major-font",
        "Georgia",
        "--minor-font",
        "Verdana",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "theme",
        "update",
        fixture,
        "--color",
        "accent1=FF0000",
        "--major-font",
        "Georgia",
        "--minor-font",
        "Verdana",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "theme update saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "theme update saved stderr");
    assert_eq!(
        rust_stdout.expect("rust theme update stdout"),
        baseline_stdout.expect("baseline theme update stdout"),
        "theme update saved stdout"
    );
    assert!(
        baseline_out.exists(),
        "Rust baseline theme update output missing"
    );
    assert!(rust_out.exists(), "Rust theme update output missing");

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "masters",
        "show",
        baseline_out_str,
        "--master",
        "1",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "masters",
        "show",
        rust_out_str,
        "--master",
        "1",
    ]);
    assert_eq!(rust_read_code, baseline_read_code, "theme readback exit");
    assert_eq!(
        rust_read_stderr, baseline_read_stderr,
        "theme readback stderr"
    );
    assert_eq!(
        rust_read_stdout.expect("rust theme readback"),
        baseline_read_stdout.expect("baseline theme readback"),
        "theme readback stdout"
    );

    for (label, args) in [
        (
            "theme update no updates",
            vec!["--json", "pptx", "theme", "update", fixture, "--dry-run"],
        ),
        (
            "theme update invalid color format",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--color",
                "accent1",
                "--dry-run",
            ],
        ),
        (
            "theme update invalid color name",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--color",
                "bad=FF0000",
                "--dry-run",
            ],
        ),
        (
            "theme update invalid hex",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--color",
                "accent1=ZZZZZZ",
                "--dry-run",
            ],
        ),
        (
            "theme update slide color oracle error",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--mode",
                "slide",
                "--slide",
                "1",
                "--color",
                "accent1=FF0000",
                "--dry-run",
            ],
        ),
        (
            "theme update slide font unsupported",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                fixture,
                "--mode",
                "slide",
                "--slide",
                "1",
                "--major-font",
                "Georgia",
                "--dry-run",
            ],
        ),
        (
            "theme update unsupported xlsx",
            vec![
                "--json",
                "pptx",
                "theme",
                "update",
                "testdata/xlsx/minimal-workbook/workbook.xlsx",
                "--color",
                "accent1=FF0000",
                "--dry-run",
            ],
        ),
    ] {
        assert_baseline_rust_json_match(&args, label);
    }
}

include!("pptx/tables.rs");

include!("pptx/comments.rs");

fn pptx_replace_text_source_sheet_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>42</v></c></row>
  </sheetData>
</worksheet>"#
}

fn pptx_replace_text_map_source_sheet_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>slide</t></is></c><c r="B1" t="inlineStr"><is><t>target</t></is></c><c r="C1" t="inlineStr"><is><t>text</t></is></c></row>
    <row r="2"><c r="A2"><v>1</v></c><c r="B2" t="inlineStr"><is><t>title</t></is></c><c r="C2" t="inlineStr"><is><t>Range Title</t></is></c></row>
    <row r="3"><c r="A3"><v>2</v></c><c r="B3" t="inlineStr"><is><t>body</t></is></c><c r="C3" t="inlineStr"><is><t>Range Body</t></is></c></row>
  </sheetData>
</worksheet>"#
}

fn pptx_update_source_sheet_xml_4x4() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:D4"/>
  <sheetData>
    <row r="1"><c r="A1"><f>SUM(B1:C1)</f><v>7</v></c><c r="B1" t="inlineStr"><is><t>Header B</t></is></c><c r="C1" t="inlineStr"><is><t>Header C</t></is></c><c r="D1" t="inlineStr"><is><t>D</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>42</v></c><c r="C2" t="inlineStr"><is><t>ok</t></is></c><c r="D2" t="inlineStr"><is><t>H</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>South</t></is></c><c r="B3"><v>55</v></c><c r="C3" t="inlineStr"><is><t>done</t></is></c><c r="D3" t="inlineStr"><is><t>L</t></is></c></row>
    <row r="4"><c r="A4" t="inlineStr"><is><t>M</t></is></c><c r="B4" t="inlineStr"><is><t>N</t></is></c><c r="C4" t="inlineStr"><is><t>O</t></is></c><c r="D4" t="inlineStr"><is><t>P</t></is></c></row>
  </sheetData>
</worksheet>"#
}

fn write_pptx_text_map_table_xlsx(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create pptx text map table xlsx");
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
  <Override PartName="/xl/tables/table1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>
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
    <sheet name="Data" sheetId="1" r:id="rId1"/>
  </sheets>
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
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:C2"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>slide</t></is></c><c r="B1" t="inlineStr"><is><t>target</t></is></c><c r="C1" t="inlineStr"><is><t>text</t></is></c></row>
    <row r="2"><c r="A2"><v>1</v></c><c r="B2" t="inlineStr"><is><t>title</t></is></c><c r="C2" t="inlineStr"><is><t>Table Title</t></is></c></row>
  </sheetData>
  <tableParts count="1"><tablePart r:id="rId1"/></tableParts>
</worksheet>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/_rels/sheet1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/tables/table1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="TextMap" displayName="TextMap" ref="A1:C2" headerRowCount="1" totalsRowShown="0">
  <autoFilter ref="A1:C2"/>
  <tableColumns count="3">
    <tableColumn id="1" name="slide"/>
    <tableColumn id="2" name="target"/>
    <tableColumn id="3" name="text"/>
  </tableColumns>
  <tableStyleInfo name="TableStyleMedium2" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/>
</table>"#,
    );
    writer.finish().expect("finish pptx text map table xlsx");
}
