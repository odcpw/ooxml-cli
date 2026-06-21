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

const RUST_ONLY_CAPABILITY_PATHS: &[&str] = &[
    "ooxml convert xlsm-to-xlsx",
    "ooxml docx scaffold",
    "ooxml pptx scaffold",
    "ooxml xlsx scaffold",
    "ooxml xlsx tables create",
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
    let missing = go_paths
        .difference(&rust_paths)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "Rust missing-command set changed: {missing:?}"
    );
    let allowed_rust_only = RUST_ONLY_CAPABILITY_PATHS
        .iter()
        .map(|path| (*path).to_string())
        .collect::<BTreeSet<_>>();
    let invented = rust_paths
        .difference(&go_paths)
        .filter(|path| !allowed_rust_only.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        invented.is_empty(),
        "Rust capabilities have unreviewed Rust-only paths: {invented:?}"
    );
    assert_eq!(
        rust_paths.len(),
        go_paths.len() + allowed_rust_only.len(),
        "Rust command count should equal Go oracle plus reviewed Rust-only features"
    );
}

#[test]
fn xlsx_conditional_format_reorder_capability_metadata() {
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");
    let reorder = command_by_path(&rust_caps, "ooxml xlsx conditional-formats reorder");

    assert_eq!(reorder["opCompatible"], Value::Bool(true));
    assert_eq!(
        reorder["use"],
        "reorder <file> --sheet <selector> --rule <selector> --priority <n>"
    );
    assert!(
        reorder["short"]
            .as_str()
            .expect("short")
            .contains("list rules first"),
        "reorder capability should include a recovery hint: {reorder:?}"
    );
    assert_eq!(
        local_flag_field(reorder, "name"),
        serde_json::json!([
            "--sheet",
            "--rule",
            "--priority",
            "--out",
            "--in-place",
            "--backup",
            "--dry-run",
            "--no-validate"
        ])
    );
}

#[test]
fn pptx_fields_set_capability_advertises_footer_synthesis() {
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");
    let fields_set = command_by_path(&rust_caps, "ooxml pptx fields set");

    assert!(
        fields_set["short"]
            .as_str()
            .expect("short")
            .contains("synthesizing missing footer placeholders"),
        "fields set capability should advertise footer placeholder synthesis: {fields_set:?}"
    );
    assert!(
        fields_set["opIneligibleReason"]
            .as_str()
            .expect("op-ineligible reason")
            .contains("synthesizes missing footer placeholders"),
        "fields set op-ineligible reason should advertise footer placeholder synthesis: {fields_set:?}"
    );
    assert!(
        !fields_set
            .to_string()
            .contains("reports missing placeholders instead of creating shapes"),
        "fields set capability must not advertise stale missing-placeholder behavior: {fields_set:?}"
    );
    assert_eq!(
        local_flag_field(fields_set, "description")[0],
        Value::String(
            "footer text; updates existing footer placeholders and creates missing visible footer placeholders"
                .to_string()
        )
    );
}

