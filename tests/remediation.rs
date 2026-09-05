use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

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
    assert!(output.stderr.is_empty());
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
    let file = workspace.0.join("input.docx");
    fs::copy("testdata/docx/scaffold-styles/dangling-style.docx", &file).unwrap();
    let out = workspace.0.join("fixed.docx");
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
