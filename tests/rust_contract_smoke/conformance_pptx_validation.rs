use super::*;

#[test]
fn pptx_presentation_child_order_is_strictly_validated() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-presentation-child-order-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("pptx child order temp dir");

    let bad_order = temp_dir.join("presentation-child-order.pptx");
    rewrite_zip_fixture(
        "testdata/pptx/minimal-title/presentation.pptx",
        &bad_order,
        |name, data| {
            if name == "ppt/presentation.xml" {
                Some((
                    name.to_string(),
                    replace_ascii(
                        data,
                        r#"<p:sldSz cx="9144000" cy="6858000" type="screen4x3"/><p:notesSz cx="6858000" cy="9144000"/>"#,
                        r#"<p:notesSz cx="6858000" cy="9144000"/><p:sldSz cx="9144000" cy="6858000" type="screen4x3"/>"#,
                    ),
                ))
            } else {
                Some((name.to_string(), data))
            }
        },
    );

    let bad_arg = bad_order.to_string_lossy().to_string();
    let validate_args = ["--json", "validate", "--strict", bad_arg.as_str()];
    let (validate_code, validate_stdout, validate_stderr) = run_ooxml(&validate_args);
    assert_eq!(validate_code, 5, "strict validate exit");
    assert_eq!(validate_stderr, None, "strict validate stderr");
    let validate_report = validate_stdout.expect("strict validate stdout");
    assert_eq!(validate_report["valid"], Value::Bool(false));
    assert_diagnostics_contain_code(&validate_report, "PPTX_PRESENTATION_CHILD_ORDER");

    let conformance_args = ["--json", "conformance", "check", bad_arg.as_str()];
    let (conformance_code, conformance_stdout, conformance_stderr) = run_ooxml(&conformance_args);
    assert_eq!(conformance_code, 5, "conformance check exit");
    assert_eq!(conformance_stderr, None, "conformance check stderr");
    let conformance_report = conformance_stdout.expect("conformance check stdout");
    assert_eq!(conformance_report["status"], "failed");
    assert_diagnostics_contain_code(
        check_by_name(&conformance_report, "repo-validation"),
        "PPTX_PRESENTATION_CHILD_ORDER",
    );
    assert_diagnostics_contain_code(
        check_by_name(&conformance_report, "repair-invariants"),
        "PPTX_PRESENTATION_CHILD_ORDER",
    );
}

#[test]
fn conformance_check_matches_rust_baseline_for_pptx_media_validation_failures() {
    for (file, expected_codes) in [
        (
            "testdata/pptx/animations-stale-media/presentation.pptx",
            &[
                "REL_DANGLING_TARGET",
                "PPTX_MISSING_MEDIA",
                "PPTX_MISSING_SLIDE_RELATIONSHIP",
                "PPTX_STALE_MEDIA_REFERENCE",
            ][..],
        ),
        (
            "testdata/pptx/corrupted-missing-media/presentation.pptx",
            &["REL_DANGLING_TARGET", "PPTX_MISSING_MEDIA"][..],
        ),
    ] {
        assert_rust_baseline_match(&["--json", "conformance", "check", file]);

        let (code, stdout, stderr) = run_ooxml(&["--json", "conformance", "check", file]);
        assert_eq!(code, 5, "Rust conformance check exit for {file}");
        assert_eq!(stderr, None, "Rust conformance check stderr for {file}");
        let report = stdout.expect("Rust conformance check stdout");
        let repo_validation = check_by_name(&report, "repo-validation");
        let codes = repo_validation["diagnostics"]
            .as_array()
            .expect("repo-validation diagnostics")
            .iter()
            .map(|diagnostic| diagnostic["code"].as_str().expect("diagnostic code"))
            .collect::<Vec<_>>();
        for expected_code in expected_codes {
            assert!(
                codes.contains(expected_code),
                "missing {expected_code} in {file}: {codes:?}"
            );
        }
    }
}

fn assert_diagnostics_contain_code(report: &Value, code: &str) {
    let codes = report["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .map(|diagnostic| diagnostic["code"].as_str().expect("diagnostic code"))
        .collect::<Vec<_>>();
    assert!(
        codes.contains(&code),
        "missing {code} in diagnostics: {codes:?}"
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
