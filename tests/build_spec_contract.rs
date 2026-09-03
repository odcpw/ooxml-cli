use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

const SOURCE_DATE_EPOCH: &str = "946684800";

#[derive(Clone, Copy)]
struct FamilyCase {
    family: &'static str,
    spec: &'static str,
    extension: &'static str,
    golden: &'static str,
}

const CASES: [FamilyCase; 3] = [
    FamilyCase {
        family: "pptx",
        spec: "testdata/pptx/build-spec/q3-review.json",
        extension: "pptx",
        golden: "testdata/golden/build-spec/contract/pptx-q3-review.json",
    },
    FamilyCase {
        family: "xlsx",
        spec: "testdata/xlsx/build-spec/sales.json",
        extension: "xlsx",
        golden: "testdata/golden/build-spec/contract/xlsx-sales.json",
    },
    FamilyCase {
        family: "docx",
        spec: "testdata/docx/build-spec/quarterly-report.json",
        extension: "docx",
        golden: "testdata/golden/build-spec/contract/docx-quarterly-report.json",
    },
];

fn run(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .env("SOURCE_DATE_EPOCH", SOURCE_DATE_EPOCH)
        .output()
        .expect("run ooxml")
}

fn run_ok(args: &[String], context: &str) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{context} failed\ncommand: ooxml {}\nstdout: {}\nstderr: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{context} returned invalid JSON ({error})\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ooxml-build-spec-contract-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create build-spec contract directory");
    path
}

fn path(path: &Path) -> String {
    path.to_str().expect("UTF-8 test path").to_string()
}

fn build(case: FamilyCase, spec: &Path, output: &Path, check: bool) -> Value {
    let mut args = vec![
        "--json".to_string(),
        case.family.to_string(),
        "build".to_string(),
        "--spec".to_string(),
        path(spec),
        "--out".to_string(),
        path(output),
    ];
    if check {
        args.push("--check".to_string());
    }
    run_ok(&args, &format!("{} representative build", case.family))
}

#[test]
fn representative_specs_pin_plans_outlines_validity_and_determinism() {
    for case in CASES {
        let temp = temp_dir(&format!("{}-golden", case.family));
        let first = temp.join(format!("first.{}", case.extension));
        let second = temp.join(format!("second.{}", case.extension));
        let report = build(case, Path::new(case.spec), &first, true);

        assert_eq!(report["validated"], true, "{}: {report}", case.family);
        assert_eq!(
            report["check"]["summary"]["errors"], 0,
            "{} check findings: {}\ncompiled plan: {}",
            case.family, report["check"], report["compiledPlan"]
        );
        if case.family == "pptx" {
            for field in [
                "totalCollisions",
                "totalTextOverflows",
                "totalOffSlide",
                "totalSafeMarginViolations",
            ] {
                assert_eq!(
                    report["layoutQa"][field], 0,
                    "PPTX layout QA findings: {}\ncompiled plan: {}",
                    report["layoutQa"], report["compiledPlan"]
                );
            }
        }
        assert_every_compiled_node_has_a_resolved_selector(case, &report);

        let contract = json!({
            "family": case.family,
            "compiledPlan": report["compiledPlan"],
            "nodeMap": report["nodeMap"],
            "outline": normalize_outline(report["outline"].clone()),
        });
        assert_golden(Path::new(case.golden), &contract);
        assert_package_proofs(case, &first);

        let second_report = build(case, Path::new(case.spec), &second, false);
        assert_eq!(
            second_report["validated"], true,
            "{} second build: {second_report}",
            case.family
        );
        assert_eq!(
            fs::read(&first).expect("read first package"),
            fs::read(&second).expect("read second package"),
            "{} builds differ under SOURCE_DATE_EPOCH={SOURCE_DATE_EPOCH}",
            case.family
        );
        fs::remove_dir_all(temp).expect("remove golden test directory");
    }
}

