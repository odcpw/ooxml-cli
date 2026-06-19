use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

static GO_OOXML_BIN: OnceLock<PathBuf> = OnceLock::new();

fn baseline() -> Value {
    serde_json::from_str(include_str!(
        "../testdata/golden/rust-port-contract/baseline.json"
    ))
    .expect("baseline JSON")
}

fn run_ooxml(args: &[&str]) -> (i32, Option<Value>, Option<Value>) {
    run_ooxml_with_env(args, &[])
}

fn run_ooxml_with_input(args: &[&str], input: &str) -> (i32, Option<Value>, Option<Value>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run Rust ooxml with stdin");
    child
        .stdin
        .take()
        .expect("Rust ooxml stdin")
        .write_all(input.as_bytes())
        .expect("write Rust ooxml stdin");
    let output = child.wait_with_output().expect("wait Rust ooxml");
    let code = output.status.code().unwrap_or(-1);
    let stdout = parse_json(&output.stdout);
    let stderr = parse_json(&output.stderr);
    (code, stdout, stderr)
}

fn run_ooxml_with_env(args: &[&str], envs: &[(&str, &str)]) -> (i32, Option<Value>, Option<Value>) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .envs(envs.iter().copied())
        .output()
        .expect("run Rust ooxml");
    let code = output.status.code().unwrap_or(-1);
    let stdout = parse_json(&output.stdout);
    let stderr = parse_json(&output.stderr);
    (code, stdout, stderr)
}

fn run_go_ooxml(args: &[&str]) -> (i32, Option<Value>, Option<Value>) {
    let output = Command::new(go_ooxml_binary())
        .args(args)
        .env("GOCACHE", "/tmp/ooxml-go-build")
        .output()
        .expect("run Go ooxml oracle");
    let code = output.status.code().unwrap_or(-1);
    let stdout = parse_json(&output.stdout);
    let stderr = parse_json(&output.stderr);
    (code, stdout, stderr)
}

fn run_go_ooxml_with_input(args: &[&str], input: &str) -> (i32, Option<Value>, Option<Value>) {
    let mut child = Command::new(go_ooxml_binary())
        .args(args)
        .env("GOCACHE", "/tmp/ooxml-go-build")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run Go ooxml oracle with stdin");
    child
        .stdin
        .take()
        .expect("Go ooxml stdin")
        .write_all(input.as_bytes())
        .expect("write Go ooxml stdin");
    let output = child.wait_with_output().expect("wait Go ooxml oracle");
    let code = output.status.code().unwrap_or(-1);
    let stdout = parse_json(&output.stdout);
    let stderr = parse_json(&output.stderr);
    (code, stdout, stderr)
}

fn assert_go_rust_match(args: &[&str]) {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(args);
    assert_eq!(rust_code, go_code, "exit code for {args:?}");
    assert_eq!(rust_stderr, go_stderr, "stderr for {args:?}");
    assert_eq!(rust_stdout, go_stdout, "stdout for {args:?}");
}

fn scrub_docx_dynamic_handles(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            for (key, item) in map.iter_mut() {
                if key == "paraId" && item.as_str().is_some() {
                    *item = Value::String("[PARA_ID]".to_string());
                } else {
                    *item = scrub_docx_dynamic_handles(item.take());
                }
                if key == "handle"
                    && let Some(handle) = item.as_str()
                    && handle.starts_with("H:docx/pt:doc/para:m:")
                {
                    *item = Value::String("H:docx/pt:doc/para:m:[PARA_ID]".to_string());
                }
            }
            Value::Object(map)
        }
        Value::Array(items) => {
            Value::Array(items.into_iter().map(scrub_docx_dynamic_handles).collect())
        }
        other => other,
    }
}

fn scrub_file_fields(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            for (key, item) in map.iter_mut() {
                if key == "file" && item.as_str().is_some() {
                    *item = Value::String("[FILE]".to_string());
                } else {
                    *item = scrub_file_fields(item.take());
                }
            }
            Value::Object(map)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(scrub_file_fields).collect()),
        other => other,
    }
}

fn go_ooxml_binary() -> &'static PathBuf {
    GO_OOXML_BIN.get_or_init(|| {
        let binary = std::env::temp_dir().join(format!("ooxml-go-oracle-{}", std::process::id()));
        let output = Command::new("go")
            .args(["build", "-o"])
            .arg(&binary)
            .arg("./cmd/ooxml")
            .env("GOCACHE", "/tmp/ooxml-go-build")
            .output()
            .expect("build Go ooxml oracle");
        assert!(
            output.status.success(),
            "build Go ooxml oracle failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        binary
    })
}

fn parse_json(bytes: &[u8]) -> Option<Value> {
    let text = std::str::from_utf8(bytes).expect("utf8").trim();
    if text.is_empty() {
        None
    } else {
        Some(serde_json::from_str(text).unwrap_or_else(|err| {
            panic!("invalid JSON {err}: {text}");
        }))
    }
}

fn write_relocated_xlsx_main_part(dest: &Path) {
    write_relocated_xlsx_main_part_with_content_type(
        dest,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
    );
}

fn write_relocated_xlsx_main_part_with_content_type(dest: &Path, workbook_content_type: &str) {
    rewrite_zip_fixture(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        dest,
        |name, data| {
            let (name, data) = match name {
                "xl/workbook.xml" => ("xl/books/book.xml".to_string(), data),
                "xl/_rels/workbook.xml.rels" => (
                    "xl/books/_rels/book.xml.rels".to_string(),
                    replace_ascii(
                        data,
                        "Target=\"worksheets/sheet1.xml\"",
                        "Target=\"../worksheets/sheet1.xml\"",
                    ),
                ),
                "_rels/.rels" => (
                    name.to_string(),
                    replace_ascii(
                        data,
                        "Target=\"xl/workbook.xml\"",
                        "Target=\"xl/books/book.xml\"",
                    ),
                ),
                "[Content_Types].xml" => (
                    name.to_string(),
                    replace_ascii(
                        replace_ascii(data, "/xl/workbook.xml", "/xl/books/book.xml"),
                        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
                        workbook_content_type,
                    ),
                ),
                _ => (name.to_string(), data),
            };
            Some((name, data))
        },
    );
}

fn write_relocated_docx_main_part(dest: &Path) {
    write_relocated_docx_main_part_with_content_type(
        dest,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
    );
}

fn write_relocated_docx_main_part_with_content_type(dest: &Path, document_content_type: &str) {
    rewrite_zip_fixture("testdata/docx/minimal/document.docx", dest, |name, data| {
        let (name, data) = match name {
            "word/document.xml" => ("word/main/document.xml".to_string(), data),
            "_rels/.rels" => (
                name.to_string(),
                replace_ascii(
                    data,
                    "Target=\"word/document.xml\"",
                    "Target=\"word/main/document.xml\"",
                ),
            ),
            "[Content_Types].xml" => (
                name.to_string(),
                replace_ascii(
                    replace_ascii(data, "/word/document.xml", "/word/main/document.xml"),
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
                    document_content_type,
                ),
            ),
            _ => (name.to_string(), data),
        };
        Some((name, data))
    });
}

fn write_nested_table_docx(dest: &Path) {
    rewrite_zip_fixture("testdata/docx/minimal/document.docx", dest, |name, data| {
        let data = if name == "word/document.xml" {
            br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p><w:r><w:t>Outer</w:t></w:r></w:p>
          <w:tbl>
            <w:tr>
              <w:tc><w:p><w:r><w:t>Inner</w:t></w:r></w:p></w:tc>
            </w:tr>
          </w:tbl>
        </w:tc>
        <w:tc><w:p><w:r><w:t>B1</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>A2</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>B2</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
  </w:body>
</w:document>"#
                .to_vec()
        } else {
            data
        };
        Some((name.to_string(), data))
    });
}

fn write_docx_with_body(dest: &Path, body_inner: &str) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
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

fn rewrite_zip_fixture<F>(source: &str, dest: &Path, mut mutator: F)
where
    F: FnMut(&str, Vec<u8>) -> Option<(String, Vec<u8>)>,
{
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let input = File::open(source).expect("open source fixture");
    let mut archive = ZipArchive::new(input).expect("read source fixture zip");
    let output = File::create(dest).expect("create rewritten fixture");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).expect("read source fixture entry");
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

fn write_unknown_package(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create unknown package");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer
        .start_file("[Content_Types].xml", options)
        .expect("write content types");
    writer
        .write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#,
        )
        .expect("write unknown content types");
    writer.finish().expect("finish unknown package");
}

fn write_table_xlsx(dest: &Path) {
    write_table_xlsx_with_sheet(dest, "Data");
}

fn write_table_xlsx_with_sheet(dest: &Path, sheet_name: &str) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create table xlsx");
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
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="{sheet_name}" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#
        ),
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
  <dimension ref="A1:B3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>East</t></is></c><c r="B2"><v>10</v></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>West</t></is></c><c r="B3"><f>SUM(B2:B2)*2</f><v>20</v></c></row>
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
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="Sales" displayName="Sales" ref="A1:B3" headerRowCount="1" totalsRowShown="0">
  <autoFilter ref="A1:B3"/>
  <tableColumns count="2">
    <tableColumn id="1" name="Region"/>
    <tableColumn id="2" name="Amount"/>
  </tableColumns>
  <tableStyleInfo name="TableStyleMedium2" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/>
</table>"#,
    );
    writer.finish().expect("finish table xlsx");
}

fn write_zip_string(
    writer: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    name: &str,
    body: &str,
) {
    writer.start_file(name, options).expect("write zip entry");
    writer.write_all(body.as_bytes()).expect("write zip data");
}

fn read_zip_string(path: &Path, name: &str) -> String {
    let input = File::open(path).expect("open xlsx");
    let mut archive = ZipArchive::new(input).expect("read xlsx");
    let mut entry = archive.by_name(name).expect("read zip entry");
    let mut body = String::new();
    entry.read_to_string(&mut body).expect("zip entry utf8");
    body
}

fn zip_entry_exists(path: &Path, name: &str) -> bool {
    let input = File::open(path).expect("open zip");
    let mut archive = ZipArchive::new(input).expect("read zip");
    archive.by_name(name).is_ok()
}

fn write_simple_xlsx_with_sheet_xml(dest: &Path, sheet_xml: &str) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create xlsx");
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
    write_zip_string(&mut writer, options, "xl/worksheets/sheet1.xml", sheet_xml);
    writer.finish().expect("finish xlsx");
}

fn write_preservation_xlsx(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create xlsx");
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
  <Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
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
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/sharedStrings.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="1" uniqueCount="1"><si><t>Preserve me</t></si></sst>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/styles.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <numFmts count="1"><numFmt numFmtId="164" formatCode="yyyy-mm-dd"/></numFmts>
  <fonts count="1"><font/></fonts><fills count="1"><fill/></fills><borders count="1"><border/></borders>
  <cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>
  <cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/><xf numFmtId="164" fontId="0" fillId="0" borderId="0" applyNumberFormat="1"/></cellXfs>
</styleSheet>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C1"/>
  <sheetData>
    <row r="1" spans="1:3"><c r="A1" t="s"><v>0</v></c><c r="B1" s="1"><v>45123</v></c><c r="C1"><f>B1*2</f><v>90246</v></c></row>
  </sheetData>
</worksheet>"#,
    );
    writer.finish().expect("finish preservation xlsx");
}

fn replace_ascii(data: Vec<u8>, from: &str, to: &str) -> Vec<u8> {
    String::from_utf8(data)
        .expect("fixture xml utf8")
        .replace(from, to)
        .into_bytes()
}

fn assert_generated_inspect_edge_cases_match_go() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-inspect-parity-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let relocated_xlsx = temp_dir.join("relocated-workbook.xlsx");
    write_relocated_xlsx_main_part(&relocated_xlsx);
    let relocated_xlsx = relocated_xlsx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "inspect", &relocated_xlsx]);

    let relocated_docx = temp_dir.join("relocated-document.docx");
    write_relocated_docx_main_part(&relocated_docx);
    let relocated_docx = relocated_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "inspect", &relocated_docx]);

    let relocated_macro_xlsx = temp_dir.join("relocated-macro-workbook.xlsm");
    write_relocated_xlsx_main_part_with_content_type(
        &relocated_macro_xlsx,
        "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
    );
    let relocated_macro_xlsx = relocated_macro_xlsx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "inspect", &relocated_macro_xlsx]);

    let relocated_macro_docx = temp_dir.join("relocated-macro-document.docm");
    write_relocated_docx_main_part_with_content_type(
        &relocated_macro_docx,
        "application/vnd.ms-word.document.macroEnabled.main+xml",
    );
    let relocated_macro_docx = relocated_macro_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "inspect", &relocated_macro_docx]);

    let malformed_xlsx = temp_dir.join("malformed-workbook.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &malformed_xlsx,
        |name, data| {
            let data = if name == "xl/workbook.xml" {
                b"<workbook><sheets><sheet".to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let malformed_xlsx = malformed_xlsx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "inspect", &malformed_xlsx]);

    let malformed_docx = temp_dir.join("malformed-document.docx");
    rewrite_zip_fixture(
        "testdata/docx/minimal/document.docx",
        &malformed_docx,
        |name, data| {
            let data = if name == "word/document.xml" {
                br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p>"#.to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let malformed_docx = malformed_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "inspect", &malformed_docx]);

    let unknown_package = temp_dir.join("unknown-package.zip");
    write_unknown_package(&unknown_package);
    let unknown_package = unknown_package.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "inspect", &unknown_package]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn frozen_cli_slice_matches_go_baseline() {
    let baseline = baseline();
    for case in baseline["cli"].as_array().expect("cli array") {
        let args: Vec<&str> = case["args"]
            .as_array()
            .expect("args")
            .iter()
            .map(|value| value.as_str().expect("arg string"))
            .collect();
        let (code, stdout, stderr) = run_ooxml(&args);
        assert_eq!(code, case["exitCode"], "exit code for {:?}", args);
        assert_eq!(
            stdout,
            nullable(&case["stdoutJson"]),
            "stdout for {:?}",
            args
        );
        assert_eq!(
            stderr,
            nullable(&case["stderrJson"]),
            "stderr for {:?}",
            args
        );
    }
}

#[test]
fn xlsx_ranges_export_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "xlsx",
            "ranges",
            "export",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "1",
            "--range",
            "A1:B2",
        ],
        vec![
            "--json",
            "xlsx",
            "ranges",
            "export",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "1",
            "--range",
            "A1:B2",
            "--include-types",
        ],
        vec![
            "--json",
            "xlsx",
            "ranges",
            "export",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
            "--sheet",
            "Types",
            "--range",
            "A1:H2",
            "--include-types",
            "--include-formulas",
            "--include-formats",
        ],
        vec![
            "--json",
            "xlsx",
            "ranges",
            "export",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "1",
            "--range",
            "A1:B2",
            "--max-cells",
            "1",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn xlsx_ranges_set_matches_go_oracle_and_saved_output() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-ranges-set-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in = temp_dir.join("go-in.xlsx");
    let rust_in = temp_dir.join("rust-in.xlsx");
    let go_out = temp_dir.join("go-out.xlsx");
    let rust_out = temp_dir.join("rust-out.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in).expect("stage go input");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &rust_in).expect("stage rust input");
    let go_in = go_in.to_string_lossy().to_string();
    let rust_in = rust_in.to_string_lossy().to_string();
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();
    let values = r#"[["Name",{"value":"42.5","type":"number"},{"formula":"SUM(B1:B1)"}],[null,true,"tail"]]"#;

    let go_args = [
        "--json", "xlsx", "ranges", "set", &go_in, "--sheet", "Sheet1", "--range", "A1:C2",
        "--values", values, "--out", &go_out,
    ];
    let rust_args = [
        "--json", "xlsx", "ranges", "set", &rust_in, "--sheet", "Sheet1", "--range", "A1:C2",
        "--values", values, "--out", &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "ranges set exit");
    assert_eq!(rust_stderr, go_stderr, "ranges set stderr");
    let go_json = scrub_paths(
        go_stdout.expect("go ranges set stdout"),
        &[(&go_in, "[IN]"), (&go_out, "[OUT]")],
    );
    let rust_json = scrub_paths(
        rust_stdout.expect("rust ranges set stdout"),
        &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")],
    );
    assert_eq!(rust_json, go_json, "ranges set stdout");

    let export_args_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C2",
        "--include-types",
        "--include-formulas",
    ];
    let export_args_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C2",
        "--include-types",
        "--include-formulas",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_args_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_args_rust);
    assert_eq!(rust_code, go_code, "saved output export exit");
    assert_eq!(rust_stderr, go_stderr, "saved output export stderr");
    assert_eq!(
        scrub_path(rust_export.expect("rust saved export"), &rust_out, "[OUT]"),
        scrub_path(go_export.expect("go saved export"), &go_out, "[OUT]"),
        "saved output readback"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B1",
        "--values",
        r#"[["Dry",1]]"#,
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B1",
        "--values",
        r#"[["Dry",1]]"#,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "ranges set dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "ranges set dry-run stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust dry-run stdout"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go dry-run stdout"), &go_in, "[IN]"),
        "ranges set dry-run stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_format_matches_go_oracle_and_saved_output() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-set-format-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in_path = temp_dir.join("go-in.xlsx");
    let rust_in_path = temp_dir.join("rust-in.xlsx");
    let go_out_path = temp_dir.join("go-out.xlsx");
    let rust_out_path = temp_dir.join("rust-out.xlsx");
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B1"/>
  <sheetData>
    <row r="1">
      <c r="A1"><v>1234.5</v></c>
      <c r="B1"><f>A1*2</f><v>2469</v></c>
    </row>
  </sheetData>
</worksheet>"#;
    write_simple_xlsx_with_sheet_xml(&go_in_path, sheet_xml);
    write_simple_xlsx_with_sheet_xml(&rust_in_path, sheet_xml);
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_out = go_out_path.to_string_lossy().to_string();
    let rust_out = rust_out_path.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--preset",
        "currency",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--preset",
        "currency",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "ranges set-format exit");
    assert_eq!(rust_stderr, go_stderr, "ranges set-format stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust ranges set-format stdout"),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go ranges set-format stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "ranges set-format stdout"
    );

    let export_args_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ];
    let export_args_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_args_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_args_rust);
    assert_eq!(rust_code, go_code, "saved output export exit");
    assert_eq!(rust_stderr, go_stderr, "saved output export stderr");
    assert_eq!(
        scrub_path(rust_export.expect("rust saved export"), &rust_out, "[OUT]"),
        scrub_path(go_export.expect("go saved export"), &go_out, "[OUT]"),
        "saved output format readback"
    );

    let dry_go = [
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "C3",
        "--preset",
        "percent",
        "--dry-run",
    ];
    let dry_rust = [
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "C3",
        "--preset",
        "percent",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_go);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_rust);
    assert_eq!(rust_code, go_code, "ranges set-format dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "ranges set-format dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust set-format dry-run stdout"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(
            go_stdout.expect("go set-format dry-run stdout"),
            &go_in,
            "[IN]"
        ),
        "ranges set-format dry-run stdout"
    );
    assert!(
        !zip_entry_exists(&rust_in_path, "xl/styles.xml"),
        "dry-run wrote styles.xml into Rust input workbook"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_format_range_edges_match_go_oracle() {
    let file = "testdata/xlsx/minimal-workbook/workbook.xlsx";
    for range in ["A1B2", "A0", "A1:B2:C3", ":B2"] {
        assert_go_rust_match(&[
            "--json",
            "xlsx",
            "ranges",
            "set-format",
            file,
            "--sheet",
            "Sheet1",
            "--range",
            range,
            "--preset",
            "number",
            "--dry-run",
        ]);
    }
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        file,
        "--sheet",
        "Sheet1",
        "--range",
        "B2:A1",
        "--preset",
        "number",
        "--dry-run",
    ]);
}

