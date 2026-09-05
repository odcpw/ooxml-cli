//! Real serializer exports, without Python, LibreOffice, network, or credentials in CI.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CASES: &[(&str, &str, bool)] = &[
    ("python-pptx/review.pptx", "pptx", true),
    ("python-docx/report.docx", "docx", true),
    ("xlsxwriter/sales.xlsx", "xlsx", true),
    ("libreoffice/review.pptx", "pptx", true),
    ("libreoffice/report.docx", "docx", false),
    ("libreoffice/sales.xlsx", "xlsx", false),
];

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Redirect to files so a verbose child cannot fill a pipe while the parent polls.
// Kill only the child started here if it exceeds its per-command budget.
fn run(scratch: &Path, args: &[&str]) -> (i32, Value) {
    let stdout = scratch.join("stdout.json");
    let stderr = scratch.join("stderr.txt");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("--json")
        .args(args)
        .stdout(Stdio::from(fs::File::create(&stdout).unwrap()))
        .stderr(Stdio::from(fs::File::create(&stderr).unwrap()))
        .spawn()
        .unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("corpus command exceeded 30 seconds: {args:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let diagnostic = fs::read_to_string(stderr).unwrap();
    assert!(diagnostic.is_empty(), "{args:?}: {diagnostic}");
    let bytes = fs::read(stdout).unwrap();
    let report = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("{args:?}: {error}: {}", String::from_utf8_lossy(&bytes)));
    (status.code().expect("child terminated by signal"), report)
}

fn success(scratch: &Path, args: &[&str]) -> Value {
    let (code, report) = run(scratch, args);
    assert_eq!(code, 0, "{args:?}: {report}");
    report
}

fn schema(scratch: &Path, path: &str, expected_clean: bool) -> bool {
    let (code, report) = run(scratch, &["conformance", "check", path, "--openxml-sdk"]);
    let schema = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "schema")
        .unwrap();
    if schema["status"] == "skipped" {
        assert_eq!(code, 0, "{report}");
        assert!(
            !std::env::var("OOXML_REQUIRE_OPENXML_SDK")
                .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true")),
            "required schema proof skipped: {report}"
        );
        eprintln!("SKIP corpus SDK proof: {path}");
        return false;
    }
    assert_eq!(schema["schemaCheck"]["checked"], true, "{report}");
    assert_eq!(
        schema["schemaCheck"]["valid"], expected_clean,
        "{path}: {report}"
    );
    assert_eq!(code, if expected_clean { 0 } else { 5 }, "{report}");
    true
}

