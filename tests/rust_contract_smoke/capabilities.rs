// Capability inventory and filter contract tests live here so the parent
// integration test crate can keep the shared Go-oracle helpers in one place.
use super::*;

const DOCX_PARENT_GROUP_COMMANDS: &[&str] = &[
    "ooxml docx",
    "ooxml docx comments",
    "ooxml docx fields",
    "ooxml docx footers",
    "ooxml docx headers",
    "ooxml docx images",
    "ooxml docx paragraphs",
    "ooxml docx styles",
    "ooxml docx tables",
];

const XLSX_PARENT_GROUP_PATHS: &[&str] = &[
    "ooxml xlsx",
    "ooxml xlsx cells",
    "ooxml xlsx charts",
    "ooxml xlsx cols",
    "ooxml xlsx colwidths",
    "ooxml xlsx comments",
    "ooxml xlsx conditional-formats",
    "ooxml xlsx data-validations",
    "ooxml xlsx filters-sorts",
    "ooxml xlsx freeze",
    "ooxml xlsx hyperlinks",
    "ooxml xlsx names",
    "ooxml xlsx pivots",
    "ooxml xlsx ranges",
    "ooxml xlsx rowheights",
    "ooxml xlsx rows",
    "ooxml xlsx sheets",
    "ooxml xlsx tables",
    "ooxml xlsx workbook",
    "ooxml xlsx workbook metadata",
];

const COMMAND_GROUP_REASON: &str = "it is a command group, not a leaf mutation command";

const PPTX_PARENT_GROUP_CAPABILITY_PATHS: &[&str] = &[
    "ooxml pptx",
    "ooxml pptx animations",
    "ooxml pptx charts",
    "ooxml pptx comments",
    "ooxml pptx extract",
    "ooxml pptx fields",
    "ooxml pptx layouts",
    "ooxml pptx masters",
    "ooxml pptx media",
    "ooxml pptx notes",
    "ooxml pptx place",
    "ooxml pptx replace",
    "ooxml pptx shapes",
    "ooxml pptx slides",
    "ooxml pptx tables",
    "ooxml pptx template",
    "ooxml pptx text",
    "ooxml pptx theme",
    "ooxml pptx translate",
    "ooxml pptx xlsx-bindings",
];

include!("capabilities/web_surface.rs");

#[test]
fn docx_parent_group_capabilities_match_go_oracle() {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "capabilities"]);
    assert_eq!(go_code, 0);
    assert_eq!(go_stderr, None);
    let go_caps = go_stdout.expect("go capabilities");

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    for path in DOCX_PARENT_GROUP_COMMANDS {
        let go_command = command_by_path(&go_caps, path);
        let rust_command = command_by_path(&rust_caps, path);
        for field in ["path", "use", "short", "opCompatible", "opIneligibleReason"] {
            assert_eq!(rust_command[field], go_command[field], "{field} for {path}");
        }
        assert!(
            rust_command["targetObjectKinds"]
                .as_array()
                .expect("Rust targetObjectKinds")
                .is_empty(),
            "group command {path} should not advertise target object kinds"
        );
        assert!(
            rust_command["localFlags"]
                .as_array()
                .expect("Rust localFlags")
                .is_empty(),
            "group command {path} should not advertise local flags"
        );
    }
}

#[test]
fn xlsx_parent_group_capabilities_match_go_oracle() {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "capabilities"]);
    assert_eq!(go_code, 0);
    assert_eq!(go_stderr, None);
    let go_caps = go_stdout.expect("go capabilities");

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    for path in XLSX_PARENT_GROUP_PATHS {
        let go_command = command_by_path(&go_caps, path);
        let rust_command = command_by_path(&rust_caps, path);
        for field in ["use", "short", "opCompatible", "opIneligibleReason"] {
            assert_eq!(rust_command[field], go_command[field], "{field} for {path}");
        }
        assert!(is_absent_or_empty_array(
            go_command.get("targetObjectKinds")
        ));
        assert!(is_absent_or_empty_array(go_command.get("localFlags")));
        assert!(is_empty_array(&rust_command["targetObjectKinds"]));
        assert!(is_empty_array(&rust_command["localFlags"]));
        assert_eq!(
            rust_command["opCompatible"], false,
            "{path} op compatibility"
        );
        assert_eq!(
            rust_command["opIneligibleReason"], COMMAND_GROUP_REASON,
            "{path} command-group reason"
        );
    }
}

