use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[path = "../src/command_text.rs"]
mod command_text;
use command_text::command_arg;
#[path = "../src/build/path_scrub.rs"]
mod path_scrub;

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ooxml-remediation-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn cli(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("--json")
        .args(args)
        .output()
        .unwrap();
    assert!(
        matches!(output.status.code(), Some(0 | 5)),
        "{args:?}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn check_fix_cli_repairs_each_family_and_dry_run_publishes_nothing() {
    let workspace = Workspace::new();
    for (source, code, extension) in [
        (
            "testdata/docx/scaffold-styles/dangling-style.docx",
            "DOCX_DANGLING_STYLE",
            "docx",
        ),
        (
            "testdata/invalid/missing-chart-source.xlsx",
            "XLSX_CHART_SOURCE_INVALID",
            "xlsx",
        ),
        (
            "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx",
            "PPTX_SHAPE_COLLISION",
            "pptx",
        ),
    ] {
        let file = workspace
            .0
            .join(format!("source with ' quotes.{extension}"));
        fs::copy(source, &file).unwrap();
        let file = file.to_str().unwrap();
        let original = fs::read(file).unwrap();
        let dry = cli(&["check", file, "--fix", "--dry-run", "--openxml-sdk", "skip"]);
        assert_eq!(dry["committed"], false);
        assert_eq!(dry["output"], Value::Null);
        assert_eq!(fs::read(file).unwrap(), original);
        assert!(dry["remediation"]["roundsRun"].as_u64().unwrap() > 0);
        assert!(
            !serde_json::to_string(&dry)
                .unwrap()
                .contains(".ooxml-rust-remediate-"),
            "{dry}"
        );
        let output = workspace.0.join(format!("output.{extension}"));
        fs::write(&output, b"sentinel").unwrap();
        let fixed = cli(&[
            "check",
            file,
            "--fix",
            "--out",
            output.to_str().unwrap(),
            "--openxml-sdk",
            "skip",
        ]);
        assert_eq!(fixed["committed"], true, "{fixed}");
        assert!(
            !fixed["remediation"]["unresolved"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["code"] == code)
        );
        assert_eq!(fixed["mutationEnvelope"]["file"], output.to_str().unwrap());
        assert_eq!(
            cli(&["validate", output.to_str().unwrap(), "--strict"])["valid"],
            true
        );
        assert_eq!(fs::read(file).unwrap(), original);
    }
}

#[test]
fn typed_check_fix_matches_cli_report_and_package_bytes() {
    let workspace = Workspace::new();
    for (family, source) in [
        ("docx", "testdata/docx/scaffold-styles/dangling-style.docx"),
        ("xlsx", "testdata/invalid/missing-chart-source.xlsx"),
        (
            "pptx",
            "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx",
        ),
    ] {
        let file = workspace.0.join(format!("input.{family}"));
        fs::copy(source, &file).unwrap();
        let out = workspace.0.join(format!("fixed.{family}"));
        let cli_report = cli(&[
            "check",
            file.to_str().unwrap(),
            "--fix",
            "--out",
            out.to_str().unwrap(),
            "--openxml-sdk",
            "skip",
        ]);
        let bytes = fs::read(&out).unwrap();
        let mut process = Command::new(env!("CARGO_BIN_EXE_ooxml"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"check_package","arguments":{"file":file,"fix":true,"output":out,"openXmlSdk":"skip"}}});
        writeln!(process.stdin.take().unwrap(), "{request}").unwrap();
        let output = process.wait_with_output().unwrap();
        assert!(output.status.success());
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        let typed = &response["result"]["structuredContent"];
        assert_eq!(
            typed["remediation"], cli_report["remediation"],
            "{response}"
        );
        assert_eq!(typed["mutationEnvelope"], cli_report["mutationEnvelope"]);
        assert_eq!(fs::read(out).unwrap(), bytes);
    }
}

#[test]
fn design_fix_and_fix_before_path_work_without_an_explicit_destination() {
    let workspace = Workspace::new();
    let input = workspace.0.join("document.docx");
    fs::copy("testdata/docx/scaffold-styles/dangling-style.docx", &input).unwrap();
    let source = fs::read(&input).unwrap();
    let fixed = cli(&["design-check", "--fix", input.to_str().unwrap()]);
    assert_eq!(fixed["committed"], true, "{fixed}");
    let output = workspace.0.join("document.fixed.docx");
    assert_eq!(fixed["output"], output.to_str().unwrap());
    assert_eq!(
        cli(&["validate", output.to_str().unwrap(), "--strict"])["valid"],
        true
    );
    assert_eq!(fs::read(&input).unwrap(), source);
    let check = cli(&[
        "check",
        "--fix",
        input.to_str().unwrap(),
        "--openxml-sdk",
        "skip",
    ]);
    assert_eq!(check["committed"], true, "{check}");
    assert_eq!(check["output"], output.to_str().unwrap());
}

