use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const STANDARD_FIXTURE_DIR: &str = "testdata/golden/vba-authoring/xlsx-standard";
const STANDARD_GOLDEN_BIN: &[u8] =
    include_bytes!("../testdata/golden/vba-authoring/xlsx-standard/vbaProject.bin");
const STANDARD_GOLDEN_INSPECT_JSON: &str =
    include_str!("../testdata/golden/vba-authoring/xlsx-standard/inspect-bin.json");
const STANDARD_GOLDEN_SHA256: &str =
    "21479229375710ab564da290ba3e32f430a70ec1bbeaac9b4998a18037faf19c";
const STANDARD_MODULE_SHA256: &str =
    "b35cd93228ec09eee5f3c026b49ffc5d6695d2b76638e9ea778a7b07588c36fc";
const CLASS_FIXTURE_DIR: &str = "testdata/golden/vba-authoring/xlsx-class";
const CLASS_GOLDEN_BIN: &[u8] =
    include_bytes!("../testdata/golden/vba-authoring/xlsx-class/vbaProject.bin");
const CLASS_GOLDEN_INSPECT_JSON: &str =
    include_str!("../testdata/golden/vba-authoring/xlsx-class/inspect-bin.json");
const CLASS_GOLDEN_SHA256: &str =
    "6afab85a97be6608d0bfdf011be599a2c4f1f018447788def5a289d9814f6172";
const USERFORM_FIXTURE_DIR: &str = "testdata/golden/vba-authoring/xlsx-userform";
const USERFORM_GOLDEN_BIN: &[u8] =
    include_bytes!("../testdata/golden/vba-authoring/xlsx-userform/vbaProject.bin");
const USERFORM_GOLDEN_INSPECT_JSON: &str =
    include_str!("../testdata/golden/vba-authoring/xlsx-userform/inspect-bin.json");
const USERFORM_GOLDEN_SHA256: &str =
    "2f2e8d21d1615bf57d7df66a257b6c8f1091109794138905c81ac4f7d5fce3c0";
const REBUILT_FIXTURE_DIR: &str = "testdata/golden/vba-authoring/xlsx-rebuilt";
const REBUILT_GOLDEN_PACKAGE: &[u8] =
    include_bytes!("../testdata/golden/vba-authoring/xlsx-rebuilt/rebuilt.xlsm");
const REBUILT_GOLDEN_MANIFEST: &[u8] =
    include_bytes!("../testdata/golden/vba-authoring/xlsx-rebuilt/vba-project.json");
const REBUILT_GOLDEN_PROVENANCE: &str =
    include_str!("../testdata/golden/vba-authoring/xlsx-rebuilt/PROVENANCE.md");
const REBUILT_GOLDEN_SHA256: &str =
    "a3101c5238f329dcfb6dde3dbdc385b8cdbce52418654e7f59c46d8f2a4d5114";
const USERFORM_RUNTIME_WARNING: &str = "generated MSForms UserForms open as package content but are not runtime-loadable; treat as package/list/extract support";

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

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n")
}

