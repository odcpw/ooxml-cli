use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn run_json(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml");
    assert_eq!(
        output.status.code(),
        Some(0),
        "command failed: {}\nstdout: {}\nstderr: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse ooxml JSON")
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ooxml-mutation-envelope-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create test directory");
    path
}

#[test]
fn docx_scaffold_and_paragraph_append_emit_additive_envelopes() {
    let dir = temp_dir("docx");
    let scaffold = dir.join("scaffold.docx");
    let appended = dir.join("appended.docx");
    let scaffold_arg = scaffold.to_string_lossy();
    let appended_arg = appended.to_string_lossy();

    let created = run_json(&[
        "--json",
        "docx",
        "scaffold",
        "--out",
        &scaffold_arg,
        "--text",
        "Envelope seed",
    ]);
    assert_eq!(created["created"], true, "legacy key must remain");
    assert_eq!(created["validated"], true, "legacy key must remain");
    let envelope = &created["mutationEnvelope"];
    assert_eq!(envelope["file"], scaffold_arg.as_ref());
    assert_eq!(envelope["family"], "docx");
    assert_eq!(envelope["destination"]["kind"], "package");
    assert_eq!(envelope["destination"]["partUri"], "/");
    assert_eq!(
        envelope["checkCommand"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>(),
        ["ooxml", "--json", "check"]
    );
    assert!(envelope["renderCommand"].as_str().is_some());
    assert_eq!(envelope["warnings"], serde_json::json!([]));
    assert_eq!(envelope["aliasesApplied"], serde_json::json!([]));
    assert!(Path::new(&*scaffold_arg).is_file());

    let appended_result = run_json(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        &scaffold_arg,
        "--text",
        "Envelope destination",
        "--out",
        &appended_arg,
    ]);
    assert_eq!(appended_result["index"], 2, "legacy key must remain");
    let envelope = &appended_result["mutationEnvelope"];
    assert_eq!(envelope["file"], appended_arg.as_ref());
    assert_eq!(envelope["destination"]["partUri"], "/word/document.xml");
    assert_eq!(envelope["destination"]["primarySelector"], "block:2");
    assert_eq!(envelope["changed"][0]["selector"], "block:2");
    assert!(
        envelope["changed"][0]["afterHash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:") && hash.len() == 71)
    );
    assert!(Path::new(&*appended_arg).is_file());

    let validated = run_json(&["--json", "validate", "--strict", &appended_arg]);
    assert_eq!(validated["valid"], true);
    let readback = run_json(&["--json", "docx", "blocks", &appended_arg]);
    assert_eq!(readback["blocks"][1]["text"], "Envelope destination");

    fs::remove_dir_all(dir).expect("remove test directory");
}

#[test]
fn capabilities_serves_the_pinned_mutation_envelope_schema() {
    let response = run_json(&["--json", "capabilities", "--schema", "mutation-envelope"]);
    assert_eq!(response["schema"], "mutation-envelope");
    let schema = &response["document"];
    assert_eq!(
        schema["$id"],
        "https://ooxml-cli.dev/schemas/mutation-envelope.schema.json"
    );
    assert_eq!(schema["additionalProperties"], false);
    for required in [
        "file",
        "family",
        "command",
        "destination",
        "changed",
        "readbackCommand",
        "validateCommand",
        "conformanceCommand",
        "checkCommand",
        "warnings",
        "aliasesApplied",
        "validated",
    ] {
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == required),
            "missing required schema field {required}"
        );
    }
}