#[test]
fn independent_producer_corpus_reads_mutates_and_preserves_schema() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/corpus");
    let scratch =
        Scratch(std::env::temp_dir().join(format!("ooxml-corpus-{}", std::process::id())));
    fs::create_dir_all(&scratch.0).unwrap();
    let listed: BTreeSet<_> = CASES.iter().map(|(path, _, _)| path.to_string()).collect();
    let actual: BTreeSet<_> = fs::read_dir(&root)
        .unwrap()
        .flat_map(|entry| {
            let entry = entry.unwrap();
            if !entry.path().is_dir() {
                return Vec::new();
            }
            fs::read_dir(entry.path())
                .unwrap()
                .map(|file| {
                    format!(
                        "{}/{}",
                        entry.file_name().to_str().unwrap(),
                        file.unwrap().file_name().to_str().unwrap()
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        listed, actual,
        "every corpus package must run through the harness"
    );
    let hashes = fs::read_to_string(root.join("SHA256SUMS")).unwrap();
    for line in hashes.lines() {
        let (expected, relative) = line.split_once("  ").unwrap();
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(fs::read(root.join(relative)).unwrap())
            ),
            expected
        );
    }
    assert_eq!(hashes.lines().count(), CASES.len());
    let started = Instant::now();
    let mut sdk_checked = 0;
    for (relative, family, sdk_clean) in CASES {
        let input = root.join(relative);
        let original = fs::read(&input).unwrap();
        let input = input.to_str().unwrap();
        let output = scratch
            .0
            .join(format!("{}.{}", relative.replace('/', "-"), family));
        let output = output.to_str().unwrap();
        let outline = success(&scratch.0, &["outline", input]);
        assert_eq!(outline["type"], *family, "{outline}");
        success(&scratch.0, &["validate", "--strict", input]);
        let (check_code, check) = run(
            &scratch.0,
            &[
                "check",
                input,
                "--openxml-sdk",
                "skip",
                "--fail-on",
                "error",
            ],
        );
        let errors = check["summary"]["errors"].as_u64().unwrap();
        assert_eq!(check_code, if errors > 0 { 5 } else { 0 }, "{check}");
        assert_eq!(check["checks"]["strict"], "passed");
        let design = success(&scratch.0, &["design-check", input]);
        if *relative == "libreoffice/review.pptx" {
            let contrast = design["findings"]
                .as_array()
                .unwrap()
                .iter()
                .find(|finding| finding["code"] == "PPTX_TEXT_CONTRAST")
                .unwrap();
            // White on the actual blue cell is ~4:1; white on the slide was 1:1.
            assert_eq!(contrast["evidence"]["background"], "4F81BD");
            assert!(contrast["evidence"]["contrastRatio"].as_f64().unwrap() > 3.0);
        }
        let mutation = match *family {
            "pptx" => success(
                &scratch.0,
                &[
                    "pptx",
                    "replace",
                    "text",
                    input,
                    "--slide",
                    "1",
                    "--target",
                    "title",
                    "--text",
                    "Corpus revised review",
                    "--out",
                    output,
                ],
            ),
            "docx" => success(
                &scratch.0,
                &[
                    "docx",
                    "paragraphs",
                    "append",
                    input,
                    "--text",
                    "Corpus appended paragraph",
                    "--out",
                    output,
                ],
            ),
            "xlsx" => success(
                &scratch.0,
                &[
                    "xlsx", "cells", "set", input, "--sheet", "Sales", "--cell", "B2", "--value",
                    "150", "--out", output,
                ],
            ),
            _ => unreachable!(),
        };
        assert_eq!(mutation["mutationEnvelope"]["file"], output, "{mutation}");
        assert_eq!(
            fs::read(input).unwrap(),
            original,
            "mutation changed input {relative}"
        );
        assert_ne!(
            fs::read(output).unwrap(),
            original,
            "mutation was a no-op {relative}"
        );
        let changed_part = match *family {
            "pptx" => "ppt/slides/slide1.xml",
            "xlsx" => "xl/worksheets/sheet1.xml",
            "docx" => "word/document.xml",
            _ => unreachable!(),
        };
        let mut before = zip::ZipArchive::new(std::io::Cursor::new(&original)).unwrap();
        let mut after = zip::ZipArchive::new(fs::File::open(output).unwrap()).unwrap();
        assert_eq!(before.len(), after.len(), "package part count changed");
        for index in 0..before.len() {
            let mut part = before.by_index(index).unwrap();
            let name = part.name().to_string();
            let mut other = after.by_name(&name).unwrap();
            if name == changed_part {
                continue;
            }
            let mut a = Vec::new();
            let mut b = Vec::new();
            part.read_to_end(&mut a).unwrap();
            other.read_to_end(&mut b).unwrap();
            assert_eq!(a, b, "unrelated part changed: {relative}: {name}");
        }
        success(&scratch.0, &["validate", "--strict", output]);
        success(&scratch.0, &["outline", output]);
        match *family {
            "pptx" => {
                let read = success(
                    &scratch.0,
                    &[
                        "pptx",
                        "shapes",
                        "show",
                        output,
                        "--slide",
                        "1",
                        "--include-text",
                    ],
                );
                assert!(read.to_string().contains("Corpus revised review"), "{read}");
            }
            "docx" => {
                let read = success(&scratch.0, &["docx", "text", output]);
                assert!(
                    read.to_string().contains("Corpus appended paragraph"),
                    "{read}"
                );
            }
            "xlsx" => {
                let read = success(
                    &scratch.0,
                    &[
                        "xlsx", "ranges", "export", output, "--sheet", "Sales", "--range", "B2:B2",
                    ],
                );
                assert_eq!(read["values"][0][0], "150", "{read}");
            }
            _ => unreachable!(),
        }
        if schema(&scratch.0, input, *sdk_clean) {
            assert!(schema(&scratch.0, output, *sdk_clean));
            sdk_checked += 1;
        }
    }
    eprintln!(
        "corpus: {}/{} round trips; SDK checked {sdk_checked}/{} pairs in {:?}",
        CASES.len(),
        CASES.len(),
        CASES.len(),
        started.elapsed()
    );
    assert!(
        started.elapsed() < Duration::from_secs(180),
        "corpus exceeded 180-second suite budget"
    );
}