#[test]
fn xlsx_ranges_set_delimited_and_stdin_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-delimited-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in = temp_dir.join("go-csv-in.xlsx");
    let rust_in = temp_dir.join("rust-csv-in.xlsx");
    let go_out = temp_dir.join("go-csv-out.xlsx");
    let rust_out = temp_dir.join("rust-csv-out.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in).expect("stage go input");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &rust_in).expect("stage rust input");
    let go_in = go_in.to_string_lossy().to_string();
    let rust_in = rust_in.to_string_lossy().to_string();
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();
    let csv = "Name,Value\nAlpha,\"two, too\"\nBeta,\"multi\nline\"\n";
    let go_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B3",
        "--data-format",
        "csv",
        "--values-file",
        "-",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B3",
        "--data-format",
        "csv",
        "--values-file",
        "-",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml_with_input(&go_args, csv);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml_with_input(&rust_args, csv);
    assert_eq!(rust_code, go_code, "CSV stdin ranges set exit");
    assert_eq!(rust_stderr, go_stderr, "CSV stdin ranges set stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust CSV stdin stdout"),
            &[(&rust_in, "[IN]"), (&rust_out, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go CSV stdin stdout"),
            &[(&go_in, "[IN]"), (&go_out, "[OUT]")]
        ),
        "CSV stdin ranges set stdout"
    );

    let export_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B3",
        "--include-types",
        "--include-formulas",
    ];
    let export_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_out,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B3",
        "--include-types",
        "--include-formulas",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_rust);
    assert_eq!(rust_code, go_code, "CSV stdin saved export exit");
    assert_eq!(rust_stderr, go_stderr, "CSV stdin saved export stderr");
    assert_eq!(
        scrub_path(
            rust_export.expect("rust CSV saved export"),
            &rust_out,
            "[OUT]"
        ),
        scrub_path(go_export.expect("go CSV saved export"), &go_out, "[OUT]"),
        "CSV stdin saved readback"
    );

    let tsv = "Name\tValue\nAlpha\ttwo\n";
    let go_tsv_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--data-format",
        "tsv",
        "--values",
        tsv,
        "--dry-run",
    ];
    let rust_tsv_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--data-format",
        "tsv",
        "--values",
        tsv,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_tsv_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_tsv_args);
    assert_eq!(rust_code, go_code, "TSV ranges set exit");
    assert_eq!(rust_stderr, go_stderr, "TSV ranges set stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust TSV stdout"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go TSV stdout"), &go_in, "[IN]"),
        "TSV ranges set stdout"
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_preserves_untouched_cell_xml() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-preserve-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("output.xlsx");
    write_preservation_xlsx(&input);
    let input_s = input.to_string_lossy().to_string();
    let output_s = output.to_string_lossy().to_string();

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "ranges",
        "set",
        &input_s,
        "--sheet",
        "Sheet1",
        "--range",
        "D1:D1",
        "--values",
        r#"[["new"]]"#,
        "--out",
        &output_s,
    ]);
    assert_eq!(code, 0, "preservation edit stderr={stderr:?}");
    assert!(stdout.is_some(), "preservation edit stdout");
    let sheet_xml = read_zip_string(&output, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains(r#"<c r="A1" t="s"><v>0</v></c>"#),
        "shared-string cell changed:\n{sheet_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="B1" s="1"><v>45123</v></c>"#),
        "styled/date cell changed:\n{sheet_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="C1"><f>B1*2</f><v>90246</v></c>"#),
        "formula cache cell changed:\n{sheet_xml}"
    );
    assert!(
        sheet_xml.contains(r#"<c r="D1" t="inlineStr"><is><t>new</t></is></c>"#),
        "new cell missing:\n{sheet_xml}"
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_in_place_backup_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-in-place-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_in = temp_dir.join("go.xlsx");
    let rust_in = temp_dir.join("rust.xlsx");
    let go_backup = temp_dir.join("go.xlsx.bak");
    let rust_backup = temp_dir.join("rust.xlsx.bak");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in).expect("stage go input");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &rust_in).expect("stage rust input");
    let go_in = go_in.to_string_lossy().to_string();
    let rust_in = rust_in.to_string_lossy().to_string();
    let go_backup = go_backup.to_string_lossy().to_string();
    let rust_backup = rust_backup.to_string_lossy().to_string();
    let go_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--values",
        r#"[["In place"]]"#,
        "--in-place",
        "--backup",
        &go_backup,
    ];
    let rust_args = [
        "--json",
        "xlsx",
        "ranges",
        "set",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--values",
        r#"[["In place"]]"#,
        "--in-place",
        "--backup",
        &rust_backup,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "in-place exit");
    assert_eq!(rust_stderr, go_stderr, "in-place stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust in-place stdout"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go in-place stdout"), &go_in, "[IN]"),
        "in-place stdout"
    );
    assert!(Path::new(&go_backup).exists(), "go backup missing");
    assert!(Path::new(&rust_backup).exists(), "rust backup missing");

    let export_go = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &go_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--include-types",
    ];
    let export_rust = [
        "--json",
        "xlsx",
        "ranges",
        "export",
        &rust_in,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:A1",
        "--include-types",
    ];
    let (go_code, go_export, go_stderr) = run_go_ooxml(&export_go);
    let (rust_code, rust_export, rust_stderr) = run_go_ooxml(&export_rust);
    assert_eq!(rust_code, go_code, "in-place readback exit");
    assert_eq!(rust_stderr, go_stderr, "in-place readback stderr");
    assert_eq!(
        scrub_path(rust_export.expect("rust in-place export"), &rust_in, "[IN]"),
        scrub_path(go_export.expect("go in-place export"), &go_in, "[IN]"),
        "in-place saved readback"
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_ranges_set_rejects_formula_and_merged_cells_like_go() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-ranges-guards-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let formula = temp_dir.join("formula.xlsx");
    let merged = temp_dir.join("merged.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &formula,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1"/>
  <sheetData><row r="1"><c r="A1"><f>SUM(B1:B1)</f><v>1</v></c></row></sheetData>
</worksheet>"#,
    );
    write_simple_xlsx_with_sheet_xml(
        &merged,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B1"/>
  <sheetData><row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c></row></sheetData>
  <mergeCells count="1"><mergeCell ref="A1:B1"/></mergeCells>
</worksheet>"#,
    );
    let formula_s = formula.to_string_lossy().to_string();
    let merged_s = merged.to_string_lossy().to_string();
    for args in [
        vec![
            "--json",
            "xlsx",
            "ranges",
            "set",
            &formula_s,
            "--sheet",
            "Sheet1",
            "--anchor",
            "A1",
            "--values",
            r#"[["replace"]]"#,
            "--dry-run",
        ],
        vec![
            "--json",
            "xlsx",
            "ranges",
            "set",
            &merged_s,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:B1",
            "--values",
            r#"[["x","y"]]"#,
            "--dry-run",
        ],
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "guard exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "guard stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "guard stderr for {args:?}");
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_cells_extract_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "xlsx",
            "cells",
            "extract",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "xlsx",
            "cells",
            "extract",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--range",
            "B1:D2",
            "--include-empty",
            "--max-rows",
            "2",
        ],
        vec![
            "--json",
            "xlsx",
            "cells",
            "extract",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
            "--sheet",
            "Types",
            "--range",
            "E2:H2",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn xlsx_sheets_show_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "xlsx",
            "sheets",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "xlsx",
            "sheets",
            "show",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
            "--sheet",
            "Types",
        ],
        vec![
            "--json",
            "xlsx",
            "sheets",
            "show",
            "testdata/xlsx/used-range/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn xlsx_tables_list_show_matches_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-xlsx-tables-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("table-workbook.xlsx");
    write_table_xlsx(&workbook);
    let workbook = workbook.to_string_lossy().to_string();

    let cases: Vec<Vec<&str>> = vec![
        vec!["--json", "xlsx", "tables", "list", &workbook],
        vec![
            "--json", "xlsx", "tables", "list", &workbook, "--sheet", "Data",
        ],
        vec![
            "--json", "xlsx", "tables", "show", &workbook, "--table", "Sales",
        ],
        vec![
            "--json",
            "xlsx",
            "tables",
            "show",
            &workbook,
            "--sheet",
            "sheetId:1",
            "--table",
            "tableId:1",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }

    for selector in [
        "tableId:1",
        "id:1",
        "table:1",
        "#1",
        "part:/xl/tables/table1.xml",
        "rid:rId1",
        "rId:rId1",
        "table:Sales",
        "displayName:Sales",
        "name:Sales",
        "Sales",
        "1",
    ] {
        assert_go_rust_match(&[
            "--json", "xlsx", "tables", "show", &workbook, "--table", selector,
        ]);
    }

    for missing in ["2", "Missing"] {
        assert_go_rust_match(&[
            "--json", "xlsx", "tables", "show", &workbook, "--table", missing,
        ]);
    }

    let spaced_dir = temp_dir.join("dir with spaces");
    fs::create_dir_all(&spaced_dir).expect("spaced temp dir");
    let spaced_workbook = spaced_dir.join("table workbook.xlsx");
    write_table_xlsx_with_sheet(&spaced_workbook, "Data Sheet");
    let spaced_workbook = spaced_workbook.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "xlsx", "tables", "list", &spaced_workbook]);
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "tables",
        "show",
        &spaced_workbook,
        "--sheet",
        "Data Sheet",
        "--table",
        "Sales",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_tables_export_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-xlsx-table-export-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let workbook = temp_dir.join("table-workbook.xlsx");
    write_table_xlsx(&workbook);
    let workbook = workbook.to_string_lossy().to_string();

    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json", "xlsx", "tables", "export", &workbook, "--table", "Sales",
        ],
        vec![
            "--json",
            "xlsx",
            "tables",
            "export",
            &workbook,
            "--table",
            "Sales",
            "--include-types",
        ],
        vec![
            "--json",
            "xlsx",
            "tables",
            "export",
            &workbook,
            "--sheet",
            "Data",
            "--table",
            "tableId:1",
            "--include-types",
            "--include-formulas",
        ],
        vec![
            "--json", "xlsx", "tables", "export", &workbook, "--table", "Missing",
        ],
        vec![
            "--json",
            "xlsx",
            "tables",
            "export",
            &workbook,
            "--table",
            "Sales",
            "--max-cells",
            "1",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }

    let data_out = temp_dir.join("table-export.json");
    let data_out = data_out.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "tables",
        "export",
        &workbook,
        "--table",
        "Sales",
        "--include-types",
        "--include-formulas",
        "--data-out",
        &data_out,
    ]);
    let saved: Value =
        serde_json::from_str(fs::read_to_string(&data_out).expect("data-out file").trim())
            .expect("data-out JSON");
    let (code, expected_full, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "tables",
        "export",
        &workbook,
        "--table",
        "Sales",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let mut expected_full = expected_full.expect("full table export");
    expected_full["dataOut"] = Value::String(data_out);
    assert_eq!(saved, expected_full);

    let spaced_dir = temp_dir.join("dir with spaces");
    fs::create_dir_all(&spaced_dir).expect("spaced temp dir");
    let spaced_workbook = spaced_dir.join("table workbook.xlsx");
    write_table_xlsx_with_sheet(&spaced_workbook, "Data Sheet");
    let spaced_workbook = spaced_workbook.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "xlsx",
        "tables",
        "export",
        &spaced_workbook,
        "--sheet",
        "Data Sheet",
        "--table",
        "Sales",
        "--include-types",
        "--include-formulas",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn inspect_matches_go_oracle_for_supported_families() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "inspect",
            "testdata/pptx/minimal-title/presentation.pptx",
        ],
        vec![
            "--json",
            "inspect",
            "testdata/pptx/table-slide/presentation.pptx",
        ],
        vec![
            "--json",
            "inspect",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "inspect",
            "testdata/xlsx/types-and-formulas/workbook.xlsx",
        ],
        vec![
            "--json",
            "inspect",
            "testdata/xlsx/chart-workbook/workbook.xlsx",
        ],
        vec!["--json", "inspect", "testdata/docx/minimal/document.docx"],
        vec![
            "--json",
            "inspect",
            "testdata/docx/mixed-blocks/document.docx",
        ],
        vec!["--json", "inspect", "testdata/docx/headers/document.docx"],
        vec![
            "--json",
            "inspect",
            "testdata/docx/with-comments/document.docx",
        ],
        vec![
            "--json",
            "inspect",
            "testdata/docx/with-image/document.docx",
        ],
        vec![
            "--json",
            "inspect",
            "testdata/docx/corrupted-missing-document/document.docx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
    assert_generated_inspect_edge_cases_match_go();
}

#[test]
fn docx_text_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/space-preserve/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/styled-headings/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/split-runs/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/styles-catalog/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/hyperlink/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/mixed-blocks/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/with-fields/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/headers/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/paraid/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/paraid-dup/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/default-ns/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/merged-table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/with-comments/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/with-media/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/with-image/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/apply-styles/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "exit code for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "stderr for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "stdout for {args:?}");
    }
}