fn seed_empty_paragraphs(path: &std::path::Path, count: usize) {
    rewrite_document(path, |xml| {
        let mut xml = xml.to_string();
        let at = xml
            .find("<w:sectPr")
            .unwrap_or_else(|| xml.find("</w:body>").unwrap());
        xml.insert_str(at, &"<w:p/>".repeat(count));
        xml
    });
}

fn rewrite_document(path: &std::path::Path, rewrite: impl Fn(&str) -> String) {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(fs::read(path).unwrap())).unwrap();
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == "word/document.xml" {
            bytes = rewrite(std::str::from_utf8(&bytes).unwrap()).into_bytes();
        }
        entries.push((name, bytes));
    }
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn repeated_delete_op_makes_progress_until_clean_or_round_limit() {
    let workspace = Workspace::new();
    let file = workspace.0.join("spacing.docx");
    cli(&[
        "docx",
        "scaffold",
        file.to_str().unwrap(),
        "--text",
        "Start",
        "--force",
    ]);
    seed_empty_paragraphs(&file, 6);
    assert_eq!(
        cli(&["validate", file.to_str().unwrap(), "--strict"])["valid"],
        true
    );
    let limited = cli(&[
        "design-check",
        file.to_str().unwrap(),
        "--fix",
        "--dry-run",
        "--max-rounds",
        "2",
    ]);
    assert_eq!(
        limited["remediation"]["termination"], "max-rounds",
        "{limited}"
    );
    assert_eq!(limited["remediation"]["roundsRun"], 2);
    let complete = cli(&["design-check", file.to_str().unwrap(), "--fix", "--dry-run"]);
    assert_eq!(
        complete["remediation"]["termination"], "clean",
        "{complete}"
    );
    assert_eq!(complete["remediation"]["roundsRun"], 3);
    assert_eq!(complete["remediation"]["after"]["summary"]["total"], 0);
}

fn normalize_report(value: Value, workspace: &Workspace, family: &str) -> Value {
    let mut paths = Vec::new();
    for stem in ["input", "output"] {
        for suffix in [
            "",
            ".fixed",
            ".style-fixed",
            ".chart-source-fixed",
            ".layout-fixed",
            ".design-fixed",
        ] {
            for render in ["", ".render"] {
                let name = format!("{stem}{suffix}.{family}{render}");
                let path = workspace.0.join(&name);
                paths.push((path_scrub::path_prefix_aliases(&path), format!("<{name}>")));
            }
        }
    }
    paths.sort_by_key(|(aliases, _)| std::cmp::Reverse(aliases[0].len()));
    fn walk(value: Value, paths: &[(Vec<String>, String)]) -> Value {
        match value {
            Value::String(text) => {
                Value::String(paths.iter().fold(text, |text, (aliases, replacement)| {
                    path_scrub::scrub_path_aliases(&text, aliases, replacement)
                }))
            }
            Value::Array(values) => {
                Value::Array(values.into_iter().map(|value| walk(value, paths)).collect())
            }
            Value::Object(values) => Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, walk(value, paths)))
                    .collect(),
            ),
            value => value,
        }
    }
    walk(value, &paths)
}

