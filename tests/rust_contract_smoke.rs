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
static GO_ORACLE_SOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

const DEFAULT_GO_ORACLE_REF: &str = "codex/ooxml-go-reference";

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
        .env("GOCACHE", go_cache_dir())
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
        .env("GOCACHE", go_cache_dir())
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

fn go_cache_dir() -> PathBuf {
    let go_cache = std::env::temp_dir().join("ooxml-go-build");
    fs::create_dir_all(&go_cache).expect("create Go oracle cache");
    go_cache
}

fn assert_go_rust_match(args: &[&str]) {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(args);
    assert_eq!(rust_code, go_code, "exit code for {args:?}");
    assert_eq!(rust_stderr, go_stderr, "stderr for {args:?}");
    assert_eq!(rust_stdout, go_stdout, "stdout for {args:?}");
}

fn write_go_oracle_pptx_comment_fixture() -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-comments-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&temp_dir).expect("comment fixture temp dir");
    let out = temp_dir.join("commented.pptx");
    let out_str = out.to_str().expect("comment fixture path");
    let (code, stdout, stderr) = run_go_ooxml(&[
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
        "Fix the title",
        "--date",
        "2026-06-06T10:30:00Z",
        "--out",
        out_str,
    ]);
    assert_eq!(code, 0, "generate PPTX comment fixture exit");
    assert_eq!(stderr, None, "generate PPTX comment fixture stderr");
    assert_eq!(
        stdout.expect("comment add stdout")["output"],
        Value::String(out_str.to_string())
    );
    out
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
        let go_cache = go_cache_dir();
        let output = Command::new("go")
            .args(["build", "-buildvcs=false", "-o"])
            .arg(&binary)
            .arg("./cmd/ooxml")
            .current_dir(go_oracle_source_dir())
            .env("GOCACHE", &go_cache)
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