fn assert_every_compiled_node_has_a_resolved_selector(case: FamilyCase, report: &Value) {
    let compiled = report["compiledPlan"]["nodeMap"]
        .as_object()
        .expect("compiled node map");
    let resolved = report["nodeMap"].as_object().expect("resolved node map");
    assert_eq!(
        compiled.keys().collect::<BTreeSet<_>>(),
        resolved.keys().collect::<BTreeSet<_>>(),
        "{} resolved node paths drifted\ncompiled plan: {}",
        case.family,
        report["compiledPlan"]
    );
    for (spec_path, node) in resolved {
        assert!(
            node["selector"]
                .as_str()
                .is_some_and(|selector| !selector.is_empty()),
            "{} node {spec_path} has no selector: {node}\ncompiled plan: {}",
            case.family,
            report["compiledPlan"]
        );
    }

    let source: Value = serde_json::from_slice(
        &fs::read(case.spec).unwrap_or_else(|error| panic!("read {}: {error}", case.spec)),
    )
    .expect("parse committed representative spec");
    let mut spec_ids = BTreeSet::new();
    collect_spec_ids(&source, &mut spec_ids);
    let resolved_ids = resolved
        .values()
        .filter_map(|node| node["specId"].as_str())
        .collect::<BTreeSet<_>>();
    assert!(
        spec_ids.iter().all(|id| resolved_ids.contains(*id)),
        "{} spec ids are not all addressable: spec={spec_ids:?}, resolved={resolved_ids:?}",
        case.family
    );
}

fn collect_spec_ids<'a>(value: &'a Value, ids: &mut BTreeSet<&'a str>) {
    match value {
        Value::Object(object) => {
            if let Some(id) = object.get("id").and_then(Value::as_str) {
                ids.insert(id);
            }
            for child in object.values() {
                collect_spec_ids(child, ids);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_spec_ids(item, ids);
            }
        }
        _ => {}
    }
}

fn assert_package_proofs(case: FamilyCase, package: &Path) {
    let strict = run_ok(
        &[
            "--json".to_string(),
            "validate".to_string(),
            "--strict".to_string(),
            path(package),
        ],
        &format!("{} strict validation", case.family),
    );
    assert_eq!(strict["valid"], true, "{}: {strict}", case.family);

    let conformance = run_ok(
        &[
            "--json".to_string(),
            "conformance".to_string(),
            "check".to_string(),
            path(package),
            "--openxml-sdk".to_string(),
        ],
        &format!("{} conformance and SDK validation", case.family),
    );
    assert_eq!(
        conformance["summary"]["errors"], 0,
        "{} conformance findings: {conformance}",
        case.family
    );
    let checks = conformance["checks"]
        .as_array()
        .expect("conformance checks");
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "repo-validation" && check["status"] == "passed"),
        "{} repo validation proof missing: {conformance}",
        case.family
    );
    let schema = checks
        .iter()
        .find(|check| check["name"] == "schema")
        .expect("schema check");
    if schema["status"] == "skipped" {
        println!(
            "SKIP Open XML SDK {} build proof: {}",
            case.family, schema["diagnostics"]
        );
        assert_ne!(
            std::env::var("OOXML_REQUIRE_OPENXML_SDK").as_deref(),
            Ok("1"),
            "Open XML SDK was required but skipped: {schema}"
        );
    } else {
        assert_eq!(schema["status"], "passed", "{}: {schema}", case.family);
        assert_eq!(schema["schemaCheck"]["valid"], true, "{schema}");
        assert_eq!(schema["schemaCheck"]["errorCount"], 0, "{schema}");
    }
}

#[test]
fn apply_edit_and_modified_spec_rebuild_have_the_same_outline() {
    for case in CASES {
        let temp = temp_dir(&format!("{}-round-trip", case.family));
        let copied_spec = copy_representative_spec(case, &temp);
        let original = temp.join(format!("original.{}", case.extension));
        let edited = temp.join(format!("edited.{}", case.extension));
        let rebuilt = temp.join(format!("rebuilt.{}", case.extension));
        let report = build(case, &copied_spec, &original, false);
        let mut document: Value =
            serde_json::from_slice(&fs::read(&copied_spec).expect("read copied spec"))
                .expect("parse copied spec");
        let (operation, expected_prefix) = round_trip_edit(case, &report, &mut document);
        let ops = temp.join("edit-ops.json");
        write_json(&ops, &Value::Array(vec![operation]));
        let applied = run_ok(
            &[
                "--json".to_string(),
                "apply".to_string(),
                path(&original),
                "--ops".to_string(),
                path(&ops),
                "--out".to_string(),
                path(&edited),
            ],
            &format!("{} apply round-trip edit", case.family),
        );
        assert_eq!(applied["validated"], true, "{}: {applied}", case.family);

        write_json(&copied_spec, &document);
        build(case, &copied_spec, &rebuilt, false);
        let baseline_outline = package_outline(&original);
        let edited_outline = package_outline(&edited);
        let rebuilt_outline = package_outline(&rebuilt);
        let mut differences = Vec::new();
        collect_diff_paths(&baseline_outline, &edited_outline, "", &mut differences);
        assert!(
            !differences.is_empty()
                && differences
                    .iter()
                    .all(|path| path.starts_with(&expected_prefix)),
            "{} edit changed nodes outside {expected_prefix}: {differences:?}\nbaseline={baseline_outline}\nedited={edited_outline}",
            case.family
        );
        assert_eq!(
            edited_outline, rebuilt_outline,
            "{} apply edit and modified-spec rebuild diverged\napply={applied}\ncompiled plan={}",
            case.family, report["compiledPlan"]
        );
        assert_package_proofs(case, &edited);
        assert_package_proofs(case, &rebuilt);
        fs::remove_dir_all(temp).expect("remove round-trip test directory");
    }
}