#[test]
fn docx_blocks_match_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/mixed-blocks/document.docx",
            "--block",
            "2",
            "--include-runs",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/merged-table/document.docx",
            "--include-runs",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/paraid/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/paraid-dup/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/styled-headings/document.docx",
            "--include-runs",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/minimal/document.docx",
            "--block",
            "99",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/minimal/document.docx",
            "--block",
            "-1",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-blocks-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx blocks temp dir");

    let malformed_docx = temp_dir.join("malformed-document.docx");
    rewrite_zip_fixture(
        "testdata/docx/minimal/document.docx",
        &malformed_docx,
        |name, data| {
            let data = if name == "word/document.xml" {
                br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p>"#.to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let malformed_docx = malformed_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "blocks", &malformed_docx]);

    let wrong_root_docx = temp_dir.join("wrong-root-document.docx");
    rewrite_zip_fixture(
        "testdata/docx/minimal/document.docx",
        &wrong_root_docx,
        |name, data| {
            let data = if name == "word/document.xml" {
                br#"<w:notDocument xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Text</w:t></w:r></w:p></w:body></w:notDocument>"#.to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let wrong_root_docx = wrong_root_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "blocks", &wrong_root_docx]);

    let missing_body_docx = temp_dir.join("missing-body-document.docx");
    rewrite_zip_fixture(
        "testdata/docx/minimal/document.docx",
        &missing_body_docx,
        |name, data| {
            let data = if name == "word/document.xml" {
                br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>Text</w:t></w:r></w:p></w:document>"#.to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let missing_body_docx = missing_body_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "blocks", &missing_body_docx]);

    let nested_table_docx = temp_dir.join("nested-table.docx");
    write_nested_table_docx(&nested_table_docx);
    let nested_table_docx = nested_table_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "blocks", &nested_table_docx]);

    let alternate_prefix_paraid_docx = temp_dir.join("alternate-prefix-paraid.docx");
    rewrite_zip_fixture(
        "testdata/docx/minimal/document.docx",
        &alternate_prefix_paraid_docx,
        |name, data| {
            let data = if name == "word/document.xml" {
                br#"<doc:document xmlns:doc="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:p14="http://schemas.microsoft.com/office/word/2010/wordml"><doc:body><doc:p p14:paraId="ABCD1234"><doc:r><doc:t>Alternate paraId prefix</doc:t></doc:r></doc:p></doc:body></doc:document>"#.to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let alternate_prefix_paraid_docx = alternate_prefix_paraid_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "blocks", &alternate_prefix_paraid_docx]);

    let foreign_metadata_docx = temp_dir.join("foreign-metadata.docx");
    rewrite_zip_fixture(
        "testdata/docx/minimal/document.docx",
        &foreign_metadata_docx,
        |name, data| {
            let data = if name == "word/document.xml" {
                br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:f="urn:foreign"><w:body><w:p f:paraId="DEAD00FF"><w:pPr><f:pStyle w:val="ForeignStyle"/><w:pStyle f:val="IgnoredStyle"/></w:pPr><w:r><w:rPr><f:b w:val="true"/><w:b f:val="false"/><f:color w:val="FF0000"/><w:color f:val="00FF00"/><w:u f:val="none"/><w:sz f:val="48"/></w:rPr><w:t>Foreign metadata</w:t></w:r></w:p></w:body></w:document>"#.to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let foreign_metadata_docx = foreign_metadata_docx.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "docx",
        "blocks",
        &foreign_metadata_docx,
        "--include-runs",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_tables_show_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--table",
            "1",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/merged-table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--details",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--table",
            "2",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--table",
            "-1",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/corrupted-missing-document/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-tables-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables temp dir");
    let nested_table_docx = temp_dir.join("nested-table.docx");
    write_nested_table_docx(&nested_table_docx);
    let nested_table_docx = nested_table_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "tables", "show", &nested_table_docx]);
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_tables_set_clear_cell_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-tables-cell-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables cell temp dir");

    let document = "testdata/docx/table/document.docx";
    let (hash_code, hash_stdout, hash_stderr) =
        run_go_ooxml(&["--json", "docx", "tables", "show", document, "--table", "1"]);
    assert_eq!(hash_code, 0, "oracle table hash lookup exit");
    assert_eq!(hash_stderr, None, "oracle table hash lookup stderr");
    let table_hash = hash_stdout.expect("oracle table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("table hash")
        .to_string();

    let go_set_out = temp_dir
        .join("tables-set-cell-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_set_out = temp_dir
        .join("tables-set-cell-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_set_args = [
        "--json",
        "docx",
        "tables",
        "set-cell",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &table_hash,
        "--text",
        "Approved",
        "--out",
        &go_set_out,
    ];
    let rust_set_args = [
        "--json",
        "docx",
        "tables",
        "set-cell",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &table_hash,
        "--text",
        "Approved",
        "--out",
        &rust_set_out,
    ];
    let (go_set_code, go_set_stdout, go_set_stderr) = run_go_ooxml(&go_set_args);
    let (rust_set_code, rust_set_stdout, rust_set_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_set_code, go_set_code, "set-cell exit");
    assert_eq!(rust_set_stderr, go_set_stderr, "set-cell stderr");
    let go_set_json = scrub_path(
        go_set_stdout.expect("Go set-cell stdout"),
        &go_set_out,
        "[SET_OUT]",
    );
    let rust_set_json = scrub_path(
        rust_set_stdout.expect("Rust set-cell stdout"),
        &rust_set_out,
        "[SET_OUT]",
    );
    assert_eq!(rust_set_json, go_set_json, "set-cell stdout");
    assert_eq!(rust_set_json["text"], Value::String("Approved".to_string()));
    assert_eq!(
        rust_set_json["previousText"],
        Value::String("B1".to_string())
    );

    let (set_validate_code, _set_validate_stdout, set_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_set_out]);
    assert_eq!(set_validate_code, 0, "set-cell validate exit");
    assert_eq!(set_validate_stderr, None, "set-cell validate stderr");

    let (go_set_read_code, go_set_read_stdout, go_set_read_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &go_set_out,
        "--table",
        "1",
    ]);
    let (rust_set_read_code, rust_set_read_stdout, rust_set_read_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &rust_set_out,
        "--table",
        "1",
    ]);
    assert_eq!(rust_set_read_code, go_set_read_code, "set readback exit");
    assert_eq!(
        rust_set_read_stderr, go_set_read_stderr,
        "set readback stderr"
    );
    let go_set_table = scrub_path(
        go_set_read_stdout.expect("Go set readback JSON")["tables"][0].clone(),
        &go_set_out,
        "[SET_OUT]",
    );
    let rust_set_table = scrub_path(
        rust_set_read_stdout.expect("Rust set readback JSON")["tables"][0].clone(),
        &rust_set_out,
        "[SET_OUT]",
    );
    assert_eq!(rust_set_table, go_set_table, "set readback table");
    assert_eq!(
        rust_set_table["cells"][0][1],
        Value::String("Approved".to_string())
    );

    let set_hash = rust_set_json["contentHash"]
        .as_str()
        .expect("set-cell content hash")
        .to_string();
    let go_clear_out = temp_dir
        .join("tables-clear-cell-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_clear_out = temp_dir
        .join("tables-clear-cell-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_clear_args = [
        "--json",
        "docx",
        "tables",
        "clear-cell",
        &go_set_out,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &set_hash,
        "--out",
        &go_clear_out,
    ];
    let rust_clear_args = [
        "--json",
        "docx",
        "tables",
        "clear-cell",
        &rust_set_out,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &set_hash,
        "--out",
        &rust_clear_out,
    ];
    let (go_clear_code, go_clear_stdout, go_clear_stderr) = run_go_ooxml(&go_clear_args);
    let (rust_clear_code, rust_clear_stdout, rust_clear_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_clear_code, go_clear_code, "clear-cell exit");
    assert_eq!(rust_clear_stderr, go_clear_stderr, "clear-cell stderr");
    let go_clear_json = scrub_paths(
        go_clear_stdout.expect("Go clear-cell stdout"),
        &[(&go_set_out, "[SET_OUT]"), (&go_clear_out, "[CLEAR_OUT]")],
    );
    let rust_clear_json = scrub_paths(
        rust_clear_stdout.expect("Rust clear-cell stdout"),
        &[
            (&rust_set_out, "[SET_OUT]"),
            (&rust_clear_out, "[CLEAR_OUT]"),
        ],
    );
    assert_eq!(rust_clear_json, go_clear_json, "clear-cell stdout");
    assert_eq!(
        rust_clear_json["previousText"],
        Value::String("Approved".to_string())
    );

    let (clear_validate_code, _clear_validate_stdout, clear_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_clear_out]);
    assert_eq!(clear_validate_code, 0, "clear-cell validate exit");
    assert_eq!(clear_validate_stderr, None, "clear-cell validate stderr");

    let (go_clear_read_code, go_clear_read_stdout, go_clear_read_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &go_clear_out,
        "--table",
        "1",
    ]);
    let (rust_clear_read_code, rust_clear_read_stdout, rust_clear_read_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &rust_clear_out,
        "--table",
        "1",
    ]);
    assert_eq!(
        rust_clear_read_code, go_clear_read_code,
        "clear readback exit"
    );
    assert_eq!(
        rust_clear_read_stderr, go_clear_read_stderr,
        "clear readback stderr"
    );
    let go_clear_table = scrub_path(
        go_clear_read_stdout.expect("Go clear readback JSON")["tables"][0].clone(),
        &go_clear_out,
        "[CLEAR_OUT]",
    );
    let rust_clear_table = scrub_path(
        rust_clear_read_stdout.expect("Rust clear readback JSON")["tables"][0].clone(),
        &rust_clear_out,
        "[CLEAR_OUT]",
    );
    assert_eq!(rust_clear_table, go_clear_table, "clear readback table");
    assert_eq!(
        rust_clear_table["cells"][0][1],
        Value::String(String::new())
    );

    let dry_args = [
        "--json",
        "docx",
        "tables",
        "set-cell",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "1",
        "--expect-hash",
        &table_hash,
        "--text",
        "",
        "--dry-run",
    ];
    let (go_dry_code, go_dry_stdout, go_dry_stderr) = run_go_ooxml(&dry_args);
    let (rust_dry_code, rust_dry_stdout, rust_dry_stderr) = run_ooxml(&dry_args);
    assert_eq!(rust_dry_code, go_dry_code, "set-cell dry-run exit");
    assert_eq!(rust_dry_stderr, go_dry_stderr, "set-cell dry-run stderr");
    let dry_json = rust_dry_stdout.expect("Rust set-cell dry-run stdout");
    assert_eq!(
        dry_json,
        go_dry_stdout.expect("Go set-cell dry-run stdout"),
        "set-cell dry-run stdout"
    );
    assert_eq!(dry_json["dryRun"], Value::Bool(true));
    assert!(dry_json.get("output").is_none(), "dry-run omits output");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_append_matches_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-paragraphs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx paragraphs temp dir");
    let go_out = temp_dir.join("append-go.docx");
    let rust_out = temp_dir.join("append-rust.docx");
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "docx",
        "paragraphs",
        "append",
        "testdata/docx/styled-headings/document.docx",
        "--text",
        "Tail Heading",
        "--style",
        "Heading1",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "paragraphs",
        "append",
        "testdata/docx/styled-headings/document.docx",
        "--text",
        "Tail Heading",
        "--style",
        "Heading1",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "append exit");
    assert_eq!(rust_stderr, go_stderr, "append stderr");
    assert_eq!(rust_stdout, go_stdout, "append stdout");
    assert!(Path::new(&rust_out).exists(), "Rust append output missing");

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "validate exit");
    assert_eq!(validate_stderr, None, "validate stderr");
    assert!(validate_stdout.is_some(), "validate stdout");

    let (go_text_code, go_text_stdout, go_text_stderr) =
        run_go_ooxml(&["--json", "docx", "text", &go_out]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_out]);
    assert_eq!(rust_text_code, go_text_code, "append readback exit");
    assert_eq!(rust_text_stderr, go_text_stderr, "append readback stderr");
    let go_text_result = go_text_stdout.expect("Go append readback JSON");
    let rust_text_result = rust_text_stdout.expect("Rust append readback JSON");
    assert_eq!(
        rust_text_result["blocks"], go_text_result["blocks"],
        "append readback blocks"
    );
    assert_eq!(rust_text_result["file"], Value::String(rust_out.clone()));

    let blocks = rust_text_result["blocks"].as_array().expect("docx blocks");
    assert_eq!(blocks.len(), 3, "appended block count");
    assert_eq!(blocks[2]["text"], Value::String("Tail Heading".to_string()));
    assert_eq!(blocks[2]["style"], Value::String("Heading1".to_string()));
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_append_dry_run_and_errors_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-paragraphs-dry-run-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx paragraphs temp dir");
    let dry_docx = temp_dir.join("dry-run.docx");
    fs::copy("testdata/docx/minimal/document.docx", &dry_docx).expect("copy dry-run docx");
    let dry_docx = dry_docx.to_string_lossy().to_string();

    assert_go_rust_match(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        &dry_docx,
        "--text",
        "Dry run tail",
        "--dry-run",
    ]);
    let (text_code, text_stdout, text_stderr) = run_ooxml(&["--json", "docx", "text", &dry_docx]);
    assert_eq!(text_code, 0);
    assert_eq!(text_stderr, None);
    let text_result = text_stdout.expect("dry-run readback");
    let blocks = text_result["blocks"].as_array().expect("docx blocks");
    assert_eq!(blocks.len(), 1, "dry-run wrote to document");
    assert_eq!(blocks[0]["text"], Value::String("Hello world".to_string()));

    let text_file = temp_dir.join("text.txt");
    fs::write(&text_file, "x").expect("write text file");
    let text_file = text_file.to_string_lossy().to_string();
    let out = temp_dir.join("bad.docx").to_string_lossy().to_string();
    let missing = temp_dir.join("missing.docx").to_string_lossy().to_string();
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "paragraphs",
            "append",
            &missing,
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "append",
            "testdata/docx/minimal/document.docx",
            "--text",
            "x",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "append",
            "testdata/docx/minimal/document.docx",
            "--text",
            "x",
            "--dry-run",
            "--out",
            &out,
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "append",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "append",
            "testdata/docx/minimal/document.docx",
            "--text",
            "x",
            "--text-file",
            &text_file,
            "--dry-run",
        ],
    ];
    for args in bad_cases {
        assert_go_rust_match(&args);
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_blocks_replace_delete_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-blocks-replace-delete-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx blocks replace/delete temp dir");

    let heading_doc = "testdata/docx/styled-headings/document.docx";
    let (heading_code, heading_stdout, heading_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", heading_doc, "--block", "1"]);
    assert_eq!(heading_code, 0, "oracle heading hash lookup exit");
    assert_eq!(heading_stderr, None, "oracle heading hash lookup stderr");
    let heading_hash =
        heading_stdout.expect("oracle heading block JSON")["blocks"][0]["contentHash"]
            .as_str()
            .expect("heading block hash")
            .to_string();

    let go_replace_out = temp_dir
        .join("blocks-replace-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_replace_out = temp_dir
        .join("blocks-replace-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_replace_args = [
        "--json",
        "docx",
        "blocks",
        "replace",
        heading_doc,
        "--block",
        "1",
        "--expect-hash",
        &heading_hash,
        "--text",
        "Hash-guarded heading",
        "--out",
        &go_replace_out,
    ];
    let rust_replace_args = [
        "--json",
        "docx",
        "blocks",
        "replace",
        heading_doc,
        "--block",
        "1",
        "--expect-hash",
        &heading_hash,
        "--text",
        "Hash-guarded heading",
        "--out",
        &rust_replace_out,
    ];
    let (go_replace_code, go_replace_stdout, go_replace_stderr) = run_go_ooxml(&go_replace_args);
    let (rust_replace_code, rust_replace_stdout, rust_replace_stderr) =
        run_ooxml(&rust_replace_args);
    assert_eq!(rust_replace_code, go_replace_code, "blocks replace exit");
    assert_eq!(
        rust_replace_stderr, go_replace_stderr,
        "blocks replace stderr"
    );
    assert_eq!(
        rust_replace_stdout, go_replace_stdout,
        "blocks replace stdout"
    );

    let (replace_validate_code, _replace_validate_stdout, replace_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_replace_out]);
    assert_eq!(replace_validate_code, 0, "blocks replace validate exit");
    assert_eq!(
        replace_validate_stderr, None,
        "blocks replace validate stderr"
    );

    let (go_replace_read_code, go_replace_read_stdout, go_replace_read_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", &go_replace_out, "--block", "1"]);
    let (rust_replace_read_code, rust_replace_read_stdout, rust_replace_read_stderr) =
        run_ooxml(&[
            "--json",
            "docx",
            "blocks",
            &rust_replace_out,
            "--block",
            "1",
        ]);
    assert_eq!(
        rust_replace_read_code, go_replace_read_code,
        "replace readback exit"
    );
    assert_eq!(
        rust_replace_read_stderr, go_replace_read_stderr,
        "replace readback stderr"
    );
    let go_replace_block =
        go_replace_read_stdout.expect("Go replace readback JSON")["blocks"][0].clone();
    let rust_replace_block =
        rust_replace_read_stdout.expect("Rust replace readback JSON")["blocks"][0].clone();
    assert_eq!(
        rust_replace_block, go_replace_block,
        "replace readback block"
    );
    assert_eq!(
        rust_replace_block["text"],
        Value::String("Hash-guarded heading".to_string())
    );
    assert_eq!(
        rust_replace_block["paragraph"]["style"],
        Value::String("Heading1".to_string())
    );

    let mixed_doc = "testdata/docx/mixed-blocks/document.docx";
    let (table_code, table_stdout, table_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", mixed_doc, "--block", "1"]);
    assert_eq!(table_code, 0, "oracle table hash lookup exit");
    assert_eq!(table_stderr, None, "oracle table hash lookup stderr");
    let table_hash = table_stdout.expect("oracle table block JSON")["blocks"][0]["contentHash"]
        .as_str()
        .expect("table block hash")
        .to_string();

    let go_delete_out = temp_dir
        .join("blocks-delete-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_delete_out = temp_dir
        .join("blocks-delete-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_delete_args = [
        "--json",
        "docx",
        "blocks",
        "delete",
        mixed_doc,
        "--block",
        "1",
        "--expect-hash",
        &table_hash,
        "--out",
        &go_delete_out,
    ];
    let rust_delete_args = [
        "--json",
        "docx",
        "blocks",
        "delete",
        mixed_doc,
        "--block",
        "1",
        "--expect-hash",
        &table_hash,
        "--out",
        &rust_delete_out,
    ];
    let (go_delete_code, go_delete_stdout, go_delete_stderr) = run_go_ooxml(&go_delete_args);
    let (rust_delete_code, rust_delete_stdout, rust_delete_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_delete_code, go_delete_code, "blocks delete exit");
    assert_eq!(rust_delete_stderr, go_delete_stderr, "blocks delete stderr");
    assert_eq!(rust_delete_stdout, go_delete_stdout, "blocks delete stdout");

    let (delete_validate_code, _delete_validate_stdout, delete_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_delete_out]);
    assert_eq!(delete_validate_code, 0, "blocks delete validate exit");
    assert_eq!(
        delete_validate_stderr, None,
        "blocks delete validate stderr"
    );

    let (go_delete_read_code, go_delete_read_stdout, go_delete_read_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", &go_delete_out]);
    let (rust_delete_read_code, rust_delete_read_stdout, rust_delete_read_stderr) =
        run_ooxml(&["--json", "docx", "blocks", &rust_delete_out]);
    assert_eq!(
        rust_delete_read_code, go_delete_read_code,
        "delete readback exit"
    );
    assert_eq!(
        rust_delete_read_stderr, go_delete_read_stderr,
        "delete readback stderr"
    );
    let go_delete_blocks =
        go_delete_read_stdout.expect("Go delete readback JSON")["blocks"].clone();
    let rust_delete_blocks =
        rust_delete_read_stdout.expect("Rust delete readback JSON")["blocks"].clone();
    assert_eq!(
        rust_delete_blocks, go_delete_blocks,
        "delete readback blocks"
    );
    assert_eq!(
        rust_delete_blocks.as_array().expect("blocks array").len(),
        3
    );

    assert_go_rust_match(&[
        "--json",
        "docx",
        "blocks",
        "replace",
        heading_doc,
        "--block",
        "1",
        "--expect-hash",
        &heading_hash,
        "--text",
        "Dry run heading",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "docx",
        "blocks",
        "delete",
        mixed_doc,
        "--block",
        "1",
        "--expect-hash",
        &table_hash,
        "--dry-run",
    ]);

    let text_file = temp_dir.join("text.txt");
    fs::write(&text_file, "text").expect("write blocks replace text file");
    let text_file = text_file.to_string_lossy().to_string();
    let bad_hash = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "0",
            "--expect-hash",
            &heading_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "1",
            "--expect-hash",
            "sha256:nothex",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "99",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--text",
            "stale",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "1",
            "--expect-hash",
            &heading_hash,
            "--text",
            "x",
            "--text-file",
            &text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            mixed_doc,
            "--block",
            "0",
            "--expect-hash",
            &table_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            mixed_doc,
            "--block",
            "1",
            "--expect-hash",
            "sha256:nothex",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            mixed_doc,
            "--block",
            "99",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            mixed_doc,
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            "testdata/docx/minimal/document.docx",
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
    ];
    for args in bad_cases {
        assert_go_rust_match(&args);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_blocks_insert_after_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-blocks-insert-after-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx blocks insert-after temp dir");

    let document = "testdata/docx/mixed-blocks/document.docx";
    let (blocks_code, blocks_stdout, blocks_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", document, "--block", "1"]);
    assert_eq!(blocks_code, 0, "oracle hash lookup exit");
    assert_eq!(blocks_stderr, None, "oracle hash lookup stderr");
    let anchor_hash = blocks_stdout.expect("oracle blocks JSON")["blocks"][0]["contentHash"]
        .as_str()
        .expect("anchor hash")
        .to_string();

    let go_out = temp_dir
        .join("blocks-insert-after-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_out = temp_dir
        .join("blocks-insert-after-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_args = [
        "--json",
        "docx",
        "blocks",
        "insert-after",
        document,
        "--block",
        "1",
        "--expect-hash",
        &anchor_hash,
        "--text",
        "Inserted after table",
        "--style",
        "Heading1",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "blocks",
        "insert-after",
        document,
        "--block",
        "1",
        "--expect-hash",
        &anchor_hash,
        "--text",
        "Inserted after table",
        "--style",
        "Heading1",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "blocks insert-after exit");
    assert_eq!(rust_stderr, go_stderr, "blocks insert-after stderr");
    assert_eq!(rust_stdout, go_stdout, "blocks insert-after stdout");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "blocks insert-after validate exit");
    assert_eq!(validate_stderr, None, "blocks insert-after validate stderr");

    let (go_read_code, go_read_stdout, go_read_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", &go_out, "--block", "2"]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) =
        run_ooxml(&["--json", "docx", "blocks", &rust_out, "--block", "2"]);
    assert_eq!(rust_read_code, go_read_code, "insert readback exit");
    assert_eq!(rust_read_stderr, go_read_stderr, "insert readback stderr");
    let go_block = go_read_stdout.expect("Go insert readback JSON")["blocks"][0].clone();
    let rust_block = rust_read_stdout.expect("Rust insert readback JSON")["blocks"][0].clone();
    assert_eq!(rust_block, go_block, "insert readback block");
    assert_eq!(
        rust_block["text"],
        Value::String("Inserted after table".to_string())
    );
    assert_eq!(
        rust_block["paragraph"]["style"],
        Value::String("Heading1".to_string())
    );

    assert_go_rust_match(&[
        "--json",
        "docx",
        "blocks",
        "insert-after",
        "testdata/docx/minimal/document.docx",
        "--block",
        "0",
        "--text",
        "Before",
        "--dry-run",
    ]);

    let text_file = temp_dir.join("text.txt");
    fs::write(&text_file, "text").expect("write insert-after text file");
    let text_file = text_file.to_string_lossy().to_string();
    let bad_hash = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--block",
            "1",
            "--expect-hash",
            "sha256:nothex",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--block",
            "0",
            "--expect-hash",
            &anchor_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--block",
            "-1",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--block",
            "99",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--text",
            "stale",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--text",
            "x",
            "--text-file",
            &text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--text",
            "x",
            "--dry-run",
        ],
    ];
    for args in bad_cases {
        assert_go_rust_match(&args);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_insert_matches_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-insert-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx insert temp dir");

    let text_file = temp_dir.join("insert.txt");
    fs::write(&text_file, "Lead\tparagraph\nline 2").expect("write insert text file");
    let text_file = text_file.to_string_lossy().to_string();
    let go_out = temp_dir
        .join("insert-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_out = temp_dir
        .join("insert-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        "testdata/docx/styled-headings/document.docx",
        "--insert-after",
        "0",
        "--text-file",
        &text_file,
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        "testdata/docx/styled-headings/document.docx",
        "--insert-after",
        "0",
        "--text-file",
        &text_file,
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "insert start exit");
    assert_eq!(rust_stderr, go_stderr, "insert start stderr");
    assert_eq!(rust_stdout, go_stdout, "insert start stdout");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "insert start validate exit");
    assert_eq!(validate_stderr, None, "insert start validate stderr");

    let (go_text_code, go_text_stdout, go_text_stderr) =
        run_go_ooxml(&["--json", "docx", "text", &go_out]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_out]);
    assert_eq!(rust_text_code, go_text_code, "insert start readback exit");
    assert_eq!(
        rust_text_stderr, go_text_stderr,
        "insert start readback stderr"
    );
    let go_text_result = go_text_stdout.expect("Go insert start readback JSON");
    let rust_text_result = rust_text_stdout.expect("Rust insert start readback JSON");
    assert_eq!(
        rust_text_result["blocks"], go_text_result["blocks"],
        "insert start readback blocks"
    );
    let blocks = rust_text_result["blocks"].as_array().expect("docx blocks");
    assert_eq!(blocks.len(), 3, "insert start block count");
    assert_eq!(
        blocks[0]["text"],
        Value::String("Lead\tparagraph\nline 2".to_string())
    );
    assert_eq!(blocks[1]["text"], Value::String("Heading Text".to_string()));

    let go_table_out = temp_dir
        .join("insert-after-table-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_table_out = temp_dir
        .join("insert-after-table-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_table_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        "testdata/docx/mixed-blocks/document.docx",
        "--insert-after",
        "1",
        "--text",
        "After table",
        "--out",
        &go_table_out,
    ];
    let rust_table_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        "testdata/docx/mixed-blocks/document.docx",
        "--insert-after",
        "1",
        "--text",
        "After table",
        "--out",
        &rust_table_out,
    ];
    let (go_table_code, go_table_stdout, go_table_stderr) = run_go_ooxml(&go_table_args);
    let (rust_table_code, rust_table_stdout, rust_table_stderr) = run_ooxml(&rust_table_args);
    assert_eq!(rust_table_code, go_table_code, "insert table exit");
    assert_eq!(rust_table_stderr, go_table_stderr, "insert table stderr");
    assert_eq!(rust_table_stdout, go_table_stdout, "insert table stdout");
    let (go_table_text_code, go_table_text_stdout, go_table_text_stderr) =
        run_go_ooxml(&["--json", "docx", "text", &go_table_out]);
    let (rust_table_text_code, rust_table_text_stdout, rust_table_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_table_out]);
    assert_eq!(
        rust_table_text_code, go_table_text_code,
        "insert table readback exit"
    );
    assert_eq!(
        rust_table_text_stderr, go_table_text_stderr,
        "insert table readback stderr"
    );
    let go_table_text = go_table_text_stdout.expect("Go insert table readback JSON");
    let rust_table_text = rust_table_text_stdout.expect("Rust insert table readback JSON");
    assert_eq!(
        rust_table_text["blocks"], go_table_text["blocks"],
        "insert table readback blocks"
    );
    let table_blocks = rust_table_text["blocks"].as_array().expect("docx blocks");
    assert_eq!(table_blocks.len(), 5, "insert table block count");
    assert_eq!(table_blocks[0]["kind"], Value::String("table".to_string()));
    assert_eq!(
        table_blocks[1]["text"],
        Value::String("After table".to_string())
    );
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_insert_dry_run_and_errors_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-insert-dry-run-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx insert temp dir");
    let dry_docx = temp_dir.join("dry-run.docx");
    fs::copy("testdata/docx/minimal/document.docx", &dry_docx).expect("copy dry-run docx");
    let dry_docx = dry_docx.to_string_lossy().to_string();

    assert_go_rust_match(&[
        "--json",
        "docx",
        "paragraphs",
        "insert",
        &dry_docx,
        "--insert-after",
        "0",
        "--text",
        "Dry run head",
        "--dry-run",
    ]);
    let (text_code, text_stdout, text_stderr) = run_ooxml(&["--json", "docx", "text", &dry_docx]);
    assert_eq!(text_code, 0);
    assert_eq!(text_stderr, None);
    let text_result = text_stdout.expect("dry-run readback");
    let blocks = text_result["blocks"].as_array().expect("docx blocks");
    assert_eq!(blocks.len(), 1, "insert dry-run wrote to document");
    assert_eq!(blocks[0]["text"], Value::String("Hello world".to_string()));

    let text_file = temp_dir.join("text.txt");
    fs::write(&text_file, "x").expect("write text file");
    let text_file = text_file.to_string_lossy().to_string();
    let out = temp_dir.join("bad.docx").to_string_lossy().to_string();
    let missing = temp_dir.join("missing.docx").to_string_lossy().to_string();
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            &missing,
            "--insert-after",
            "-1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            "testdata/docx/styled-headings/document.docx",
            "--insert-after",
            "-1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            "testdata/docx/styled-headings/document.docx",
            "--insert-after",
            "99",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            "testdata/docx/styled-headings/document.docx",
            "--insert-after",
            "1",
            "--text",
            "x",
            "--dry-run",
            "--out",
            &out,
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            "testdata/docx/styled-headings/document.docx",
            "--insert-after",
            "1",
            "--text",
            "x",
            "--text-file",
            &text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--insert-after",
            "0",
            "--text",
            "x",
            "--dry-run",
        ],
    ];
    for args in bad_cases {
        assert_go_rust_match(&args);
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_set_clear_and_handles_match_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-set-clear-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx set/clear temp dir");

    let go_set_out = temp_dir.join("set-go.docx").to_string_lossy().to_string();
    let rust_set_out = temp_dir.join("set-rust.docx").to_string_lossy().to_string();
    let go_set_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/styled-headings/document.docx",
        "--index",
        "1",
        "--text",
        "Updated Heading",
        "--out",
        &go_set_out,
    ];
    let rust_set_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/styled-headings/document.docx",
        "--index",
        "1",
        "--text",
        "Updated Heading",
        "--out",
        &rust_set_out,
    ];
    let (go_set_code, go_set_stdout, go_set_stderr) = run_go_ooxml(&go_set_args);
    let (rust_set_code, rust_set_stdout, rust_set_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_set_code, go_set_code, "set exit");
    assert_eq!(rust_set_stderr, go_set_stderr, "set stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_set_stdout.expect("Rust set stdout")),
        scrub_docx_dynamic_handles(go_set_stdout.expect("Go set stdout")),
        "set stdout"
    );
    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_set_out]);
    assert_eq!(validate_code, 0, "set validate exit");
    assert_eq!(validate_stderr, None, "set validate stderr");

    let (go_text_code, go_text_stdout, go_text_stderr) =
        run_go_ooxml(&["--json", "docx", "text", &go_set_out]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_set_out]);
    assert_eq!(rust_text_code, go_text_code, "set readback exit");
    assert_eq!(rust_text_stderr, go_text_stderr, "set readback stderr");
    let go_text = go_text_stdout.expect("Go set readback");
    let rust_text = rust_text_stdout.expect("Rust set readback");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_text["blocks"].clone()),
        scrub_docx_dynamic_handles(go_text["blocks"].clone()),
        "set readback blocks"
    );
    let set_blocks = rust_text["blocks"].as_array().expect("docx blocks");
    assert_eq!(
        set_blocks[0]["text"],
        Value::String("Updated Heading".to_string())
    );
    assert_eq!(
        set_blocks[0]["style"],
        Value::String("Heading1".to_string())
    );
    assert_eq!(
        set_blocks[1]["text"],
        Value::String("Body text".to_string())
    );

    let go_run_out = temp_dir
        .join("set-run-props-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_run_out = temp_dir
        .join("set-run-props-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_run_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/mixed-blocks/document.docx",
        "--index",
        "2",
        "--text",
        "Updated bold heading",
        "--out",
        &go_run_out,
    ];
    let rust_run_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/mixed-blocks/document.docx",
        "--index",
        "2",
        "--text",
        "Updated bold heading",
        "--out",
        &rust_run_out,
    ];
    let (go_run_code, go_run_stdout, go_run_stderr) = run_go_ooxml(&go_run_args);
    let (rust_run_code, rust_run_stdout, rust_run_stderr) = run_ooxml(&rust_run_args);
    assert_eq!(rust_run_code, go_run_code, "run-props set exit");
    assert_eq!(rust_run_stderr, go_run_stderr, "run-props set stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_run_stdout.expect("Rust run-props set stdout")),
        scrub_docx_dynamic_handles(go_run_stdout.expect("Go run-props set stdout")),
        "run-props set stdout"
    );
    let (go_runs_code, go_runs_stdout, go_runs_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "blocks",
        &go_run_out,
        "--block",
        "2",
        "--include-runs",
    ]);
    let (rust_runs_code, rust_runs_stdout, rust_runs_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "blocks",
        &rust_run_out,
        "--block",
        "2",
        "--include-runs",
    ]);
    assert_eq!(rust_runs_code, go_runs_code, "run-props readback exit");
    assert_eq!(
        rust_runs_stderr, go_runs_stderr,
        "run-props readback stderr"
    );
    let go_runs = go_runs_stdout.expect("Go run-props readback");
    let rust_runs = rust_runs_stdout.expect("Rust run-props readback");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_runs["blocks"].clone()),
        scrub_docx_dynamic_handles(go_runs["blocks"].clone()),
        "run-props readback blocks"
    );
    let run_block = &rust_runs["blocks"].as_array().expect("docx blocks")[0];
    assert_eq!(
        run_block["text"],
        Value::String("Updated bold heading".to_string())
    );
    assert_eq!(run_block["paragraph"]["runs"][0]["bold"], Value::Bool(true));

    let text_file = temp_dir.join("replacement.txt");
    fs::write(&text_file, "line 1\tcol 2\nline 2").expect("write text file");
    let text_file = text_file.to_string_lossy().to_string();
    let go_file_out = temp_dir
        .join("set-file-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_file_out = temp_dir
        .join("set-file-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_file_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/minimal/document.docx",
        "--index",
        "1",
        "--text-file",
        &text_file,
        "--out",
        &go_file_out,
    ];
    let rust_file_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/minimal/document.docx",
        "--index",
        "1",
        "--text-file",
        &text_file,
        "--out",
        &rust_file_out,
    ];
    let (go_file_code, go_file_stdout, go_file_stderr) = run_go_ooxml(&go_file_args);
    let (rust_file_code, rust_file_stdout, rust_file_stderr) = run_ooxml(&rust_file_args);
    assert_eq!(rust_file_code, go_file_code, "set file exit");
    assert_eq!(rust_file_stderr, go_file_stderr, "set file stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_file_stdout.expect("Rust set file stdout")),
        scrub_docx_dynamic_handles(go_file_stdout.expect("Go set file stdout")),
        "set file stdout"
    );
    let (file_text_code, file_text_stdout, file_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_file_out]);
    assert_eq!(file_text_code, 0);
    assert_eq!(file_text_stderr, None);
    let file_blocks = file_text_stdout
        .expect("set file readback")
        .get("blocks")
        .and_then(Value::as_array)
        .cloned()
        .expect("docx blocks");
    assert_eq!(
        file_blocks[0]["text"],
        Value::String("line 1\tcol 2\nline 2".to_string())
    );

    let go_clear_out = temp_dir.join("clear-go.docx").to_string_lossy().to_string();
    let rust_clear_out = temp_dir
        .join("clear-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_clear_args = [
        "--json",
        "docx",
        "paragraphs",
        "clear",
        "testdata/docx/styled-headings/document.docx",
        "--index",
        "1",
        "--out",
        &go_clear_out,
    ];
    let rust_clear_args = [
        "--json",
        "docx",
        "paragraphs",
        "clear",
        "testdata/docx/styled-headings/document.docx",
        "--index",
        "1",
        "--out",
        &rust_clear_out,
    ];
    let (go_clear_code, go_clear_stdout, go_clear_stderr) = run_go_ooxml(&go_clear_args);
    let (rust_clear_code, rust_clear_stdout, rust_clear_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_clear_code, go_clear_code, "clear exit");
    assert_eq!(rust_clear_stderr, go_clear_stderr, "clear stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_clear_stdout.expect("Rust clear stdout")),
        scrub_docx_dynamic_handles(go_clear_stdout.expect("Go clear stdout")),
        "clear stdout"
    );
    let (go_clear_text_code, go_clear_text_stdout, go_clear_text_stderr) =
        run_go_ooxml(&["--json", "docx", "text", &go_clear_out]);
    let (rust_clear_text_code, rust_clear_text_stdout, rust_clear_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_clear_out]);
    assert_eq!(
        rust_clear_text_code, go_clear_text_code,
        "clear readback exit"
    );
    assert_eq!(
        rust_clear_text_stderr, go_clear_text_stderr,
        "clear readback stderr"
    );
    let go_clear_text = go_clear_text_stdout.expect("Go clear readback");
    let rust_clear_text = rust_clear_text_stdout.expect("Rust clear readback");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_clear_text["blocks"].clone()),
        scrub_docx_dynamic_handles(go_clear_text["blocks"].clone()),
        "clear readback blocks"
    );
    let clear_blocks = rust_clear_text["blocks"].as_array().expect("docx blocks");
    assert_eq!(clear_blocks[0]["text"], Value::String(String::new()));
    assert_eq!(
        clear_blocks[0]["style"],
        Value::String("Heading1".to_string())
    );

    let go_stamped = temp_dir
        .join("handle-stamped-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_stamped = temp_dir
        .join("handle-stamped-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_stamp_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/minimal/document.docx",
        "--index",
        "1",
        "--text",
        "Target",
        "--out",
        &go_stamped,
    ];
    let rust_stamp_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        "testdata/docx/minimal/document.docx",
        "--index",
        "1",
        "--text",
        "Target",
        "--out",
        &rust_stamped,
    ];
    let (_, go_stamp_stdout, _) = run_go_ooxml(&go_stamp_args);
    let (_, rust_stamp_stdout, _) = run_ooxml(&rust_stamp_args);
    let go_handle = go_stamp_stdout
        .expect("Go handle stamp")
        .get("handle")
        .and_then(Value::as_str)
        .expect("Go paragraph handle")
        .to_string();
    let rust_handle = rust_stamp_stdout
        .expect("Rust handle stamp")
        .get("handle")
        .and_then(Value::as_str)
        .expect("Rust paragraph handle")
        .to_string();

    let go_prepended = temp_dir
        .join("handle-prepended-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_prepended = temp_dir
        .join("handle-prepended-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_prepend_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        &go_stamped,
        "--insert-after",
        "0",
        "--text",
        "New top",
        "--out",
        &go_prepended,
    ];
    let rust_prepend_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        &rust_stamped,
        "--insert-after",
        "0",
        "--text",
        "New top",
        "--out",
        &rust_prepended,
    ];
    let (go_prepend_code, go_prepend_stdout, go_prepend_stderr) = run_go_ooxml(&go_prepend_args);
    let (rust_prepend_code, rust_prepend_stdout, rust_prepend_stderr) =
        run_ooxml(&rust_prepend_args);
    assert_eq!(rust_prepend_code, go_prepend_code, "prepend exit");
    assert_eq!(rust_prepend_stderr, go_prepend_stderr, "prepend stderr");
    assert_eq!(
        scrub_file_fields(rust_prepend_stdout.expect("Rust prepend stdout")),
        scrub_file_fields(go_prepend_stdout.expect("Go prepend stdout")),
        "prepend stdout"
    );

    let go_resolved = temp_dir
        .join("handle-resolved-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_resolved = temp_dir
        .join("handle-resolved-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_resolve_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        &go_prepended,
        "--handle",
        &go_handle,
        "--text",
        "Same paragraph",
        "--out",
        &go_resolved,
    ];
    let rust_resolve_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        &rust_prepended,
        "--handle",
        &rust_handle,
        "--text",
        "Same paragraph",
        "--out",
        &rust_resolved,
    ];
    let (go_resolve_code, go_resolve_stdout, go_resolve_stderr) = run_go_ooxml(&go_resolve_args);
    let (rust_resolve_code, rust_resolve_stdout, rust_resolve_stderr) =
        run_ooxml(&rust_resolve_args);
    assert_eq!(rust_resolve_code, go_resolve_code, "handle resolve exit");
    assert_eq!(
        rust_resolve_stderr, go_resolve_stderr,
        "handle resolve stderr"
    );
    let rust_resolve_result = rust_resolve_stdout.expect("Rust handle resolve stdout");
    let go_resolve_result = go_resolve_stdout.expect("Go handle resolve stdout");
    assert_eq!(
        scrub_file_fields(scrub_docx_dynamic_handles(rust_resolve_result.clone())),
        scrub_file_fields(scrub_docx_dynamic_handles(go_resolve_result)),
        "handle resolve stdout"
    );
    assert_eq!(rust_resolve_result["index"], Value::from(2));
    assert_eq!(
        rust_resolve_result["previousText"],
        Value::String("Target".to_string())
    );
    let (resolved_text_code, resolved_text_stdout, resolved_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_resolved]);
    assert_eq!(resolved_text_code, 0);
    assert_eq!(resolved_text_stderr, None);
    let resolved_blocks = resolved_text_stdout
        .expect("handle resolved readback")
        .get("blocks")
        .and_then(Value::as_array)
        .cloned()
        .expect("docx blocks");
    assert_eq!(
        resolved_blocks[0]["text"],
        Value::String("New top".to_string())
    );
    assert_eq!(
        resolved_blocks[1]["text"],
        Value::String("Same paragraph".to_string())
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_set_clear_errors_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-set-clear-errors-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx set/clear errors temp dir");
    let text_file = temp_dir.join("text.txt");
    fs::write(&text_file, "x").expect("write text file");
    let text_file = text_file.to_string_lossy().to_string();
    let empty_text_file = temp_dir.join("empty.txt");
    fs::write(&empty_text_file, "").expect("write empty text file");
    let empty_text_file = empty_text_file.to_string_lossy().to_string();
    let missing_text_file = temp_dir.join("missing.txt").to_string_lossy().to_string();
    let missing = temp_dir.join("missing.docx").to_string_lossy().to_string();
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            &missing,
            "--index",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--handle",
            "H:docx/pt:doc/para:m:ABCDEF01",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--text",
            "x",
            "--text-file",
            &text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--text",
            "",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--text-file",
            &empty_text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--text-file",
            &missing_text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/mixed-blocks/document.docx",
            "--index",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/styled-headings/document.docx",
            "--index",
            "99",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--index",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--handle",
            "H:pptx/s:256/shape:n:2",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/minimal/document.docx",
            "--handle",
            "H:docx/pt:doc/para:m:DOESNOTEXIST",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/paraid-dup/document.docx",
            "--handle",
            "H:docx/pt:doc/para:m:DEAD00FF",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "clear",
            "testdata/docx/minimal/document.docx",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "clear",
            "testdata/docx/minimal/document.docx",
            "--index",
            "1",
            "--handle",
            "H:docx/pt:doc/para:m:ABCDEF01",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "clear",
            "testdata/docx/mixed-blocks/document.docx",
            "--index",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "clear",
            "testdata/docx/styled-headings/document.docx",
            "--index",
            "99",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "clear",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--index",
            "1",
            "--dry-run",
        ],
    ];
    for args in bad_cases {
        assert_go_rust_match(&args);
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_styles_list_and_show_match_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "styles",
            "list",
            "testdata/docx/styles-catalog/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "list",
            "testdata/docx/styles-catalog/document.docx",
            "--type",
            "paragraph",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "list",
            "testdata/docx/styles-catalog/document.docx",
            "--type",
            "Paragraph",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "show",
            "testdata/docx/styles-catalog/document.docx",
            "--style",
            "Heading1",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "show",
            "testdata/docx/styles-catalog/document.docx",
            "--style",
            "NonExistent",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "show",
            "testdata/docx/minimal/document.docx",
            "--style",
            "Heading1",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "list",
            "testdata/docx/styles-catalog/document.docx",
            "--type",
            "list",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "show",
            "testdata/docx/styles-catalog/document.docx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn docx_styles_apply_matches_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-styles-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx styles temp dir");

    let go_para_out = temp_dir
        .join("apply-para-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_para_out = temp_dir
        .join("apply-para-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_para_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "1",
        "--target",
        "paragraph",
        "--style",
        "Heading2",
        "--out",
        &go_para_out,
    ];
    let rust_para_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "1",
        "--target",
        "paragraph",
        "--style",
        "Heading2",
        "--out",
        &rust_para_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_para_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_para_args);
    assert_eq!(rust_code, go_code, "paragraph apply exit");
    assert_eq!(rust_stderr, go_stderr, "paragraph apply stderr");
    assert_eq!(
        scrub_file_fields(scrub_docx_dynamic_handles(
            rust_stdout.expect("Rust paragraph style apply stdout")
        )),
        scrub_file_fields(scrub_docx_dynamic_handles(
            go_stdout.expect("Go paragraph style apply stdout")
        )),
        "paragraph apply stdout"
    );
    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_para_out]);
    assert_eq!(validate_code, 0, "paragraph apply validate exit");
    assert_eq!(validate_stderr, None, "paragraph apply validate stderr");
    let (blocks_code, blocks_stdout, blocks_stderr) =
        run_ooxml(&["--json", "docx", "blocks", &rust_para_out, "--block", "1"]);
    assert_eq!(blocks_code, 0, "paragraph apply readback exit");
    assert_eq!(blocks_stderr, None, "paragraph apply readback stderr");
    let blocks = blocks_stdout.expect("paragraph apply blocks");
    assert_eq!(
        blocks["blocks"][0]["paragraph"]["style"],
        Value::String("Heading2".to_string())
    );

    let go_run_out = temp_dir
        .join("apply-run-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_run_out = temp_dir
        .join("apply-run-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_run_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "2",
        "--target",
        "run",
        "--style",
        "Emphasis",
        "--out",
        &go_run_out,
    ];
    let rust_run_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "2",
        "--target",
        "run",
        "--style",
        "Emphasis",
        "--out",
        &rust_run_out,
    ];
    let (go_run_code, go_run_stdout, go_run_stderr) = run_go_ooxml(&go_run_args);
    let (rust_run_code, rust_run_stdout, rust_run_stderr) = run_ooxml(&rust_run_args);
    assert_eq!(rust_run_code, go_run_code, "run apply exit");
    assert_eq!(rust_run_stderr, go_run_stderr, "run apply stderr");
    assert_eq!(
        scrub_file_fields(scrub_docx_dynamic_handles(
            rust_run_stdout.expect("Rust run style apply stdout")
        )),
        scrub_file_fields(scrub_docx_dynamic_handles(
            go_run_stdout.expect("Go run style apply stdout")
        )),
        "run apply stdout"
    );
    assert!(
        read_zip_string(Path::new(&rust_run_out), "word/document.xml")
            .contains("w:rStyle w:val=\"Emphasis\""),
        "run style was not written to document.xml"
    );

    let go_table_out = temp_dir
        .join("apply-table-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_table_out = temp_dir
        .join("apply-table-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_table_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "1",
        "--target",
        "table",
        "--style",
        "TableGrid",
        "--out",
        &go_table_out,
    ];
    let rust_table_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "1",
        "--target",
        "table",
        "--style",
        "TableGrid",
        "--out",
        &rust_table_out,
    ];
    let (go_table_code, go_table_stdout, go_table_stderr) = run_go_ooxml(&go_table_args);
    let (rust_table_code, rust_table_stdout, rust_table_stderr) = run_ooxml(&rust_table_args);
    assert_eq!(rust_table_code, go_table_code, "table apply exit");
    assert_eq!(rust_table_stderr, go_table_stderr, "table apply stderr");
    assert_eq!(
        scrub_file_fields(rust_table_stdout.expect("Rust table style apply stdout")),
        scrub_file_fields(go_table_stdout.expect("Go table style apply stdout")),
        "table apply stdout"
    );
    let table_xml = read_zip_string(Path::new(&rust_table_out), "word/document.xml");
    assert!(
        table_xml.contains("w:tblStyle w:val=\"TableGrid\""),
        "table style was not written to document.xml"
    );

    let (hash_code, hash_stdout, hash_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "blocks",
        "testdata/docx/apply-styles/document.docx",
        "--block",
        "1",
    ]);
    assert_eq!(hash_code, 0, "hash readback exit");
    assert_eq!(hash_stderr, None, "hash readback stderr");
    let hash_json = hash_stdout.expect("hash readback");
    let hash = hash_json["blocks"][0]["contentHash"]
        .as_str()
        .expect("content hash");
    let hash_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "1",
        "--target",
        "paragraph",
        "--style",
        "Heading2",
        "--expect-hash",
        hash,
        "--dry-run",
    ];
    let (go_hash_code, go_hash_stdout, go_hash_stderr) = run_go_ooxml(&hash_args);
    let (rust_hash_code, rust_hash_stdout, rust_hash_stderr) = run_ooxml(&hash_args);
    assert_eq!(rust_hash_code, go_hash_code, "hash guarded apply exit");
    assert_eq!(
        rust_hash_stderr, go_hash_stderr,
        "hash guarded apply stderr"
    );
    assert_eq!(
        scrub_docx_dynamic_handles(rust_hash_stdout.expect("Rust hash apply stdout")),
        scrub_docx_dynamic_handles(go_hash_stdout.expect("Go hash apply stdout")),
        "hash guarded apply stdout"
    );

    let style_handle_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/styled-headings/document.docx",
        "--index",
        "1",
        "--target",
        "paragraph",
        "--style",
        "H:docx/pt:styles/style:n:Heading1",
        "--no-validate",
        "--dry-run",
    ];
    let (go_handle_code, go_handle_stdout, go_handle_stderr) = run_go_ooxml(&style_handle_args);
    let (rust_handle_code, rust_handle_stdout, rust_handle_stderr) = run_ooxml(&style_handle_args);
    assert_eq!(rust_handle_code, go_handle_code, "style handle apply exit");
    assert_eq!(
        rust_handle_stderr, go_handle_stderr,
        "style handle apply stderr"
    );
    assert_eq!(
        scrub_docx_dynamic_handles(rust_handle_stdout.expect("Rust style handle stdout")),
        scrub_docx_dynamic_handles(go_handle_stdout.expect("Go style handle stdout")),
        "style handle apply stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_styles_apply_errors_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-styles-errors-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx styles errors temp dir");
    let out = temp_dir.join("bad.docx").to_string_lossy().to_string();
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "0",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--target",
            "bogus",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "99",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "NoSuchStyle",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "Emphasis",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "3",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--expect-hash",
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--handle",
            "H:docx/pt:doc/para:m:ABCDEF01",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--handle",
            "H:docx/pt:doc/para:m:ABCDEF01",
            "--target",
            "table",
            "--style",
            "TableGrid",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--out",
            &out,
        ],
    ];
    for args in bad_cases {
        assert_go_rust_match(&args);
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_comments_list_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "comments",
            "list",
            "testdata/docx/with-comments/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "comments",
            "list",
            "testdata/docx/with-comments/document.docx",
            "--comment-id",
            "0",
        ],
        vec![
            "--json",
            "docx",
            "comments",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "comments",
            "list",
            "testdata/docx/with-comments/document.docx",
            "--comment-id",
            "99",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn docx_comments_add_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-comments-add-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx comments temp dir");
    let go_out = temp_dir.join("comments-go.docx");
    let rust_out = temp_dir.join("comments-rust.docx");
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "docx",
        "comments",
        "add",
        "testdata/docx/minimal/document.docx",
        "--anchor-block",
        "1",
        "--author",
        "Bob",
        "--initials",
        "BB",
        "--text",
        "Brand new",
        "--date",
        "2025-06-06T10:30:00Z",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "comments",
        "add",
        "testdata/docx/minimal/document.docx",
        "--anchor-block",
        "1",
        "--author",
        "Bob",
        "--initials",
        "BB",
        "--text",
        "Brand new",
        "--date",
        "2025-06-06T10:30:00Z",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "comments add exit");
    assert_eq!(rust_stderr, go_stderr, "comments add stderr");
    assert_eq!(rust_stdout, go_stdout, "comments add stdout");
    assert!(
        Path::new(&rust_out).exists(),
        "Rust comments output missing"
    );

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "validate exit");
    assert_eq!(validate_stderr, None, "validate stderr");
    assert!(validate_stdout.is_some(), "validate stdout");

    let (go_list_code, go_list_stdout, go_list_stderr) =
        run_go_ooxml(&["--json", "docx", "comments", "list", &go_out]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) =
        run_ooxml(&["--json", "docx", "comments", "list", &rust_out]);
    assert_eq!(rust_list_code, go_list_code, "comments list readback exit");
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "comments list readback stderr"
    );
    let go_list = go_list_stdout.expect("Go comments list JSON");
    let rust_list = rust_list_stdout.expect("Rust comments list JSON");
    assert_eq!(
        rust_list["comments"], go_list["comments"],
        "comments list readback"
    );
    assert_eq!(
        rust_list["comments"][0]["text"],
        Value::String("Brand new".to_string())
    );
    assert_eq!(
        rust_list["comments"][0]["author"],
        Value::String("Bob".to_string())
    );

    let dry_run = [
        "--json",
        "docx",
        "comments",
        "add",
        "testdata/docx/minimal/document.docx",
        "--author",
        "Bob",
        "--text",
        "Dry run",
        "--date",
        "2025-06-06T10:30:00Z",
        "--dry-run",
    ];
    assert_go_rust_match(&dry_run);

    let missing_author = [
        "--json",
        "docx",
        "comments",
        "add",
        "testdata/docx/minimal/document.docx",
        "--text",
        "No author",
        "--dry-run",
    ];
    assert_go_rust_match(&missing_author);

    let unsupported_type = [
        "--json",
        "docx",
        "comments",
        "add",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--author",
        "Bob",
        "--text",
        "Wrong package",
        "--date",
        "2025-06-06T10:30:00Z",
        "--dry-run",
    ];
    assert_go_rust_match(&unsupported_type);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_comments_edit_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-comments-edit-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx comments temp dir");
    let go_out = temp_dir.join("comments-edit-go.docx");
    let rust_out = temp_dir.join("comments-edit-rust.docx");
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let (hash_code, hash_stdout, hash_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "comments",
        "list",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
    ]);
    assert_eq!(hash_code, 0, "hash list exit");
    assert_eq!(hash_stderr, None, "hash list stderr");
    let hash_json = hash_stdout.expect("hash list JSON");
    let hash = hash_json["comments"][0]["contentHash"]
        .as_str()
        .expect("comment content hash");

    let go_args = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--text",
        "Updated comment",
        "--author",
        "Carol",
        "--date",
        "2030-01-02T03:04:05Z",
        "--expect-hash",
        hash,
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--text",
        "Updated comment",
        "--author",
        "Carol",
        "--date",
        "2030-01-02T03:04:05Z",
        "--expect-hash",
        hash,
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "comments edit exit");
    assert_eq!(rust_stderr, go_stderr, "comments edit stderr");
    assert_eq!(rust_stdout, go_stdout, "comments edit stdout");
    assert!(Path::new(&rust_out).exists(), "Rust edit output missing");

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "validate exit");
    assert_eq!(validate_stderr, None, "validate stderr");
    assert!(validate_stdout.is_some(), "validate stdout");

    let (go_list_code, go_list_stdout, go_list_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "comments",
        "list",
        &go_out,
        "--comment-id",
        "0",
    ]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "comments",
        "list",
        &rust_out,
        "--comment-id",
        "0",
    ]);
    assert_eq!(rust_list_code, go_list_code, "comments edit readback exit");
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "comments edit readback stderr"
    );
    let go_list = go_list_stdout.expect("Go comments edit readback JSON");
    let rust_list = rust_list_stdout.expect("Rust comments edit readback JSON");
    assert_eq!(
        rust_list["comments"], go_list["comments"],
        "comments edit readback"
    );

    let wrong_hash = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--text",
        "x",
        "--expect-hash",
        "sha256:bogus",
        "--dry-run",
    ];
    assert_go_rust_match(&wrong_hash);

    let by_handle = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/docx/with-comments/document.docx",
        "--handle",
        "H:docx/pt:doc/comment:n:0",
        "--text",
        "Edited by handle",
        "--date",
        "2031-02-03T04:05:06Z",
        "--dry-run",
    ];
    assert_go_rust_match(&by_handle);

    let stale_handle = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/docx/with-comments/document.docx",
        "--handle",
        "H:docx/pt:doc/comment:n:9999",
        "--text",
        "x",
        "--dry-run",
    ];
    assert_go_rust_match(&stale_handle);

    let unsupported_type = [
        "--json",
        "docx",
        "comments",
        "edit",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--comment-id",
        "0",
        "--text",
        "Wrong package",
        "--dry-run",
    ];
    assert_go_rust_match(&unsupported_type);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_comments_remove_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-comments-remove-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx comments temp dir");
    let go_out = temp_dir.join("comments-remove-go.docx");
    let rust_out = temp_dir.join("comments-remove-rust.docx");
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let (hash_code, hash_stdout, hash_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "comments",
        "list",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
    ]);
    assert_eq!(hash_code, 0, "hash list exit");
    assert_eq!(hash_stderr, None, "hash list stderr");
    let hash_json = hash_stdout.expect("hash list JSON");
    let hash = hash_json["comments"][0]["contentHash"]
        .as_str()
        .expect("comment content hash");

    let go_args = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--expect-hash",
        hash,
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--expect-hash",
        hash,
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "comments remove exit");
    assert_eq!(rust_stderr, go_stderr, "comments remove stderr");
    assert_eq!(rust_stdout, go_stdout, "comments remove stdout");
    assert!(Path::new(&rust_out).exists(), "Rust remove output missing");

    let remove_json = rust_stdout.expect("Rust remove JSON");
    assert_eq!(
        remove_json["operation"],
        Value::String("removed".to_string())
    );
    assert_eq!(remove_json["rangeMarkersRemoved"], Value::Bool(true));

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "validate exit");
    assert_eq!(validate_stderr, None, "validate stderr");
    assert!(validate_stdout.is_some(), "validate stdout");

    let (go_list_code, go_list_stdout, go_list_stderr) =
        run_go_ooxml(&["--json", "docx", "comments", "list", &go_out]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) =
        run_ooxml(&["--json", "docx", "comments", "list", &rust_out]);
    assert_eq!(
        rust_list_code, go_list_code,
        "comments remove readback exit"
    );
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "comments remove readback stderr"
    );
    let go_list = go_list_stdout.expect("Go comments remove readback JSON");
    let rust_list = rust_list_stdout.expect("Rust comments remove readback JSON");
    assert_eq!(
        rust_list["comments"], go_list["comments"],
        "comments remove readback"
    );
    assert_eq!(rust_list["comments"], Value::Array(Vec::new()));

    let rust_document_xml = read_zip_string(Path::new(&rust_out), "word/document.xml");
    assert!(
        !rust_document_xml.contains("commentRangeStart")
            && !rust_document_xml.contains("commentRangeEnd")
            && !rust_document_xml.contains("commentReference"),
        "comment markers/reference survived removal:\n{rust_document_xml}"
    );

    let wrong_hash = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--comment-id",
        "0",
        "--expect-hash",
        "sha256:bogus",
        "--dry-run",
    ];
    assert_go_rust_match(&wrong_hash);

    let by_handle = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--handle",
        "H:docx/pt:doc/comment:n:0",
        "--dry-run",
    ];
    assert_go_rust_match(&by_handle);

    let stale_handle = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--handle",
        "H:docx/pt:doc/comment:n:9999",
        "--dry-run",
    ];
    assert_go_rust_match(&stale_handle);

    let no_comments = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/minimal/document.docx",
        "--comment-id",
        "0",
        "--dry-run",
    ];
    assert_go_rust_match(&no_comments);

    let missing_id = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--dry-run",
    ];
    assert_go_rust_match(&missing_id);

    let unsupported_type = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--comment-id",
        "0",
        "--dry-run",
    ];
    assert_go_rust_match(&unsupported_type);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_fields_list_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "fields",
            "list",
            "testdata/docx/with-fields/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "fields",
            "list",
            "testdata/docx/with-fields/document.docx",
            "--type",
            "PAGE",
        ],
        vec![
            "--json",
            "docx",
            "fields",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "fields",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-fields-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx fields temp dir");

    let ordered_docx = temp_dir.join("ordered-fields.docx");
    write_docx_with_body(
        &ordered_docx,
        r#"    <w:p>
      <w:r><w:fldChar w:fldCharType="begin"/></w:r>
      <w:r><w:instrText xml:space="preserve"> NUMPAGES </w:instrText></w:r>
      <w:r><w:fldChar w:fldCharType="separate"/></w:r>
      <w:r><w:t>3</w:t></w:r>
      <w:r><w:fldChar w:fldCharType="end"/></w:r>
      <w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple>
    </w:p>"#,
    );
    let ordered_docx = ordered_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "fields", "list", &ordered_docx]);

    let switched_docx = temp_dir.join("switched-field.docx");
    write_docx_with_body(
        &switched_docx,
        r#"    <w:p>
      <w:fldSimple w:instr=" PAGE \* MERGEFORMAT "><w:r><w:t>1</w:t></w:r></w:fldSimple>
    </w:p>"#,
    );
    let switched_docx = switched_docx.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "docx",
        "fields",
        "list",
        &switched_docx,
        "--type",
        "PAGE",
    ]);

    let table_docx = temp_dir.join("table-field.docx");
    write_docx_with_body(
        &table_docx,
        r#"    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p>
            <w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple>
          </w:p>
        </w:tc>
      </w:tr>
    </w:tbl>"#,
    );
    let table_docx = table_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "fields", "list", &table_docx]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_headers_and_footers_list_match_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "headers",
            "list",
            "testdata/docx/headers/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "list",
            "testdata/docx/headers/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "headers",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "headers",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn docx_headers_and_footers_show_match_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "headers",
            "show",
            "testdata/docx/headers/document.docx",
            "--type",
            "default",
        ],
        vec![
            "--json",
            "docx",
            "headers",
            "show",
            "testdata/docx/headers/document.docx",
            "--selector",
            "header:1:default",
        ],
        vec![
            "--json",
            "docx",
            "headers",
            "show",
            "testdata/docx/headers/document.docx",
            "--selector",
            "id:rId10/p:1",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "show",
            "testdata/docx/headers/document.docx",
            "--id",
            "rId11",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "show",
            "testdata/docx/headers/document.docx",
            "--selector",
            "footer:1:default",
        ],
        vec![
            "--json",
            "docx",
            "headers",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn docx_images_list_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "images",
            "list",
            "testdata/docx/with-image/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "images",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "images",
            "list",
            "testdata/docx/with-media/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "images",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn frozen_pptx_mutation_and_validate_match_go_baseline() {
    let baseline = baseline();
    let temp_dir = std::env::temp_dir().join(format!("ooxml-rust-contract-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let edited = temp_dir.join("edited.pptx");
    let render_dir = temp_dir.join("rendered");
    let edited_str = edited.to_str().expect("temp path");
    let render_dir_str = render_dir.to_str().expect("render path");

    let edit_args = [
        "--json",
        "pptx",
        "replace",
        "text",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--slide",
        "1",
        "--target",
        "title",
        "--text",
        "Rust Port Contract",
        "--out",
        edited_str,
    ];
    let (edit_code, edit_stdout, edit_stderr) = run_ooxml(&edit_args);
    assert_eq!(edit_code, 0);
    assert_eq!(edit_stderr, None);
    let edit_expected = baseline["mutation"]["edit"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            edit_stdout.expect("edit stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        edit_expected
    );
    assert!(edited.exists());

    let validate_args = ["--json", "--strict", "validate", edited_str];
    let (validate_code, validate_stdout, validate_stderr) = run_ooxml(&validate_args);
    assert_eq!(validate_code, 0);
    assert_eq!(validate_stderr, None);
    let validate_expected = baseline["mutation"]["validate"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            validate_stdout.expect("validate stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        validate_expected
    );

    let render_args = [
        "pptx",
        "render",
        edited_str,
        "--out",
        render_dir_str,
        "--slides",
        "1",
        "--format",
        "json",
    ];
    let (render_code, render_stdout, render_stderr) =
        run_ooxml_with_env(&render_args, &[("OOXML_RUST_MOCK_RENDER", "1")]);
    assert_eq!(render_code, 0);
    assert_eq!(render_stderr, None);
    let render_expected = baseline["mutation"]["render"]["stdoutJson"].clone();
    assert_eq!(
        scrub_paths(
            render_stdout.expect("render stdout"),
            &[
                (edited_str, "[EDITED_PPTX]"),
                (render_dir_str, "[RENDER_DIR]")
            ]
        ),
        render_expected
    );

    let verify_args = [
        "--format",
        "json",
        "verify",
        edited_str,
        "--baseline",
        "testdata/pptx/minimal-title/presentation.pptx",
    ];
    let (verify_code, verify_stdout, verify_stderr) = run_ooxml(&verify_args);
    assert_eq!(verify_code, 0);
    assert_eq!(verify_stderr, None);
    let verify_expected = baseline["mutation"]["verify"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            verify_stdout.expect("verify stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        verify_expected
    );
}

#[test]
fn frozen_serve_flow_matches_go_baseline() {
    let baseline = baseline();
    let temp_dir = std::env::temp_dir().join(format!("ooxml-rust-serve-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);
    let mut replacements = vec![
        (input_str.clone(), "[SERVE_INPUT_XLSX]".to_string()),
        (output_str.clone(), "[SERVE_OUT_XLSX]".to_string()),
    ];
    let mut flow = Vec::new();

    let open = rpc_request(
        1,
        "open",
        serde_json::json!({"file": input_str, "out": output_str}),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();
    replacements.push((session.clone(), "[SESSION]".to_string()));
    flow.push(flow_item("open", open, open_response, &replacements));

    let op = rpc_request(
        2,
        "op",
        serde_json::json!({
            "session": session,
            "command": "xlsx cells set",
            "args": {"sheet": "1", "cell": "A1", "value": "serve-contract"},
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    let working = op_response["result"]["readback"]["file"]
        .as_str()
        .expect("working package")
        .to_string();
    replacements.push((working, "[SESSION_WORKING_PACKAGE]".to_string()));
    flow.push(flow_item("op", op, op_response, &replacements));

    let inspect = rpc_request(
        3,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx ranges export",
            "args": {"sheet": "1", "range": "A1", "include-types": true},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    flow.push(flow_item(
        "inspect",
        inspect,
        inspect_response,
        &replacements,
    ));

    for (id, method) in [(4, "validate"), (5, "plan"), (6, "commit")] {
        let request = rpc_request(id, method, serde_json::json!({"session": session}));
        let response = serve_roundtrip(&mut stdin, &mut reader, &request);
        flow.push(flow_item(method, request, response, &replacements));
    }

    let dry_open = rpc_request(
        7,
        "open",
        serde_json::json!({"file": input_str, "dryRun": true}),
    );
    let dry_open_response = serve_roundtrip(&mut stdin, &mut reader, &dry_open);
    let dry_session = dry_open_response["result"]["sessionId"]
        .as_str()
        .expect("dry session id")
        .to_string();
    replacements.push((dry_session.clone(), "[DRY_RUN_SESSION]".to_string()));
    flow.push(flow_item(
        "open",
        dry_open,
        dry_open_response,
        &replacements,
    ));

    let abort = rpc_request(8, "abort", serde_json::json!({"session": dry_session}));
    let abort_response = serve_roundtrip(&mut stdin, &mut reader, &abort);
    flow.push(flow_item("abort", abort, abort_response, &replacements));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    assert_eq!(Value::Array(flow), baseline["serve"]["flow"]);
}

#[test]
fn serve_inspect_supports_xlsx_cells_extract() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-cells-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(1, "open", serde_json::json!({"file": input_str}));
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let inspect = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx cells extract",
            "args": {"sheet": "1", "range": "B1:D2", "includeEmpty": true, "maxRows": 2},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    let working = inspect_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "cells",
        "extract",
        working,
        "--sheet",
        "1",
        "--range",
        "B1:D2",
        "--include-empty",
        "--max-rows",
        "2",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(
        inspect_response["result"],
        expected.expect("extract stdout")
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_inspect_supports_xlsx_sheets_show() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-sheets-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    std::fs::copy("testdata/xlsx/used-range/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(1, "open", serde_json::json!({"file": input_str}));
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let inspect = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx sheets show",
            "args": {"sheet": "sheetId:1"},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    let working = inspect_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "sheets",
        "show",
        working,
        "--sheet",
        "sheetId:1",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(inspect_response["result"], expected.expect("show stdout"));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_inspect_supports_xlsx_tables_show() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-tables-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("table-workbook.xlsx");
    write_table_xlsx(&input);
    let input_str = input.to_str().expect("input path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open = rpc_request(1, "open", serde_json::json!({"file": input_str}));
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let list = rpc_request(
        2,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables list",
            "args": {"sheet": "Data"},
        }),
    );
    let list_response = serve_roundtrip(&mut stdin, &mut reader, &list);
    let working = list_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json", "xlsx", "tables", "list", working, "--sheet", "Data",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(list_response["result"], expected.expect("list stdout"));

    let inspect = rpc_request(
        3,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables show",
            "args": {"sheet": "Data", "table": "Sales"},
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    let working = inspect_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json", "xlsx", "tables", "show", working, "--sheet", "Data", "--table", "Sales",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(inspect_response["result"], expected.expect("show stdout"));

    let export = rpc_request(
        4,
        "inspect",
        serde_json::json!({
            "session": session,
            "command": "xlsx tables export",
            "args": {"sheet": "Data", "table": "Sales", "includeTypes": true, "includeFormulas": true},
        }),
    );
    let export_response = serve_roundtrip(&mut stdin, &mut reader, &export);
    let working = export_response["result"]["file"]
        .as_str()
        .expect("working file");
    let (code, expected, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "tables",
        "export",
        working,
        "--sheet",
        "Data",
        "--table",
        "Sales",
        "--include-types",
        "--include-formulas",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    assert_eq!(export_response["result"], expected.expect("export stdout"));

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
}

#[test]
fn serve_pptx_generic_web_agent_edit_path_works() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-serve-pptx-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = "testdata/pptx/minimal-title/presentation.pptx";
    let output = temp_dir.join("serve-pptx-out.pptx");
    let output_str = output.to_str().expect("output path").to_string();
    let marker = format!("Rust serve web {}", std::process::id());

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            1,
            "open",
            serde_json::json!({"file": input, "out": output_str}),
        ),
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let list_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx slides list",
                "args": {},
            }),
        ),
    );
    assert_eq!(
        list_response["result"]["slides"][0]["number"],
        Value::from(1)
    );

    let inspect_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            3,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx slides show",
                "args": {"slide": 1, "include-text": true},
            }),
        ),
    );
    assert_eq!(
        inspect_response["result"]["slides"][0]["shapes"][0]["textContent"],
        Value::String("Minimal Title Slide".to_string())
    );

    let shapes_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            31,
            "inspect",
            serde_json::json!({
                "session": session,
                "command": "pptx shapes show",
                "args": {"slide": 1, "include-text": true, "include-bounds": true},
            }),
        ),
    );
    assert_eq!(
        shapes_response["result"]["shapes"][0]["primarySelector"],
        Value::String("title".to_string())
    );

    let op_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            4,
            "op",
            serde_json::json!({
                "session": session,
                "command": "pptx replace text",
                "args": {"slide": 1, "target": "title", "text": marker},
            }),
        ),
    );
    assert_eq!(op_response["result"]["readback"]["newText"], marker);

    for (id, method) in [(5, "validate"), (6, "commit")] {
        let response = serve_roundtrip(
            &mut stdin,
            &mut reader,
            &rpc_request(id, method, serde_json::json!({"session": session})),
        );
        assert!(
            response.get("error").is_none(),
            "{method} returned error: {response:?}"
        );
    }

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "slides",
        "show",
        &output_str,
        "--slide",
        "1",
        "--include-text",
    ]);
    assert_eq!(show_code, 0);
    assert_eq!(show_stderr, None);
    assert_eq!(
        show_stdout.expect("show stdout")["slides"][0]["shapes"][0]["textContent"],
        Value::String(marker)
    );
}