#[test]
fn artifact_proof_matrix_classifies_inventory_coverage() {
    let Some(powershell) = powershell_for_artifact_proof_matrix_test() else {
        eprintln!("skipping artifact proof matrix test because PowerShell is not available");
        return;
    };

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-artifact-proof-matrix-{}-{suffix}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("artifact proof matrix temp dir");

    let capabilities_path = temp_dir.join("capabilities.json");
    let evidence_path = temp_dir.join("evidence.json");
    let out_json = temp_dir.join("matrix.json");
    let out_markdown = temp_dir.join("matrix.md");

    let capabilities = serde_json::json!({
        "commands": [
            proof_matrix_capability_command(
                "ooxml xlsx cells set",
                "set <file> --sheet <selector> --cell <cell> --value <value>",
                &["--sheet", "--cell", "--value", "--out", "--in-place", "--dry-run"],
                true,
                &["cell"],
            ),
            proof_matrix_capability_command(
                "ooxml docx scaffold",
                "scaffold --out <file>",
                &["--out"],
                false,
                &["package"],
            ),
            proof_matrix_capability_command(
                "ooxml xlsx scaffold",
                "scaffold --out <file>",
                &["--out"],
                false,
                &["package", "sheet"],
            ),
            proof_matrix_capability_command(
                "ooxml xlsx comments add",
                "add <file> --sheet <selector> --cell <cell> --text <text>",
                &["--sheet", "--cell", "--text", "--out", "--in-place", "--dry-run"],
                true,
                &["comment"],
            ),
            proof_matrix_capability_command(
                "ooxml xlsx cells list",
                "list <file> --sheet <selector>",
                &["--sheet"],
                false,
                &["cell"],
            ),
        ]
    });
    let evidence = serde_json::json!({
        "proofs": [
            {
                "commandPath": "ooxml xlsx cells set",
                "generatedOutputPath": "proof-artifacts/cells-set.xlsx",
                "inputFixtureType": "scaffold-derived",
                "tiers": {
                    "structural": { "status": "passed" },
                    "readback": { "status": "passed" },
                    "validate": { "status": "passed" },
                    "conformance": { "status": "passed" },
                    "office": { "status": "passed" }
                }
            },
            {
                "commandPath": "ooxml docx scaffold",
                "tiers": {
                    "validate": { "status": "passed" },
                    "conformance": { "status": "passed" }
                }
            },
            {
                "commandPath": "ooxml xlsx scaffold",
                "tiers": {
                    "readback": { "status": "passed" }
                }
            }
        ]
    });
    fs::write(
        &capabilities_path,
        serde_json::to_vec_pretty(&capabilities).expect("capabilities JSON"),
    )
    .expect("write capabilities JSON");
    fs::write(
        &evidence_path,
        serde_json::to_vec_pretty(&evidence).expect("evidence JSON"),
    )
    .expect("write evidence JSON");

    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools")
        .join("artifact-proof-matrix.ps1");
    let output = Command::new(powershell)
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&script)
        .arg("-RepoRoot")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("-CapabilitiesJsonPath")
        .arg(&capabilities_path)
        .arg("-EvidencePath")
        .arg(&evidence_path)
        .arg("-OutJson")
        .arg(&out_json)
        .arg("-OutMarkdown")
        .arg(&out_markdown)
        .output()
        .expect("run artifact proof matrix");
    assert!(
        output.status.success(),
        "artifact proof matrix failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let matrix_text = fs::read_to_string(&out_json).expect("matrix JSON text");
    let matrix: Value =
        serde_json::from_str(matrix_text.trim_start_matches('\u{feff}')).expect("matrix JSON");
    assert_eq!(
        matrix["schemaVersion"],
        "ooxml-cli.artifact-proof-matrix.v2"
    );
    assert_eq!(matrix["summary"]["mutatingCommandCount"], 4);
    assert_eq!(matrix["summary"]["proofRowsPresent"], 3);
    assert_eq!(matrix["summary"]["commandsWithoutProofRows"], 1);
    assert_eq!(
        matrix["summary"]["commandsLackingStrictValidationConformanceOrOfficeOpenProof"],
        3
    );
    assert_eq!(
        matrix["summary"]["proofCoverageByClass"]["office-proven"],
        1
    );
    assert_eq!(
        matrix["summary"]["proofCoverageByClass"]["strict-conformance-proven"],
        1
    );
    assert_eq!(
        matrix["summary"]["proofCoverageByClass"]["partial-proof"],
        1
    );
    assert_eq!(
        matrix["summary"]["proofCoverageByClass"]["contract-only"],
        1
    );
    assert_eq!(matrix["summary"]["scaffoldDerivedProvenCommandCount"], 1);

    let cells_set = proof_matrix_row_by_path(&matrix, "ooxml xlsx cells set");
    assert_eq!(cells_set["inputFixtureType"], "scaffold-derived");
    assert_eq!(cells_set["requiredGaps"], serde_json::json!([]));
    let comments = proof_matrix_row_by_path(&matrix, "ooxml xlsx comments add");
    assert_eq!(comments["proofCoverage"], "contract-only");
    assert_eq!(comments["proofRowStatus"], "missing");
    assert_eq!(
        comments["strictProofGaps"],
        serde_json::json!(["validate", "conformance", "office"])
    );
    assert_eq!(
        comments["lacksStrictValidationConformanceOrOfficeOpenProof"],
        Value::Bool(true)
    );
    assert!(proof_matrix_row(&matrix, "ooxml xlsx cells list").is_none());

    let office_commands = matrix["questions"]["officeProvenCommands"]
        .as_array()
        .expect("office proven commands");
    assert!(office_commands.contains(&Value::String("ooxml xlsx cells set".to_string())));
    let scaffold_derived_commands = matrix["questions"]["scaffoldDerivedProvenCommands"]
        .as_array()
        .expect("scaffold-derived proven commands");
    assert!(scaffold_derived_commands.contains(&Value::String("ooxml xlsx cells set".to_string())));
    let missing_proof_rows = matrix["questions"]["commandsWithoutProofRows"]
        .as_array()
        .expect("commands without proof rows");
    assert!(missing_proof_rows.contains(&Value::String("ooxml xlsx comments add".to_string())));

    let markdown = fs::read_to_string(&out_markdown).expect("matrix markdown");
    assert!(markdown.contains("Commands with no proof row yet: 1"));
    assert!(markdown.contains("Scaffold-derived commands with complete proof: 1"));
    assert!(markdown.contains("| contract-only | 1 |"));

    let _ = fs::remove_dir_all(&temp_dir);
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

fn powershell_for_artifact_proof_matrix_test() -> Option<&'static str> {
    ["powershell.exe", "powershell", "pwsh"]
        .into_iter()
        .find(|candidate| {
            Command::new(candidate)
                .arg("-NoProfile")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-Command")
                .arg("$PSVersionTable.PSVersion.ToString()")
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        })
}

fn proof_matrix_capability_command(
    path: &str,
    use_text: &str,
    flags: &[&str],
    op_compatible: bool,
    target_object_kinds: &[&str],
) -> Value {
    serde_json::json!({
        "path": path,
        "use": use_text,
        "short": format!("Synthetic proof matrix command for {path}"),
        "opCompatible": op_compatible,
        "opIneligibleReason": if op_compatible {
            Value::Null
        } else {
            Value::String("synthetic read-only or generator command".to_string())
        },
        "localFlags": flags
            .iter()
            .map(|name| serde_json::json!({ "name": name }))
            .collect::<Vec<_>>(),
        "targetObjectKinds": target_object_kinds,
    })
}

fn proof_matrix_row_by_path<'a>(matrix: &'a Value, path: &str) -> &'a Value {
    proof_matrix_row(matrix, path).unwrap_or_else(|| panic!("missing matrix row {path}"))
}

fn proof_matrix_row<'a>(matrix: &'a Value, path: &str) -> Option<&'a Value> {
    matrix["rows"]
        .as_array()
        .expect("matrix rows")
        .iter()
        .find(|row| row["commandPath"].as_str() == Some(path))
}