fn copy_representative_spec(case: FamilyCase, temp: &Path) -> PathBuf {
    let relative = Path::new(case.spec);
    let destination = temp.join(relative);
    fs::create_dir_all(destination.parent().expect("spec parent")).expect("create spec parent");
    fs::copy(relative, &destination).expect("copy representative spec");
    if case.family == "xlsx" {
        fs::copy(
            "testdata/xlsx/build-spec/targets.csv",
            temp.join("testdata/xlsx/build-spec/targets.csv"),
        )
        .expect("copy XLSX data file");
    } else {
        fs::create_dir_all(temp.join("testdata")).expect("create asset directory");
        fs::copy(
            "testdata/test_image.png",
            temp.join("testdata/test_image.png"),
        )
        .expect("copy representative image");
    }
    destination
}

fn round_trip_edit(case: FamilyCase, report: &Value, document: &mut Value) -> (Value, String) {
    match case.family {
        "pptx" => {
            let spec_path = "/slides/3/images/0/caption";
            let node = &report["nodeMap"][spec_path];
            let selector = node["selector"].as_str().expect("caption selector");
            let slide = node["slide"].as_u64().expect("caption slide");
            document
                .pointer_mut(spec_path)
                .expect("caption spec node")
                .clone_from(&json!("Round-trip caption"));
            let outline = &report["outline"]["slides"][(slide - 1) as usize]["shapes"];
            let shape_index = outline
                .as_array()
                .expect("slide shapes")
                .iter()
                .position(|shape| shape["selector"] == selector)
                .expect("caption shape in outline");
            (
                json!({
                    "id": "round-trip-edit",
                    "command": "pptx text set",
                    "args": {"slide": slide, "target": selector, "text": "Round-trip caption"}
                }),
                format!("/slides/{}/shapes/{shape_index}", slide - 1),
            )
        }
        "xlsx" => {
            let spec_path = "/sheets/0/freeze";
            let selector = report["nodeMap"][spec_path]["selector"]
                .as_str()
                .expect("freeze selector");
            let sheet = selector.strip_prefix("sheet:").unwrap_or(selector);
            document
                .pointer_mut(spec_path)
                .expect("freeze spec node")
                .clone_from(&json!("B2"));
            (
                json!({
                    "id": "round-trip-edit",
                    "command": "xlsx freeze set",
                    "args": {"sheet": sheet, "rows": 1, "cols": 1}
                }),
                "/sheets/0/freeze".to_string(),
            )
        }
        "docx" => {
            let spec_path = "/blocks/12";
            let selector = report["nodeMap"][spec_path]["selector"]
                .as_str()
                .expect("paragraph selector");
            let index = selector
                .strip_prefix("block:")
                .and_then(|value| value.parse::<usize>().ok())
                .expect("block selector index");
            document
                .pointer_mut(&format!("{spec_path}/text"))
                .expect("paragraph text node")
                .clone_from(&json!("The round-trip edit is deterministic."));
            let blocks = report["outline"]["blocks"].as_array().expect("DOCX blocks");
            let block_index = blocks
                .iter()
                .position(|block| {
                    block["primarySelector"]
                        .as_str()
                        .and_then(|value| value.parse::<usize>().ok())
                        == Some(index)
                })
                .expect("paragraph in outline");
            (
                json!({
                    "id": "round-trip-edit",
                    "command": "docx paragraphs set",
                    "args": {"index": index, "text": "The round-trip edit is deterministic."}
                }),
                format!("/blocks/{block_index}"),
            )
        }
        other => panic!("unexpected family {other}"),
    }
}

