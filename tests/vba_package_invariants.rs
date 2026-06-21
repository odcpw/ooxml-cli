use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const VBA_REL_TYPE: &str = "http://schemas.microsoft.com/office/2006/relationships/vbaProject";
const VBA_CONTENT_TYPE: &str = "application/vnd.ms-office.vbaProject";
const XLSX_MAIN_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const XLSM_MAIN_CONTENT_TYPE: &str = "application/vnd.ms-excel.sheet.macroEnabled.main+xml";

fn run_ooxml(args: &[&str]) -> (i32, Option<Value>, Option<Value>) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .output()
        .expect("run Rust ooxml");
    (
        output.status.code().unwrap_or(-1),
        parse_json(&output.stdout),
        parse_json(&output.stderr),
    )
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

fn temp_dir(name: &str) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("ooxml-{name}-{}-{suffix}", std::process::id()))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[test]
fn validate_and_conformance_reject_broken_xlsm_vba_wiring() {
    let temp_dir = temp_dir("vba-package-invariants");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let valid = build_valid_xlsm(&temp_dir);

    let missing_rel = temp_dir.join("missing-vba-rel.xlsm");
    rewrite_zip(&valid, &missing_rel, |name, data| {
        if name == "xl/_rels/workbook.xml.rels" {
            let xml = String::from_utf8(data).expect("rels utf8");
            return Some((
                name.to_string(),
                remove_relationships_by_type(&xml, VBA_REL_TYPE).into_bytes(),
            ));
        }
        Some((name.to_string(), data))
    });
    assert_package_rejected(
        &missing_rel,
        &[
            "VBA_PROJECT_RELATIONSHIP_MISSING",
            "VBA_PROJECT_ORPHAN_PART",
        ],
        &["xl/_rels/workbook.xml.rels", "vbaProject relationship"],
    );

    let missing_content_type = temp_dir.join("missing-vba-content-type.xlsm");
    rewrite_zip(&valid, &missing_content_type, |name, data| {
        if name == "[Content_Types].xml" {
            let xml = String::from_utf8(data).expect("content types utf8");
            return Some((
                name.to_string(),
                remove_overrides_by_part_name(&xml, "/xl/vbaProject.bin").into_bytes(),
            ));
        }
        Some((name.to_string(), data))
    });
    assert_package_rejected(
        &missing_content_type,
        &["VBA_PROJECT_CONTENT_TYPE_INVALID"],
        &["xl/vbaProject.bin", VBA_CONTENT_TYPE],
    );

    let wrong_content_type = temp_dir.join("wrong-vba-content-type.xlsm");
    rewrite_zip(&valid, &wrong_content_type, |name, data| {
        if name == "[Content_Types].xml" {
            return Some((
                name.to_string(),
                replace_once(data, VBA_CONTENT_TYPE, "application/octet-stream"),
            ));
        }
        Some((name.to_string(), data))
    });
    assert_package_rejected(
        &wrong_content_type,
        &["VBA_PROJECT_CONTENT_TYPE_INVALID"],
        &["xl/vbaProject.bin", VBA_CONTENT_TYPE],
    );

    let orphan_non_macro = temp_dir.join("orphan-vba-project.xlsx");
    rewrite_zip(&valid, &orphan_non_macro, |name, data| {
        if name == "[Content_Types].xml" {
            let data = replace_once(data, XLSM_MAIN_CONTENT_TYPE, XLSX_MAIN_CONTENT_TYPE);
            return Some((name.to_string(), data));
        }
        if name == "xl/_rels/workbook.xml.rels" {
            let xml = String::from_utf8(data).expect("rels utf8");
            return Some((
                name.to_string(),
                remove_relationships_by_type(&xml, VBA_REL_TYPE).into_bytes(),
            ));
        }
        Some((name.to_string(), data))
    });
    assert_package_rejected(
        &orphan_non_macro,
        &[
            "VBA_PROJECT_RELATIONSHIP_MISSING",
            "VBA_MAIN_PART_NOT_MACRO_ENABLED",
            "VBA_PROJECT_ORPHAN_PART",
        ],
        &["xl/vbaProject.bin", "macroEnabled"],
    );

    let duplicate_rel = temp_dir.join("duplicate-vba-rel.xlsm");
    rewrite_zip(&valid, &duplicate_rel, |name, data| {
        if name == "xl/_rels/workbook.xml.rels" {
            let xml = String::from_utf8(data).expect("rels utf8");
            let duplicate = format!(
                r#"<Relationship Id="rIdVbaDuplicate" Type="{VBA_REL_TYPE}" Target="vbaProject.bin"/>"#
            );
            return Some((
                name.to_string(),
                insert_before(&xml, "</Relationships>", &duplicate).into_bytes(),
            ));
        }
        Some((name.to_string(), data))
    });
    assert_package_rejected(
        &duplicate_rel,
        &["VBA_PROJECT_RELATIONSHIP_DUPLICATE"],
        &["vbaProject relationships"],
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

fn build_valid_xlsm(temp_dir: &Path) -> PathBuf {
    let source = temp_dir.join("Hello.bas");
    fs::write(
        &source,
        "Attribute VB_Name = \"Hello\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n",
    )
    .expect("write VBA source");
    let bin = temp_dir.join("vbaProject.bin");
    let xlsm = temp_dir.join("valid.xlsm");
    let source = path_string(&source);
    let bin = path_string(&bin);
    let xlsm_arg = path_string(&xlsm);

    assert_ok(
        "build vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "build-bin",
            "--family",
            "xlsx",
            "--source",
            &source,
            "--out",
            &bin,
        ]),
    );
    assert_ok(
        "attach vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "attach",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--bin",
            &bin,
            "--out",
            &xlsm_arg,
        ]),
    );
    assert_ok(
        "validate valid xlsm",
        run_ooxml(&["--json", "validate", "--strict", &xlsm_arg]),
    );
    assert_ok(
        "conformance valid xlsm",
        run_ooxml(&["--json", "conformance", "check", &xlsm_arg]),
    );
    xlsm
}

