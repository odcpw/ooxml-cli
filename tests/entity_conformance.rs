use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const ENTITY_ESCAPED: &str = r#"A &amp; B &lt;x&gt; &gt; &quot; &apos; &#8217; &#x1F600;"#;

fn decoded_entity_text() -> String {
    "A & B <x> > \" ' \u{2019} \u{1F600}".to_string()
}

fn temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ooxml-{name}-{}-{suffix}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn run_json(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml");
    assert!(
        output.status.success(),
        "ooxml {args:?} failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "invalid JSON from {args:?}: {err}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn read_zip_string(path: &Path, entry_name: &str) -> String {
    let file = File::open(path).expect("open zip");
    let mut archive = ZipArchive::new(file).expect("read zip");
    let mut entry = archive.by_name(entry_name).expect("open zip entry");
    let mut text = String::new();
    entry.read_to_string(&mut text).expect("read zip entry");
    text
}

fn rewrite_zip_fixture<F>(source: &Path, dest: &Path, mut mutator: F)
where
    F: FnMut(&str, Vec<u8>) -> Option<(String, Vec<u8>)>,
{
    let input = File::open(source).expect("open source fixture");
    let mut archive = ZipArchive::new(input).expect("read source fixture zip");
    let output = File::create(dest).expect("create rewritten fixture");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("read source fixture entry");
        if entry.is_dir() {
            writer
                .add_directory(entry.name(), options)
                .expect("copy fixture directory");
            continue;
        }
        let source_name = entry.name().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).expect("read fixture entry");
        if let Some((dest_name, data)) = mutator(&source_name, data) {
            writer
                .start_file(dest_name, options)
                .expect("write fixture entry");
            writer.write_all(&data).expect("write fixture data");
        }
    }
    writer.finish().expect("finish rewritten fixture");
}

fn replace_entry_text(source: &Path, dest: &Path, entry_name: &str, from: &str, to: &str) {
    rewrite_zip_fixture(source, dest, |name, data| {
        let data = if name == entry_name {
            String::from_utf8(data)
                .expect("entry utf8")
                .replace(from, to)
                .into_bytes()
        } else {
            data
        };
        Some((name.to_string(), data))
    });
}

fn write_zip_string(
    writer: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    entry_name: &str,
    text: &str,
) {
    writer.start_file(entry_name, options).expect("zip entry");
    writer.write_all(text.as_bytes()).expect("zip text");
}

fn write_docx_with_body(dest: &Path, body_inner: &str) {
    let output = File::create(dest).expect("create docx");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
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
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "word/document.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
{body_inner}
    <w:sectPr/>
  </w:body>
</w:document>"#
        ),
    );
    writer.finish().expect("finish docx");
}

