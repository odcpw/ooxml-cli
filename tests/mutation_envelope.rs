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

#[test]
fn xlsx_and_pptx_scaffolds_emit_family_specific_envelopes() {
    let dir = temp_dir("scaffolds");
    for (family, extension, title_flag) in
        [("xlsx", "xlsx", "--sheet"), ("pptx", "pptx", "--title")]
    {
        let output = dir.join(format!("created.{extension}"));
        let output_arg = output.to_string_lossy();
        let result = run_json(&[
            "--json",
            family,
            "scaffold",
            "--out",
            &output_arg,
            title_flag,
            "Envelope seed",
        ]);
        let envelope = &result["mutationEnvelope"];
        assert_eq!(envelope["file"], output_arg.as_ref(), "{family}");
        assert_eq!(envelope["family"], family, "{family}");
        assert_eq!(envelope["destination"]["kind"], "package", "{family}");
        assert_eq!(envelope["destination"]["partUri"], "/", "{family}");
        assert!(envelope["renderCommand"].is_string(), "{family}");
        assert_eq!(
            envelope.get("layoutCheckCommand").is_some(),
            family == "pptx",
            "{family}"
        );
        let validated = run_json(&["--json", "validate", "--strict", &output_arg]);
        assert_eq!(validated["valid"], true, "{family}");
        let outline = run_json(&["--json", "outline", &output_arg, "--depth", "3"]);
        assert_eq!(outline["file"], output_arg.as_ref(), "{family}");
    }
    fs::remove_dir_all(dir).expect("remove test directory");
}

#[test]
fn xlsx_cell_and_pptx_shape_envelopes_preserve_writer_destinations() {
    let dir = temp_dir("objects");

    let xlsx_source = dir.join("source.xlsx");
    let xlsx_output = dir.join("cell.xlsx");
    let xlsx_source_arg = xlsx_source.to_string_lossy();
    let xlsx_output_arg = xlsx_output.to_string_lossy();
    run_json(&[
        "--json",
        "xlsx",
        "scaffold",
        "--out",
        &xlsx_source_arg,
        "--sheet",
        "Sheet1",
    ]);
    let cell = run_json(&[
        "--json",
        "xlsx",
        "cells",
        "set",
        &xlsx_source_arg,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--value",
        "Envelope cell",
        "--out",
        &xlsx_output_arg,
    ]);
    let cell_envelope = &cell["mutationEnvelope"];
    assert_eq!(cell_envelope["destination"]["kind"], "cell");
    assert_eq!(cell_envelope["destination"]["primarySelector"], "cell:A1");
    assert_eq!(cell_envelope["destination"]["handle"], cell["handle"]);
    assert_eq!(
        cell_envelope["destination"]["partUri"],
        "/xl/worksheets/sheet1.xml"
    );
    assert!(
        cell_envelope["readbackCommand"]
            .as_str()
            .unwrap()
            .contains("xlsx cells extract")
    );

    let pptx_source = dir.join("source.pptx");
    let pptx_output = dir.join("shape.pptx");
    let pptx_source_arg = pptx_source.to_string_lossy();
    let pptx_output_arg = pptx_output.to_string_lossy();
    run_json(&[
        "--json",
        "pptx",
        "scaffold",
        "--out",
        &pptx_source_arg,
        "--title",
        "Envelope slide",
    ]);
    let shape = run_json(&[
        "--json",
        "pptx",
        "add-textbox",
        &pptx_source_arg,
        "--slide",
        "1",
        "--text",
        "Envelope shape",
        "--x",
        "100000",
        "--y",
        "100000",
        "--cx",
        "1000000",
        "--cy",
        "500000",
        "--out",
        &pptx_output_arg,
    ]);
    let shape_envelope = &shape["mutationEnvelope"];
    assert_eq!(shape_envelope["destination"]["kind"], "shape");
    assert_eq!(
        shape_envelope["destination"]["primarySelector"],
        shape["destination"]["primarySelector"]
    );
    assert_eq!(
        shape_envelope["destination"]["selectors"],
        serde_json::json!([
            shape["destination"]["primarySelector"].clone(),
            shape["destination"]["handle"].clone(),
            shape["destination"]["selectors"][1].clone()
        ])
    );
    assert!(shape_envelope["layoutCheckCommand"].is_string());
    let layout = run_json(&["--json", "pptx", "validate-layout", &pptx_output_arg]);
    assert!(layout["slideReports"].is_array());

    for file in [&xlsx_output_arg, &pptx_output_arg] {
        let validated = run_json(&["--json", "validate", "--strict", file]);
        assert_eq!(validated["valid"], true, "{}", file);
    }
    fs::remove_dir_all(dir).expect("remove test directory");
}
