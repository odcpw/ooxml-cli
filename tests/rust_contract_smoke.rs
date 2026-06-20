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

#[path = "rust_contract_smoke/docx.rs"]
mod docx;

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