fn go_oracle_source_dir() -> &'static PathBuf {
    GO_ORACLE_SOURCE_DIR.get_or_init(|| {
        if let Ok(path) = std::env::var("OOXML_GO_ORACLE_DIR")
            && !path.trim().is_empty()
        {
            return PathBuf::from(path);
        }

        let ref_name =
            std::env::var("OOXML_GO_ORACLE_REF").unwrap_or_else(|_| DEFAULT_GO_ORACLE_REF.into());
        let unique_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let source_dir = std::env::temp_dir().join(format!(
            "ooxml-go-oracle-src-{}-{unique_suffix}",
            std::process::id()
        ));
        let output = Command::new("git")
            .args(["worktree", "add", "--detach"])
            .arg(&source_dir)
            .arg(&ref_name)
            .output()
            .expect("create Go oracle worktree");
        assert!(
            output.status.success(),
            "create Go oracle worktree for {ref_name:?} failed\nstdout:\n{}\nstderr:\n{}\nSet OOXML_GO_ORACLE_DIR to a prepared frozen Go reference checkout if needed.",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        source_dir
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

fn write_defined_names_xlsx(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create names xlsx");
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
  <Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet3.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
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
  <bookViews><workbookView activeTab="2" firstSheet="0"/></bookViews>
  <sheets>
    <sheet name="Summary" sheetId="1" r:id="rId1"/>
    <sheet name="Data" sheetId="2" r:id="rId2"/>
    <sheet name="Tail" sheetId="3" r:id="rId3"/>
  </sheets>
  <definedNames>
    <definedName name="GlobalName">Summary!$A$1</definedName>
    <definedName name="LocalSummary" localSheetId="0">Summary!$A$1</definedName>
    <definedName name="LocalData" localSheetId="1">Data!$A$1</definedName>
    <definedName name="LocalTail" localSheetId="2">Tail!$A$1</definedName>
  </definedNames>
</workbook>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet3.xml"/>
</Relationships>"#,
    );
    for sheet_number in 1..=3 {
        write_zip_string(
            &mut writer,
            options,
            &format!("xl/worksheets/sheet{sheet_number}.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
</worksheet>"#,
        );
    }
    writer.finish().expect("finish names xlsx");
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

fn write_calc_chain_xlsx(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create calc-chain xlsx");
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
  <Override PartName="/xl/calcChain.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml"/>
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
  <Relationship Id="rIdCalc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain" Target="calcChain.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B1"/>
  <sheetData>
    <row r="1">
      <c r="A1"><f>SUM(B1:B1)</f><v>7</v></c>
      <c r="B1"><v>7</v></c>
    </row>
  </sheetData>
</worksheet>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/calcChain.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<calcChain xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><c r="A1" i="1"/></calcChain>"#,
    );
    writer.finish().expect("finish calc-chain xlsx");
}

fn assert_xlsx_full_calc_flags(path: &Path) {
    let workbook = read_zip_string(path, "xl/workbook.xml");
    assert!(
        workbook.contains(r#"fullCalcOnLoad="1""#),
        "workbook missing fullCalcOnLoad flag: {workbook}"
    );
    assert!(
        workbook.contains(r#"forceFullCalc="1""#),
        "workbook missing forceFullCalc flag: {workbook}"
    );
}

fn assert_xlsx_calc_chain_removed(path: &Path) {
    assert!(
        !zip_entry_exists(path, "xl/calcChain.xml"),
        "calcChain part still exists"
    );
    let content_types = read_zip_string(path, "[Content_Types].xml");
    assert!(
        !content_types.contains("calcChain"),
        "content types still mention calcChain: {content_types}"
    );
    let rels = read_zip_string(path, "xl/_rels/workbook.xml.rels");
    assert!(
        !rels.contains("calcChain") && !rels.contains("rIdCalc"),
        "workbook rels still mention calcChain: {rels}"
    );
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

#[path = "rust_contract_smoke/xlsx.rs"]
mod xlsx;

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
fn validate_rejects_corrupted_docx_and_xlsx_like_go_oracle() {
    for args in [
        vec![
            "--json",
            "--strict",
            "validate",
            "testdata/docx/corrupted-missing-document/document.docx",
        ],
        vec![
            "--json",
            "--strict",
            "validate",
            "testdata/xlsx/corrupted-missing-worksheet/workbook.xlsx",
        ],
    ] {
        assert_go_rust_match(&args);
    }
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
fn docx_fields_insert_and_set_result_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-fields-edit-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx fields edit temp dir");

    let go_insert_out = temp_dir.join("go-insert.docx");
    let rust_insert_out = temp_dir.join("rust-insert.docx");
    let go_insert_out = go_insert_out.to_string_lossy().to_string();
    let rust_insert_out = rust_insert_out.to_string_lossy().to_string();
    let insert_input = "testdata/docx/minimal/document.docx";
    let go_insert_args = [
        "--json",
        "docx",
        "fields",
        "insert",
        insert_input,
        "--location",
        "body:1",
        "--field-code",
        "PAGE",
        "--result",
        "1",
        "--out",
        &go_insert_out,
    ];
    let rust_insert_args = [
        "--json",
        "docx",
        "fields",
        "insert",
        insert_input,
        "--location",
        "body:1",
        "--field-code",
        "PAGE",
        "--result",
        "1",
        "--out",
        &rust_insert_out,
    ];
    let (go_insert_code, go_insert_stdout, go_insert_stderr) = run_go_ooxml(&go_insert_args);
    let (rust_insert_code, rust_insert_stdout, rust_insert_stderr) = run_ooxml(&rust_insert_args);
    assert_eq!(rust_insert_code, go_insert_code, "fields insert exit");
    assert_eq!(rust_insert_stderr, go_insert_stderr, "fields insert stderr");
    assert_eq!(
        scrub_path(
            rust_insert_stdout.expect("rust fields insert stdout"),
            &rust_insert_out,
            "[OUT]"
        ),
        scrub_path(
            go_insert_stdout.expect("go fields insert stdout"),
            &go_insert_out,
            "[OUT]"
        ),
        "fields insert stdout"
    );
    let (validate_code, _, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_insert_out]);
    assert_eq!(validate_code, 0, "inserted docx validates");
    assert_eq!(validate_stderr, None, "inserted docx validation stderr");
    let (go_list_code, go_list_stdout, go_list_stderr) =
        run_go_ooxml(&["--json", "docx", "fields", "list", &go_insert_out]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) =
        run_ooxml(&["--json", "docx", "fields", "list", &rust_insert_out]);
    assert_eq!(rust_list_code, go_list_code, "insert readback list exit");
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "insert readback list stderr"
    );
    assert_eq!(
        scrub_path(
            rust_list_stdout.expect("rust insert readback"),
            &rust_insert_out,
            "[OUT]"
        ),
        scrub_path(
            go_list_stdout.expect("go insert readback"),
            &go_insert_out,
            "[OUT]"
        ),
        "insert readback list stdout"
    );

    assert_go_rust_match(&[
        "--json",
        "docx",
        "fields",
        "insert",
        insert_input,
        "--location",
        "body:1",
        "--field-code",
        "STYLEREF",
        "--dry-run",
    ]);

    let set_input = "testdata/docx/with-fields/document.docx";
    let go_set_out = temp_dir.join("go-set.docx");
    let rust_set_out = temp_dir.join("rust-set.docx");
    let go_set_out = go_set_out.to_string_lossy().to_string();
    let rust_set_out = rust_set_out.to_string_lossy().to_string();
    let go_set_args = [
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "body:1:0",
        "--result",
        "42",
        "--out",
        &go_set_out,
    ];
    let rust_set_args = [
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "body:1:0",
        "--result",
        "42",
        "--out",
        &rust_set_out,
    ];
    let (go_set_code, go_set_stdout, go_set_stderr) = run_go_ooxml(&go_set_args);
    let (rust_set_code, rust_set_stdout, rust_set_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_set_code, go_set_code, "fields set-result exit");
    assert_eq!(rust_set_stderr, go_set_stderr, "fields set-result stderr");
    assert_eq!(
        scrub_path(
            rust_set_stdout.expect("rust fields set stdout"),
            &rust_set_out,
            "[OUT]"
        ),
        scrub_path(
            go_set_stdout.expect("go fields set stdout"),
            &go_set_out,
            "[OUT]"
        ),
        "fields set-result stdout"
    );
    let (validate_code, _, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_set_out]);
    assert_eq!(validate_code, 0, "set-result docx validates");
    assert_eq!(validate_stderr, None, "set-result validation stderr");

    let go_header_out = temp_dir.join("go-header.docx");
    let rust_header_out = temp_dir.join("rust-header.docx");
    let go_header_out = go_header_out.to_string_lossy().to_string();
    let rust_header_out = rust_header_out.to_string_lossy().to_string();
    let go_header_args = [
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "header1:1:0",
        "--result",
        "9",
        "--out",
        &go_header_out,
    ];
    let rust_header_args = [
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "header1:1:0",
        "--result",
        "9",
        "--out",
        &rust_header_out,
    ];
    let (go_header_code, go_header_stdout, go_header_stderr) = run_go_ooxml(&go_header_args);
    let (rust_header_code, rust_header_stdout, rust_header_stderr) = run_ooxml(&rust_header_args);
    assert_eq!(rust_header_code, go_header_code, "header field set exit");
    assert_eq!(
        rust_header_stderr, go_header_stderr,
        "header field set stderr"
    );
    assert_eq!(
        scrub_path(
            rust_header_stdout.expect("rust header field set stdout"),
            &rust_header_out,
            "[OUT]"
        ),
        scrub_path(
            go_header_stdout.expect("go header field set stdout"),
            &go_header_out,
            "[OUT]"
        ),
        "header field set stdout"
    );

    assert_go_rust_match(&[
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "body:1",
        "--result",
        "42",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "body:1:0",
        "--result",
        "42",
        "--expect-hash",
        "sha256:bogus",
        "--dry-run",
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
    assert_go_rust_match(&[
        "--json",
        "docx",
        "fields",
        "set-result",
        &table_docx,
        "--selector",
        "body:1:0",
        "--result",
        "2",
        "--dry-run",
    ]);

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
fn docx_headers_and_footers_set_text_match_go_oracle() {
    let dry_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "headers",
            "set-text",
            "testdata/docx/headers/document.docx",
            "--selector",
            "header:1:default/p:1",
            "--text",
            "Selector Header",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "set-text",
            "testdata/docx/headers/document.docx",
            "--selector",
            "footer:1:default",
            "--index",
            "1",
            "--text",
            "Selector Footer",
            "--dry-run",
        ],
    ];
    for args in dry_cases {
        assert_go_rust_match(&args);
    }

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-header-footer-set-text-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_out = temp_dir.join("go create header.docx");
    let rust_out = temp_dir.join("rust create header.docx");
    let go_out_str = go_out.to_string_lossy().to_string();
    let rust_out_str = rust_out.to_string_lossy().to_string();
    let go_args = [
        "--json",
        "docx",
        "headers",
        "set-text",
        "testdata/docx/minimal/document.docx",
        "--type",
        "default",
        "--index",
        "1",
        "--text",
        "Brand New Header",
        "--out",
        &go_out_str,
    ];
    let rust_args = [
        "--json",
        "docx",
        "headers",
        "set-text",
        "testdata/docx/minimal/document.docx",
        "--type",
        "default",
        "--index",
        "1",
        "--text",
        "Brand New Header",
        "--out",
        &rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "create header exit");
    assert_eq!(rust_stderr, go_stderr, "create header stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust create header stdout"),
            &rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go create header stdout"),
            &go_out_str,
            "[OUT]"
        ),
        "create header stdout"
    );
    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["validate", "--strict", &rust_out_str]);
    assert_eq!(validate_code, 0, "created header validates");
    assert_eq!(validate_stderr, None, "created header validate stderr");
    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "headers",
        "show",
        &rust_out_str,
        "--selector",
        "header:1:default",
    ]);
    assert_eq!(show_code, 0, "created header show exit");
    assert_eq!(show_stderr, None, "created header show stderr");
    assert_eq!(
        show_stdout.expect("created header show")["paragraphs"][0]["text"],
        Value::String("Brand New Header".to_string())
    );

    let go_footer_out = temp_dir.join("go add footer ref.docx");
    let rust_footer_out = temp_dir.join("rust add footer ref.docx");
    let go_footer_out_str = go_footer_out.to_string_lossy().to_string();
    let rust_footer_out_str = rust_footer_out.to_string_lossy().to_string();
    let go_args = [
        "--json",
        "docx",
        "footers",
        "set-text",
        "testdata/docx/with-media/document.docx",
        "--type",
        "default",
        "--index",
        "1",
        "--text",
        "Footer Wired",
        "--out",
        &go_footer_out_str,
    ];
    let rust_args = [
        "--json",
        "docx",
        "footers",
        "set-text",
        "testdata/docx/with-media/document.docx",
        "--type",
        "default",
        "--index",
        "1",
        "--text",
        "Footer Wired",
        "--out",
        &rust_footer_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "add footer ref exit");
    assert_eq!(rust_stderr, go_stderr, "add footer ref stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust add footer stdout"),
            &rust_footer_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go add footer stdout"),
            &go_footer_out_str,
            "[OUT]"
        ),
        "add footer ref stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
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