fn package_outline(package: &Path) -> Value {
    let outline = run_ok(
        &[
            "--json".to_string(),
            "outline".to_string(),
            path(package),
            "--depth".to_string(),
            "3".to_string(),
            "--text-preview".to_string(),
            "240".to_string(),
        ],
        "package outline",
    );
    normalize_outline(outline)
}

fn normalize_outline(mut outline: Value) -> Value {
    let object = outline.as_object_mut().expect("outline object");
    object.remove("file");
    object.remove("fileSizeBytes");
    object.remove("checkCommand");
    object.remove("documentHash");
    remove_transient_handles(&mut outline);
    outline
}

fn remove_transient_handles(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("handle");
            for child in object.values_mut() {
                remove_transient_handles(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                remove_transient_handles(item);
            }
        }
        _ => {}
    }
}

fn collect_diff_paths(left: &Value, right: &Value, path: &str, differences: &mut Vec<String>) {
    match (left, right) {
        (Value::Object(left), Value::Object(right)) => {
            let keys = left.keys().chain(right.keys()).collect::<BTreeSet<_>>();
            for key in keys {
                let child = format!("{path}/{key}");
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        collect_diff_paths(left, right, &child, differences)
                    }
                    _ => differences.push(child),
                }
            }
        }
        (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                collect_diff_paths(left, right, &format!("{path}/{index}"), differences);
            }
        }
        _ if left != right => differences.push(path.to_string()),
        _ => {}
    }
}

