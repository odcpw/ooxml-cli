use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SAMPLE: &str = "testdata/legion-proof/sample-summary.json";

fn sample() -> Value {
    serde_json::from_str(&fs::read_to_string(SAMPLE).expect("read Legion summary fixture"))
        .expect("parse Legion summary fixture")
}

fn assert_stage_status(value: &Value, field: &str) {
    assert_eq!(value[field]["status"], "passed", "{field}: {value}");
}

fn pwsh() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        let private = PathBuf::from(home).join("pwsh/pwsh");
        if private.is_file() {
            return Some(private);
        }
    }
    Command::new("pwsh")
        .args([
            "-NoProfile",
            "-Command",
            "$PSVersionTable.PSVersion.ToString()",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| PathBuf::from("pwsh"))
}

fn test_output_dir() -> PathBuf {
    let root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target"));
    root.join(format!("legion-proof-report-test-{}", std::process::id()))
}

#[test]
fn committed_legion_summary_has_the_full_proof_contract() {
    let summary = sample();
    assert_eq!(summary["schemaVersion"], "ooxml-cli.legion-proof.v1");
    assert_eq!(summary["fixtureOnly"], true);
    assert_eq!(summary["status"], "passed");
    assert_eq!(summary["skipOffice"], false);
    assert_eq!(summary["counts"]["recipesTotal"], 5);
    assert_eq!(summary["counts"]["recipesPassed"], 5);

    let stage_ids = summary["stages"]
        .as_array()
        .expect("stages array")
        .iter()
        .map(|stage| stage["id"].as_str().expect("stage id"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        stage_ids,
        BTreeSet::from([
            "canonical-recipes",
            "mutation-contract-evidence",
            "openxml-validator-build",
            "release-build",
            "windows-office-edit-smoke",
        ])
    );

    let recipes = summary["recipes"].as_array().expect("recipes array");
    let recipe_ids = recipes
        .iter()
        .map(|recipe| recipe["id"].as_str().expect("recipe id"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        recipe_ids,
        BTreeSet::from([
            "deck-markdown",
            "deck-spec",
            "document-markdown",
            "document-spec",
            "workbook-spec",
        ])
    );
    for recipe in recipes {
        assert_stage_status(recipe, "build");
        assert_stage_status(recipe, "strictValidation");
        assert_stage_status(recipe, "openXmlSdk");
        assert_eq!(recipe["office"]["status"], "passed");
        assert_eq!(recipe["office"]["openStatus"], "passed");
        assert_eq!(recipe["office"]["saveStatus"], "passed");
        assert_eq!(recipe["office"]["repairPromptDetected"], false);
        assert_eq!(recipe["roundTrip"]["sourceUnchanged"], true);
        assert_eq!(recipe["roundTrip"]["strictValidation"], "passed");
        assert_eq!(recipe["roundTrip"]["openXmlSdk"], "passed");
        for field in ["inputSha256", "sourceSha256After", "savedSha256"] {
            let hash = recipe["roundTrip"][field].as_str().expect("SHA-256 string");
            assert_eq!(hash.len(), 64, "{field}: {hash}");
            assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
        }
    }
    assert!(
        summary["proofBoundary"]
            .as_str()
            .expect("proof boundary")
            .contains("not live Office proof")
    );
}

#[test]
fn powershell_report_generator_renders_each_proof_dimension() {
    let Some(pwsh) = pwsh() else {
        eprintln!("skipping PowerShell report generator test: pwsh is unavailable");
        return;
    };
    let output_dir = test_output_dir();
    let _ = fs::remove_dir_all(&output_dir);
    fs::create_dir_all(&output_dir).expect("create report test directory");
    let report = output_dir.join("report.md");
    let output = Command::new(pwsh)
        .args([
            "-NoProfile",
            "-File",
            "tools/legion-proof.ps1",
            "-RenderReportOnly",
            "-SummaryFixture",
            SAMPLE,
            "-ReportPath",
        ])
        .arg(&report)
        .output()
        .expect("run Legion PowerShell report generator");
    assert!(
        output.status.success(),
        "report generator failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let markdown = fs::read_to_string(&report).expect("read generated report");
    for expected in [
        "Overall status: **passed**",
        "## Prerequisites",
        "## Stages",
        "## Contract scenarios",
        "contract-pptx-charts-create | pptx | passed | microsoft-office-com-open",
        "## Canonical recipes",
        "deck-spec | pptx | passed | passed | passed | passed | not detected",
        "Scenarios: 1/1 passed.",
        "Recipes: 5/5 passed.",
    ] {
        assert!(
            markdown.contains(expected),
            "missing {expected:?}:\n{markdown}"
        );
    }
    let _ = fs::remove_dir_all(output_dir);
}

#[test]
fn timeout_cleanup_targets_only_the_spawned_powershell_child() {
    let script = fs::read_to_string("tools/legion-proof.ps1").expect("read Legion proof script");
    assert!(script.contains("Stop-Process -Id $process.Id"));
    assert!(!script.contains("Get-Process -Name"));
    assert!(!script.contains("Stop-NewOfficeProcesses"));
    assert!(script.contains("stopped only its own PowerShell child"));
}