#[path = "rust_contract_smoke/pptx.rs"]
mod pptx;

#[path = "rust_contract_smoke/serve.rs"]
mod serve;

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
        vec!["--json", "pptx", "extract", "text", pptx],
        vec![
            "--json",
            "pptx",
            "extract",
            "text",
            "testdata/pptx/title-content/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "text",
            "testdata/pptx/title-content/presentation.pptx",
            "--slide",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "text",
            "testdata/pptx/title-content/presentation.pptx",
            "--slide",
            "3",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "text",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ] {
        assert_go_rust_match(&args);
    }

    let commented_pptx_path = write_go_oracle_pptx_comment_fixture();
    let commented_pptx = commented_pptx_path
        .to_str()
        .expect("commented PPTX fixture path");
    for args in [
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            "testdata/pptx/title-content/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            "testdata/pptx/title-content/presentation.pptx",
            "--slide",
            "1",
        ],
        vec!["--json", "pptx", "comments", "list", commented_pptx],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            commented_pptx,
            "--slide",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            commented_pptx,
            "--slide",
            "1",
            "--comment-id",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            commented_pptx,
            "--slide",
            "1",
            "--comment-id",
            "999",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            "testdata/pptx/title-content/presentation.pptx",
            "--slide",
            "999",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            "testdata/pptx/title-content/presentation.pptx",
            "--comment-id",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ] {
        assert_go_rust_match(&args);
    }

    for args in [
        vec![
            "--json",
            "pptx",
            "extract",
            "notes",
            "testdata/pptx/notes-slide/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "notes",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--slide",
            "2",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "notes",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--slide",
            "99",
        ],
        vec![
            "--json",
            "pptx",
            "extract",
            "notes",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "pptx",
            "notes",
            "show",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--slide",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "notes",
            "show",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--slide",
            "2",
        ],
        vec!["--json", "pptx", "notes", "show", pptx, "--slide", "1"],
        vec![
            "--json",
            "pptx",
            "notes",
            "show",
            "testdata/pptx/notes-slide/presentation.pptx",
            "--slide",
            "99",
        ],
        vec![
            "--json",
            "pptx",
            "notes",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--slide",
            "1",
        ],
    ] {
        assert_go_rust_match(&args);
    }

    for args in [
        vec![
            "--json",
            "pptx",
            "masters",
            "list",
            "testdata/pptx/minimal-title/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "list",
            "testdata/pptx/multi-layout/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "show",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--master",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "show",
            "testdata/pptx/multi-layout/presentation.pptx",
            "--master",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "show",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--master",
            "999",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--master",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "list",
            "testdata/pptx/minimal-title/presentation.pptx",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "list",
            "testdata/pptx/title-content/presentation.pptx",
            "--master",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "show",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--layout",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "show",
            "testdata/pptx/title-content/presentation.pptx",
            "--layout",
            "Title and Content",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "show",
            "testdata/pptx/title-content/presentation.pptx",
            "--layout",
            "NOPE",
        ],
        vec![
            "--json",
            "pptx",
            "layouts",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--layout",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--target",
            "table:1",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--target",
            "@all-tables",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--details",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "1",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "99",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--target",
            "title",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/pptx/table-slide/presentation.pptx",
            "--slide",
            "2",
            "--table-id",
            "999",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--slide",
            "1",
        ],
    ] {
        assert_go_rust_match(&args);
    }

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

