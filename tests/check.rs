use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

const GOLDEN_CASES: &[(&str, &str, &str, i32, Option<&str>)] = &[
    (
        "pptx-clean",
        "testdata/pptx/title-content/presentation.pptx",
        "pptx",
        0,
        None,
    ),
    (
        "xlsx-clean",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "xlsx",
        0,
        None,
    ),
    (
        "docx-clean",
        "testdata/docx/minimal/document.docx",
        "docx",
        0,
        None,
    ),
    (
        "xlsx-pivot-table-parts",
        "testdata/xlsx/invalid/pivot-table-parts.xlsx",
        "xlsx",
        5,
        Some("XML_UNKNOWN_CHILD"),
    ),
    (
        "docx-dangling-style",
        "testdata/docx/scaffold-styles/dangling-style.docx",
        "docx",
        5,
        Some("DOCX_DANGLING_STYLE"),
    ),
    (
        "pptx-inherited-overlap",
        "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx",
        "pptx",
        0,
        Some("PPTX_SHAPE_COLLISION"),
    ),
];

const FINDING_FIELDS: [&str; 7] = [
    "code",
    "docs",
    "fixCommand",
    "location",
    "message",
    "part",
    "severity",
];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn run_with_env(args: &[&str], env: &[(&str, &str)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .envs(env.iter().copied())
        .output()
        .expect("run ooxml with environment")
}

fn run_check(file: &str) -> Output {
    run(&["--json", "check", file, "--openxml-sdk", "skip"])
}

fn parse_report(output: &Output, context: &str) -> Value {
    assert!(
        output.stderr.is_empty(),
        "{context}: unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{context}: invalid check JSON: {error}: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn assert_or_update_golden(name: &str, actual: &[u8]) {
    let path = Path::new("testdata/golden/check").join(format!("{name}.json"));
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        fs::create_dir_all(path.parent().expect("golden parent"))
            .expect("create check golden directory");
        fs::write(&path, actual).expect("write check golden");
        return;
    }
    let expected = fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "missing check golden {}: {error}; run UPDATE_GOLDENS=1 cargo test --test check, then review every diff",
            path.display()
        )
    });
    assert_eq!(actual, expected, "check golden {}", path.display());
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ooxml-check-test-{label}-{}", std::process::id()))
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("UTF-8 test path")
}

fn run_ok(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("command JSON")
}

fn rewrite_xlsx_references(source: &Path, destination: &Path) {
    let mut archive = ZipArchive::new(File::open(source).expect("open reference source"))
        .expect("read reference source zip");
    let mut writer = ZipWriter::new(File::create(destination).expect("create reference fixture"));
    let mut changed_chart = false;
    let mut changed_table = false;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("read zip entry");
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("read zip entry bytes");
        let options = SimpleFileOptions::default()
            .compression_method(entry.compression())
            .unix_permissions(entry.unix_mode().unwrap_or(0o644));
        if entry.is_dir() {
            writer
                .add_directory(name, options)
                .expect("copy zip directory");
            continue;
        }
        if name == "xl/charts/chart1.xml" {
            let xml = String::from_utf8(bytes).expect("chart XML UTF-8");
            let replaced = xml.replace("Data!", "MissingChart!");
            changed_chart = replaced != xml;
            bytes = replaced.into_bytes();
        } else if name == "xl/tables/table1.xml" {
            let xml = String::from_utf8(bytes).expect("table XML UTF-8");
            let replaced = xml.replace("A1:B4", "NOT_A_RANGE");
            changed_table = replaced != xml;
            bytes = replaced.into_bytes();
        }
        writer.start_file(name, options).expect("copy zip file");
        writer.write_all(&bytes).expect("write zip entry");
    }
    writer.finish().expect("finish reference fixture");
    assert!(changed_chart, "chart source replacement must be exercised");
    assert!(
        changed_table,
        "table reference replacement must be exercised"
    );
}

#[test]
fn six_recipe_and_bad_fixture_reports_match_deterministic_goldens() {
    for (name, file, family, expected_exit, expected_code) in GOLDEN_CASES {
        let first = run_check(file);
        let second = run_check(file);
        assert_eq!(first.status.code(), Some(*expected_exit), "{name}");
        assert_eq!(
            first.stdout, second.stdout,
            "{name}: nondeterministic stdout"
        );
        assert_eq!(
            first.stderr, second.stderr,
            "{name}: nondeterministic stderr"
        );
        assert_or_update_golden(name, &first.stdout);

        let report = parse_report(&first, name);
        assert_eq!(report["schemaVersion"], "ooxml-cli.check.v1", "{name}");
        assert_eq!(report["family"], *family, "{name}");
        assert_eq!(report["file"], *file, "{name}");
        assert_eq!(report["failOn"], "error", "{name}");
        for level in ["structural", "strict", "schema", "visual"] {
            assert!(report["proofLevel"][level].is_string(), "{name}: {level}");
        }
        let findings = report["findings"].as_array().expect("findings array");
        for finding in findings {
            assert_eq!(
                finding
                    .as_object()
                    .expect("finding object")
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                FINDING_FIELDS.into_iter().collect(),
                "{name}: stable finding envelope"
            );
            assert!(
                finding["fixCommand"]
                    .as_str()
                    .is_some_and(|command| command.starts_with("ooxml ")),
                "{name}: every finding needs an executable fix command: {finding}"
            );
            assert!(
                finding["docs"]
                    .as_str()
                    .is_some_and(|docs| !docs.is_empty()),
                "{name}: every finding needs docs: {finding}"
            );
        }
        assert_eq!(
            report["summary"]["total"].as_u64(),
            Some(findings.len() as u64),
            "{name}: summary total"
        );
        if let Some(expected_code) = expected_code {
            assert!(
                findings
                    .iter()
                    .any(|finding| finding["code"] == *expected_code),
                "{name}: missing {expected_code}: {report}"
            );
        } else {
            assert_eq!(report["summary"]["errors"], 0, "{name}: {report}");
        }
    }
}

