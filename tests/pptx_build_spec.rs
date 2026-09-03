use ooxml_cli::build::{BuildFamily, compile_pptx_spec, load_spec_file, load_spec_str};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const Q3_SPEC: &str = "testdata/pptx/build-spec/q3-review.json";
const Q3_GOLDEN: &str = include_str!("../testdata/golden/build-spec/pptx/q3-review-summary.json");

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn json_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "JSON stdout ({error}): {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ooxml-pptx-build-spec-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create test directory");
    path
}

#[test]
fn q3_recipe_compiles_to_the_expected_atomic_operation_sequence() {
    let spec = load_spec_file(BuildFamily::Pptx, Q3_SPEC).expect("load committed Q3 spec");
    let compiled = compile_pptx_spec(&spec).expect("compile committed Q3 spec");
    let commands = compiled
        .plan
        .operations
        .iter()
        .map(|operation| operation.command.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        [
            "pptx scaffold",
            "pptx new-slide-from-layout",
            "pptx slides delete",
            "pptx new-slide-from-layout",
            "pptx new-slide-from-layout",
            "pptx new-slide-from-layout",
            "pptx new-slide-from-layout",
            "pptx charts create",
            "pptx notes set",
            "pptx place table",
            "pptx place image",
            "pptx add-textbox",
        ]
    );
    assert_eq!(compiled.assets.len(), 2);
    assert_eq!(
        compiled.plan.operations[3].args["paragraphsFile"],
        "body:1=@generated/slide-002-bullets-000.json"
    );
    assert_eq!(
        compiled.plan.operations[7].args["valuesJson"],
        r#"[["","Actual"],["Q1",38.0],["Q2",41.0],["Q3",45.0]]"#
    );
}

