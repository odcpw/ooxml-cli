use serde_json::Value;
use std::process::Command;

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

#[test]
fn vba_run_smoke_rejects_bad_cli_contract_before_office() {
    let (code, stdout, stderr) =
        run_ooxml(&["--json", "vba", "run-smoke", "--smoke-mode", "Bogus"]);
    assert_ne!(code, 0, "invalid smoke mode should fail");
    assert_eq!(stdout, None);
    let error = stderr.expect("invalid smoke mode stderr");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--smoke-mode must be Standard or Class")
    );

    let (code, stdout, stderr) =
        run_ooxml(&["--json", "vba", "run-smoke", "--timeout-seconds", "0"]);
    assert_ne!(code, 0, "zero timeout should fail");
    assert_eq!(stdout, None);
    let error = stderr.expect("zero timeout stderr");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--timeout-seconds must be greater than zero")
    );

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "vba",
        "run-smoke",
        "workbook.xlsm",
        "--smoke-mode",
        "Class",
    ]);
    assert_ne!(code, 0, "smoke mode should be generated-only");
    assert_eq!(stdout, None);
    let error = stderr.expect("provided file smoke mode stderr");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--smoke-mode is only used when vba run-smoke generates a workbook")
    );

    let (code, stdout, stderr) = run_ooxml(&["--json", "vba", "run-smoke", "--macro", "Foo"]);
    assert_ne!(
        code, 0,
        "generated smoke should keep the known macro entrypoint"
    );
    assert_eq!(stdout, None);
    let error = stderr.expect("generated macro override stderr");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--macro is only supported when passing an existing .xlsm file")
    );
}

#[test]
fn vba_run_smoke_is_advertised_as_explicit_opt_in_capability() {
    let (code, stdout, stderr) = run_ooxml(&["--json", "capabilities", "--for", "vba"]);
    assert_eq!(code, 0, "capabilities exit");
    assert_eq!(stderr, None, "capabilities stderr");
    let caps = stdout.expect("capabilities stdout");
    let commands = caps["commands"].as_array().expect("commands array");
    let run_smoke = commands
        .iter()
        .find(|command| command["path"] == "ooxml vba run-smoke")
        .expect("vba run-smoke capability");
    assert_eq!(run_smoke["opCompatible"], false);
    assert!(
        run_smoke["opIneligibleReason"]
            .as_str()
            .unwrap()
            .contains("explicitly asks for macro execution proof")
    );
    let flags = run_smoke["localFlags"].as_array().expect("flags array");
    assert!(flags.iter().any(|flag| flag["name"] == "--smoke-mode"));
    assert!(flags.iter().any(|flag| flag["name"] == "--timeout-seconds"));
}