#[test]
fn frozen_mcp_discovery_and_flow_match_go_baseline() {
    let baseline = baseline();
    let temp_dir = std::env::temp_dir().join(format!("ooxml-rust-mcp-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("mcp-out.xlsx");
    std::fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn mcp");
    let mut stdin = child.stdin.take().expect("mcp stdin");
    let stdout = child.stdout.take().expect("mcp stdout");
    let mut reader = BufReader::new(stdout);

    let initialize = rpc_request(
        1,
        "initialize",
        serde_json::json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "rust-contract", "version": "0.0.0"},
        }),
    );
    let initialize_response = serve_roundtrip(&mut stdin, &mut reader, &initialize);
    let tools_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(2, "tools/list", serde_json::json!({})),
    );
    let resources_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(3, "resources/list", serde_json::json!({})),
    );
    let templates_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(4, "resources/templates/list", serde_json::json!({})),
    );
    let command_uri = "resource://command/xlsx%20cells%20set";
    let command_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(5, "resources/read", serde_json::json!({"uri": command_uri})),
    );
    let discovery = serde_json::json!({
        "initialize": initialize_response["result"].clone(),
        "tools": summarize_mcp_tools(&tools_response["result"]),
        "resources": sort_by_string_field(resources_response["result"]["resources"].clone(), "uri"),
        "resourceTemplates": templates_response["result"]["resourceTemplates"].clone(),
        "commandResource": summarize_mcp_command_resource(&command_response["result"], command_uri),
    });
    assert_eq!(discovery, baseline["mcp"]["discovery"]);

    let mut replacements = vec![
        (input_str.clone(), "[MCP_INPUT_XLSX]".to_string()),
        (output_str.clone(), "[MCP_OUT_XLSX]".to_string()),
    ];
    let mut flow = Vec::new();

    let open = rpc_request(
        6,
        "tools/call",
        serde_json::json!({
            "name": "open",
            "arguments": {"file": input_str, "out": output_str},
        }),
    );
    let open_response = serve_roundtrip(&mut stdin, &mut reader, &open);
    let session = open_response["result"]["structuredContent"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();
    replacements.push((session.clone(), "[MCP_SESSION]".to_string()));
    flow.push(flow_item("tools/call", open, open_response, &replacements));

    let op = rpc_request(
        7,
        "tools/call",
        serde_json::json!({
            "name": "op",
            "arguments": {
                "session": session,
                "command": "xlsx cells set",
                "args": {"sheet": "1", "cell": "A1", "value": "mcp-contract"},
            },
        }),
    );
    let op_response = serve_roundtrip(&mut stdin, &mut reader, &op);
    let working = op_response["result"]["structuredContent"]["readback"]["file"]
        .as_str()
        .expect("working package")
        .to_string();
    replacements.push((working, "[SESSION_WORKING_PACKAGE]".to_string()));
    flow.push(flow_item("tools/call", op, op_response, &replacements));

    let inspect = rpc_request(
        8,
        "tools/call",
        serde_json::json!({
            "name": "inspect",
            "arguments": {
                "session": session,
                "command": "xlsx ranges export",
                "args": {"sheet": "1", "range": "A1", "include-types": true},
            },
        }),
    );
    let inspect_response = serve_roundtrip(&mut stdin, &mut reader, &inspect);
    flow.push(flow_item(
        "tools/call",
        inspect,
        inspect_response,
        &replacements,
    ));

    for (id, name) in [(9, "validate"), (10, "plan"), (11, "commit")] {
        let request = rpc_request(
            id,
            "tools/call",
            serde_json::json!({"name": name, "arguments": {"session": session}}),
        );
        let response = serve_roundtrip(&mut stdin, &mut reader, &request);
        flow.push(flow_item("tools/call", request, response, &replacements));
    }

    let dry_open = rpc_request(
        12,
        "tools/call",
        serde_json::json!({
            "name": "open",
            "arguments": {"file": input_str, "dryRun": true},
        }),
    );
    let dry_open_response = serve_roundtrip(&mut stdin, &mut reader, &dry_open);
    let dry_session = dry_open_response["result"]["structuredContent"]["sessionId"]
        .as_str()
        .expect("dry session id")
        .to_string();
    replacements.push((dry_session.clone(), "[MCP_DRY_RUN_SESSION]".to_string()));
    flow.push(flow_item(
        "tools/call",
        dry_open,
        dry_open_response,
        &replacements,
    ));

    let abort = rpc_request(
        13,
        "tools/call",
        serde_json::json!({"name": "abort", "arguments": {"session": dry_session}}),
    );
    let abort_response = serve_roundtrip(&mut stdin, &mut reader, &abort);
    flow.push(flow_item(
        "tools/call",
        abort,
        abort_response,
        &replacements,
    ));

    drop(stdin);
    let status = child.wait().expect("mcp exit");
    assert!(status.success());
    assert_eq!(Value::Array(flow), baseline["mcp"]["flow"]["flow"]);
}