#[test]
fn three_from_scratch_recipes_have_zero_check_errors() {
    let temp = temp_dir("recipes");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).expect("create recipe temp directory");
    let pptx = temp.join("recipe.pptx");
    let xlsx = temp.join("recipe.xlsx");
    let docx = temp.join("recipe.docx");

    run_ok(&[
        "--json",
        "pptx",
        "scaffold",
        path_text(&pptx),
        "--title",
        "Quarterly review",
        "--subtitle",
        "Prepared for the team",
    ]);
    run_ok(&[
        "--json",
        "xlsx",
        "scaffold",
        path_text(&xlsx),
        "--sheet",
        "Summary",
    ]);
    run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_text(&docx),
        "--text",
        "Proof-ready report",
    ]);

    for package in [&pptx, &xlsx, &docx] {
        let report = parse_report(
            &run_check(path_text(package)),
            &format!("recipe {}", package.display()),
        );
        assert_eq!(report["summary"]["errors"], 0, "{report}");
        assert_eq!(report["proofLevel"]["structural"], "passed", "{report}");
        assert_eq!(report["proofLevel"]["strict"], "passed", "{report}");
    }
    fs::remove_dir_all(temp).expect("remove recipe temp directory");
}

#[test]
fn xlsx_formula_defined_name_table_chart_and_pivot_sources_are_checked() {
    let temp = temp_dir("xlsx-references");
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).expect("create reference temp directory");
    let named = temp.join("named.xlsx");
    let tabled = temp.join("tabled.xlsx");
    let formula = temp.join("formula.xlsx");
    let corrupted = temp.join("corrupted.xlsx");

    run_ok(&[
        "--json",
        "xlsx",
        "names",
        "add",
        "testdata/xlsx/chart-workbook/workbook.xlsx",
        "--name",
        "BrokenName",
        "--ref",
        "MissingName!$A$1",
        "--out",
        path_text(&named),
    ]);
    run_ok(&[
        "--json",
        "xlsx",
        "tables",
        "create",
        path_text(&named),
        "--sheet",
        "Data",
        "--range",
        "A1:B4",
        "--table",
        "Sales",
        "--out",
        path_text(&tabled),
    ]);
    run_ok(&[
        "--json",
        "xlsx",
        "cells",
        "set",
        path_text(&tabled),
        "--sheet",
        "Data",
        "--cell",
        "C1",
        "--formula",
        "=MissingFormula!A1",
        "--out",
        path_text(&formula),
    ]);
    rewrite_xlsx_references(&formula, &corrupted);

    let report = parse_report(&run_check(path_text(&corrupted)), "XLSX references");
    let findings = report["findings"].as_array().expect("reference findings");
    let codes = findings
        .iter()
        .filter_map(|finding| finding["code"].as_str())
        .collect::<BTreeSet<_>>();
    for code in [
        "XLSX_FORMULA_MISSING_SHEET",
        "XLSX_DEFINED_NAME_REFERENCE_INVALID",
        "XLSX_TABLE_REFERENCE_INVALID",
        "XLSX_CHART_SOURCE_INVALID",
    ] {
        assert!(codes.contains(code), "missing {code}: {report}");
    }

    let pivot = parse_report(
        &run_check("testdata/xlsx/invalid/pivot-table-parts.xlsx"),
        "pivot source",
    );
    assert!(
        pivot["findings"]
            .as_array()
            .expect("pivot findings")
            .iter()
            .any(|finding| finding["code"] == "XLSX_PIVOT_SOURCE_INVALID"),
        "{pivot}"
    );
    fs::remove_dir_all(temp).expect("remove reference temp directory");
}