fn assert_ok(label: &str, outcome: (i32, Option<Value>, Option<Value>)) -> Value {
    let (code, stdout, stderr) = outcome;
    assert_eq!(code, 0, "{label} exit, stderr: {stderr:?}");
    assert_eq!(stderr, None, "{label} stderr");
    stdout.unwrap_or_else(|| panic!("{label} stdout"))
}

fn assert_package_rejected(
    path: &Path,
    expected_codes: &[&str],
    expected_message_fragments: &[&str],
) {
    let file = path_string(path);
    assert_command_rejected(
        &format!("validate {}", path.display()),
        &["--json", "validate", "--strict", &file],
        expected_codes,
        expected_message_fragments,
    );
    assert_command_rejected(
        &format!("conformance {}", path.display()),
        &["--json", "conformance", "check", &file],
        expected_codes,
        expected_message_fragments,
    );
}

fn assert_command_rejected(
    label: &str,
    args: &[&str],
    expected_codes: &[&str],
    expected_message_fragments: &[&str],
) {
    let (code, stdout, stderr) = run_ooxml(args);
    assert_ne!(code, 0, "{label} should fail");
    let report = stdout
        .or(stderr)
        .unwrap_or_else(|| panic!("{label} should emit JSON"));
    let mut codes = Vec::new();
    collect_codes(&report, &mut codes);
    for expected in expected_codes {
        assert!(
            codes.iter().any(|code| code == expected),
            "{label} missing diagnostic code {expected}; got {codes:?}\nreport: {report}"
        );
    }
    let body = serde_json::to_string(&report).expect("serialize diagnostic report");
    for expected in expected_message_fragments {
        assert!(
            body.contains(expected),
            "{label} missing diagnostic message fragment {expected:?}\nreport: {report}"
        );
    }
}

fn collect_codes(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(code) = map.get("code").and_then(Value::as_str) {
                out.push(code.to_string());
            }
            for item in map.values() {
                collect_codes(item, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_codes(item, out);
            }
        }
        _ => {}
    }
}

fn rewrite_zip<F>(source: &Path, dest: &Path, mut mutator: F)
where
    F: FnMut(&str, Vec<u8>) -> Option<(String, Vec<u8>)>,
{
    let input = File::open(source).expect("open source package");
    let mut archive = ZipArchive::new(input).expect("read source package zip");
    let output = File::create(dest).expect("create rewritten package");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).expect("read source entry");
        if entry.is_dir() {
            writer
                .add_directory(entry.name(), options)
                .expect("copy directory");
            continue;
        }
        let source_name = entry.name().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).expect("read source data");
        if let Some((dest_name, data)) = mutator(&source_name, data) {
            writer.start_file(dest_name, options).expect("write entry");
            writer.write_all(&data).expect("write data");
        }
    }
    writer.finish().expect("finish rewritten package");
}

fn remove_relationships_by_type(xml: &str, rel_type: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<Relationship ") {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find("/>") else {
            out.push_str(tail);
            return out;
        };
        let element = &tail[..end + 2];
        if !element.contains(rel_type) {
            out.push_str(element);
        }
        rest = &tail[end + 2..];
    }
    out.push_str(rest);
    out
}

fn remove_overrides_by_part_name(xml: &str, part_name: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<Override ") {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find("/>") else {
            out.push_str(tail);
            return out;
        };
        let element = &tail[..end + 2];
        if !element.contains(part_name) {
            out.push_str(element);
        }
        rest = &tail[end + 2..];
    }
    out.push_str(rest);
    assert_ne!(out, xml, "fixture should contain Override for {part_name}");
    out
}

fn replace_once(data: Vec<u8>, from: &str, to: &str) -> Vec<u8> {
    let text = String::from_utf8(data).expect("zip text entry utf8");
    assert!(text.contains(from), "fixture text should contain {from:?}");
    text.replacen(from, to, 1).into_bytes()
}

fn insert_before(text: &str, needle: &str, insertion: &str) -> String {
    let Some(index) = text.find(needle) else {
        panic!("fixture text should contain {needle:?}");
    };
    let mut out = String::with_capacity(text.len() + insertion.len());
    out.push_str(&text[..index]);
    out.push_str(insertion);
    out.push_str(&text[index..]);
    out
}
