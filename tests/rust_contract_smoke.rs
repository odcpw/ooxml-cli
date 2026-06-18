use serde_json::Value;
use std::process::Command;

fn baseline() -> Value {
    serde_json::from_str(include_str!(
        "../testdata/golden/rust-port-contract/baseline.json"
    ))
    .expect("baseline JSON")
}

fn run_ooxml(args: &[&str]) -> (i32, Option<Value>, Option<Value>) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run Rust ooxml");
    let code = output.status.code().unwrap_or(-1);
    let stdout = parse_json(&output.stdout);
    let stderr = parse_json(&output.stderr);
    (code, stdout, stderr)
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
fn frozen_cli_slice_matches_go_baseline() {
    let baseline = baseline();
    for case in baseline["cli"].as_array().expect("cli array") {
        let args: Vec<&str> = case["args"]
            .as_array()
            .expect("args")
            .iter()
            .map(|value| value.as_str().expect("arg string"))
            .collect();
        let (code, stdout, stderr) = run_ooxml(&args);
        assert_eq!(code, case["exitCode"], "exit code for {:?}", args);
        assert_eq!(
            stdout,
            nullable(&case["stdoutJson"]),
            "stdout for {:?}",
            args
        );
        assert_eq!(
            stderr,
            nullable(&case["stderrJson"]),
            "stderr for {:?}",
            args
        );
    }
}

#[test]
fn frozen_pptx_mutation_and_validate_match_go_baseline() {
    let baseline = baseline();
    let temp_dir = std::env::temp_dir().join(format!("ooxml-rust-contract-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let edited = temp_dir.join("edited.pptx");
    let edited_str = edited.to_str().expect("temp path");

    let edit_args = [
        "--json",
        "pptx",
        "replace",
        "text",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--slide",
        "1",
        "--target",
        "title",
        "--text",
        "Rust Port Contract",
        "--out",
        edited_str,
    ];
    let (edit_code, edit_stdout, edit_stderr) = run_ooxml(&edit_args);
    assert_eq!(edit_code, 0);
    assert_eq!(edit_stderr, None);
    let edit_expected = baseline["mutation"]["edit"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            edit_stdout.expect("edit stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        edit_expected
    );
    assert!(edited.exists());

    let validate_args = ["--json", "--strict", "validate", edited_str];
    let (validate_code, validate_stdout, validate_stderr) = run_ooxml(&validate_args);
    assert_eq!(validate_code, 0);
    assert_eq!(validate_stderr, None);
    let validate_expected = baseline["mutation"]["validate"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            validate_stdout.expect("validate stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        validate_expected
    );
}

fn nullable(value: &Value) -> Option<Value> {
    if value.is_null() {
        None
    } else {
        Some(value.clone())
    }
}

fn scrub_path(value: Value, from: &str, to: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace(from, to)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| scrub_path(item, from, to))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, scrub_path(value, from, to)))
                .collect(),
        ),
        other => other,
    }
}
