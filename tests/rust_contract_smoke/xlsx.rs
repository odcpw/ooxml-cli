// XLSX command-family parity tests live in a child module to keep this integration
// test crate navigable while preserving the shared oracle/fixture helpers above.
use super::*;

include!("xlsx/scaffold.rs");
include!("xlsx/forms.rs");
include!("xlsx/ranges_cells.rs");

include!("xlsx/charts.rs");
include!("xlsx/conditional_formatting.rs");

fn assert_xlsx_structure_command_matches(
    label: &str,
    baseline_args: &[&str],
    rust_args: &[&str],
    replacements: &[(&str, &str)],
) -> Value {
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, baseline_code, "{label} exit");
    assert_eq!(rust_stderr, baseline_stderr, "{label} stderr");
    let rust_result = rust_stdout.expect("rust xlsx structure stdout");
    assert_eq!(
        scrub_paths(rust_result.clone(), replacements),
        scrub_paths(
            baseline_stdout.unwrap_or_else(|| panic!("baseline xlsx structure stdout for {label}")),
            replacements
        ),
        "{label} stdout"
    );
    rust_result
}

include!("xlsx/data_validations.rs");

include!("xlsx/filters_sorts.rs");

include!("xlsx/comments.rs");

fn assert_xlsx_strict_valid(path: &str) {
    let (code, stdout, stderr) = run_ooxml(&["--json", "--strict", "validate", path]);
    assert_eq!(code, 0, "strict validate exit for {path}");
    assert_eq!(stderr, None, "strict validate stderr for {path}");
    assert_eq!(
        stdout.expect("strict validate stdout")["valid"],
        Value::Bool(true),
        "strict validate result for {path}"
    );
}

fn assert_rust_baseline_match_scrubbed(
    label: &str,
    baseline_args: &[&str],
    rust_args: &[&str],
    replacements: &[(&str, &str)],
) -> Option<Value> {
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, baseline_code, "{label} exit");
    assert_eq!(
        rust_stderr
            .clone()
            .map(|value| scrub_paths(value, replacements)),
        baseline_stderr.map(|value| scrub_paths(value, replacements)),
        "{label} stderr"
    );
    assert_eq!(
        rust_stdout
            .clone()
            .map(|value| scrub_paths(value, replacements)),
        baseline_stdout.map(|value| scrub_paths(value, replacements)),
        "{label} stdout"
    );
    rust_stdout
}

include!("xlsx/pivots.rs");

include!("xlsx/hyperlinks.rs");

include!("xlsx/workbook_metadata.rs");

include!("xlsx/sheets.rs");

include!("xlsx/names.rs");

include!("xlsx/tables.rs");
