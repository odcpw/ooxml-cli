use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE_DIR: &str = "testdata/golden/vba-authoring/xlsx-class";
const GOLDEN_BIN: &[u8] =
    include_bytes!("../testdata/golden/vba-authoring/xlsx-class/vbaProject.bin");
const GOLDEN_INSPECT_JSON: &str =
    include_str!("../testdata/golden/vba-authoring/xlsx-class/inspect-bin.json");
const GOLDEN_SHA256: &str = "6afab85a97be6608d0bfdf011be599a2c4f1f018447788def5a289d9814f6172";

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
fn xlsx_class_vba_build_bin_matches_golden_and_attaches() {
    let temp_dir = temp_dir("vba-authoring-golden");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let agent_source = format!("{FIXTURE_DIR}/AgentSmoke.bas");
    let worker_source = format!("{FIXTURE_DIR}/Worker.cls");
    let generated_bin_path = temp_dir.join("vbaProject.bin");
    let generated_bin = path_string(&generated_bin_path);

    let build = assert_ok(
        "build xlsx class vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "build-bin",
            "--family",
            "xlsx",
            "--source",
            &agent_source,
            "--source",
            &worker_source,
            "--out",
            &generated_bin,
        ]),
    );
    assert_eq!(build["backend"], "pure-rust");
    assert_eq!(build["family"], "xlsx");
    assert_eq!(build["bytesWritten"], GOLDEN_BIN.len());
    assert_eq!(build["sha256"], GOLDEN_SHA256);
    assert_eq!(build["modules"].as_array().expect("build modules").len(), 4);
    assert_eq!(build["modules"][0]["name"], "ThisWorkbook");
    assert_eq!(build["modules"][0]["hostSynthesized"], true);
    assert_eq!(build["modules"][1]["name"], "Sheet1");
    assert_eq!(build["modules"][1]["hostSynthesized"], true);
    assert_eq!(build["modules"][2]["name"], "AgentSmoke");
    assert_eq!(build["modules"][2]["kind"], "standard");
    assert_eq!(build["modules"][3]["name"], "Worker");
    assert_eq!(build["modules"][3]["kind"], "class");

    let generated_bytes = fs::read(&generated_bin_path).expect("read generated vbaProject.bin");
    assert_eq!(generated_bytes, GOLDEN_BIN, "vbaProject.bin golden drift");
    assert_eq!(sha256_hex(&generated_bytes), GOLDEN_SHA256);

    let mut inspect = assert_ok(
        "inspect generated vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "inspect-bin",
            &generated_bin,
            "--family",
            "xlsx",
        ]),
    );
    let golden_inspect: Value =
        serde_json::from_str(GOLDEN_INSPECT_JSON).expect("parse inspect-bin golden");
    inspect["file"] = golden_inspect["file"].clone();
    inspect["attachCommandTemplate"] = golden_inspect["attachCommandTemplate"].clone();
    assert_eq!(inspect, golden_inspect, "inspect-bin JSON golden drift");

    let attached_path = temp_dir.join("workbook.xlsm");
    let attached = path_string(&attached_path);
    let attach = assert_ok(
        "attach generated vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "attach",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--bin",
            &generated_bin,
            "--out",
            &attached,
        ]),
    );
    assert_eq!(attach["result"]["macroEnabled"], true);
    assert_eq!(attach["vba"]["hasVbaProject"], true);

    let validate = assert_ok(
        "validate attached workbook",
        run_ooxml(&["--json", "validate", "--strict", &attached]),
    );
    assert_eq!(validate["valid"], true);

    let _ = fs::remove_dir_all(&temp_dir);
}