#[test]
fn pptx_parent_group_capabilities_match_go_oracle_metadata() {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "capabilities"]);
    assert_eq!(go_code, 0);
    assert_eq!(go_stderr, None);
    let go_caps = go_stdout.expect("go capabilities");

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    for path in PPTX_PARENT_GROUP_CAPABILITY_PATHS {
        let go_command = command_by_path(&go_caps, path);
        let rust_command = command_by_path(&rust_caps, path);

        assert_eq!(
            go_command["opCompatible"],
            Value::Bool(false),
            "Go opCompatible for {path}"
        );
        assert_eq!(
            rust_command["opCompatible"],
            Value::Bool(false),
            "Rust opCompatible for {path}"
        );
        assert_eq!(rust_command["use"], go_command["use"], "use for {path}");
        assert_eq!(
            rust_command["short"], go_command["short"],
            "short for {path}"
        );
        for field in ["targetObjectKinds", "localFlags"] {
            assert!(
                optional_array_is_empty(go_command, field),
                "Go {field} should be absent or empty for {path}: {}",
                go_command[field]
            );
            assert!(
                optional_array_is_empty(rust_command, field),
                "Rust {field} should be absent or empty for {path}: {}",
                rust_command[field]
            );
        }
        assert_eq!(
            rust_command["opIneligibleReason"], go_command["opIneligibleReason"],
            "opIneligibleReason for {path}"
        );
    }

    let go_diff = command_by_path(&go_caps, "ooxml pptx diff");
    let rust_diff = command_by_path(&rust_caps, "ooxml pptx diff");
    assert_eq!(go_diff["opCompatible"], Value::Bool(false));
    assert_eq!(
        go_diff["use"],
        Value::String("diff <baseline> <candidate>".to_string())
    );
    assert_eq!(rust_diff["use"], go_diff["use"], "use for ooxml pptx diff");
    assert_eq!(
        rust_diff["short"], go_diff["short"],
        "short for ooxml pptx diff"
    );
    assert_eq!(
        rust_diff["opCompatible"], go_diff["opCompatible"],
        "opCompatible for ooxml pptx diff"
    );
    assert!(
        optional_array_is_empty(rust_diff, "targetObjectKinds"),
        "Rust targetObjectKinds should be absent or empty for ooxml pptx diff"
    );
    assert_eq!(
        local_flag_field(rust_diff, "name"),
        serde_json::json!(["--render", "--threshold", "--out"]),
        "Rust localFlags should advertise ported visual diff flags"
    );
    assert!(
        rust_diff["opIneligibleReason"]
            .as_str()
            .expect("Rust op-ineligible reason")
            .contains("read-only package comparison"),
        "Rust reason should describe read-only diff support"
    );
}

#[test]
fn rust_capability_inventory_is_go_oracle_subset() {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "capabilities"]);
    assert_eq!(go_code, 0);
    assert_eq!(go_stderr, None);
    let go_caps = go_stdout.expect("go capabilities");

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    assert_no_duplicate_command_paths(&go_caps, "Go oracle");
    assert_no_duplicate_command_paths(&rust_caps, "Rust");

    let go_paths = capability_paths(&go_caps);
    let rust_paths = capability_paths(&rust_caps);
    assert_eq!(go_paths.len(), 295, "Go oracle command count changed");
    assert_eq!(
        rust_paths.len(),
        295,
        "Rust supported command count changed"
    );
    assert_eq!(
        go_paths.len() - rust_paths.len(),
        0,
        "Rust missing-command count changed"
    );
    let invented = rust_paths
        .difference(&go_paths)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        invented.is_empty(),
        "Rust capabilities must be a Go-oracle command subset; invented paths: {invented:?}"
    );
}

fn assert_no_duplicate_command_paths(capabilities: &Value, label: &str) {
    let commands = capabilities["commands"].as_array().expect("commands array");
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for command in commands {
        let path = command["path"].as_str().expect("command path").to_string();
        if !seen.insert(path.clone()) {
            duplicates.insert(path);
        }
    }
    assert!(
        duplicates.is_empty(),
        "{label} capabilities must not duplicate command paths: {duplicates:?}"
    );
}

fn command_by_path<'a>(capabilities: &'a Value, path: &str) -> &'a Value {
    let commands = capabilities["commands"].as_array().expect("commands array");
    commands
        .iter()
        .find(|command| command["path"].as_str() == Some(path))
        .unwrap_or_else(|| panic!("missing command {path}: {commands:?}"))
}

fn is_absent_or_empty_array(value: Option<&Value>) -> bool {
    value
        .map(|value| value.as_array().is_some_and(Vec::is_empty))
        .unwrap_or(true)
}

fn is_empty_array(value: &Value) -> bool {
    value.as_array().is_some_and(Vec::is_empty)
}

fn optional_array_is_empty(value: &Value, field: &str) -> bool {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|items| items.is_empty())
        .unwrap_or(true)
}