#[test]
fn xlsx_standard_vba_build_bin_matches_golden_and_attaches_to_existing_and_scaffolded_workbooks() {
    let temp_dir = temp_dir("vba-xlsx-standard-authoring-golden");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let source = format!("{STANDARD_FIXTURE_DIR}/AgentSmoke.bas");
    let generated_bin_path = temp_dir.join("vbaProject.bin");
    let generated_bin = path_string(&generated_bin_path);

    let build = assert_ok(
        "build xlsx standard vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "build-bin",
            "--family",
            "xlsx",
            "--source",
            &source,
            "--out",
            &generated_bin,
        ]),
    );
    assert_eq!(build["backend"], "pure-rust");
    assert_eq!(build["family"], "xlsx");
    assert_eq!(build["bytesWritten"], STANDARD_GOLDEN_BIN.len());
    assert_eq!(build["sha256"], STANDARD_GOLDEN_SHA256);
    assert_eq!(build["modules"].as_array().expect("build modules").len(), 1);
    assert_eq!(build["modules"][0]["name"], "AgentSmoke");
    assert_eq!(build["modules"][0]["kind"], "standard");
    assert_eq!(build["modules"][0]["hostSynthesized"], false);

    let generated_bytes = fs::read(&generated_bin_path).expect("read generated vbaProject.bin");
    assert_eq!(
        generated_bytes, STANDARD_GOLDEN_BIN,
        "standard vbaProject.bin golden drift"
    );
    assert_eq!(sha256_hex(&generated_bytes), STANDARD_GOLDEN_SHA256);

    let mut inspect = assert_ok(
        "inspect generated standard vbaProject.bin",
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
        serde_json::from_str(STANDARD_GOLDEN_INSPECT_JSON).expect("parse inspect-bin golden");
    inspect["file"] = golden_inspect["file"].clone();
    inspect["attachCommandTemplate"] = golden_inspect["attachCommandTemplate"].clone();
    assert_eq!(
        inspect, golden_inspect,
        "standard inspect-bin JSON golden drift"
    );

    let existing_attached_path = temp_dir.join("existing-workbook.xlsm");
    let existing_attached = path_string(&existing_attached_path);
    let existing_attach = assert_ok(
        "attach standard vbaProject.bin to existing workbook",
        run_ooxml(&[
            "--json",
            "vba",
            "attach",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--bin",
            &generated_bin,
            "--out",
            &existing_attached,
        ]),
    );
    assert_eq!(existing_attach["result"]["action"], "attach");
    assert_eq!(existing_attach["result"]["family"], "xlsx");
    assert_eq!(existing_attach["result"]["macroEnabled"], true);
    assert_eq!(existing_attach["vba"]["hasVbaProject"], true);
    assert_eq!(
        existing_attach["vba"]["vbaProject"]["sha256"],
        STANDARD_GOLDEN_SHA256
    );
    let existing_validate = assert_ok(
        "validate existing attached workbook",
        run_ooxml(&["--json", "validate", "--strict", &existing_attached]),
    );
    assert_eq!(existing_validate["valid"], true);
    assert_eq!(existing_validate["summary"]["errors"], 0);
    let existing_conformance = assert_ok(
        "conformance check existing attached workbook",
        run_ooxml(&["--json", "conformance", "check", &existing_attached]),
    );
    assert_eq!(existing_conformance["status"], "passed");
    assert_eq!(existing_conformance["summary"]["failed"], 0);

    let scaffold_path = temp_dir.join("scaffold.xlsx");
    let scaffold_attached_path = temp_dir.join("scaffold.xlsm");
    let extract_dir = temp_dir.join("macros");
    let create_input_path = temp_dir.join("create-input.xlsx");
    let created_xlsm_path = temp_dir.join("created.xlsm");
    let created_bin_path = temp_dir.join("created-vbaProject.bin");
    let scaffold = path_string(&scaffold_path);
    let scaffold_attached = path_string(&scaffold_attached_path);
    let extract = path_string(&extract_dir);
    let create_input = path_string(&create_input_path);
    let created_xlsm = path_string(&created_xlsm_path);
    let created_bin = path_string(&created_bin_path);
    let scaffold_result = assert_ok(
        "scaffold workbook for standard VBA",
        run_ooxml(&["--json", "xlsx", "scaffold", &scaffold, "--force"]),
    );
    assert_eq!(scaffold_result["family"], "xlsx");
    assert_eq!(scaffold_result["created"], true);

    let scaffold_attach = assert_ok(
        "attach standard vbaProject.bin to scaffolded workbook",
        run_ooxml(&[
            "--json",
            "vba",
            "attach",
            &scaffold,
            "--bin",
            &generated_bin,
            "--out",
            &scaffold_attached,
        ]),
    );
    assert_eq!(scaffold_attach["result"]["action"], "attach");
    assert_eq!(scaffold_attach["result"]["family"], "xlsx");
    assert_eq!(scaffold_attach["result"]["macroEnabled"], true);
    assert_eq!(scaffold_attach["vba"]["hasVbaProject"], true);
    assert_eq!(
        scaffold_attach["vba"]["vbaProject"]["sha256"],
        STANDARD_GOLDEN_SHA256
    );

    let validate = assert_ok(
        "validate scaffold-attached workbook",
        run_ooxml(&["--json", "validate", "--strict", &scaffold_attached]),
    );
    assert_eq!(validate["valid"], true);
    assert_eq!(validate["summary"]["errors"], 0);

    let conformance = assert_ok(
        "conformance check scaffold-attached workbook",
        run_ooxml(&["--json", "conformance", "check", &scaffold_attached]),
    );
    assert_eq!(conformance["status"], "passed");
    assert_eq!(conformance["family"], "xlsx");
    assert_eq!(conformance["summary"]["failed"], 0);

    let list = assert_ok(
        "list scaffold-attached workbook VBA",
        run_ooxml(&["--json", "vba", "list", &scaffold_attached]),
    );
    assert_eq!(list["project"]["family"], "xlsx");
    assert_eq!(list["project"]["moduleCount"], 1);
    assert_eq!(list["project"]["warnings"], Value::Null);
    assert_eq!(list["project"]["modules"][0]["name"], "AgentSmoke");
    assert_eq!(list["project"]["modules"][0]["kind"], "standard");
    assert_eq!(
        list["project"]["modules"][0]["sha256"],
        STANDARD_MODULE_SHA256
    );
    assert_eq!(
        list["validateCommand"],
        format!(
            "ooxml --json validate --strict {}",
            command_arg_for_test(&scaffold_attached)
        )
    );
    assert_eq!(
        list["conformanceCommand"],
        format!(
            "ooxml --json conformance check {}",
            command_arg_for_test(&scaffold_attached)
        )
    );

    let create_scaffold = assert_ok(
        "scaffold workbook for vba create --pure",
        run_ooxml(&["--json", "xlsx", "scaffold", &create_input, "--force"]),
    );
    assert_eq!(create_scaffold["family"], "xlsx");
    assert_eq!(create_scaffold["created"], true);

    let create = assert_ok(
        "create pure xlsm from standard source",
        run_ooxml(&[
            "--json",
            "vba",
            "create",
            &create_input,
            "--pure",
            "--family",
            "xlsx",
            "--source",
            &source,
            "--out",
            &created_xlsm,
        ]),
    );
    assert_eq!(create["backend"], "pure-rust");
    assert_eq!(create["createMode"], "pure");
    assert_eq!(create["result"]["action"], "attach");
    assert_eq!(create["result"]["family"], "xlsx");
    assert_eq!(create["result"]["macroEnabled"], true);
    assert_eq!(create["authoring"]["sha256"], STANDARD_GOLDEN_SHA256);
    assert_eq!(
        create["vba"]["vbaProject"]["sha256"],
        STANDARD_GOLDEN_SHA256
    );

    let create_validate = assert_ok(
        "validate pure-created workbook",
        run_ooxml(&["--json", "validate", "--strict", &created_xlsm]),
    );
    assert_eq!(create_validate["valid"], true);
    assert_eq!(create_validate["summary"]["errors"], 0);
    let create_conformance = assert_ok(
        "conformance check pure-created workbook",
        run_ooxml(&["--json", "conformance", "check", &created_xlsm]),
    );
    assert_eq!(create_conformance["status"], "passed");
    assert_eq!(create_conformance["family"], "xlsx");
    assert_eq!(create_conformance["summary"]["failed"], 0);
    let _extract_bin = assert_ok(
        "extract vbaProject.bin from pure-created workbook",
        run_ooxml(&[
            "--json",
            "vba",
            "extract-bin",
            &created_xlsm,
            "--out",
            &created_bin,
        ]),
    );
    let created_bin_bytes = fs::read(&created_bin_path).expect("read pure-created vbaProject.bin");
    assert_eq!(created_bin_bytes.len(), STANDARD_GOLDEN_BIN.len());
    assert_eq!(sha256_hex(&created_bin_bytes), STANDARD_GOLDEN_SHA256);
    assert_eq!(
        created_bin_bytes, STANDARD_GOLDEN_BIN,
        "pure-created extracted vbaProject.bin drifted from standard golden"
    );

    let extract_result = assert_ok(
        "extract AgentSmoke from scaffold-attached workbook",
        run_ooxml(&[
            "--json",
            "vba",
            "extract",
            &scaffold_attached,
            "--out-dir",
            &extract,
            "--module",
            "module:AgentSmoke",
        ]),
    );
    assert_eq!(
        extract_result["modules"]
            .as_array()
            .expect("extract modules")
            .len(),
        1
    );
    assert_eq!(extract_result["modules"][0]["name"], "AgentSmoke");
    assert_eq!(
        extract_result["modules"][0]["sha256"],
        STANDARD_MODULE_SHA256
    );
    let extracted_source =
        fs::read_to_string(extract_dir.join("AgentSmoke.bas")).expect("read extracted module");
    let fixture_source = fs::read_to_string(format!("{STANDARD_FIXTURE_DIR}/AgentSmoke.bas"))
        .expect("read standard source fixture");
    assert_eq!(
        normalize_newlines(&extracted_source),
        normalize_newlines(&fixture_source),
        "extracted standard source drifted from fixture"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_class_vba_build_bin_matches_golden_and_attaches() {
    let temp_dir = temp_dir("vba-authoring-golden");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let agent_source = format!("{CLASS_FIXTURE_DIR}/AgentSmoke.bas");
    let worker_source = format!("{CLASS_FIXTURE_DIR}/Worker.cls");
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
    assert_eq!(build["bytesWritten"], CLASS_GOLDEN_BIN.len());
    assert_eq!(build["sha256"], CLASS_GOLDEN_SHA256);
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
    assert_eq!(
        generated_bytes, CLASS_GOLDEN_BIN,
        "vbaProject.bin golden drift"
    );
    assert_eq!(sha256_hex(&generated_bytes), CLASS_GOLDEN_SHA256);

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
        serde_json::from_str(CLASS_GOLDEN_INSPECT_JSON).expect("parse inspect-bin golden");
    inspect["file"] = golden_inspect["file"].clone();
    inspect["attachCommandTemplate"] = golden_inspect["attachCommandTemplate"].clone();
    assert_eq!(inspect, golden_inspect, "inspect-bin JSON golden drift");

    let attached_path = temp_dir.join("workbook.xlsm");
    let attached_bin_path = temp_dir.join("attached-vbaProject.bin");
    let extract_dir = temp_dir.join("macros");
    let attached = path_string(&attached_path);
    let attached_bin = path_string(&attached_bin_path);
    let extract = path_string(&extract_dir);
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

    let conformance = assert_ok(
        "conformance check attached workbook",
        run_ooxml(&["--json", "conformance", "check", &attached]),
    );
    assert_eq!(conformance["status"], "passed");
    assert_eq!(conformance["summary"]["failed"], 0);

    let list = assert_ok(
        "list attached workbook VBA",
        run_ooxml(&["--json", "vba", "list", &attached]),
    );
    assert_eq!(list["project"]["family"], "xlsx");
    assert_eq!(list["project"]["moduleCount"], 4);
    assert_eq!(list["project"]["warnings"], Value::Null);
    assert!(
        list["project"]["modules"]
            .as_array()
            .expect("list modules")
            .iter()
            .any(|module| module["name"] == "AgentSmoke" && module["kind"] == "standard")
    );
    assert!(
        list["project"]["modules"]
            .as_array()
            .expect("list modules")
            .iter()
            .any(|module| module["name"] == "Worker" && module["kind"] == "class")
    );
    assert_eq!(
        list["validateCommand"],
        format!(
            "ooxml --json validate --strict {}",
            command_arg_for_test(&attached)
        )
    );
    assert_eq!(
        list["conformanceCommand"],
        format!(
            "ooxml --json conformance check {}",
            command_arg_for_test(&attached)
        )
    );

    let extract_bin = assert_ok(
        "extract vbaProject.bin from class-attached workbook",
        run_ooxml(&[
            "--json",
            "vba",
            "extract-bin",
            &attached,
            "--out",
            &attached_bin,
        ]),
    );
    assert_eq!(extract_bin["bytesWritten"], CLASS_GOLDEN_BIN.len());
    let attached_bin_bytes =
        fs::read(&attached_bin_path).expect("read class-attached vbaProject.bin");
    assert_eq!(sha256_hex(&attached_bin_bytes), CLASS_GOLDEN_SHA256);
    assert_eq!(
        attached_bin_bytes, CLASS_GOLDEN_BIN,
        "class-attached extracted vbaProject.bin drifted from golden"
    );

    let extract_result = assert_ok(
        "extract Worker from attached workbook",
        run_ooxml(&[
            "--json",
            "vba",
            "extract",
            &attached,
            "--out-dir",
            &extract,
            "--module",
            "module:Worker",
        ]),
    );
    assert_eq!(
        extract_result["conformanceCommand"],
        format!(
            "ooxml --json conformance check {}",
            command_arg_for_test(&attached)
        )
    );
    let extracted_worker =
        fs::read_to_string(extract_dir.join("Worker.cls")).expect("read extracted Worker");
    assert!(extracted_worker.contains("Public Function Message()"));
    assert!(extracted_worker.contains("Hello from build-bin attach"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_userform_vba_build_bin_matches_golden_and_is_byte_deterministic() {
    let temp_dir = temp_dir("vba-xlsx-userform-authoring-golden");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let agent_source = format!("{USERFORM_FIXTURE_DIR}/AgentSmoke.bas");
    let form_source = format!("{USERFORM_FIXTURE_DIR}/Dialog.frm");
    let generated_bin_path = temp_dir.join("vbaProject.bin");
    let second_bin_path = temp_dir.join("vbaProject-second.bin");
    let generated_bin = path_string(&generated_bin_path);
    let second_bin = path_string(&second_bin_path);

    let build = assert_ok(
        "build xlsx userform vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "build-bin",
            "--family",
            "xlsx",
            "--source",
            &agent_source,
            "--source",
            &form_source,
            "--out",
            &generated_bin,
        ]),
    );
    assert_eq!(build["backend"], "pure-rust");
    assert_eq!(build["family"], "xlsx");
    assert_eq!(build["bytesWritten"], USERFORM_GOLDEN_BIN.len());
    assert_eq!(build["sha256"], USERFORM_GOLDEN_SHA256);
    assert_eq!(build["userFormsRuntimeLoadable"], false);
    assert!(
        build["warnings"]
            .as_array()
            .expect("build warnings")
            .iter()
            .any(|warning| warning == USERFORM_RUNTIME_WARNING)
    );
    assert_eq!(build["modules"].as_array().expect("build modules").len(), 4);
    assert_eq!(build["modules"][0]["name"], "ThisWorkbook");
    assert_eq!(build["modules"][0]["hostSynthesized"], true);
    assert_eq!(build["modules"][1]["name"], "Sheet1");
    assert_eq!(build["modules"][1]["hostSynthesized"], true);
    assert_eq!(build["modules"][2]["name"], "AgentSmoke");
    assert_eq!(build["modules"][2]["kind"], "standard");
    assert_eq!(build["modules"][3]["name"], "Dialog");
    assert_eq!(build["modules"][3]["kind"], "userform");

    let _second_build = assert_ok(
        "build xlsx userform vbaProject.bin a second time",
        run_ooxml(&[
            "--json",
            "vba",
            "build-bin",
            "--family",
            "xlsx",
            "--source",
            &agent_source,
            "--source",
            &form_source,
            "--out",
            &second_bin,
        ]),
    );
    let generated_bytes = fs::read(&generated_bin_path).expect("read generated vbaProject.bin");
    let second_bytes = fs::read(&second_bin_path).expect("read second vbaProject.bin");
    assert_eq!(
        generated_bytes, second_bytes,
        "two userform builds must be byte-stable"
    );
    assert_eq!(
        generated_bytes, USERFORM_GOLDEN_BIN,
        "userform vbaProject.bin golden drift"
    );
    assert_eq!(sha256_hex(&generated_bytes), USERFORM_GOLDEN_SHA256);

    let mut inspect = assert_ok(
        "inspect generated userform vbaProject.bin",
        run_ooxml(&[
            "--json",
            "vba",
            "inspect-bin",
            &generated_bin,
            "--family",
            "xlsx",
        ]),
    );
    let golden_inspect: Value = serde_json::from_str(USERFORM_GOLDEN_INSPECT_JSON)
        .expect("parse userform inspect-bin golden");
    inspect["file"] = golden_inspect["file"].clone();
    inspect["attachCommandTemplate"] = golden_inspect["attachCommandTemplate"].clone();
    assert_eq!(
        inspect, golden_inspect,
        "userform inspect-bin JSON golden drift"
    );

    let scaffold_path = temp_dir.join("scaffold.xlsx");
    let attached_path = temp_dir.join("userform.xlsm");
    let extract_dir = temp_dir.join("macros");
    let rebuilt_path = temp_dir.join("rebuilt.xlsm");
    let rebuilt_extract_dir = temp_dir.join("rebuilt-macros");
    let scaffold = path_string(&scaffold_path);
    let attached = path_string(&attached_path);
    let extract = path_string(&extract_dir);
    let rebuilt = path_string(&rebuilt_path);
    let rebuilt_extract = path_string(&rebuilt_extract_dir);

    let scaffold_result = assert_ok(
        "scaffold workbook for userform VBA",
        run_ooxml(&["--json", "xlsx", "scaffold", &scaffold, "--force"]),
    );
    assert_eq!(scaffold_result["family"], "xlsx");
    assert_eq!(scaffold_result["created"], true);

    let attach = assert_ok(
        "attach userform vbaProject.bin to scaffolded workbook",
        run_ooxml(&[
            "--json",
            "vba",
            "attach",
            &scaffold,
            "--bin",
            &generated_bin,
            "--out",
            &attached,
        ]),
    );
    assert_eq!(attach["result"]["action"], "attach");
    assert_eq!(attach["result"]["family"], "xlsx");
    assert_eq!(attach["result"]["macroEnabled"], true);

    let _agent_extract = assert_ok(
        "extract AgentSmoke from userform workbook",
        run_ooxml(&[
            "--json",
            "vba",
            "extract",
            &attached,
            "--out-dir",
            &extract,
            "--module",
            "module:AgentSmoke",
        ]),
    );
    let form_extract = assert_ok(
        "extract Dialog from userform workbook",
        run_ooxml(&[
            "--json",
            "vba",
            "extract",
            &attached,
            "--out-dir",
            &extract,
            "--module",
            "module:Dialog",
        ]),
    );
    assert_eq!(form_extract["modules"][0]["name"], "Dialog");
    assert_eq!(form_extract["modules"][0]["kind"], "userform");

    let extracted_agent =
        fs::read_to_string(extract_dir.join("AgentSmoke.bas")).expect("read AgentSmoke.bas");
    let fixture_agent = fs::read_to_string(&agent_source).expect("read fixture AgentSmoke.bas");
    assert_eq!(
        normalize_newlines(&extracted_agent),
        normalize_newlines(&fixture_agent),
        "extracted standard source drifted from userform fixture"
    );
    let extracted_form =
        fs::read_to_string(extract_dir.join("Dialog.frm")).expect("read Dialog.frm");
    let fixture_form = fs::read_to_string(&form_source).expect("read fixture Dialog.frm");
    assert_eq!(
        normalize_newlines(&extracted_form),
        normalize_newlines(&fixture_form),
        "extracted UserForm wrapper or caption drifted from fixture"
    );
    assert!(extracted_form.starts_with(
        "VERSION 5.00\r\nBegin {C62A69F0-16DC-11CE-9E98-00AA00574A4F} Dialog\r\n   Caption = \"Golden Dialog\"\r\nEnd\r\nAttribute VB_Name = \"Dialog\"\r\n"
    ));
    assert!(
        extracted_form
            .contains("    Caption = \"Runtime caption must not replace designer caption\"\r\n")
    );

    let rebuild = assert_ok(
        "rebuild userform workbook from extracted sources",
        run_ooxml(&[
            "--json",
            "vba",
            "rebuild",
            &attached,
            "--source-dir",
            &extract,
            "--out",
            &rebuilt,
        ]),
    );
    assert_eq!(rebuild["userFormsRuntimeLoadable"], false);
    assert_eq!(rebuild["authoring"]["userFormsRuntimeLoadable"], false);
    assert_eq!(rebuild["authoring"]["sha256"], USERFORM_GOLDEN_SHA256);
    assert_eq!(rebuild["sourcesDiscovered"].as_array().unwrap().len(), 2);
    assert!(
        rebuild["authoring"]["warnings"]
            .as_array()
            .expect("rebuild warnings")
            .iter()
            .any(|warning| warning == USERFORM_RUNTIME_WARNING)
    );

    let _rebuilt_form_extract = assert_ok(
        "extract Dialog from rebuilt userform workbook",
        run_ooxml(&[
            "--json",
            "vba",
            "extract",
            &rebuilt,
            "--out-dir",
            &rebuilt_extract,
            "--module",
            "module:Dialog",
        ]),
    );
    let rebuilt_form = fs::read_to_string(rebuilt_extract_dir.join("Dialog.frm"))
        .expect("read rebuilt Dialog.frm");
    assert_eq!(
        normalize_newlines(&rebuilt_form),
        normalize_newlines(&fixture_form),
        "extract-to-rebuild must preserve the designer caption"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn xlsx_extract_rebuild_package_and_manifest_match_reviewed_goldens() {
    let temp_dir = temp_dir("vba-xlsx-rebuilt-package-golden");
    fs::create_dir_all(&temp_dir).expect("create rebuilt golden temp dir");
    let input_path = temp_dir.join("workbook.xlsx");
    let original_path = temp_dir.join("workbook.xlsm");
    let rebuilt_path = temp_dir.join("rebuilt.xlsm");
    let source_dir = temp_dir.join("sources");
    let input = path_string(&input_path);
    let original = path_string(&original_path);
    let rebuilt = path_string(&rebuilt_path);
    let sources = path_string(&source_dir);
    let standard_source = format!("{CLASS_FIXTURE_DIR}/AgentSmoke.bas");
    let class_source = format!("{CLASS_FIXTURE_DIR}/Worker.cls");

    assert_ok(
        "scaffold rebuilt-package golden workbook",
        run_ooxml(&["--json", "xlsx", "scaffold", &input, "--force"]),
    );
    assert_ok(
        "strict validate rebuilt-package golden scaffold",
        run_ooxml(&["--json", "validate", "--strict", &input]),
    );
    assert_ok(
        "create rebuilt-package golden XLSM",
        run_ooxml(&[
            "--json",
            "vba",
            "create",
            &input,
            "--pure",
            "--family",
            "xlsx",
            "--source",
            &standard_source,
            "--source",
            &class_source,
            "--out",
            &original,
        ]),
    );
    assert_ok(
        "strict validate rebuilt-package golden original",
        run_ooxml(&["--json", "validate", "--strict", &original]),
    );
    let before = assert_ok(
        "list rebuilt-package golden original",
        run_ooxml(&["--json", "vba", "list", &original]),
    );
    let extract = assert_ok(
        "extract rebuilt-package golden sources",
        run_ooxml(&["--json", "vba", "extract", &original, "--out-dir", &sources]),
    );
    assert_eq!(
        extract["manifestPath"],
        source_dir
            .join("vba-project.json")
            .to_string_lossy()
            .as_ref()
    );
    let generated_manifest =
        fs::read(source_dir.join("vba-project.json")).expect("read generated manifest");
    assert_eq!(
        generated_manifest, REBUILT_GOLDEN_MANIFEST,
        "extract manifest drifted from reviewed golden"
    );

    assert_ok(
        "rebuild reviewed golden package",
        run_ooxml(&[
            "--json",
            "vba",
            "rebuild",
            &original,
            "--source-dir",
            &sources,
            "--out",
            &rebuilt,
        ]),
    );
    let validate = assert_ok(
        "strict validate rebuilt golden package",
        run_ooxml(&["--json", "validate", "--strict", &rebuilt]),
    );
    assert_eq!(validate["valid"], true);
    assert_eq!(validate["summary"]["errors"], 0);
    let conformance = assert_ok(
        "conformance check rebuilt golden package",
        run_ooxml(&["--json", "conformance", "check", &rebuilt]),
    );
    assert_eq!(conformance["status"], "passed");
    assert_eq!(conformance["summary"]["failed"], 0);
    let after = assert_ok(
        "list rebuilt golden package",
        run_ooxml(&["--json", "vba", "list", &rebuilt]),
    );
    assert_eq!(
        golden_module_signature(&before),
        golden_module_signature(&after),
        "rebuilt package module name/kind/line-count drift"
    );

    let generated = fs::read(&rebuilt_path).expect("read generated rebuilt package");
    assert_eq!(generated.len(), REBUILT_GOLDEN_PACKAGE.len());
    assert_eq!(sha256_hex(&generated), REBUILT_GOLDEN_SHA256);
    assert_eq!(
        generated, REBUILT_GOLDEN_PACKAGE,
        "rebuilt XLSM package golden drift"
    );
    let golden_validate = assert_ok(
        "strict validate checked-in rebuilt package golden",
        run_ooxml(&[
            "--json",
            "validate",
            "--strict",
            &format!("{REBUILT_FIXTURE_DIR}/rebuilt.xlsm"),
        ]),
    );
    assert_eq!(golden_validate["valid"], true);
    assert!(REBUILT_GOLDEN_PROVENANCE.contains(REBUILT_GOLDEN_SHA256));
    assert!(REBUILT_GOLDEN_PROVENANCE.contains("not desktop Office proof"));

    let _ = fs::remove_dir_all(&temp_dir);
}

fn golden_module_signature(list: &Value) -> Vec<(String, String, u64)> {
    list["project"]["modules"]
        .as_array()
        .expect("golden vba list modules")
        .iter()
        .map(|module| {
            (
                module["name"].as_str().expect("module name").to_string(),
                module["kind"].as_str().expect("module kind").to_string(),
                module["lineCount"].as_u64().expect("module line count"),
            )
        })
        .collect()
}
