use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CLEAN_SCHEMA_ARTIFACTS: [&str; 20] = [
    "testdata/docx/default-ns/document.docx",
    "testdata/docx/headers/document.docx",
    "testdata/docx/hyperlink/document.docx",
    "testdata/docx/minimal/document.docx",
    "testdata/docx/paraid/document.docx",
    "testdata/docx/space-preserve/document.docx",
    "testdata/docx/split-runs/document.docx",
    "testdata/docx/styled-headings/document.docx",
    "testdata/docx/styles-catalog/document.docx",
    "testdata/docx/with-comments/document.docx",
    "testdata/docx/with-fields/document.docx",
    "testdata/docx/with-image/document.docx",
    "testdata/docx/with-media/document.docx",
    "testdata/pptx/edge-empty-paragraphs/presentation.pptx",
    "testdata/pptx/edge-large-deck/presentation.pptx",
    "testdata/pptx/edge-mixed-bullets/presentation.pptx",
    "testdata/pptx/edge-nested-groups/presentation.pptx",
    "testdata/pptx/geometry/flip-both/presentation.pptx",
    "testdata/pptx/geometry/rotation-45/presentation.pptx",
    "testdata/pptx/minimal-title/presentation.pptx",
];

#[test]
fn known_bad_pivot_fixture_reports_openxml_sdk_schema_finding() {
    let fixture = repo_path("testdata/xlsx/invalid/pivot-table-parts.xlsx");
    let (output, report) = run_conformance(&fixture);
    let schema = schema_check(&report);
    if schema_was_skipped(schema, &fixture) {
        return;
    }

    assert_eq!(output.status.code(), Some(5), "report: {report}");
    assert_eq!(schema["status"], "failed");
    assert_eq!(schema["schemaCheck"]["validator"], "openxml-sdk");
    assert_eq!(schema["schemaCheck"]["schema"], "Office2019");
    assert_eq!(schema["schemaCheck"]["errorCount"], 1);
    let findings = schema["diagnostics"]
        .as_array()
        .expect("schema diagnostics array");
    assert_eq!(findings.len(), 1, "schema check: {schema}");
    assert_eq!(findings[0]["part"], "/xl/worksheets/sheet1.xml");
    assert_eq!(findings[0]["xpath"], "/x:worksheet[1]");
    assert!(
        findings[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("pivotTableParts")),
        "schema finding: {}",
        findings[0]
    );
}

#[test]
fn normalized_pivot_fixture_is_openxml_sdk_schema_clean() {
    let fixture = repo_path("testdata/xlsx/invalid/pivot-table-parts.xlsx");
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-openxml-sdk-normalized-pivot-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("create Open XML SDK test directory");
    let output_path = temp_dir.join("normalized.xlsx");

    let repair = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args([
            "--json",
            "repair",
            "normalize",
            path_str(&fixture),
            "--out",
            path_str(&output_path),
        ])
        .output()
        .expect("run repair normalize");
    assert!(
        repair.status.success(),
        "repair normalize failed: stdout={} stderr={}",
        String::from_utf8_lossy(&repair.stdout),
        String::from_utf8_lossy(&repair.stderr)
    );

    let (output, report) = run_conformance(&output_path);
    let schema = schema_check(&report);
    if schema_was_skipped(schema, &output_path) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return;
    }
    assert!(output.status.success(), "report: {report}");
    assert_eq!(schema["status"], "passed");
    assert_eq!(schema["schemaCheck"]["checked"], true);
    assert_eq!(schema["schemaCheck"]["valid"], true);
    assert_eq!(schema["schemaCheck"]["errorCount"], 0);
    assert!(schema.get("diagnostics").is_none());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn openxml_sdk_schema_gate_covers_at_least_twenty_artifacts() {
    let mut validated = 0usize;
    for relative in CLEAN_SCHEMA_ARTIFACTS {
        let artifact = repo_path(relative);
        let (_output, report) = run_conformance(&artifact);
        let schema = schema_check(&report);
        if schema_was_skipped(schema, &artifact) {
            return;
        }
        assert_eq!(
            schema["status"], "passed",
            "Open XML SDK rejected {relative}: {schema}"
        );
        assert_eq!(schema["schemaCheck"]["checked"], true);
        assert_eq!(schema["schemaCheck"]["errorCount"], 0);
        validated += 1;
    }
    println!(
        "Open XML SDK schema gate validated {validated}/{} artifacts",
        CLEAN_SCHEMA_ARTIFACTS.len()
    );
    assert_eq!(validated, CLEAN_SCHEMA_ARTIFACTS.len());
}