#[test]
fn mcp_command_resources_cover_advertised_rust_capabilities() {
    let (cap_code, cap_stdout, cap_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(cap_code, 0);
    assert_eq!(cap_stderr, None);
    let capabilities = cap_stdout.expect("capabilities stdout");
    let commands = capabilities["commands"]
        .as_array()
        .expect("capability commands");

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn mcp");
    let mut stdin = child.stdin.take().expect("mcp stdin");
    let stdout = child.stdout.take().expect("mcp stdout");
    let mut reader = BufReader::new(stdout);

    let initialize = rpc_request(
        1,
        "initialize",
        serde_json::json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "rust-contract", "version": "0.0.0"},
        }),
    );
    let initialize_response = serve_roundtrip(&mut stdin, &mut reader, &initialize);
    assert!(
        initialize_response.get("error").is_none(),
        "initialize returned error: {initialize_response:?}"
    );

    let mut id = 2;
    let capabilities_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            id,
            "resources/read",
            serde_json::json!({"uri": "resource://capabilities"}),
        ),
    );
    id += 1;
    assert!(
        capabilities_response.get("error").is_none(),
        "capabilities resource returned error: {capabilities_response:?}"
    );
    let capabilities_text = capabilities_response["result"]["contents"][0]["text"]
        .as_str()
        .expect("capabilities resource text");
    let mcp_capabilities: Value =
        serde_json::from_str(capabilities_text).expect("MCP capabilities JSON");
    assert_eq!(
        mcp_capabilities["commands"], capabilities["commands"],
        "MCP capabilities should expose the same command inventory as CLI capabilities"
    );
    assert_eq!(
        mcp_capabilities["contractVersion"], capabilities["contractVersion"],
        "MCP capabilities should expose CLI contract version"
    );
    assert_eq!(
        mcp_capabilities["exitCodes"], capabilities["exitCodes"],
        "MCP capabilities should expose CLI exit-code contract"
    );
    assert_eq!(
        mcp_capabilities["resourceTemplates"][0]["uriTemplate"],
        Value::String("resource://command/{path}".to_string())
    );

    for command in commands {
        let path = command["path"].as_str().expect("command path");
        let mut request_paths = vec![path.to_string()];
        if let Some(shorthand) = path.strip_prefix("ooxml ") {
            request_paths.push(shorthand.to_string());
        }

        for request_path in request_paths {
            let uri = command_resource_uri(&request_path);
            let response = serve_roundtrip(
                &mut stdin,
                &mut reader,
                &rpc_request(id, "resources/read", serde_json::json!({"uri": uri})),
            );
            id += 1;
            assert!(
                response.get("error").is_none(),
                "command resource for {request_path:?} returned error: {response:?}"
            );
            let summary = summarize_mcp_command_resource(
                &response["result"],
                response["result"]["contents"][0]["uri"]
                    .as_str()
                    .expect("resource uri"),
            );
            assert_eq!(summary["path"], command["path"], "path for {request_path}");
            assert_eq!(
                summary["opCompatible"], command["opCompatible"],
                "opCompatible for {request_path}"
            );
            assert_eq!(
                summary["flags"],
                local_flag_field(command, "name"),
                "flags for {request_path}"
            );
            assert_eq!(
                summary["argNames"],
                local_flag_field(command, "argName"),
                "argNames for {request_path}"
            );
        }
    }

    let unknown = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            id,
            "resources/read",
            serde_json::json!({"uri": "resource://command/xlsx%20not%20real"}),
        ),
    );
    id += 1;
    assert!(
        unknown.get("error").is_some(),
        "unknown command resource should fail: {unknown:?}"
    );
    let bad_escape = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            id,
            "resources/read",
            serde_json::json!({"uri": "resource://command/xlsx%ZZbad"}),
        ),
    );
    assert!(
        bad_escape.get("error").is_some(),
        "invalid command resource URI should fail: {bad_escape:?}"
    );

    drop(stdin);
    let status = child.wait().expect("mcp exit");
    assert!(status.success());
}