#[path = "rust_contract_smoke/capabilities.rs"]
mod capabilities;

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

fn assert_object_kind(capabilities: &Value, kind: &str) {
    let kinds = capabilities["objectKinds"]
        .as_array()
        .expect("objectKinds array");
    assert!(
        kinds.iter().any(|value| value.as_str() == Some(kind)),
        "missing object kind {kind}: {kinds:?}"
    );
    assert!(
        capabilities["objectKindsIndex"].get(kind).is_some(),
        "missing objectKindsIndex entry for {kind}"
    );
}

fn assert_object_kind_command(capabilities: &Value, kind: &str, path: &str) {
    let commands = capabilities["objectKindsIndex"][kind]
        .as_array()
        .unwrap_or_else(|| panic!("objectKindsIndex entry for {kind} is not an array"));
    assert!(
        commands.iter().any(|value| value.as_str() == Some(path)),
        "missing {path} in objectKindsIndex[{kind}]: {commands:?}"
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

fn assert_rust_emitted_ooxml_command_succeeds(result: &Value, field: &str) {
    let command = result[field]
        .as_str()
        .unwrap_or_else(|| panic!("{field} command string"));
    let args = emitted_ooxml_args(command);
    let borrowed = args.iter().map(String::as_str).collect::<Vec<_>>();
    let (code, stdout, stderr) = run_ooxml(&borrowed);
    assert_eq!(code, 0, "emitted {field} exit for {command}");
    assert_eq!(stderr, None, "emitted {field} stderr for {command}");
    assert!(stdout.is_some(), "emitted {field} stdout for {command}");
}

fn assert_rust_emitted_ooxml_command_exits_zero(result: &Value, field: &str) {
    let command = result[field]
        .as_str()
        .unwrap_or_else(|| panic!("{field} command string"));
    let args = emitted_ooxml_args(command);
    let borrowed = args.iter().map(String::as_str).collect::<Vec<_>>();
    let (code, _, stderr) = run_ooxml(&borrowed);
    assert_eq!(code, 0, "emitted {field} exit for {command}");
    assert_eq!(stderr, None, "emitted {field} stderr for {command}");
}

fn command_arg_for_test(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let needs_quotes = value.chars().any(|ch| {
        matches!(
            ch,
            ' ' | '\t'
                | '\r'
                | '\n'
                | '\''
                | '"'
                | '\\'
                | '$'
                | '`'
                | '<'
                | '>'
                | '|'
                | '&'
                | ';'
                | '('
                | ')'
        )
    });
    if !needs_quotes {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn emitted_ooxml_args(command: &str) -> Vec<String> {
    let command = command
        .strip_prefix("ooxml ")
        .unwrap_or_else(|| panic!("emitted command must start with ooxml: {command}"));
    shell_words(command).unwrap_or_else(|err| panic!("parse emitted command {command:?}: {err}"))
}

fn shell_words(command: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut started = false;
    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                current.push(ch);
            }
            continue;
        }
        if in_double {
            match ch {
                '"' => in_double = false,
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                _ => current.push(ch),
            }
            continue;
        }
        match ch {
            '\'' => {
                in_single = true;
                started = true;
            }
            '"' => {
                in_double = true;
                started = true;
            }
            '\\' => {
                started = true;
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ch if ch.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            _ => {
                started = true;
                current.push(ch);
            }
        }
    }
    if in_single || in_double {
        return Err("unterminated quote".to_string());
    }
    if started {
        words.push(current);
    }
    Ok(words)
}

fn scrub_dynamic(value: Value, replacements: &[(String, String)]) -> Value {
    match value {
        Value::String(text) => {
            let replacements = scrub_replacement_variants(
                replacements
                    .iter()
                    .map(|(from, to)| (from.as_str(), to.as_str())),
            );
            Value::String(scrub_string(text, replacements))
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
        Value::String(text) => Value::String(scrub_string(
            text,
            scrub_replacement_variants(replacements.iter().copied()),
        )),
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

fn scrub_replacement_variants<'a, I>(replacements: I) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut variants = Vec::new();
    for (from, to) in replacements {
        if from.is_empty() {
            continue;
        }
        let mut from_variants = vec![from.to_string()];
        let slash_normalized = from.replace('\\', "/");
        if slash_normalized != from {
            from_variants.push(slash_normalized);
        }
        let json_escaped = from.replace('\\', r"\\");
        if json_escaped != from {
            from_variants.push(json_escaped);
        }
        for variant in from_variants {
            variants.push((variant.clone(), to.to_string()));
            variants.push((format!("'{variant}'"), to.to_string()));
        }
    }
    variants.sort_by_key(|variant| std::cmp::Reverse(variant.0.len()));
    variants
}

fn scrub_string(mut text: String, replacements: Vec<(String, String)>) -> String {
    let mut placeholders = BTreeSet::new();
    for (from, to) in replacements {
        text = text.replace(&from, &to);
        if is_path_placeholder(&to) {
            placeholders.insert(to);
        }
    }
    for placeholder in placeholders {
        text = text.replace(&format!("{placeholder}\\"), &format!("{placeholder}/"));
    }
    text
}

fn is_path_placeholder(value: &str) -> bool {
    value.starts_with('[')
        && value.ends_with(']')
        && [
            "FILE", "XLSX", "XLSM", "PPTX", "PPTM", "DOCX", "DOCM", "PACKAGE", "DIR", "OUT",
        ]
        .iter()
        .any(|needle| value.contains(needle))
}