#[test]
fn missing_sdk_is_reported_as_skipped_with_doctor_remediation() {
    let fixture = repo_path("testdata/xlsx/minimal-workbook/workbook.xlsx");
    let isolated_home = std::env::temp_dir().join(format!(
        "ooxml-openxml-sdk-missing-home-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&isolated_home);
    std::fs::create_dir_all(&isolated_home).expect("create isolated doctor home");
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args([
            "--json",
            "conformance",
            "check",
            path_str(&fixture),
            "--openxml-sdk",
        ])
        .env("HOME", &isolated_home)
        .env("USERPROFILE", &isolated_home)
        .env("PATH", &isolated_home)
        .env_remove("DOTNET_ROOT")
        .output()
        .expect("run conformance check without an SDK");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).expect("conformance JSON");
    let schema = schema_check(&report);
    assert_eq!(schema["status"], "skipped");
    assert_eq!(schema["schemaCheck"]["checked"], false);
    assert_eq!(
        schema["diagnostics"][0]["code"],
        "OOXML_OPENXML_SDK_SKIPPED"
    );
    assert!(
        schema["diagnostics"][0]["remediation"]
            .as_str()
            .is_some_and(|value| value.contains(".NET 8 SDK"))
    );
    assert!(
        schema["diagnostics"][0]["remediationCommand"]
            .as_str()
            .is_some_and(|value| value.contains("dotnet-install.sh"))
    );
    let _ = std::fs::remove_dir_all(&isolated_home);
}

fn run_conformance(path: &Path) -> (Output, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args([
            "--json",
            "conformance",
            "check",
            path_str(path),
            "--openxml-sdk",
        ])
        .output()
        .expect("run conformance check --openxml-sdk");
    assert!(
        output.stderr.is_empty(),
        "unexpected stderr for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "invalid conformance JSON for {}: {err}; stdout={}",
            path.display(),
            String::from_utf8_lossy(&output.stdout)
        )
    });
    (output, report)
}

fn schema_check(report: &Value) -> &Value {
    report["checks"]
        .as_array()
        .expect("conformance checks array")
        .iter()
        .find(|check| check["name"] == "schema")
        .unwrap_or_else(|| panic!("missing schema check: {report}"))
}

fn schema_was_skipped(schema: &Value, artifact: &Path) -> bool {
    if schema["status"] != "skipped" {
        return false;
    }
    assert_eq!(schema["schemaCheck"]["checked"], false);
    assert_eq!(
        schema["diagnostics"][0]["code"],
        "OOXML_OPENXML_SDK_SKIPPED"
    );
    assert!(
        schema["diagnostics"][0].get("remediation").is_some(),
        "skipped schema check must carry doctor remediation: {schema}"
    );
    let sdk_required = std::env::var("OOXML_REQUIRE_OPENXML_SDK")
        .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
    if sdk_required {
        panic!(
            "OOXML_REQUIRE_OPENXML_SDK is set but schema proof was skipped for {}: {}",
            artifact.display(),
            schema
        );
    }
    eprintln!(
        "SKIP Open XML SDK schema proof for {}: {}",
        artifact.display(),
        schema["diagnostics"][0]["message"]
    );
    true
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("UTF-8 fixture path")
}
