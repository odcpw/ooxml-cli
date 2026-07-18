// Capability inventory and filter contract tests live here so the parent
// integration test crate can keep the shared Rust-baseline helpers in one place.
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
    "ooxml agent-triage",
    "ooxml convert xlsm-to-xlsx",
    "ooxml docx scaffold",
    "ooxml docx tables create",
    "ooxml pptx scaffold",
    "ooxml repair normalize",
    "ooxml vba build-bin",
    "ooxml vba rebuild",
    "ooxml vba run-smoke",
    "ooxml xlsx conditional-formats",
    "ooxml xlsx conditional-formats add",
    "ooxml xlsx conditional-formats delete",
    "ooxml xlsx conditional-formats list",
    "ooxml xlsx conditional-formats reorder",
    "ooxml xlsx conditional-formats show",
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
    "ooxml xlsx data-validations",
    "ooxml xlsx filters-sorts",
    "ooxml xlsx forms",
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
fn object_kinds_index_matches_command_target_object_kinds() {
    let (all_code, all_stdout, all_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(all_code, 0);
    assert_eq!(all_stderr, None);
    let all_caps = all_stdout.expect("all capabilities");
    assert_object_kinds_index_matches_commands(&all_caps);

    let (package_code, package_stdout, package_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "package"]);
    assert_eq!(package_code, 0);
    assert_eq!(package_stderr, None);
    let package_caps = package_stdout.expect("package capabilities");
    assert_object_kinds_index_matches_commands(&package_caps);
}

