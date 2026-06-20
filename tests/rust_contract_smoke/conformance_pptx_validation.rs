use super::*;

#[test]
fn conformance_check_matches_go_for_pptx_media_validation_failures() {
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
        assert_go_rust_match(&["--json", "conformance", "check", file]);

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

fn check_by_name<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing check {name}: {}", report["checks"]))
}