#[test]
fn web_smoke_binary_readback_checks_are_supported() {
    let baseline = baseline();
    let web_smoke = &baseline["webSmoke"];
    let checks = web_smoke["binaryReadbackChecks"]
        .as_array()
        .expect("web smoke readback checks")
        .iter()
        .map(|value| value.as_str().expect("check string"))
        .collect::<Vec<_>>();
    assert!(checks.contains(&"validate --strict"));
    assert!(checks.contains(&"pptx slides show"));
    assert!(checks.contains(&"docx text"));
    assert!(checks.contains(&"xlsx sheets list"));

    let pptx = web_smoke["agentDefaultFixture"]
        .as_str()
        .expect("pptx fixture");
    let docx = web_smoke["docxDefaultFixture"]
        .as_str()
        .expect("docx fixture");
    let xlsx = web_smoke["xlsxDefaultFixture"]
        .as_str()
        .expect("xlsx fixture");

    for file in [pptx, docx, xlsx] {
        let (code, stdout, stderr) = run_ooxml(&["--json", "--strict", "validate", file]);
        assert_eq!(code, 0, "validate exit for {file}");
        assert_eq!(stderr, None, "validate stderr for {file}");
        assert_eq!(stdout.expect("validate stdout")["valid"], Value::Bool(true));
    }

    let (pptx_code, pptx_stdout, pptx_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "slides",
        "show",
        pptx,
        "--slide",
        "1",
        "--include-text",
    ]);
    assert_eq!(pptx_code, 0);
    assert_eq!(pptx_stderr, None);
    assert_eq!(
        pptx_stdout.expect("pptx stdout")["slides"][0]["shapes"][0]["textContent"],
        Value::String("Minimal Title Slide".to_string())
    );
    for fixture in [
        pptx,
        "testdata/pptx/notes-slide/presentation.pptx",
        "testdata/pptx/table-slide/presentation.pptx",
        "testdata/pptx/corrupted-dangling-layout/presentation.pptx",
    ] {
        let pptx_list_args = ["--json", "pptx", "slides", "list", fixture];
        let (go_list_code, go_list_stdout, go_list_stderr) = run_go_ooxml(&pptx_list_args);
        let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&pptx_list_args);
        assert_eq!(
            rust_list_code, go_list_code,
            "pptx slides list exit for {fixture}"
        );
        assert_eq!(
            rust_list_stderr, go_list_stderr,
            "pptx slides list stderr for {fixture}"
        );
        assert_eq!(
            rust_list_stdout, go_list_stdout,
            "pptx slides list stdout for {fixture}"
        );
    }

    let pptx_selectors_args = [
        "--json",
        "pptx",
        "slides",
        "selectors",
        pptx,
        "--slide",
        "1",
    ];
    let (go_selectors_code, go_selectors_stdout, go_selectors_stderr) =
        run_go_ooxml(&pptx_selectors_args);
    let (rust_selectors_code, rust_selectors_stdout, rust_selectors_stderr) =
        run_ooxml(&pptx_selectors_args);
    assert_eq!(
        rust_selectors_code, go_selectors_code,
        "pptx slides selectors exit"
    );
    assert_eq!(
        rust_selectors_stderr, go_selectors_stderr,
        "pptx slides selectors stderr"
    );
    assert_eq!(
        rust_selectors_stdout, go_selectors_stdout,
        "pptx slides selectors stdout"
    );

    for args in [
        [
            "--json",
            "pptx",
            "shapes",
            "show",
            pptx,
            "--slide",
            "1",
            "--include-text",
            "--include-bounds",
        ],
        [
            "--json",
            "pptx",
            "shapes",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--include-text",
            "--include-bounds",
        ],
        [
            "--json",
            "pptx",
            "shapes",
            "show",
            "testdata/pptx/picture-placeholder/presentation.pptx",
            "--slide",
            "2",
            "--include-text",
            "--include-bounds",
        ],
    ] {
        let (go_shapes_code, go_shapes_stdout, go_shapes_stderr) = run_go_ooxml(&args);
        let (rust_shapes_code, rust_shapes_stdout, rust_shapes_stderr) = run_ooxml(&args);
        assert_eq!(rust_shapes_code, go_shapes_code, "pptx shapes show exit");
        assert_eq!(
            rust_shapes_stderr, go_shapes_stderr,
            "pptx shapes show stderr for {args:?}"
        );
        assert_eq!(
            rust_shapes_stdout, go_shapes_stdout,
            "pptx shapes show stdout for {args:?}"
        );
    }

    let table_selectors_args = [
        "--json",
        "pptx",
        "slides",
        "selectors",
        "testdata/pptx/table-slide/presentation.pptx",
        "--slide",
        "2",
    ];
    let (go_table_selectors_code, go_table_selectors_stdout, go_table_selectors_stderr) =
        run_go_ooxml(&table_selectors_args);
    let (rust_table_selectors_code, rust_table_selectors_stdout, rust_table_selectors_stderr) =
        run_ooxml(&table_selectors_args);
    assert_eq!(
        rust_table_selectors_code, go_table_selectors_code,
        "pptx table selectors exit"
    );
    assert_eq!(
        rust_table_selectors_stderr, go_table_selectors_stderr,
        "pptx table selectors stderr"
    );
    assert_eq!(
        rust_table_selectors_stdout, go_table_selectors_stdout,
        "pptx table selectors stdout"
    );

    let (docx_code, docx_stdout, docx_stderr) = run_ooxml(&["--json", "docx", "text", docx]);
    assert_eq!(docx_code, 0);
    assert_eq!(docx_stderr, None);
    assert!(
        docx_stdout.expect("docx stdout")["blocks"]
            .as_array()
            .expect("docx blocks")
            .iter()
            .any(|block| block["text"]
                .as_str()
                .unwrap_or_default()
                .contains("Hello world"))
    );

    let xlsx_args = ["--json", "xlsx", "sheets", "list", xlsx];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&xlsx_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&xlsx_args);
    assert_eq!(rust_code, go_code, "xlsx sheets list exit");
    assert_eq!(rust_stderr, go_stderr, "xlsx sheets list stderr");
    assert_eq!(rust_stdout, go_stdout, "xlsx sheets list stdout");
}