#[test]
fn remediation_reports_match_environment_neutral_lf_goldens() {
    let workspace = Workspace::new();
    for (family, source) in [
        ("docx", "testdata/docx/scaffold-styles/dangling-style.docx"),
        ("xlsx", "testdata/invalid/missing-chart-source.xlsx"),
        (
            "pptx",
            "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx",
        ),
    ] {
        let input = workspace.0.join(format!("input.{family}"));
        let output = workspace.0.join(format!("output.{family}"));
        fs::copy(source, &input).unwrap();
        let before = fs::read(&input).unwrap();
        for dry_run in [false, true] {
            let mut argv = vec![
                "check",
                input.to_str().unwrap(),
                "--fix",
                "--openxml-sdk",
                "skip",
            ];
            if dry_run {
                argv.push("--dry-run");
            } else {
                argv.extend(["--out", output.to_str().unwrap()]);
            }
            let report = cli(&argv);
            assert_eq!(report["remediation"]["termination"], "clean");
            let report = normalize_report(report, &workspace, family);
            let text = format!("{}\n", serde_json::to_string_pretty(&report).unwrap());
            assert!(
                !text.contains(".ooxml-rust-")
                    && !text.contains(&workspace.0.to_string_lossy().to_string()),
                "{text}"
            );
            let path = format!(
                "testdata/golden/remediation/{family}-{}.json",
                if dry_run { "dry-run" } else { "fixed" }
            );
            if std::env::var_os("UPDATE_GOLDENS").is_some() {
                fs::create_dir_all("testdata/golden/remediation").unwrap();
                fs::write(&path, &text).unwrap();
            }
            let expected = fs::read(&path).unwrap();
            assert!(!expected.contains(&b'\r'), "golden must use LF");
            assert_eq!(text, String::from_utf8(expected).unwrap(), "{path}");
        }
        assert_eq!(fs::read(input).unwrap(), before);
        assert_eq!(
            cli(&["validate", output.to_str().unwrap(), "--strict"])["valid"],
            true
        );
        let schema = cli(&[
            "conformance",
            "check",
            output.to_str().unwrap(),
            "--openxml-sdk",
        ]);
        let schema = schema["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["name"] == "schema")
            .unwrap();
        if schema["status"] == "skipped" {
            assert!(!std::env::var("OOXML_REQUIRE_OPENXML_SDK").is_ok_and(|value| value == "1"));
            eprintln!("{family}: SDK unavailable; report golden unchanged");
        } else {
            assert_eq!(schema["schemaCheck"]["valid"], true, "{schema}");
        }
    }
}

#[test]
fn unfixable_strict_failure_preserves_input_and_existing_output() {
    let workspace = Workspace::new();
    let file = workspace.0.join("numbering.docx");
    cli(&[
        "docx",
        "scaffold",
        file.to_str().unwrap(),
        "--text",
        "Start",
        "--force",
    ]);
    rewrite_document(&file, |_| {
        include_str!("../testdata/docx/scaffold-styles/dangling-numbering-document.xml").to_string()
    });
    let original = fs::read(&file).unwrap();
    let output = workspace.0.join("output.docx");
    fs::write(&output, b"preserve destination").unwrap();
    let before = cli(&["check", file.to_str().unwrap(), "--openxml-sdk", "skip"]);
    let fixed = cli(&[
        "check",
        file.to_str().unwrap(),
        "--fix",
        "--out",
        output.to_str().unwrap(),
        "--openxml-sdk",
        "skip",
    ]);
    assert_eq!(fixed["committed"], false, "{fixed}");
    assert_eq!(fixed["validated"], false);
    assert_eq!(fixed["remediation"]["termination"], "no-fix");
    let before = before["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"].as_str().unwrap().contains("NUMBERING"))
        .unwrap();
    let after = fixed["remediation"]["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == before["code"])
        .unwrap();
    assert_eq!(before["message"], after["message"]);
    assert_eq!(fs::read(file).unwrap(), original);
    assert_eq!(fs::read(output).unwrap(), b"preserve destination");
}

#[test]
fn in_place_backup_and_original_adjacent_design_config_are_respected() {
    let workspace = Workspace::new();
    let file = workspace.0.join("input.docx");
    fs::copy("testdata/docx/scaffold-styles/dangling-style.docx", &file).unwrap();
    let original = fs::read(&file).unwrap();
    let backup = workspace.0.join("backup.docx");
    let fixed = cli(&[
        "check",
        file.to_str().unwrap(),
        "--fix",
        "--in-place",
        "--backup",
        backup.to_str().unwrap(),
        "--openxml-sdk",
        "skip",
    ]);
    assert_eq!(fixed["committed"], true);
    assert_eq!(fs::read(backup).unwrap(), original);
    assert_ne!(fs::read(&file).unwrap(), original);
    seed_empty_paragraphs(&file, 6);
    fs::write(
        workspace.0.join(".ooxml-design.json"),
        r#"{"ignore":["DOCX_EXCESS_EMPTY_PARAGRAPHS"]}"#,
    )
    .unwrap();
    let destination = Workspace::new();
    for command in ["check", "design-check"] {
        let output = destination.0.join(format!("{command}.docx"));
        let mut args = vec![
            command,
            file.to_str().unwrap(),
            "--fix",
            "--out",
            output.to_str().unwrap(),
        ];
        if command == "check" {
            args.extend(["--openxml-sdk", "skip"]);
        }
        let fixed = cli(&args);
        assert_eq!(fixed["remediation"]["roundsRun"], 0, "{fixed}");
        assert_eq!(fs::read(output).unwrap(), fs::read(&file).unwrap());
    }
}
