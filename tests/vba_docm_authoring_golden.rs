use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE_DIR: &str = "testdata/golden/vba-authoring/docx-standard";
const GOLDEN_BIN: &[u8] =
    include_bytes!("../testdata/golden/vba-authoring/docx-standard/vbaProject.bin");
const GOLDEN_INSPECT_JSON: &str =
    include_str!("../testdata/golden/vba-authoring/docx-standard/inspect-bin.json");
const GOLDEN_SHA256: &str = "d372fcdb4a7e43352242b92c67f348a630a75247087f689357537476f15502a3";
const MODULE_SHA256: &str = "85637d6493ad14f741652e5cff805b1f04898cf3aa5102425a2c066bd909edf3";
const CLASS_FIXTURE_DIR: &str = "testdata/golden/vba-authoring/docx-class";
const CLASS_GOLDEN_BIN: &[u8] =
    include_bytes!("../testdata/golden/vba-authoring/docx-class/vbaProject.bin");
const CLASS_GOLDEN_INSPECT_JSON: &str =
    include_str!("../testdata/golden/vba-authoring/docx-class/inspect-bin.json");
const CLASS_GOLDEN_SHA256: &str =
    "9a0d1e425908a52909d472e794640dec13fd27d56f8b6588a3609d0420070aec";
const CLASS_AGENT_SHA256: &str = "b156bf5d498a54c67b47c89ee1552221542c435822027b17dd7304beb3a548ac";
const CLASS_WORKER_SHA256: &str =
    "a88104a638f283e926aceb9a02000954e23a5ba23ec11bdf46952817b9f66d03";

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