#[test]
fn pptx_entity_text_survives_readback_and_matched_node_rewrite() {
    let temp = temp_dir("entity-pptx");
    let scaffold = temp.join("scaffold.pptx");
    let scaffold_str = scaffold.to_string_lossy().to_string();
    run_json(&[
        "--json",
        "pptx",
        "scaffold",
        "--out",
        &scaffold_str,
        "--title",
        "Entity Sentinel Target",
        "--subtitle",
        "Subtitle",
    ]);

    let replace_input = temp.join("replace-input.pptx");
    replace_entry_text(
        &scaffold,
        &replace_input,
        "ppt/slides/slide1.xml",
        "Entity Sentinel Target",
        &format!("{ENTITY_ESCAPED} Target"),
    );
    let replace_output = temp.join("replace-output.pptx");
    let replace_input_str = replace_input.to_string_lossy().to_string();
    let replace_output_str = replace_output.to_string_lossy().to_string();
    run_json(&[
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        &replace_input_str,
        "--match-text",
        "Target",
        "--new-text",
        "Done",
        "--expect-count",
        "1",
        "--out",
        &replace_output_str,
    ]);
    let slide_xml = read_zip_string(&replace_output, "ppt/slides/slide1.xml");
    assert!(
        slide_xml.contains("A &amp; B &lt;x&gt; &gt;"),
        "rewritten slide dropped escaped entity text: {slide_xml}"
    );
    assert!(
        !slide_xml.contains("&amp;#8217;") && !slide_xml.contains("&amp;#x1F600;"),
        "rewritten slide double-escaped numeric refs: {slide_xml}"
    );
    let extracted = run_json(&["--json", "pptx", "extract", "text", &replace_output_str]);
    assert_eq!(
        extracted["slides"][0]["shapes"][0]["text"]["plainText"],
        format!("{} Done", decoded_entity_text()),
        "replace readback did not preserve decoded entity text"
    );

    let notes_input = temp.join("notes-entities.pptx");
    replace_entry_text(
        Path::new("testdata/pptx/notes-slide/presentation.pptx"),
        &notes_input,
        "ppt/notesSlides/notesSlide1.xml",
        "These are the speaker notes for this slide.",
        ENTITY_ESCAPED,
    );
    let notes_input_str = notes_input.to_string_lossy().to_string();
    let notes = run_json(&[
        "--json",
        "pptx",
        "notes",
        "show",
        &notes_input_str,
        "--slide",
        "2",
    ]);
    assert_eq!(
        notes["notes"]["paragraphs"][0]["text"],
        decoded_entity_text()
    );

    let commented = temp.join("commented.pptx");
    let commented_str = commented.to_string_lossy().to_string();
    run_json(&[
        "--json",
        "pptx",
        "comments",
        "add",
        "testdata/pptx/title-content/presentation.pptx",
        "--slide",
        "1",
        "--author",
        "Alice",
        "--initials",
        "AB",
        "--text",
        "ENTITY_PLACEHOLDER",
        "--date",
        "2026-07-08T00:00:00Z",
        "--out",
        &commented_str,
    ]);
    let comments_input = temp.join("comments-entities.pptx");
    replace_entry_text(
        &commented,
        &comments_input,
        "ppt/comments/comment1.xml",
        "ENTITY_PLACEHOLDER",
        ENTITY_ESCAPED,
    );
    let comments_input_str = comments_input.to_string_lossy().to_string();
    let comments = run_json(&[
        "--json",
        "pptx",
        "comments",
        "list",
        &comments_input_str,
        "--slide",
        "1",
    ]);
    assert_eq!(
        comments["slides"][0]["comments"][0]["text"],
        decoded_entity_text()
    );

    let table_input = temp.join("table-entities.pptx");
    replace_entry_text(
        Path::new("testdata/pptx/table-simple/presentation.pptx"),
        &table_input,
        "ppt/slides/slide2.xml",
        "R1C1",
        ENTITY_ESCAPED,
    );
    let table_input_str = table_input.to_string_lossy().to_string();
    let table = run_json(&[
        "--json",
        "pptx",
        "tables",
        "show",
        &table_input_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(table["tables"][0]["cells"][1][1], decoded_entity_text());
}

#[test]
fn xlsx_entity_formulas_survive_list_and_unrelated_update() {
    let temp = temp_dir("entity-xlsx");
    let scaffold = temp.join("scaffold.xlsx");
    let scaffold_str = scaffold.to_string_lossy().to_string();
    run_json(&["--json", "xlsx", "scaffold", "--out", &scaffold_str]);

    let formula = format!("A1&lt;10 {ENTITY_ESCAPED}");
    let expected_formula = format!("A1<10 {}", decoded_entity_text());
    let workbook_with_names = temp.join("names.xlsx");
    rewrite_zip_fixture(&scaffold, &workbook_with_names, |name, data| {
        let data = if name == "xl/workbook.xml" {
            let xml = String::from_utf8(data).expect("workbook xml utf8");
            xml.replace(
                "</sheets>",
                &format!(
                    r#"</sheets><definedNames><definedName name="EntityName">{formula}</definedName></definedNames>"#
                ),
            )
            .into_bytes()
        } else {
            data
        };
        Some((name.to_string(), data))
    });
    let names_str = workbook_with_names.to_string_lossy().to_string();
    let names = run_json(&["--json", "xlsx", "names", "list", &names_str]);
    assert_eq!(names["names"][0]["ref"], expected_formula);

    let workbook_with_dv = temp.join("dv.xlsx");
    rewrite_zip_fixture(&scaffold, &workbook_with_dv, |name, data| {
        let data = if name == "xl/worksheets/sheet1.xml" {
            let xml = String::from_utf8(data).expect("worksheet xml utf8");
            xml.replace(
                "</worksheet>",
                &format!(
                    r#"<dataValidations count="1"><dataValidation type="custom" sqref="A1:A5" prompt="Line1&#10;Line2"><formula1>{formula}</formula1></dataValidation></dataValidations></worksheet>"#
                ),
            )
            .into_bytes()
        } else {
            data
        };
        Some((name.to_string(), data))
    });
    let dv_str = workbook_with_dv.to_string_lossy().to_string();
    let listed = run_json(&[
        "--json",
        "xlsx",
        "data-validations",
        "list",
        &dv_str,
        "--sheet",
        "Sheet1",
    ]);
    assert_eq!(listed["dataValidations"][0]["formula1"], expected_formula);
    assert_eq!(
        listed["dataValidations"][0]["prompt"],
        Value::String("Line1\nLine2".to_string())
    );

    let updated = temp.join("dv-updated.xlsx");
    let updated_str = updated.to_string_lossy().to_string();
    run_json(&[
        "--json",
        "xlsx",
        "data-validations",
        "update",
        &dv_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A5",
        "--error-message",
        "Still valid",
        "--out",
        &updated_str,
    ]);
    let updated_list = run_json(&[
        "--json",
        "xlsx",
        "data-validations",
        "list",
        &updated_str,
        "--sheet",
        "Sheet1",
    ]);
    assert_eq!(
        updated_list["dataValidations"][0]["formula1"],
        expected_formula
    );
    let sheet_xml = read_zip_string(&updated, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains("A1&lt;10 A &amp; B &lt;x&gt; &gt;"),
        "updated validation did not re-escape formula text: {sheet_xml}"
    );
    assert!(
        !sheet_xml.contains("&amp;#8217;") && !sheet_xml.contains("&amp;#x1F600;"),
        "updated validation double-escaped numeric refs: {sheet_xml}"
    );
}

#[test]
fn docx_entity_text_and_untouched_runs_survive_replace() {
    let temp = temp_dir("entity-docx");
    let input = temp.join("input.docx");
    write_docx_with_body(
        &input,
        &format!(
            r#"    <w:p><w:r><w:t xml:space="preserve">{ENTITY_ESCAPED} </w:t></w:r><w:r><w:t>TARGET</w:t></w:r></w:p>"#
        ),
    );
    let input_str = input.to_string_lossy().to_string();
    let text = run_json(&["--json", "docx", "text", &input_str]);
    assert_eq!(
        text["blocks"][0]["text"],
        format!("{} TARGET", decoded_entity_text())
    );

    let output = temp.join("output.docx");
    let output_str = output.to_string_lossy().to_string();
    run_json(&[
        "--json",
        "docx",
        "replace",
        &input_str,
        "--find",
        "TARGET",
        "--replace",
        "DONE",
        "--out",
        &output_str,
    ]);
    let document_xml = read_zip_string(&output, "word/document.xml");
    assert!(
        !document_xml.contains("&amp;#8217;") && !document_xml.contains("&amp;#x1F600;"),
        "docx replace double-escaped untouched numeric refs: {document_xml}"
    );
    let replaced = run_json(&["--json", "docx", "text", &output_str]);
    assert_eq!(
        replaced["blocks"][0]["text"],
        format!("{} DONE", decoded_entity_text())
    );
}
