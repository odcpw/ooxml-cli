use serde_json::Value;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::ZipArchive;

#[test]
fn paragraph_lists_support_three_levels_restart_insert_and_determinism() {
    let temp = temp_dir("lists");
    let first = build_list_document(&temp, "first");
    let second = build_list_document(&temp, "second");
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());

    let numbering = zip_text(&first, "word/numbering.xml");
    assert!(numbering.contains(r#"<w:num w:numId="3"><w:abstractNumId w:val="1"/>"#));
    assert!(numbering.contains(r#"<w:lvlOverride w:ilvl="0"><w:startOverride w:val="1"/>"#));
    let document = zip_text(&first, "word/document.xml");
    for (style, num_id, level) in [
        ("ListBullet", 1, 0),
        ("ListBullet", 1, 1),
        ("ListBullet", 1, 2),
        ("ListNumber", 3, 0),
    ] {
        assert!(document.contains(&format!(r#"<w:pStyle w:val="{style}"/>"#)));
        assert!(document.contains(&format!(
            r#"<w:ilvl w:val="{level}"/><w:numId w:val="{num_id}"/>"#
        )));
    }
    let blocks = run_ok(&["--json", "docx", "blocks", path(&first)]);
    let blocks = blocks["blocks"].as_array().expect("blocks array");
    for text in ["Bullet zero", "Bullet one", "Bullet two", "Restart one"] {
        let block = blocks.iter().find(|block| block["text"] == text).unwrap();
        assert!(
            block["contentHash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(block["numId"].is_number());
        assert!(block["listLevel"].is_number());
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn page_and_section_breaks_are_schema_clean_and_hash_visible() {
    let temp = temp_dir("breaks");
    let source = temp.join("source.docx");
    let page = temp.join("page.docx");
    let section = temp.join("section.docx");
    run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path(&source),
        "--text",
        "First page",
    ]);
    let page_report = run_ok(&[
        "--json",
        "docx",
        "breaks",
        "insert",
        path(&source),
        "--page",
        "--out",
        path(&page),
    ]);
    assert_eq!(page_report["break"], "page");
    assert!(
        page_report["documentHash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    let section_report = run_ok(&[
        "--json",
        "docx",
        "breaks",
        "insert",
        path(&page),
        "--section",
        "--out",
        path(&section),
    ]);
    assert_eq!(section_report["break"], "section");
    assert_eq!(
        zip_text(&page, "word/document.xml")
            .matches(r#"w:type="page""#)
            .count(),
        1
    );
    assert_eq!(
        zip_text(&section, "word/document.xml")
            .matches("<w:sectPr")
            .count(),
        2
    );
    for package in [&source, &page, &section] {
        assert_all_proofs(package);
    }
    fs::remove_dir_all(temp).unwrap();
}

fn build_list_document(temp: &Path, label: &str) -> PathBuf {
    let mut current = temp.join(format!("{label}-0.docx"));
    run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path(&current),
        "--text",
        "List report",
    ]);
    assert_all_proofs(&current);
    for (offset, text, kind, level, restart, insert) in [
        (1, "Bullet zero", "bullet", "0", false, false),
        (2, "Bullet one", "bullet", "1", false, false),
        (3, "Bullet two", "bullet", "2", false, true),
        (4, "Restart one", "number", "0", true, false),
    ] {
        let output = temp.join(format!("{label}-{offset}.docx"));
        let mut args = vec![
            "--json",
            "docx",
            "paragraphs",
            if insert { "insert" } else { "append" },
            path(&current),
            "--text",
            text,
            "--list",
            kind,
            "--level",
            level,
        ];
        if insert {
            args.extend(["--after", "3"]);
        }
        if restart {
            args.push("--restart");
        }
        args.extend(["--out", path(&output)]);
        let report = run_ok(&args);
        assert_eq!(report["list"], kind);
        assert_eq!(report["listLevel"], level.parse::<u32>().unwrap());
        assert!(
            report["documentHash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(report["blockHashes"].is_array());
        assert_all_proofs(&output);
        current = output;
    }
    current
}

fn run(args: &[&str]) -> (Output, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .unwrap();
    let stream = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let report = serde_json::from_slice(stream)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(stream)));
    (output, report)
}

fn run_ok(args: &[&str]) -> Value {
    let (output, report) = run(args);
    assert!(output.status.success(), "{args:?}: {report}");
    report
}

fn assert_all_proofs(package: &Path) {
    let (output, report) = run(&["--json", "validate", "--strict", path(package)]);
    assert!(
        output.status.success(),
        "strict rejected {}: {report}",
        package.display()
    );
    let (output, report) = run(&["--json", "conformance", "check", path(package)]);
    assert!(
        output.status.success(),
        "child-order/style integrity rejected {}: {report}",
        package.display()
    );
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return;
    };
    let dotnet = home.join("dotnet/dotnet");
    let validator = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    if !dotnet.is_file() || !validator.is_file() {
        println!(
            "SKIP Open XML SDK validation for {}: validator unavailable",
            package.display()
        );
        return;
    }
    let output = Command::new(dotnet)
        .args([
            validator.as_os_str(),
            "--json".as_ref(),
            package.as_os_str(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "SDK rejected {}: {}",
        package.display(),
        String::from_utf8_lossy(&output.stdout)
    );
}

fn zip_text(package: &Path, part: &str) -> String {
    let mut archive = ZipArchive::new(File::open(package).unwrap()).unwrap();
    let mut text = String::new();
    archive
        .by_name(part)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ooxml-docx-structure-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}