#[test]
fn invalid_specs_report_exact_paths_suggestions_and_publish_nothing() {
    struct ErrorCase {
        name: &'static str,
        family: &'static str,
        fixture: &'static str,
        expected_exit: i32,
        code: &'static str,
        path: &'static str,
        suggestion: &'static str,
    }
    let cases = [
        ErrorCase {
            name: "unknown-field",
            family: "pptx",
            fixture: "testdata/build-spec-contract/invalid/unknown-field.json",
            expected_exit: 2,
            code: "BUILD_SPEC_UNKNOWN_FIELD",
            path: "/slieds",
            suggestion: "slides",
        },
        ErrorCase {
            name: "wrong-type",
            family: "xlsx",
            fixture: "testdata/build-spec-contract/invalid/wrong-type.json",
            expected_exit: 2,
            code: "BUILD_SPEC_TYPE_MISMATCH",
            path: "/sheets",
            suggestion: "expected array",
        },
        ErrorCase {
            name: "missing-file",
            family: "pptx",
            fixture: "testdata/build-spec-contract/invalid/missing-file.json",
            expected_exit: 3,
            code: "BUILD_SPEC_FILE_READ_FAILED",
            path: "/slides/0/images/0/path",
            suggestion: "existing image path relative to the build spec file",
        },
        ErrorCase {
            name: "bad-style",
            family: "pptx",
            fixture: "testdata/build-spec-contract/invalid/bad-style.json",
            expected_exit: 2,
            code: "BUILD_SPEC_VALUE_INVALID",
            path: "/slides/0/tables/0/style",
            suggestion: "use Light1 or Medium2",
        },
        ErrorCase {
            name: "bad-layout",
            family: "pptx",
            fixture: "testdata/build-spec-contract/invalid/bad-layout.json",
            expected_exit: 2,
            code: "BUILD_SPEC_VALUE_INVALID",
            path: "/slides/0/layout",
            suggestion: "ooxml pptx layouts list",
        },
    ];

    let temp = temp_dir("invalid");
    for case in cases {
        let output_path = temp.join(format!("{}.{}", case.name, case.family));
        let output = run(&[
            "--json".to_string(),
            case.family.to_string(),
            "build".to_string(),
            "--spec".to_string(),
            case.fixture.to_string(),
            "--out".to_string(),
            path(&output_path),
        ]);
        assert_eq!(
            output.status.code(),
            Some(case.expected_exit),
            "{} stdout={} stderr={}",
            case.name,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let outer: Value = serde_json::from_slice(&output.stderr)
            .unwrap_or_else(|error| panic!("{} JSON error envelope: {error}", case.name));
        let message = outer["error"]["message"]
            .as_str()
            .unwrap_or_else(|| panic!("{} missing error message: {outer}", case.name));
        let detail: Value = serde_json::from_str(message).unwrap_or_else(|error| {
            panic!(
                "{} structured build diagnostic: {error}: {message}",
                case.name
            )
        });
        let diagnostic = detail
            .get("diagnostics")
            .and_then(Value::as_array)
            .and_then(|diagnostics| {
                diagnostics.iter().find(|diagnostic| {
                    diagnostic["code"] == case.code && diagnostic["path"] == case.path
                })
            })
            .unwrap_or(&detail);
        assert_eq!(diagnostic["code"], case.code, "{}: {detail}", case.name);
        assert_eq!(diagnostic["path"], case.path, "{}: {detail}", case.name);
        assert!(
            diagnostic.to_string().contains(case.suggestion),
            "{} lacks suggestion {:?}: {detail}",
            case.name,
            case.suggestion
        );
        assert!(
            !output_path.exists(),
            "{} published a partial output at {}",
            case.name,
            output_path.display()
        );
    }
    fs::remove_dir_all(temp).expect("remove invalid-spec test directory");
}

#[test]
fn xlsx_two_hundred_thousand_cells_stays_within_the_build_budget() {
    const ROWS: usize = 2_000;
    const COLUMNS: usize = 100;
    const CELLS: usize = ROWS * COLUMNS;
    let temp = temp_dir("xlsx-200k");
    let data = temp.join("data.csv");
    let mut csv = String::with_capacity(CELLS * 4);
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            if column > 0 {
                csv.push(',');
            }
            if row == 0 {
                csv.push_str(&format!("C{}", column + 1));
            } else {
                csv.push_str(&((row * COLUMNS + column) % 10_000).to_string());
            }
        }
        csv.push('\n');
    }
    fs::write(&data, csv).expect("write 200,000-cell CSV");
    let columns = (0..COLUMNS)
        .map(|index| json!({"name": format!("C{}", index + 1), "type": "number"}))
        .collect::<Vec<_>>();
    let spec = temp.join("large.json");
    write_json(
        &spec,
        &json!({
            "schemaVersion": 1,
            "family": "xlsx",
            "sheets": [{
                "name": "Data",
                "dataFile": {"path": "data.csv", "format": "csv"},
                "columns": columns
            }]
        }),
    );
    let output = temp.join("large.xlsx");
    let budget = std::env::var("OOXML_BUILD_200K_BUDGET_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(60));
    let started = Instant::now();
    let report = build(
        FamilyCase {
            family: "xlsx",
            spec: "",
            extension: "xlsx",
            golden: "",
        },
        &spec,
        &output,
        false,
    );
    let elapsed = started.elapsed();
    assert_eq!(report["validated"], true, "large build: {report}");
    assert_eq!(
        report["outline"]["sheets"][0]["cellCount"], CELLS,
        "large build did not materialize the full matrix: {report}"
    );
    assert!(
        elapsed <= budget,
        "200,000-cell XLSX build exceeded {:?}: elapsed={elapsed:?}\ncompiled plan={}\noutline={}",
        budget,
        report["compiledPlan"],
        report["outline"]
    );
    let strict = run_ok(
        &[
            "--json".to_string(),
            "validate".to_string(),
            "--strict".to_string(),
            path(&output),
        ],
        "large XLSX strict validation",
    );
    assert_eq!(strict["valid"], true, "large XLSX: {strict}");
    fs::remove_dir_all(temp).expect("remove large XLSX test directory");
}

fn assert_golden(golden: &Path, value: &Value) {
    let mut rendered = serde_json::to_string_pretty(value).expect("serialize contract golden");
    rendered.push('\n');
    if std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1") {
        fs::create_dir_all(golden.parent().expect("golden parent"))
            .expect("create golden directory");
        fs::write(golden, &rendered).expect("write reviewed build-spec contract golden");
    }
    let expected = fs::read_to_string(golden).unwrap_or_else(|error| {
        panic!(
            "missing {}: {error}; rerun with UPDATE_GOLDENS=1",
            golden.display()
        )
    });
    assert_eq!(rendered, expected, "{} drifted", golden.display());
}

fn write_json(path: &Path, value: &Value) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("serialize JSON fixture");
    bytes.push(b'\n');
    fs::write(path, bytes).expect("write JSON fixture");
}