#[test]
fn q3_recipe_builds_strict_sdk_clean_renderable_deterministic_deck() {
    let temp = temp_dir("q3");
    let first = temp.join("q3-first.pptx");
    let second = temp.join("q3-second.pptx");
    let first_path = first.to_str().expect("first output path");
    let second_path = second.to_str().expect("second output path");

    let built = run(&[
        "--json", "pptx", "build", "--spec", Q3_SPEC, "--out", first_path, "--check",
    ]);
    assert!(
        built.status.success(),
        "build stderr: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = json_stdout(&built);
    assert_eq!(result["validated"], true);
    assert_eq!(result["mutationEnvelope"]["opsCount"], 12);
    assert_eq!(result["check"]["summary"]["errors"], 0);
    assert_eq!(result["layoutQa"]["totalCollisions"], 0);
    assert_eq!(result["layoutQa"]["totalTextOverflows"], 0);
    assert_eq!(result["layoutQa"]["totalOffSlide"], 0);
    assert_eq!(result["layoutQa"]["totalSafeMarginViolations"], 0);
    assert_eq!(
        semantic_summary(&result),
        serde_json::from_str::<Value>(Q3_GOLDEN).unwrap()
    );

    let strict = run(&["--json", "validate", "--strict", first_path]);
    assert!(
        strict.status.success(),
        "strict stderr: {}",
        String::from_utf8_lossy(&strict.stderr)
    );

    let sdk = Path::new("/home/oliver/dotnet/dotnet");
    let validator = Path::new(
        "/home/oliver/Projects/odcpw/ooxml-cli/tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll",
    );
    if sdk.is_file() && validator.is_file() {
        let output = Command::new(sdk)
            .arg(validator)
            .arg(&first)
            .output()
            .expect("run Open XML SDK validator");
        assert!(
            output.status.success(),
            "SDK stdout: {}\nSDK stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("0 errors"));
    }

    let render_dir = temp.join("render");
    let render = run(&[
        "--json",
        "pptx",
        "render",
        first_path,
        "--out",
        render_dir.to_str().expect("render path"),
    ]);
    assert!(
        render.status.success(),
        "render stderr: {}",
        String::from_utf8_lossy(&render.stderr)
    );
    let render_result = json_stdout(&render);
    assert_eq!(render_result["status"], "ok");
    assert_eq!(render_result["slides"].as_array().unwrap().len(), 5);

    let second_build = run(&[
        "--json",
        "pptx",
        "build",
        "--spec",
        Q3_SPEC,
        "--out",
        second_path,
    ]);
    assert!(
        second_build.status.success(),
        "second build stderr: {}",
        String::from_utf8_lossy(&second_build.stderr)
    );
    assert_eq!(
        fs::read(&first).expect("read first deck"),
        fs::read(&second).expect("read second deck"),
        "identical specs must produce byte-identical presentations"
    );
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn dry_run_validates_the_full_staged_batch_without_publishing() {
    let temp = temp_dir("dry-run");
    let output = temp.join("planned.pptx");
    let run = run(&[
        "--json",
        "pptx",
        "build",
        "--spec",
        Q3_SPEC,
        "--out",
        output.to_str().unwrap(),
        "--dry-run",
    ]);
    assert!(
        run.status.success(),
        "dry-run stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let result = json_stdout(&run);
    assert_eq!(result["dryRun"], true);
    assert_eq!(result["validated"], true);
    assert_eq!(result["mutationEnvelope"]["committed"], false);
    assert_eq!(
        result["mutationEnvelope"]["plan"].as_array().unwrap().len(),
        12
    );
    assert_eq!(result["outline"], Value::Null);
    assert!(!output.exists());
    let serialized = serde_json::to_string(&result).unwrap();
    assert!(!serialized.contains(&format!("ooxml-pptx-build-{}", std::process::id())));
    assert!(serialized.contains("<build-stage>"));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn compiler_maps_rich_textbox_and_global_field_vocabulary_without_dropping_it() {
    let spec = load_spec_str(
        BuildFamily::Pptx,
        &json!({
            "schemaVersion": 1,
            "family": "pptx",
            "footer": "Internal",
            "slideNumbers": true,
            "slides": [{
                "layout": "Title Only",
                "title": "Details",
                "textBoxes": [{
                    "id": "callout",
                    "slot": "body",
                    "paragraphs": [{
                        "text": "Decision",
                        "bold": true,
                        "size": "18pt",
                        "color": "4472C4",
                        "align": "center"
                    }]
                }]
            }]
        })
        .to_string(),
    )
    .expect("rich spec loads");
    let compiled = compile_pptx_spec(&spec).expect("rich spec compiles");
    assert_eq!(
        compiled
            .plan
            .operations
            .iter()
            .map(|operation| operation.command.as_str())
            .collect::<Vec<_>>(),
        [
            "pptx scaffold",
            "pptx new-slide-from-layout",
            "pptx slides delete",
            "pptx add-textbox",
            "pptx fields set"
        ]
    );
    let paragraphs: Value = serde_json::from_slice(&compiled.assets[0].contents).unwrap();
    assert_eq!(paragraphs[0]["size"], 18.0);
    assert_eq!(compiled.plan.operations[4].args["showSlideNumber"], true);
}

fn semantic_summary(result: &Value) -> Value {
    let slides = result["outline"]["slides"]
        .as_array()
        .unwrap()
        .iter()
        .map(|slide| {
            json!({
                "number": slide["number"],
                "layout": slide["layout"],
                "title": slide["title"],
                "shapeCount": slide["shapeCount"],
                "tableCount": slide["tableCount"],
                "imageCount": slide["imageCount"],
                "notes": slide["notes"],
            })
        })
        .collect::<Vec<_>>();
    let commands = result["compiledPlan"]["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|operation| operation["command"].clone())
        .collect::<Vec<_>>();
    json!({
        "schemaVersion": result["schemaVersion"],
        "validated": result["validated"],
        "slides": slides,
        "operations": commands,
        "layoutQa": {
            "slidesWithIssues": result["layoutQa"]["slidesWithIssues"],
            "totalCollisions": result["layoutQa"]["totalCollisions"],
            "totalTextOverflows": result["layoutQa"]["totalTextOverflows"],
            "totalOffSlide": result["layoutQa"]["totalOffSlide"],
            "totalSafeMarginViolations": result["layoutQa"]["totalSafeMarginViolations"],
        },
        "checkErrors": result["check"]["summary"]["errors"],
        "nodeIds": [
            result["nodeMap"]["/slides/0"]["specId"],
            result["nodeMap"]["/slides/1"]["specId"],
            result["nodeMap"]["/slides/2"]["specId"],
            result["nodeMap"]["/slides/3"]["specId"],
            result["nodeMap"]["/slides/4"]["specId"],
        ]
    })
}
