use super::*;

#[test]
fn conformance_coverage_keeps_relationship_read_error_in_go_parity() {
    let args = ["--json", "conformance", "coverage"];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_code, go_code, "exit code for {args:?}");
    assert_eq!(rust_stderr, go_stderr, "stderr for {args:?}");
    assert_eq!(rust_stdout, go_stdout, "stdout for {args:?}");

    let report = rust_stdout.expect("coverage report");
    let body = serde_json::to_string(&report).expect("coverage JSON");
    assert!(
        body.contains("OOXML_RELS_READ_ERROR"),
        "relationship read-error diagnostic should remain in static coverage: {body}"
    );
}
