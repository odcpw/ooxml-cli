use super::*;

#[test]
fn conformance_check_matches_rust_baseline_for_invalid_package_open_wrapper_report() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-wrapper-open-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance wrapper temp dir");
    let invalid_package = temp_dir.join("not-a-zip.xlsx");
    fs::write(&invalid_package, b"not a zip").expect("write invalid package");

    let invalid_arg = invalid_package.to_string_lossy().to_string();
    let args = ["--json", "conformance", "check", invalid_arg.as_str()];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);

    assert_eq!(rust_code, baseline_code, "exit code for invalid package");
    assert_eq!(rust_stderr, baseline_stderr, "stderr for invalid package");
    assert_eq!(
        rust_stdout.clone().map(scrub_file_fields),
        baseline_stdout.map(scrub_file_fields),
        "stdout for invalid package"
    );

    let report = rust_stdout.expect("rust invalid package report");
    assert_eq!(report["family"], "unknown");
    assert_eq!(report["status"], "failed");
    assert_eq!(report["summary"]["failed"], 1);
    assert_eq!(report["summary"]["errors"], 1);
    let package_open = check_by_name(&report, "package-open");
    assert_eq!(package_open["status"], "failed");
    assert_eq!(package_open["diagnostics"][0]["code"], "OOXML_OPEN_FAILED");
}

#[test]
fn conformance_check_matches_rust_baseline_for_repo_validation_failed_report() {
    let args = [
        "--json",
        "conformance",
        "check",
        "testdata/xlsx/corrupted-missing-worksheet/workbook.xlsx",
    ];
    assert_rust_baseline_match(&args);

    let (code, stdout, stderr) = run_ooxml(&args);
    assert_eq!(code, 5);
    assert_eq!(stderr, None);
    let report = stdout.expect("repo validation failure report");
    let repo_validation = check_by_name(&report, "repo-validation");
    assert_eq!(repo_validation["status"], "failed");
    assert!(
        repo_validation["diagnostics"]
            .as_array()
            .expect("repo-validation diagnostics")
            .iter()
            .any(|diag| diag["code"] == "REL_DANGLING_TARGET")
    );
    assert!(
        !serde_json::to_string(repo_validation)
            .expect("repo-validation JSON")
            .contains("OOXML_VALIDATE_FAILED"),
        "ordinary validation diagnostics are not lifecycle wrapper errors"
    );
}

#[test]
fn conformance_check_matches_rust_baseline_for_repair_invariant_diagnostics_without_wrapper() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-wrapper-invariants-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance wrapper invariant temp dir");

    let bad_content_type = temp_dir.join("workbook-content-type-mismatch.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &bad_content_type,
        |name, data| {
            if name == "[Content_Types].xml" {
                Some((
                    name.to_string(),
                    replace_ascii(
                        data,
                        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
                        "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
                    ),
                ))
            } else {
                Some((name.to_string(), data))
            }
        },
    );

    let bad_arg = bad_content_type.to_string_lossy().to_string();
    let args = ["--json", "conformance", "check", bad_arg.as_str()];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);

    assert_eq!(
        rust_code, baseline_code,
        "exit code for repair invariant fixture"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "stderr for repair invariant fixture"
    );
    assert_eq!(
        rust_stdout.clone().map(scrub_file_fields),
        baseline_stdout.map(scrub_file_fields),
        "stdout for repair invariant fixture"
    );

    let report = rust_stdout.expect("repair invariant report");
    let repair_invariants = check_by_name(&report, "repair-invariants");
    assert_eq!(repair_invariants["status"], "failed");
    assert!(
        !serde_json::to_string(repair_invariants)
            .expect("repair invariant JSON")
            .contains("OOXML_REPAIR_INVARIANT_FAILED"),
        "deterministic repair diagnostics are distinct from unexpected wrapper errors"
    );
}

fn check_by_name<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing check {name}: {}", report["checks"]))
}