#[test]
fn capabilities_schema_shape_is_stable_for_typed_builder() {
    let (all_code, all_stdout, all_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(all_code, 0);
    assert_eq!(all_stderr, None);
    let all_caps = all_stdout.expect("all capabilities");
    assert!(
        all_caps.get("filter").is_none(),
        "unfiltered capabilities should omit filter, not emit null"
    );

    for flag in all_caps["globalFlags"]
        .as_array()
        .expect("globalFlags array")
    {
        assert!(
            flag["default"].is_string(),
            "global flag default stays string-typed for {}",
            flag["name"]
        );
    }

    for alias in all_caps["filterAliases"]
        .as_array()
        .expect("filterAliases array")
    {
        assert_eq!(
            json_object_keys(alias),
            BTreeSet::from(["alias".to_string(), "canonical".to_string()])
        );
        assert!(alias["alias"].is_string(), "alias should be string");
        assert!(alias["canonical"].is_string(), "canonical should be string");
    }

    let allowed_command_keys = BTreeSet::from([
        "flagConstraints".to_string(),
        "localFlags".to_string(),
        "opCompatible".to_string(),
        "opIneligibleReason".to_string(),
        "path".to_string(),
        "short".to_string(),
        "targetObjectKinds".to_string(),
        "use".to_string(),
    ]);
    let required_command_keys = BTreeSet::from([
        "localFlags".to_string(),
        "opCompatible".to_string(),
        "path".to_string(),
        "short".to_string(),
        "targetObjectKinds".to_string(),
        "use".to_string(),
    ]);
    let expected_flag_keys = BTreeSet::from([
        "argName".to_string(),
        "description".to_string(),
        "name".to_string(),
        "type".to_string(),
    ]);

    for command in all_caps["commands"].as_array().expect("commands array") {
        let path = command["path"].as_str().expect("command path");
        let command_keys = json_object_keys(command);
        assert!(
            command_keys.is_subset(&allowed_command_keys),
            "unexpected command keys for {path}: {command_keys:?}"
        );
        assert!(
            required_command_keys.is_subset(&command_keys),
            "missing required command keys for {path}: {command_keys:?}"
        );
        assert!(command["use"].is_string(), "use for {path}");
        assert!(command["short"].is_string(), "short for {path}");
        assert!(
            command["targetObjectKinds"].is_array(),
            "targetObjectKinds for {path}"
        );
        assert!(command["localFlags"].is_array(), "localFlags for {path}");
        assert!(
            command["opCompatible"].is_boolean(),
            "opCompatible for {path}"
        );
        if let Some(reason) = command.get("opIneligibleReason") {
            assert!(
                reason.is_string(),
                "opIneligibleReason should be omitted or string for {path}"
            );
        }
        if let Some(flag_constraints) = command.get("flagConstraints") {
            assert!(
                flag_constraints.is_object(),
                "flagConstraints should be omitted or object for {path}"
            );
        }
        for flag in command["localFlags"].as_array().expect("localFlags array") {
            assert_eq!(
                json_object_keys(flag),
                expected_flag_keys,
                "flag keys for {path}"
            );
            for field in ["argName", "description", "name", "type"] {
                assert!(
                    flag[field].is_string(),
                    "{field} should be string for {path}"
                );
            }
        }
    }
}

#[test]
fn capabilities_filtered_empty_schema_keeps_index_and_suggestions() {
    let (code, stdout, stderr) = run_ooxml(&["--json", "capabilities", "--for", "slidez"]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let caps = stdout.expect("unknown-filter capabilities");
    assert_eq!(
        caps["commands"].as_array().expect("commands array").len(),
        0
    );
    assert_eq!(
        caps["filter"]["requested"],
        Value::String("slidez".to_string())
    );
    assert_eq!(
        caps["filter"]["normalized"],
        Value::String("slidez".to_string())
    );
    assert!(
        !caps["filter"]["suggestions"]
            .as_array()
            .expect("filter suggestions")
            .is_empty(),
        "empty filters should teach likely alternatives"
    );
    let index = caps["objectKindsIndex"]
        .as_object()
        .expect("objectKindsIndex object");
    let object_kinds = caps["objectKinds"].as_array().expect("objectKinds array");
    assert_eq!(index.len(), object_kinds.len(), "index key count");
    for kind in object_kinds {
        let kind = kind.as_str().expect("object kind string");
        assert!(
            index[kind].as_array().expect("index array").is_empty(),
            "filtered-empty index should keep empty array for {kind}"
        );
    }
}

#[test]
fn capabilities_accepts_command_local_strict_global_flag() {
    let (all_code, all_stdout, all_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(all_code, 0);
    assert_eq!(all_stderr, None);
    let all_caps = all_stdout.expect("all capabilities");

    let (strict_code, strict_stdout, strict_stderr) =
        run_ooxml(&["--json", "capabilities", "--strict"]);
    assert_eq!(strict_code, 0);
    assert_eq!(strict_stderr, None);
    let strict_caps = strict_stdout.expect("strict capabilities");
    assert_eq!(
        strict_caps["commands"]
            .as_array()
            .expect("strict commands")
            .len(),
        all_caps["commands"].as_array().expect("all commands").len()
    );
}

#[test]
fn discovery_alias_shapes_stay_surface_specific() {
    let (caps_code, caps_stdout, caps_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(caps_code, 0);
    assert_eq!(caps_stderr, None);
    let caps = caps_stdout.expect("capabilities");
    assert!(
        caps["filterAliases"]
            .as_array()
            .expect("capabilities filterAliases")
            .iter()
            .all(|alias| alias["alias"].is_string() && alias["canonical"].is_string()),
        "capabilities filterAliases should be object records"
    );

    let (triage_code, triage_stdout, triage_stderr) = run_ooxml(&["--json", "agent-triage"]);
    assert_eq!(triage_code, 0);
    assert_eq!(triage_stderr, None);
    let triage = triage_stdout.expect("agent-triage");
    assert!(
        triage["discovery"]["filterAliases"]
            .as_array()
            .expect("triage discovery filterAliases")
            .iter()
            .all(Value::is_string),
        "agent-triage filterAliases should stay compact strings"
    );
}

#[test]
fn docx_parent_group_capabilities_match_rust_baseline() {
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&["--json", "capabilities"]);
    assert_eq!(baseline_code, 0);
    assert_eq!(baseline_stderr, None);
    let baseline_caps = baseline_stdout.expect("baseline capabilities");

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    for path in DOCX_PARENT_GROUP_COMMANDS {
        let baseline_command = command_by_path(&baseline_caps, path);
        let rust_command = command_by_path(&rust_caps, path);
        for field in ["path", "use", "short", "opCompatible", "opIneligibleReason"] {
            assert_eq!(
                rust_command[field], baseline_command[field],
                "{field} for {path}"
            );
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
fn xlsx_parent_group_capabilities_match_rust_baseline() {
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&["--json", "capabilities"]);
    assert_eq!(baseline_code, 0);
    assert_eq!(baseline_stderr, None);
    let baseline_caps = baseline_stdout.expect("baseline capabilities");

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    for path in XLSX_PARENT_GROUP_PATHS {
        let baseline_command = command_by_path(&baseline_caps, path);
        let rust_command = command_by_path(&rust_caps, path);
        for field in ["use", "short", "opCompatible", "opIneligibleReason"] {
            assert_eq!(
                rust_command[field], baseline_command[field],
                "{field} for {path}"
            );
        }
        assert!(is_absent_or_empty_array(
            baseline_command.get("targetObjectKinds")
        ));
        assert!(is_absent_or_empty_array(baseline_command.get("localFlags")));
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
fn pptx_parent_group_capabilities_match_rust_baseline_metadata() {
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&["--json", "capabilities"]);
    assert_eq!(baseline_code, 0);
    assert_eq!(baseline_stderr, None);
    let baseline_caps = baseline_stdout.expect("baseline capabilities");

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    for path in PPTX_PARENT_GROUP_CAPABILITY_PATHS {
        let baseline_command = command_by_path(&baseline_caps, path);
        let rust_command = command_by_path(&rust_caps, path);

        assert_eq!(
            baseline_command["opCompatible"],
            Value::Bool(false),
            "Rust baseline opCompatible for {path}"
        );
        assert_eq!(
            rust_command["opCompatible"],
            Value::Bool(false),
            "Rust opCompatible for {path}"
        );
        assert_eq!(
            rust_command["use"], baseline_command["use"],
            "use for {path}"
        );
        assert_eq!(
            rust_command["short"], baseline_command["short"],
            "short for {path}"
        );
        for field in ["targetObjectKinds", "localFlags"] {
            assert!(
                optional_array_is_empty(baseline_command, field),
                "Rust baseline {field} should be absent or empty for {path}: {}",
                baseline_command[field]
            );
            assert!(
                optional_array_is_empty(rust_command, field),
                "Rust {field} should be absent or empty for {path}: {}",
                rust_command[field]
            );
        }
        assert_eq!(
            rust_command["opIneligibleReason"], baseline_command["opIneligibleReason"],
            "opIneligibleReason for {path}"
        );
    }

    let baseline_diff = command_by_path(&baseline_caps, "ooxml pptx diff");
    let rust_diff = command_by_path(&rust_caps, "ooxml pptx diff");
    assert_eq!(baseline_diff["opCompatible"], Value::Bool(false));
    assert_eq!(
        baseline_diff["use"],
        Value::String("diff <baseline> <candidate>".to_string())
    );
    assert_eq!(
        rust_diff["use"], baseline_diff["use"],
        "use for ooxml pptx diff"
    );
    assert_eq!(
        rust_diff["short"], baseline_diff["short"],
        "short for ooxml pptx diff"
    );
    assert_eq!(
        rust_diff["opCompatible"], baseline_diff["opCompatible"],
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
fn rust_capability_inventory_is_rust_baseline_subset() {
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&["--json", "capabilities"]);
    assert_eq!(baseline_code, 0);
    assert_eq!(baseline_stderr, None);
    let baseline_caps = baseline_stdout.expect("baseline capabilities");

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    assert_no_duplicate_command_paths(&baseline_caps, "Rust baseline");
    assert_no_duplicate_command_paths(&rust_caps, "Rust");

    let baseline_paths = capability_paths(&baseline_caps);
    let rust_paths = capability_paths(&rust_caps);
    let missing = baseline_paths
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
        .difference(&baseline_paths)
        .filter(|path| !allowed_rust_only.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        invented.is_empty(),
        "Rust capabilities have unreviewed Rust-only paths: {invented:?}"
    );
    assert!(
        rust_paths.len() <= baseline_paths.len() + allowed_rust_only.len(),
        "Rust command count exceeds baseline plus reviewed Rust-only features: baseline={}, rust={}, allowed={}",
        baseline_paths.len(),
        rust_paths.len(),
        allowed_rust_only.len()
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
fn xlsx_conditional_format_add_capability_advertises_rule_constraints() {
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");
    let add = command_by_path(&rust_caps, "ooxml xlsx conditional-formats add");
    let constraints = &add["flagConstraints"];

    assert_eq!(constraints["modeFlag"], "--type");
    assert_eq!(constraints["defaultMode"], "expression");
    let modes = constraints["modes"].as_array().expect("constraint modes");
    assert!(
        modes.iter().any(|mode| {
            mode["value"] == "expression"
                && mode["required"] == serde_json::json!(["--range", "--formula"])
        }),
        "expression mode should advertise required authoring flags: {constraints:?}"
    );
    assert!(
        modes
            .iter()
            .any(|mode| { mode["value"] == "color-scale" && mode["repeat"]["--cfvo"] == "2 or 3" }),
        "color-scale mode should advertise cfvo repeat count: {constraints:?}"
    );
    assert!(
        modes.iter().any(|mode| {
            mode["value"] == "icon-set"
                && mode["forbidden"]
                    .as_array()
                    .expect("icon-set forbidden flags")
                    .contains(&Value::String("--color".to_string()))
        }),
        "icon-set mode should forbid color flags: {constraints:?}"
    );
}

#[test]
fn xlsx_data_validation_create_capability_advertises_type_constraints() {
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");
    let create = command_by_path(&rust_caps, "ooxml xlsx data-validations create");
    let constraints = &create["flagConstraints"];

    assert_eq!(constraints["modeFlag"], "--type");
    assert_eq!(
        constraints["outputRequiredOneOf"],
        serde_json::json!(["--out", "--in-place", "--dry-run"])
    );
    let modes = constraints["modes"].as_array().expect("constraint modes");
    assert!(
        modes.iter().any(|mode| {
            mode["value"] == "list"
                && mode["oneOf"] == serde_json::json!(["--list-values", "--list-range"])
                && mode["forbidden"]
                    .as_array()
                    .expect("list forbidden flags")
                    .contains(&Value::String("--operator".to_string()))
        }),
        "list mode should advertise its required source and forbidden flags: {constraints:?}"
    );
    assert!(
        modes.iter().any(|mode| {
            mode["value"] == "textLength"
                && mode["aliases"]
                    .as_array()
                    .expect("textLength aliases")
                    .contains(&Value::String("text-length".to_string()))
        }),
        "textLength mode should advertise accepted CLI aliases: {constraints:?}"
    );
    assert!(
        constraints["rules"]
            .as_array()
            .expect("constraint rules")
            .iter()
            .any(|rule| rule.as_str().unwrap_or_default().contains("between")),
        "data-validation constraints should explain formula2 for between operators: {constraints:?}"
    );
}

#[test]
fn chart_create_capabilities_advertise_source_constraints() {
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    let xlsx = command_by_path(&rust_caps, "ooxml xlsx charts create");
    let xlsx_constraints = &xlsx["flagConstraints"];
    assert_eq!(xlsx_constraints["modeFlag"], "--type");
    let xlsx_sources = xlsx_constraints["sourceModes"]
        .as_array()
        .expect("xlsx chart source modes");
    assert!(
        xlsx_sources.iter().any(|mode| {
            mode["name"] == "range"
                && mode["required"] == serde_json::json!(["--sheet", "--range"])
                && mode["conflictsWith"] == serde_json::json!(["--table"])
        }),
        "xlsx chart create should advertise range source constraints: {xlsx_constraints:?}"
    );
    assert!(
        xlsx_sources.iter().any(|mode| {
            mode["name"] == "table"
                && mode["required"] == serde_json::json!(["--table"])
                && mode["conflictsWith"] == serde_json::json!(["--range"])
        }),
        "xlsx chart create should advertise table source constraints: {xlsx_constraints:?}"
    );

    let pptx = command_by_path(&rust_caps, "ooxml pptx charts create");
    let pptx_constraints = &pptx["flagConstraints"];
    let pptx_sources = pptx_constraints["sourceModes"]
        .as_array()
        .expect("pptx chart source modes");
    assert!(
        pptx_sources.iter().any(|mode| {
            mode["name"] == "inline-json"
                && mode["required"] == serde_json::json!(["--values-json"])
                && mode["conflictsWith"]
                    .as_array()
                    .expect("inline-json conflicts")
                    .contains(&Value::String("--source-file".to_string()))
        }),
        "pptx chart create should advertise inline JSON source constraints: {pptx_constraints:?}"
    );
    assert!(
        pptx_sources.iter().any(|mode| {
            mode["name"] == "external-xlsx"
                && mode["required"] == serde_json::json!(["--source-file", "--source-range"])
                && mode["optional"]
                    .as_array()
                    .expect("external optional flags")
                    .contains(&Value::String("--embed-workbook".to_string()))
        }),
        "pptx chart create should advertise external workbook source constraints: {pptx_constraints:?}"
    );
}

#[test]
fn vba_create_capability_advertises_mode_constraints() {
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");
    let create = command_by_path(&rust_caps, "ooxml vba create");
    let constraints = &create["flagConstraints"];

    let modes = constraints["modes"].as_array().expect("constraint modes");
    let pure = modes
        .iter()
        .find(|mode| mode["name"] == "pure")
        .expect("pure mode constraints");
    assert!(
        local_flag_field(create, "name")
            .as_array()
            .expect("vba create local flag names")
            .contains(&Value::String("--dry-run".to_string())),
        "vba create should advertise --dry-run because pure mode accepts it: {create:?}"
    );
    assert!(
        pure["allowedFlags"]
            .as_array()
            .expect("pure allowed flags")
            .contains(&Value::String("--source".to_string()))
    );
    assert!(
        pure["allowedFlags"]
            .as_array()
            .expect("pure allowed flags")
            .contains(&Value::String("--dry-run".to_string()))
    );
    assert!(
        pure["conflictsWith"]
            .as_array()
            .expect("pure conflict flags")
            .contains(&Value::String("--office-create-script".to_string()))
    );

    let legacy = modes
        .iter()
        .find(|mode| mode["name"] == "legacy-office-com")
        .expect("legacy mode constraints");
    assert!(
        legacy["allowedFlags"]
            .as_array()
            .expect("legacy allowed flags")
            .contains(&Value::String("--office-create-script".to_string()))
    );
    assert!(
        legacy["conflictsWith"]
            .as_array()
            .expect("legacy conflict flags")
            .contains(&Value::String("--out".to_string()))
    );
    assert!(
        constraints["rules"]
            .as_array()
            .expect("constraint rules")
            .iter()
            .any(|rule| rule.as_str().unwrap_or_default().contains("--pure cannot")),
        "vba create should advertise pure/legacy conflict rules: {constraints:?}"
    );
}

#[test]
fn vba_source_capabilities_scope_minimal_userforms_honestly() {
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "vba"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("Rust VBA capabilities");

    for path in [
        "ooxml vba build-bin",
        "ooxml vba rebuild",
        "ooxml vba extract",
    ] {
        let command = command_by_path(&rust_caps, path);
        let contract = serde_json::to_string(command)
            .expect("serialize VBA capability")
            .to_ascii_lowercase();
        for required in [".frm", "xlsm", "not runtime-loadable", "pptm/docm"] {
            assert!(
                contract.contains(required),
                "{path} should scope minimal UserForm support with {required:?}: {command:?}"
            );
        }
    }
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
fn xlsx_matrix_data_format_help_explains_typed_json_and_text_delimited_values() {
    let (code, stdout, stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let capabilities = stdout.expect("capabilities");
    for path in ["ooxml xlsx ranges set", "ooxml xlsx tables append-rows"] {
        let command = command_by_path(&capabilities, path);
        let data_format = command["localFlags"]
            .as_array()
            .expect("local flags")
            .iter()
            .find(|flag| flag["name"] == "--data-format")
            .expect("data-format flag");
        assert_eq!(
            data_format["description"],
            "matrix format: JSON preserves typed numbers and booleans; CSV/TSV values are imported as text",
            "{path} should make type preservation explicit"
        );
    }
}

#[test]
fn artifact_proof_matrix_classifies_inventory_coverage() {
    let Some(powershell) = powershell_for_windows_contract_test() else {
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
    let office_edit_summary_path = temp_dir.join("office-edit-smoke-summary.json");
    let oracle_evidence_path = temp_dir.join("office-oracle-evidence.json");
    let oracle_summary_path = temp_dir.join("office-oracle-summary.json");
    let out_json = temp_dir.join("matrix.json");
    let out_markdown = temp_dir.join("matrix.md");
    let oracle_vba_artifact = temp_dir.join("oracle").join("vba-attached.xlsm");
    let oracle_template_artifact = temp_dir.join("oracle").join("template-compiled.pptx");

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
                "ooxml vba attach",
                "attach <file> --bin <vbaProject.bin> --out <file>",
                &["--bin", "--out"],
                true,
                &["vba", "module"],
            ),
            proof_matrix_capability_command(
                "ooxml convert xlsm-to-xlsx",
                "xlsm-to-xlsx <input.xlsm> --out <output.xlsx>",
                &["--out"],
                true,
                &["package", "vba"],
            ),
            proof_matrix_capability_command(
                "ooxml pptx template compile",
                "compile <spec> --out <file>",
                &["--out"],
                false,
                &["template", "slide"],
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
            },
            {
                "commandPath": "ooxml convert xlsm-to-xlsx",
                "generatedOutputPath": "proof-artifacts/converted-alias.xlsx",
                "inputFixtureType": "office-authored macro package",
                "tiers": {
                    "structural": { "status": "passed" },
                    "readback": { "status": "passed" },
                    "validate": { "status": "passed" },
                    "conformance": { "status": "passed" },
                    "office": { "status": "passed" }
                }
            }
        ]
    });
    let oracle_evidence = serde_json::json!({
        "officeOracleProofs": [
            {
                "commandPath": "ooxml vba attach",
                "artifact": oracle_vba_artifact,
                "inputFixtureType": "realistic fixture"
            },
            {
                "commandPath": "ooxml pptx template compile",
                "generatedOutputPath": oracle_template_artifact,
                "inputFixtureType": "template manifest/spec"
            }
        ]
    });
    let oracle_summary = serde_json::json!([
        {
            "timestampUtc": "2026-06-20T18:26:56.6796007Z",
            "file": oracle_vba_artifact,
            "family": "xlsx",
            "officeApplication": "Excel",
            "officeVersion": "16.0",
            "officeBuild": "20026",
            "status": "passed",
            "visible": false,
            "elapsedMs": 5175,
            "errorType": "",
            "errorMessage": ""
        },
        {
            "timestampUtc": "2026-06-20T19:58:13.1570108Z",
            "file": oracle_template_artifact,
            "family": "pptx",
            "officeApplication": "PowerPoint",
            "officeVersion": "16.0",
            "officeBuild": "20026",
            "status": "passed",
            "visible": false,
            "elapsedMs": 4880,
            "errorType": "",
            "errorMessage": ""
        }
    ]);
    let office_edit_summary = serde_json::json!({
        "schemaVersion": "ooxml-cli.office-edit-smoke.v1",
        "scenarios": [
            {
                "name": "xlsx-scaffold-explicit-command-path",
                "commandPath": "ooxml xlsx scaffold",
                "family": "xlsx",
                "inputFixtureType": "scaffold",
                "output": "proof-artifacts/xlsx-scaffold.xlsx",
                "mutation": {
                    "status": "passed",
                    "command": "smoke summary carried explicit commandPath"
                },
                "readback": {
                    "status": "passed",
                    "command": "ooxml --json inspect proof-artifacts/xlsx-scaffold.xlsx",
                    "artifact": "proof-artifacts/xlsx-scaffold.xlsx"
                },
                "openXmlSdk": {
                    "status": "passed",
                    "command": "dotnet run --project tools/openxml-validator proof-artifacts/xlsx-scaffold.xlsx",
                    "artifact": "proof-artifacts/xlsx-scaffold.xlsx"
                },
                "validation": {
                    "status": "passed",
                    "command": "ooxml --json validate --strict proof-artifacts/xlsx-scaffold.xlsx",
                    "artifact": "proof-artifacts/xlsx-scaffold.xlsx"
                },
                "conformance": {
                    "status": "passed",
                    "command": "ooxml --json conformance check proof-artifacts/xlsx-scaffold.xlsx",
                    "artifact": "proof-artifacts/xlsx-scaffold.xlsx"
                },
                "microsoftOffice": {
                    "status": "passed",
                    "detail": "Excel opened the scaffold without repair.",
                    "artifact": "proof-artifacts/xlsx-scaffold.xlsx"
                }
            },
            {
                "name": "xlsx-comments-add",
                "family": "xlsx",
                "inputFixtureType": "scaffold-derived",
                "output": "proof-artifacts/xlsx-comments-add.xlsx",
                "mutation": {
                    "status": "passed",
                    "command": "ooxml --json xlsx comments add input.xlsx --sheet Sheet1 --cell A1 --text Note --out proof-artifacts/xlsx-comments-add.xlsx"
                },
                "readback": {
                    "status": "passed",
                    "command": "ooxml --json inspect proof-artifacts/xlsx-comments-add.xlsx",
                    "artifact": "proof-artifacts/xlsx-comments-add.xlsx"
                },
                "openXmlSdk": {
                    "status": "passed",
                    "command": "dotnet run --project tools/openxml-validator proof-artifacts/xlsx-comments-add.xlsx",
                    "artifact": "proof-artifacts/xlsx-comments-add.xlsx"
                },
                "validation": {
                    "status": "passed",
                    "command": "ooxml --json validate --strict proof-artifacts/xlsx-comments-add.xlsx",
                    "artifact": "proof-artifacts/xlsx-comments-add.xlsx"
                },
                "conformance": {
                    "status": "passed",
                    "command": "ooxml --json conformance check proof-artifacts/xlsx-comments-add.xlsx",
                    "artifact": "proof-artifacts/xlsx-comments-add.xlsx"
                },
                "microsoftOffice": {
                    "status": "passed",
                    "detail": "Excel opened the file without repair.",
                    "artifact": "proof-artifacts/xlsx-comments-add.xlsx"
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
    fs::write(
        &office_edit_summary_path,
        serde_json::to_vec_pretty(&office_edit_summary).expect("office edit summary JSON"),
    )
    .expect("write office edit summary JSON");
    fs::write(
        &oracle_evidence_path,
        serde_json::to_vec_pretty(&oracle_evidence).expect("oracle evidence JSON"),
    )
    .expect("write oracle evidence JSON");
    fs::write(
        &oracle_summary_path,
        serde_json::to_vec_pretty(&oracle_summary).expect("oracle summary JSON"),
    )
    .expect("write oracle summary JSON");

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
        .arg("-OfficeEditSmokeSummaryPath")
        .arg(&office_edit_summary_path)
        .arg("-OfficeOracleSummaryPath")
        .arg(&oracle_summary_path)
        .arg("-OfficeOracleEvidencePath")
        .arg(&oracle_evidence_path)
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
    assert_eq!(matrix["summary"]["mutatingCommandCount"], 7);
    assert_eq!(matrix["summary"]["proofRowsPresent"], 7);
    assert_eq!(matrix["summary"]["commandsWithoutProofRows"], 0);
    assert_eq!(
        matrix["summary"]["commandsLackingStrictValidationConformanceOrOfficeOpenProof"],
        3
    );
    assert_eq!(
        matrix["summary"]["proofCoverageByClass"]["office-proven"],
        6
    );
    assert_eq!(
        matrix["summary"]["proofCoverageByClass"]["strict-conformance-proven"],
        1
    );
    assert_eq!(
        matrix["summary"]["proofCoverageByClass"]["partial-proof"],
        0
    );
    assert_eq!(
        matrix["summary"]["proofCoverageByClass"]["contract-only"],
        0
    );
    assert_eq!(matrix["summary"]["scaffoldDerivedProvenCommandCount"], 2);
    assert_eq!(matrix["summary"]["officeEditSmokeEvidenceCommandCount"], 2);
    assert_eq!(matrix["summary"]["officeOracleEvidenceCommandCount"], 2);

    let cells_set = proof_matrix_row_by_path(&matrix, "ooxml xlsx cells set");
    assert_eq!(cells_set["inputFixtureType"], "scaffold-derived");
    assert_eq!(cells_set["requiredGaps"], serde_json::json!([]));
    let xlsx_scaffold = proof_matrix_row_by_path(&matrix, "ooxml xlsx scaffold");
    assert_eq!(xlsx_scaffold["proofCoverage"], "office-proven");
    assert_eq!(xlsx_scaffold["proofRowStatus"], "present");
    assert_eq!(xlsx_scaffold["inputFixtureType"], "scaffold");
    assert_eq!(xlsx_scaffold["requiredGaps"], serde_json::json!([]));
    assert_eq!(xlsx_scaffold["tiers"]["office"]["status"], "passed");
    let vba_attach = proof_matrix_row_by_path(&matrix, "ooxml vba attach");
    assert_eq!(vba_attach["proofCoverage"], "office-proven");
    assert_eq!(vba_attach["proofRowStatus"], "present");
    assert_eq!(vba_attach["tiers"]["office"]["status"], "passed");
    assert_eq!(
        vba_attach["strictProofGaps"],
        serde_json::json!(["validate", "conformance"])
    );
    let converted = proof_matrix_row_by_path(&matrix, "ooxml convert xlsm-to-xlsx");
    assert_eq!(converted["proofCoverage"], "office-proven");
    assert_eq!(converted["proofRowStatus"], "present");
    assert_eq!(
        converted["inputFixtureType"],
        "office-authored macro package"
    );
    assert_eq!(converted["requiredGaps"], serde_json::json!([]));
    assert_eq!(converted["strictProofGaps"], serde_json::json!([]));
    assert_eq!(converted["tiers"]["validate"]["status"], "passed");
    assert_eq!(converted["tiers"]["conformance"]["status"], "passed");
    assert_eq!(converted["tiers"]["office"]["status"], "passed");
    let template_compile = proof_matrix_row_by_path(&matrix, "ooxml pptx template compile");
    assert_eq!(
        template_compile["inputFixtureType"],
        "template manifest/spec"
    );
    assert_eq!(template_compile["tiers"]["office"]["status"], "passed");
    let comments = proof_matrix_row_by_path(&matrix, "ooxml xlsx comments add");
    assert_eq!(comments["proofCoverage"], "office-proven");
    assert_eq!(comments["proofRowStatus"], "present");
    assert_eq!(comments["inputFixtureType"], "scaffold-derived");
    assert_eq!(comments["requiredGaps"], serde_json::json!([]));
    assert_eq!(comments["strictProofGaps"], serde_json::json!([]));
    assert_eq!(comments["tiers"]["structural"]["status"], "passed");
    assert_eq!(comments["tiers"]["readback"]["status"], "passed");
    assert_eq!(comments["tiers"]["validate"]["status"], "passed");
    assert_eq!(comments["tiers"]["conformance"]["status"], "passed");
    assert_eq!(comments["tiers"]["office"]["status"], "passed");
    assert_eq!(
        comments["lacksStrictValidationConformanceOrOfficeOpenProof"],
        Value::Bool(false)
    );
    assert!(proof_matrix_row(&matrix, "ooxml xlsx cells list").is_none());

    let office_commands = matrix["questions"]["officeProvenCommands"]
        .as_array()
        .expect("office proven commands");
    assert!(office_commands.contains(&Value::String("ooxml xlsx cells set".to_string())));
    assert!(office_commands.contains(&Value::String("ooxml xlsx comments add".to_string())));
    assert!(office_commands.contains(&Value::String("ooxml convert xlsm-to-xlsx".to_string())));
    assert!(office_commands.contains(&Value::String("ooxml vba attach".to_string())));
    assert!(office_commands.contains(&Value::String("ooxml pptx template compile".to_string())));
    let scaffold_derived_commands = matrix["questions"]["scaffoldDerivedProvenCommands"]
        .as_array()
        .expect("scaffold-derived proven commands");
    assert!(scaffold_derived_commands.contains(&Value::String("ooxml xlsx cells set".to_string())));
    assert!(
        scaffold_derived_commands.contains(&Value::String("ooxml xlsx comments add".to_string()))
    );
    let missing_proof_rows = matrix["questions"]["commandsWithoutProofRows"]
        .as_array()
        .expect("commands without proof rows");
    assert!(missing_proof_rows.is_empty());

    let markdown = fs::read_to_string(&out_markdown).expect("matrix markdown");
    assert!(markdown.contains("Commands with no proof row yet: 0"));
    assert!(markdown.contains("Scaffold-derived commands with complete proof: 2"));
    assert!(markdown.contains("| contract-only | 0 |"));
    assert!(markdown.contains("| office-proven | 6 |"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn office_edit_smoke_summary_command_paths_match_capability_keys() {
    let Some(powershell) = powershell_for_windows_contract_test() else {
        eprintln!(
            "skipping Office edit smoke commandPath test because PowerShell is not available"
        );
        return;
    };

    let script_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools")
        .join("windows-office-edit-smoke.ps1");
    let script = fs::read_to_string(&script_path).expect("read Office edit smoke script");
    let helpers = powershell_function_block(
        &script,
        "function Test-ScenarioCommandPathStopArgument",
        "function Invoke-Checked",
    );
    let probe = format!(
        r#"
{helpers}
$ErrorActionPreference = "Stop"
$cases = @(
    [pscustomobject]@{{ Name = "xlsx scaffold"; Arguments = @("--json", "xlsx", "scaffold", "C:\tmp\scaffold.xlsx", "--sheet", "OfficeScaffold"); Expected = "ooxml xlsx scaffold" }},
    [pscustomobject]@{{ Name = "xlsx names add"; Arguments = @("--json", "--strict", "xlsx", "names", "add", "C:\tmp\input.xlsx", "--name", "OfficeSmokeRange", "--out", "C:\tmp\output.xlsx"); Expected = "ooxml xlsx names add" }},
    [pscustomobject]@{{ Name = "xlsx conditional formats add"; Arguments = @("--json", "xlsx", "conditional-formats", "add", "C:\tmp\input.xlsx", "--sheet", "1", "--range", "E2:E5", "--out", "C:\tmp\output.xlsx"); Expected = "ooxml xlsx conditional-formats add" }},
    [pscustomobject]@{{ Name = "xlsx pivots create"; Arguments = @("--json", "xlsx", "pivots", "create", "C:\tmp\input.xlsx", "--sheet", "1", "--range", "A1:C5", "--out", "C:\tmp\output.xlsx"); Expected = "ooxml xlsx pivots create" }},
    [pscustomobject]@{{ Name = "docx scaffold"; Arguments = @("--json", "docx", "scaffold", "C:\tmp\scaffold.docx", "--text", "Office scaffold"); Expected = "ooxml docx scaffold" }},
    [pscustomobject]@{{ Name = "docx tables set-cell"; Arguments = @("--json", "docx", "tables", "set-cell", "C:\tmp\input.docx", "--table", "1", "--row", "1", "--col", "2", "--out", "C:\tmp\output.docx"); Expected = "ooxml docx tables set-cell" }},
    [pscustomobject]@{{ Name = "pptx replace text occurrences"; Arguments = @("--json", "pptx", "replace", "text-occurrences", "C:\tmp\input.pptx", "--match-text", "Minimal Title Slide", "--out", "C:\tmp\output.pptx"); Expected = "ooxml pptx replace text-occurrences" }},
    [pscustomobject]@{{ Name = "pptx place table-from-xlsx"; Arguments = @("--json", "pptx", "place", "table-from-xlsx", "C:\tmp\input.pptx", "--workbook", "C:\tmp\source.xlsx", "--out", "C:\tmp\output.pptx"); Expected = "ooxml pptx place table-from-xlsx" }},
    [pscustomobject]@{{ Name = "pptx new slide from layout"; Arguments = @("--json", "pptx", "new-slide-from-layout", "C:\tmp\input.pptx", "--layout", "9", "--out", "C:\tmp\output.pptx"); Expected = "ooxml pptx new-slide-from-layout" }}
)
$rows = foreach ($case in $cases) {{
    $actual = Get-ScenarioCommandPath -Arguments $case.Arguments
    [pscustomobject]@{{
        name = $case.Name
        expected = $case.Expected
        actual = $actual
        passed = ($actual -eq $case.Expected)
    }}
}}
$rows | ConvertTo-Json -Depth 6
if (@($rows | Where-Object {{ -not $_.passed }}).Count -gt 0) {{
    exit 1
}}
"#
    );

    let output = Command::new(powershell)
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(probe)
        .output()
        .expect("run Office edit smoke commandPath probe");
    assert!(
        output.status.success(),
        "Office edit smoke commandPath probe failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let rows: Value = serde_json::from_str(
        String::from_utf8_lossy(&output.stdout)
            .trim_start_matches('\u{feff}')
            .trim(),
    )
    .expect("commandPath probe JSON");
    let rows = rows.as_array().expect("commandPath rows");
    assert_eq!(rows.len(), 9);
    assert!(
        rows.iter()
            .all(|row| row["actual"] == row["expected"] && row["passed"] == Value::Bool(true)),
        "unexpected commandPath probe rows: {rows:#?}"
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

fn json_object_keys(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("JSON object")
        .keys()
        .cloned()
        .collect()
}

fn assert_object_kinds_index_matches_commands(capabilities: &Value) {
    let mut expected = capabilities["objectKinds"]
        .as_array()
        .expect("objectKinds array")
        .iter()
        .map(|kind| {
            (
                kind.as_str().expect("object kind string").to_string(),
                std::collections::BTreeSet::<String>::new(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    for command in capabilities["commands"].as_array().expect("commands array") {
        let path = command["path"].as_str().expect("command path");
        for kind in command["targetObjectKinds"]
            .as_array()
            .unwrap_or_else(|| panic!("targetObjectKinds for {path} is not an array"))
        {
            let kind = kind.as_str().expect("target object kind string");
            expected
                .get_mut(kind)
                .unwrap_or_else(|| panic!("{path} advertises unknown object kind {kind}"))
                .insert(path.to_string());
        }
    }

    let actual = capabilities["objectKindsIndex"]
        .as_object()
        .expect("objectKindsIndex object");
    let expected_keys = expected.keys().cloned().collect::<BTreeSet<_>>();
    let actual_keys = actual.keys().cloned().collect::<BTreeSet<_>>();
    assert_eq!(actual_keys, expected_keys, "objectKindsIndex keys");

    for (kind, expected_paths) in expected {
        let actual_paths = actual[&kind]
            .as_array()
            .unwrap_or_else(|| panic!("objectKindsIndex[{kind}] is not an array"))
            .iter()
            .map(|path| {
                path.as_str()
                    .unwrap_or_else(|| panic!("objectKindsIndex[{kind}] path is not a string"))
                    .to_string()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_paths, expected_paths,
            "objectKindsIndex[{kind}] should be derived from command targetObjectKinds"
        );
    }
}

fn powershell_function_block(script: &str, start_marker: &str, end_marker: &str) -> String {
    let start = script
        .find(start_marker)
        .unwrap_or_else(|| panic!("missing PowerShell helper start marker {start_marker}"));
    let end = script[start..]
        .find(end_marker)
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("missing PowerShell helper end marker {end_marker}"));
    script[start..end].to_string()
}

fn powershell_for_windows_contract_test() -> Option<&'static str> {
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