#[test]
fn openxml_sdk_policy_tracks_doctor_and_require_never_silently_skips() {
    let doctor_output = run(&["--json", "doctor", "--only", "openxml-sdk-validator"]);
    assert!(
        doctor_output.stderr.is_empty(),
        "doctor stderr: {}",
        String::from_utf8_lossy(&doctor_output.stderr)
    );
    let doctor: Value =
        serde_json::from_slice(&doctor_output.stdout).expect("Open XML SDK doctor JSON");
    let doctor_check = &doctor["checks"][0];
    let available = doctor_check["status"] == "ok";
    let sdk_required_by_environment =
        std::env::var("OOXML_REQUIRE_OPENXML_SDK").is_ok_and(|value| value == "1");
    if sdk_required_by_environment {
        assert!(
            available,
            "OOXML_REQUIRE_OPENXML_SDK=1 but doctor reports no validator: {doctor_check}"
        );
    }

    let auto_output = run(&[
        "--json",
        "check",
        "testdata/docx/minimal/document.docx",
        "--openxml-sdk",
        "auto",
    ]);
    assert_eq!(auto_output.status.code(), Some(0));
    let auto = parse_report(&auto_output, "SDK auto");
    assert_eq!(
        auto["proofLevel"]["schema"] != "skipped",
        available,
        "{auto}"
    );

    let auto_findings = auto["findings"].as_array().expect("auto findings");
    if available {
        assert!(
            !auto_findings
                .iter()
                .any(|finding| finding["code"] == "CHECK_OPENXML_SDK_SKIPPED"),
            "{auto}"
        );
    } else {
        let skipped = auto_findings
            .iter()
            .find(|finding| finding["code"] == "CHECK_OPENXML_SDK_SKIPPED")
            .unwrap_or_else(|| panic!("auto must log the unavailable validator: {auto}"));
        assert_eq!(skipped["severity"], "info", "{skipped}");
        assert!(
            skipped["fixCommand"]
                .as_str()
                .is_some_and(|command| !command.is_empty()),
            "{skipped}"
        );
    }

    let required_file = if available {
        "testdata/xlsx/invalid/pivot-table-parts.xlsx"
    } else {
        "testdata/docx/minimal/document.docx"
    };
    let required = run(&["--json", "check", required_file, "--openxml-sdk", "require"]);
    assert_eq!(required.status.code(), Some(5), "SDK require");
    let required = parse_report(&required, "SDK require");
    if available {
        assert_eq!(required["proofLevel"]["schema"], "failed", "{required}");
        assert!(
            required["findings"]
                .as_array()
                .expect("schema findings")
                .iter()
                .any(|finding| finding["code"] == "OOXML_OPENXML_SDK_SCHEMA"),
            "{required}"
        );
    } else {
        let finding = required["findings"]
            .as_array()
            .expect("required findings")
            .iter()
            .find(|finding| finding["code"] == "CHECK_OPENXML_SDK_REQUIRED")
            .unwrap_or_else(|| panic!("require must fail on unavailable validator: {required}"));
        let expected_fix = doctor_check["remediationCommand"]
            .as_str()
            .unwrap_or("ooxml --json doctor --only openxml-sdk-validator");
        assert_eq!(finding["severity"], "error", "{finding}");
        assert_eq!(finding["fixCommand"], expected_fix, "{finding}");
        assert_eq!(
            required["summary"]["errors"], 1,
            "missing SDK must be the only error for a clean document: {required}"
        );
    }
}

#[test]
fn fail_on_text_mode_and_optional_render_preserve_the_same_proof_contract() {
    let overlap = "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx";
    let default = run_check(overlap);
    assert_eq!(default.status.code(), Some(0));
    let strict_warnings = run(&[
        "--json",
        "check",
        overlap,
        "--openxml-sdk",
        "skip",
        "--fail-on",
        "warning",
    ]);
    assert_eq!(strict_warnings.status.code(), Some(5));
    let strict_report = parse_report(&strict_warnings, "fail-on warning");
    assert_eq!(strict_report["status"], "warning");
    assert_eq!(strict_report["failOn"], "warning");

    let text = run(&[
        "--format",
        "text",
        "check",
        overlap,
        "--openxml-sdk",
        "skip",
    ]);
    assert_eq!(text.status.code(), Some(0));
    assert!(text.stderr.is_empty());
    let text = String::from_utf8(text.stdout).expect("UTF-8 text report");
    assert!(
        text.contains("Proof: structural=passed, strict=passed, schema=skipped, visual=skipped")
    );
    assert!(text.contains("[warning] PPTX_SHAPE_COLLISION"));
    assert!(text.contains("fix: ooxml --json pptx shapes set-bounds"));

    let rendered = run_with_env(
        &[
            "--json",
            "check",
            "testdata/pptx/title-content/presentation.pptx",
            "--openxml-sdk",
            "skip",
            "--render",
        ],
        &[("OOXML_RUST_MOCK_RENDER", "1")],
    );
    assert_eq!(rendered.status.code(), Some(0));
    let rendered = parse_report(&rendered, "mock render");
    assert_eq!(rendered["proofLevel"]["visual"], "passed");
    assert_eq!(rendered["checks"]["visual"], "passed");
}

#[test]
fn invalid_modes_return_the_standard_structured_error_without_running_proof() {
    for (flag, value, fragment) in [
        ("--openxml-sdk", "sometimes", "auto, require, or skip"),
        ("--fail-on", "info", "error or warning"),
    ] {
        let output = run(&[
            "--json",
            "check",
            "testdata/docx/minimal/document.docx",
            flag,
            value,
        ]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).expect("structured error JSON");
        assert_eq!(error["error"]["code"], "invalid_args");
        assert!(
            error["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains(fragment)),
            "{error}"
        );
    }
}