fn assert_ok(label: &str, outcome: (i32, Option<Value>, Option<Value>)) -> Value {
    let (code, stdout, stderr) = outcome;
    assert_eq!(code, 0, "{label} exit, stderr: {stderr:?}");
    assert_eq!(stderr, None, "{label} stderr");
    stdout.unwrap_or_else(|| panic!("{label} stdout"))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn docx_standard_vba_build_bin_matches_golden_and_attaches_to_scaffold() {
    let temp_dir = temp_dir("vba-docm-authoring-golden");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let source = format!("{FIXTURE_DIR}/AgentDoc.bas");
    let generated_bin_path = temp_dir.join("vbaProject.bin");
    let generated_bin = path_string(&generated_bin_path);

    let build = assert_ok(
        "build docx standard vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "build-bin",
            "--family",
            "docx",
            "--source",
            &source,
            "--out",
            &generated_bin,
        ]),
    );
    assert_eq!(build["backend"], "pure-rust");
    assert_eq!(build["family"], "docx");
    assert_eq!(build["bytesWritten"], GOLDEN_BIN.len());
    assert_eq!(build["sha256"], GOLDEN_SHA256);
    assert_eq!(build["projectName"], "Project");
    assert_eq!(build["modules"].as_array().expect("build modules").len(), 2);
    assert_eq!(build["modules"][0]["name"], "ThisDocument");
    assert_eq!(build["modules"][0]["kind"], "document");
    assert_eq!(build["modules"][0]["hostSynthesized"], true);
    assert_eq!(build["modules"][1]["name"], "AgentDoc");
    assert_eq!(build["modules"][1]["kind"], "standard");
    assert_eq!(build["modules"][1]["hostSynthesized"], false);
    assert!(
        build["attachCommandTemplate"]
            .as_str()
            .expect("attach template")
            .contains("document.docm")
    );

    let generated_bytes = fs::read(&generated_bin_path).expect("read generated vbaProject.bin");
    assert_eq!(generated_bytes, GOLDEN_BIN, "vbaProject.bin golden drift");
    assert_eq!(sha256_hex(&generated_bytes), GOLDEN_SHA256);

    let mut inspect = assert_ok(
        "inspect generated docx vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "inspect-bin",
            &generated_bin,
            "--family",
            "docx",
        ]),
    );
    let golden_inspect: Value =
        serde_json::from_str(GOLDEN_INSPECT_JSON).expect("parse inspect-bin golden");
    inspect["file"] = golden_inspect["file"].clone();
    inspect["attachCommandTemplate"] = golden_inspect["attachCommandTemplate"].clone();
    assert_eq!(inspect, golden_inspect, "inspect-bin JSON golden drift");

    let docx_path = temp_dir.join("document.docx");
    let docm_path = temp_dir.join("document.docm");
    let extract_dir = temp_dir.join("macros");
    let docx = path_string(&docx_path);
    let docm = path_string(&docm_path);
    let extract = path_string(&extract_dir);

    let scaffold_result = assert_ok(
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
    assert_eq!(scaffold_result["family"], "docx");
    assert_eq!(scaffold_result["created"], true);

    let attach = assert_ok(
        "attach generated vbaProject.bin to docx",
        run_ooxml(&[
            "--json",
            "vba",
            "attach",
            &docx,
            "--bin",
            &generated_bin,
            "--out",
            &docm,
        ]),
    );
    assert_eq!(attach["result"]["action"], "attach");
    assert_eq!(attach["result"]["family"], "docx");
    assert_eq!(attach["result"]["macroEnabled"], true);
    assert_eq!(attach["vba"]["hasVbaProject"], true);
    assert_eq!(attach["vba"]["macroExtension"], ".docm");
    assert_eq!(attach["vba"]["vbaProject"]["sha256"], GOLDEN_SHA256);

    let validate = assert_ok(
        "validate attached docm",
        run_ooxml(&["--json", "validate", "--strict", &docm]),
    );
    assert_eq!(validate["valid"], true);
    assert_eq!(validate["summary"]["errors"], 0);

    let conformance = assert_ok(
        "conformance check attached docm",
        run_ooxml(&["--json", "conformance", "check", &docm]),
    );
    assert_eq!(conformance["status"], "passed");
    assert_eq!(conformance["family"], "docx");
    assert_eq!(conformance["summary"]["failed"], 0);

    let list = assert_ok(
        "list attached docm VBA",
        run_ooxml(&["--json", "vba", "list", &docm]),
    );
    assert_eq!(list["project"]["family"], "docx");
    assert_eq!(list["project"]["moduleCount"], 2);
    assert_eq!(list["project"]["warnings"], Value::Null);
    let host_warnings = &list["project"]["hostCompatibilityWarnings"];
    assert!(
        host_warnings.is_null() || host_warnings.as_array().is_some_and(Vec::is_empty),
        "standard-only DOCM should not carry host compatibility warnings: {list:?}"
    );
    assert_eq!(list["project"]["modules"][0]["name"], "ThisDocument");
    assert_eq!(list["project"]["modules"][0]["kind"], "class");
    assert_eq!(list["project"]["modules"][1]["name"], "AgentDoc");
    assert_eq!(list["project"]["modules"][1]["kind"], "standard");
    assert_eq!(list["project"]["modules"][1]["sha256"], MODULE_SHA256);

    let extract_result = assert_ok(
        "extract AgentDoc from attached docm",
        run_ooxml(&[
            "--json",
            "vba",
            "extract",
            &docm,
            "--out-dir",
            &extract,
            "--module",
            "module:AgentDoc",
        ]),
    );
    assert_eq!(
        extract_result["modules"]
            .as_array()
            .expect("extract modules")
            .len(),
        1
    );
    assert_eq!(extract_result["modules"][0]["name"], "AgentDoc");
    assert_eq!(extract_result["modules"][0]["sha256"], MODULE_SHA256);
    let extracted_source =
        fs::read_to_string(extract_dir.join("AgentDoc.bas")).expect("read extracted module");
    assert!(extracted_source.contains("Public Sub MarkDocument()"));
    assert!(extracted_source.contains("Hello from DOCM build-bin attach"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_class_vba_build_bin_matches_golden_and_attaches_to_scaffold() {
    let temp_dir = temp_dir("vba-docm-class-authoring-golden");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let agent_source = format!("{CLASS_FIXTURE_DIR}/AgentDoc.bas");
    let worker_source = format!("{CLASS_FIXTURE_DIR}/Worker.cls");
    let generated_bin_path = temp_dir.join("vbaProject.bin");
    let generated_bin = path_string(&generated_bin_path);

    let build = assert_ok(
        "build docx class vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "build-bin",
            "--family",
            "docx",
            "--source",
            &agent_source,
            "--source",
            &worker_source,
            "--out",
            &generated_bin,
        ]),
    );
    assert_eq!(build["backend"], "pure-rust");
    assert_eq!(build["family"], "docx");
    assert_eq!(build["bytesWritten"], CLASS_GOLDEN_BIN.len());
    assert_eq!(build["sha256"], CLASS_GOLDEN_SHA256);
    assert_eq!(build["projectName"], "Project");
    assert_eq!(build["modules"].as_array().expect("build modules").len(), 3);
    assert_eq!(build["modules"][0]["name"], "ThisDocument");
    assert_eq!(build["modules"][0]["kind"], "document");
    assert_eq!(build["modules"][0]["hostSynthesized"], true);
    assert_eq!(build["modules"][1]["name"], "AgentDoc");
    assert_eq!(build["modules"][1]["kind"], "standard");
    assert_eq!(build["modules"][1]["hostSynthesized"], false);
    assert_eq!(build["modules"][2]["name"], "Worker");
    assert_eq!(build["modules"][2]["kind"], "class");
    assert_eq!(build["modules"][2]["hostSynthesized"], false);

    let generated_bytes = fs::read(&generated_bin_path).expect("read generated vbaProject.bin");
    assert_eq!(
        generated_bytes, CLASS_GOLDEN_BIN,
        "DOCM class vbaProject.bin golden drift"
    );
    assert_eq!(sha256_hex(&generated_bytes), CLASS_GOLDEN_SHA256);

    let mut inspect = assert_ok(
        "inspect generated docx class vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "inspect-bin",
            &generated_bin,
            "--family",
            "docx",
        ]),
    );
    let golden_inspect: Value =
        serde_json::from_str(CLASS_GOLDEN_INSPECT_JSON).expect("parse class inspect-bin golden");
    inspect["file"] = golden_inspect["file"].clone();
    inspect["attachCommandTemplate"] = golden_inspect["attachCommandTemplate"].clone();
    assert_eq!(
        inspect, golden_inspect,
        "DOCM class inspect-bin JSON golden drift"
    );

    let docx_path = temp_dir.join("document.docx");
    let docm_path = temp_dir.join("document.docm");
    let extract_dir = temp_dir.join("macros");
    let docx = path_string(&docx_path);
    let docm = path_string(&docm_path);
    let extract = path_string(&extract_dir);

    assert_ok(
        "scaffold docx for class",
        run_ooxml(&[
            "--json",
            "docx",
            "scaffold",
            &docx,
            "--text",
            "Macro class target",
        ]),
    );
    let attach = assert_ok(
        "attach generated class vbaProject.bin to docx",
        run_ooxml(&[
            "--json",
            "vba",
            "attach",
            &docx,
            "--bin",
            &generated_bin,
            "--out",
            &docm,
        ]),
    );
    assert_eq!(attach["result"]["family"], "docx");
    assert_eq!(attach["result"]["macroEnabled"], true);
    assert_eq!(attach["vba"]["vbaProject"]["sha256"], CLASS_GOLDEN_SHA256);

    let validate = assert_ok(
        "validate attached class docm",
        run_ooxml(&["--json", "validate", "--strict", &docm]),
    );
    assert_eq!(validate["valid"], true);
    let conformance = assert_ok(
        "conformance check attached class docm",
        run_ooxml(&["--json", "conformance", "check", &docm]),
    );
    assert_eq!(conformance["status"], "passed");
    assert_eq!(conformance["summary"]["failed"], 0);

    let list = assert_ok(
        "list attached class docm VBA",
        run_ooxml(&["--json", "vba", "list", &docm]),
    );
    assert_eq!(list["project"]["family"], "docx");
    assert_eq!(list["project"]["moduleCount"], 3);
    assert_eq!(list["project"]["warnings"], Value::Null);
    assert_eq!(list["project"]["modules"][0]["name"], "ThisDocument");
    assert_eq!(list["project"]["modules"][1]["name"], "AgentDoc");
    assert_eq!(list["project"]["modules"][1]["kind"], "standard");
    assert_eq!(list["project"]["modules"][1]["sha256"], CLASS_AGENT_SHA256);
    assert_eq!(list["project"]["modules"][2]["name"], "Worker");
    assert_eq!(list["project"]["modules"][2]["kind"], "class");
    assert_eq!(list["project"]["modules"][2]["sha256"], CLASS_WORKER_SHA256);

    let extract_result = assert_ok(
        "extract Worker from attached class docm",
        run_ooxml(&[
            "--json",
            "vba",
            "extract",
            &docm,
            "--out-dir",
            &extract,
            "--module",
            "module:Worker",
        ]),
    );
    assert_eq!(
        extract_result["modules"]
            .as_array()
            .expect("extract modules")
            .len(),
        1
    );
    assert_eq!(extract_result["modules"][0]["name"], "Worker");
    assert_eq!(extract_result["modules"][0]["sha256"], CLASS_WORKER_SHA256);
    let extracted_worker =
        fs::read_to_string(extract_dir.join("Worker.cls")).expect("read extracted Worker");
    assert!(extracted_worker.contains("Public Function Message()"));
    assert!(extracted_worker.contains("Hello from DOCM class"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_pure_create_infers_family_and_rejects_userforms() {
    let temp_dir = temp_dir("vba-docm-pure-create");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let source = format!("{FIXTURE_DIR}/AgentDoc.bas");
    let docx_path = temp_dir.join("created.docx");
    let docm_path = temp_dir.join("created.docm");
    let docx = path_string(&docx_path);
    let docm = path_string(&docm_path);

    assert_ok(
        "scaffold docx for pure create",
        run_ooxml(&[
            "--json",
            "docx",
            "scaffold",
            &docx,
            "--text",
            "Pure create target",
        ]),
    );
    let create = assert_ok(
        "pure create docm",
        run_ooxml(&[
            "--json", "vba", "create", &docx, "--pure", "--source", &source, "--out", &docm,
        ]),
    );
    assert_eq!(create["backend"], "pure-rust");
    assert_eq!(create["createMode"], "pure");
    assert_eq!(create["authoring"]["family"], "docx");
    assert_eq!(create["authoring"]["projectName"], "Project");
    assert_eq!(create["authoring"]["modules"][0]["name"], "ThisDocument");
    assert_eq!(create["authoring"]["modules"][0]["hostSynthesized"], true);
    assert_eq!(create["authoring"]["modules"][1]["name"], "AgentDoc");
    assert_eq!(create["result"]["family"], "docx");
    assert_eq!(create["vba"]["hasVbaProject"], true);

    let validate = assert_ok(
        "validate pure-created docm",
        run_ooxml(&["--json", "validate", "--strict", &docm]),
    );
    assert_eq!(validate["valid"], true);

    let rejected_bin = path_string(&temp_dir.join("rejected.bin"));
    let form_source_path = temp_dir.join("Dialog.frm");
    fs::write(
        &form_source_path,
        "VERSION 5.00\r\nBegin VB.UserForm Dialog\r\nEnd\r\n",
    )
    .expect("write userform source");
    let form_source = path_string(&form_source_path);
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "vba",
        "build-bin",
        "--family",
        "docx",
        "--source",
        &form_source,
        "--out",
        &rejected_bin,
    ]);
    assert_eq!(code, 2, "docx userform refusal exit");
    assert_eq!(stdout, None, "docx userform refusal stdout");
    let stderr = stderr.expect("docx userform refusal stderr");
    assert_eq!(stderr["error"]["code"], "invalid_args");
    assert!(
        stderr["error"]["message"]
            .as_str()
            .expect("message")
            .contains("VBA source must be .bas or .cls"),
        "unexpected userform refusal: {stderr:?}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
