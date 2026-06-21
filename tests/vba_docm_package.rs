use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn run_ooxml(args: &[&str]) -> (i32, Option<Value>, Option<Value>) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
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

fn assert_ok(label: &str, outcome: (i32, Option<Value>, Option<Value>)) -> Value {
    let (code, stdout, stderr) = outcome;
    assert_eq!(code, 0, "{label} exit, stderr: {stderr:?}");
    assert_eq!(stderr, None, "{label} stderr");
    stdout.unwrap_or_else(|| panic!("{label} stdout"))
}

#[test]
fn docm_opaque_vba_attach_inspect_extract_remove_without_office() {
    let temp_dir = temp_dir("docm-vba-package");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let source_path = temp_dir.join("HelloDocm.bas");
    fs::write(
        &source_path,
        "Attribute VB_Name = \"HelloDocm\"\r\nPublic Sub HelloDocm()\r\nEnd Sub\r\n",
    )
    .expect("write macro source");
    let source = source_path.to_string_lossy().to_string();
    let worker_path = temp_dir.join("Worker.cls");
    fs::write(
        &worker_path,
        "Attribute VB_Name = \"Worker\"\r\nPublic Function Message() As String\r\n    Message = \"Hello from docm host-risk fixture\"\r\nEnd Function\r\n",
    )
    .expect("write class source");
    let worker = worker_path.to_string_lossy().to_string();

    let vba_bin_path = temp_dir.join("vbaProject.bin");
    let vba_bin = vba_bin_path.to_string_lossy().to_string();
    let build = assert_ok(
        "build opaque VBA bin",
        run_ooxml(&[
            "--json",
            "vba",
            "build-bin",
            "--family",
            "xlsx",
            "--source",
            &source,
            "--source",
            &worker,
            "--out",
            &vba_bin,
        ]),
    );
    assert_eq!(build["family"], Value::String("xlsx".to_string()));

    let inspect_bin = assert_ok(
        "inspect opaque VBA bin as docx",
        run_ooxml(&["--json", "vba", "inspect-bin", &vba_bin, "--family", "docx"]),
    );
    assert_eq!(inspect_bin["family"], Value::String("docx".to_string()));
    assert_eq!(
        inspect_bin["project"]["family"],
        Value::String("docx".to_string())
    );
    assert!(
        inspect_bin["attachCommandTemplate"]
            .as_str()
            .expect("attach template")
            .contains("document.docm"),
        "docx inspect-bin should advertise a DOCM attach template: {inspect_bin:?}"
    );

    let docx_path = temp_dir.join("document.docx");
    let docx = docx_path.to_string_lossy().to_string();
    assert_ok(
        "scaffold docx",
        run_ooxml(&[
            "--json",
            "docx",
            "scaffold",
            &docx,
            "--text",
            "Macro target",
        ]),
    );

    let docm_path = temp_dir.join("document.docm");
    let docm = docm_path.to_string_lossy().to_string();
    let attach = assert_ok(
        "attach opaque VBA bin to docx",
        run_ooxml(&[
            "--json", "vba", "attach", &docx, "--bin", &vba_bin, "--out", &docm,
        ]),
    );
    assert_eq!(
        attach["result"]["family"],
        Value::String("docx".to_string())
    );
    assert_eq!(attach["vba"]["family"], Value::String("docx".to_string()));
    assert_eq!(attach["vba"]["macroEnabled"], Value::Bool(true));
    assert_eq!(
        attach["vba"]["mainPartUri"],
        Value::String("/word/document.xml".to_string())
    );
    assert_eq!(
        attach["vba"]["mainContentType"],
        Value::String("application/vnd.ms-word.document.macroEnabled.main+xml".to_string())
    );
    assert_eq!(
        attach["vba"]["vbaProject"]["partUri"],
        Value::String("/word/vbaProject.bin".to_string())
    );
    assert_eq!(
        attach["vba"]["vbaProject"]["contentType"],
        Value::String("application/vnd.ms-office.vbaProject".to_string())
    );
    assert!(
        attach["packageReadbackCommand"]
            .as_str()
            .expect("package readback command")
            .contains("docx blocks"),
        "DOCM attach should advertise docx readback: {attach:?}"
    );
    assert!(
        attach["officeCheckCommand"]
            .as_str()
            .expect("office check command")
            .contains("vba office-check"),
        "DOCM attach should advertise Word office-check: {attach:?}"
    );

    let validate = assert_ok(
        "validate docm",
        run_ooxml(&["--json", "validate", "--strict", &docm]),
    );
    assert_eq!(validate["valid"], Value::Bool(true));
    let conformance = assert_ok(
        "conformance docm",
        run_ooxml(&["--json", "conformance", "check", &docm]),
    );
    assert_eq!(conformance["status"], Value::String("passed".to_string()));

    let inspect = assert_ok(
        "inspect docm",
        run_ooxml(&["--json", "vba", "inspect", &docm]),
    );
    assert_eq!(inspect["vba"]["family"], Value::String("docx".to_string()));
    assert_eq!(inspect["vba"]["hasVbaProject"], Value::Bool(true));

    let list = assert_ok(
        "list docm modules",
        run_ooxml(&["--json", "vba", "list", &docm]),
    );
    assert_eq!(list["project"]["family"], Value::String("docx".to_string()));
    assert!(
        list["project"]["modules"]
            .as_array()
            .expect("modules")
            .iter()
            .any(|module| module["name"] == Value::String("HelloDocm".to_string())),
        "DOCM list should expose attached VBA module: {list:?}"
    );
    assert!(
        list["project"]["hostCompatibilityWarnings"]
            .as_array()
            .expect("host warnings")
            .iter()
            .any(|warning| warning["code"]
                == Value::String("VBA_HOST_NON_WORD_MODULES_IN_DOCM".to_string())),
        "DOCM list should warn when an Excel-shaped opaque binary is attached to Word: {list:?}"
    );

    let extract_dir = temp_dir.join("extracted");
    let extract_dir_string = extract_dir.to_string_lossy().to_string();
    assert_ok(
        "extract docm modules",
        run_ooxml(&[
            "--json",
            "vba",
            "extract",
            &docm,
            "--out-dir",
            &extract_dir_string,
        ]),
    );
    let extracted_source =
        fs::read_to_string(extract_dir.join("HelloDocm.bas")).expect("read extracted module");
    assert!(
        extracted_source.contains("Public Sub HelloDocm"),
        "extracted module source mismatch: {extracted_source}"
    );

    let extracted_bin_path = temp_dir.join("extracted-vbaProject.bin");
    let extracted_bin = extracted_bin_path.to_string_lossy().to_string();
    assert_ok(
        "extract docm vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "extract-bin",
            &docm,
            "--out",
            &extracted_bin,
        ]),
    );
    assert_eq!(
        fs::read(&vba_bin_path).expect("original vbaProject.bin"),
        fs::read(&extracted_bin_path).expect("extracted vbaProject.bin"),
        "opaque vbaProject.bin bytes should survive DOCM attach/extract"
    );

    let removed_path = temp_dir.join("document-without-macros.docx");
    let removed = removed_path.to_string_lossy().to_string();
    let remove = assert_ok(
        "remove docm vba project",
        run_ooxml(&["--json", "vba", "remove", &docm, "--out", &removed]),
    );
    assert_eq!(
        remove["result"]["family"],
        Value::String("docx".to_string())
    );
    assert_eq!(remove["result"]["macroEnabled"], Value::Bool(false));
    assert_eq!(remove["vba"]["macroEnabled"], Value::Bool(false));
    assert_eq!(remove["vba"]["hasVbaProject"], Value::Bool(false));

    let removed_validate = assert_ok(
        "validate removed docx",
        run_ooxml(&["--json", "validate", "--strict", &removed]),
    );
    assert_eq!(removed_validate["valid"], Value::Bool(true));
    let removed_inspect = assert_ok(
        "inspect removed docx",
        run_ooxml(&["--json", "vba", "inspect", &removed]),
    );
    assert_eq!(
        removed_inspect["vba"]["family"],
        Value::String("docx".to_string())
    );
    assert_eq!(removed_inspect["vba"]["macroEnabled"], Value::Bool(false));
    assert_eq!(removed_inspect["vba"]["hasVbaProject"], Value::Bool(false));

    let _ = fs::remove_dir_all(&temp_dir);
}
