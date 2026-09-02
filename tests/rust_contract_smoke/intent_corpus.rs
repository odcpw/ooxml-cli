use super::*;

fn explicit_json_error(args: &[&str]) -> Value {
    let output = run_ooxml_process(args);
    assert_eq!(output.code, 2, "invalid invocation: {args:?}");
    assert!(
        output.stderr.is_empty(),
        "explicit JSON diagnostics must not contaminate stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("JSON stdout is UTF-8");
    assert_eq!(text.lines().count(), 1, "one JSON object: {text:?}");
    serde_json::from_str(&text).expect("structured invalid-args JSON")
}

#[test]
fn invalid_args_envelope_redirects_the_five_known_first_guess_failures() {
    let cases = [
        (
            vec!["--json", "xlsx", "colwidths", "set", "--col", "A:E"],
            serde_json::json!(["--range"]),
            Some("ooxml --json xlsx colwidths set --range A:E"),
        ),
        (
            vec!["--json", "xlsx", "freeze", "set", "--cell", "A2"],
            serde_json::json!(["--rows", "--cols"]),
            None,
        ),
        (
            vec!["--json", "xlsx", "charts", "create", "--values", "[]"],
            serde_json::json!(["--range", "--table"]),
            None,
        ),
        (
            vec!["--json", "docx", "styles", "apply", "--block", "1"],
            serde_json::json!(["--index"]),
            Some("ooxml --json docx styles apply --index 1"),
        ),
        (
            vec!["--json", "pptx", "text", "set", "--text", "X"],
            serde_json::json!(["ooxml pptx replace text"]),
            None,
        ),
    ];

    for (args, suggestions, corrected) in cases {
        let value = explicit_json_error(&args);
        let error = &value["error"];
        assert_eq!(error["code"], "invalid_args", "{args:?}");
        assert_eq!(error["exitCode"], 2, "{args:?}");
        assert_eq!(error["didYouMean"], suggestions, "{args:?}");
        assert!(
            error["validFlags"]
                .as_array()
                .is_some_and(|flags| !flags.is_empty()),
            "{args:?}: {error:?}"
        );
        assert!(
            error["helpCommand"]
                .as_str()
                .is_some_and(|command| command.starts_with("ooxml help ")),
            "{args:?}: {error:?}"
        );
        assert!(
            error["hint"].as_str().is_some_and(|hint| !hint.is_empty()),
            "{args:?}: {error:?}"
        );
        assert_eq!(error["correctedCommand"].as_str(), corrected, "{args:?}");
    }
}

#[test]
fn invalid_args_text_mode_prints_the_same_recovery_fields_on_stderr() {
    let output = run_ooxml_process(&[
        "--format",
        "text",
        "xlsx",
        "colwidths",
        "set",
        "--col",
        "A:E",
    ]);
    assert_eq!(output.code, 2);
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("text diagnostics are UTF-8");
    assert!(stderr.starts_with("error [invalid_args]: unknown flag: --col\n"));
    assert!(stderr.contains("hint: column spans use --range"));
    assert!(stderr.contains("did you mean: --range"));
    assert!(stderr.contains("valid flags:\n"));
    assert!(stderr.contains("help: ooxml help xlsx colwidths set"));
    assert!(
        stderr.contains("corrected command: ooxml --format text xlsx colwidths set --range A:E")
    );
}

#[test]
fn missing_required_flags_include_manifest_usage_and_valid_flag_inventory() {
    let value = explicit_json_error(&["--json", "xlsx", "colwidths", "set", "workbook.xlsx"]);
    let error = &value["error"];
    let hint = error["hint"].as_str().expect("required-argument hint");
    assert!(
        hint.contains("required flags: --sheet, --range, --width"),
        "{hint}"
    );
    assert!(
        hint.contains("Example: ooxml xlsx colwidths set <file> --sheet <sheet> --range <columns> --width <width>"),
        "{hint}"
    );
    assert_eq!(error["helpCommand"], "ooxml help xlsx colwidths set");
    assert!(
        error["validFlags"]
            .as_array()
            .expect("valid flags")
            .iter()
            .any(|flag| flag == &serde_json::json!({"flag": "--range", "use": "--range <range>"}))
    );
}

#[test]
fn unknown_command_tokens_suggest_a_nearby_manifest_path() {
    let value = explicit_json_error(&["--json", "xlsx", "colwidhts", "set"]);
    let error = &value["error"];
    assert_eq!(
        error["message"],
        "unknown command token 'colwidhts'; run `ooxml help` for usage or `ooxml --json capabilities` for the command inventory"
    );
    assert!(
        error["didYouMean"]
            .as_array()
            .expect("command suggestions")
            .iter()
            .any(|command| command == "ooxml xlsx colwidths set"),
        "{error:?}"
    );
    assert_eq!(error["helpCommand"], "ooxml help");
    assert_eq!(error["correctedCommand"], "ooxml --json xlsx colwidths set");
}

#[test]
fn capabilities_golden_includes_the_documented_error_envelope() {
    let actual = run_ooxml_process(&["--json", "capabilities"]);
    assert_eq!(actual.code, 0);
    assert!(actual.stderr.is_empty());
    let golden_path = Path::new("testdata/golden/command-manifest-contract/capabilities.json");
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::write(golden_path, &actual.stdout).expect("update reviewed capabilities golden");
    }
    let expected = std::fs::read(golden_path).expect("capabilities golden");
    assert_eq!(actual.stdout, expected, "capabilities golden drift");
    let document: Value = serde_json::from_slice(&actual.stdout).expect("capabilities JSON");
    assert_eq!(
        document["errorEnvelope"]["code"],
        "stable machine-readable error category"
    );
    assert_eq!(
        document["errorEnvelope"]["channels"]["explicitJson"],
        "one JSON object on stdout; diagnostics remain empty"
    );
}
