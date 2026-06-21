use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE_DIR: &str = "testdata/golden/vba-authoring/pptx-standard";
const GOLDEN_BIN: &[u8] =
    include_bytes!("../testdata/golden/vba-authoring/pptx-standard/vbaProject.bin");
const GOLDEN_INSPECT_JSON: &str =
    include_str!("../testdata/golden/vba-authoring/pptx-standard/inspect-bin.json");
const GOLDEN_SHA256: &str = "8752348bae9b3fd624c476431d706ddf03a95ddbdb24e47465ebf98a8a389d0f";
const MODULE_SHA256: &str = "2b88ddbc49a635fc2518ad88a68714f3d7985d91bf62b7a0ec1b07c0b864d21d";
const CLASS_FIXTURE_DIR: &str = "testdata/golden/vba-authoring/pptx-class";
const CLASS_GOLDEN_BIN: &[u8] =
    include_bytes!("../testdata/golden/vba-authoring/pptx-class/vbaProject.bin");
const CLASS_GOLDEN_INSPECT_JSON: &str =
    include_str!("../testdata/golden/vba-authoring/pptx-class/inspect-bin.json");
const CLASS_GOLDEN_SHA256: &str =
    "417f50943286b0a7e4d01afbc7a659970bc42c586ecd9843122b4bff33ea03ea";