#[test]
fn capabilities_advertise_supported_web_agent_surface() {
    let (all_code, all_stdout, all_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(all_code, 0);
    assert_eq!(all_stderr, None);
    let all_caps = all_stdout.expect("all capabilities");
    assert_command(&all_caps, "ooxml version", false);
    assert_command(&all_caps, "ooxml docx fields list", false);
    assert_command(&all_caps, "ooxml docx headers list", false);
    assert_command(&all_caps, "ooxml docx footers list", false);
    assert_command(&all_caps, "ooxml docx headers show", false);
    assert_command(&all_caps, "ooxml docx footers show", false);
    assert_command(&all_caps, "ooxml docx images list", false);
    assert_command(&all_caps, "ooxml docx tables show", false);
    assert_command(&all_caps, "ooxml docx tables set-cell", false);
    assert_command(&all_caps, "ooxml docx tables clear-cell", false);
    assert_command(&all_caps, "ooxml docx blocks replace", false);
    assert_command(&all_caps, "ooxml docx blocks delete", false);
    assert_command(&all_caps, "ooxml docx blocks insert-after", false);

    let (pptx_code, pptx_stdout, pptx_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "pptx"]);
    assert_eq!(pptx_code, 0);
    assert_eq!(pptx_stderr, None);
    let pptx_caps = pptx_stdout.expect("pptx capabilities");
    assert_eq!(
        pptx_caps["contractVersion"],
        Value::String("ooxml-cli.agent-capabilities.v4".to_string())
    );
    assert_command(&pptx_caps, "ooxml pptx slides list", false);
    assert_command(&pptx_caps, "ooxml pptx slides selectors", false);
    assert_command(&pptx_caps, "ooxml pptx slides show", false);
    assert_command(&pptx_caps, "ooxml pptx shapes show", false);
    assert_command(&pptx_caps, "ooxml pptx replace text", true);

    let (package_code, package_stdout, package_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "package"]);
    assert_eq!(package_code, 0);
    assert_eq!(package_stderr, None);
    let package_caps = package_stdout.expect("package capabilities");
    assert_no_command(&package_caps, "ooxml docx blocks");

    let (xlsx_code, xlsx_stdout, xlsx_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "xlsx"]);
    assert_eq!(xlsx_code, 0);
    assert_eq!(xlsx_stderr, None);
    let xlsx_caps = xlsx_stdout.expect("xlsx capabilities");
    assert_command(&xlsx_caps, "ooxml xlsx sheets list", false);
    assert_command(&xlsx_caps, "ooxml xlsx sheets show", false);
    assert_command(&xlsx_caps, "ooxml xlsx ranges export", false);
    assert_command(&xlsx_caps, "ooxml xlsx ranges set", false);
    assert_command(&xlsx_caps, "ooxml xlsx ranges set-format", false);
    assert_command(&xlsx_caps, "ooxml xlsx cells extract", false);
    assert_command(&xlsx_caps, "ooxml xlsx cells set", true);
    assert_command(&xlsx_caps, "ooxml xlsx tables list", false);
    assert_command(&xlsx_caps, "ooxml xlsx tables show", false);
    assert_command(&xlsx_caps, "ooxml xlsx tables export", false);

    let (table_code, table_stdout, table_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "table"]);
    assert_eq!(table_code, 0);
    assert_eq!(table_stderr, None);
    let table_caps = table_stdout.expect("table capabilities");
    assert_command(&table_caps, "ooxml xlsx tables list", false);
    assert_command(&table_caps, "ooxml xlsx tables show", false);
    assert_command(&table_caps, "ooxml xlsx tables export", false);
    assert_command(&table_caps, "ooxml docx tables set-cell", false);
    assert_command(&table_caps, "ooxml docx tables clear-cell", false);
    assert_command(&table_caps, "ooxml docx blocks delete", false);
    assert_no_command(&table_caps, "ooxml docx blocks");
    assert_no_command(&table_caps, "ooxml docx tables show");

    let (paragraph_code, paragraph_stdout, paragraph_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "paragraph"]);
    assert_eq!(paragraph_code, 0);
    assert_eq!(paragraph_stderr, None);
    let paragraph_caps = paragraph_stdout.expect("paragraph capabilities");
    assert_command(&paragraph_caps, "ooxml docx blocks replace", false);
    assert_command(&paragraph_caps, "ooxml docx blocks delete", false);
    assert_command(&paragraph_caps, "ooxml docx blocks insert-after", false);
    assert_command(&paragraph_caps, "ooxml docx paragraphs append", false);
    assert_command(&paragraph_caps, "ooxml docx paragraphs insert", false);
    assert_no_command(&paragraph_caps, "ooxml docx blocks");

    let (style_code, style_stdout, style_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "style"]);
    assert_eq!(style_code, 0);
    assert_eq!(style_stderr, None);
    let style_caps = style_stdout.expect("style capabilities");
    assert_command(&style_caps, "ooxml xlsx ranges set-format", false);
    assert_command(&style_caps, "ooxml docx styles list", false);
    assert_command(&style_caps, "ooxml docx styles show", false);
    assert_command(&style_caps, "ooxml docx styles apply", false);

    let (comment_code, comment_stdout, comment_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "comment"]);
    assert_eq!(comment_code, 0);
    assert_eq!(comment_stderr, None);
    let comment_caps = comment_stdout.expect("comment capabilities");
    assert_command(&comment_caps, "ooxml docx comments list", false);
    assert_command(&comment_caps, "ooxml docx comments add", false);
    assert_command(&comment_caps, "ooxml docx comments edit", false);
    assert_command(&comment_caps, "ooxml docx comments remove", false);

    let (field_code, field_stdout, field_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "field"]);
    assert_eq!(field_code, 0);
    assert_eq!(field_stderr, None);
    let field_caps = field_stdout.expect("field capabilities");
    assert_command(&field_caps, "ooxml docx fields list", false);

    let (header_code, header_stdout, header_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "header"]);
    assert_eq!(header_code, 0);
    assert_eq!(header_stderr, None);
    let header_caps = header_stdout.expect("header capabilities");
    assert_command(&header_caps, "ooxml docx headers list", false);
    assert_command(&header_caps, "ooxml docx footers list", false);
    assert_command(&header_caps, "ooxml docx headers show", false);

    let (footer_code, footer_stdout, footer_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "footer"]);
    assert_eq!(footer_code, 0);
    assert_eq!(footer_stderr, None);
    let footer_caps = footer_stdout.expect("footer capabilities");
    assert_command(&footer_caps, "ooxml docx headers list", false);
    assert_command(&footer_caps, "ooxml docx footers list", false);
    assert_command(&footer_caps, "ooxml docx footers show", false);

    let (image_code, image_stdout, image_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "image"]);
    assert_eq!(image_code, 0);
    assert_eq!(image_stderr, None);
    let image_caps = image_stdout.expect("image capabilities");
    assert_command(&image_caps, "ooxml docx images list", false);

    let (docx_code, docx_stdout, docx_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "docx"]);
    assert_eq!(docx_code, 0);
    assert_eq!(docx_stderr, None);
    let docx_caps = docx_stdout.expect("docx capabilities");
    assert_command(&docx_caps, "ooxml docx fields list", false);
    assert_command(&docx_caps, "ooxml docx headers list", false);
    assert_command(&docx_caps, "ooxml docx footers list", false);
    assert_command(&docx_caps, "ooxml docx headers show", false);
    assert_command(&docx_caps, "ooxml docx footers show", false);
    assert_command(&docx_caps, "ooxml docx images list", false);
    assert_command(&docx_caps, "ooxml docx tables show", false);
    assert_command(&docx_caps, "ooxml docx tables set-cell", false);
    assert_command(&docx_caps, "ooxml docx tables clear-cell", false);
    assert_command(&docx_caps, "ooxml docx blocks replace", false);
    assert_command(&docx_caps, "ooxml docx blocks delete", false);
    assert_command(&docx_caps, "ooxml docx blocks insert-after", false);
    assert_command(&docx_caps, "ooxml docx paragraphs append", false);
    assert_command(&docx_caps, "ooxml docx paragraphs insert", false);
    assert_command(&docx_caps, "ooxml docx styles apply", false);
    assert_command(&docx_caps, "ooxml docx comments remove", false);
}