const CLASS_AGENT_SHA256: &str = "ff8301f03a2a427aae0349353b44642ed5bfbcfc0dced957e14a1c3c57c030dd";
const CLASS_WORKER_SHA256: &str =
    "1b7170e03af1254fbf2bdcbe96715b3181d0c551a29babba3dbd46e822370c08";

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
fn pptx_standard_vba_build_bin_matches_golden_and_attaches_to_scaffold() {
    let temp_dir = temp_dir("vba-pptm-authoring-golden");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let source = format!("{FIXTURE_DIR}/AgentSlide.bas");
    let generated_bin_path = temp_dir.join("vbaProject.bin");
    let generated_bin = path_string(&generated_bin_path);

    let build = assert_ok(
        "build pptx standard vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "build-bin",
            "--family",
            "pptx",
            "--source",
            &source,
            "--out",
            &generated_bin,
        ]),
    );
    assert_eq!(build["backend"], "pure-rust");
    assert_eq!(build["family"], "pptx");
    assert_eq!(build["bytesWritten"], GOLDEN_BIN.len());
    assert_eq!(build["sha256"], GOLDEN_SHA256);
    assert_eq!(build["modules"].as_array().expect("build modules").len(), 1);
    assert_eq!(build["modules"][0]["name"], "AgentSlide");
    assert_eq!(build["modules"][0]["kind"], "standard");
    assert_eq!(build["modules"][0]["hostSynthesized"], false);

    let generated_bytes = fs::read(&generated_bin_path).expect("read generated vbaProject.bin");
    assert_eq!(generated_bytes, GOLDEN_BIN, "vbaProject.bin golden drift");
    assert_eq!(sha256_hex(&generated_bytes), GOLDEN_SHA256);

    let mut inspect = assert_ok(
        "inspect generated pptx vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "inspect-bin",
            &generated_bin,
            "--family",
            "pptx",
        ]),
    );
    let golden_inspect: Value =
        serde_json::from_str(GOLDEN_INSPECT_JSON).expect("parse inspect-bin golden");
    inspect["file"] = golden_inspect["file"].clone();
    inspect["attachCommandTemplate"] = golden_inspect["attachCommandTemplate"].clone();
    assert_eq!(inspect, golden_inspect, "inspect-bin JSON golden drift");

    let scaffold_path = temp_dir.join("deck.pptx");
    let pptm_path = temp_dir.join("deck.pptm");
    let extract_dir = temp_dir.join("macros");
    let scaffold = path_string(&scaffold_path);
    let pptm = path_string(&pptm_path);
    let extract = path_string(&extract_dir);

    let scaffold_result = assert_ok(
        "scaffold pptx",
        run_ooxml(&[
            "--json",
            "pptx",
            "scaffold",
            &scaffold,
            "--title",
            "Macro Deck",
            "--subtitle",
            "Pure VBA",
            "--force",
        ]),
    );
    assert_eq!(scaffold_result["family"], "pptx");
    assert_eq!(scaffold_result["created"], true);

    let attach = assert_ok(
        "attach generated vbaProject.bin to pptx",
        run_ooxml(&[
            "--json",
            "vba",
            "attach",
            &scaffold,
            "--bin",
            &generated_bin,
            "--out",
            &pptm,
        ]),
    );
    assert_eq!(attach["result"]["action"], "attach");
    assert_eq!(attach["result"]["family"], "pptx");
    assert_eq!(attach["result"]["macroEnabled"], true);
    assert_eq!(attach["vba"]["hasVbaProject"], true);
    assert_eq!(attach["vba"]["macroExtension"], ".pptm");
    assert_eq!(attach["vba"]["vbaProject"]["sha256"], GOLDEN_SHA256);

    let validate = assert_ok(
        "validate attached pptm",
        run_ooxml(&["--json", "validate", "--strict", &pptm]),
    );
    assert_eq!(validate["valid"], true);
    assert_eq!(validate["summary"]["errors"], 0);

    let conformance = assert_ok(
        "conformance check attached pptm",
        run_ooxml(&["--json", "conformance", "check", &pptm]),
    );
    assert_eq!(conformance["status"], "passed");
    assert_eq!(conformance["family"], "pptx");
    assert_eq!(conformance["summary"]["failed"], 0);

    let list = assert_ok(
        "list attached pptm VBA",
        run_ooxml(&["--json", "vba", "list", &pptm]),
    );
    assert_eq!(list["project"]["family"], "pptx");
    assert_eq!(list["project"]["moduleCount"], 1);
    assert_eq!(list["project"]["warnings"], Value::Null);
    assert_eq!(list["project"]["modules"][0]["name"], "AgentSlide");
    assert_eq!(list["project"]["modules"][0]["kind"], "standard");
    assert_eq!(list["project"]["modules"][0]["sha256"], MODULE_SHA256);

    let extract_result = assert_ok(
        "extract AgentSlide from attached pptm",
        run_ooxml(&[
            "--json",
            "vba",
            "extract",
            &pptm,
            "--out-dir",
            &extract,
            "--module",
            "module:AgentSlide",
        ]),
    );
    assert_eq!(
        extract_result["modules"]
            .as_array()
            .expect("extract modules")
            .len(),
        1
    );
    assert_eq!(extract_result["modules"][0]["name"], "AgentSlide");
    assert_eq!(extract_result["modules"][0]["sha256"], MODULE_SHA256);
    let extracted_source =
        fs::read_to_string(extract_dir.join("AgentSlide.bas")).expect("read extracted module");
    assert!(extracted_source.contains("Public Sub MarkDeck()"));
    assert!(extracted_source.contains("Hello from PPTM build-bin attach"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn pptx_class_vba_build_bin_matches_golden_and_attaches_to_scaffold() {
    let temp_dir = temp_dir("vba-pptm-class-authoring-golden");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let agent_source = format!("{CLASS_FIXTURE_DIR}/AgentSlide.bas");
    let worker_source = format!("{CLASS_FIXTURE_DIR}/Worker.cls");
    let generated_bin_path = temp_dir.join("vbaProject.bin");
    let generated_bin = path_string(&generated_bin_path);

    let build = assert_ok(
        "build pptx class vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "build-bin",
            "--family",
            "pptx",
            "--source",
            &agent_source,
            "--source",
            &worker_source,
            "--out",
            &generated_bin,
        ]),
    );
    assert_eq!(build["backend"], "pure-rust");
    assert_eq!(build["family"], "pptx");
    assert_eq!(build["bytesWritten"], CLASS_GOLDEN_BIN.len());
    assert_eq!(build["sha256"], CLASS_GOLDEN_SHA256);
    assert_eq!(build["modules"].as_array().expect("build modules").len(), 2);
    assert_eq!(build["modules"][0]["name"], "AgentSlide");
    assert_eq!(build["modules"][0]["kind"], "standard");
    assert_eq!(build["modules"][0]["hostSynthesized"], false);
    assert_eq!(build["modules"][1]["name"], "Worker");
    assert_eq!(build["modules"][1]["kind"], "class");
    assert_eq!(build["modules"][1]["hostSynthesized"], false);

    let generated_bytes = fs::read(&generated_bin_path).expect("read generated vbaProject.bin");
    assert_eq!(
        generated_bytes, CLASS_GOLDEN_BIN,
        "PPTM class vbaProject.bin golden drift"
    );
    assert_eq!(sha256_hex(&generated_bytes), CLASS_GOLDEN_SHA256);

    let mut inspect = assert_ok(
        "inspect generated pptx class vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "inspect-bin",
            &generated_bin,
            "--family",
            "pptx",
        ]),
    );
    let golden_inspect: Value =
        serde_json::from_str(CLASS_GOLDEN_INSPECT_JSON).expect("parse class inspect-bin golden");
    inspect["file"] = golden_inspect["file"].clone();
    inspect["attachCommandTemplate"] = golden_inspect["attachCommandTemplate"].clone();
    assert_eq!(
        inspect, golden_inspect,
        "PPTM class inspect-bin JSON golden drift"
    );

    let scaffold_path = temp_dir.join("deck.pptx");
    let pptm_path = temp_dir.join("deck.pptm");
    let extract_dir = temp_dir.join("macros");
    let scaffold = path_string(&scaffold_path);
    let pptm = path_string(&pptm_path);
    let extract = path_string(&extract_dir);

    let scaffold_result = assert_ok(
        "scaffold pptx for class",
        run_ooxml(&[
            "--json",
            "pptx",
            "scaffold",
            &scaffold,
            "--title",
            "Macro Deck",
            "--subtitle",
            "Pure VBA Class",
            "--force",
        ]),
    );
    assert_eq!(scaffold_result["family"], "pptx");
    assert_eq!(scaffold_result["created"], true);

    let attach = assert_ok(
        "attach generated class vbaProject.bin to pptx",
        run_ooxml(&[
            "--json",
            "vba",
            "attach",
            &scaffold,
            "--bin",
            &generated_bin,
            "--out",
            &pptm,
        ]),
    );
    assert_eq!(attach["result"]["action"], "attach");
    assert_eq!(attach["result"]["family"], "pptx");
    assert_eq!(attach["result"]["macroEnabled"], true);
    assert_eq!(attach["vba"]["hasVbaProject"], true);
    assert_eq!(attach["vba"]["macroExtension"], ".pptm");
    assert_eq!(attach["vba"]["vbaProject"]["sha256"], CLASS_GOLDEN_SHA256);

    let validate = assert_ok(
        "validate attached class pptm",
        run_ooxml(&["--json", "validate", "--strict", &pptm]),
    );
    assert_eq!(validate["valid"], true);
    assert_eq!(validate["summary"]["errors"], 0);

    let conformance = assert_ok(
        "conformance check attached class pptm",
        run_ooxml(&["--json", "conformance", "check", &pptm]),
    );
    assert_eq!(conformance["status"], "passed");
    assert_eq!(conformance["family"], "pptx");
    assert_eq!(conformance["summary"]["failed"], 0);

    let list = assert_ok(
        "list attached class pptm VBA",
        run_ooxml(&["--json", "vba", "list", &pptm]),
    );
    assert_eq!(list["project"]["family"], "pptx");
    assert_eq!(list["project"]["moduleCount"], 2);
    assert_eq!(list["project"]["warnings"], Value::Null);
    assert_eq!(list["project"]["modules"][0]["name"], "AgentSlide");
    assert_eq!(list["project"]["modules"][0]["kind"], "standard");
    assert_eq!(list["project"]["modules"][0]["sha256"], CLASS_AGENT_SHA256);
    assert_eq!(list["project"]["modules"][1]["name"], "Worker");
    assert_eq!(list["project"]["modules"][1]["kind"], "class");
    assert_eq!(list["project"]["modules"][1]["sha256"], CLASS_WORKER_SHA256);

    let extract_result = assert_ok(
        "extract Worker from attached class pptm",
        run_ooxml(&[
            "--json",
            "vba",
            "extract",
            &pptm,
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
    assert!(extracted_worker.contains("Attribute VB_Name = \"Worker\""));
    assert!(
        extracted_worker
            .contains("Attribute VB_Base = \"0{FCFB3D2A-A0FA-1068-A738-08002B3371B5}\"")
    );
    assert!(extracted_worker.contains("Public Function Message()"));
    assert!(extracted_worker.contains("Hello from PPTM class"));

    let _ = fs::remove_dir_all(&temp_dir);
}