#[test]
fn rust_capability_inventory_is_go_oracle_subset() {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "capabilities"]);
    assert_eq!(go_code, 0);
    assert_eq!(go_stderr, None);
    let go_caps = go_stdout.expect("go capabilities");

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    let go_paths = capability_paths(&go_caps);
    let rust_paths = capability_paths(&rust_caps);
    assert_eq!(go_paths.len(), 290, "Go oracle command count changed");
    assert_eq!(rust_paths.len(), 45, "Rust supported command count changed");
    assert_eq!(
        go_paths.len() - rust_paths.len(),
        245,
        "Rust missing-command count changed"
    );
    let invented = rust_paths
        .difference(&go_paths)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        invented.is_empty(),
        "Rust capabilities must be a Go-oracle command subset; invented paths: {invented:?}"
    );
}

fn rpc_request(id: i64, method: &str, params: Value) -> Value {
    serde_json::json!({
        "id": id,
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    })
}

fn serve_roundtrip(stdin: &mut impl Write, reader: &mut impl BufRead, request: &Value) -> Value {
    writeln!(
        stdin,
        "{}",
        serde_json::to_string(request).expect("serialize request")
    )
    .expect("write serve request");
    stdin.flush().expect("flush serve request");
    let mut line = String::new();
    reader.read_line(&mut line).expect("read serve response");
    assert!(!line.trim().is_empty(), "empty serve response");
    serde_json::from_str(&line).expect("decode serve response")
}

fn summarize_mcp_tools(result: &Value) -> Value {
    let mut tools: Vec<Value> = result["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| {
            let schema = &tool["inputSchema"];
            let properties = schema["properties"]
                .as_object()
                .expect("properties object")
                .keys()
                .cloned()
                .map(Value::String)
                .collect::<Vec<_>>();
            serde_json::json!({
                "name": tool["name"],
                "properties": properties,
                "required": schema["required"],
                "additionalProperties": schema["additionalProperties"],
            })
        })
        .collect();
    tools.sort_by(|a, b| a["name"].as_str().unwrap().cmp(b["name"].as_str().unwrap()));
    Value::Array(tools)
}

fn summarize_mcp_command_resource(result: &Value, uri: &str) -> Value {
    let text = result["contents"][0]["text"]
        .as_str()
        .expect("resource text");
    let body: Value = serde_json::from_str(text).expect("command resource JSON");
    let flags = body["localFlags"]
        .as_array()
        .expect("local flags")
        .iter()
        .map(|flag| flag["name"].clone())
        .collect::<Vec<_>>();
    let arg_names = body["localFlags"]
        .as_array()
        .expect("local flags")
        .iter()
        .map(|flag| flag["argName"].clone())
        .collect::<Vec<_>>();
    serde_json::json!({
        "uri": uri,
        "path": body["path"],
        "opCompatible": body["opCompatible"],
        "flags": flags,
        "argNames": arg_names,
    })
}

fn command_resource_uri(path: &str) -> String {
    format!("resource://command/{}", path.replace(' ', "%20"))
}

fn local_flag_field(command: &Value, field: &str) -> Value {
    Value::Array(
        command["localFlags"]
            .as_array()
            .expect("local flags")
            .iter()
            .map(|flag| flag[field].clone())
            .collect(),
    )
}

fn capability_paths(capabilities: &Value) -> BTreeSet<String> {
    capabilities["commands"]
        .as_array()
        .expect("commands array")
        .iter()
        .map(|command| command["path"].as_str().expect("command path").to_string())
        .collect()
}

fn assert_command(capabilities: &Value, path: &str, op_compatible: bool) {
    let commands = capabilities["commands"].as_array().expect("commands array");
    let command = commands
        .iter()
        .find(|command| command["path"].as_str() == Some(path))
        .unwrap_or_else(|| panic!("missing command {path}: {commands:?}"));
    assert_eq!(
        command["opCompatible"],
        Value::Bool(op_compatible),
        "opCompatible for {path}"
    );
}

fn assert_no_command(capabilities: &Value, path: &str) {
    let commands = capabilities["commands"].as_array().expect("commands array");
    assert!(
        !commands
            .iter()
            .any(|command| command["path"].as_str() == Some(path)),
        "unexpected command {path}: {commands:?}"
    );
}

fn sort_by_string_field(value: Value, field: &str) -> Value {
    let mut items = value.as_array().expect("array").clone();
    items.sort_by(|a, b| a[field].as_str().unwrap().cmp(b[field].as_str().unwrap()));
    Value::Array(items)
}

fn flow_item(
    method: &str,
    request: Value,
    response: Value,
    replacements: &[(String, String)],
) -> Value {
    serde_json::json!({
        "method": method,
        "request": scrub_dynamic(request, replacements),
        "response": scrub_dynamic(response, replacements),
    })
}

fn nullable(value: &Value) -> Option<Value> {
    if value.is_null() {
        None
    } else {
        Some(value.clone())
    }
}

fn scrub_path(value: Value, from: &str, to: &str) -> Value {
    scrub_paths(value, &[(from, to)])
}

fn scrub_dynamic(value: Value, replacements: &[(String, String)]) -> Value {
    match value {
        Value::String(text) => {
            let mut text = text;
            for (from, to) in replacements {
                text = text.replace(from, to);
            }
            Value::String(text)
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| scrub_dynamic(item, replacements))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, scrub_dynamic(value, replacements)))
                .collect(),
        ),
        other => other,
    }
}

fn scrub_paths(value: Value, replacements: &[(&str, &str)]) -> Value {
    match value {
        Value::String(text) => {
            let mut text = text;
            for (from, to) in replacements {
                text = text.replace(from, to);
            }
            Value::String(text)
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| scrub_paths(item, replacements))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, scrub_paths(value, replacements)))
                .collect(),
        ),
        other => other,
    }
}
